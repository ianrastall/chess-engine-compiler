use super::*;
use sha2::{Digest, Sha256};

const ZIG_URL: &str = "https://ziglang.org/download/0.17.0/zig-x86_64-windows-0.17.0.zip";
const ZIG_SHA256: &str = "b5663f69581dcf391293fbf16c06cb80d81d806545ce618b4d0bab7f0eb8c428";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct SetupOptions {
    pub tool_id: String,
    pub msys_root: String,
    pub python_path: String,
    pub destination_root: String,
}
impl Default for SetupOptions {
    fn default() -> Self {
        Self {
            tool_id: String::new(),
            msys_root: detect_msys().unwrap_or_default(),
            python_path: String::new(),
            destination_root: tools_root().to_string_lossy().into(),
        }
    }
}
#[derive(Clone, Debug, Serialize)]
pub struct Tool {
    pub id: String,
    pub name: String,
    pub status: String,
    pub details: String,
    pub url: String,
    pub install_available: bool,
}
pub fn tools_root() -> PathBuf {
    PathBuf::from(
        std::env::var_os("LOCALAPPDATA").unwrap_or_else(|| std::env::temp_dir().into_os_string()),
    )
    .join("Jupiter/ChessEngineCompiler/tools")
}
pub fn managed_zig(root: &Path) -> PathBuf {
    root.join("zig/0.17.0/zig-x86_64-windows-0.17.0/zig.exe")
}
pub fn find_tool(name: &str) -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|p| p.join(name))
        .find(|p| {
            p.is_file() && (!p.to_string_lossy().contains("WindowsApps") || name == "winget.exe")
        })
}
fn python(o: &SetupOptions) -> Option<PathBuf> {
    let selected = PathBuf::from(&o.python_path);
    if selected.is_absolute() && selected.is_file() {
        Some(selected)
    } else {
        find_tool("python.exe").or_else(|| {
            let local = PathBuf::from(std::env::var_os("LOCALAPPDATA").unwrap_or_default());
            let programs = PathBuf::from(std::env::var_os("ProgramFiles").unwrap_or_default());
            [
                local.join("Programs/Python/Python314/python.exe"),
                local.join("Programs/Python/Python313/python.exe"),
                programs.join("Python314/python.exe"),
            ]
            .into_iter()
            .find(|p| p.is_file())
        })
    }
}
fn winget_package(id: &str) -> Option<&'static str> {
    match id {
        "git" => Some("Git.Git"),
        "msys2" => Some("MSYS2.MSYS2"),
        "rust" => Some("Rustlang.Rustup"),
        "python" => Some("Python.Python.3.14"),
        "msvc" => Some("Microsoft.VisualStudio.2022.BuildTools"),
        _ => None,
    }
}
pub fn catalog(o: &SetupOptions) -> Vec<Tool> {
    let winget = find_tool("winget.exe").is_some();
    let msys = Path::new(&o.msys_root);
    let gcc = toolchain_valid(msys)
        && git_executable(msys).is_some()
        && ["usr/bin/curl.exe", "usr/bin/wget.exe"]
            .iter()
            .all(|p| msys.join(p).is_file());
    let vs = lc0::defaults().visual_studio;
    let meson_root = Path::new(&o.destination_root).join("lc0");
    let specs = [
        (
            "gcc",
            "C/C++ (MSYS2 GCC packages)",
            gcc.then(|| msys.to_path_buf()),
            "Installs git, make, curl, wget, coreutils and UCRT64 GCC in the selected MSYS2 root. Run MSYS2's full update first if its database is out of date. Existing packages are kept when already current.",
            "https://www.msys2.org/",
            msys.join("usr/bin/bash.exe").is_file(),
        ),
        (
            "git",
            "Git for Windows",
            git_executable(Path::new("")).filter(|p| !p.to_string_lossy().contains("msys")),
            "Uses WinGet package Git.Git and its interactive installer. Restart this app afterward to refresh PATH.",
            "https://git-scm.com/downloads/win",
            winget,
        ),
        (
            "msvc",
            "Microsoft C++ build tools",
            (!vs.is_empty()).then(|| PathBuf::from(&vs)),
            "Uses WinGet Microsoft.VisualStudio.2022.BuildTools. In the installer select Desktop development with C++, MSVC x64 tools and a Windows SDK. Administrator approval may be requested.",
            "https://visualstudio.microsoft.com/downloads/#build-tools-for-visual-studio-2022",
            winget,
        ),
        (
            "msys2",
            "MSYS2",
            msys.join("usr/bin/bash.exe")
                .is_file()
                .then(|| msys.to_path_buf()),
            "Uses WinGet MSYS2.MSYS2 and its interactive installer. Then select the MSYS2 root in the engine form and install the C/C++ GCC packages here.",
            "https://www.msys2.org/",
            winget,
        ),
        (
            "python",
            "Python 3",
            python(o),
            "Uses WinGet Python.Python.3.14 and its interactive installer. Then install Meson and Ninja in the app's private Python environment. Restart this app to refresh PATH.",
            "https://www.python.org/downloads/windows/",
            winget,
        ),
        (
            "meson",
            "Python tools (Meson and Ninja)",
            (meson_root
                .join("Lib/site-packages/mesonbuild/__init__.py")
                .is_file()
                && meson_root.join("Scripts/ninja.exe").is_file())
            .then(|| meson_root.clone()),
            "Creates an app-private Python virtual environment and installs Meson 1.12.1 and Ninja 1.13.2 with pip. Uses the selected Python or Python on PATH; does not change global Python packages.",
            "https://mesonbuild.com/Getting-meson.html",
            python(o).is_some(),
        ),
        (
            "rust",
            "Rust (Cargo and rustc)",
            find_tool("cargo.exe").filter(|_| find_tool("rustc.exe").is_some()),
            "Uses WinGet Rustlang.Rustup and its interactive installer. Choose the Windows MSVC toolchain. Also install Microsoft C++ build tools. Restart this app afterward.",
            "https://www.rust-lang.org/tools/install",
            winget,
        ),
        (
            "zig",
            "Zig 0.17.0",
            managed_zig(Path::new(&o.destination_root))
                .is_file()
                .then(|| managed_zig(Path::new(&o.destination_root))),
            "Downloads the official Zig 0.17.0 Windows x64 zip, checks its pinned SHA-256 and extracts it into this app's versioned tools folder. No administrator rights or PATH changes are needed. Avalanche can use it immediately.",
            "https://ziglang.org/download/",
            true,
        ),
        (
            "cuda",
            "NVIDIA CUDA 12.9",
            std::env::var_os("CUDA_PATH")
                .map(PathBuf::from)
                .filter(|p| p.join("bin/nvcc.exe").is_file()),
            "Use the official CUDA 12.9 download page and select Windows/x64. CUDA setup depends on your GPU and driver; no automatic installation is offered. Lc0 CUDA also needs Visual Studio 2022.",
            "https://developer.nvidia.com/cuda-12-9-1-download-archive",
            false,
        ),
    ];
    specs
        .into_iter()
        .map(|(id, name, path, details, url, available)| Tool {
            id: id.into(),
            name: name.into(),
            status: path
                .as_ref()
                .map(|p| format!("Detected: {}", p.display()))
                .unwrap_or_else(|| "Not detected".into()),
            details: format!("{details}{}", if winget_package(id).is_some() && !winget { " WinGet is not detected. Use Official downloads or install Microsoft's App Installer first." } else if id == "meson" && python(o).is_none() { " Install Python first to enable this action." } else if id == "gcc" && !msys.join("usr/bin/bash.exe").is_file() { " Install MSYS2 and select its root first to enable this action." } else { "" }),
            url: url.into(),
            install_available: available && path.is_none(),
        })
        .collect()
}
fn validate(o: &SetupOptions) -> Result<()> {
    if !Path::new(&o.destination_root).is_absolute() {
        return Err("Choose an absolute tools folder.".into());
    }
    if !catalog(o).iter().any(|t| t.id == o.tool_id) {
        return Err("Select a supported build tool.".into());
    }
    if o.tool_id == "cuda" {
        return Err("CUDA requires its official interactive download page.".into());
    }
    Ok(())
}
fn install(
    o: &SetupOptions,
    job: &Path,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<String> {
    let root = Path::new(&o.destination_root);
    if catalog(o)
        .iter()
        .find(|t| t.id == o.tool_id)
        .is_some_and(|t| t.status.starts_with("Detected:"))
    {
        return Ok("The selected tool is already detected. No installation was run.".into());
    }
    if let Some(package) = winget_package(&o.tool_id) {
        let winget = find_tool("winget.exe").ok_or("WinGet is unavailable. Use Official downloads, or install Microsoft's App Installer from the Microsoft Store.")?;
        phase(shared, "Run the selected tool's interactive installer");
        run(
            Command::new(winget).args([
                "install",
                "--id",
                package,
                "--exact",
                "--source",
                "winget",
                "--interactive",
                "--no-upgrade",
                "--accept-source-agreements",
                "--accept-package-agreements",
            ]),
            shared,
            cancel,
            Duration::from_secs(7200),
            None,
        )?;
        return Ok("Installer finished. Complete any installer prompts, then restart this app and check tool detection. A C++ workload must be selected in the Visual Studio installer.".into());
    }
    match o.tool_id.as_str() {
        "gcc" => {
            let msys = Path::new(&o.msys_root);
            if !msys.is_absolute() || !msys.join("usr/bin/bash.exe").is_file() {
                return Err("Select the installed MSYS2 root in the engine form first.".into());
            }
            phase(shared, "Install MSYS2 UCRT64 compiler packages");
            let path = std::env::join_paths(
                [msys.join("ucrt64/bin"), msys.join("usr/bin")]
                    .into_iter()
                    .chain(std::env::split_paths(
                        &std::env::var_os("PATH").unwrap_or_default(),
                    )),
            )
            .map_err(|e| e.to_string())?;
            run(Command::new(msys.join("usr/bin/bash.exe")).args(["--noprofile", "--norc", "-c", "exec pacman -S --needed --noconfirm git make curl wget coreutils mingw-w64-ucrt-x86_64-gcc"])
                .env("PATH", path).env("MSYSTEM", "UCRT64").env("CHERE_INVOKING", "1"), shared, cancel, Duration::from_secs(3600), None)?;
            if !toolchain_valid(msys) {
                return Err("Package setup finished but UCRT64 GCC is incomplete. Review the saved log and MSYS2 update instructions.".into());
            }
        }
        "meson" => {
            let base_python =
                python(o).ok_or("Select a Python executable or install Python first.")?;
            let venv = root.join("lc0");
            let executable = venv.join("Scripts/python.exe");
            if !executable.is_file() {
                phase(shared, "Create the app-private Python environment");
                run(
                    Command::new(base_python).args(["-m", "venv"]).arg(&venv),
                    shared,
                    cancel,
                    Duration::from_secs(180),
                    None,
                )?;
            }
            phase(shared, "Install Meson and Ninja in the private environment");
            run(
                Command::new(&executable).args([
                    "-m",
                    "pip",
                    "install",
                    "meson==1.12.1",
                    "ninja==1.13.2",
                ]),
                shared,
                cancel,
                Duration::from_secs(1200),
                None,
            )?;
            run(
                Command::new(&executable).args(["-m", "mesonbuild.mesonmain", "--version"]),
                shared,
                cancel,
                Duration::from_secs(30),
                None,
            )?;
            run(
                Command::new(venv.join("Scripts/ninja.exe")).arg("--version"),
                shared,
                cancel,
                Duration::from_secs(30),
                None,
            )?;
        }
        "zig" => {
            let system = PathBuf::from(
                std::env::var_os("SystemRoot").unwrap_or_else(|| "C:/Windows".into()),
            )
            .join("System32");
            let archive = job.join("zig-0.17.0.zip");
            phase(shared, "Download Zig 0.17.0 from ziglang.org");
            run(
                Command::new(system.join("curl.exe"))
                    .args(["--fail", "--location", "--retry", "2", "--output"])
                    .arg(&archive)
                    .arg(ZIG_URL),
                shared,
                cancel,
                Duration::from_secs(1800),
                None,
            )?;
            let mut file = File::open(&archive).map_err(|e| e.to_string())?;
            let mut hash = Sha256::new();
            std::io::copy(&mut file, &mut hash).map_err(|e| e.to_string())?;
            if format!("{:x}", hash.finalize()) != ZIG_SHA256 {
                return Err("The Zig archive does not match its published SHA-256. It was not extracted or executed.".into());
            }
            let staging = job.join("extracted");
            fs::create_dir(&staging).map_err(|e| e.to_string())?;
            phase(shared, "Extract verified Zig toolchain");
            run(
                Command::new(system.join("tar.exe"))
                    .arg("-xf")
                    .arg(&archive)
                    .arg("-C")
                    .arg(&staging),
                shared,
                cancel,
                Duration::from_secs(180),
                None,
            )?;
            let staged_exe = staging.join("zig-x86_64-windows-0.17.0/zig.exe");
            let version = run(
                Command::new(&staged_exe).arg("version"),
                shared,
                cancel,
                Duration::from_secs(30),
                None,
            )?;
            if version.trim() != "0.17.0" {
                return Err("The extracted Zig version is not 0.17.0.".into());
            }
            let destination = root.join("zig/0.17.0");
            if destination.exists() {
                return Err("A Zig destination folder already exists but is incomplete. It was preserved; review it before retrying.".into());
            }
            fs::create_dir_all(root.join("zig")).map_err(|e| e.to_string())?;
            fs::rename(staging, destination).map_err(|e| e.to_string())?;
        }
        _ => return Err("Unsupported setup action.".into()),
    }
    fs::write(job.join("setup.json"), serde_json::to_vec_pretty(&serde_json::json!({"tool":o.tool_id,"options":o,"zigUrl":if o.tool_id == "zig" {Some(ZIG_URL)} else {None},"zigArchiveSha256":if o.tool_id == "zig" {Some(ZIG_SHA256)} else {None},"completed":true})).unwrap()).map_err(|e|e.to_string())?;
    Ok("Tool setup completed. Use Check tools to refresh detection. Existing settings and engine jobs were preserved.".into())
}
impl Compiler {
    pub fn start_tool_setup(&self, options: SetupOptions) -> Result<()> {
        let _gate = self.gate.lock().unwrap();
        validate(&options)?;
        if matches!(self.snapshot().state.as_str(), "Running" | "Cancelling") {
            return Err("An operation is already running.".into());
        }
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
        let job = Path::new(&options.destination_root)
            .join("setup-jobs")
            .join(format!(
                "{}-{}-{}",
                SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .unwrap()
                    .as_millis(),
                std::process::id(),
                JOB_SEQUENCE.fetch_add(1, Ordering::Relaxed)
            ));
        fs::create_dir_all(&job).map_err(|e| e.to_string())?;
        self.cancel.store(false, Ordering::SeqCst);
        {
            let mut state = self.shared.lock().unwrap();
            state.lines.clear();
            state.host_reported = false;
            state.log = Some(File::create(job.join("setup.log")).map_err(|e| e.to_string())?);
            state.snapshot = Snapshot {
                state: "Running".into(),
                phase: "Set up build tools".into(),
                log_path: job.join("setup.log").to_string_lossy().into(),
                ..Snapshot::default()
            };
        }
        let shared = self.shared.clone();
        let cancel = self.cancel.clone();
        *self.worker.lock().unwrap() = Some(thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                install(&options, &job, &shared, &cancel)
            }));
            let mut state = shared.lock().unwrap();
            let (status,message)=match result { _ if cancel.load(Ordering::SeqCst)=>("Cancelled","Setup cancelled. Logs and any partially installed tools were preserved. Check their installers before retrying.".into()), Ok(Ok(message))=>("Completed",message), Ok(Err(e))=>("Failed",e), Err(_)=>("Failed","An internal tool setup error occurred. Review the saved log.".into()) };
            state.snapshot.state = status.into();
            state.snapshot.message = message;
            let message = state.snapshot.message.clone();
            state.line(&message);
            if let Some(log) = state.log.as_mut() {
                let _ = log.flush();
            }
            state.log = None;
        }));
        Ok(())
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn setup_actions_are_allowlisted_and_require_absolute_storage() {
        assert_eq!(winget_package("rust"), Some("Rustlang.Rustup"));
        assert!(winget_package("rust;calc").is_none());
        let mut o = SetupOptions {
            tool_id: "unknown".into(),
            ..SetupOptions::default()
        };
        assert!(validate(&o).is_err());
        o.tool_id = "zig".into();
        o.destination_root = "relative".into();
        assert!(validate(&o).is_err());
    }
}
