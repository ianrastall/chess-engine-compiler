use super::*;
use sha2::{Digest, Sha256};

const REPOSITORY: &str = "https://github.com/Witek902/Caissa";
const NETWORK_BASE: &str = "https://github.com/Witek902/Caissa-Nets/releases/download";
pub(super) fn defaults() -> BuildOptions {
    BuildOptions {
        engine: "caissa".into(),
        source_ref: "2.0".into(),
        pgo: false,
        auto_architecture: true,
        architecture: hardware::detect().cpp_recommended,
        ..BuildOptions::default()
    }
}

fn downloader(root: &Path) -> bool {
    ["usr/bin/curl.exe"].iter().any(|p| root.join(p).is_file())
}

pub(super) fn plan(o: &BuildOptions) -> Result<Plan> {
    if o.pgo {
        return Err("Caissa PGO is not supported by this adapter. Turn PGO off.".into());
    }
    if !hardware::detect().cpp_choices.contains(&o.architecture) {
        return Err(
            "This CPU cannot run the selected Caissa target. Select x86-64 or a compatible AVX2 target.".into(),
        );
    }
    let root = Path::new(&o.msys_root);
    if !toolchain_valid(root) {
        return Err("MSYS2 UCRT64 is incomplete. Select its root folder; install make, git, curl and mingw-w64-ucrt-x86_64-gcc in MSYS2.".into());
    }
    if !downloader(root) || !root.join("ucrt64/bin/gcc.exe").is_file() {
        return Err(
            "MSYS2 needs curl and UCRT64 GCC to download Caissa's network and compile its source."
                .into(),
        );
    }
    if git_executable(root).is_none() {
        return Err(
            "Git was not found. Install Git for Windows or MSYS2 git, then restart the app.".into(),
        );
    }
    Ok(Plan { options: o.clone(), steps: vec![
        format!("Fetch Witek902/Caissa at {} into a new job folder", o.source_ref),
        "Record the resolved commit, GCC version and network SHA-256".into(),
        "Download the network named by the source and embed it in a static executable".into(),
        format!("Regular MSYS2 UCRT64 GCC build, target={}, jobs={}; PGO is unavailable", o.architecture, o.jobs),
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
    phase(shared, "Fetch Caissa source");
    git_run(&["init", "--quiet"])?;
    git_run(&["remote", "add", "origin", &format!("{REPOSITORY}.git")])?;
    git_run(&["fetch", "--depth=1", "origin", &o.source_ref])?;
    git_run(&["checkout", "--detach", "FETCH_HEAD"])?;
    let commit = git_run(&["rev-parse", "HEAD"])?;
    let src = source.join("src");
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
    let version = network
        .strip_suffix(".pnn")
        .ok_or("Unsupported Caissa network extension.")?;
    let network_url = format!("{NETWORK_BASE}/{version}/{network}");
    let network_dir = source.join("data/neuralNets");
    fs::create_dir_all(&network_dir).map_err(|e| e.to_string())?;
    run(
        Command::new(root.join("usr/bin/curl.exe"))
            .args(["--fail", "--location", "--retry", "2", "--output"])
            .arg(network_dir.join(&network))
            .arg(&network_url)
            .env("PATH", &path),
        shared,
        cancel,
        Duration::from_secs(1200),
        None,
    )?;
    let mut file = File::open(network_dir.join(&network)).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() == 0 {
        return Err("The network download is empty.".into());
    }
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    let network_hash = format!("{:x}", hasher.finalize());
    let network_digest_verified = network == "eval-ml-8-152B-spsa.pnn";
    if network_digest_verified
        && network_hash != "ee9f6bbb0ef6cd1f38a13cf095340968c2ad18d0fa1359d493043e61acc829fa"
    {
        return Err("Caissa network SHA-256 does not match its published digest.".into());
    }
    let target_flags = match o.architecture.as_str() {
        "x86-64" => "$(COMMONFLAGS) -DUSE_SSE -DUSE_SSE2 -march=x86-64",
        "avx2" => {
            "$(COMMONFLAGS) -DUSE_SSE -DUSE_SSE2 -DUSE_SSE4 -DUSE_POPCNT -DUSE_AVX -DUSE_AVX2 -msse4.2 -mpopcnt -mavx2 -mfma -mbmi"
        }
        "bmi2" => {
            "$(COMMONFLAGS) -DUSE_SSE -DUSE_SSE2 -DUSE_SSE4 -DUSE_POPCNT -DUSE_AVX -DUSE_AVX2 -DUSE_BMI2 -msse4.2 -mpopcnt -mavx2 -mfma -mbmi -mbmi2"
        }
        _ => return Err("Unsupported Caissa target.".into()),
    };
    phase(shared, "Compile Caissa (regular GCC build)");
    make(&[
        "ob",
        "CC=g++",
        &format!("-j{}", o.jobs),
        &format!("NATIVEFLAGS={target_flags}"),
    ])?;
    let executable = src.join("caissa.exe");
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
    let output = result.join("caissa.exe");
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
        return Err("Caissa did not complete the standalone UCI handshake. Review the log.".into());
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
        return Err("Caissa did not finish its standalone search benchmark.".into());
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
    let manifest = serde_json::json!({ "schemaVersion": 2, "engine": "caissa", "sourceUrl": REPOSITORY, "commit": commit.trim(), "options": o, "compiler": compiler, "network": network, "networkSha256": network_hash, "networkUrl": network_url, "networkEmbedded": true, "networkDigestVerifiedAgainstUpstream": network_digest_verified, "pgoProfilesApplied": o.pgo, "uciVerified": true, "standaloneSearchVerified": true, "logPath": job.join("build.log"), "sourceArchive": "source.tar" });
    fs::write(
        result.join("build.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().into_owned())
}

fn network_name(makefile: &str) -> Result<String> {
    let path = makefile
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once('=')?;
            (key.trim() == "DEFAULT_EVALFILE").then(|| value.trim())
        })
        .ok_or("The selected Caissa source does not name a supported default network.")?;
    let name = path
        .strip_prefix("../data/neuralNets/")
        .ok_or("Unsupported Caissa network path.")?;
    if name.is_empty()
        || name.starts_with('.')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    {
        return Err("Unsupported Caissa network name.".into());
    }
    Ok(name.into())
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
    fn requires_safe_network_and_completed_benchmark() {
        assert_eq!(
            network_name("DEFAULT_EVALFILE = ../data/neuralNets/test.pnn").unwrap(),
            "test.pnn"
        );
        assert!(network_name("DEFAULT_EVALFILE = ../data/neuralNets/../escape").is_err());
        assert!(bench_verified("123 nodes 456 nps"));
        assert!(!bench_verified("0 nodes 456 nps"));
    }
}
