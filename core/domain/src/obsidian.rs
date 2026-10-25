use super::*;
use sha2::{Digest, Sha256};

const REPOSITORY: &str = "https://github.com/gab8192/Obsidian";
const NETWORK_BASE: &str = "https://github.com/gab8192/Obsidian-nets/releases/download/nets";
pub(super) fn defaults() -> BuildOptions {
    BuildOptions {
        engine: "obsidian".into(),
        source_ref: "v16.0".into(),
        pgo: false,
        auto_architecture: true,
        architecture: hardware::detect().obsidian_recommended,
        ..BuildOptions::default()
    }
}

fn downloader(root: &Path) -> bool {
    ["usr/bin/curl.exe"].iter().any(|p| root.join(p).is_file())
}

pub(super) fn plan(o: &BuildOptions) -> Result<Plan> {
    if !hardware::detect()
        .obsidian_choices
        .contains(&o.architecture)
    {
        return Err(
            "This CPU cannot run the selected Obsidian target. Select native or sse2.".into(),
        );
    }
    let root = Path::new(&o.msys_root);
    if !toolchain_valid(root) {
        return Err("MSYS2 UCRT64 is incomplete. Select its root folder; install make, git, wget and mingw-w64-ucrt-x86_64-gcc in MSYS2.".into());
    }
    if !downloader(root) || !root.join("ucrt64/bin/gcc.exe").is_file() {
        return Err("MSYS2 needs curl and UCRT64 GCC to download Obsidian's network and compile its source.".into());
    }
    if git_executable(root).is_none() {
        return Err(
            "Git was not found. Install Git for Windows or MSYS2 git, then restart the app.".into(),
        );
    }
    Ok(Plan { options: o.clone(), steps: vec![
        format!("Fetch gab8192/Obsidian at {} into a new job folder", o.source_ref),
        "Record the resolved commit, GCC version and network SHA-256".into(),
        "Download the network named by the source and embed it in a static executable".into(),
        format!("Build with MSYS2 UCRT64 GCC, build={}, jobs={}, PGO={}", o.architecture, o.jobs, o.pgo),
        "Verify UCI and a standalone benchmark with only System32 on PATH; archive source, license, build manifest and full log".into(),
    ] })
}

