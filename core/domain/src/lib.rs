use serde::{Deserialize, Serialize};
use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{BufRead, BufReader, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
        mpsc,
    },
    thread::{self, JoinHandle},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

type Result<T> = std::result::Result<T, String>;
mod berserk;
pub mod github;
pub mod hardware;
mod lc0;
mod obsidian;
mod reckless;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct BuildOptions {
    pub engine: String,
    pub source_ref: String,
    pub msys_root: String,
    pub output_root: String,
    pub architecture: String,
    pub jobs: u32,
    pub pgo: bool,
    pub strip: bool,
    pub backend: String,
    pub python_path: String,
    pub visual_studio: String,
    pub cuda_root: String,
    pub weights_path: String,
    pub auto_architecture: bool,
}

impl Default for BuildOptions {
    fn default() -> Self {
        Self {
            engine: "stockfish".into(),
            source_ref: "sf_19".into(),
            msys_root: detect_msys().unwrap_or_default(),
            output_root: String::new(),
            architecture: "native".into(),
            jobs: thread::available_parallelism()
                .map(|n| n.get() as u32)
                .unwrap_or(2)
                .min(16),
            pgo: true,
            strip: true,
            backend: "eigen".into(),
            python_path: String::new(),
            visual_studio: String::new(),
            cuda_root: String::new(),
            weights_path: String::new(),
            auto_architecture: false,
        }
    }
}

pub fn defaults(engine: &str) -> BuildOptions {
    match engine {
        "lc0" => lc0::defaults(),
        "berserk" => berserk::defaults(),
        "obsidian" => obsidian::defaults(),
        "reckless" => reckless::defaults(),
        _ => BuildOptions {
            auto_architecture: true,
            architecture: hardware::detect().recommended,
            ..BuildOptions::default()
        },
    }
}

fn engine_name(engine: &str) -> &'static str {
    match engine {
        "lc0" => "Lc0",
        "berserk" => "Berserk",
        "obsidian" => "Obsidian",
        "reckless" => "Reckless",
        _ => "Stockfish",
    }
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Snapshot {
    pub state: String,
    pub phase: String,
    pub message: String,
    pub output: String,
    pub log_path: String,
    pub log_tail: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Plan {
    pub steps: Vec<String>,
    pub options: BuildOptions,
}

pub fn detect_msys() -> Option<String> {
    let mut roots = Vec::new();
    if let Some(root) = std::env::var_os("CEC_MSYS_ROOT") {
        roots.push(PathBuf::from(root));
    }
    if let Some(paths) = std::env::var_os("PATH") {
        for path in std::env::split_paths(&paths) {
            if let Some(root) = path.parent().and_then(Path::parent) {
                roots.push(root.to_path_buf());
            }
        }
    }
    roots.extend([PathBuf::from("C:/msys64"), PathBuf::from("D:/msys64")]);
    roots
        .into_iter()
        .find(|r| toolchain_valid(r))
        .map(|p| p.to_string_lossy().into_owned())
}

fn toolchain_valid(root: &Path) -> bool {
    ["usr/bin/bash.exe", "usr/bin/make.exe", "ucrt64/bin/g++.exe"]
        .iter()
        .all(|p| root.join(p).is_file())
}

fn git_executable(root: &Path) -> Option<PathBuf> {
    let bundled = root.join("usr/bin/git.exe");
    if bundled.is_file() {
        return Some(bundled);
    }
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|path| path.join("git.exe"))
        .find(|path| path.is_file())
}

