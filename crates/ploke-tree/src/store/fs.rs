use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use ploke_records::branch::{BranchLogBody, BranchLogRecord, Prototype1BranchRegistry};
use ploke_records::channel::{Envelope, ToChild, ToParent};
use ploke_records::child_plan::ChildPlanRecord;
use ploke_records::evaluation::Artifact as EvaluationArtifact;
use ploke_records::history::SealedBlockRecord;
use ploke_records::identity::ParentIdentityRecord;
use ploke_records::invocation::{
    InvocationRecord, Role, SuccessorCompletionRecord, SuccessorReadyRecord,
};
use ploke_records::journal::JournalEntry;
use ploke_records::protocol::{
    Artifact as ProtocolArtifact, TOOL_CALL_INTENT_SEGMENTATION, TOOL_CALL_REVIEW,
    TOOL_CALL_SEGMENT_REVIEW,
};
use ploke_records::run_profile::{RunProfileCommitmentRecord, RunProfileRecord};
use ploke_records::scheduler::{
    NodeRecord, RunnerRequestRecord, RunnerResultRecord, SchedulerStateRecord,
};
use serde::Deserialize;

use crate::{RunForest, assemble_run_forest};

use super::{
    BranchRegistryEvidence, ChannelEvidence, ChildPlanEvidence, ChildPlanSummary,
    EvaluationArtifactSummary, EvaluationEvidence, HistoryEvidence, JsonlEvidence, JsonlRecord,
    PassiveEvidence, ProtocolArtifactSummary, ProtocolArtifactsEvidence, RunAttemptEvidence,
    RunAttemptSummary, RunForestInput, RunProfileEvidence, RunRecordSet, TransitionJournal,
};

/// Read-only filesystem loader for one Prototype 1 run root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsRunStore {
    run_root: PathBuf,
    parent_identity_path: Option<PathBuf>,
    protocol_artifacts_dir: Option<PathBuf>,
}

impl FsRunStore {
    pub fn new(run_root: impl Into<PathBuf>) -> Self {
        Self {
            run_root: run_root.into(),
            parent_identity_path: None,
            protocol_artifacts_dir: None,
        }
    }