pub(super) fn build(
    o: &BuildOptions,
    job: &Path,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<String> {
    let root = Path::new(&o.msys_root);
    let path = std::env::join_paths(
        [root.join("ucrt64/bin"), root.join("usr/bin")]
            .into_iter()
            .chain(std::env::split_paths(
                &std::env::var_os("PATH").unwrap_or_default(),
            )),
    )
    .map_err(|e| e.to_string())?;
    let git = git_executable(root).ok_or("Git was not found.")?;
    let source = job.join("source");
    fs::create_dir(&source).map_err(|e| e.to_string())?;
    let git_run = |args: &[&str]| {
        run(
            Command::new(&git)
                .args(["-c", "core.hooksPath=NUL"])
                .args(args)
                .current_dir(&source)
                .env("PATH", &path)
                .env("GIT_TERMINAL_PROMPT", "0"),
            shared,
            cancel,
            Duration::from_secs(600),
            None,
        )
    };
    phase(shared, "Fetch Obsidian source");
    git_run(&["init", "--quiet"])?;
    git_run(&["remote", "add", "origin", &format!("{REPOSITORY}.git")])?;
    git_run(&["fetch", "--depth=1", "origin", &o.source_ref])?;
    git_run(&["checkout", "--detach", "FETCH_HEAD"])?;
    let commit = git_run(&["rev-parse", "HEAD"])?;
    let src = source.clone();
    let make = |args: &[&str]| {
        run(
            Command::new(root.join("usr/bin/bash.exe"))
                .args(["--noprofile", "--norc", "-c", "exec make \"$@\"", "cec"])
                .args(args)
                .current_dir(&src)
                .env("PATH", &path)
                .env("MSYSTEM", "UCRT64")
                .env("CHERE_INVOKING", "1")
                .env("OS", "Windows_NT"),
            shared,
            cancel,
            Duration::from_secs(7200),
            None,
        )
    };
    phase(shared, "Check compiler");
    let compiler = run(
        Command::new(root.join("ucrt64/bin/gcc.exe"))
            .arg("--version")
            .env("PATH", &path),
        shared,
        cancel,
        Duration::from_secs(30),
        None,
    )?;
    phase(shared, "Download embedded network");
    let makefile = fs::read_to_string(src.join("Makefile")).map_err(|e| e.to_string())?;
    let network = network_name(&makefile)?;
    let network_url = format!("{NETWORK_BASE}/{network}");
    run(
        Command::new(root.join("usr/bin/curl.exe"))
            .args(["--fail", "--location", "--retry", "2", "--output"])
            .arg(src.join(&network))
            .arg(&network_url)
            .env("PATH", &path),
        shared,
        cancel,
        Duration::from_secs(1200),
        None,
    )?;
    let mut file = File::open(src.join(&network)).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() == 0 {
        return Err("The network download is empty.".into());
    }
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    let network_hash = format!("{:x}", hasher.finalize());
    // Override the upstream flags to honor symbol retention and avoid its
    // Windows PGO directory bug. Architecture flags remain upstream-owned.
    let original_flags = makefile
        .lines()
        .find_map(|line| line.strip_prefix("FLAGS = "))
        .ok_or("The selected source has an unsupported Makefile (FLAGS).")?;
    if !original_flags.contains("-s ") {
        return Err("Unsupported Obsidian compiler flags.".into());
    }
    // Keep make variable expansion, including the source-selected network and
    // optimization flags. Never construct a shell command from user input.
    let base_flags = original_flags.replacen("-s ", "", 1) + " -static";
    let arch = format!("build={}", o.architecture);
    let jobs = format!("-j{}", o.jobs);
    let compile = |profile: &str| {
        let flags = format!(
            "FLAGS={base_flags} {profile} {}",
            architecture_flags(&o.architecture)
        );
        make(&["-B", "nopgo", &jobs, &arch, &flags])
    };
    if o.pgo {
        phase(shared, "PGO: build instrumented engine");
        compile("-fprofile-generate")?;
        phase(shared, "PGO: run Obsidian benchmark");
        let training = run(
            Command::new(src.join("Obsidian.exe"))
                .arg("bench")
                .current_dir(&src)
                .env("PATH", &path),
            shared,
            cancel,
            Duration::from_secs(1800),
            None,
        )?;
        if !bench_verified(&training) {
            return Err("Obsidian profiling benchmark did not finish.".into());
        }
        phase(shared, "PGO: rebuild using profiles");
        let rebuild = compile("-fprofile-use -Werror=missing-profile")?;
        if rebuild.contains("profile count data file not found") {
            return Err("GCC did not apply Obsidian's profiles.".into());
        }
    } else {
        phase(shared, "Compile Obsidian");
        compile("")?;
    }
    let executable = src.join("Obsidian.exe");
    if o.strip {
        phase(shared, "Strip executable symbols");
        run(
            Command::new(root.join("ucrt64/bin/strip.exe"))
                .arg(&executable)
                .env("PATH", &path),
            shared,
            cancel,
            Duration::from_secs(60),
            None,
        )?;
    }
    phase(shared, "Save build result");
    let result = job.join("result");
    fs::create_dir(&result).map_err(|e| e.to_string())?;
    let output = result.join("Obsidian.exe");
    fs::copy(&executable, &output).map_err(|e| e.to_string())?;
    let system_path =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:/Windows".into()))
            .join("System32");
    phase(shared, "Verify standalone UCI handshake");
    let uci = run(
        Command::new(&output)
            .current_dir(&result)
            .env("PATH", &system_path),
        shared,
        cancel,
        Duration::from_secs(60),
        Some("uci\nisready\nquit\n"),
    )?;
    if !uci.lines().any(|s| s.trim() == "uciok") || !uci.lines().any(|s| s.trim() == "readyok") {
        return Err(
            "Obsidian did not complete the standalone UCI handshake. Review the log.".into(),
        );
    }
    phase(shared, "Verify standalone search");
    let bench = run(
        Command::new(&output)
            .arg("bench")
            .current_dir(&result)
            .env("PATH", &system_path),
        shared,
        cancel,
        Duration::from_secs(300),
        None,
    )?;
    if !bench_verified(&bench) {
        return Err("Obsidian did not finish its standalone search benchmark.".into());
    }
    phase(shared, "Archive source and build provenance");
    git_run(&[
        "archive",
        "--format=tar",
        "--output=../result/source.tar",
        "HEAD",
    ])?;
    if source.join("LICENSE").is_file() {
        fs::copy(source.join("LICENSE"), result.join("LICENSE")).map_err(|e| e.to_string())?;
    }
    let manifest = serde_json::json!({ "schemaVersion": 2, "engine": "obsidian", "sourceUrl": REPOSITORY, "commit": commit.trim(), "options": o, "compiler": compiler, "network": network, "networkSha256": network_hash, "networkUrl": network_url, "networkEmbedded": true, "networkDigestVerifiedAgainstUpstream": false, "pgoProfilesApplied": o.pgo, "uciVerified": true, "standaloneSearchVerified": true, "logPath": job.join("build.log"), "sourceArchive": "source.tar" });
    fs::write(
        result.join("build.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().into_owned())
}

fn network_name(makefile: &str) -> Result<String> {
    let name = makefile
        .lines()
        .find_map(|line| line.strip_prefix("DEFAULT_NET = "))
        .ok_or("The selected Obsidian source does not name a supported default network.")?
        .trim();
    if name.is_empty()
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
        || name.starts_with('.')
    {
        return Err("Unsupported Obsidian network file name.".into());
    }
    Ok(name.into())
}
fn architecture_flags(arch: &str) -> &'static str {
    match arch {
        "native" => "-march=native", // automatic recommendation selects explicit targets
        "sse2" => "-m64 -mpopcnt -msse -msse2",
        "ssse3" => "-m64 -mpopcnt -msse -msse2 -mssse3",
        "avx2" => "-m64 -mpopcnt -mssse3 -msse4.1 -mbmi -mfma -mavx2",
        "avx2-pext" => "-m64 -mpopcnt -mssse3 -msse4.1 -mbmi -mfma -mavx2 -DUSE_PEXT -mbmi2",
        "avx512" => {
            "-m64 -mpopcnt -mssse3 -msse4.1 -mbmi -mfma -mavx2 -mavx512f -mavx512bw -DUSE_PEXT -mbmi2"
        }
        _ => unreachable!("architecture was validated before execution"),
    }
}
fn bench_verified(output: &str) -> bool {
    output.lines().any(|line| {
        let fields: Vec<_> = line.split_whitespace().collect();
        fields.len() == 4
            && fields[1] == "nodes"
            && fields[3] == "nps"
            && fields[0].parse::<u64>().is_ok_and(|n| n > 0)
            && fields[2].parse::<u64>().is_ok_and(|n| n > 0)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rejects_unsafe_networks_and_incomplete_search() {
        assert_eq!(
            network_name("DEFAULT_NET = net87perm.bin").unwrap(),
            "net87perm.bin"
        );
        for name in ["../escape", "$(shell calc)", "net;calc", ""] {
            assert!(network_name(&format!("DEFAULT_NET = {name}")).is_err());
        }
        assert!(bench_verified("Obsidian\n123 nodes 456 nps"));
        assert!(!bench_verified("0 nodes 456 nps"));
        assert!(!bench_verified("uciok\nreadyok"));
    }
}
