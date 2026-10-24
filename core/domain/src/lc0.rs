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
    if !Path::new(&o.weights_path).is_absolute() || !Path::new(&o.weights_path).is_file() {
        return Err("Choose an Lc0 network weights file (.pb.gz or .pb). It is required for the standalone search test and is copied into the result.".into());
    }
    if git_executable(Path::new("")).is_none() {
        return Err("Install Git for Windows and restart the app.".into());
    }
    Ok(Plan { options: o.clone(), steps: vec![
        format!("Fetch LeelaChessZero/lc0 at {} and its pinned lczero-common submodule into a new job", o.source_ref),
        "Load the selected MSVC x64 environment; check Python, Meson and Ninja without changing installed tools".into(),
        format!("Configure Meson release build with {} backend; download upstream dependency wraps", o.backend),
        format!("Compile using Ninja with {} parallel jobs", o.jobs),
        "Copy lc0.exe, selected network, configuration and required CUDA runtime libraries into the result".into(),
        "Verify UCI and a real neural-network search from the result with a clean runtime PATH".into(),
        "Save source/dependency archives, licenses, network SHA-256, versions, commit, manifest and full log".into(),
    ] })
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
    let source = job.join("source");
    fs::create_dir(&source).map_err(|e| e.to_string())?;
    let git = git_executable(Path::new("")).ok_or("Git was not found")?;
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
    phase(shared, "Fetch Lc0 source and pinned submodule");
    git_run(&["init", "--quiet"])?;
    git_run(&[
        "remote",
        "add",
        "origin",
        "https://github.com/LeelaChessZero/lc0.git",
    ])?;
    git_run(&["fetch", "--depth=1", "origin", &o.source_ref])?;
    git_run(&["checkout", "--detach", "FETCH_HEAD"])?;
    git_run(&["submodule", "update", "--init", "--recursive", "--depth=1"])?;
    let commit = git_run(&["rev-parse", "HEAD"])?;
    let submodules = git_run(&["submodule", "status", "--recursive"])?;
    phase(shared, "Check MSVC, Python, Meson and Ninja");
    let env = environment(o, cancel)?;
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
    phase(shared, "Configure Lc0 release build");
    let build_dir = source.join("build");
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
            "-Dgtest=false",
            "-Dopenblas=false",
            "-Dmkl=false",
            "-Ddnnl=false",
            "-Daccelerate=false",
            "-Dispc=false",
            "-Donnx=false",
            "-Dcudnn=false",
            "-Dopencl=false",
            "-Ddx=false",
            "-Dmetal=disabled",
            "-Dblas=true",
        ])
        .arg(format!("-Dplain_cuda={}", o.backend == "cuda-auto"))
        .arg(format!("-Ddefault_backend={}", o.backend));
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
    tool(&mut setup, Duration::from_secs(1200))?;
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
    let weights_name = if o.weights_path.to_ascii_lowercase().ends_with(".gz") {
        "weights.pb.gz"
    } else {
        "weights.pb"
    };
    fs::copy(&o.weights_path, result.join(weights_name)).map_err(|e| e.to_string())?;
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
        "--output=../result/source.tar",
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
    let manifest = serde_json::json!({"schemaVersion": 2, "engine": "lc0", "sourceUrl": "https://github.com/LeelaChessZero/lc0", "commit": commit.trim(), "submodules": submodules.trim(), "options": o, "python": python_version.trim(), "meson": meson_version.trim(), "ninja": ninja_version.trim(), "weightsFile": weights_name, "weightsSha256": weights_hash, "runtimeFiles": runtime_files, "uciVerified": true, "standaloneSearchVerified": true, "logPath": job.join("build.log"), "sourceArchive": "source.tar", "dependencyArchive": "dependencies.tar"});
    fs::write(
        result.join("build.json"),
        serde_json::to_vec_pretty(&manifest).unwrap(),
    )
    .map_err(|e| e.to_string())?;
    Ok(output.to_string_lossy().into())
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
    run_unlogged(command, shared, cancel, timeout, None)
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
