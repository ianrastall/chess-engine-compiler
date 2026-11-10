use super::*;
use sha2::{Digest, Sha256};

fn find_tool(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|p| p.join(name))
        .find(|p| p.is_file() && !p.to_string_lossy().contains("WindowsApps"))
}

pub(super) fn defaults() -> BuildOptions {
    let vswhere = PathBuf::from(std::env::var_os("ProgramFiles(x86)").unwrap_or_default())
        .join("Microsoft Visual Studio/Installer/vswhere.exe");
    // Prefer VS 2022: CUDA 12.x does not support the VS 2026 compiler.
    let vs = |version: bool| {
        let mut command = Command::new(&vswhere);
        command.args([
            "-latest",
            "-products",
            "*",
            "-requires",
            "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
            "-property",
            "installationPath",
        ]);
        if version {
            command.args(["-version", "[17.0,18.0)"]);
        }
        hidden(&mut command)
            .output()
            .ok()
            .filter(|o| o.status.success())
            .map(|o| String::from_utf8_lossy(&o.stdout).trim().to_owned())
            .unwrap_or_default()
    };
    let mut visual_studio = vs(true);
    if visual_studio.is_empty() {
        visual_studio = vs(false);
    }
    let cuda_root = std::env::var("CUDA_PATH").unwrap_or_default();
    BuildOptions {
        engine: "lc0".into(),
        source_ref: "v0.32.1".into(),
        backend: if Path::new(&cuda_root).join("bin/nvcc.exe").is_file() {
            "cuda-auto"
        } else {
            "eigen"
        }
        .into(),
        python_path: std::env::var("CEC_PYTHON")
            .ok()
            .or_else(|| {
                std::env::var_os("LOCALAPPDATA")
                    .map(PathBuf::from)
                    .map(|p| p.join("Jupiter/ChessEngineCompiler/tools/lc0/Scripts/python.exe"))
                    .filter(|p| p.is_file())
                    .map(|p| p.to_string_lossy().into())
            })
            .or_else(|| find_tool("python.exe").map(|p| p.to_string_lossy().into()))
            .unwrap_or_default(),
        visual_studio,
        cuda_root,
        pgo: false,
        strip: false,
        ..BuildOptions::default()
    }
}

pub(super) fn plan(o: &BuildOptions) -> Result<Plan> {
    if !matches!(o.backend.as_str(), "eigen" | "cuda-auto") {
        return Err("Select CPU (Eigen) or NVIDIA GPU (CUDA).".into());
    }
    if !Path::new(&o.python_path).is_absolute() || !Path::new(&o.python_path).is_file() {
        return Err("Select a Python 3 executable with Meson and Ninja installed. See Help > Build requirements.".into());
    }
    if !Path::new(&o.visual_studio)
        .join("VC/Auxiliary/Build/vcvarsall.bat")
        .is_file()
    {
        return Err("Select a Visual Studio installation with Desktop development with C++ and the Windows SDK.".into());
    }
    if o.backend == "cuda-auto"
        && !["bin/nvcc.exe", "lib/x64/cublas.lib", "lib/x64/cudart.lib"]
            .iter()
            .all(|p| Path::new(&o.cuda_root).join(p).is_file())
    {
        return Err("Select the CUDA Toolkit root containing nvcc, cuBLAS and the CUDA runtime. CUDA 12.x requires Visual Studio 2022.".into());
    }
    let weights = Path::new(&o.weights_path);
    if o.weights_path.is_empty() && o.release_id == 0 {
        return Err("Choose an Lc0 network weights file (.pb.gz or .pb). It is required for the standalone search test and is copied into the result.".into());
    }
    if !o.weights_path.is_empty() {
        validate_weights_path(weights, o.release_id != 0)?;
    }
    if git_executable(Path::new("")).is_none() {
        return Err("Install Git for Windows and restart the app.".into());
    }
    if o.release_id != 0 && find_tool("curl.exe").is_none() {
        return Err(
            "Windows curl.exe is required for the verified Lc0 dependency downloads.".into(),
        );
    }
    Ok(Plan { options: o.clone(), steps: vec![
        format!("Fetch LeelaChessZero/lc0 at {} and its pinned lczero-common submodule into a new job", o.source_ref),
        "Load the selected MSVC x64 environment; check Python, Meson and Ninja without changing installed tools".into(),
        format!("Configure Meson release build with {} backend; download upstream dependency wraps", o.backend),
        format!("Compile using Ninja with {} parallel jobs", o.jobs),
        if o.weights_path.is_empty() {
            format!("Use the recipe network {} and check its size and SHA-256", recipes::LC0.network.name)
        } else { "Use the selected local weights file and record its SHA-256".into() },
        "Copy lc0.exe, network, configuration and required CUDA runtime libraries into the result".into(),
        "Verify UCI and a real neural-network search from the result with a clean runtime PATH".into(),
        "Save source/dependency archives, licenses, network SHA-256, versions, commit, manifest and full log".into(),
    ] })
}