pub fn plan(options: &BuildOptions) -> Result<Plan> {
    let mut resolved = options.clone();
    if resolved.auto_architecture {
        match resolved.engine.as_str() {
            "stockfish" => resolved.architecture = hardware::detect().recommended,
            "berserk" => resolved.architecture = hardware::detect().berserk_recommended,
            "obsidian" => resolved.architecture = hardware::detect().obsidian_recommended,
            "reckless" => resolved.architecture = hardware::detect().reckless_recommended,
            _ => {}
        }
    }
    let options = &resolved;
    if !matches!(
        options.engine.as_str(),
        "stockfish" | "lc0" | "berserk" | "obsidian" | "reckless"
    ) {
        return Err("Select Stockfish, Lc0, Berserk, Obsidian or Reckless.".into());
    }
    if options.source_ref.is_empty()
        || options.source_ref.starts_with('-')
        || options.source_ref.len() > 160
        || !options
            .source_ref
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-._/".contains(c))
    {
        return Err(
            "Enter a source tag, branch or commit using letters, digits, /, -, _ or .".into(),
        );
    }
    if !(1..=256).contains(&options.jobs) {
        return Err("Parallel jobs must be between 1 and 256.".into());
    }
    if !Path::new(&options.output_root).is_absolute() {
        return Err("Choose an absolute output folder.".into());
    }
    if options.engine == "lc0" {
        return lc0::plan(options);
    }
    if options.engine == "berserk" {
        return berserk::plan(options);
    }
    if options.engine == "obsidian" {
        return obsidian::plan(options);
    }
    if options.engine == "reckless" {
        return reckless::plan(options);
    }
    let compatible = hardware::detect().choices.contains(&options.architecture);
    if !compatible {
        return Err(
            "This CPU cannot run the selected architecture. Select native or x86-64.".into(),
        );
    }
    if !Path::new(&options.output_root).is_absolute() {
        return Err("Choose an absolute output folder.".into());
    }
    if !toolchain_valid(Path::new(&options.msys_root)) {
        return Err("MSYS2 UCRT64 is incomplete. Select its root folder; install make, git and mingw-w64-ucrt-x86_64-gcc in MSYS2.".into());
    }
    if git_executable(Path::new(&options.msys_root)).is_none() {
        return Err(
            "Git was not found. Install Git for Windows or MSYS2 git, then restart the app.".into(),
        );
    }
    Ok(Plan { options: options.clone(), steps: vec![
        format!("Fetch official-stockfish/Stockfish at {} into a new job folder", options.source_ref),
        "Record the resolved commit and tool versions".into(),
        "Download the NNUE networks specified by that source".into(),
        format!("make -j{} {} ARCH={} COMP=mingw", options.jobs, if options.pgo { "profile-build" } else { "build" }, options.architecture),
        if options.strip { "Strip executable symbols".into() } else { "Keep executable symbols".into() },
        "Verify the UCI handshake; save executable, source archive, license, manifest and full log".into(),
    ] })
}

struct Shared {
    host_reported: bool,
    snapshot: Snapshot,
    lines: VecDeque<String>,
    log: Option<File>,
}

impl Shared {
    fn line(&mut self, text: &str) {
        // Stockfish's Makefile reports four real PGO stages; do not invent percentages.
        if text.starts_with("Step ") && text.as_bytes().get(6..10) == Some(b"/4. ") {
            self.snapshot.phase = format!("PGO: {}", text.trim_end_matches(" ..."));
        }
        if let Some(log) = self.log.as_mut() {
            let _ = writeln!(log, "{text}");
        }
        self.lines.push_back(text.chars().take(4096).collect());
        while self.lines.len() > 300 {
            self.lines.pop_front();
        }
    }
}

pub struct Compiler {
    pub github: Mutex<github::Worker>,
    shared: Arc<Mutex<Shared>>,
    cancel: Arc<AtomicBool>,
    worker: Mutex<Option<JoinHandle<()>>>,
    gate: Mutex<()>,
}

impl Default for Compiler {
    fn default() -> Self {
        Self {
            github: Mutex::new(github::Worker::default()),
            shared: Arc::new(Mutex::new(Shared {
                host_reported: false,
                snapshot: Snapshot {
                    state: "Ready".into(),
                    phase: "Configure build".into(),
                    ..Snapshot::default()
                },
                lines: VecDeque::new(),
                log: None,
            })),
            cancel: Arc::new(AtomicBool::new(false)),
            worker: Mutex::new(None),
            gate: Mutex::new(()),
        }
    }
}

impl Compiler {
    pub fn snapshot(&self) -> Snapshot {
        let shared = self.shared.lock().unwrap();
        let mut snapshot = shared.snapshot.clone();
        snapshot.log_tail = shared.lines.iter().cloned().collect::<Vec<_>>().join("\n");
        snapshot
    }

