use super::*;
use sha2::{Digest, Sha256};

const VIRI_COMMIT: &str = "0631113e2215714c54d397cce9a8caebc5e6f8ee";
const RUST_TARGET: &str = "x86_64-pc-windows-msvc";
const RUST_TOOLCHAIN: &str = "stable-x86_64-pc-windows-msvc";

#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Engine {
    PlentyChess,
    Stormphrax,
    Viridithas,
}

pub(super) struct Spec {
    engine: Engine,
    pub(super) repository: &'static str,
    tag: &'static str,
}

pub(super) fn spec(id: &str) -> Option<Spec> {
    use Engine::*;
    let (engine, repository, tag) = match id {
        "plentychess" => (PlentyChess, "Yoshie2000/PlentyChess", "b-v8.0.0"),
        "stormphrax" => (Stormphrax, "Ciekce/Stormphrax", "v8.0.0"),
        "viridithas" => (Viridithas, "cosmobobak/viridithas", "v20.0.0"),
        _ => return None,
    };
    Some(Spec {
        engine,
        repository,
        tag,
    })
}

pub(super) fn defaults(id: &str) -> BuildOptions {
    BuildOptions {
        engine: id.into(),
        source_ref: spec(id).unwrap().tag.into(),
        pgo: false,
        auto_architecture: true,
        architecture: recommended(id),
        ..BuildOptions::default()
    }
}

pub(super) fn recommended(id: &str) -> String {
    match id {
        "plentychess" => hardware::detect().plentychess_recommended,
        "stormphrax" if hardware::detect().stormphrax_choices.is_empty() => String::new(),
        _ => "native".into(),
    }
}

pub(super) fn clang_tools_present(root: &Path) -> bool {
    [
        "usr/bin/bash.exe",
        "usr/bin/make.exe",
        "usr/bin/curl.exe",
        "ucrt64/bin/clang++.exe",
        "ucrt64/bin/clang.exe",
        "ucrt64/bin/ld.lld.exe",
    ]
    .iter()
    .all(|file| root.join(file).is_file())
}

impl Spec {
    pub(super) fn plan(&self, o: &BuildOptions) -> Result<Plan> {
        let name = engine_name(&o.engine);
        if self.engine == Engine::Stormphrax && o.pgo {
            return Err("Stormphrax PGO is unavailable. Turn PGO off.".into());
        }
        let hardware = hardware::detect();
        let valid_target = match self.engine {
            Engine::PlentyChess => hardware.plentychess_choices.contains(&o.architecture),
            Engine::Stormphrax => hardware.stormphrax_choices.contains(&o.architecture),
            Engine::Viridithas => o.architecture == "native",
        };
        if !valid_target {
            return Err(format!(
                "Select a compatible {name} target. The C++ builds require AVX2 and related CPU features."
            ));
        }
        let root = Path::new(&o.msys_root);
        if git_executable(root).is_none() {
            return Err("Install Git for Windows or MSYS2 Git before building.".into());
        }
        let toolchain = if self.engine == Engine::Viridithas {
            if o.source_ref != self.tag && o.source_ref != VIRI_COMMIT {
                return Err(format!(
                    "Viridithas has a verified network mapping for {}. Select that tag or commit {}.",
                    self.tag, VIRI_COMMIT
                ));
            }
            if tools::find_tool("rustup.exe").is_none() || tools::find_tool("curl.exe").is_none() {
                return Err("Install Rustup with stable Windows MSVC Rust, Microsoft C++ build tools, Windows SDK and curl. Restart the app after changing PATH.".into());
            }
            "Cargo / stable Windows MSVC Rust"
        } else {
            if !root.is_absolute() || !clang_tools_present(root) {
                return Err("Select an MSYS2 root with UCRT64 Clang and LLD, make and curl. See Settings > Build tools.".into());
            }
            if o.strip && !root.join("ucrt64/bin/strip.exe").is_file() {
                return Err("Install UCRT64 binutils or turn off symbol stripping.".into());
            }
            if o.pgo && !root.join("ucrt64/bin/llvm-profdata.exe").is_file() {
                return Err("Install the MSYS2 UCRT64 LLVM package (llvm-profdata) before enabling PlentyChess PGO.".into());
            }
            "MSYS2 UCRT64 Clang / LLD"
        };
        Ok(Plan {
            options: o.clone(),
            steps: vec![
                format!("Fetch {} at {} into a new job folder", self.repository, o.source_ref),
                "Fetch and hash the source-selected network; check its published digest for the verified release network".into(),
                format!("Build {name} with {toolchain}, target {}, {} jobs", o.architecture, o.jobs),
                if o.pgo { "Run the instrumented engine benchmark, merge LLVM profiles, then recompile with PGO".into() } else { "Use regular release compilation".into() },
                "Verify standalone UCI and search, compare the source benchmark when available, and save source, license, manifest and log".into(),
            ],
        })
    }