pub(super) fn validate_weights_path(weights: &Path, managed: bool) -> Result<()> {
    if weights.is_absolute() && weights.is_file() {
        return Ok(());
    }
    Err(if managed {
        "The selected Lc0 weights file was not found. Open Manage Lc0 weights to choose a file or use the default network."
    } else {
        "The selected Lc0 weights file was not found. Browse for the file or download the default network."
    }.into())
}

fn environment(o: &BuildOptions, cancel: &AtomicBool) -> Result<Vec<(String, String)>> {
    use std::io::Read;
    use std::os::windows::process::CommandExt;
    // Fixed command text. The installation path is passed as an environment value,
    // never interpolated as command syntax. Do not log the inherited environment.
    let mut child = hidden(
        Command::new("cmd.exe")
            .args(["/d", "/s", "/c"])
            .raw_arg("\"\"%CEC_VCVARS%\" x64 >nul && set\"")
            .env(
                "CEC_VCVARS",
                Path::new(&o.visual_studio).join("VC/Auxiliary/Build/vcvarsall.bat"),
            ),
    )
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| format!("Could not load MSVC: {e}"))?;
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    let capture = |pipe: Box<dyn Read + Send>| {
        thread::spawn(move || {
            let mut bytes = Vec::new();
            let _ = pipe.take(1024 * 1024).read_to_end(&mut bytes);
            bytes
        })
    };
    let out = capture(Box::new(stdout));
    let err = capture(Box::new(stderr));
    let started = std::time::Instant::now();
    let status = loop {
        if cancel.load(Ordering::SeqCst) || started.elapsed() > Duration::from_secs(30) {
            stop_tree(&mut child);
            let _ = out.join();
            let _ = err.join();
            return Err("MSVC environment setup was cancelled or timed out.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(80)),
            Err(e) => {
                stop_tree(&mut child);
                let _ = out.join();
                let _ = err.join();
                return Err(e.to_string());
            }
        }
    };
    let stdout = out.join().map_err(|_| "MSVC environment capture failed")?;
    let stderr = err.join().map_err(|_| "MSVC diagnostic capture failed")?;
    if !status.success() {
        return Err(format!(
            "MSVC environment setup failed. Check the selected Visual Studio C++ workload. {}",
            String::from_utf8_lossy(&stderr).trim()
        ));
    }
    let mut vars: Vec<(String, String)> = String::from_utf8_lossy(&stdout)
        .lines()
        .filter_map(|line| line.split_once('='))
        .filter(|(key, _)| !key.is_empty())
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
    let base = vars
        .iter()
        .find(|(k, _)| k.eq_ignore_ascii_case("PATH"))
        .map(|(_, v)| v.clone())
        .unwrap_or_default();
    vars.retain(|(k, _)| !k.eq_ignore_ascii_case("PATH") && !k.eq_ignore_ascii_case("CEC_VCVARS"));
    let python = Path::new(&o.python_path)
        .parent()
        .ok_or("Invalid Python path")?;
    let mut paths = vec![python.to_path_buf(), python.join("Scripts")];
    if o.backend == "cuda-auto" {
        paths.push(Path::new(&o.cuda_root).join("bin"));
    }
    paths.extend(std::env::split_paths(&base));
    vars.push((
        "PATH".into(),
        std::env::join_paths(paths)
            .map_err(|e| e.to_string())?
            .to_string_lossy()
            .into(),
    ));
    vars.extend([
        ("CC".into(), "cl".into()),
        ("CXX".into(), "cl".into()),
        ("CC_LD".into(), "link".into()),
        ("CXX_LD".into(), "link".into()),
    ]);
    Ok(vars)
}

