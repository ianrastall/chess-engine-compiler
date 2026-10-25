use super::*;
use sha2::{Digest, Sha256};

const REPOSITORY: &str = "https://github.com/SnowballSH/Avalanche";

pub(super) fn defaults() -> BuildOptions {
    let managed = tools::managed_zig(&tools::tools_root());
    BuildOptions {
        engine: "avalanche".into(),
        source_ref: "v5.0.0".into(),
        pgo: false,
        auto_architecture: true,
        architecture: "native".into(),
        zig_path: if managed.is_file() {
            managed.to_string_lossy().into()
        } else {
            tools::find_tool("zig.exe")
                .map(|p| p.to_string_lossy().into())
                .unwrap_or_default()
        },
        ..BuildOptions::default()
    }
}
pub(super) fn plan(o: &BuildOptions) -> Result<Plan> {
    if o.pgo {
        return Err("Avalanche PGO is not supported. Turn PGO off.".into());
    }
    if !matches!(o.architecture.as_str(), "native" | "x86_64") {
        return Err("Select Avalanche native or x86_64.".into());
    }
    if !Path::new(&o.zig_path).is_absolute() || !Path::new(&o.zig_path).is_file() {
        return Err("Select Zig 0.17.0 or install it in Settings > Build tools.".into());
    }
    if git_executable(Path::new("")).is_none() {
        return Err("Install Git for Windows first.".into());
    }
    Ok(Plan {options:o.clone(),steps:vec![format!("Fetch SnowballSH/Avalanche at {} into a new job",o.source_ref),
        "Check Zig 0.17.0; build a Windows x64 ReleaseFast executable with the source's embedded network and vendored C translator".into(),
        "Run UCI and the standalone benchmark with only System32 on PATH; require the exact bench.nodes reference".into(),
        "Save source/network hashes, compiler version, options, source archive, license, reference nodes and full log".into()]})
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
    phase(shared, "Fetch Avalanche source");
    git_run(&["init", "--quiet"])?;
    git_run(&["remote", "add", "origin", &format!("{REPOSITORY}.git")])?;
    git_run(&["fetch", "--depth=1", "origin", &o.source_ref])?;
    git_run(&["checkout", "--detach", "FETCH_HEAD"])?;
    let commit = git_run(&["rev-parse", "HEAD"])?;
    phase(shared, "Check Zig compiler");
    let compiler = run(
        Command::new(&o.zig_path).arg("version"),
        shared,
        cancel,
        Duration::from_secs(30),
        None,
    )?;
    if compiler.trim() != "0.17.0" {
        return Err(format!(
            "Avalanche v5.0.0 requires Zig 0.17.0; selected compiler reports {}. Use Settings > Build tools to install the supported version.",
            compiler.trim()
        ));
    }
    let script = fs::read_to_string(source.join("build.zig")).map_err(|e| e.to_string())?;
    let network = network_name(&script)?;
    let mut file = File::open(source.join(&network)).map_err(|e| e.to_string())?;
    if file.metadata().map_err(|e| e.to_string())?.len() < 1024 {
        return Err("Avalanche's tracked network is missing or is a pointer file.".into());
    }
    let mut hash = Sha256::new();
    std::io::copy(&mut file, &mut hash).map_err(|e| e.to_string())?;
    let network_hash = format!("{:x}", hash.finalize());
    let reference_bench = fs::read_to_string(source.join("bench.nodes"))
        .map_err(|e| e.to_string())?
        .trim()
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or("Missing Avalanche benchmark reference.")?;
    phase(shared, "Compile Avalanche with Zig");
    run(
        Command::new(&o.zig_path)
            .args(["build", "--release=fast", "-Dtarget=x86_64-windows-gnu"])
            .arg(format!("-Dcpu={}", o.architecture))
            .arg(format!("-Dstrip={}", o.strip))
            .arg(format!("-Dversion={}", commit.trim()))
            .arg(format!("-j{}", o.jobs))
            .env("ZIG_LOCAL_CACHE_DIR", job.join("zig-cache"))
            .env("ZIG_GLOBAL_CACHE_DIR", job.join("zig-global-cache"))
            .current_dir(&source),
        shared,
        cancel,
        Duration::from_secs(7200),
        None,
    )?;
    let executable = source.join("zig-out/bin/Avalanche.exe");
    phase(shared, "Save build result");
    let result = job.join("result");
    fs::create_dir(&result).map_err(|e| e.to_string())?;
    let output = result.join("Avalanche.exe");
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
            "Avalanche did not complete the standalone UCI handshake. Review the log.".into(),
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
    let benchmark_nodes = bench_nodes(&bench).ok_or("Avalanche did not complete its benchmark.")?;
    if benchmark_nodes != reference_bench {
        return Err(format!(
            "Avalanche benchmark mismatch: expected {reference_bench}, got {benchmark_nodes}."
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
    let manifest = serde_json::json!({"schemaVersion":2,"engine":"avalanche","sourceUrl":REPOSITORY,"commit":commit.trim(),"options":o,"compiler":compiler,"network":network,"networkSha256":network_hash,"networkEmbedded":true,"networkTrackedInSource":true,"benchmarkNodes":benchmark_nodes,"referenceBenchmarkNodes":reference_bench,"referenceBenchmarkMatched":true,"uciVerified":true,"standaloneSearchVerified":true,"pgoProfilesApplied":false,"syzygy":true,"sourceArchive":"source.tar","logPath":job.join("build.log")});
    fs::write(
        result.join("build.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().into_owned())
}

fn network_name(script: &str) -> Result<String> {
    let path = script
        .lines()
        .find_map(|line| line.strip_prefix(r#"const DEFAULT_NET = ""#))
        .and_then(|v| v.strip_suffix(r#"";"#))
        .ok_or("Unsupported Avalanche network declaration.")?;
    let name = path
        .strip_prefix("nets/")
        .ok_or("Unsupported Avalanche network path.")?;
    if name.is_empty()
        || name.starts_with('.')
        || !name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    {
        return Err("Unsupported Avalanche network name.".into());
    }
    Ok(path.into())
}
fn bench_nodes(output: &str) -> Option<u64> {
    output.lines().find_map(|line| {
        let f: Vec<_> = line.split_whitespace().collect();
        (f.len() == 4 && f[1] == "nodes" && f[3] == "nps")
            .then(|| f[0].parse().ok().filter(|n| *n > 0))
            .flatten()
    })
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn checks_tracked_network_path_and_positive_benchmark() {
        assert_eq!(
            network_name(r#"const DEFAULT_NET = "nets/net.nnue";"#).unwrap(),
            "nets/net.nnue"
        );
        assert!(network_name(r#"const DEFAULT_NET = "nets/../escape";"#).is_err());
        assert_eq!(bench_nodes("16645617 nodes 1000 nps"), Some(16645617));
        assert_eq!(bench_nodes("0 nodes 1000 nps"), None);
    }
}