    pub fn with_parent_identity_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.parent_identity_path = Some(path.into());
        self
    }

    pub fn with_parent_root(mut self, parent_root: impl Into<PathBuf>) -> Self {
        self.parent_identity_path = Some(
            parent_root
                .into()
                .join(".ploke")
                .join("prototype1")
                .join("parent_identity.json"),
        );
        self
    }

    pub fn with_protocol_artifacts_dir(mut self, path: impl Into<PathBuf>) -> Self {
        self.protocol_artifacts_dir = Some(path.into());
        self
    }

    pub fn load(&self) -> Result<RunForestInput, FsRunStoreError> {
        let scheduler =
            self.read_json::<SchedulerStateRecord>(&self.run_root.join("scheduler.json"))?;
        let mut node_records = Vec::new();
        let mut successor_ready = Vec::new();
        let mut successor_completion = Vec::new();

        for node_dir in sorted_child_dirs(&self.run_root.join("nodes"))? {
            let node_path = node_dir.join("node.json");
            if node_path.is_file() {
                node_records.push(self.read_json::<NodeRecord>(&node_path)?);
            }

            successor_ready.extend(
                self.read_json_dir::<SuccessorReadyRecord>(&node_dir.join("successor-ready"))?,
            );
            successor_completion.extend(self.read_json_dir::<SuccessorCompletionRecord>(
                &node_dir.join("successor-completion"),
            )?);
        }

        let parent_identity = match &self.parent_identity_path {
            Some(path) if path.is_file() => Some(self.read_json::<ParentIdentityRecord>(path)?),
            _ => None,
        };

        Ok(RunForestInput {
            scheduler,
            node_records,
            parent_identity,
            successor_ready,
            successor_completion,
            passive_evidence: self.load_passive_evidence()?,
        })
    }

    pub fn load_record_set(&self) -> Result<RunRecordSet, FsRunStoreError> {
        Ok(RunRecordSet {
            forest_input: self.load()?,
            history_blocks: self.load_history_blocks()?,
            transition_journal: self.load_transition_journal()?,
        })
    }

    pub fn load_forest(&self) -> Result<RunForest, FsRunStoreError> {
        Ok(assemble_run_forest(self.load()?))
    }

    pub fn load_history_blocks(&self) -> Result<Vec<SealedBlockRecord>, FsRunStoreError> {
        let blocks_dir = self.run_root.join("history").join("blocks");
        if !blocks_dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut blocks = Vec::new();
        for path in sorted_files_with_prefix(&blocks_dir, "segment-", "jsonl")? {
            let file = fs::File::open(&path).map_err(|source| FsRunStoreError::Io {
                path: path.clone(),
                source,
            })?;
            for line in io::BufReader::new(file).lines() {
                let line = line.map_err(|source| FsRunStoreError::Io {
                    path: path.clone(),
                    source,
                })?;
                if line.trim().is_empty() {
                    continue;
                }

                let record =
                    serde_json::from_str::<SealedBlockRecord>(&line).map_err(|source| {
                        FsRunStoreError::Json {
                            path: path.clone(),
                            source,
                        }
                    })?;
                blocks.push(record);
            }
        }

        Ok(blocks)
    }

    pub fn load_transition_journal(&self) -> Result<TransitionJournal, FsRunStoreError> {
        let path = self.run_root.join("transition-journal.jsonl");
        if !path.is_file() {
            return Ok(TransitionJournal::default());
        }

        let entries = self.read_jsonl_records::<JournalEntry>(&path)?;
        Ok(TransitionJournal { entries })
    }

    fn load_passive_evidence(&self) -> Result<PassiveEvidence, FsRunStoreError> {
        Ok(PassiveEvidence {
            branch_registry: self.load_branch_registry_evidence()?,
            transition_journal: self.load_transition_journal_evidence()?,
            history: self.load_history_evidence()?,
            channel_envelopes: self.load_channel_evidence()?,
            child_plans: self.load_child_plan_evidence()?,
            evaluations: self.load_evaluation_evidence()?,
            protocol_artifacts: self.load_protocol_artifacts_evidence()?,
            run_profile: self.load_run_profile_evidence()?,
            run_attempts: self.load_run_attempt_evidence()?,
            attempt_runner_results: self.load_attempt_runner_results()?,
        })
    }

    fn load_branch_registry_evidence(
        &self,
    ) -> Result<Option<BranchRegistryEvidence>, FsRunStoreError> {
        let path = self.run_root.join("branches.json");
        if !path.is_file() {
            return Ok(None);
        }

        if let Some(evidence) = self.load_branch_log_evidence(&path)? {
            return Ok(Some(evidence));
        }

        let registry = self.read_json::<Prototype1BranchRegistry>(&path)?;
        let source_node_count = registry.source_nodes.len();
        let branch_count = registry
            .source_nodes
            .iter()
            .map(|source| source.branches.len())
            .sum();
        let active_target_count = registry.active_targets.len();
        Ok(Some(BranchRegistryEvidence {
            source_node_count,
            branch_count,
            active_target_count,
            record_count: 1,
            registry_snapshot_count: 1,
            parent_comparison_count: 0,
            latest_campaign_id: Some(registry.campaign_id),
            latest_recorded_at: None,
        }))
    }

    fn load_branch_log_evidence(
        &self,
        path: &Path,
    ) -> Result<Option<BranchRegistryEvidence>, FsRunStoreError> {
        let records = match self.read_jsonl_records::<BranchLogRecord>(path) {
            Ok(records) => records,
            Err(FsRunStoreError::JsonLine { line_number: 1, .. }) => return Ok(None),
            Err(source) => return Err(source),
        };

        if records.is_empty() {
            return Ok(Some(BranchRegistryEvidence::default()));
        }

        let mut evidence = BranchRegistryEvidence {
            record_count: records.len(),
            ..BranchRegistryEvidence::default()
        };
        let mut latest_registry = None;
        for record in records {
            evidence.latest_recorded_at = Some(record.record.recorded_at.clone());
            match record.record.body {
                BranchLogBody::RegistrySnapshot(registry) => {
                    evidence.registry_snapshot_count += 1;
                    evidence.latest_campaign_id = Some(registry.campaign_id.clone());
                    latest_registry = Some(registry);
                }
                BranchLogBody::ParentComparison(comparison) => {
                    evidence.parent_comparison_count += 1;
                    evidence.latest_campaign_id = Some(comparison.campaign_id);
                }
            }
        }

        if let Some(registry) = latest_registry {
            evidence.source_node_count = registry.source_nodes.len();
            evidence.branch_count = registry
                .source_nodes
                .iter()
                .map(|source| source.branches.len())
                .sum();
            evidence.active_target_count = registry.active_targets.len();
        }

        Ok(Some(evidence))
    }

    fn load_transition_journal_evidence(&self) -> Result<Option<JsonlEvidence>, FsRunStoreError> {
        let path = self.run_root.join("transition-journal.jsonl");
        if !path.is_file() {
            return Ok(None);
        }

        self.count_jsonl_records::<JournalEntry>(&path).map(Some)
    }

    fn load_history_evidence(&self) -> Result<Option<HistoryEvidence>, FsRunStoreError> {
        let blocks_dir = self.run_root.join("history").join("blocks");
        if !blocks_dir.is_dir() {
            return Ok(None);
        }

        let mut evidence = HistoryEvidence::default();
        for path in sorted_files_with_prefix(&blocks_dir, "segment-", "jsonl")? {
            let file = fs::File::open(&path).map_err(|source| FsRunStoreError::Io {
                path: path.clone(),
                source,
            })?;
            for line in io::BufReader::new(file).lines() {
                let line = line.map_err(|source| FsRunStoreError::Io {
                    path: path.clone(),
                    source,
                })?;
                if line.trim().is_empty() {
                    continue;
                }

                evidence.line_count += 1;
                match serde_json::from_str::<SealedBlockRecord>(&line) {
                    Ok(record) => {
                        evidence.sealed_block_count += 1;
                        evidence.admitted_entry_count += record.entries.len();
                    }
                    Err(source) if source.is_data() => {
                        evidence.record_parse_error_count += 1;
                    }
                    Err(_) => evidence.json_parse_error_count += 1,
                }
            }
        }

        Ok(Some(evidence))
    }

    fn load_channel_evidence(&self) -> Result<Option<ChannelEvidence>, FsRunStoreError> {
        let paths = sorted_nested_channel_jsonl_files(&self.run_root)?;
        if paths.is_empty() {
            return Ok(None);
        }

        let mut evidence = ChannelEvidence::default();
        for path in paths {
            evidence.file_count += 1;
            let file = fs::File::open(&path).map_err(|source| FsRunStoreError::Io {
                path: path.clone(),
                source,
            })?;
            for line in io::BufReader::new(file).lines() {
                let line = line.map_err(|source| FsRunStoreError::Io {
                    path: path.clone(),
                    source,
                })?;
                if line.trim().is_empty() {
                    continue;
                }

                evidence.line_count += 1;
                if serde_json::from_str::<Envelope<ToParent>>(&line).is_ok()
                    || serde_json::from_str::<Envelope<ToChild>>(&line).is_ok()
                {
                    evidence.parsed_count += 1;
                } else {
                    evidence.parse_error_count += 1;
                }
            }
        }

        Ok(Some(evidence))
    }

    fn load_child_plan_evidence(&self) -> Result<Option<ChildPlanEvidence>, FsRunStoreError> {
        let dir = self.run_root.join("messages").join("child-plan");
        if !dir.is_dir() {
            return Ok(None);
        }

        let mut summary = ChildPlanSummary::default();
        let mut index = BTreeMap::new();
        for path in sorted_json_files(&dir)? {
            summary.file_count += 1;
            let plan = self.read_json::<ChildPlanRecord>(&path)?;
            summary.parsed_count += 1;
            summary.child_count += plan.children.len();
            summary.children_with_surface_count += plan
                .children
                .iter()
                .filter(|child| child.surface.is_some())
                .count();
            summary.rejected_surface_attempt_count += plan.rejected_surface_attempts.len();
            index.insert(plan.parent_node_id.as_str().to_owned(), plan);
        }

        Ok(Some(ChildPlanEvidence { summary, index }))
    }

    pub fn load_evaluation_evidence(&self) -> Result<Option<EvaluationEvidence>, FsRunStoreError> {
        let dir = self.run_root.join("evaluations");
        if !dir.is_dir() {
            return Ok(None);
        }

        let mut index = BTreeMap::new();
        let mut summary = EvaluationArtifactSummary::default();
        for path in sorted_json_files(&dir)? {
            summary.file_count += 1;
            let artifact = self.read_json::<EvaluationArtifact>(&path)?;
            summary.parsed_count += 1;
            if artifact.overall_disposition == ploke_records::branch::Disposition::Keep {
                summary.keep_count += 1;
            } else {
                summary.reject_count += 1;
            }
            index.insert(artifact.branch_id.clone(), artifact);
        }

        Ok(Some(EvaluationEvidence { summary, index }))
    }

    fn load_protocol_artifacts_evidence(
        &self,
    ) -> Result<Option<ProtocolArtifactsEvidence>, FsRunStoreError> {
        let Some(dir) = &self.protocol_artifacts_dir else {
            return Ok(None);
        };
        if !dir.is_dir() {
            return Ok(None);
        }

        let mut summary = ProtocolArtifactSummary::default();
        let mut index = BTreeMap::new();
        for path in sorted_json_files(dir)? {
            summary.file_count += 1;
            let artifact = self.read_json::<ProtocolArtifact>(&path)?;
            summary.parsed_count += 1;
            match artifact.procedure_name.as_str() {
                TOOL_CALL_INTENT_SEGMENTATION => summary.intent_segmentation_count += 1,
                TOOL_CALL_REVIEW => summary.review_count += 1,
                TOOL_CALL_SEGMENT_REVIEW => summary.segment_review_count += 1,
                _ => {}
            }
            if artifact.body().is_some() {
                summary.typed_payload_count += 1;
            }
            index.insert(protocol_artifact_key(&path), artifact);
        }

        Ok(Some(ProtocolArtifactsEvidence { summary, index }))
    }

    fn load_run_profile_evidence(&self) -> Result<Option<RunProfileEvidence>, FsRunStoreError> {
        let profile_path = self.run_root.join("run-profile.toml");
        let commitment_path = self.run_root.join("run-profile.commitment.json");
        if !profile_path.is_file() && !commitment_path.is_file() {
            return Ok(None);
        }

        let profile = if profile_path.is_file() {
            Some(self.read_toml::<RunProfileRecord>(&profile_path)?)
        } else {
            None
        };
        let commitment = if commitment_path.is_file() {
            Some(self.read_json::<RunProfileCommitmentRecord>(&commitment_path)?)
        } else {
            None
        };

        Ok(Some(RunProfileEvidence {
            profile,
            commitment,
        }))
    }

    fn load_run_attempt_evidence(&self) -> Result<Option<RunAttemptEvidence>, FsRunStoreError> {
        let nodes_root = self.run_root.join("nodes");
        if !nodes_root.is_dir() {
            return Ok(None);
        }

        let mut summary = RunAttemptSummary::default();
        let mut runner_requests = BTreeMap::new();
        let mut runner_results = BTreeMap::new();
        let mut invocations = BTreeMap::new();

        for node_dir in sorted_child_dirs(&nodes_root)? {
            let request_path = node_dir.join("runner-request.json");
            if request_path.is_file() {
                summary.runner_request_file_count += 1;
                let request = self.read_json::<RunnerRequestRecord>(&request_path)?;
                summary.runner_request_parsed_count += 1;
                runner_requests.insert(run_relative_key(&self.run_root, &request_path), request);
            }

            let result_path = node_dir.join("runner-result.json");
            if result_path.is_file() {
                summary.runner_result_file_count += 1;
                let result = self.read_json::<RunnerResultRecord>(&result_path)?;
                summary.runner_result_parsed_count += 1;
                runner_results.insert(run_relative_key(&self.run_root, &result_path), result);
            }

            for invocation_path in sorted_json_files(&node_dir.join("invocations"))? {
                summary.invocation_file_count += 1;
                let invocation = self.read_json::<InvocationRecord>(&invocation_path)?;
                summary.invocation_parsed_count += 1;
                match invocation.role {
                    Role::Child => summary.child_invocation_count += 1,
                    Role::Successor => summary.successor_invocation_count += 1,
                }
                invocations.insert(
                    run_relative_key(&self.run_root, &invocation_path),
                    invocation,
                );
            }
        }

        if summary.runner_request_file_count == 0
            && summary.runner_result_file_count == 0
            && summary.invocation_file_count == 0
        {
            return Ok(None);
        }

        Ok(Some(RunAttemptEvidence {
            summary,
            runner_requests,
            runner_results,
            invocations,
        }))
    }

    fn load_attempt_runner_results(
        &self,
    ) -> Result<BTreeMap<String, RunnerResultRecord>, FsRunStoreError> {
        let nodes_root = self.run_root.join("nodes");
        if !nodes_root.is_dir() {
            return Ok(BTreeMap::new());
        }

        let mut attempt_runner_results = BTreeMap::new();
        for node_dir in sorted_child_dirs(&nodes_root)? {
            for result_path in sorted_json_files(&node_dir.join("results"))? {
                let result = self.read_json::<RunnerResultRecord>(&result_path)?;
                attempt_runner_results
                    .insert(run_relative_key(&self.run_root, &result_path), result);
            }
        }

        Ok(attempt_runner_results)
    }

    fn read_json<T>(&self, path: &Path) -> Result<T, FsRunStoreError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let bytes = fs::read(path).map_err(|source| FsRunStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_slice(&bytes).map_err(|source| FsRunStoreError::Json {
            path: path.to_path_buf(),
            source,
        })
    }

    fn read_toml<T>(&self, path: &Path) -> Result<T, FsRunStoreError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let source_text = fs::read_to_string(path).map_err(|source| FsRunStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&source_text).map_err(|source| FsRunStoreError::Toml {
            path: path.to_path_buf(),
            source,
        })
    }

    fn read_json_dir<T>(&self, dir: &Path) -> Result<Vec<T>, FsRunStoreError>
    where
        T: for<'de> Deserialize<'de>,
    {
        if !dir.is_dir() {
            return Ok(Vec::new());
        }

        let mut records = Vec::new();
        for path in sorted_json_files(dir)? {
            records.push(self.read_json::<T>(&path)?);
        }
        Ok(records)
    }

    fn count_jsonl_records<T>(&self, path: &Path) -> Result<JsonlEvidence, FsRunStoreError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let file = fs::File::open(path).map_err(|source| FsRunStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let mut evidence = JsonlEvidence::default();

        for line in io::BufReader::new(file).lines() {
            let line = line.map_err(|source| FsRunStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }

            evidence.line_count += 1;
            match serde_json::from_str::<T>(&line) {
                Ok(_) => evidence.parsed_count += 1,
                Err(_) => evidence.parse_error_count += 1,
            }
        }

        Ok(evidence)
    }

    fn read_jsonl_records<T>(&self, path: &Path) -> Result<Vec<JsonlRecord<T>>, FsRunStoreError>
    where
        T: for<'de> Deserialize<'de>,
    {
        let file = fs::File::open(path).map_err(|source| FsRunStoreError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        let mut records = Vec::new();

        for (line_number, line) in io::BufReader::new(file).lines().enumerate() {
            let line = line.map_err(|source| FsRunStoreError::Io {
                path: path.to_path_buf(),
                source,
            })?;
            if line.trim().is_empty() {
                continue;
            }

            let record =
                serde_json::from_str::<T>(&line).map_err(|source| FsRunStoreError::JsonLine {
                    path: path.to_path_buf(),
                    line_number: line_number + 1,
                    source,
                })?;
            records.push(JsonlRecord {
                line_number: line_number + 1,
                record,
            });
        }

        Ok(records)
    }
}

