use std::fs::{self, File};
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use reqwest::blocking::Client;
use serde::Deserialize;

use crate::layout::datasets_dir;
use crate::registry::builtin_dataset_registry_entry;
use crate::spec::{
    EvalBudget, IssueInput, MultiSweBenchSource, PrepareError, PrepareSingleRunRequest,
    PreparedSingleRun, RunSource,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PrepareMsbSingleRunRequest {
    pub dataset_file: Option<PathBuf>,
    pub dataset_key: Option<String>,
    pub instance_id: String,
    pub repo_cache: PathBuf,
    pub runs_root: PathBuf,
    pub budget: EvalBudget,
}

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
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
        let dataset_file = canonicalize_file(dataset.file, PrepareError::MissingDatasetFile)?;
        let record = load_instance(&dataset_file, &self.instance_id)?;

        let task_id = record
            .instance_id
            .clone()
            .filter(|id| !id.trim().is_empty())
            .unwrap_or_else(|| format!("{}__{}-{}", record.org, record.repo, record.number));
        let repo_root = self.repo_cache.join(&record.org).join(&record.repo);
        let output_dir = self.runs_root.join(&task_id);
        let language = dataset.language.or_else(|| {
            dataset_file
                .parent()
                .and_then(|parent| parent.file_name())
                .map(|name| name.to_string_lossy().into_owned())
        });

        let mut prepared = PrepareSingleRunRequest {
            task_id: task_id.clone(),
            repo_root,
            issue: IssueInput {
                title: record.title,
                body: record.body,
                body_path: None,
            },
            output_dir,
            base_sha: Some(record.base.sha),
            budget: self.budget,
        }
        .prepare()?;

        prepared.source = Some(RunSource::MultiSweBench(MultiSweBenchSource {
            dataset_file,
            dataset_url: dataset.url,
            instance_id: task_id,
            org: record.org,
            repo: record.repo,
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
    let file = File::open(dataset_file).map_err(|source| PrepareError::OpenDatasetFile {
        path: dataset_file.clone(),
        source,
    })?;
    let reader = BufReader::new(file);

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

        if record.instance_id.as_deref() == Some(wanted_instance_id) {
            return Ok(record);
        }
    }

    Err(PrepareError::MissingDatasetInstance {
        path: dataset_file.clone(),
        instance_id: wanted_instance_id.to_string(),
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
        let runs_root = tmp.path().join("runs");
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
            runs_root,
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
}