    pub fn start(&self, options: BuildOptions) -> Result<()> {
        let _gate = self.gate.lock().unwrap();
        let options = plan(&options)?.options;
        if matches!(self.snapshot().state.as_str(), "Running" | "Cancelling") {
            return Err("A build is already running.".into());
        }
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
        self.cancel.store(false, Ordering::SeqCst);
        {
            let mut shared = self.shared.lock().unwrap();
            shared.lines.clear();
            shared.host_reported = false;
            shared.snapshot = Snapshot {
                state: "Running".into(),
                phase: "Prepare workspace".into(),
                ..Snapshot::default()
            };
        }
        let shared = self.shared.clone();
        let cancel = self.cancel.clone();
        *self.worker.lock().unwrap() = Some(thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build(&options, &shared, &cancel)
            }));
            let mut state = shared.lock().unwrap();
            let (status, message) = match result {
                Ok(Ok(output)) => {
                    state.snapshot.output = output;
                    (
                        "Completed",
                        format!(
                            "{} compiled; UCI and standalone search checks passed.",
                            engine_name(&options.engine)
                        ),
                    )
                }
                _ if cancel.load(Ordering::SeqCst) => (
                    "Cancelled",
                    "Build cancelled. Source and log remain in the job folder.".into(),
                ),
                Ok(Err(error)) => ("Failed", error),
                Err(_) => (
                    "Failed",
                    "An internal build error occurred. Review the saved log.".into(),
                ),
            };
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

    pub fn cancel(&self) {
        let mut shared = self.shared.lock().unwrap();
        if shared.snapshot.state == "Running" {
            shared.snapshot.state = "Cancelling".into();
            shared.snapshot.message = "Stopping the build process tree…".into();
            self.cancel.store(true, Ordering::SeqCst);
        }
    }
}

impl Drop for Compiler {
    fn drop(&mut self) {
        self.cancel.store(true, Ordering::SeqCst);
        if let Some(worker) = self.worker.get_mut().unwrap().take() {
            let _ = worker.join();
        }
    }
}

fn hidden(command: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
}

fn stop_tree(child: &mut std::process::Child) {
    #[cfg(windows)]
    {
        let _ =
            hidden(Command::new("taskkill").args(["/PID", &child.id().to_string(), "/T", "/F"]))
                .output();
    }
    let _ = child.kill();
    let _ = child.wait();
}

fn run(
    command: &mut Command,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
    timeout: Duration,
    input: Option<&str>,
) -> Result<String> {
    shared.lock().unwrap().line(&format!("$ {command:?}"));
    run_unlogged(command, shared, cancel, timeout, input)
}

fn powershell_executable() -> Option<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|p| p.join("pwsh.exe"))
        .chain(
            std::env::var_os("ProgramFiles")
                .map(PathBuf::from)
                .map(|p| p.join("PowerShell/7/pwsh.exe")),
        )
        .find(|p| p.is_file())
}

fn powershell_command(original: &Command, host: &Path) -> Result<Command> {
    let arguments: Vec<_> = original
        .get_args()
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let spec = serde_json::json!({"executable":original.get_program().to_string_lossy(),"arguments":arguments});
    let mut command = Command::new(host);
    command.args(["-NoLogo", "-NoProfile", "-NonInteractive", "-Command", "$ErrorActionPreference='Stop'; $PSNativeCommandArgumentPassing='Standard'; $PSNativeCommandUseErrorActionPreference=$false; $PSStyle.OutputRendering='PlainText'; try { $spec=ConvertFrom-Json -InputObject $env:CEC_PROCESS_SPEC; Remove-Item Env:CEC_PROCESS_SPEC; & $spec.executable @($spec.arguments); exit $LASTEXITCODE } catch { [Console]::Error.WriteLine('PowerShell could not start the selected build tool.'); exit 1 }"]);
    for (key, value) in original.get_envs() {
        if let Some(value) = value {
            command.env(key, value);
        } else {
            command.env_remove(key);
        }
    }
    command.env("CEC_PROCESS_SPEC", spec.to_string());
    if let Some(dir) = original.get_current_dir() {
        command.current_dir(dir);
    }
    Ok(command)
}