    pub(super) fn build(
        &self,
        o: &BuildOptions,
        job: &Path,
        shared: &Arc<Mutex<Shared>>,
        cancel: &AtomicBool,
    ) -> Result<String> {
        let root = Path::new(&o.msys_root);
        let source = job.join("source");
        fs::create_dir(&source).map_err(|e| e.to_string())?;
        let execute = |command: &mut Command, seconds| {
            run(
                command.current_dir(&source),
                shared,
                cancel,
                Duration::from_secs(seconds),
                None,
            )
        };
        let git = git_executable(root).ok_or("Git was not found.")?;
        let git_run = |args: &[&str]| {
            execute(
                Command::new(&git)
                    .args(["-c", "core.hooksPath=NUL", "-c", "core.autocrlf=false"])
                    .args(args)
                    .env("GIT_TERMINAL_PROMPT", "0"),
                600,
            )
        };
        let repo = format!("https://github.com/{}", self.repository);
        phase(shared, &format!("Fetch {} source", engine_name(&o.engine)));
        git_run(&["init", "--quiet"])?;
        git_run(&["remote", "add", "origin", &format!("{repo}.git")])?;
        git_run(&["fetch", "--depth=1", "origin", &o.source_ref])?;
        git_run(&["checkout", "--detach", "FETCH_HEAD"])?;
        let commit = git_run(&["rev-parse", "HEAD"])?;
        let message = git_run(&["log", "-1", "--format=%B"])?;
        let reference = message.lines().rev().find_map(|line| {
            line.trim()
                .strip_prefix("Bench:")?
                .trim()
                .parse::<u64>()
                .ok()
                .filter(|&n| n > 0)
        });
        let (network, network_url, expected_hash) = self.network(&source, commit.trim())?;
        let curl = if self.engine == Engine::Viridithas {
            tools::find_tool("curl.exe").ok_or("curl was not found.")?
        } else {
            root.join("usr/bin/curl.exe")
        };
        phase(shared, "Download and check the embedded network");
        execute(
            Command::new(curl).args([
                "--fail",
                "--location",
                "--retry",
                "2",
                "--output",
                &network,
                &network_url,
            ]),
            1200,
        )?;
        let network_hash = hash_file(&source.join(&network))?;
        if expected_hash.is_some_and(|expected| expected != network_hash) {
            return Err(format!(
                "The SHA-256 of {network} does not match the published release digest."
            ));
        }
        let filename = format!("{}.exe", o.engine);
        let (exe, compiler, build_tools) = if self.engine == Engine::Viridithas {
            build_viridithas(o, job, &source, &execute, shared, reference)?
        } else {
            self.build_clang(o, &source, &filename, &network, &execute, shared)?
        };

        let result = job.join("result");
        fs::create_dir(&result).map_err(|e| e.to_string())?;
        let output = result.join(&filename);
        fs::copy(exe, &output).map_err(|e| e.to_string())?;
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
        if !["uciok", "readyok"]
            .iter()
            .all(|token| uci.lines().any(|line| line.trim() == *token))
        {
            return Err("The executable did not complete the standalone UCI handshake.".into());
        }
        phase(shared, "Verify standalone search benchmark");
        let bench = run(
            Command::new(&output)
                .arg("bench")
                .current_dir(&result)
                .env("PATH", &system_path),
            shared,
            cancel,
            Duration::from_secs(600),
            None,
        )?;
        let nodes = verify_bench(&bench, reference)?;
        phase(shared, "Archive source and build provenance");
        git_run(&[
            "archive",
            "--format=tar",
            "--output=../result/source.tar",
            "HEAD",
        ])?;
        fs::copy(source.join("LICENSE"), result.join("LICENSE")).map_err(|e| e.to_string())?;
        let cargo_lock = if self.engine == Engine::Viridithas {
            fs::copy(source.join("Cargo.lock"), result.join("Cargo.lock"))
                .map_err(|e| e.to_string())?;
            Some("Cargo.lock")
        } else {
            None
        };
        let manifest = serde_json::json!({
            "schemaVersion": 2, "engine": o.engine, "sourceUrl": repo, "commit": commit.trim(),
            "options": o, "compiler": compiler, "buildTools": build_tools, "network": network,
            "networkUrl": network_url, "networkSha256": network_hash, "networkEmbedded": true,
            "networkDigestVerifiedAgainstUpstream": expected_hash.is_some(), "uciVerified": true,
            "standaloneSearchVerified": true, "benchmarkNodes": nodes, "referenceNodes": reference,
            "pgoProfilesApplied": o.pgo, "logPath": job.join("build.log"), "sourceArchive": "source.tar",
            "cargoLock": cargo_lock,
        });
        fs::write(
            result.join("build.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .map_err(|e| e.to_string())?;
        Ok(output.to_string_lossy().into_owned())
    }

    fn network(
        &self,
        source: &Path,
        commit: &str,
    ) -> Result<(String, String, Option<&'static str>)> {
        if self.engine == Engine::Viridithas {
            if commit != VIRI_COMMIT {
                return Err("This Viridithas revision has no verified network mapping.".into());
            }
            return Ok((
                "viridithas.nnue.zst".into(),
                "https://github.com/cosmobobak/viridithas-networks/releases/download/v109/sandhi-s2-b200.nnue.zst".into(),
                Some("066c172e22c25ee3d3b4084ad44ff761426f906a9c536cabd0127d17413d0585"),
            ));
        }
        let text = fs::read_to_string(source.join("network.txt"))
            .map_err(|e| format!("This source needs a supported network.txt: {e}"))?;
        let id = text.trim();
        if id.is_empty()
            || id.len() > 128
            || !id
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'-' || b == b'_')
        {
            return Err("The source does not identify a supported network release name.".into());
        }
        let (repository, extension, hash) = if self.engine == Engine::PlentyChess {
            (
                "Yoshie2000/PlentyNetworks",
                "bin",
                (id == "0178r")
                    .then_some("663e0531ac71c6c7c6d3bc4f1fac923bf69136f9786fa1da5c4ac25382aee54b"),
            )
        } else {
            (
                "Ciekce/stormphrax-nets",
                "nnue",
                (id == "undertown")
                    .then_some("04d651e078b7c7334709dbd772d40a23c0a5480e93e19521a03020c7d633f2cf"),
            )
        };
        let filename = format!("{id}.{extension}");
        let url = format!("https://github.com/{repository}/releases/download/{id}/{filename}");
        Ok((filename, url, hash))
    }

    fn build_clang(
        &self,
        o: &BuildOptions,
        source: &Path,
        filename: &str,
        network: &str,
        execute: &impl Fn(&mut Command, u64) -> Result<String>,
        shared: &Arc<Mutex<Shared>>,
    ) -> Result<(PathBuf, String, Vec<String>)> {
        let root = Path::new(&o.msys_root);
        let bin = root.join("ucrt64/bin");
        let path = std::env::join_paths([bin.clone(), root.join("usr/bin")].into_iter().chain(
            std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()),
        ))
        .map_err(|e| e.to_string())?;
        let clang = execute(
            Command::new(bin.join("clang++.exe"))
                .arg("--version")
                .env("PATH", &path),
            30,
        )?;
        let lld = execute(
            Command::new(bin.join("ld.lld.exe"))
                .arg("--version")
                .env("PATH", &path),
            30,
        )?;
        let make_version = execute(
            Command::new(root.join("usr/bin/make.exe"))
                .arg("--version")
                .env("PATH", &path),
            30,
        )?;
        let mut tools = vec![lld, make_version];
        let exe = source.join(filename);
        let make = |args: &[&str]| {
            execute(
                Command::new(root.join("usr/bin/bash.exe"))
                    .args([
                        "--noprofile",
                        "--norc",
                        "-c",
                        "exec /usr/bin/make \"$@\"",
                        "cec",
                    ])
                    .arg(format!("-j{}", o.jobs))
                    .arg(format!("EXE={filename}"))
                    .arg(format!("EVALFILE={network}"))
                    .args(args)
                    .env("PATH", &path)
                    .env("MSYSTEM", "UCRT64")
                    .env("OS", "Windows_NT")
                    .env_remove("MAKEFLAGS")
                    .env_remove("MFLAGS")
                    .env_remove("LLVM_PROFILE_FILE"),
                7200,
            )
        };
        if self.engine == Engine::PlentyChess {
            let arch = format!("arch={}", o.architecture);
            phase(shared, "Preprocess PlentyChess network");
            make(&["process-net", &arch, "CXX=clang++"])?;
            phase(shared, "Compile PlentyChess");
            if o.pgo {
                let profdata = bin.join("llvm-profdata.exe");
                tools.push(execute(
                    Command::new(&profdata).arg("--version").env("PATH", &path),
                    30,
                )?);
                make(&[
                    "_nopgo",
                    &arch,
                    "CXX=clang++",
                    "CXXFLAGS_EXTRA=-fprofile-instr-generate",
                ])?;
                phase(shared, "Collect and merge PlentyChess profiles");
                let bench = execute(
                    Command::new(&exe)
                        .arg("bench")
                        .env("PATH", &path)
                        .env("LLVM_PROFILE_FILE", "default.profraw"),
                    600,
                )?;
                verify_bench(&bench, None)?;
                execute(
                    Command::new(&profdata)
                        .args(["merge", "-output=default.profdata", "default.profraw"])
                        .env("PATH", &path),
                    60,
                )?;
                phase(shared, "Recompile PlentyChess with LLVM profiles");
                make(&[
                    "-B",
                    "_nopgo",
                    &arch,
                    "CXX=clang++",
                    "CXXFLAGS_EXTRA=-fprofile-instr-use=default.profdata -Werror=profile-instr-out-of-date -Werror=profile-instr-unprofiled",
                ])?;
            } else {
                make(&["_nopgo", &arch, "CXX=clang++"])?;
            }
        } else {
            phase(shared, "Preprocess Stormphrax network and compile");
            make(&[
                &o.architecture,
                "LDFLAGS=-fuse-ld=lld -static",
                "MKDIR=mkdir -p",
            ])?;
        }
        if o.strip {
            phase(shared, "Strip executable symbols");
            execute(
                Command::new(bin.join("strip.exe"))
                    .arg(&exe)
                    .env("PATH", &path),
                60,
            )?;
        }
        Ok((exe, clang, tools))
    }
}

