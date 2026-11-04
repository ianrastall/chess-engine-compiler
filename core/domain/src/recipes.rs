//! Bundled, typed recipe data. Upstream files are inspected for capabilities.
use super::*;
use std::{collections::BTreeMap, sync::LazyLock};

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct ReleaseRule {
    schema: u32,
    pub id: String,
    pub repository: String,
    tag_prefix: String,
    minimum_version: [u32; 3],
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Stockfish {
    #[serde(flatten)]
    pub release: ReleaseRule,
    pub source_directory: String,
    pub makefile: String,
    pub evaluation_header: String,
    pub compiler: String,
    pub targets: StockfishTargets,
}

#[derive(Deserialize)]
pub(super) struct StockfishTargets {
    pub network: String,
    pub regular: String,
    pub optimized: String,
    pub strip: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct Lc0 {
    #[serde(flatten)]
    pub release: ReleaseRule,
    option_files: Vec<String>,
    required_options: Vec<String>,
    optional_flags: BTreeMap<String, String>,
    pub network: Network,
}

#[derive(Deserialize)]
pub(super) struct Network {
    pub name: String,
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
}

pub(super) static STOCKFISH: LazyLock<Stockfish> =
    LazyLock::new(|| serde_json::from_str(include_str!("../recipes/stockfish.json")).unwrap());
pub(super) static LC0: LazyLock<Lc0> =
    LazyLock::new(|| serde_json::from_str(include_str!("../recipes/lc0.json")).unwrap());

pub(super) fn release_rule(engine: &str) -> Option<&'static ReleaseRule> {
    match engine {
        "stockfish" => Some(&STOCKFISH.release),
        "lc0" => Some(&LC0.release),
        _ => None,
    }
}

impl ReleaseRule {
    pub fn accepts(&self, tag: &str) -> bool {
        let Some(version) = tag.strip_prefix(&self.tag_prefix) else {
            return false;
        };
        let mut numbers = [0; 3];
        let mut count = 0;
        for (i, part) in version.split('.').enumerate() {
            let (Some(target), Ok(number)) = (numbers.get_mut(i), part.parse::<u32>()) else {
                return false;
            };
            if !part.bytes().all(|b| b.is_ascii_digit()) {
                return false;
            }
            *target = number;
            count += 1;
        }
        self.schema == 1
            && (self.tag_prefix != "v" || count == 3)
            && numbers >= self.minimum_version
    }
}

pub(super) fn needs_recipe(message: impl std::fmt::Display) -> String {
    format!("Needs a recipe update: {message}")
}

impl Stockfish {
    pub fn network_names(&self, source: &Path) -> Result<Vec<String>> {
        let header = fs::read_to_string(source.join(&self.evaluation_header))
            .map_err(|e| needs_recipe(format!("Stockfish evaluation header: {e}")))?;
        let names: Vec<_> = header
            .lines()
            .filter_map(|line| {
                let mut words = line.split_whitespace();
                if words.next()? != "#define" || !words.next()?.starts_with("EvalFileDefaultName") {
                    return None;
                }
                Some(words.next()?.trim_matches('"').to_owned())
            })
            .collect();
        if names.is_empty() || names.iter().any(|name| network_prefix(name).is_none()) {
            return Err(needs_recipe(
                "the source has no recognized default NNUE declarations.",
            ));
        }
        Ok(names)
    }
}

pub(super) fn network_prefix(name: &str) -> Option<&str> {
    let hash = name.strip_prefix("nn-")?.strip_suffix(".nnue")?;
    (hash.len() == 12 && hash.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hash)
}

impl Lc0 {
    pub fn flags(&self, source: &Path, backend: &str, managed: bool) -> Result<Vec<String>> {
        let options = self
            .option_files
            .iter()
            .find_map(|name| fs::read_to_string(source.join(name)).ok())
            .ok_or_else(|| needs_recipe("Lc0 Meson option file was not found."))?;
        let options: std::collections::HashSet<_> = options
            .lines()
            .filter_map(|line| {
                let argument = line
                    .trim_start()
                    .strip_prefix("option")?
                    .trim_start()
                    .strip_prefix('(')?
                    .trim_start();
                let quote = argument.chars().next()?;
                matches!(quote, '\'' | '"').then_some(argument.get(1..)?.split(quote).next()?)
            })
            .collect();
        if !source.join("meson.build").is_file()
            || self
                .required_options
                .iter()
                .any(|key| !options.contains(key.as_str()))
        {
            return Err(needs_recipe(
                "the Lc0 source lacks the expected Meson backend options.",
            ));
        }
        let mut flags = vec![
            "-Dblas=true".into(),
            format!("-Dplain_cuda={}", backend == "cuda-auto"),
            format!("-Ddefault_backend={backend}"),
        ];
        flags.extend(self.optional_flags.iter().filter_map(|(key, value)| {
            if !options.contains(key.as_str())
                || (key == "native_arch" && !managed)
                || (key == "native_cuda" && backend != "cuda-auto")
            {
                return None;
            }
            Some(format!("-D{key}={value}"))
        }));
        Ok(flags)
    }
}

pub(super) fn sha256(path: &Path) -> Result<String> {
    use sha2::{Digest, Sha256};
    let mut digest = Sha256::new();
    let mut file = File::open(path).map_err(|e| format!("Open {}: {e}", path.display()))?;
    if std::io::copy(&mut file, &mut digest).map_err(|e| e.to_string())? == 0 {
        return Err(format!("{} is empty.", path.display()));
    }
    Ok(format!("{:x}", digest.finalize()))
}