fn run_unlogged(
    command: &mut Command,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
    timeout: Duration,
    input: Option<&str>,
) -> Result<String> {
    if cancel.load(Ordering::SeqCst) {
        return Err("Cancelled".into());
    }
    let mut hosted;
    let command = if input.is_none()
        && let Some(host) = powershell_executable()
    {
        hosted = powershell_command(command, &host)?;
        let mut state = shared.lock().unwrap();
        if !state.host_reported {
            state.line("Process host: PowerShell 7 (interactive UCI checks use direct input)");
            state.host_reported = true;
        }
        &mut hosted
    } else {
        if input.is_none() {
            let mut state = shared.lock().unwrap();
            if !state.host_reported {
                state.line("Process host: native (PowerShell 7 was not found)");
                state.host_reported = true;
            }
        }
        command
    };
    let mut child = hidden(command)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .map_err(|e| format!("Could not start a build tool: {e}"))?;
    if let Some(input) = input {
        if let Some(mut stdin) = child.stdin.take() {
            let _ = stdin.write_all(input.as_bytes());
        }
    } else {
        drop(child.stdin.take());
    }
    let (tx, rx) = mpsc::channel();
    let mut readers = Vec::new();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();
    for pipe in [
        Box::new(stdout) as Box<dyn std::io::Read + Send>,
        Box::new(stderr),
    ] {
        let tx = tx.clone();
        readers.push(thread::spawn(move || {
            for line in BufReader::new(pipe)
                .lines()
                .map_while(std::result::Result::ok)
            {
                if tx.send(line).is_err() {
                    break;
                }
            }
        }));
    }
    drop(tx);
    let started = std::time::Instant::now();
    let mut captured = String::new();
    let status = loop {
        for line in rx.try_iter() {
            shared.lock().unwrap().line(&line);
            if captured.len() < 256 * 1024 {
                captured.push_str(&line);
                captured.push('\n');
            }
        }
        if cancel.load(Ordering::SeqCst) || started.elapsed() > timeout {
            stop_tree(&mut child);
            for reader in readers {
                let _ = reader.join();
            }
            for line in rx.try_iter() {
                shared.lock().unwrap().line(&line);
            }
            return Err(if cancel.load(Ordering::SeqCst) {
                "Cancelled"
            } else {
                "Build tool timed out. Review the log and retry."
            }
            .into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(80)),
            Err(e) => {
                stop_tree(&mut child);
                return Err(e.to_string());
            }
        }
    };
    for reader in readers {
        let _ = reader.join();
    }
    for line in rx.try_iter() {
        shared.lock().unwrap().line(&line);
        if captured.len() < 256 * 1024 {
            captured.push_str(&line);
            captured.push('\n');
        }
    }
    if !status.success() {
        return Err(format!(
            "Build tool exited with {status}. Review the log, correct the reported problem and retry."
        ));
    }
    Ok(captured)
}

fn phase(shared: &Arc<Mutex<Shared>>, text: &str) {
    let mut shared = shared.lock().unwrap();
    shared.snapshot.phase = text.into();
    shared.line(text);
}

static JOB_SEQUENCE: AtomicU64 = AtomicU64::new(0);