#[derive(Debug)]
pub enum FsRunStoreError {
    Io {
        path: PathBuf,
        source: io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
    Toml {
        path: PathBuf,
        source: toml::de::Error,
    },
    JsonLine {
        path: PathBuf,
        line_number: usize,
        source: serde_json::Error,
    },
}

impl fmt::Display for FsRunStoreError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io { path, source } => {
                write!(formatter, "failed to read {}: {source}", path.display())
            }
            Self::Json { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
            Self::Toml { path, source } => {
                write!(formatter, "failed to parse {}: {source}", path.display())
            }
            Self::JsonLine {
                path,
                line_number,
                source,
            } => {
                write!(
                    formatter,
                    "failed to parse {} line {}: {source}",
                    path.display(),
                    line_number
                )
            }
        }
    }
}

impl Error for FsRunStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
            Self::Toml { source, .. } => Some(source),
            Self::JsonLine { source, .. } => Some(source),
        }
    }
}

fn sorted_child_dirs(parent: &Path) -> Result<Vec<PathBuf>, FsRunStoreError> {
    if !parent.is_dir() {
        return Ok(Vec::new());
    }

    let mut dirs = Vec::new();
    for entry in fs::read_dir(parent).map_err(|source| FsRunStoreError::Io {
        path: parent.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| FsRunStoreError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_dir() {
            dirs.push(path);
        }
    }
    dirs.sort();
    Ok(dirs)
}

fn sorted_json_files(parent: &Path) -> Result<Vec<PathBuf>, FsRunStoreError> {
    sorted_files_with_extension(parent, "json")
}

fn sorted_files_with_extension(
    parent: &Path,
    extension: &str,
) -> Result<Vec<PathBuf>, FsRunStoreError> {
    if !parent.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for entry in fs::read_dir(parent).map_err(|source| FsRunStoreError::Io {
        path: parent.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| FsRunStoreError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|value| value.to_str()) == Some(extension) {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths)
}

fn sorted_files_with_prefix(
    parent: &Path,
    prefix: &str,
    extension: &str,
) -> Result<Vec<PathBuf>, FsRunStoreError> {
    let mut paths = sorted_files_with_extension(parent, extension)?;
    paths.retain(|path| {
        path.file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name.starts_with(prefix))
    });
    Ok(paths)
}

fn sorted_nested_channel_jsonl_files(run_root: &Path) -> Result<Vec<PathBuf>, FsRunStoreError> {
    let nodes_root = run_root.join("nodes");
    if !nodes_root.is_dir() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    for node_dir in sorted_child_dirs(&nodes_root)? {
        let channels_dir = node_dir.join("channels");
        if !channels_dir.is_dir() {
            continue;
        }

        for stream_dir in sorted_child_dirs(&channels_dir)? {
            paths.extend(sorted_files_with_extension(&stream_dir, "jsonl")?);
        }
    }
    paths.sort();
    Ok(paths)
}

fn protocol_artifact_key(path: &Path) -> String {
    path.file_stem()
        .and_then(|name| name.to_str())
        .or_else(|| path.file_name().and_then(|name| name.to_str()))
        .unwrap_or("unknown")
        .to_owned()
}

fn run_relative_key(run_root: &Path, path: &Path) -> String {
    path.strip_prefix(run_root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}
