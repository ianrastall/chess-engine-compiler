use super::*;
use sha2::{Digest, Sha256};

const REPOSITORY: &str = "https://github.com/codedeliveryservice/Reckless";
const NETWORK_BASE: &str =
    "https://github.com/codedeliveryservice/RecklessNetworks/releases/download/networks";
pub(super) fn defaults() -> BuildOptions {
    BuildOptions {
        engine: "reckless".into(),
        source_ref: "v0.9.0".into(),
        pgo: false,
        auto_architecture: true,
        architecture: hardware::detect().reckless_recommended,
        ..BuildOptions::default()
    }
}

fn tool(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|p| p.join(name))
        .find(|p| p.is_file())
}
pub(super) fn plan(o: &BuildOptions) -> Result<Plan> {
    if o.pgo {
        return Err("Reckless PGO is not supported by this adapter. Turn PGO off.".into());
    }
    if !hardware::detect()
        .reckless_choices
        .contains(&o.architecture)
    {
        return Err("Select a compatible Reckless target.".into());
    }
    if tool("cargo.exe").is_none()
        || tool("rustc.exe").is_none()
        || tool("curl.exe").is_none()
        || git_executable(Path::new("")).is_none()
    {
        return Err("Install Rust (Cargo/rustc), Git for Windows and curl on PATH, plus MSVC x64 C++ build tools and a Windows SDK. Restart the app after changing PATH.".into());
    }
    Ok(Plan { options: o.clone(), steps: vec![
        format!("Fetch codedeliveryservice/Reckless at {} into a new job", o.source_ref),
        "Record Rust/Cargo versions and source commit".into(),
        "Build the locked release with static MSVC CRT and the source-selected embedded network; Syzygy and PGO are not included".into(),
        "Verify UCI and a standalone benchmark with only System32 on PATH; save source archive, license, network SHA-256, options and full log".into(),
    ] })
}
pub(super) fn build(
    o: &BuildOptions,
    job: &Path,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<String> {
    let git = git_executable(Path::new("")).ok_or("Git was not found.")?;
    let source = job.join("source");
    fs::create_dir(&source).map_err(|e| e.to_string())?;
    let git_run = |args: &[&str]| {
        run(
            Command::new(&git)
                .args(["-c", "core.hooksPath=NUL"])
                .args(args)
                .current_dir(&source)
                .env("GIT_TERMINAL_PROMPT", "0"),
            shared,
            cancel,
            Duration::from_secs(600),
            None,
        )
    };
    phase(shared, "Fetch Reckless source");
    git_run(&["init", "--quiet"])?;
    git_run(&["remote", "add", "origin", &format!("{REPOSITORY}.git")])?;
    git_run(&["fetch", "--depth=1", "origin", &o.source_ref])?;
    git_run(&["checkout", "--detach", "FETCH_HEAD"])?;
    let commit = git_run(&["rev-parse", "HEAD"])?;
    // A private workspace prevents Cargo from joining any enclosing application
    // workspace when the output folder happens to be inside a Rust repository.
    // Preserve upstream sources and use the committed lockfile unchanged.
    let upstream_manifest =
        fs::read_to_string(source.join("Cargo.toml")).map_err(|e| e.to_string())?;
    let workspace = workspace_manifest(&upstream_manifest);
    fs::write(job.join("Cargo.toml"), &workspace).map_err(|e| e.to_string())?;
    fs::copy(source.join("Cargo.lock"), job.join("Cargo.lock")).map_err(|e| e.to_string())?;
    phase(shared, "Check Rust toolchain");
    let rustc = tool("rustc.exe").ok_or("rustc was not found")?;
    let cargo = tool("cargo.exe").ok_or("Cargo was not found")?;
    let compiler = run(
        Command::new(&rustc).arg("--version").current_dir(&source),
        shared,
        cancel,
        Duration::from_secs(30),
        None,
    )?;
    let cargo_version = run(
        Command::new(&cargo).arg("--version").current_dir(&source),
        shared,
        cancel,
        Duration::from_secs(30),
        None,
    )?;
    let features = if o.architecture == "avx2" {
        "+avx2,+crt-static"
    } else {
        "+crt-static"
    };
    let cpu = if o.architecture == "native" {
        "native"
    } else {
        "x86-64"
    };
    let flags = format!("-C\x1ftarget-cpu={cpu}\x1f-C\x1ftarget-feature={features}");
    let target = "x86_64-pc-windows-msvc";
    let target_dir = job.join("cargo-target");
    phase(shared, "Compile Reckless (without Syzygy)");
    run(
        Command::new(&cargo)
            .args([
                "build",
                "--release",
                "--locked",
                "--no-default-features",
                "--bin",
                "reckless",
                "--target",
                target,
                "--target-dir",
            ])
            .arg(&target_dir)
            .arg("--jobs")
            .arg(o.jobs.to_string())
            .current_dir(&source)
            .env_remove("RUSTFLAGS")
            .env_remove("EVALFILE")
            .env_remove("CARGO_BUILD_RUSTFLAGS")
            .env_remove("CARGO_TARGET_X86_64_PC_WINDOWS_MSVC_RUSTFLAGS")
            .env("CARGO_ENCODED_RUSTFLAGS", &flags)
            .env(
                "CARGO_PROFILE_RELEASE_STRIP",
                if o.strip { "symbols" } else { "none" },
            ),
        shared,
        cancel,
        Duration::from_secs(7200),
        None,
    )?;
    let network = network_name(
        &fs::read_to_string(source.join("build/build.rs")).map_err(|e| e.to_string())?,
    )?;
    let mut file = File::open(source.join("networks").join(&network)).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() == 0 {
        return Err("The embedded network is empty.".into());
    }
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    let network_hash = format!("{:x}", hasher.finalize());
    let network_url = format!("{NETWORK_BASE}/{network}");
    let executable = target_dir.join(target).join("release/reckless.exe");
    phase(shared, "Save build result");
    let result = job.join("result");
    fs::create_dir(&result).map_err(|e| e.to_string())?;
    let output = result.join("reckless.exe");
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
            "Reckless did not complete the standalone UCI handshake. Review the log.".into(),
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
        return Err("Reckless did not finish its standalone search benchmark.".into());
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
    let manifest = serde_json::json!({ "schemaVersion": 2, "engine": "reckless", "sourceUrl": REPOSITORY, "commit": commit.trim(), "options": o, "compiler": compiler, "cargoVersion": cargo_version, "rustFlags": flags, "syzygy": false, "privateBuildWorkspace": true, "cargoWorkspaceManifest": workspace, "network": network, "networkSha256": network_hash, "networkUrl": network_url, "networkEmbedded": true, "networkDigestVerifiedAgainstUpstream": false, "pgoProfilesApplied": o.pgo, "uciVerified": true, "standaloneSearchVerified": true, "logPath": job.join("build.log"), "sourceArchive": "source.tar" });
    fs::write(
        result.join("build.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().into_owned())
}

fn workspace_manifest(upstream: &str) -> String {
    let mut workspace = String::from("[workspace]\nmembers = [\"source\"]\nresolver = \"3\"\n");
    let mut profile = false;
    for line in upstream.lines() {
        if line.trim_start().starts_with('[') {
            profile = line.trim_start().starts_with("[profile.");
        }
        if profile {
            workspace.push_str(line);
            workspace.push('\n');
        }
    }
    workspace
}
fn network_name(build_script: &str) -> Result<String> {
    let name = build_script
        .lines()
        .find_map(|line| line.strip_prefix("const NETWORK_NAME: &str = \""))
        .and_then(|line| line.strip_suffix("\";"))
        .ok_or("The selected Reckless source does not name a supported network.")?;
    if name.is_empty()
        || name.starts_with('.')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    {
        return Err("Unsupported Reckless network file name.".into());
    }
    Ok(name.into())
}
fn bench_verified(output: &str) -> bool {
    output.lines().any(|line| {
        let fields: Vec<_> = line.split_whitespace().collect();
        fields.len() == 5
            && fields[0] == "Bench:"
            && fields[2] == "nodes"
            && fields[4] == "nps"
            && fields[1].parse::<u64>().is_ok_and(|n| n > 0)
            && fields[3].parse::<u64>().is_ok_and(|n| n > 0)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn requires_completed_search_and_safe_network() {
        let workspace = workspace_manifest(
            "[package]\nname=\"engine\"\n[profile.release]\nlto=\"fat\"\ncodegen-units=1\n[dependencies]\nlibc=\"0.2\"\n",
        );
        assert!(workspace.contains("[profile.release]\nlto=\"fat\"\ncodegen-units=1"));
        assert!(!workspace.contains("[dependencies]"));
        assert!(bench_verified("Bench: 123 nodes 456 nps"));
        assert_eq!(
            network_name(r#"const NETWORK_NAME: &str = "net.nnue";"#).unwrap(),
            "net.nnue"
        );
        assert!(!bench_verified("Bench: 0 nodes 456 nps"));
        assert!(network_name("const NETWORK_NAME: &str = \"../bad\";").is_err());
    }
}
