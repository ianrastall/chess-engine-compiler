use super::*;
use sha2::{Digest, Sha256};

const REPOSITORY: &str = "https://github.com/jhonnold/berserk";
const NETWORK_BASE: &str =
    "https://github.com/jhonnold/berserk-networks/releases/download/networks";
// Upstream adds -static only when its shell keeps quotes from echo (cmd.exe).
// MSYS2 bash does not, so request the static runtime explicitly.
const LIBS: &str = "LIBS=-pthread -lm -static";

pub(super) fn defaults() -> BuildOptions {
    BuildOptions {
        engine: "berserk".into(),
        source_ref: "14".into(),
        // GCC PGO applied its profiles but measured about 3.5% slower than a
        // regular build for Berserk 14 (docs/berserk.md), so it is opt-in.
        pgo: false,
        auto_architecture: true,
        architecture: hardware::detect().berserk_recommended,
        ..BuildOptions::default()
    }
}

fn downloader(root: &Path) -> bool {
    ["usr/bin/wget.exe", "usr/bin/curl.exe"]
        .iter()
        .any(|p| root.join(p).is_file())
}

pub(super) fn plan(o: &BuildOptions) -> Result<Plan> {
    if !hardware::detect().berserk_choices.contains(&o.architecture) {
        return Err(
            "This CPU cannot run the selected Berserk target. Select native or x86-64.".into(),
        );
    }
    let root = Path::new(&o.msys_root);
    if !toolchain_valid(root) {
        return Err("MSYS2 UCRT64 is incomplete. Select its root folder; install make, git, wget and mingw-w64-ucrt-x86_64-gcc in MSYS2.".into());
    }
    if !downloader(root) || !root.join("usr/bin/sha256sum.exe").is_file() {
        return Err("MSYS2 needs wget (or curl) and sha256sum to download and verify Berserk's network. Run pacman -S --needed wget coreutils.".into());
    }
    if git_executable(root).is_none() {
        return Err(
            "Git was not found. Install Git for Windows or MSYS2 git, then restart the app.".into(),
        );
    }
    let arch = &o.architecture;
    Ok(Plan { options: o.clone(), steps: vec![
        format!("Fetch jhonnold/berserk at {} into a new job folder", o.source_ref),
        "Record the resolved commit and GCC version".into(),
        "Download the network named by that source and verify its SHA-256 against the file name; it is embedded in the executable".into(),
        if o.pgo {
            format!("Build an instrumented engine (GCC, ARCH={arch}, static runtime), run berserk bench 13 for profiles, then rebuild using them. A build without applied profiles fails.")
        } else {
            format!("make all CC=gcc ARCH={arch} with the static runtime (one compiler invocation; parallel jobs do not apply)")
        },
        if o.strip { "Strip executable symbols".into() } else { "Keep executable symbols".into() },
        "Verify UCI and a standalone search from the result with a clean runtime PATH; save executable, source archive, license, manifest and full log".into(),
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
    phase(shared, "Fetch Berserk source");
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
    phase(shared, "Download and verify network");
    make(&["download-network"])?;
    let networks: Vec<_> = fs::read_dir(&src)
        .map_err(|e| e.to_string())?
        .filter_map(|e| e.ok())
        .map(|e| e.file_name().to_string_lossy().into_owned())
        .filter(|n| n.starts_with("berserk-") && n.ends_with(".nn"))
        .collect();
    let [network] = networks.as_slice() else {
        return Err(
            "Berserk's network download did not produce exactly one network file. Review the log."
                .into(),
        );
    };
    let mut file = File::open(src.join(network)).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    let network_hash = format!("{:x}", hasher.finalize());
    // Berserk names each network by the first 12 hex digits of its SHA-256.
    if *network != format!("berserk-{}.nn", &network_hash[..12]) {
        return Err(format!(
            "The downloaded network {network} does not match its SHA-256 ({network_hash})."
        ));
    }
    let arch = format!("ARCH={}", o.architecture);
    let common = [arch.as_str(), "CC=gcc", LIBS];
    if o.pgo {
        // Upstream's pgo target passes -fprofile-generate=pgo. GCC on Windows
        // cannot read those mangled profile paths back, which silently yields
        // an unoptimized build. Without a directory, profiles sit beside the
        // sources and are found.
        phase(shared, "PGO: build instrumented engine");
        make(&[&["all", "PGOFLAGS=-fprofile-generate"][..], &common[..]].concat())?;
        phase(shared, "PGO: run berserk bench 13 for profiles");
        let training = run(
            Command::new(src.join("berserk.exe"))
                .args(["bench", "13"])
                .current_dir(&src)
                .env("PATH", &path),
            shared,
            cancel,
            Duration::from_secs(1800),
            None,
        )?;
        if !training.contains("Results:") {
            return Err(
                "The instrumented engine did not finish its profiling benchmark. Review the log."
                    .into(),
            );
        }
        phase(shared, "PGO: rebuild using profiles");
        let rebuild = make(&[&["all", "PGOFLAGS=-fprofile-use"][..], &common[..]].concat())?;
        if rebuild.contains("profile count data file not found") {
            return Err("GCC did not apply the PGO profiles, so the result would not be profile-optimized. Review the log.".into());
        }
    } else {
        phase(shared, "Compile Berserk");
        make(&[&["all"][..], &common[..]].concat())?;
    }
    let executable = src.join("berserk.exe");
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
    let output = result.join("berserk.exe");
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
            "Berserk did not complete the standalone UCI handshake. Review the log.".into(),
        );
    }
    phase(shared, "Verify standalone search");
    let bench = run(
        Command::new(&output)
            .args(["bench", "10"])
            .current_dir(&result)
            .env("PATH", &system_path),
        shared,
        cancel,
        Duration::from_secs(300),
        None,
    )?;
    if !bench.lines().any(|line| {
        line.trim()
            .strip_prefix("Results:")
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse::<u64>().ok())
            .is_some_and(|n| n > 0)
    }) {
        return Err(
            "The standalone engine did not finish its search benchmark. Review the log.".into(),
        );
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
    let manifest = serde_json::json!({ "schemaVersion": 2, "engine": "berserk", "sourceUrl": REPOSITORY, "commit": commit.trim(), "options": o, "compiler": compiler, "network": network, "networkSha256": network_hash, "networkUrl": format!("{NETWORK_BASE}/{network}"), "networkEmbedded": true, "pgoProfilesApplied": o.pgo, "uciVerified": true, "standaloneSearchVerified": true, "logPath": job.join("build.log"), "sourceArchive": "source.tar" });
    fs::write(
        result.join("build.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_berserk_targets_and_tools() {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "usr/bin/bash.exe",
            "usr/bin/make.exe",
            "ucrt64/bin/g++.exe",
            "usr/bin/git.exe",
            "usr/bin/wget.exe",
            "usr/bin/sha256sum.exe",
        ] {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, []).unwrap();
        }
        let mut o = BuildOptions {
            msys_root: dir.path().to_string_lossy().into(),
            output_root: dir.path().join("out").to_string_lossy().into(),
            ..defaults()
        };
        assert_eq!(o.engine, "berserk");
        assert!(super::super::plan(&o).is_ok());
        o.auto_architecture = false;
        o.architecture = "x86-64-avx2".into();
        assert!(plan(&o).unwrap_err().contains("Berserk target"));
        o.architecture = "x86-64".into();
        assert!(plan(&o).is_ok());
        fs::remove_file(dir.path().join("usr/bin/wget.exe")).unwrap();
        assert!(plan(&o).unwrap_err().contains("wget"));
    }
}
