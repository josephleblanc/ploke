use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use reqwest::blocking::Client;
use serde::Deserialize;

use crate::layout::datasets_dir;
use crate::registry::builtin_dataset_registry_entry;
use crate::spec::{
    EvalBudget, IssueInput, MultiSweBenchSource, PrepareError, PrepareSingleRunRequest,
    PreparedMsbBatch, PreparedSingleRun, RunSource,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareMsbSingleRunRequest {
    pub dataset_file: Option<PathBuf>,
    pub dataset_key: Option<String>,
    pub instance_id: String,
    pub repo_cache: PathBuf,
    pub instances_root: PathBuf,
    pub budget: EvalBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareMsbBatchRequest {
    pub dataset_file: Option<PathBuf>,
    pub dataset_key: Option<String>,
    pub batch_id: String,
    pub select_all: bool,
    pub instance_ids: Vec<String>,
    pub specifics: Vec<String>,
    pub limit: Option<usize>,
    pub repo_cache: PathBuf,
    pub instances_root: PathBuf,
    pub batches_root: PathBuf,
    pub budget: EvalBudget,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedMsbBatchBundle {
    pub batch: PreparedMsbBatch,
    pub runs: Vec<PreparedSingleRun>,
}

#[derive(Debug, Clone, Deserialize)]
struct MultiSweBenchRecord {
    instance_id: Option<String>,
    org: String,
    repo: String,
    number: u64,
    title: Option<String>,
    body: Option<String>,
    base: MultiSweBenchBase,
    #[serde(default)]
    fix_patch: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
struct MultiSweBenchBase {
    sha: String,
}

#[derive(Debug)]
struct ResolvedDataset {
    file: PathBuf,
    url: Option<String>,
    language: Option<String>,
}

impl PrepareMsbSingleRunRequest {
    pub fn prepare(self) -> Result<PreparedSingleRun, PrepareError> {
        let dataset = resolve_dataset(self.dataset_file, self.dataset_key)?;
        let dataset_file =
            canonicalize_file(dataset.file.clone(), PrepareError::MissingDatasetFile)?;
        let record = load_instance(&dataset_file, &self.instance_id)?;
        prepare_record(
            &dataset,
            &dataset_file,
            &record,
            self.repo_cache,
            self.instances_root,
            self.budget,
        )
    }
}

impl PrepareMsbBatchRequest {
    pub fn prepare(self) -> Result<PreparedMsbBatchBundle, PrepareError> {
        let dataset = resolve_dataset(self.dataset_file, self.dataset_key)?;
        let dataset_file =
            canonicalize_file(dataset.file.clone(), PrepareError::MissingDatasetFile)?;
        let records = load_instances(&dataset_file)?;
        let selected_records = select_records(
            &records,
            self.select_all,
            &self.instance_ids,
            &self.specifics,
            self.limit,
        )?;
        let runs = selected_records
            .iter()
            .map(|record| {
                prepare_record(
                    &dataset,
                    &dataset_file,
                    record,
                    self.repo_cache.clone(),
                    self.instances_root.clone(),
                    self.budget.clone(),
                )
            })
            .collect::<Result<Vec<_>, _>>()?;
        let batch_output_dir = self.batches_root.join(&self.batch_id);
        let batch = PreparedMsbBatch {
            batch_id: self.batch_id,
            dataset_file,
            dataset_url: dataset.url,
            repo_cache: self.repo_cache,
            instances_root: self.instances_root,
            output_dir: batch_output_dir,
            budget: self.budget,
            instances: runs.iter().map(|run| run.task_id.clone()).collect(),
            campaign: None,
        };
        Ok(PreparedMsbBatchBundle { batch, runs })
    }
}

fn prepare_record(
    dataset: &ResolvedDataset,
    dataset_file: &PathBuf,
    record: &MultiSweBenchRecord,
    repo_cache: PathBuf,
    instances_root: PathBuf,
    budget: EvalBudget,
) -> Result<PreparedSingleRun, PrepareError> {
    let task_id = record_task_id(record);
    let repo_root = repo_cache.join(&record.org).join(&record.repo);
    let output_dir = instances_root.join(&task_id);
    let language = dataset.language.clone().or_else(|| {
        dataset_file
            .parent()
            .and_then(|parent| parent.file_name())
            .map(|name| name.to_string_lossy().into_owned())
    });

    let mut prepared = PrepareSingleRunRequest {
        task_id: task_id.clone(),
        repo_root,
        issue: IssueInput {
            title: record.title.clone(),
            body: record.body.clone(),
            body_path: None,
        },
        output_dir,
        base_sha: Some(record.base.sha.clone()),
        budget,
    }
    .prepare()?;

    prepared.source = Some(RunSource::MultiSweBench(MultiSweBenchSource {
        dataset_file: dataset_file.clone(),
        dataset_url: dataset.url.clone(),
        instance_id: task_id,
        org: record.org.clone(),
        repo: record.repo.clone(),
        number: record.number,
        language,
        expected_patch_files: record
            .fix_patch
            .as_deref()
            .map(extract_patch_files)
            .unwrap_or_default(),
    }));

    Ok(prepared)
}

fn record_task_id(record: &MultiSweBenchRecord) -> String {
    record
        .instance_id
        .clone()
        .filter(|id| !id.trim().is_empty())
        .unwrap_or_else(|| format!("{}__{}-{}", record.org, record.repo, record.number))
}

fn extract_patch_files(patch: &str) -> Vec<PathBuf> {
    let mut files = Vec::new();
    for line in patch.lines() {
        let Some(path) = line.strip_prefix("+++ ") else {
            continue;
        };
        if path == "/dev/null" {
            continue;
        }
        let path = path.strip_prefix("b/").unwrap_or(path);
        let path = path.trim();
        if path.is_empty() {
            continue;
        }
        let candidate = PathBuf::from(path);
        if !files.contains(&candidate) {
            files.push(candidate);
        }
    }
    files
}

fn load_instance(
    dataset_file: &PathBuf,
    wanted_instance_id: &str,
) -> Result<MultiSweBenchRecord, PrepareError> {
    load_instances(dataset_file)?
        .into_iter()
        .find(|record| record_task_id(record) == wanted_instance_id)
        .ok_or_else(|| PrepareError::MissingDatasetInstance {
            path: dataset_file.clone(),
            instance_id: wanted_instance_id.to_string(),
        })
}

fn load_instances(dataset_file: &PathBuf) -> Result<Vec<MultiSweBenchRecord>, PrepareError> {
    let file = File::open(dataset_file).map_err(|source| PrepareError::OpenDatasetFile {
        path: dataset_file.clone(),
        source,
    })?;
    let reader = BufReader::new(file);
    let mut records = Vec::new();

    for (line_no, line) in reader.lines().enumerate() {
        let line = line.map_err(|source| PrepareError::ReadDatasetLine {
            path: dataset_file.clone(),
            line: line_no + 1,
            source,
        })?;
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        let record: MultiSweBenchRecord =
            serde_json::from_str(trimmed).map_err(|source| PrepareError::ParseDatasetLine {
                path: dataset_file.clone(),
                line: line_no + 1,
                source,
            })?;
        records.push(record);
    }
    Ok(records)
}

fn select_records(
    records: &[MultiSweBenchRecord],
    select_all: bool,
    instance_ids: &[String],
    specifics: &[String],
    limit: Option<usize>,
) -> Result<Vec<MultiSweBenchRecord>, PrepareError> {
    if !select_all && instance_ids.is_empty() && specifics.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "pass --all, at least one --instance, or at least one --specific".to_string(),
        });
    }

    let selected = records
        .iter()
        .filter(|record| {
            select_all
                || instance_matches(record, instance_ids)
                || specific_matches(record, specifics)
        })
        .take(limit.unwrap_or(usize::MAX))
        .cloned()
        .collect::<Vec<_>>();

    if selected.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "the selection matched zero dataset rows".to_string(),
        });
    }

    Ok(selected)
}

