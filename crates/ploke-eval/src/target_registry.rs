use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::layout::{datasets_dir, registries_dir};
use crate::registry::builtin_dataset_registry_entry;
use crate::spec::PrepareError;

pub const TARGET_REGISTRY_SCHEMA_VERSION: &str = "target-registry.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum BenchmarkFamily {
    MultiSweBenchRust,
}

impl BenchmarkFamily {
    fn file_stem(self) -> &'static str {
        match self {
            Self::MultiSweBenchRust => "multi-swe-bench-rust",
        }
    }

    fn display_name(self) -> &'static str {
        match self {
            Self::MultiSweBenchRust => "multi-swe-bench-rust",
        }
    }
}

#[derive(Debug, Clone)]
pub struct RegistryRecomputeRequest {
    pub benchmark_family: BenchmarkFamily,
    pub dataset_keys: Vec<String>,
    pub dataset_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TargetRegistry {
    pub schema_version: String,
    pub benchmark_family: BenchmarkFamily,
    pub updated_at: String,
    pub dataset_sources: Vec<RegistryDatasetSource>,
    pub entries: Vec<RegistryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub struct RegistryDatasetSource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub path: PathBuf,
    pub label: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistryEntry {
    pub instance_id: String,
    pub dataset_label: String,
    pub repo_family: String,
    pub source: RegistrySource,
    pub state: RegistryEntryState,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RegistrySource {
    pub dataset_path: PathBuf,
    pub org: String,
    pub repo: String,
    pub number: u64,
    pub base_sha: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RegistryEntryState {
    Active,
    Ineligible { reason: String },
}

#[derive(Debug, Clone, Deserialize)]
struct DatasetRecord {
    instance_id: Option<String>,
    org: String,
    repo: String,
    number: u64,
    title: Option<String>,
    base: DatasetBase,
}

#[derive(Debug, Clone, Deserialize)]
struct DatasetBase {
    sha: String,
}

#[derive(Debug, Clone)]
struct RegistryUniverseEntry {
    instance_id: String,
    dataset_label: String,
    source: RegistrySource,
}

pub fn target_registry_path(benchmark_family: BenchmarkFamily) -> Result<PathBuf, PrepareError> {
    Ok(registries_dir()?.join(format!("{}.json", benchmark_family.file_stem())))
}

pub fn load_target_registry(
    benchmark_family: BenchmarkFamily,
) -> Result<TargetRegistry, PrepareError> {
    let path = target_registry_path(benchmark_family)?;
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest { path, source })
}

pub fn recompute_target_registry(
    request: RegistryRecomputeRequest,
) -> Result<(PathBuf, TargetRegistry), PrepareError> {
    let prior = load_target_registry(request.benchmark_family).ok();
    let dataset_sources =
        resolve_registry_dataset_sources(&request.dataset_keys, &request.dataset_files)?;
    let dataset_sources = if dataset_sources.is_empty() {
        prior.map(|registry| registry.dataset_sources).ok_or_else(|| {
            PrepareError::InvalidBatchSelection {
                detail:
                    "registry recompute requires at least one dataset file or dataset key on first run"
                        .to_string(),
            }
        })?
    } else {
        dataset_sources
    };

    let mut entries = load_registry_universe(&dataset_sources)?;
    entries.sort_by(|left, right| left.instance_id.cmp(&right.instance_id));

    let registry = TargetRegistry {
        schema_version: TARGET_REGISTRY_SCHEMA_VERSION.to_string(),
        benchmark_family: request.benchmark_family,
        updated_at: Utc::now().to_rfc3339(),
        dataset_sources,
        entries,
    };

    let path = target_registry_path(request.benchmark_family)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let serialized = serde_json::to_string_pretty(&registry).map_err(PrepareError::Serialize)?;
    fs::write(&path, serialized).map_err(|source| PrepareError::WriteManifest {
        path: path.clone(),
        source,
    })?;

    Ok((path, registry))
}

pub fn render_target_registry_status(registry: &TargetRegistry) -> String {
    let total_entries = registry.entries.len();
    let active_total = registry
        .entries
        .iter()
        .filter(|entry| matches!(entry.state, RegistryEntryState::Active))
        .count();
    let ineligible_total = total_entries.saturating_sub(active_total);

    let mut by_dataset = BTreeMap::<String, usize>::new();
    let mut by_repo = BTreeMap::<String, usize>::new();
    for entry in &registry.entries {
        *by_dataset.entry(entry.dataset_label.clone()).or_default() += 1;
        *by_repo.entry(entry.repo_family.clone()).or_default() += 1;
    }

    let mut out = String::new();
    out.push_str(&format!(
        "target registry: {} [{}]\n",
        registry.benchmark_family.display_name(),
        TARGET_REGISTRY_SCHEMA_VERSION
    ));
    out.push_str(&format!("updated: {}\n", registry.updated_at));
    out.push_str(&format!(
        "datasets: {} | repo families: {} | entries: {} | active: {} | ineligible: {}\n",
        registry.dataset_sources.len(),
        by_repo.len(),
        total_entries,
        active_total,
        ineligible_total
    ));
    out.push('\n');
    out.push_str("by dataset:\n");
    for (label, count) in by_dataset {
        out.push_str(&format!("  {:<24} {}\n", label, count));
    }

    out
}

pub fn resolve_registry_dataset_sources(
    dataset_keys: &[String],
    dataset_files: &[PathBuf],
) -> Result<Vec<RegistryDatasetSource>, PrepareError> {
    let mut sources = Vec::new();

    for key in dataset_keys {
        let entry = builtin_dataset_registry_entry(key)
            .ok_or_else(|| PrepareError::UnknownDatasetKey(key.clone()))?;
        let path = datasets_dir()?.join(entry.filename);
        if !path.exists() {
            return Err(PrepareError::MissingDatasetFile(path));
        }
        sources.push(RegistryDatasetSource {
            key: Some(key.clone()),
            path,
            label: key.clone(),
            url: Some(entry.url.to_string()),
        });
    }

    for path in dataset_files {
        let path = canonicalize_existing_file(path)?;
        sources.push(RegistryDatasetSource {
            key: None,
            label: dataset_label_for_path(&path),
            path,
            url: None,
        });
    }

    sources.sort_by(|left, right| {
        left.label
            .cmp(&right.label)
            .then_with(|| left.path.cmp(&right.path))
    });
    sources.dedup_by(|left, right| left.path == right.path);
    Ok(sources)
}

fn load_registry_universe(
    sources: &[RegistryDatasetSource],
) -> Result<Vec<RegistryEntry>, PrepareError> {
    let mut entries = Vec::new();
    let mut seen = BTreeSet::new();

    for source in sources {
        let file =
            File::open(&source.path).map_err(|source_err| PrepareError::OpenDatasetFile {
                path: source.path.clone(),
                source: source_err,
            })?;

        for (line_no, line) in BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|source_err| PrepareError::ReadDatasetLine {
                path: source.path.clone(),
                line: line_no + 1,
                source: source_err,
            })?;
            let trimmed = line.trim();
            if trimmed.is_empty() {
                continue;
            }

            let record: DatasetRecord = serde_json::from_str(trimmed).map_err(|source_err| {
                PrepareError::ParseDatasetLine {
                    path: source.path.clone(),
                    line: line_no + 1,
                    source: source_err,
                }
            })?;
            let entry = registry_universe_entry(source, record);

            if !seen.insert(entry.instance_id.clone()) {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "duplicate benchmark instance '{}' across registry dataset sources",
                        entry.instance_id
                    ),
                });
            }