pub(super) fn build(
    o: &BuildOptions,
    job: &Path,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<String> {
    // MSVC applies legacy path limits to both compiler inputs and generated
    // files. Keep both Meson trees short and retain the checkout with the job.
    let workspace = short_workspace(job)?;
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        build_in_workspace(o, job, workspace.path(), shared, cancel)
    }));
    let source = workspace.path().join("s");
    if source.is_dir()
        && let Err(error) = fs::rename(&source, job.join("source"))
    {
        let retained = workspace.keep();
        let message = format!(
            "Could not move the Lc0 checkout into the job: {error}. It remains in {}.",
            retained.display()
        );
        shared.lock().unwrap().line(&message);
        let _ = fs::write(job.join("source-location.txt"), message);
    }
    match result {
        Ok(result) => result,
        Err(panic) => std::panic::resume_unwind(panic),
    }
}

fn build_in_workspace(
    o: &BuildOptions,
    job: &Path,
    workspace: &Path,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<String> {
    let recipe = &*recipes::LC0;
    let repository = format!("https://github.com/{}", recipe.release.repository);
    let source = workspace.join("s");
    fs::create_dir(&source).map_err(|e| e.to_string())?;
    let git = git_executable(Path::new("")).ok_or("Git was not found")?;
    let git_run = |args: &[&str]| {
        run(
            Command::new(&git)
                .args([
                    "-c",
                    "core.hooksPath=NUL",
                    "-c",
                    "core.autocrlf=false",
                    "-c",
                    "core.longpaths=true",
                ])
                .args(args)
                .current_dir(&source)
                .env("GIT_TERMINAL_PROMPT", "0"),
            shared,
            cancel,
            Duration::from_secs(600),
            None,
        )
    };
    phase(shared, "Fetch Lc0 source and pinned submodule");
    git_run(&["init", "--quiet"])?;
    git_run(&["remote", "add", "origin", &format!("{repository}.git")])?;
    git_run(&["fetch", "--depth=1", "origin", &fetch_reference(o)])?;
    git_run(&["checkout", "--detach", "FETCH_HEAD"])?;
    let flags = recipe.flags(&source, &o.backend, o.release_id != 0)?;
    git_run(&["submodule", "update", "--init", "--recursive", "--depth=1"])?;
    let commit = git_run(&["rev-parse", "HEAD"])?;
    let submodules = git_run(&["submodule", "status", "--recursive"])?;
    phase(shared, "Check MSVC, Python, Meson and Ninja");
    let mut env = environment(o, cancel)?;
    if o.release_id != 0 {
        env.retain(|(key, _)| !key.eq_ignore_ascii_case("MESON_PACKAGE_CACHE_DIR"));
    }
    // Command's Debug formatter includes explicit env values, so attach the
    // tool environment only in the runner after logging the executable/args.
    let tool = |command: &mut Command, timeout| {
        run_with_environment(command, &env, shared, cancel, timeout)
    };
    let python_version = tool(
        Command::new(&o.python_path).arg("--version"),
        Duration::from_secs(30),
    )?;
    let meson_version = tool(Command::new(&o.python_path).args(["-m", "mesonbuild.mesonmain", "--version"]), Duration::from_secs(30))
        .map_err(|e| format!("Meson is unavailable in the selected Python. Install meson and ninja there; see Help > Build requirements. {e}"))?;
    let ninja_version = tool(
        Command::new("ninja.exe").arg("--version"),
        Duration::from_secs(30),
    )?;
    let weights = if o.weights_path.is_empty() {
        default_network(job, shared, cancel)?
    } else {
        PathBuf::from(&o.weights_path)
    };
    let archives = if o.release_id != 0 {
        fetch_wraps(&source, shared, cancel)?
    } else {
        Vec::new()
    };
    phase(shared, "Configure Lc0 release build");
    let build_dir = workspace.join("b");
    let mut setup = Command::new(&o.python_path);
    setup
        .args(["-m", "mesonbuild.mesonmain", "setup"])
        .arg(&build_dir)
        .arg(&source)
        .args([
            "--backend=ninja",
            "--buildtype=release",
            "-Ddefault_library=static",
            "-Db_vscrt=mt",
        ])
        .args(&flags);
    if o.release_id != 0 {
        setup
            .arg("--wrap-mode=nodownload")
            .env_remove("MESON_PACKAGE_CACHE_DIR");
    }
    // Upstream's native_arch option uses -march=native only for non-MSVC.
    let cpu_target = if o.release_id != 0 && !hardware::detect().plentychess_choices.is_empty() {
        setup.arg("-Dcpp_args=/arch:AVX2");
        "avx2"
    } else {
        "msvc-default"
    };
    if o.backend == "cuda-auto" {
        setup
            .arg(format!(
                "-Dcudnn_libdirs={}",
                Path::new(&o.cuda_root).join("lib/x64").display()
            ))
            .arg(format!(
                "-Dcudnn_include={}",
                Path::new(&o.cuda_root).join("include").display()
            ));
    }
    let configured = tool(&mut setup, Duration::from_secs(1200));
    let _ = fs::copy(
        build_dir.join("meson-logs/meson-log.txt"),
        job.join("meson-log.txt"),
    );
    configured?;
    phase(shared, "Compile Lc0");
    tool(
        Command::new(&o.python_path)
            .args(["-m", "mesonbuild.mesonmain", "compile", "-C"])
            .arg(&build_dir)
            .arg(format!("-j{}", o.jobs)),
        Duration::from_secs(7200),
    )?;
    let result = job.join("result");
    fs::create_dir(&result).map_err(|e| e.to_string())?;
    let output = result.join("lc0.exe");
    fs::copy(build_dir.join("lc0.exe"), &output).map_err(|e| e.to_string())?;
    phase(shared, "Save network and runtime libraries");
    let weights_name = if weights
        .extension()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("gz"))
    {
        "weights.pb.gz"
    } else {
        "weights.pb"
    };
    fs::copy(&weights, result.join(weights_name)).map_err(|e| e.to_string())?;
    let mut file = File::open(result.join(weights_name)).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher).map_err(|e| e.to_string())?;
    let weights_hash = format!("{:x}", hasher.finalize());
    let mut runtime_files = Vec::new();
    if o.backend == "cuda-auto" {
        let license = Path::new(&o.cuda_root).join("LICENSE");
        if license.is_file() {
            fs::copy(license, result.join("CUDA-LICENSE.txt")).map_err(|e| e.to_string())?;
        }
        for entry in fs::read_dir(Path::new(&o.cuda_root).join("bin")).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            let name = entry.file_name().to_string_lossy().into_owned();
            if ["cublas64_", "cublasLt64_", "cudart64_"]
                .iter()
                .any(|p| name.starts_with(p))
                && name.ends_with(".dll")
            {
                fs::copy(entry.path(), result.join(&name)).map_err(|e| e.to_string())?;
                runtime_files.push(name);
            }
        }
        if runtime_files.len() < 3 {
            return Err(
                "CUDA runtime DLLs were not found in the selected toolkit bin folder.".into(),
            );
        }
    }
    fs::write(
        result.join("lc0.config"),
        format!("# Network: {weights_name}, discovered beside lc0.exe\n--weights=<autodiscover>\n--backend={}\n", o.backend),
    )
    .map_err(|e| e.to_string())?;
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
        return Err("Lc0 did not complete the standalone UCI handshake.".into());
    }
    phase(shared, "Verify standalone neural-network search");
    let bench = run(
        Command::new(&output)
            .args([
                "benchmark",
                "--nodes=32",
                "--num-positions=1",
                "--threads=1",
                "--minibatch-size=1",
            ])
            .current_dir(job)
            .env("PATH", &system_path),
        shared,
        cancel,
        Duration::from_secs(300),
        None,
    )?;
    if !bench.lines().any(|line| {
        line.split_once("Nodes searched")
            .and_then(|(_, v)| v.trim().strip_prefix(':'))
            .and_then(|v| v.trim().parse::<u64>().ok())
            .is_some_and(|n| n > 0)
    }) {
        return Err("Lc0 did not complete a neural-network search. The network may be incompatible with this backend; review the saved log.".into());
    }
    phase(shared, "Archive source and build provenance");
    git_run(&[
        "archive",
        "--format=tar",
        &format!("--output={}", result.join("source.tar").display()),
        "HEAD",
    ])?;
    // Include the downloaded dependency sources and their license files, not
    // only the parent repository's git archive (which excludes submodules).
    run(
        Command::new("tar.exe")
            .args(["-cf"])
            .arg(result.join("dependencies.tar"))
            .args(["--exclude=.git", "-C"])
            .arg(&source)
            .args(["libs", "subprojects"]),
        shared,
        cancel,
        Duration::from_secs(300),
        None,
    )?;
    for name in ["COPYING", "AUTHORS"] {
        if source.join(name).is_file() {
            fs::copy(source.join(name), result.join(name)).map_err(|e| e.to_string())?;
        }
    }
    fs::copy(
        build_dir.join("meson-info/intro-buildoptions.json"),
        result.join("meson-buildoptions.json"),
    )
    .map_err(|e| e.to_string())?;
    fs::copy(
        build_dir.join("meson-info/intro-compilers.json"),
        result.join("meson-compilers.json"),
    )
    .map_err(|e| e.to_string())?;
    let manifest = serde_json::json!({"schemaVersion": 3, "engine": "lc0", "sourceUrl": repository, "commit": commit.trim(), "submodules": submodules.trim(), "options": o, "recipe": recipe.release.id, "actualArchitecture": cpu_target, "mesonFlags": flags, "dependencyArchives": archives, "python": python_version.trim(), "meson": meson_version.trim(), "ninja": ninja_version.trim(), "weightsFile": weights_name, "weightsSha256": weights_hash, "weightsUrl": o.weights_path.is_empty().then_some(&recipe.network.url), "runtimeFiles": runtime_files, "uciVerified": true, "standaloneSearchVerified": true, "logPath": job.join("build.log"), "sourceArchive": "source.tar", "dependencyArchive": "dependencies.tar"});
    fs::write(
        result.join("build.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().into())
}

fn short_workspace(job: &Path) -> Result<tempfile::TempDir> {
    let mut builder = tempfile::Builder::new();
    builder.prefix("cec-");
    let temp = std::env::temp_dir();
    let mut ancestors: Vec<_> = job.ancestors().collect();
    if ancestors.last().is_some_and(|root| temp.starts_with(root))
        && let Ok(workspace) = builder.tempdir_in(temp)
    {
        return Ok(workspace);
    }
    // Keep Meson's source and build trees on the same drive, including when
    // the job folder and Windows TEMP are on different volumes.
    ancestors.reverse();
    for parent in ancestors {
        if let Ok(workspace) = builder.tempdir_in(parent) {
            return Ok(workspace);
        }
    }
    Err("Could not create a short Lc0 build directory on the output drive.".into())
}

fn default_network(
    job: &Path,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    let network = &recipes::LC0.network;
    phase(shared, "Verify the Lc0 recipe network");
    let weights = job.join(&network.name);
    if let Some(local) = std::env::var_os("LOCALAPPDATA") {
        let existing = PathBuf::from(local)
            .join("Jupiter/ChessEngineCompiler/networks")
            .join(&network.name);
        if existing.is_file() {
            fs::copy(existing, &weights).map_err(|e| e.to_string())?;
            if matches_default_network(&weights) {
                return Ok(weights);
            }
        }
    }
    phase(shared, "Download the Lc0 recipe network");
    fetch_default_network(&weights, shared, cancel)?;
    Ok(weights)
}

fn matches_default_network(path: &Path) -> bool {
    let network = &recipes::LC0.network;
    path.metadata()
        .is_ok_and(|meta| meta.len() == network.bytes)
        && recipes::sha256(path).is_ok_and(|hash| hash == network.sha256)
}

fn fetch_default_network(
    path: &Path,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<()> {
    let network = &recipes::LC0.network;
    curl_download(&network.url, path, network.bytes, shared, cancel)?;
    if !matches_default_network(path) {
        return Err("The Lc0 recipe network did not match its expected size and SHA-256.".into());
    }
    Ok(())
}

impl Compiler {
    pub fn start_lc0_weights_download(&self, folder: PathBuf) -> Result<()> {
        if !folder.is_absolute() {
            return Err("Choose an absolute Lc0 weights folder.".into());
        }
        let _gate = self.gate.lock().unwrap();
        self.prepare()?;
        let shared = self.shared.clone();
        let cancel = self.cancel.clone();
        *self.worker.lock().unwrap() = Some(thread::spawn(move || {
            let result = save_default_network(&folder, &shared, &cancel);
            let mut state = shared.lock().unwrap();
            let (status, message) = match result {
                Ok(path) => {
                    state.snapshot.output = path.to_string_lossy().into_owned();
                    (
                        "Completed",
                        "Default Lc0 weights verified and ready to use.".into(),
                    )
                }
                Err(_) if cancel.load(Ordering::SeqCst) => {
                    ("Cancelled", "Weights download cancelled.".into())
                }
                Err(error) => ("Failed", error),
            };
            state.snapshot.state = status.into();
            state.snapshot.message = message.clone();
            state.line(&message);
        }));
        Ok(())
    }
}

fn save_default_network(
    folder: &Path,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<PathBuf> {
    fs::create_dir_all(folder)
        .map_err(|e| format!("Could not open the Lc0 weights folder: {e}"))?;
    let output = folder.join(&recipes::LC0.network.name);
    phase(shared, "Verify default Lc0 weights");
    if matches_default_network(&output) {
        return Ok(output);
    }
    let conflict = "A file with the default network's name failed verification. Move it out of the weights folder and retry.";
    if output.exists() {
        return Err(conflict.into());
    }
    let temporary = tempfile::NamedTempFile::new_in(folder)
        .map_err(|e| format!("Could not create a temporary weights file: {e}"))?
        .into_temp_path();
    phase(shared, "Download default Lc0 weights");
    fetch_default_network(&temporary, shared, cancel)?;
    if cancel.load(Ordering::SeqCst) {
        return Err("Weights download cancelled.".into());
    }
    if let Err(error) = temporary.persist_noclobber(&output)
        && !matches_default_network(&output)
    {
        return Err(if error.error.kind() == std::io::ErrorKind::AlreadyExists {
            conflict.into()
        } else {
            format!("Could not save the verified Lc0 weights: {}", error.error)
        });
    }
    Ok(output)
}

fn fetch_wraps(
    source: &Path,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<Vec<serde_json::Value>> {
    phase(shared, "Fetch and verify Lc0 dependency archives");
    let package_cache = source.join("subprojects/packagecache");
    fs::create_dir_all(&package_cache).map_err(|e| e.to_string())?;
    let mut records = Vec::new();
    for wrap in &recipes::LC0.archive_wraps {
        for archive in recipes::wrap_archives(&source.join(format!("subprojects/{wrap}.wrap")))? {
            let path = package_cache.join(&archive.filename);
            let mut last_error = String::new();
            let mut verified = None;
            for url in archive.urls {
                match curl_download(&url, &path, 256 * 1024 * 1024, shared, cancel) {
                    Ok(()) if recipes::sha256(&path).is_ok_and(|hash| hash == archive.sha256) => {
                        verified = Some(url);
                        break;
                    }
                    Ok(()) => last_error = "SHA-256 did not match the source wrap.".into(),
                    Err(error) => last_error = error,
                }
                if cancel.load(Ordering::SeqCst) {
                    return Err("Dependency download cancelled.".into());
                }
            }
            let url = verified.ok_or_else(|| {
                format!(
                    "Could not verify Lc0 dependency {}: {last_error}",
                    archive.filename
                )
            })?;
            records.push(
                serde_json::json!({"file": archive.filename, "sha256": archive.sha256, "url": url}),
            );
        }
    }
    Ok(records)
}

fn curl_download(
    url: &str,
    path: &Path,
    max_bytes: u64,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<()> {
    let curl =
        find_tool("curl.exe").ok_or("Windows curl.exe is required for this Lc0 download.")?;
    run(
        Command::new(curl)
            .args([
                "--disable",
                "--fail",
                "--location",
                "--proto",
                "=https",
                "--proto-redir",
                "=https",
                "--retry",
                "2",
                "--max-filesize",
                &max_bytes.to_string(),
                "--output",
            ])
            .arg(path)
            .arg(url),
        shared,
        cancel,
        Duration::from_secs(600),
        None,
    )?;
    Ok(())
}

fn run_with_environment(
    command: &mut Command,
    env: &[(String, String)],
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
    timeout: Duration,
) -> Result<String> {
    // Log arguments before adding the MSVC environment; environment variables
    // can contain secrets and must never appear in the build log.
    shared.lock().unwrap().line(&format!("$ {command:?}"));
    command.envs(env.iter().map(|(k, v)| (k, v)));
    run_unlogged(command, shared, cancel, timeout, ProcessInput::None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validates_lc0_dependencies_independently_of_stockfish() {
        let dir = tempfile::tempdir().unwrap();
        for file in [
            "python.exe",
            "VC/Auxiliary/Build/vcvarsall.bat",
            "net.pb.gz",
            "cuda/bin/nvcc.exe",
            "cuda/lib/x64/cublas.lib",
            "cuda/lib/x64/cudart.lib",
        ] {
            let path = dir.path().join(file);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, []).unwrap();
        }
        let mut o = BuildOptions {
            engine: "lc0".into(),
            source_ref: "v0.32.1".into(),
            backend: "eigen".into(),
            python_path: dir.path().join("python.exe").to_string_lossy().into(),
            visual_studio: dir.path().to_string_lossy().into(),
            weights_path: dir.path().join("net.pb.gz").to_string_lossy().into(),
            output_root: dir.path().join("out").to_string_lossy().into(),
            msys_root: "not installed".into(),
            ..BuildOptions::default()
        };
        assert!(super::super::plan(&o).is_ok());
        o.backend = "cuda-auto".into();
        assert!(plan(&o).unwrap_err().contains("CUDA Toolkit"));
        o.cuda_root = dir.path().join("cuda").to_string_lossy().into();
        assert!(plan(&o).is_ok());
        o.backend = "onnx-dml".into();
        assert!(plan(&o).is_err());
        o.backend = "eigen".into();
        o.weights_path.clear();
        assert!(plan(&o).unwrap_err().contains("weights file"));
        o.release_id = 1;
        assert!(plan(&o).is_ok());
        o.weights_path = dir.path().join("moved.pb.gz").to_string_lossy().into();
        assert!(plan(&o).unwrap_err().contains("Manage Lc0 weights"));
    }

    #[test]
    fn default_network_download_preserves_an_existing_unverified_file() {
        let dir = tempfile::tempdir().unwrap();
        let output = dir.path().join(&recipes::LC0.network.name);
        fs::write(&output, b"existing file").unwrap();
        let core = Compiler::default();
        let error = save_default_network(dir.path(), &core.shared, &core.cancel).unwrap_err();
        assert!(error.contains("failed verification"));
        assert_eq!(fs::read(output).unwrap(), b"existing file");
    }

    #[test]
    fn msvc_path_is_data_and_environment_is_not_logged() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("MSVC & %PATH%");
        fs::create_dir_all(root.join("VC/Auxiliary/Build")).unwrap();
        fs::write(
            root.join("VC/Auxiliary/Build/vcvarsall.bat"),
            "@echo off\r\nset CEC_ENV_TEST=loaded\r\n",
        )
        .unwrap();
        let o = BuildOptions {
            visual_studio: root.to_string_lossy().into(),
            python_path: dir.path().join("python.exe").to_string_lossy().into(),
            ..BuildOptions::default()
        };
        let env = environment(&o, &AtomicBool::new(false)).unwrap();
        assert!(
            env.iter()
                .any(|(k, v)| k == "CEC_ENV_TEST" && v == "loaded")
        );
        let core = Compiler::default();
        run_with_environment(
            Command::new("cmd.exe").args(["/d", "/c", "exit 0"]),
            &[("CEC_SECRET".into(), "private-marker-7821".into())],
            &core.shared,
            &core.cancel,
            Duration::from_secs(10),
        )
        .unwrap();
        assert!(!core.snapshot().log_tail.contains("private-marker-7821"));
    }
}