fn instance_matches(record: &MultiSweBenchRecord, instance_ids: &[String]) -> bool {
    if instance_ids.is_empty() {
        return false;
    }
    let task_id = record_task_id(record);
    instance_ids.iter().any(|wanted| wanted == &task_id)
}

fn specific_matches(record: &MultiSweBenchRecord, specifics: &[String]) -> bool {
    if specifics.is_empty() {
        return false;
    }

    let task_id = record_task_id(record);
    let pr_id = format!("{}/{}:pr-{}", record.org, record.repo, record.number);
    specifics.iter().any(|specific| {
        task_id.contains(specific)
            || pr_id.contains(specific)
            || record.org.contains(specific)
            || record.repo.contains(specific)
            || record
                .title
                .as_deref()
                .is_some_and(|title| title.contains(specific))
    })
}

fn canonicalize_file<F>(path: PathBuf, missing: F) -> Result<PathBuf, PrepareError>
where
    F: FnOnce(PathBuf) -> PrepareError,
{
    if !path.exists() {
        return Err(missing(path));
    }
    path.canonicalize()
        .map_err(|source| PrepareError::Canonicalize { path, source })
}

fn resolve_dataset(
    dataset_file: Option<PathBuf>,
    dataset_key: Option<String>,
) -> Result<ResolvedDataset, PrepareError> {
    match (dataset_file, dataset_key) {
        (Some(file), None) => Ok(ResolvedDataset {
            file,
            url: None,
            language: None,
        }),
        (None, Some(key)) => {
            let entry = builtin_dataset_registry_entry(&key)
                .ok_or_else(|| PrepareError::UnknownDatasetKey(key.clone()))?;
            let cache_dir = datasets_dir()?;
            fs::create_dir_all(&cache_dir).map_err(|source| PrepareError::CreateOutputDir {
                path: cache_dir.clone(),
                source,
            })?;
            let cached_file = cache_dir.join(entry.filename);
            if !cached_file.exists() {
                download_dataset(entry.url, &cached_file)?;
            }
            Ok(ResolvedDataset {
                file: cached_file,
                url: Some(entry.url.to_string()),
                language: Some(entry.language.to_string()),
            })
        }
        _ => Err(PrepareError::InvalidDatasetLocator),
    }
}