            entries.push(RegistryEntry {
                instance_id: entry.instance_id,
                dataset_label: entry.dataset_label,
                repo_family: format!("{}__{}", entry.source.org, entry.source.repo),
                source: entry.source,
                state: RegistryEntryState::Active,
            });
        }
    }

    Ok(entries)
}

fn registry_universe_entry(
    source: &RegistryDatasetSource,
    record: DatasetRecord,
) -> RegistryUniverseEntry {
    let instance_id = record
        .instance_id
        .clone()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| format!("{}__{}-{}", record.org, record.repo, record.number));

    let _ = record.title;

    RegistryUniverseEntry {
        instance_id,
        dataset_label: source.label.clone(),
        source: RegistrySource {
            dataset_path: source.path.clone(),
            org: record.org,
            repo: record.repo,
            number: record.number,
            base_sha: record.base.sha,
        },
    }
}

fn dataset_label_for_path(path: &Path) -> String {
    path.file_stem()
        .map(|value| value.to_string_lossy().into_owned())
        .unwrap_or_else(|| "dataset".to_string())
        .trim_end_matches("_dataset")
        .to_string()
}

fn canonicalize_existing_file(path: &Path) -> Result<PathBuf, PrepareError> {
    if !path.exists() {
        return Err(PrepareError::MissingDatasetFile(path.to_path_buf()));
    }
    path.canonicalize()
        .map_err(|source| PrepareError::Canonicalize {
            path: path.to_path_buf(),
            source,
        })
}
