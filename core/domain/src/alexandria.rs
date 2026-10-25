use super::*;
use sha2::{Digest, Sha256};

const REPOSITORY: &str = "https://github.com/PGG106/Alexandria";

pub(super) fn defaults() -> BuildOptions {
    BuildOptions {
        engine: "alexandria".into(),
        source_ref: "v9.1.0".into(),
        pgo: false,
        auto_architecture: true,
        architecture: {
            let arch = hardware::detect().cpp_recommended;
            if arch == "x86-64" {
                String::new()
            } else {
                arch
            }
        },
        ..BuildOptions::default()
    }
}

fn downloader(root: &Path) -> bool {
    ["usr/bin/curl.exe"].iter().any(|p| root.join(p).is_file())
}

pub(super) fn plan(o: &BuildOptions) -> Result<Plan> {
    if o.pgo {
        return Err("Alexandria PGO is not supported by this adapter. Turn PGO off.".into());
    }
    if !matches!(o.architecture.as_str(), "avx2" | "bmi2") {
        return Err("Alexandria requires an AVX2/FMA-compatible target; the scalar baseline is not supported because it did not match the reference benchmark.".into());
    }
    if !hardware::detect().cpp_choices.contains(&o.architecture) {
        return Err(
            "This CPU cannot run the selected Alexandria target. Select a compatible AVX2 target."
                .into(),
        );
    }
    let root = Path::new(&o.msys_root);
    if !toolchain_valid(root) {
        return Err("MSYS2 UCRT64 is incomplete. Select its root folder; install make, git, curl and mingw-w64-ucrt-x86_64-gcc in MSYS2.".into());
    }
    if !downloader(root) || !root.join("ucrt64/bin/gcc.exe").is_file() {
        return Err(
            "MSYS2 needs curl and UCRT64 GCC to download Alexandria's network and compile its source."
                .into(),
        );
    }
    if git_executable(root).is_none() {
        return Err(
            "Git was not found. Install Git for Windows or MSYS2 git, then restart the app.".into(),
        );
    }
    Ok(Plan { options: o.clone(), steps: vec![
        format!("Fetch PGG106/Alexandria at {} into a new job folder", o.source_ref),
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
    phase(shared, "Fetch Alexandria source");
    git_run(&["init", "--quiet"])?;
    git_run(&["remote", "add", "origin", &format!("{REPOSITORY}.git")])?;
    git_run(&["fetch", "--depth=1", "origin", &o.source_ref])?;
    git_run(&["checkout", "--detach", "FETCH_HEAD"])?;
    let commit = git_run(&["rev-parse", "HEAD"])?;
    let src = source.clone();
    let commit_message = git_run(&["log", "-1", "--format=%B"])?;
    let reference_bench = reference_nodes(&commit_message);
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
    phase(shared, "Find the source-pinned network");
    let expected_hash = fs::read_to_string(source.join("net-hash.txt"))
        .map_err(|e| e.to_string())?
        .trim()
        .to_ascii_lowercase();
    if expected_hash.len() != 64 || !expected_hash.bytes().all(|c| c.is_ascii_hexdigit()) {
        return Err("Alexandria source has an invalid network SHA-256.".into());
    }
    let catalog = run(
        Command::new(root.join("usr/bin/curl.exe"))
            .args([
                "--silent",
                "--show-error",
                "--fail",
                "--location",
                "--max-time",
                "60",
                "--header",
                "Accept: application/vnd.github+json",
                "--user-agent",
                "ChessEngineCompiler",
                "https://api.github.com/repos/PGG106/Alexandria-networks/releases?per_page=100",
            ])
            .env("PATH", &path),
        shared,
        cancel,
        Duration::from_secs(90),
        None,
    )?;
    let network_url = network_url(&catalog, &expected_hash)?;
    let network = "nn.net";
    run(
        Command::new(root.join("usr/bin/curl.exe"))
            .args(["--fail", "--location", "--retry", "2", "--output"])
            .arg(src.join(network))
            .arg(&network_url)
            .env("PATH", &path),
        shared,
        cancel,
        Duration::from_secs(1200),
        None,
    )?;
    let network_hash = hash_file(&src.join(network))?;
    if network_hash != expected_hash {
        return Err("Alexandria network does not match the source-pinned SHA-256.".into());
    }
    let network_digest_verified = true;
    let (simd, cpu) = match o.architecture.as_str() {
        "avx2" => ("$(AVX2FLAGS)", "-msse4.2 -mpopcnt -mavx2 -mbmi -mfma"),
        "bmi2" => (
            "$(BMI2FLAGS)",
            "-msse4.2 -mpopcnt -mavx2 -mbmi -mbmi2 -mfma",
        ),
        _ => return Err("Unsupported Alexandria target.".into()),
    };
    phase(shared, "Preprocess network and compile Alexandria");
    let flags = format!(
        r#"CXXFLAGS=-funroll-loops -O3 -flto -flto-partition=one -fno-exceptions -std=c++23 -DNDEBUG -static -pthread -DEVALFILE=\"processed.net\" {simd}"#
    );
    make(&[
        "all",
        "CXX=g++",
        "build=custom",
        "_ROOT=.",
        &format!("NATIVE={cpu}"),
        &flags,
        &format!("-j{}", o.jobs),
    ])?;
    let processed_network_hash = hash_file(&src.join("processed.net"))?;
    let executable = src.join("Alexandria.exe");
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
    let output = result.join("Alexandria.exe");
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
            "Alexandria did not complete the standalone UCI handshake. Review the log.".into(),
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
    let benchmark_nodes =
        bench_nodes(&bench).ok_or("Alexandria did not complete its standalone benchmark.")?;
    if reference_bench.is_some_and(|n| n != benchmark_nodes) {
        return Err(format!(
            "Alexandria benchmark mismatch: source expects {reference_bench:?}, built engine searched {benchmark_nodes} nodes."
        ));
    }
    phase(shared, "Archive source and build provenance");
    git_run(&[
        "archive",
        "--format=tar",
        "--output=../result/source.tar",
        "HEAD",
    ])?;
    if source.join("LICENSE.md").is_file() {
        fs::copy(source.join("LICENSE.md"), result.join("LICENSE")).map_err(|e| e.to_string())?;
    }
    let manifest = serde_json::json!({ "schemaVersion": 2, "engine": "alexandria", "sourceUrl": REPOSITORY, "commit": commit.trim(), "options": o, "compiler": compiler, "processedNetworkSha256": processed_network_hash, "benchmarkNodes": benchmark_nodes, "referenceBenchmarkNodes": reference_bench, "network": network, "networkSha256": network_hash, "networkUrl": network_url, "networkEmbedded": true, "networkDigestVerifiedAgainstUpstream": network_digest_verified, "pgoProfilesApplied": o.pgo, "uciVerified": true, "standaloneSearchVerified": true, "logPath": job.join("build.log"), "sourceArchive": "source.tar" });
    fs::write(
        result.join("build.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().into_owned())
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    Ok(format!("{:x}", hasher.finalize()))
}
fn network_url(catalog: &str, expected: &str) -> Result<String> {
    let releases: serde_json::Value = serde_json::from_str(catalog)
        .map_err(|e| format!("Invalid Alexandria network catalog: {e}"))?;
    for release in releases
        .as_array()
        .ok_or("Invalid Alexandria network release list.")?
    {
        for asset in release["assets"].as_array().into_iter().flatten() {
            if asset["name"] == "nn.net"
                && asset["digest"]
                    .as_str()
                    .is_some_and(|d| d.eq_ignore_ascii_case(&format!("sha256:{expected}")))
            {
                let url = asset["browser_download_url"].as_str().unwrap_or_default();
                if url
                    .starts_with("https://github.com/PGG106/Alexandria-networks/releases/download/")
                    && url.ends_with("/nn.net")
                {
                    return Ok(url.into());
                }
            }
        }
    }
    Err("No published Alexandria network matches this source's SHA-256. Choose a supported release or retry after checking GitHub access limits.".into())
}
fn bench_nodes(output: &str) -> Option<u64> {
    output.lines().find_map(|line| {
        let fields: Vec<_> = line.split_whitespace().collect();
        (fields.len() == 4 && fields[1] == "nodes" && fields[3] == "nps")
            .then(|| fields[0].parse::<u64>().ok().filter(|n| *n > 0))
            .flatten()
    })
}
fn reference_nodes(message: &str) -> Option<u64> {
    let lower = message.to_ascii_lowercase();
    lower
        .split("bench:")
        .nth(1)?
        .split_whitespace()
        .next()?
        .parse()
        .ok()
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn matches_network_hash_and_reference_benchmark() {
        assert_eq!(
            reference_nodes("Bump version Bench: 7415803"),
            Some(7415803)
        );
        assert_eq!(bench_nodes("7415803 nodes 1000 nps"), Some(7415803));
        assert_eq!(bench_nodes("0 nodes 1000 nps"), None);
        assert!(network_url("[]", &"a".repeat(64)).is_err());
        let catalog = serde_json::json!([{"assets":[{"name":"nn.net","digest":"sha256:abc","browser_download_url":"https://evil.invalid/nn.net"}]}]);
        assert!(network_url(&catalog.to_string(), "abc").is_err());
    }
}
