//! GitHub browsing and downloads run independently from engine compilation.
use super::*;
use reqwest::{blocking::Client, header};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;

#[derive(Clone, Default)]
pub struct Query {
    pub action: i32,
    pub engine: String,
    pub auth_mode: String,
    pub token: String,
    pub page: u32,
    pub id: u64,
    pub output_folder: String,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct SourceVersion {
    pub id: u64,
    pub label: String,
    pub reference: String,
    pub date: String,
    pub url: String,
    pub prerelease: bool,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct DownloadItem {
    pub id: u64,
    pub name: String,
    pub size: u64,
    pub date: String,
    pub digest: String,
    pub expired: bool,
    pub details: String,
}
#[derive(Clone, Debug, Default, Serialize)]
pub struct NetworkSnapshot {
    pub state: String,
    pub message: String,
    pub releases: Vec<SourceVersion>,
    pub development: Vec<SourceVersion>,
    pub items: Vec<DownloadItem>,
    pub login: String,
    pub output: String,
    pub received: u64,
    pub total: u64,
    pub page: u32,
    pub more: bool,
    pub default_branch: String,
    pub releases_more: bool,
    pub development_more: bool,
}
pub struct Worker {
    shared: Arc<Mutex<NetworkSnapshot>>,
    cancel: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}
impl Default for Worker {
    fn default() -> Self {
        Self {
            shared: Arc::new(Mutex::new(NetworkSnapshot::default())),
            cancel: Arc::new(AtomicBool::new(false)),
            thread: None,
        }
    }
}
impl Worker {
    pub fn snapshot(&self) -> NetworkSnapshot {
        self.shared.lock().unwrap().clone()
    }
    pub fn cancel(&self) {
        self.cancel.store(true, Ordering::SeqCst);
    }
    pub fn start(&mut self, query: Query) -> Result<()> {
        if self.snapshot().state == "Running" {
            return Err("A GitHub operation is already running.".into());
        }
        repository(&query.engine)?;
        if !(0..=6).contains(&query.action) {
            return Err("Unsupported GitHub operation.".into());
        }
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
        self.cancel.store(false, Ordering::SeqCst);
        *self.shared.lock().unwrap() = NetworkSnapshot {
            state: "Running".into(),
            message: "Contacting GitHub…".into(),
            page: query.page.max(1),
            ..Default::default()
        };
        let (shared, cancel) = (self.shared.clone(), self.cancel.clone());
        self.thread = Some(thread::spawn(move || {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                execute(&query, &shared, &cancel)
            }));
            let mut state = shared.lock().unwrap();
            if cancel.load(Ordering::SeqCst) {
                state.state = "Cancelled".into();
                state.message = "GitHub operation cancelled.".into();
            } else {
                match result {
                    Ok(Ok(())) => {
                        state.state = "Completed".into();
                        if state.message == "Contacting GitHub…" {
                            state.message = "GitHub data loaded.".into();
                        }
                    }
                    Ok(Err(error)) => {
                        state.state = "Failed".into();
                        state.message = error;
                    }
                    Err(_) => {
                        state.state = "Failed".into();
                        state.message = "GitHub operation failed internally.".into();
                    }
                }
            }
        }));
        Ok(())
    }
}
impl Drop for Worker {
    fn drop(&mut self) {
        self.cancel();
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}

fn repository(engine: &str) -> Result<&'static str> {
    match engine {
        "stockfish" => Ok("official-stockfish/Stockfish"),
        "lc0" => Ok("LeelaChessZero/lc0"),
        _ => Err("Select Stockfish or Lc0.".into()),
    }
}
fn field(value: &Value, name: &str) -> String {
    value[name].as_str().unwrap_or_default().to_owned()
}
fn number(value: &Value, name: &str) -> u64 {
    value[name].as_u64().unwrap_or_default()
}
fn check_cancel(cancel: &AtomicBool) -> Result<()> {
    if cancel.load(Ordering::SeqCst) {
        Err("Cancelled".into())
    } else {
        Ok(())
    }
}
fn gh() -> Result<PathBuf> {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default()).map(|p| p.join("gh.exe")).find(|p| p.is_file())
        .ok_or("GitHub CLI was not found. Install it for browser sign-in, or save a personal access token in Settings > GitHub.".into())
}
fn private_cli_token() -> Result<String> {
    let output = hidden(
        Command::new(gh()?)
            .args(["auth", "token", "--hostname", "github.com"])
            .env_remove("GH_TOKEN")
            .env_remove("GITHUB_TOKEN"),
    )
    .output()
    .map_err(|_| "Could not read GitHub CLI's credential store.")?;
    if !output.status.success() {
        return Err(
            "GitHub CLI is not signed in. Use Settings > GitHub > Sign in through browser.".into(),
        );
    }
    let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if token.is_empty() {
        return Err("GitHub CLI did not supply a credential.".into());
    }
    Ok(token)
}
fn headers(token: &str) -> Result<header::HeaderMap> {
    let mut headers = header::HeaderMap::new();
    headers.insert(
        header::USER_AGENT,
        header::HeaderValue::from_static("Jupiter-ChessEngineCompiler"),
    );
    headers.insert(
        header::ACCEPT,
        header::HeaderValue::from_static("application/vnd.github+json"),
    );
    headers.insert(
        "X-GitHub-Api-Version",
        header::HeaderValue::from_static("2026-03-10"),
    );
    if !token.is_empty() {
        let mut value = header::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| "The GitHub token contains invalid characters.")?;
        value.set_sensitive(true);
        headers.insert(header::AUTHORIZATION, value);
    }
    Ok(headers)
}
fn client(token: &str) -> Result<Client> {
    Client::builder()
        .default_headers(headers(token)?)
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(30))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 || attempt.url().scheme() != "https" {
                attempt.error("Unsafe or excessive redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "Could not initialize HTTPS for GitHub.".into())
}
fn response_error(response: &reqwest::blocking::Response) -> String {
    status_error(response.status().as_u16())
}
fn status_error(status: u16) -> String {
    match status {
        401 => "GitHub rejected the credential. Replace the token or sign in again.".into(),
        403 | 429 => "GitHub denied this request or its API limit was reached. Check token permissions or retry later. Settings > GitHub supports authenticated access.".into(),
        404 => "This GitHub version, asset or workflow artifact is unavailable to this account.".into(),
        410 => "This workflow artifact has expired and can no longer be downloaded.".into(),
        code => format!("GitHub returned HTTP {code}. Retry or check the repository on GitHub."),
    }
}
fn get_json(client: &Client, path: &str, cancel: &AtomicBool) -> Result<(Value, bool)> {
    let mut last = String::new();
    for attempt in 0..3 {
        match get_json_once(client, path, cancel) {
            Ok(value) => return Ok(value),
            Err(error)
                if error.contains("HTTP 5")
                    || error.contains("invalid or oversized")
                    || error.contains("Could not contact") =>
            {
                last = error
            }
            Err(error) => return Err(error),
        }
        if attempt < 2 {
            for _ in 0..(attempt + 1) * 10 {
                check_cancel(cancel)?;
                thread::sleep(Duration::from_millis(100));
            }
        }
    }
    Err(last)
}
fn get_json_once(client: &Client, path: &str, cancel: &AtomicBool) -> Result<(Value, bool)> {
    check_cancel(cancel)?;
    let response = client
        .get(format!("https://api.github.com/{path}"))
        .send()
        .map_err(|_| "Could not contact GitHub. Check the connection and retry.")?;
    if !response.status().is_success() {
        return Err(response_error(&response));
    }
    let more = response
        .headers()
        .get(header::LINK)
        .and_then(|s| s.to_str().ok())
        .is_some_and(|s| s.contains("rel=\"next\""));
    let mut bytes = Vec::new();
    response
        .take(4 * 1024 * 1024)
        .read_to_end(&mut bytes)
        .map_err(|_| "Could not read GitHub's response.")?;
    check_cancel(cancel)?;
    Ok((
        serde_json::from_slice(&bytes)
            .map_err(|_| "GitHub returned an invalid or oversized response.")?,
        more,
    ))
}
fn release(value: &Value) -> SourceVersion {
    let tag = field(value, "tag_name");
    SourceVersion {
        id: number(value, "id"),
        label: format!(
            "{}{}",
            tag,
            if value["prerelease"].as_bool().unwrap_or(false) {
                " (prerelease)"
            } else {
                ""
            }
        ),
        reference: tag,
        date: field(value, "published_at"),
        url: field(value, "html_url"),
        prerelease: value["prerelease"].as_bool().unwrap_or(false),
    }
}
fn item(value: &Value, artifact: bool) -> DownloadItem {
    DownloadItem {
        id: number(value, "id"),
        name: field(value, "name"),
        size: number(value, if artifact { "size_in_bytes" } else { "size" }),
        date: field(value, if artifact { "created_at" } else { "updated_at" }),
        digest: field(value, "digest"),
        expired: value["expired"].as_bool().unwrap_or(false),
        details: if artifact {
            format!(
                "{} · {} · expires {}",
                field(&value["workflow_run"], "head_branch"),
                field(&value["workflow_run"], "head_sha")
                    .chars()
                    .take(12)
                    .collect::<String>(),
                field(value, "expires_at")
            )
        } else {
            field(value, "content_type")
        },
    }
}
fn execute(query: &Query, shared: &Arc<Mutex<NetworkSnapshot>>, cancel: &AtomicBool) -> Result<()> {
    if query.action == 6 {
        sign_in(shared, cancel)?;
    }
    let token = if query.action == 6 {
        private_cli_token()?
    } else {
        match query.auth_mode.as_str() {
            "anonymous" | "" => String::new(),
            "token" => query.token.clone(),
            "cli" => private_cli_token()?,
            _ => return Err("Select anonymous, token or GitHub CLI authentication.".into()),
        }
    };
    if matches!(query.action, 2 | 4 | 5 | 6) && token.is_empty() {
        return Err("Connect a GitHub account or save a token in Settings > GitHub to use workflow artifacts and account features.".into());
    }
    let client = client(&token)?;
    let repo = repository(&query.engine)?;
    let page = query.page.max(1);
    match query.action {
        0 => {
            let (info, _) = get_json(&client, &format!("repos/{repo}"), cancel)?;
            let branch = field(&info, "default_branch");
            let (versions, more_releases) = get_json(
                &client,
                &format!("repos/{repo}/releases?per_page=30&page={page}"),
                cancel,
            )?;
            let releases: Vec<_> = versions
                .as_array()
                .ok_or("GitHub releases were not a list.")?
                .iter()
                .filter(|v| !v["draft"].as_bool().unwrap_or(false))
                .map(release)
                .collect();
            let (commits, more_commits) = get_json(
                &client,
                &format!("repos/{repo}/commits?per_page=30&page={page}"),
                cancel,
            )?;
            let mut development: Vec<_> =
                releases.iter().filter(|r| r.prerelease).cloned().collect();
            development.extend(
                commits
                    .as_array()
                    .ok_or("GitHub commits were not a list.")?
                    .iter()
                    .map(|v| {
                        let sha = field(v, "sha");
                        let date = field(&v["commit"]["committer"], "date");
                        SourceVersion {
                            label: format!(
                                "{} · {} · {}",
                                &date[..date.len().min(10)],
                                sha.chars().take(10).collect::<String>(),
                                field(&v["commit"], "message")
                                    .lines()
                                    .next()
                                    .unwrap_or_default()
                                    .chars()
                                    .take(100)
                                    .collect::<String>()
                            ),
                            reference: sha,
                            date,
                            url: field(v, "html_url"),
                            ..Default::default()
                        }
                    }),
            );
            development.sort_by(|a, b| b.date.cmp(&a.date));
            let mut state = shared.lock().unwrap();
            state.releases = releases;
            state.development = development;
            state.more = more_releases || more_commits;
            state.default_branch = branch;
            state.releases_more = more_releases;
            state.development_more = more_commits;
        }
        1 | 2 => {
            let artifact = query.action == 2;
            let path = if artifact {
                format!("repos/{repo}/actions/artifacts?per_page=30&page={page}")
            } else {
                format!(
                    "repos/{repo}/releases/{}/assets?per_page=30&page={page}",
                    query.id
                )
            };
            let (values, more) = match get_json(&client, &path, cancel) {
                Ok(result) => result,
                Err(error)
                    if artifact
                        && (error.contains("HTTP 5") || error.contains("invalid or oversized")) =>
                {
                    let (items, more) = workflow_artifacts(&client, repo, page, cancel)?;
                    let mut state = shared.lock().unwrap();
                    state.items = items;
                    state.more = more;
                    state.message = format!(
                        "GitHub’s repository artifact list is unavailable; showing workflow-run page {page} instead."
                    );
                    return Ok(());
                }
                Err(error) => return Err(error),
            };
            let array = if artifact {
                &values["artifacts"]
            } else {
                &values
            };
            let items = array
                .as_array()
                .ok_or("GitHub downloads were not a list.")?
                .iter()
                .map(|v| item(v, artifact))
                .collect();
            let mut state = shared.lock().unwrap();
            state.items = items;
            state.more = more;
        }
        3 | 4 => download(query, &client, &token, repo, shared, cancel)?,
        5 | 6 => {
            let (user, _) = get_json(&client, "user", cancel)?;
            shared.lock().unwrap().login = field(&user, "login");
        }
        _ => return Err("Unsupported GitHub operation.".into()),
    }
    Ok(())
}
fn workflow_artifacts(
    client: &Client,
    repo: &str,
    page: u32,
    cancel: &AtomicBool,
) -> Result<(Vec<DownloadItem>, bool)> {
    let (runs, more) = get_json(
        client,
        &format!("repos/{repo}/actions/runs?status=completed&per_page=10&page={page}"),
        cancel,
    )?;
    let mut items = Vec::new();
    for run in runs["workflow_runs"]
        .as_array()
        .ok_or("GitHub workflow runs were not a list.")?
    {
        let mut artifact_page = 1;
        loop {
            let (artifacts, more_artifacts) = get_json(
                client,
                &format!(
                    "repos/{repo}/actions/runs/{}/artifacts?per_page=100&page={artifact_page}",
                    number(run, "id")
                ),
                cancel,
            )?;
            for value in artifacts["artifacts"]
                .as_array()
                .ok_or("GitHub workflow artifacts were not a list.")?
            {
                let mut value = value.clone();
                if value["workflow_run"].is_null() {
                    value["workflow_run"] = serde_json::json!({"head_branch":run["head_branch"],"head_sha":run["head_sha"]});
                }
                items.push(item(&value, true));
            }
            if !more_artifacts {
                break;
            }
            artifact_page += 1;
        }
    }
    items.sort_by(|a, b| b.date.cmp(&a.date));
    Ok((items, more))
}

fn safe_name(name: &str, id: u64, artifact: bool) -> String {
    let name: String = name
        .chars()
        .map(|c| {
            if c.is_control() || "<>:\"/\\|?*".contains(c) {
                '_'
            } else {
                c
            }
        })
        .take(180)
        .collect();
    let name = name.trim_matches(['.', ' ']);
    let stem = name
        .split('.')
        .next()
        .unwrap_or_default()
        .to_ascii_uppercase();
    let reserved = matches!(stem.as_str(), "CON" | "PRN" | "AUX" | "NUL")
        || (stem.len() == 4
            && (stem.starts_with("COM") || stem.starts_with("LPT"))
            && stem.as_bytes()[3].is_ascii_digit());
    let name = if name.is_empty() || reserved {
        format!("asset-{id}")
    } else {
        name.to_owned()
    };
    if artifact {
        format!("{name}.zip")
    } else {
        name
    }
}
fn download(
    query: &Query,
    client: &Client,
    token: &str,
    repo: &str,
    shared: &Arc<Mutex<NetworkSnapshot>>,
    cancel: &AtomicBool,
) -> Result<()> {
    if !Path::new(&query.output_folder).is_absolute() {
        return Err("Choose an absolute download folder.".into());
    }
    let artifact = query.action == 4;
    let path = if artifact {
        format!("repos/{repo}/actions/artifacts/{}", query.id)
    } else {
        format!("repos/{repo}/releases/assets/{}", query.id)
    };
    let (metadata, _) = get_json(client, &path, cancel)?;
    if metadata["expired"].as_bool().unwrap_or(false) {
        return Err("This workflow artifact has expired.".into());
    }
    let name = safe_name(&field(&metadata, "name"), query.id, artifact);
    let folder = Path::new(&query.output_folder).join(format!(
        "{}-{}-{}-{}-{}",
        query.engine,
        query.id,
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis(),
        std::process::id(),
        JOB_SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    fs::create_dir_all(&query.output_folder)
        .map_err(|_| "Could not create the download folder.")?;
    fs::create_dir(&folder).map_err(|_| "Could not create a new download job folder.")?;
    let partial = folder.join(format!("{name}.partial"));
    let result = (|| {
        let url = format!(
            "https://api.github.com/{path}{}",
            if artifact { "/zip" } else { "" }
        );
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|_| "Could not initialize the download worker.")?;
        let (received, total, hasher) = runtime.block_on(receive_download(
            &url, token, &partial, &name, shared, cancel,
        ))?;
        if received == 0 || (total > 0 && received != total) {
            return Err("GitHub returned an incomplete download.".into());
        }
        let digest = format!("sha256:{:x}", hasher.finalize());
        let expected = field(&metadata, "digest");
        if expected.starts_with("sha256:") && !expected.eq_ignore_ascii_case(&digest) {
            return Err("The download's SHA-256 does not match GitHub's published digest.".into());
        }
        check_cancel(cancel)?;
        let output = folder.join(&name);
        fs::rename(&partial, &output).map_err(|_| "Could not finish the download file.")?;
        let manifest = serde_json::json!({"schemaVersion":1,"repository":repo,"kind":if artifact {"workflowArtifact"}else{"releaseAsset"},"id":query.id,"name":name,"sha256":digest,"publishedDigest":expected,"received":received,"sourceUrl":format!("https://api.github.com/{path}")});
        fs::write(
            folder.join("download.json"),
            serde_json::to_vec_pretty(&manifest).unwrap(),
        )
        .map_err(|_| "The file downloaded, but its record could not be saved.")?;
        let mut state = shared.lock().unwrap();
        state.output = output.to_string_lossy().into();
        state.message = "Download saved. Files are not extracted or executed.".into();
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&partial);
    }
    result
}
async fn wait_cancel(cancel: &AtomicBool) {
    while !cancel.load(Ordering::SeqCst) {
        tokio::time::sleep(Duration::from_millis(100)).await;
    }
}
async fn receive_download(
    url: &str,
    token: &str,
    partial: &Path,
    name: &str,
    shared: &Arc<Mutex<NetworkSnapshot>>,
    cancel: &AtomicBool,
) -> Result<(u64, u64, Sha256)> {
    let client = reqwest::Client::builder()
        .default_headers(headers(token)?)
        .connect_timeout(Duration::from_secs(10))
        .read_timeout(Duration::from_secs(15))
        .timeout(Duration::from_secs(1800))
        .redirect(reqwest::redirect::Policy::custom(|attempt| {
            if attempt.previous().len() >= 10 || (attempt.url().scheme() != "https" && !cfg!(test))
            {
                attempt.error("Unsafe redirect")
            } else {
                attempt.follow()
            }
        }))
        .build()
        .map_err(|_| "Could not initialize the download connection.")?;
    let mut response = tokio::select! {
        response=client.get(url).header(header::ACCEPT, if url.ends_with("/zip") { "application/vnd.github+json" } else { "application/octet-stream" }).send()=>response.map_err(|_| "Could not start the GitHub download.")?,
        _=wait_cancel(cancel)=>return Err("Cancelled".into()),
    };
    if !response.status().is_success() {
        return Err(status_error(response.status().as_u16()));
    }
    let total = response.content_length().unwrap_or(0);
    {
        let mut state = shared.lock().unwrap();
        state.message = format!("Downloading {name}");
        state.total = total;
    }
    let mut file = File::create(partial).map_err(|_| "Could not create the download file.")?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    loop {
        let chunk = tokio::select! {
            chunk=response.chunk()=>chunk.map_err(|_| "The GitHub download was interrupted. Retry to start a new download.")?,
            _=wait_cancel(cancel)=>return Err("Cancelled".into()),
        };
        let Some(chunk) = chunk else {
            break;
        };
        file.write_all(&chunk)
            .map_err(|_| "Could not save the download. Check disk space.")?;
        hasher.update(&chunk);
        received += chunk.len() as u64;
        shared.lock().unwrap().received = received;
    }
    file.sync_all()
        .map_err(|_| "Could not finish saving the download.")?;
    Ok((received, total, hasher))
}
fn sign_in(shared: &Arc<Mutex<NetworkSnapshot>>, cancel: &AtomicBool) -> Result<()> {
    let mut child = hidden(
        Command::new(gh()?)
            .args([
                "auth",
                "login",
                "--hostname",
                "github.com",
                "--web",
                "--git-protocol",
                "https",
                "--skip-ssh-key",
            ])
            .env_remove("GH_TOKEN")
            .env_remove("GITHUB_TOKEN"),
    )
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|_| "Could not start GitHub browser sign-in.")?;
    if let Some(mut input) = child.stdin.take() {
        let _ = input.write_all(b"\n");
    }
    let (tx, rx) = mpsc::channel();
    let mut readers = Vec::new();
    for pipe in [
        Box::new(child.stdout.take().unwrap()) as Box<dyn Read + Send>,
        Box::new(child.stderr.take().unwrap()),
    ] {
        let tx = tx.clone();
        readers.push(thread::spawn(move || {
            for line in BufReader::new(pipe)
                .lines()
                .map_while(std::result::Result::ok)
            {
                let _ = tx.send(line);
            }
        }));
    }
    drop(tx);
    let started = std::time::Instant::now();
    let status = loop {
        for line in rx.try_iter() {
            if line.contains("code:") || line.contains("https://github.com/login/device") {
                shared.lock().unwrap().message = line;
            }
        }
        if cancel.load(Ordering::SeqCst) || started.elapsed() > Duration::from_secs(900) {
            stop_tree(&mut child);
            for reader in readers {
                let _ = reader.join();
            }
            return Err("Browser sign-in was cancelled or expired.".into());
        }
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) => thread::sleep(Duration::from_millis(100)),
            Err(_) => {
                stop_tree(&mut child);
                return Err("GitHub sign-in process failed.".into());
            }
        }
    };
    for reader in readers {
        let _ = reader.join();
    }
    if !status.success() {
        return Err(
            "GitHub browser sign-in did not complete. Retry, or use a personal access token."
                .into(),
        );
    }
    shared.lock().unwrap().message = "GitHub browser sign-in completed.".into();
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn filenames_cannot_escape_and_releases_keep_refs() {
        assert_eq!(safe_name("../CON:bad.exe", 1, false), "_CON_bad.exe");
        assert_eq!(safe_name("CON", 1, false), "asset-1");
        assert_eq!(safe_name("workflow", 2, true), "workflow.zip");
        let version =
            release(&serde_json::json!({"id":12,"tag_name":"dev-20261024","prerelease":true}));
        assert_eq!(version.reference, "dev-20261024");
        assert!(version.prerelease);
        assert!(repository("../anything").is_err());
    }

    fn read_headers(stream: &mut std::net::TcpStream) -> String {
        stream
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        let mut bytes = Vec::new();
        let mut byte = [0];
        while !bytes.ends_with(b"\r\n\r\n") {
            if stream.read(&mut byte).unwrap() == 0 {
                break;
            }
            bytes.push(byte[0]);
        }
        String::from_utf8_lossy(&bytes).to_ascii_lowercase()
    }
    #[test]
    fn download_redirect_strips_credentials_and_hashes_exact_bytes() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let server = thread::spawn(move || {
            let (mut first, _) = listener.accept().unwrap();
            let headers = read_headers(&mut first);
            assert!(headers.contains("authorization: bearer fixture-private-token"));
            assert!(headers.contains("accept: application/vnd.github+json"));
            write!(first,"HTTP/1.1 302 Found\r\nLocation: http://localhost:{port}/data\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
            drop(first);
            let (mut second, _) = listener.accept().unwrap();
            let headers = read_headers(&mut second);
            assert!(!headers.contains("authorization:"));
            second
                .write_all(
                    b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\nfixture",
                )
                .unwrap();
        });
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("download.partial");
        let shared = Arc::new(Mutex::new(NetworkSnapshot::default()));
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let (received, total, hash) = rt
            .block_on(receive_download(
                &format!("http://127.0.0.1:{port}/zip"),
                "fixture-private-token",
                &path,
                "test",
                &shared,
                &AtomicBool::new(false),
            ))
            .unwrap();
        assert_eq!((received, total), (7, 7));
        assert_eq!(fs::read(path).unwrap(), b"fixture");
        assert_eq!(hash.finalize(), Sha256::digest(b"fixture"));
        assert!(
            !serde_json::to_string(&*shared.lock().unwrap())
                .unwrap()
                .contains("fixture-private-token")
        );
        server.join().unwrap();
    }
    #[test]
    fn cancellation_interrupts_a_stalled_download() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        let (tx, rx) = mpsc::channel();
        let server = thread::spawn(move || {
            let (mut stream, _) = listener.accept().unwrap();
            let _ = read_headers(&mut stream);
            stream
                .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 100000\r\n\r\nx")
                .unwrap();
            let _ = rx.recv_timeout(Duration::from_secs(5));
        });
        let cancel = Arc::new(AtomicBool::new(false));
        let flag = cancel.clone();
        let trigger = thread::spawn(move || {
            thread::sleep(Duration::from_millis(300));
            flag.store(true, Ordering::SeqCst);
        });
        let dir = tempfile::tempdir().unwrap();
        let shared = Arc::new(Mutex::new(NetworkSnapshot::default()));
        let started = std::time::Instant::now();
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        assert!(
            rt.block_on(receive_download(
                &format!("http://127.0.0.1:{port}/stalled"),
                "",
                &dir.path().join("part"),
                "test",
                &shared,
                &cancel
            ))
            .is_err()
        );
        assert!(started.elapsed() < Duration::from_secs(2));
        let _ = tx.send(());
        server.join().unwrap();
        trigger.join().unwrap();
    }
}