fn build(
    options: &BuildOptions,
    shared: &Arc<Mutex<Shared>>,
    cancel: &AtomicBool,
) -> Result<String> {
    let job_id = format!(
        "{}-{}-{}",
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
        std::process::id(),
        JOB_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    );
    let job = Path::new(&options.output_root).join("jobs").join(&job_id);
    fs::create_dir_all(&job).map_err(|e| format!("Could not create the output folder: {e}"))?;
    let log_path = job.join("build.log");
    {
        let mut shared = shared.lock().unwrap();
        shared.log = Some(File::create(&log_path).map_err(|e| e.to_string())?);
        shared.snapshot.log_path = log_path.to_string_lossy().into_owned();
    }
    if options.engine == "lc0" {
        return lc0::build(options, &job, shared, cancel);
    }
    if options.engine == "berserk" {
        return berserk::build(options, &job, shared, cancel);
    }
    if options.engine == "obsidian" {
        return obsidian::build(options, &job, shared, cancel);
    }
    if options.engine == "reckless" {
        return reckless::build(options, &job, shared, cancel);
    }
    let root = Path::new(&options.msys_root);
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
    let execute_git = |args: &[&str]| {
        run(
            Command::new(&git)
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
    phase(shared, "Fetch Stockfish source");
    execute_git(&["init", "--quiet"])?;
    execute_git(&[
        "remote",
        "add",
        "origin",
        "https://github.com/official-stockfish/Stockfish.git",
    ])?;
    execute_git(&[
        "-c",
        "core.hooksPath=NUL",
        "fetch",
        "--depth=1",
        "origin",
        &options.source_ref,
    ])?;
    execute_git(&[
        "-c",
        "core.hooksPath=NUL",
        "checkout",
        "--detach",
        "FETCH_HEAD",
    ])?;
    let commit = execute_git(&["rev-parse", "HEAD"])?;
    let make = |args: &[&str]| {
        run(
            Command::new(root.join("usr/bin/bash.exe"))
                .args(["--noprofile", "--norc", "-c", "exec make \"$@\"", "cec"])
                .args(args)
                .current_dir(source.join("src"))
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
        Command::new(root.join("ucrt64/bin/g++.exe"))
            .arg("--version")
            .env("PATH", &path),
        shared,
        cancel,
        Duration::from_secs(30),
        None,
    )?;
    phase(shared, "Download NNUE networks");
    make(&["net"])?;
    phase(
        shared,
        if options.pgo {
            "Compile with profile-guided optimization"
        } else {
            "Compile Stockfish"
        },
    );
    let jobs = format!("-j{}", options.jobs);
    let arch = format!("ARCH={}", options.architecture);
    make(&[
        &jobs,
        if options.pgo {
            "profile-build"
        } else {
            "build"
        },
        &arch,
        "COMP=mingw",
    ])?;
    if options.strip {
        phase(shared, "Strip executable symbols");
        make(&["strip", &arch, "COMP=mingw"])?;
    }
    phase(shared, "Verify UCI handshake");
    let executable = source.join("src/stockfish.exe");
    let uci = run(
        Command::new(&executable)
            .current_dir(source.join("src"))
            .env("PATH", &path),
        shared,
        cancel,
        Duration::from_secs(60),
        Some("uci\nisready\nquit\n"),
    )?;
    if !uci.lines().any(|line| line.trim() == "uciok")
        || !uci.lines().any(|line| line.trim() == "readyok")
    {
        return Err(
            "The compiled engine did not complete its UCI handshake. Review the log.".into(),
        );
    }
    phase(shared, "Save build result");
    let result = job.join("result");
    fs::create_dir(&result).map_err(|e| e.to_string())?;
    let output = result.join("stockfish.exe");
    fs::copy(&executable, &output).map_err(|e| e.to_string())?;
    phase(shared, "Verify standalone search");
    let system_path =
        PathBuf::from(std::env::var_os("SystemRoot").unwrap_or_else(|| "C:/Windows".into()))
            .join("System32");
    let bench = run(
        Command::new(&output)
            .args(["bench", "16", "1", "4", "default", "depth"])
            .current_dir(&result)
            .env("PATH", system_path),
        shared,
        cancel,
        Duration::from_secs(60),
        None,
    )?;
    if !bench.contains("Nodes searched") {
        return Err(
            "The standalone engine did not finish its search benchmark. Review the log.".into(),
        );
    }
    // Save the exact tracked source corresponding to the compiled binary.
    execute_git(&[
        "archive",
        "--format=tar",
        "--output=../result/source.tar",
        "HEAD",
    ])?;
    for name in ["Copying.txt", "AUTHORS"] {
        if source.join(name).is_file() {
            fs::copy(source.join(name), result.join(name)).map_err(|e| e.to_string())?;
        }
    }
    let manifest = serde_json::json!({ "schemaVersion": 1, "engine": "stockfish", "sourceUrl": "https://github.com/official-stockfish/Stockfish", "commit": commit.trim(), "options": options, "compiler": compiler, "uciVerified": true, "standaloneSearchVerified": true, "logPath": log_path, "sourceArchive": "source.tar" });
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
    fn options() -> (tempfile::TempDir, BuildOptions) {
        let dir = tempfile::tempdir().unwrap();
        for name in [
            "usr/bin/bash.exe",
            "usr/bin/make.exe",
            "ucrt64/bin/g++.exe",
            "usr/bin/git.exe",
        ] {
            let path = dir.path().join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            fs::write(path, []).unwrap();
        }
        let options = BuildOptions {
            msys_root: dir.path().to_string_lossy().into(),
            output_root: dir.path().join("out").to_string_lossy().into(),
            ..BuildOptions::default()
        };
        (dir, options)
    }
    #[test]
    fn rejects_unsafe_or_invalid_build_inputs() {
        let (_dir, mut options) = options();
        assert!(plan(&options).is_ok());
        for reference in [
            "--upload-pack=evil",
            "master;calc",
            "$(calc)",
            "",
            "foo\nbar",
        ] {
            options.source_ref = reference.into();
            assert!(plan(&options).is_err());
        }
        options.source_ref = "sf_19".into();
        options.jobs = 0;
        assert!(plan(&options).is_err());
        options.jobs = 2;
        options.engine = "lc0".into();
        assert!(plan(&options).is_err());
        options.engine = "crafty".into();
        assert!(plan(&options).is_err());
        options.engine = "stockfish".into();
        options.output_root = "relative".into();
        assert!(plan(&options).is_err());
    }
    #[test]
    fn cancellation_stops_a_process_tree() {
        let core = Compiler::default();
        let shared = core.shared.clone();
        let cancel = core.cancel.clone();
        let task = thread::spawn(move || {
            run(
                Command::new("powershell.exe").args([
                    "-NoProfile",
                    "-Command",
                    "Start-Sleep -Seconds 30",
                ]),
                &shared,
                &cancel,
                Duration::from_secs(60),
                None,
            )
        });
        thread::sleep(Duration::from_millis(400));
        core.cancel.store(true, Ordering::SeqCst);
        let started = std::time::Instant::now();
        assert!(task.join().unwrap().is_err());
        assert!(started.elapsed() < Duration::from_secs(10));
    }
}
