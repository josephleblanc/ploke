use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum OutputMode {
    Json,
    Pretty,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvalBudget {
    pub max_turns: u32,
    pub max_tool_calls: u32,
    pub wall_clock_secs: u32,
}

impl Default for EvalBudget {
    fn default() -> Self {
        Self {
            max_turns: 40,
            max_tool_calls: 200,
            wall_clock_secs: 1800,
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameworkConfig {
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub tools: BTreeMap<String, FrameworkToolConfig>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FrameworkToolConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedCampaignContext {
    pub campaign_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    #[serde(default, skip_serializing_if = "FrameworkConfig::is_default")]
    pub framework: FrameworkConfig,
}

impl FrameworkConfig {
    pub fn is_default(&self) -> bool {
        self.tools.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct IssueInput {
    pub title: Option<String>,
    pub body: Option<String>,
    pub body_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrepareSingleRunRequest {
    pub task_id: String,
    pub repo_root: PathBuf,
    pub issue: IssueInput,
    pub output_dir: PathBuf,
    pub base_sha: Option<String>,
    pub budget: EvalBudget,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum RunSource {
    MultiSweBench(MultiSweBenchSource),
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MultiSweBenchSource {
    pub dataset_file: PathBuf,
    pub dataset_url: Option<String>,
    pub instance_id: String,
    pub org: String,
    pub repo: String,
    pub number: u64,
    pub language: Option<String>,
    #[serde(default)]
    pub expected_patch_files: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedSingleRun {
    pub task_id: String,
    pub repo_root: PathBuf,
    pub output_dir: PathBuf,
    pub issue: IssueInput,
    pub base_sha: Option<String>,
    pub head_sha: Option<String>,
    pub budget: EvalBudget,
    pub source: Option<RunSource>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign: Option<PreparedCampaignContext>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedMsbBatch {
    pub batch_id: String,
    pub dataset_file: PathBuf,
    pub dataset_url: Option<String>,
    pub repo_cache: PathBuf,
    pub runs_root: PathBuf,
    pub output_dir: PathBuf,
    pub budget: EvalBudget,
    pub instances: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub campaign: Option<PreparedCampaignContext>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PrepareWrite {
    Stdout,
    File(PathBuf),
}

#[derive(Debug, Error)]
pub enum PrepareError {
    #[error("task id cannot be empty")]
    EmptyTaskId,
    #[error("repo root '{0}' does not exist")]
    MissingRepoRoot(PathBuf),
    #[error("repo root '{0}' is not a directory")]
    RepoRootNotDirectory(PathBuf),
    #[error("output dir '{0}' is not a directory")]
    OutputDirNotDirectory(PathBuf),
    #[error("issue body file '{0}' does not exist")]
    MissingIssueBody(PathBuf),
    #[error("dataset file '{0}' does not exist")]
    MissingDatasetFile(PathBuf),
    #[error("could not determine a home directory; set PLOKE_EVAL_HOME explicitly")]
    MissingHomeDirectory,
    #[error("specify exactly one of dataset file or dataset key")]
    InvalidDatasetLocator,
    #[error("dataset registry key '{0}' was not found")]
    UnknownDatasetKey(String),
    #[error("failed to read issue body file '{path}': {source}")]
    ReadIssueBody {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to download dataset '{url}': {source}")]
    DownloadDataset { url: String, source: reqwest::Error },
    #[error("dataset download returned unsuccessful status {status} for '{url}'")]
    DownloadDatasetStatus { url: String, status: u16 },
    #[error("run manifest '{0}' does not exist")]
    MissingRunManifest(PathBuf),
    #[error("batch manifest '{0}' does not exist")]
    MissingBatchManifest(PathBuf),
    #[error("failed to run git command '{command}': {source}")]
    GitCommand {
        command: String,
        source: std::io::Error,
    },
    #[error("git command '{command}' failed with status {status}")]
    GitCommandStatus { command: String, status: i32 },
    #[error("failed to open dataset file '{path}': {source}")]
    OpenDatasetFile {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read run manifest '{path}': {source}")]
    ReadManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read batch manifest '{path}': {source}")]
    ReadBatchManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse run manifest '{path}': {source}")]
    ParseManifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to parse batch manifest '{path}': {source}")]
    ParseBatchManifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to read model registry file '{path}': {source}")]
    ReadModelRegistry {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse model registry file '{path}': {source}")]
    ParseModelRegistry {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to write model registry file '{path}': {source}")]
    WriteModelRegistry {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize model registry: {0}")]
    SerializeModelRegistry(serde_json::Error),
    #[error("failed to read active model file '{path}': {source}")]
    ReadActiveModel {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse active model file '{path}': {source}")]
    ParseActiveModel {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to write active model file '{path}': {source}")]
    WriteActiveModel {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize active model selection: {0}")]
    SerializeActiveModel(serde_json::Error),
    #[error("failed to read provider preferences file '{path}': {source}")]
    ReadProviderPrefs {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse provider preferences file '{path}': {source}")]
    ParseProviderPrefs {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to write provider preferences file '{path}': {source}")]
    WriteProviderPrefs {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize provider preferences: {0}")]
    SerializeProviderPrefs(serde_json::Error),
    #[error("campaign manifest file '{0}' does not exist")]
    MissingCampaignManifest(PathBuf),
    #[error("failed to read campaign manifest file '{path}': {source}")]
    ReadCampaignManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse campaign manifest file '{path}': {source}")]
    ParseCampaignManifest {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to write campaign manifest file '{path}': {source}")]
    WriteCampaignManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize campaign manifest: {0}")]
    SerializeCampaignManifest(serde_json::Error),
    #[error("failed to read starting db cache metadata file '{path}': {source}")]
    ReadStartingDbCacheMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse starting db cache metadata file '{path}': {source}")]
    ParseStartingDbCacheMetadata {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to write starting db cache metadata file '{path}': {source}")]
    WriteStartingDbCacheMetadata {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize starting db cache metadata: {0}")]
    SerializeStartingDbCacheMetadata(serde_json::Error),
    #[error("failed to write starting db cache snapshot file '{path}': {source}")]
    WriteStartingDbCacheSnapshot {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read last run record '{path}': {source}")]
    ReadLastRunRecord {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse last run record '{path}': {source}")]
    ParseLastRunRecord {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to write last run record '{path}': {source}")]
    WriteLastRunRecord {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to read protocol artifact '{path}': {source}")]
    ReadProtocolArtifact {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to parse protocol artifact '{path}': {source}")]
    ParseProtocolArtifact {
        path: PathBuf,
        source: serde_json::Error,
    },
    #[error("failed to create protocol artifact dir '{path}': {source}")]
    CreateProtocolArtifactDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write protocol artifact '{path}': {source}")]
    WriteProtocolArtifact {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize protocol artifact: {0}")]
    SerializeProtocolArtifact(serde_json::Error),
    #[error("model '{model}' was not found in registry '{path}'")]
    UnknownModelInRegistry { model: String, path: PathBuf },
    #[error("model registry file '{0}' does not exist")]
    MissingModelRegistry(PathBuf),
    #[error("active model file '{0}' does not exist")]
    MissingActiveModel(PathBuf),
    #[error("no completed eval run was found in '{0}'")]
    MissingLastRun(PathBuf),
    #[error("completed run '{0}' does not contain a final snapshot")]
    MissingFinalSnapshot(PathBuf),
    #[error("failed to read dataset line {line} from '{path}': {source}")]
    ReadDatasetLine {
        path: PathBuf,
        line: usize,
        source: std::io::Error,
    },
    #[error("failed to parse dataset line {line} from '{path}': {source}")]
    ParseDatasetLine {
        path: PathBuf,
        line: usize,
        source: serde_json::Error,
    },
    #[error("instance '{instance_id}' was not found in dataset '{path}'")]
    MissingDatasetInstance { path: PathBuf, instance_id: String },
    #[error("batch selection is invalid: {detail}")]
    InvalidBatchSelection { detail: String },
    #[error("issue input must include at least a title or a body")]
    EmptyIssue,
    #[error("failed to canonicalize '{path}': {source}")]
    Canonicalize {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to create output dir '{path}': {source}")]
    CreateOutputDir {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to write manifest '{path}': {source}")]
    WriteManifest {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("failed to serialize manifest: {0}")]
    Serialize(serde_json::Error),
    #[error("database setup failed during '{phase}': {detail}")]
    DatabaseSetup { phase: &'static str, detail: String },
    #[error("timed out waiting for '{phase}' after {secs} seconds")]
    Timeout { phase: &'static str, secs: u64 },
    #[error("event stream closed while waiting for '{phase}'")]
    EventStreamClosed { phase: &'static str },
    #[error("indexing failed: {detail}")]
    IndexingFailed { detail: String },
    #[error("snapshot failed: {detail}")]
    SnapshotFailed { detail: String },
    #[error("Error with database")]
    PlokeError(#[from] ploke_db::DbError),
}

impl PrepareSingleRunRequest {
    pub fn prepare(self) -> Result<PreparedSingleRun, PrepareError> {
        let task_id = self.task_id.trim().to_string();
        if task_id.is_empty() {
            return Err(PrepareError::EmptyTaskId);
        }

        let repo_root = canonicalize_dir(self.repo_root, PrepareError::MissingRepoRoot)?;
        let output_dir = ensure_dir(self.output_dir)?;

        let mut issue = self.issue;
        if issue.body.is_none() {
            if let Some(path) = issue.body_path.as_ref() {
                let body =
                    fs::read_to_string(path).map_err(|source| PrepareError::ReadIssueBody {
                        path: path.clone(),
                        source,
                    })?;
                issue.body = Some(body);
            }
        }

        if issue.title.as_deref().is_none_or(str::is_empty)
            && issue.body.as_deref().is_none_or(str::is_empty)
        {
            return Err(PrepareError::EmptyIssue);
        }

        let issue_body_path = match issue.body_path.take() {
            Some(path) => Some(canonicalize_existing_file(path)?),
            None => None,
        };
        issue.body_path = issue_body_path;

        let head_sha = resolve_head_sha(&repo_root);

        Ok(PreparedSingleRun {
            task_id,
            repo_root,
            output_dir,
            issue,
            base_sha: self.base_sha,
            head_sha,
            budget: self.budget,
            source: None,
            campaign: None,
        })
    }
}

impl PreparedSingleRun {
    pub fn manifest_path(&self) -> PathBuf {
        self.output_dir.join("run.json")
    }

    pub fn render_json(&self, mode: OutputMode) -> Result<String, PrepareError> {
        match mode {
            OutputMode::Json => serde_json::to_string(self).map_err(PrepareError::Serialize),
            OutputMode::Pretty => {
                serde_json::to_string_pretty(self).map_err(PrepareError::Serialize)
            }
        }
    }

    pub fn write_manifest(
        &self,
        mode: OutputMode,
        write: PrepareWrite,
    ) -> Result<(), PrepareError> {
        let serialized = self.render_json(mode)?;
        match write {
            PrepareWrite::Stdout => {
                println!("{serialized}");
                Ok(())
            }
            PrepareWrite::File(path) => fs::write(&path, serialized)
                .map_err(|source| PrepareError::WriteManifest { path, source }),
        }
    }
}

impl PreparedMsbBatch {
    pub fn manifest_path(&self) -> PathBuf {
        self.output_dir.join("batch.json")
    }

    pub fn render_json(&self, mode: OutputMode) -> Result<String, PrepareError> {
        match mode {
            OutputMode::Json => serde_json::to_string(self).map_err(PrepareError::Serialize),
            OutputMode::Pretty => {
                serde_json::to_string_pretty(self).map_err(PrepareError::Serialize)
            }
        }
    }

    pub fn write_manifest(&self, mode: OutputMode) -> Result<(), PrepareError> {
        let serialized = self.render_json(mode)?;
        let path = self.manifest_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        fs::write(&path, serialized).map_err(|source| PrepareError::WriteManifest { path, source })
    }
}

fn canonicalize_dir<F>(path: PathBuf, missing: F) -> Result<PathBuf, PrepareError>
where
    F: FnOnce(PathBuf) -> PrepareError,
{
    if !path.exists() {
        return Err(missing(path));
    }
    if !path.is_dir() {
        return Err(PrepareError::RepoRootNotDirectory(path));
    }
    path.canonicalize()
        .map_err(|source| PrepareError::Canonicalize { path, source })
}

fn ensure_dir(path: PathBuf) -> Result<PathBuf, PrepareError> {
    if path.exists() {
        if !path.is_dir() {
            return Err(PrepareError::OutputDirNotDirectory(path));
        }
    } else {
        fs::create_dir_all(&path).map_err(|source| PrepareError::CreateOutputDir {
            path: path.clone(),
            source,
        })?;
    }
    path.canonicalize()
        .map_err(|source| PrepareError::Canonicalize { path, source })
}

fn canonicalize_existing_file(path: PathBuf) -> Result<PathBuf, PrepareError> {
    if !path.exists() {
        return Err(PrepareError::MissingIssueBody(path));
    }
    path.canonicalize()
        .map_err(|source| PrepareError::Canonicalize { path, source })
}

fn resolve_head_sha(repo_root: &Path) -> Option<String> {
    let git_dir = repo_root.join(".git");
    let head = if git_dir.is_file() {
        None
    } else {
        fs::read_to_string(git_dir.join("HEAD")).ok()
    }?;

    let head = head.trim();
    if let Some(rest) = head.strip_prefix("ref: ") {
        let ref_path = git_dir.join(rest);
        fs::read_to_string(ref_path)
            .ok()
            .map(|sha| sha.trim().to_string())
    } else if !head.is_empty() {
        Some(head.to_string())
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use tempfile::tempdir;

    #[test]
    fn prepare_single_run_loads_issue_body_and_canonicalizes_paths() {
        let tmp = tempdir().expect("tempdir");
        let repo_root = tmp.path().join("repo");
        let output_dir = tmp.path().join("out");
        let issue_path = tmp.path().join("issue.md");

        fs::create_dir_all(repo_root.join(".git")).expect("repo dir");
        fs::write(repo_root.join(".git/HEAD"), "deadbeef\n").expect("head");
        fs::write(&issue_path, "repro details").expect("issue body");

        let prepared = PrepareSingleRunRequest {
            task_id: "demo-task".to_string(),
            repo_root,
            issue: IssueInput {
                title: Some("Fix parsing bug".to_string()),
                body: None,
                body_path: Some(issue_path.clone()),
            },
            output_dir,
            base_sha: Some("abc123".to_string()),
            budget: EvalBudget::default(),
        }
        .prepare()
        .expect("prepare run");

        assert_eq!(prepared.task_id, "demo-task");
        assert_eq!(prepared.issue.body.as_deref(), Some("repro details"));
        assert_eq!(
            prepared.issue.body_path.as_deref(),
            issue_path.canonicalize().ok().as_deref()
        );
        assert_eq!(prepared.head_sha.as_deref(), Some("deadbeef"));
        assert!(prepared.source.is_none());
        assert!(prepared.repo_root.is_absolute());
        assert!(prepared.output_dir.is_absolute());
    }
}