fn download_dataset(url: &str, out_file: &PathBuf) -> Result<(), PrepareError> {
    let client = Client::new();
    let response = client
        .get(url)
        .send()
        .map_err(|source| PrepareError::DownloadDataset {
            url: url.to_string(),
            source,
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(PrepareError::DownloadDatasetStatus {
            url: url.to_string(),
            status: status.as_u16(),
        });
    }
    let bytes = response
        .bytes()
        .map_err(|source| PrepareError::DownloadDataset {
            url: url.to_string(),
            source,
        })?;
    fs::write(out_file, bytes).map_err(|source| PrepareError::WriteManifest {
        path: out_file.clone(),
        source,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn prepare_msb_single_maps_dataset_row_into_run_manifest() {
        let tmp = tempdir().expect("tempdir");
        let dataset_dir = tmp.path().join("rust");
        let repo_cache = tmp.path().join("repos");
        let repo_root = repo_cache.join("clap-rs").join("clap");
        let instances_root = tmp.path().join("instances");
        let dataset_file = dataset_dir.join("mini.jsonl");

        fs::create_dir_all(repo_root.join(".git")).expect("repo");
        fs::create_dir_all(&dataset_dir).expect("dataset dir");
        fs::write(repo_root.join(".git/HEAD"), "cafebabe\n").expect("head");
        fs::write(
            &dataset_file,
            r#"{"instance_id":"clap-rs__clap-1234","org":"clap-rs","repo":"clap","number":1234,"title":"Fix clap regression","body":"repro body","base":{"sha":"abc123"},"fix_patch":"diff --git a/src/lib.rs b/src/lib.rs\n--- a/src/lib.rs\n+++ b/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n"}"#,
        )
        .expect("dataset");

        let prepared = PrepareMsbSingleRunRequest {
            dataset_file: Some(dataset_file),
            dataset_key: None,
            instance_id: "clap-rs__clap-1234".to_string(),
            repo_cache,
            instances_root,
            budget: EvalBudget::default(),
        }
        .prepare()
        .expect("prepare msb run");

        assert_eq!(prepared.task_id, "clap-rs__clap-1234");
        assert_eq!(prepared.issue.title.as_deref(), Some("Fix clap regression"));
        assert_eq!(prepared.issue.body.as_deref(), Some("repro body"));
        assert_eq!(prepared.base_sha.as_deref(), Some("abc123"));
        assert_eq!(prepared.head_sha.as_deref(), Some("cafebabe"));
        match prepared.source {
            Some(RunSource::MultiSweBench(source)) => {
                assert_eq!(source.org, "clap-rs");
                assert_eq!(source.repo, "clap");
                assert_eq!(source.number, 1234);
                assert!(source.dataset_url.is_none());
                assert_eq!(source.language.as_deref(), Some("rust"));
                assert_eq!(
                    source.expected_patch_files,
                    vec![PathBuf::from("src/lib.rs")]
                );
            }
            other => panic!("unexpected source: {other:?}"),
        }
    }

    #[test]
    fn extract_patch_files_dedupes_and_skips_dev_null() {
        let patch = "\
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1 @@
-old
+new
diff --git a/src/new.rs b/src/new.rs
--- /dev/null
+++ b/src/new.rs
@@ -0,0 +1 @@
+created
diff --git a/src/lib.rs b/src/lib.rs
--- a/src/lib.rs
+++ b/src/lib.rs
@@ -1 +1 @@
-old
+newer
";
        assert_eq!(
            extract_patch_files(patch),
            vec![PathBuf::from("src/lib.rs"), PathBuf::from("src/new.rs")]
        );
    }

    #[test]
    fn prepare_msb_batch_builds_batch_manifest_and_selected_runs() {
        let tmp = tempdir().expect("tempdir");
        let dataset_dir = tmp.path().join("rust");
        let repo_cache = tmp.path().join("repos");
        let repo_root = repo_cache.join("BurntSushi").join("ripgrep");
        let instances_root = tmp.path().join("instances");
        let batches_root = tmp.path().join("batches");
        let dataset_file = dataset_dir.join("mini.jsonl");

        fs::create_dir_all(repo_root.join(".git")).expect("repo");
        fs::create_dir_all(&dataset_dir).expect("dataset dir");
        fs::write(repo_root.join(".git/HEAD"), "cafebabe\n").expect("head");
        fs::write(
            &dataset_file,
            concat!(
                r#"{"instance_id":"BurntSushi__ripgrep-2209","org":"BurntSushi","repo":"ripgrep","number":2209,"title":"Fix multiline replacement","body":"body one","base":{"sha":"abc123"},"fix_patch":"diff --git a/crates/printer/src/util.rs b/crates/printer/src/util.rs\n--- a/crates/printer/src/util.rs\n+++ b/crates/printer/src/util.rs\n@@ -1 +1 @@\n-old\n+new\n"}"#,
                "\n",
                r#"{"instance_id":"BurntSushi__ripgrep-2210","org":"BurntSushi","repo":"ripgrep","number":2210,"title":"Another fix","body":"body two","base":{"sha":"def456"},"fix_patch":"diff --git a/crates/core/src/lib.rs b/crates/core/src/lib.rs\n--- a/crates/core/src/lib.rs\n+++ b/crates/core/src/lib.rs\n@@ -1 +1 @@\n-old\n+new\n"}"#
            ),
        )
        .expect("dataset");

        let prepared = PrepareMsbBatchRequest {
            dataset_file: Some(dataset_file),
            dataset_key: None,
            batch_id: "ripgrep-2209".to_string(),
            select_all: false,
            instance_ids: Vec::new(),
            specifics: vec!["2209".to_string()],
            limit: None,
            repo_cache,
            instances_root: instances_root.clone(),
            batches_root: batches_root.clone(),
            budget: EvalBudget::default(),
        }
        .prepare()
        .expect("prepare msb batch");

        assert_eq!(prepared.batch.batch_id, "ripgrep-2209");
        assert_eq!(prepared.batch.output_dir, batches_root.join("ripgrep-2209"));
        assert_eq!(
            prepared.batch.instances,
            vec!["BurntSushi__ripgrep-2209".to_string()]
        );
        assert_eq!(prepared.runs.len(), 1);
        assert_eq!(prepared.runs[0].task_id, "BurntSushi__ripgrep-2209");
        assert_eq!(
            prepared.runs[0].manifest_path(),
            instances_root
                .join("BurntSushi__ripgrep-2209")
                .join("run.json")
        );
    }

    #[test]
    fn select_records_matches_official_pr_id_specific() {
        let records = vec![
            MultiSweBenchRecord {
                instance_id: Some("BurntSushi__ripgrep-2209".to_string()),
                org: "BurntSushi".to_string(),
                repo: "ripgrep".to_string(),
                number: 2209,
                title: Some("Fix multiline replacement".to_string()),
                body: None,
                base: MultiSweBenchBase {
                    sha: "abc123".to_string(),
                },
                fix_patch: None,
            },
            MultiSweBenchRecord {
                instance_id: Some("BurntSushi__ripgrep-2210".to_string()),
                org: "BurntSushi".to_string(),
                repo: "ripgrep".to_string(),
                number: 2210,
                title: Some("Fix something else".to_string()),
                body: None,
                base: MultiSweBenchBase {
                    sha: "def456".to_string(),
                },
                fix_patch: None,
            },
        ];

        let selected = select_records(
            &records,
            false,
            &[],
            &["BurntSushi/ripgrep:pr-2209".to_string()],
            None,
        )
        .expect("select by specific");

        assert_eq!(selected.len(), 1);
        assert_eq!(record_task_id(&selected[0]), "BurntSushi__ripgrep-2209");
    }
}