fn build_viridithas(
    o: &BuildOptions,
    job: &Path,
    source: &Path,
    execute: &impl Fn(&mut Command, u64) -> Result<String>,
    shared: &Arc<Mutex<Shared>>,
    reference: Option<u64>,
) -> Result<(PathBuf, String, Vec<String>)> {
    let rustup = tools::find_tool("rustup.exe").ok_or("Rustup was not found.")?;
    let rust = |tool: &str| {
        let mut command = Command::new(&rustup);
        command
            .args(["run", RUST_TOOLCHAIN, tool])
            .env("RUSTUP_AUTO_INSTALL", "0");
        command
    };
    let compiler = execute(rust("rustc").arg("-vV"), 30)?;
    let cargo = execute(rust("cargo").arg("--version"), 30)?;
    let mut tools = vec![cargo];
    // Isolate this single package from any enclosing workspace. The upstream
    // release omits a lockfile, so resolve once and reuse it for both passes.
    fs::OpenOptions::new()
        .append(true)
        .open(source.join("Cargo.toml"))
        .and_then(|mut file| file.write_all(b"\n[workspace]\n"))
        .map_err(|e| e.to_string())?;
    phase(shared, "Resolve and lock Viridithas dependencies");
    execute(rust("cargo").arg("generate-lockfile"), 600)?;
    let target_dir = job.join("cargo-target");
    let exe = target_dir.join(RUST_TARGET).join("release/viridithas.exe");
    let compile = |extra: &str| {
        execute(
            rust("cargo")
                .args([
                    "build",
                    "--release",
                    "--locked",
                    "--bin",
                    "viridithas",
                    "--target",
                    RUST_TARGET,
                    "--target-dir",
                ])
                .arg(&target_dir)
                .arg("--jobs")
                .arg(o.jobs.to_string())
                .env_remove("EVALFILE")
                .env_remove("RUSTC")
                .env_remove("RUSTC_WRAPPER")
                .env_remove("RUSTC_WORKSPACE_WRAPPER")
                .env(
                    "CARGO_ENCODED_RUSTFLAGS",
                    format!("-Ctarget-cpu=native\x1f-Ctarget-feature=+crt-static{extra}"),
                )
                .env(
                    "CARGO_PROFILE_RELEASE_STRIP",
                    if o.strip { "symbols" } else { "none" },
                ),
            7200,
        )
    };
    phase(shared, "Compile Viridithas with Cargo");
    if o.pgo {
        let sysroot = execute(rust("rustc").args(["--print", "sysroot"]), 30)?;
        let profdata = Path::new(sysroot.trim())
            .join("lib/rustlib")
            .join(RUST_TARGET)
            .join("bin/llvm-profdata.exe");
        if !profdata.is_file() {
            return Err(format!(
                "Install the matching Rust LLVM tools explicitly: rustup component add llvm-tools --toolchain {RUST_TOOLCHAIN}"
            ));
        }
        tools.push(execute(Command::new(&profdata).arg("--version"), 30)?);
        let profiles = job.join("pgo");
        fs::create_dir(&profiles).map_err(|e| e.to_string())?;
        compile(&format!("\x1f-Cprofile-generate={}", profiles.display()))?;
        phase(shared, "Collect and merge Viridithas profiles");
        let bench = execute(
            Command::new(&exe)
                .arg("bench")
                .env_remove("LLVM_PROFILE_FILE"),
            600,
        )?;
        verify_bench(&bench, reference)?;
        let merged = job.join("merged.profdata");
        execute(
            Command::new(&profdata)
                .arg("merge")
                .arg("-o")
                .arg(&merged)
                .arg(&profiles),
            60,
        )?;
        phase(shared, "Recompile Viridithas with LLVM profiles");
        compile(&format!("\x1f-Cprofile-use={}", merged.display()))?;
    } else {
        compile("")?;
    }
    Ok((exe, compiler, tools))
}

