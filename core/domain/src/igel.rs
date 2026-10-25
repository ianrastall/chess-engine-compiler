use super::*;
use sha2::{Digest, Sha256};

const REPOSITORY: &str = "https://github.com/vshcherbyna/igel";
const NETWORK_BASE: &str = "https://github.com/vshcherbyna/igel/releases/download/";
pub(super) fn defaults() -> BuildOptions {
    BuildOptions {
        engine: "igel".into(),
        source_ref: "3.8.0".into(),
        pgo: false,
        auto_architecture: true,
        architecture: hardware::detect().igel_recommended,
        ..BuildOptions::default()
    }
}

fn downloader(root: &Path) -> bool {
    ["usr/bin/curl.exe"].iter().any(|p| root.join(p).is_file())
}

pub(super) fn plan(o: &BuildOptions) -> Result<Plan> {
    if o.pgo {
        return Err(
            "Igel PGO requires a separate Clang adapter and is not supported. Turn PGO off.".into(),
        );
    }
    if !hardware::detect().igel_choices.contains(&o.architecture) {
        return Err(
            "This CPU cannot run the selected Igel target. Igel NNUE requires AVX2 and POPCNT."
                .into(),
        );
    }
    let root = Path::new(&o.msys_root);
    if !toolchain_valid(root) {
        return Err("MSYS2 UCRT64 is incomplete. Select its root folder; install make, git, curl and mingw-w64-ucrt-x86_64-gcc in MSYS2.".into());
    }
    if !downloader(root) || !root.join("ucrt64/bin/gcc.exe").is_file() {
        return Err(
            "MSYS2 needs curl and UCRT64 GCC to download Igel's network and compile its source."
                .into(),
        );
    }
    if git_executable(root).is_none() {
        return Err(
            "Git was not found. Install Git for Windows or MSYS2 git, then restart the app.".into(),
        );
    }
    Ok(Plan { options: o.clone(), steps: vec![
        format!("Fetch vshcherbyna/igel at {} into a new job folder", o.source_ref),
        "Record the resolved commit, GCC version and verify the network SHA-256".into(),
        "Download the network named by the source and embed it in a static executable".into(),
        format!("Regular static GCC build, target={}; Syzygy included, no PGO (one compiler invocation)", o.architecture),
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
    phase(shared, "Fetch Igel source");
    git_run(&["init", "--quiet"])?;
    git_run(&["remote", "add", "origin", &format!("{REPOSITORY}.git")])?;
    git_run(&["fetch", "--depth=1", "origin", &o.source_ref])?;
    git_run(&["checkout", "--detach", "FETCH_HEAD"])?;
    let commit = git_run(&["rev-parse", "HEAD"])?;
    let src = source.join("src");
    let commit_message = git_run(&["log", "-1", "--format=%B"])?;
    let expected_bench = reference_nodes(&commit_message);
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
    phase(shared, "Download and verify embedded network");
    let makefile = fs::read_to_string(src.join("makefile")).map_err(|e| e.to_string())?;
    let (network, network_url) = network_spec(&makefile)?;
    let network_path = src.join("weights").join(&network);
    fs::create_dir_all(src.join("weights")).map_err(|e| e.to_string())?;
    run(
        Command::new(root.join("usr/bin/curl.exe"))
            .args(["--fail", "--location", "--retry", "2", "--output"])
            .arg(&network_path)
            .arg(&network_url)
            .env("PATH", &path),
        shared,
        cancel,
        Duration::from_secs(1800),
        None,
    )?;
    let mut file = File::open(&network_path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    let network_hash = format!("{:x}", hasher.finalize());
    verify_network(&network, &network_hash)?;
    let pext = o.architecture == "avx2-pext";
    let libs = format!(
        "LIBS=-std=c++17 -m64 -mpopcnt -pthread -lstdc++ -lm -mavx2 -static {}",
        if pext { "-mbmi2" } else { "" }
    );
    let defs = format!(
        "DEFS=-DNDEBUG -D_BTYPE={} -DSYZYGY_SUPPORT=TRUE -DUSE_AVX2=1",
        u8::from(pext)
    );
    let optim = if o.architecture == "native" {
        "OPTIM=-O3 -march=native -flto=auto -funroll-loops"
    } else {
        "OPTIM=-O3 -flto=auto -funroll-loops"
    };
    phase(shared, "Compile Igel (GCC regular build)");
    // Override native feature probing for explicit, CPU-validated targets.
    make(&[
        "basic",
        "CC=g++",
        "EXE=igel.exe",
        "GCCDEFINES=",
        &libs,
        &defs,
        optim,
    ])?;
    let executable = src.join("igel.exe");
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
    let output = result.join("igel.exe");
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
        return Err("Igel did not complete the standalone UCI handshake. Review the log.".into());
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
    let actual_bench = benchmark_nodes(&bench)
        .ok_or("Igel did not finish a positive-node standalone benchmark.")?;
    if expected_bench.is_some_and(|expected| expected != actual_bench) {
        return Err(format!(
            "Igel benchmark mismatch: expected {} nodes from the source commit, got {actual_bench}.",
            expected_bench.unwrap()
        ));
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
    let manifest = serde_json::json!({ "schemaVersion": 2, "engine": "igel", "sourceUrl": REPOSITORY, "commit": commit.trim(), "options": o, "compiler": compiler, "network": network, "networkSha256": network_hash, "networkUrl": network_url, "networkEmbedded": true, "networkDigestVerifiedAgainstUpstream": true, "benchmarkNodes": actual_bench, "referenceBenchmarkNodes": expected_bench, "referenceBenchmarkMatched": expected_bench.is_some(), "syzygy": true, "pgoProfilesApplied": o.pgo, "uciVerified": true, "standaloneSearchVerified": true, "logPath": job.join("build.log"), "sourceArchive": "source.tar" });
    fs::write(
        result.join("build.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().into_owned())
}

fn network_spec(makefile: &str) -> Result<(String, String)> {
    let assignment = |key: &str| {
        makefile.lines().find_map(|line| {
            let (name, value) = line.split_once('=')?;
            (name.trim() == key).then(|| value.trim().to_owned())
        })
    };
    let path = assignment("EVALFILE").ok_or("Unsupported Igel makefile: missing EVALFILE.")?;
    let network = path
        .strip_prefix("weights/")
        .ok_or("Unsupported Igel network path.")?;
    if network.len() < 9
        || !network
            .get(..8)
            .is_some_and(|v| v.bytes().all(|c| c.is_ascii_hexdigit()))
        || !network
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    {
        return Err("Unsupported Igel network name.".into());
    }
    let url = assignment("NETWORK_URL").ok_or("Unsupported Igel makefile: missing network URL.")?;
    if !url.starts_with(NETWORK_BASE)
        || !url.ends_with(&format!("/{network}"))
        || !url
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || ":/-_.".contains(c))
        || url.contains("..")
    {
        return Err("Unsupported Igel network URL.".into());
    }
    Ok((network.into(), url))
}
fn verify_network(name: &str, hash: &str) -> Result<()> {
    const STANDARD_SHA256: &str =
        "6b953e786f925e9df8a641b170087aae1b40071bd3495ba3b91737756b9f06d3";
    if name.get(..8) != hash.get(..8) || (name == "6b953e78.standard" && hash != STANDARD_SHA256) {
        return Err("Igel network SHA-256 does not match its source-selected identity.".into());
    }
    Ok(())
}
fn reference_nodes(message: &str) -> Option<u64> {
    message.lines().find_map(|line| {
        line.trim()
            .to_ascii_lowercase()
            .strip_prefix("bench:")
            .and_then(|v| v.split_whitespace().next())
            .and_then(|v| v.parse().ok())
    })
}
fn benchmark_nodes(output: &str) -> Option<u64> {
    output.lines().find_map(|line| {
        line.strip_prefix("Nodes : ")
            .and_then(|v| v.trim().parse().ok())
            .filter(|n| *n > 0)
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn verifies_network_identity_and_reference_benchmark() {
        assert_eq!(reference_nodes("change\nbench: 2325568"), Some(2325568));
        assert_eq!(benchmark_nodes("Nodes : 2325568"), Some(2325568));
        assert_eq!(benchmark_nodes("Nodes : 0"), None);
        assert!(verify_network("6b953e78.standard", &"0".repeat(64)).is_err());
        assert!(
            network_spec("EVALFILE = weights/../bad\nNETWORK_URL = https://evil.invalid/bad")
                .is_err()
        );
        assert!(network_spec("EVALFILE = weights/6b953e78.standard\nNETWORK_URL = https://github.com/vshcherbyna/igel/releases/download/0.8/6b953e78.standard").is_ok());
    }
}
