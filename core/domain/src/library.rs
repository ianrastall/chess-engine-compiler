//! Stable release discovery and sequential builds for the managed library.
use super::*;
use std::collections::{HashMap, HashSet};

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Query {
    pub catalog_root: String,
    pub options: Vec<BuildOptions>,
    pub scope: u32,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Row {
    pub engine: String,
    pub tag: String,
    pub state: String,
    pub message: String,
    pub output: String,
    pub log_path: String,
}

#[derive(Deserialize, Serialize)]
struct Catalog {
    version: u32,
    baseline: HashSet<u64>,
    releases: Vec<github::SourceVersion>,
    #[serde(default)]
    etag: Option<String>,
}

pub(super) fn cached_etag(root: &Path, engine: &str) -> Result<Option<String>> {
    Ok(read_catalog(&catalog_path(root, engine)?)?.and_then(|catalog| catalog.etag))
}

fn catalog_path(root: &Path, engine: &str) -> Result<PathBuf> {
    if !root.is_absolute() || recipes::release_rule(engine).is_none() {
        return Err("Select a supported engine and an absolute library state folder.".into());
    }
    Ok(root.join(format!("{engine}-releases.json")))
}

fn read_catalog(path: &Path) -> Result<Option<Catalog>> {
    match fs::read(path) {
        Ok(data) => {
            let catalog: Catalog = serde_json::from_slice(&data)
                .map_err(|e| format!("Could not read the release catalog: {e}"))?;
            if catalog.version != 1 {
                return Err("The release catalog format is newer than this app.".into());
            }
            Ok(Some(catalog))
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("Could not open the release catalog: {e}")),
    }
}

pub(super) fn observe(
    root: &Path,
    engine: &str,
    mut releases: Vec<github::SourceVersion>,
    etag: Option<String>,
) -> Result<()> {
    let path = catalog_path(root, engine)?;
    fs::create_dir_all(root).map_err(|e| e.to_string())?;
    let lock = fs::OpenOptions::new()
        .write(true)
        .create(true)
        .truncate(false)
        .open(path.with_extension("lock"))
        .map_err(|e| e.to_string())?;
    lock.lock().map_err(|e| e.to_string())?;
    releases.sort_by(|a, b| b.date.cmp(&a.date).then_with(|| b.id.cmp(&a.id)));
    let mut tags = HashSet::new();
    releases.retain(|release| tags.insert(release.reference.clone()));
    let baseline = read_catalog(&path)?.map_or_else(
        || releases.iter().skip(2).map(|release| release.id).collect(),
        |catalog| catalog.baseline,
    );
    let catalog = Catalog {
        version: 1,
        baseline,
        releases,
        etag,
    };
    let tmp = path.with_extension("partial");
    fs::write(&tmp, serde_json::to_vec(&catalog).unwrap()).map_err(|e| e.to_string())?;
    fs::rename(tmp, path).map_err(|e| format!("Could not save the release catalog: {e}"))
}

struct Entry {
    rank: usize,
    options: BuildOptions,
    row: Row,
}

impl Entry {
    fn ready(&self) -> bool {
        matches!(self.row.state.as_str(), "Ready" | "Failed" | "Cancelled")
    }
}

type ReleaseKey = (String, u64, String, String);

#[derive(Default)]
struct History {
    verified: HashMap<(ReleaseKey, String), (PathBuf, Row)>,
    attempts: HashMap<ReleaseKey, (PathBuf, Row)>,
}

#[derive(Deserialize, Serialize)]
struct Attempt {
    recipe: String,
    release_id: u64,
    backend: String,
    row: Row,
}

fn remember<K: Eq + std::hash::Hash>(
    index: &mut HashMap<K, (PathBuf, Row)>,
    key: K,
    folder: &Path,
    row: Row,
) {
    let entry = index.entry(key).or_default();
    if folder > entry.0.as_path() {
        *entry = (folder.to_path_buf(), row);
    }
}

fn history(root: &Path) -> History {
    let mut history = History::default();
    if !root.is_absolute() {
        return history;
    }
    let Ok(jobs) = fs::read_dir(root.join("jobs")) else {
        return history;
    };
    for job in jobs.flatten() {
        let job = job.path();
        if let Ok(bytes) = fs::read(job.join("release-attempt.json"))
            && let Ok(attempt) = serde_json::from_slice::<Attempt>(&bytes)
            && recipes::release_rule(&attempt.row.engine)
                .is_some_and(|rule| rule.id == attempt.recipe)
            && matches!(
                attempt.row.state.as_str(),
                "Failed" | "Cancelled" | "Needs recipe update"
            )
        {
            let key = (
                attempt.row.engine.clone(),
                attempt.release_id,
                attempt.row.tag.clone(),
                attempt.backend,
            );
            remember(&mut history.attempts, key, &job, attempt.row);
        }
        let folder = job.join("result");
        let Ok(bytes) = fs::read(folder.join("build.json")) else {
            continue;
        };
        let Ok(manifest) = serde_json::from_slice::<serde_json::Value>(&bytes) else {
            continue;
        };
        let Ok(options) = serde_json::from_value::<BuildOptions>(manifest["options"].clone())
        else {
            continue;
        };
        if options.release_id == 0
            || !manifest["uciVerified"].as_bool().unwrap_or(false)
            || !manifest["standaloneSearchVerified"]
                .as_bool()
                .unwrap_or(false)
            || !recipes::release_rule(&options.engine).is_some_and(|r| manifest["recipe"] == r.id)
        {
            continue;
        }
        let weights_sha256 = if options.engine == "lc0" {
            let Some(name @ ("weights.pb" | "weights.pb.gz")) = manifest["weightsFile"].as_str()
            else {
                continue;
            };
            let Some(digest) = manifest["weightsSha256"].as_str() else {
                continue;
            };
            if !folder.join(name).is_file() {
                continue;
            }
            digest.to_owned()
        } else {
            String::new()
        };
        let exe = folder.join(format!("{}.exe", options.engine));
        if exe.is_file() {
            let key = (
                options.engine.clone(),
                options.release_id,
                options.source_ref.clone(),
                options.backend,
            );
            remember(
                &mut history.verified,
                (key, weights_sha256),
                &job,
                Row {
                    engine: options.engine,
                    tag: options.source_ref,
                    state: "Verified".into(),
                    message: "UCI and standalone search passed.".into(),
                    output: exe.to_string_lossy().into_owned(),
                    log_path: job.join("build.log").to_string_lossy().into_owned(),
                },
            );
        }
    }
    history
}

fn requested_weights_digest(options: &BuildOptions) -> Result<String> {
    if options.engine != "lc0" {
        return Ok(String::new());
    }
    if options.weights_path.is_empty() {
        return Ok(recipes::LC0.network.sha256.clone());
    }
    let path = Path::new(&options.weights_path);
    lc0::validate_weights_path(path, true)?;
    recipes::sha256(path)
}

fn entries(query: &Query) -> Result<Vec<Entry>> {
    if query.scope > 2 || query.options.len() > 2 {
        return Err("Select new releases, the latest release, or release history.".into());
    }
    let mut entries = Vec::new();
    let mut engines = HashSet::new();
    let mut indexes = HashMap::new();
    for defaults in &query.options {
        let engine = &defaults.engine;
        let rule = recipes::release_rule(engine).ok_or("Select Stockfish or Lc0.")?;
        if !engines.insert(engine.as_str()) {
            return Err("Select each engine once.".into());
        }
        let Some(catalog) = read_catalog(&catalog_path(Path::new(&query.catalog_root), engine)?)?
        else {
            continue;
        };
        let root = Path::new(&defaults.output_root);
        let index = indexes.entry(root).or_insert_with(|| history(root));
        let weights_digest = requested_weights_digest(defaults);
        for (rank, release) in catalog.releases.iter().enumerate() {
            if (query.scope == 0 && rank >= 2 && catalog.baseline.contains(&release.id))
                || (query.scope == 1 && rank != 0)
            {
                continue;
            }
            let mut options = defaults.clone();
            options.release_id = release.id;
            options.source_ref = release.reference.clone();
            if engine == "stockfish" {
                options.auto_architecture = true;
                options.pgo = true;
                options.strip = true;
                options.backend.clear();
            }
            let mut row = Row {
                engine: engine.clone(),
                tag: release.reference.clone(),
                ..Row::default()
            };
            let key = (
                engine.clone(),
                release.id,
                row.tag.clone(),
                options.backend.clone(),
            );
            if let Ok(digest) = &weights_digest
                && let Some((_, verified)) = index.verified.get(&(key.clone(), digest.clone()))
            {
                row = verified.clone();
            } else if !rule.accepts(&row.tag) {
                row.state = "Needs recipe update".into();
                row.message =
                    "This release is outside the current recipe's supported source family.".into();
            } else if let Err(error) = &weights_digest {
                row.state = "Blocked".into();
                row.message = error.clone();
            } else {
                match plan(&options) {
                    Ok(resolved) => {
                        options = resolved.options;
                        row.state = "Ready".into();
                        row.message = if engine == "stockfish" {
                            format!("{} · PGO", options.architecture)
                        } else {
                            options.backend.clone()
                        };
                        if let Some((_, attempt)) = index.attempts.get(&key) {
                            row = attempt.clone();
                        }
                    }
                    Err(error) => {
                        row.state = "Blocked".into();
                        row.message = error;
                    }
                }
            }
            entries.push(Entry { rank, row, options });
        }
    }
    entries.sort_by_key(|entry| entry.rank);
    Ok(entries)
}

pub fn preview(query: &Query) -> Result<Vec<Row>> {
    Ok(entries(query)?.into_iter().map(|entry| entry.row).collect())
}

impl Compiler {
    pub fn start_library(&self, query: Query) -> Result<()> {
        let _gate = self.gate.lock().unwrap();
        if matches!(self.snapshot().state.as_str(), "Running" | "Cancelling") {
            return Err("A build is already running.".into());
        }
        if let Some(worker) = self.worker.lock().unwrap().take() {
            let _ = worker.join();
        }
        let initial = entries(&query)?;
        let mut roots: Vec<_> = initial
            .iter()
            .filter(|entry| entry.ready())
            .map(|entry| PathBuf::from(&entry.options.output_root))
            .collect();
        roots.sort();
        roots.dedup();
        let mut locks = Vec::new();
        let mut canonical = HashSet::new();
        for root in roots {
            fs::create_dir_all(&root).map_err(|e| e.to_string())?;
            let path = fs::canonicalize(&root).map_err(|e| e.to_string())?;
            if !canonical.insert(path) {
                continue;
            }
            let lock = fs::OpenOptions::new()
                .write(true)
                .create(true)
                .truncate(false)
                .open(root.join(".release-library.lock"))
                .map_err(|e| e.to_string())?;
            lock.try_lock()
                .map_err(|_| format!("Another release queue is using {}.", root.display()))?;
            locks.push(lock);
        }
        let entries = entries(&query)?;
        if !entries.iter().any(Entry::ready) {
            return Err(
                "There are no ready releases to build. Check releases or review blocked entries."
                    .into(),
            );
        }
        self.prepare()?;
        self.shared.lock().unwrap().snapshot.library =
            entries.iter().map(|entry| entry.row.clone()).collect();
        let shared = self.shared.clone();
        let cancel = self.cancel.clone();
        *self.worker.lock().unwrap() = Some(thread::spawn(move || {
            let _locks = locks;
            let mut built = 0;
            let mut failed = 0;
            for (index, entry) in entries.into_iter().enumerate() {
                if !entry.ready() {
                    continue;
                }
                if cancel.load(Ordering::SeqCst) {
                    break;
                }
                {
                    let mut state = shared.lock().unwrap();
                    state.lines.clear();
                    state.host_reported = false;
                    state.snapshot.output.clear();
                    state.snapshot.log_path.clear();
                    state.snapshot.phase = format!(
                        "Prepare {} {}",
                        engine_name(&entry.row.engine),
                        entry.row.tag
                    );
                    state.snapshot.message = state.snapshot.phase.clone();
                    state.snapshot.library[index].state = "Building".into();
                }
                let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    build(&entry.options, &shared, &cancel)
                }))
                .unwrap_or_else(|_| {
                    Err("An internal build error occurred. Review the saved log.".into())
                });
                let mut state = shared.lock().unwrap();
                let (status, message, output) = match result {
                    Ok(output) => {
                        built += 1;
                        (
                            "Verified",
                            "UCI and standalone search passed.".into(),
                            output,
                        )
                    }
                    Err(_) if cancel.load(Ordering::SeqCst) => (
                        "Cancelled",
                        "The job was cancelled. Its files and log were kept.".into(),
                        String::new(),
                    ),
                    Err(error) => {
                        failed += 1;
                        let status = if error.starts_with("Needs a recipe update:") {
                            "Needs recipe update"
                        } else {
                            "Failed"
                        };
                        (status, error, String::new())
                    }
                };
                state.line(&message);
                state.log = None;
                state.snapshot.output = output.clone();
                let log_path = state.snapshot.log_path.clone();
                let row = &mut state.snapshot.library[index];
                row.state = status.into();
                row.message = message;
                row.output = output;
                row.log_path = log_path;
                let attempt = (status != "Verified" && !row.log_path.is_empty()).then(|| Attempt {
                    recipe: recipes::release_rule(&row.engine).unwrap().id.clone(),
                    release_id: entry.options.release_id,
                    backend: entry.options.backend,
                    row: row.clone(),
                });
                drop(state);
                if let Some(attempt) = attempt
                    && let Ok(bytes) = serde_json::to_vec_pretty(&attempt)
                {
                    let path =
                        Path::new(&attempt.row.log_path).with_file_name("release-attempt.json");
                    if let Err(error) = fs::write(path, bytes) {
                        shared
                            .lock()
                            .unwrap()
                            .line(&format!("Could not save the queue outcome: {error}"));
                    }
                }
            }
            let mut state = shared.lock().unwrap();
            state.snapshot.state = if cancel.load(Ordering::SeqCst) {
                "Cancelled"
            } else if failed > 0 {
                "Failed"
            } else {
                "Completed"
            }
            .into();
            state.snapshot.phase = "Release queue".into();
            state.snapshot.message = format!("{built} verified, {failed} failed.");
        }));
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_releases_accumulate_and_verified_results_are_reused() {
        let (dir, mut stockfish) = crate::tests::options();
        let root = dir.path();
        let versions = |tags: &[(u64, &str)]| {
            tags.iter()
                .map(|(id, tag)| github::SourceVersion {
                    id: *id,
                    reference: (*tag).into(),
                    date: format!("2026-09-{id:02}T00:00:00Z"),
                    ..Default::default()
                })
                .collect()
        };
        let mut query = Query {
            catalog_root: root.to_string_lossy().into_owned(),
            options: vec![stockfish.clone()],
            scope: 0,
        };
        observe(
            root,
            "stockfish",
            versions(&[(2, "sf_18"), (1, "sf_17.1"), (3, "sf_19")]),
            Some("test-etag".into()),
        )
        .unwrap();
        assert_eq!(
            preview(&query)
                .unwrap()
                .iter()
                .map(|r| r.tag.as_str())
                .collect::<Vec<_>>(),
            ["sf_19", "sf_18"]
        );
        assert_eq!(
            cached_etag(root, "stockfish").unwrap().as_deref(),
            Some("test-etag")
        );
        observe(
            root,
            "stockfish",
            versions(&[(1, "sf_17.1"), (2, "sf_18"), (3, "sf_19"), (4, "sf_20")]),
            None,
        )
        .unwrap();
        let rows = preview(&query).unwrap();
        assert_eq!(
            rows.iter().map(|r| r.tag.as_str()).collect::<Vec<_>>(),
            ["sf_20", "sf_19", "sf_18"]
        );

        let result = Path::new(&stockfish.output_root).join("jobs/1/result");
        fs::create_dir_all(&result).unwrap();
        stockfish.release_id = 3;
        stockfish.source_ref = "sf_19".into();
        stockfish.backend.clear();
        let manifest = serde_json::json!({
            "options": stockfish, "uciVerified": true, "standaloneSearchVerified": true,
            "recipe": recipes::STOCKFISH.release.id,
        });
        fs::write(
            result.join("build.json"),
            serde_json::to_vec(&manifest).unwrap(),
        )
        .unwrap();
        assert_ne!(preview(&query).unwrap()[1].state, "Verified");
        fs::write(result.join("stockfish.exe"), b"fixture").unwrap();
        assert_eq!(preview(&query).unwrap()[1].state, "Verified");
        let failed = result.parent().unwrap().with_file_name("2");
        fs::create_dir_all(&failed).unwrap();
        let attempt = Attempt {
            recipe: recipes::STOCKFISH.release.id.clone(),
            release_id: 4,
            backend: String::new(),
            row: Row {
                engine: "stockfish".into(),
                tag: "sf_20".into(),
                state: "Needs recipe update".into(),
                message: "fixture source mismatch".into(),
                ..Default::default()
            },
        };
        fs::write(
            failed.join("release-attempt.json"),
            serde_json::to_vec(&attempt).unwrap(),
        )
        .unwrap();
        assert_eq!(
            preview(&query).unwrap()[0].message,
            "fixture source mismatch"
        );
        assert!(!entries(&query).unwrap()[0].ready());
        query.scope = 1;
        assert_eq!(preview(&query).unwrap().len(), 1);
        query.scope = 2;
        assert_eq!(preview(&query).unwrap().len(), 4);
    }

    #[test]
    fn lc0_reuses_only_results_with_matching_weights() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let python = root.join("python.exe");
        let vcvars = root.join("VC/Auxiliary/Build/vcvarsall.bat");
        fs::create_dir_all(vcvars.parent().unwrap()).unwrap();
        fs::write(&vcvars, []).unwrap();
        fs::write(&python, []).unwrap();
        let mut options = BuildOptions {
            engine: "lc0".into(),
            backend: "eigen".into(),
            output_root: root.to_string_lossy().into_owned(),
            python_path: python.to_string_lossy().into_owned(),
            visual_studio: root.to_string_lossy().into_owned(),
            release_id: 1,
            source_ref: "v0.32.1".into(),
            ..BuildOptions::default()
        };
        observe(
            root,
            "lc0",
            vec![github::SourceVersion {
                id: options.release_id,
                reference: options.source_ref.clone(),
                ..Default::default()
            }],
            None,
        )
        .unwrap();
        let custom = root.join("custom.pb.gz");
        fs::write(&custom, b"custom weights").unwrap();
        let custom_digest = recipes::sha256(&custom).unwrap();
        for (job, digest) in [
            ("1", recipes::LC0.network.sha256.as_str()),
            ("2", custom_digest.as_str()),
        ] {
            let result = root.join("jobs").join(job).join("result");
            fs::create_dir_all(&result).unwrap();
            fs::write(result.join("lc0.exe"), b"fixture").unwrap();
            fs::write(result.join("weights.pb.gz"), b"fixture").unwrap();
            let manifest = serde_json::json!({
                "options": options, "uciVerified": true, "standaloneSearchVerified": true,
                "recipe": recipes::LC0.release.id, "weightsFile": "weights.pb.gz",
                "weightsSha256": digest,
            });
            fs::write(result.join("build.json"), manifest.to_string()).unwrap();
            options.weights_path = custom.to_string_lossy().into_owned();
        }
        let mut query = Query {
            catalog_root: root.to_string_lossy().into_owned(),
            options: vec![options],
            scope: 0,
        };
        let row = || preview(&query).unwrap().remove(0);
        assert_eq!(Path::new(&row().output), root.join("jobs/2/result/lc0.exe"));
        fs::write(&custom, b"replacement weights at the same path").unwrap();
        assert_eq!(row().state, "Ready");
        fs::remove_file(&custom).unwrap();
        assert_eq!(row().state, "Blocked");
        query.options[0].weights_path.clear();
        let row = || preview(&query).unwrap().remove(0);
        assert_eq!(Path::new(&row().output), root.join("jobs/1/result/lc0.exe"));
        fs::remove_file(root.join("jobs/1/result/weights.pb.gz")).unwrap();
        assert_eq!(row().state, "Ready");
    }
}