fn hash_file(path: &Path) -> Result<String> {
    let mut file = File::open(path).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    if std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())? == 0 {
        return Err("The embedded network file is empty.".into());
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn verify_bench(output: &str, expected: Option<u64>) -> Result<u64> {
    let nodes = output
        .lines()
        .rev()
        .find_map(|line| {
            let fields: Vec<_> = line.split_whitespace().collect();
            match fields.as_slice() {
                ["Nodes", "searched", ":", nodes] => nodes.parse::<u64>().ok(),
                [nodes, "nodes", nps, "nps"] if nps.parse::<u64>().is_ok_and(|n| n > 0) => {
                    nodes.parse::<u64>().ok()
                }
                _ => None,
            }
            .filter(|&n| n > 0)
        })
        .ok_or("The engine did not report a completed standalone search benchmark.")?;
    if let Some(reference) = expected
        && nodes != reference
    {
        return Err(format!(
            "The benchmark searched {nodes} nodes. The source expects {reference}."
        ));
    }
    Ok(nodes)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn benchmark_requires_a_complete_summary_and_matches_reference() {
        assert_eq!(verify_bench("Nodes searched  : 1234", Some(1234)), Ok(1234));
        assert_eq!(verify_bench("1234 nodes 5678 nps", Some(1234)), Ok(1234));
        assert!(verify_bench("1234 nodes 0 nps", None).is_err());
        assert!(verify_bench("info depth 12 nodes 1234 nps 5678", None).is_err());
        assert!(verify_bench("1234 nodes 5678 nps", Some(4321)).is_err());
    }
}
