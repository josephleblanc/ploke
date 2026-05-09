//! Read-only tree projections over passive Ploke records.
//!
//! This crate assembles UI-facing DTOs from passive records. Its filesystem
//! loader only deserializes machine-readable run files; it does not mutate loop
//! state, parse human output, or infer History authority from passive evidence.

use std::collections::BTreeMap;
use std::error::Error;
use std::fmt;
use std::fs;
use std::io::{self, BufRead};
use std::path::{Path, PathBuf};

use ploke_records::branch::Prototype1BranchRegistry;
use ploke_records::channel::{Envelope, ToChild, ToParent};
use ploke_records::evaluation::Artifact as EvaluationArtifact;
use ploke_records::history::SealedBlockRecord;
use ploke_records::identity::ParentIdentityRecord;
use ploke_records::invocation::{
    SuccessorCompletionRecord, SuccessorCompletionStatus, SuccessorReadyRecord,
};
use ploke_records::journal::JournalEntry;
use ploke_records::scheduler::{NodeRecord, NodeStatusRecord, SchedulerStateRecord};
use serde::{Deserialize, Serialize};

/// In-memory inputs for one run-forest projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunForestInput {
    pub scheduler: SchedulerStateRecord,
    #[serde(default)]
    pub node_records: Vec<NodeRecord>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_identity: Option<ParentIdentityRecord>,
    #[serde(default)]
    pub successor_ready: Vec<SuccessorReadyRecord>,
    #[serde(default)]
    pub successor_completion: Vec<SuccessorCompletionRecord>,
    #[serde(default)]
    pub passive_evidence: PassiveEvidence,
}

/// UI-facing forest assembled from passive records.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct RunForest {
    pub campaign: CampaignRef,
    pub roots: Vec<NodeKey>,
    pub nodes: Vec<TreeNode>,
    pub lanes: Lanes,
    #[serde(default)]
    pub passive_evidence: PassiveEvidence,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl RunForest {
    /// Assemble a read-only projection from already-loaded passive records.
    pub fn from_records(input: RunForestInput) -> Self {
        assemble_run_forest(input)
    }
}

/// Assemble a read-only projection from already-loaded passive records.
pub fn assemble_run_forest(input: RunForestInput) -> RunForest {
    let RunForestInput {
        scheduler,
        node_records,
        parent_identity,
        successor_ready,
        successor_completion,
        passive_evidence,
    } = input;

    let campaign_id = scheduler.campaign_id.as_str().to_owned();
    let mut diagnostics = Vec::new();
    let merged_node_records = merge_node_records(&scheduler.nodes, node_records, &mut diagnostics);
    let mut nodes = merged_node_records
        .iter()
        .map(TreeNode::from_scheduler_node)
        .collect::<Vec<_>>();

    let mut index_by_key = BTreeMap::new();
    for (idx, node) in nodes.iter().enumerate() {
        if index_by_key.insert(node.key.clone(), idx).is_some() {
            diagnostics.push(Diagnostic::forest(
                DiagnosticSeverity::Error,
                "duplicate_scheduler_node",
                format!("scheduler contains duplicate node id {}", node.key.as_str()),
            ));
        }
    }

    let mut roots = Vec::new();
    for source in &merged_node_records {
        let key = NodeKey::from(source.node_id.as_str());
        let Some(idx) = index_by_key.get(&key).copied() else {
            continue;
        };

        match source.parent_node_id.as_ref() {
            Some(parent_id) => {
                let parent_key = NodeKey::from(parent_id.as_str());
                if let Some(parent_idx) = index_by_key.get(&parent_key).copied() {
                    nodes[idx].parent = Some(parent_key.clone());
                    nodes[parent_idx].children.push(key);
                } else {
                    roots.push(key.clone());
                    nodes[idx].diagnostics.push(Diagnostic::node(
                        DiagnosticSeverity::Warning,
                        "missing_scheduler_parent",
                        format!(
                            "node {} names missing scheduler parent {}",
                            key.as_str(),
                            parent_key.as_str()
                        ),
                        key,
                    ));
                }
            }
            None => roots.push(key),
        }
    }

    if let Some(parent) = parent_identity {
        attach_parent_identity(
            &mut nodes,
            &index_by_key,
            &mut diagnostics,
            &campaign_id,
            parent,
        );
    }

    for ready in successor_ready {
        attach_successor_ready(
            &mut nodes,
            &index_by_key,
            &mut diagnostics,
            &campaign_id,
            ready,
        );
    }

    for completion in successor_completion {
        attach_successor_completion(
            &mut nodes,
            &index_by_key,
            &mut diagnostics,
            &campaign_id,
            completion,
        );
    }

    RunForest {
        campaign: CampaignRef {
            campaign_id,
            updated_at: scheduler.updated_at,
        },
        roots,
        nodes,
        lanes: Lanes {
            frontier: scheduler
                .frontier_node_ids
                .iter()
                .map(|id| NodeKey::from(id.as_str()))
                .collect(),
            completed: scheduler
                .completed_node_ids
                .iter()
                .map(|id| NodeKey::from(id.as_str()))
                .collect(),
            failed: scheduler
                .failed_node_ids
                .iter()
                .map(|id| NodeKey::from(id.as_str()))
                .collect(),
        },
        passive_evidence,
        diagnostics,
    }
}

fn merge_node_records(
    scheduler_nodes: &[NodeRecord],
    node_records: Vec<NodeRecord>,
    diagnostics: &mut Vec<Diagnostic>,
) -> Vec<NodeRecord> {
    let mut merged = scheduler_nodes.to_vec();
    let mut index_by_node_id = BTreeMap::new();

    for (idx, node) in merged.iter().enumerate() {
        if index_by_node_id
            .insert(node.node_id.as_str().to_owned(), idx)
            .is_some()
        {
            diagnostics.push(Diagnostic::forest(
                DiagnosticSeverity::Error,
                "duplicate_scheduler_node",
                format!("scheduler contains duplicate node id {}", node.node_id),
            ));
        }
    }

    for node in node_records {
        let node_id = node.node_id.as_str().to_owned();
        if let Some(idx) = index_by_node_id.get(&node_id).copied() {
            merged[idx] = node;
        } else {
            index_by_node_id.insert(node_id, merged.len());
            merged.push(node);
        }
    }

    merged
}

/// Read-only filesystem loader for one Prototype 1 run root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FsRunStore {
    run_root: PathBuf,
    parent_identity_path: Option<PathBuf>,
}

impl FsRunStore {
    pub fn new(run_root: impl Into<PathBuf>) -> Self {
        Self {
            run_root: run_root.into(),
            parent_identity_path: None,
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

    pub fn load_forest(&self) -> Result<RunForest, FsRunStoreError> {
        Ok(assemble_run_forest(self.load()?))
    }

    fn load_passive_evidence(&self) -> Result<PassiveEvidence, FsRunStoreError> {
        Ok(PassiveEvidence {
            branch_registry: self.load_branch_registry_evidence()?,
            transition_journal: self.load_transition_journal_evidence()?,
            history: self.load_history_evidence()?,
            channel_envelopes: self.load_channel_evidence()?,
            evaluations: self.load_evaluation_evidence()?,
        })
    }

    fn load_branch_registry_evidence(
        &self,
    ) -> Result<Option<BranchRegistryEvidence>, FsRunStoreError> {
        let path = self.run_root.join("branches.json");
        if !path.is_file() {
            return Ok(None);
        }

        let registry = self.read_json::<Prototype1BranchRegistry>(&path)?;
        let branch_count = registry
            .source_nodes
            .iter()
            .map(|source| source.branches.len())
            .sum();
        Ok(Some(BranchRegistryEvidence {
            source_node_count: registry.source_nodes.len(),
            branch_count,
            active_target_count: registry.active_targets.len(),
        }))
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
                    Err(_) => match serde_json::from_str::<serde_json::Value>(&line) {
                        Ok(value) => {
                            evidence.record_parse_error_count += 1;
                            if let Some(entries) = value.get("entries").and_then(|v| v.as_array()) {
                                evidence.sealed_block_count += 1;
                                evidence.admitted_entry_count += entries.len();
                            }
                        }
                        Err(_) => evidence.json_parse_error_count += 1,
                    },
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

    fn load_evaluation_evidence(&self) -> Result<Option<EvaluationEvidence>, FsRunStoreError> {
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
        }
    }
}

impl Error for FsRunStoreError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
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

fn attach_parent_identity(
    nodes: &mut [TreeNode],
    index_by_key: &BTreeMap<NodeKey, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    campaign_id: &str,
    parent: ParentIdentityRecord,
) {
    let evidence = EvidenceRef {
        kind: EvidenceKind::ParentIdentity,
        authority: AuthorityLabel::TypedRecordEvidence,
        node_key: Some(NodeKey::from(parent.node_id.as_str())),
        runtime_id: None,
        recorded_at: Some(parent.created_at),
        detail: Some(format!("parent_id={}", parent.parent_id)),
    };

    if parent.campaign_id != campaign_id {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "parent_identity_campaign_mismatch",
            format!(
                "parent identity campaign {} does not match scheduler campaign {}",
                parent.campaign_id, campaign_id
            ),
            evidence,
        ));
        return;
    }

    let key = evidence
        .node_key
        .clone()
        .expect("parent identity has node key");
    if let Some(idx) = index_by_key.get(&key).copied() {
        nodes[idx].evidence.push(evidence);
    } else {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "parent_identity_without_scheduler_node",
            format!(
                "parent identity references node {} absent from scheduler projection",
                key.as_str()
            ),
            evidence,
        ));
    }
}

fn attach_successor_ready(
    nodes: &mut [TreeNode],
    index_by_key: &BTreeMap<NodeKey, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    campaign_id: &str,
    ready: SuccessorReadyRecord,
) {
    let key = NodeKey::from(ready.node_id.as_str());
    let evidence = EvidenceRef {
        kind: EvidenceKind::SuccessorReady,
        authority: AuthorityLabel::TypedRecordEvidence,
        node_key: Some(key.clone()),
        runtime_id: Some(ready.runtime_id.to_string()),
        recorded_at: Some(ready.recorded_at),
        detail: Some(format!("pid={}", ready.pid)),
    };

    if ready.campaign_id != campaign_id {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "successor_ready_campaign_mismatch",
            format!(
                "successor ready campaign {} does not match scheduler campaign {}",
                ready.campaign_id, campaign_id
            ),
            evidence,
        ));
        return;
    }

    if let Some(idx) = index_by_key.get(&key).copied() {
        nodes[idx].evidence.push(evidence);
    } else {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "successor_ready_without_scheduler_node",
            format!(
                "successor ready record references node {} absent from scheduler projection",
                key.as_str()
            ),
            evidence,
        ));
    }
}

fn attach_successor_completion(
    nodes: &mut [TreeNode],
    index_by_key: &BTreeMap<NodeKey, usize>,
    diagnostics: &mut Vec<Diagnostic>,
    campaign_id: &str,
    completion: SuccessorCompletionRecord,
) {
    let key = NodeKey::from(completion.node_id.as_str());
    let status = completion.status;
    let evidence = EvidenceRef {
        kind: EvidenceKind::SuccessorCompletion,
        authority: AuthorityLabel::TypedRecordEvidence,
        node_key: Some(key.clone()),
        runtime_id: Some(completion.runtime_id.to_string()),
        recorded_at: Some(completion.recorded_at),
        detail: completion.detail,
    };

    if completion.campaign_id != campaign_id {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "successor_completion_campaign_mismatch",
            format!(
                "successor completion campaign {} does not match scheduler campaign {}",
                completion.campaign_id, campaign_id
            ),
            evidence,
        ));
        return;
    }

    if let Some(idx) = index_by_key.get(&key).copied() {
        nodes[idx].evidence.push(evidence.clone());
        if status == SuccessorCompletionStatus::Failed {
            nodes[idx].diagnostics.push(Diagnostic::node_with_evidence(
                DiagnosticSeverity::Warning,
                "successor_completion_failed",
                format!("successor completion failed for node {}", key.as_str()),
                key,
                evidence,
            ));
        }
    } else {
        diagnostics.push(Diagnostic::forest_with_evidence(
            DiagnosticSeverity::Warning,
            "successor_completion_without_scheduler_node",
            format!(
                "successor completion references node {} absent from scheduler projection",
                key.as_str()
            ),
            evidence,
        ));
    }
}

/// Passive evidence counts loaded beside the scheduler tree.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct PassiveEvidence {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub branch_registry: Option<BranchRegistryEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub transition_journal: Option<JsonlEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub history: Option<HistoryEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub channel_envelopes: Option<ChannelEvidence>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluations: Option<EvaluationEvidence>,
}

/// Counts from a passive Prototype 1 branch registry.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct BranchRegistryEvidence {
    pub source_node_count: usize,
    pub branch_count: usize,
    pub active_target_count: usize,
}

/// Counts from a JSONL evidence file parsed as passive records.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct JsonlEvidence {
    pub line_count: usize,
    pub parsed_count: usize,
    pub parse_error_count: usize,
}

/// Counts from passive History block storage.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct HistoryEvidence {
    pub line_count: usize,
    pub sealed_block_count: usize,
    pub admitted_entry_count: usize,
    pub record_parse_error_count: usize,
    pub json_parse_error_count: usize,
}

/// Counts from passive runtime channel envelope files.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChannelEvidence {
    pub file_count: usize,
    pub line_count: usize,
    pub parsed_count: usize,
    pub parse_error_count: usize,
}

/// Read-only typed evidence loaded from evaluation artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
pub struct EvaluationEvidence {
    pub summary: EvaluationArtifactSummary,
    pub index: BTreeMap<String, EvaluationArtifact>,
}

/// Counts from persisted evaluation artifacts.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvaluationArtifactSummary {
    pub file_count: usize,
    pub parsed_count: usize,
    pub keep_count: usize,
    pub reject_count: usize,
}

/// Stable node key used by tree projections.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct NodeKey(String);

impl NodeKey {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for NodeKey {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

impl From<String> for NodeKey {
    fn from(value: String) -> Self {
        Self(value)
    }
}

/// Coarse kind of node visible to UI renderers.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    SchedulerSearchNode,
}

/// Authority level for a projected node or attached evidence.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthorityLabel {
    MutableProjection,
    TypedRecordEvidence,
    LiveTransport,
    SealedVerifiedHistory,
    DegradedObservation,
}

/// Progress folded from scheduler node status.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Progress {
    pub phase: Phase,
    pub terminality: Terminality,
    pub result_class: ResultClass,
}

/// UI phase for a scheduler node.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Planned,
    WorkspaceStaged,
    BinaryBuilt,
    Running,
    Completed,
    Failed,
    Unknown,
}

/// Whether a node appears terminal from the projection source.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Terminality {
    NonTerminal,
    Terminal,
    Unknown,
}

/// Result class visible from passive records.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResultClass {
    Success,
    Failure,
    Unknown,
}

/// Reference to a source record that contributed to the projection.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct EvidenceRef {
    pub kind: EvidenceKind,
    pub authority: AuthorityLabel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_key: Option<NodeKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub runtime_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

/// Source record kind for an evidence reference.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceKind {
    SchedulerNode,
    ParentIdentity,
    SuccessorReady,
    SuccessorCompletion,
}

/// Projection diagnostic for conservative assembly gaps.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Diagnostic {
    pub severity: DiagnosticSeverity,
    pub code: String,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub node_key: Option<NodeKey>,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
}

impl Diagnostic {
    fn forest(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            node_key: None,
            evidence: Vec::new(),
        }
    }

    fn forest_with_evidence(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        evidence: EvidenceRef,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            node_key: None,
            evidence: vec![evidence],
        }
    }

    fn node(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        node_key: NodeKey,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            node_key: Some(node_key),
            evidence: Vec::new(),
        }
    }

    fn node_with_evidence(
        severity: DiagnosticSeverity,
        code: impl Into<String>,
        message: impl Into<String>,
        node_key: NodeKey,
        evidence: EvidenceRef,
    ) -> Self {
        Self {
            severity,
            code: code.into(),
            message: message.into(),
            node_key: Some(node_key),
            evidence: vec![evidence],
        }
    }
}

/// Severity for projection diagnostics.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

/// Lightweight campaign identity for renderers.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CampaignRef {
    pub campaign_id: String,
    pub updated_at: String,
}

/// Scheduler lane membership as a projection aid.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lanes {
    pub frontier: Vec<NodeKey>,
    pub completed: Vec<NodeKey>,
    pub failed: Vec<NodeKey>,
}

/// One tree node in a run forest.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreeNode {
    pub key: NodeKey,
    pub kind: NodeKind,
    pub authority: AuthorityLabel,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent: Option<NodeKey>,
    #[serde(default)]
    pub children: Vec<NodeKey>,
    pub generation: u32,
    pub branch_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_branch_id: Option<String>,
    pub candidate_id: String,
    pub instance_id: String,
    pub source_state_id: String,
    pub target_relpath: String,
    pub progress: Progress,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub evidence: Vec<EvidenceRef>,
    #[serde(default)]
    pub diagnostics: Vec<Diagnostic>,
}

impl TreeNode {
    fn from_scheduler_node(record: &NodeRecord) -> Self {
        let key = NodeKey::from(record.node_id.as_str());
        Self {
            key: key.clone(),
            kind: NodeKind::SchedulerSearchNode,
            authority: AuthorityLabel::MutableProjection,
            parent: None,
            children: Vec::new(),
            generation: record.generation,
            branch_id: record.branch_id.as_str().to_owned(),
            parent_branch_id: record
                .parent_branch_id
                .as_ref()
                .map(|branch_id| branch_id.as_str().to_owned()),
            candidate_id: record.candidate_id.as_str().to_owned(),
            instance_id: record.instance_id.as_str().to_owned(),
            source_state_id: record.source_state_id.as_str().to_owned(),
            target_relpath: record.target_relpath.to_string_lossy().into_owned(),
            progress: progress_from_status(record.status),
            created_at: record.created_at.clone(),
            updated_at: record.updated_at.clone(),
            evidence: vec![EvidenceRef {
                kind: EvidenceKind::SchedulerNode,
                authority: AuthorityLabel::MutableProjection,
                node_key: Some(key),
                runtime_id: None,
                recorded_at: Some(record.updated_at.clone()),
                detail: None,
            }],
            diagnostics: Vec::new(),
        }
    }
}

fn progress_from_status(status: NodeStatusRecord) -> Progress {
    match status {
        NodeStatusRecord::Planned => Progress {
            phase: Phase::Planned,
            terminality: Terminality::NonTerminal,
            result_class: ResultClass::Unknown,
        },
        NodeStatusRecord::WorkspaceStaged => Progress {
            phase: Phase::WorkspaceStaged,
            terminality: Terminality::NonTerminal,
            result_class: ResultClass::Unknown,
        },
        NodeStatusRecord::BinaryBuilt => Progress {
            phase: Phase::BinaryBuilt,
            terminality: Terminality::NonTerminal,
            result_class: ResultClass::Unknown,
        },
        NodeStatusRecord::Running => Progress {
            phase: Phase::Running,
            terminality: Terminality::NonTerminal,
            result_class: ResultClass::Unknown,
        },
        NodeStatusRecord::Succeeded => Progress {
            phase: Phase::Completed,
            terminality: Terminality::Terminal,
            result_class: ResultClass::Success,
        },
        NodeStatusRecord::Failed => Progress {
            phase: Phase::Failed,
            terminality: Terminality::Terminal,
            result_class: ResultClass::Failure,
        },
    }
}

#[cfg(test)]
mod tests {
    use std::fs;
    use std::path::{Path, PathBuf};
    use std::time::{SystemTime, UNIX_EPOCH};

    use ploke_records::identity::ParentIdentityRecord;
    use ploke_records::ids::{
        BranchId, CampaignId, CandidateId, InstanceId, RuntimeId, SchedulerNodeId, SourceStateId,
    };

    use super::*;

    #[test]
    fn scheduler_nodes_become_mutable_projection_tree_nodes() {
        let forest = assemble_run_forest(RunForestInput {
            scheduler: scheduler(vec![node("node-0", None, NodeStatusRecord::Running)]),
            node_records: Vec::new(),
            parent_identity: None,
            successor_ready: Vec::new(),
            successor_completion: Vec::new(),
            passive_evidence: PassiveEvidence::default(),
        });

        assert_eq!(forest.roots, vec![NodeKey::from("node-0")]);
        assert_eq!(forest.nodes.len(), 1);
        assert_eq!(forest.nodes[0].authority, AuthorityLabel::MutableProjection);
        assert_eq!(forest.nodes[0].progress.phase, Phase::Running);
        assert_eq!(
            forest.nodes[0].evidence[0].authority,
            AuthorityLabel::MutableProjection
        );
    }

    #[test]
    fn parent_child_relationships_use_only_explicit_scheduler_parent_fields() {
        let mut branch_only = node("branch-only", None, NodeStatusRecord::Planned);
        branch_only.parent_branch_id = Some(BranchId("branch-root".to_owned()));

        let forest = assemble_run_forest(RunForestInput {
            scheduler: scheduler(vec![
                node("root", None, NodeStatusRecord::Succeeded),
                node("child", Some("root"), NodeStatusRecord::Planned),
                branch_only,
            ]),
            node_records: Vec::new(),
            parent_identity: None,
            successor_ready: Vec::new(),
            successor_completion: Vec::new(),
            passive_evidence: PassiveEvidence::default(),
        });

        let root = forest.node("root");
        let child = forest.node("child");
        let branch_only = forest.node("branch-only");

        assert_eq!(
            forest.roots,
            vec![NodeKey::from("root"), NodeKey::from("branch-only")]
        );
        assert_eq!(root.children, vec![NodeKey::from("child")]);
        assert_eq!(child.parent, Some(NodeKey::from("root")));
        assert_eq!(branch_only.parent_branch_id, Some("branch-root".to_owned()));
        assert_eq!(branch_only.parent, None);
        assert!(branch_only.children.is_empty());
    }

    #[test]
    fn missing_scheduler_parent_stays_root_with_diagnostic() {
        let forest = assemble_run_forest(RunForestInput {
            scheduler: scheduler(vec![node(
                "child",
                Some("missing"),
                NodeStatusRecord::Planned,
            )]),
            node_records: Vec::new(),
            parent_identity: None,
            successor_ready: Vec::new(),
            successor_completion: Vec::new(),
            passive_evidence: PassiveEvidence::default(),
        });

        let child = forest.node("child");
        assert_eq!(forest.roots, vec![NodeKey::from("child")]);
        assert_eq!(child.parent, None);
        assert_eq!(child.diagnostics[0].code, "missing_scheduler_parent");
    }

    #[test]
    fn successor_records_attach_evidence_without_history_authority() {
        let forest = assemble_run_forest(RunForestInput {
            scheduler: scheduler(vec![node("node-1", None, NodeStatusRecord::Succeeded)]),
            node_records: Vec::new(),
            parent_identity: None,
            successor_ready: vec![SuccessorReadyRecord {
                schema_version: "prototype1-successor-ready.v1".to_owned(),
                campaign_id: "campaign-1".to_owned(),
                node_id: "node-1".to_owned(),
                runtime_id: RuntimeId("runtime-1".to_owned()),
                pid: 42,
                recorded_at: "2026-05-08T12:02:00Z".to_owned(),
            }],
            successor_completion: vec![SuccessorCompletionRecord {
                schema_version: "prototype1-successor-completion.v1".to_owned(),
                campaign_id: "campaign-1".to_owned(),
                node_id: "node-1".to_owned(),
                runtime_id: RuntimeId("runtime-1".to_owned()),
                status: SuccessorCompletionStatus::Succeeded,
                trace_path: None,
                detail: None,
                recorded_at: "2026-05-08T12:03:00Z".to_owned(),
            }],
            passive_evidence: PassiveEvidence::default(),
        });

        let node = forest.node("node-1");
        assert_eq!(node.authority, AuthorityLabel::MutableProjection);
        assert!(
            node.evidence
                .iter()
                .any(|evidence| evidence.kind == EvidenceKind::SuccessorReady
                    && evidence.authority == AuthorityLabel::TypedRecordEvidence)
        );
        assert!(node.evidence.iter().any(|evidence| evidence.kind
            == EvidenceKind::SuccessorCompletion
            && evidence.authority == AuthorityLabel::TypedRecordEvidence));
        assert!(
            !node
                .evidence
                .iter()
                .any(|evidence| evidence.authority == AuthorityLabel::SealedVerifiedHistory)
        );
    }

    #[test]
    fn successor_completion_failure_adds_diagnostic_not_history_authority() {
        let forest = assemble_run_forest(RunForestInput {
            scheduler: scheduler(vec![node("node-1", None, NodeStatusRecord::Succeeded)]),
            node_records: Vec::new(),
            parent_identity: None,
            successor_ready: Vec::new(),
            successor_completion: vec![SuccessorCompletionRecord {
                schema_version: "prototype1-successor-completion.v1".to_owned(),
                campaign_id: "campaign-1".to_owned(),
                node_id: "node-1".to_owned(),
                runtime_id: RuntimeId("runtime-1".to_owned()),
                status: SuccessorCompletionStatus::Failed,
                trace_path: None,
                detail: Some("rehydration failed".to_owned()),
                recorded_at: "2026-05-08T12:03:00Z".to_owned(),
            }],
            passive_evidence: PassiveEvidence::default(),
        });

        let node = forest.node("node-1");
        assert_eq!(node.authority, AuthorityLabel::MutableProjection);
        assert_eq!(node.diagnostics[0].code, "successor_completion_failed");
        assert_eq!(
            node.diagnostics[0].evidence[0].authority,
            AuthorityLabel::TypedRecordEvidence
        );
    }

    #[test]
    fn run_forest_dto_serializes_to_json() {
        let forest = assemble_run_forest(RunForestInput {
            scheduler: scheduler(vec![node("node-0", None, NodeStatusRecord::Running)]),
            node_records: Vec::new(),
            parent_identity: None,
            successor_ready: Vec::new(),
            successor_completion: Vec::new(),
            passive_evidence: PassiveEvidence::default(),
        });

        let json = serde_json::to_value(&forest).expect("serialize forest");
        assert_eq!(json["nodes"][0]["authority"], "mutable_projection");
    }

    #[test]
    fn separate_node_records_supplement_and_override_scheduler_nodes() {
        let mut root_override = node("root", None, NodeStatusRecord::Succeeded);
        root_override.generation = 0;

        let forest = assemble_run_forest(RunForestInput {
            scheduler: scheduler(vec![node("root", None, NodeStatusRecord::Planned)]),
            node_records: vec![
                root_override,
                node("child", Some("root"), NodeStatusRecord::Running),
            ],
            parent_identity: None,
            successor_ready: Vec::new(),
            successor_completion: Vec::new(),
            passive_evidence: PassiveEvidence::default(),
        });

        assert_eq!(forest.nodes.len(), 2);
        assert_eq!(forest.node("root").progress.phase, Phase::Completed);
        assert_eq!(forest.node("root").children, vec![NodeKey::from("child")]);
        assert_eq!(forest.node("child").parent, Some(NodeKey::from("root")));
        assert!(forest.lanes.frontier.is_empty());
    }

    #[test]
    fn fs_run_store_loads_synthetic_run_read_only_counts() {
        let root = temp_run_root("synthetic");
        fs::create_dir_all(root.join("nodes").join("root")).expect("create root node dir");
        fs::create_dir_all(root.join("nodes").join("child")).expect("create child node dir");
        fs::create_dir_all(root.join("history").join("blocks")).expect("create history dir");
        fs::create_dir_all(
            root.join("nodes")
                .join("child")
                .join("channels")
                .join("runtime-1"),
        )
        .expect("create channel dir");

        write_json(
            &root.join("scheduler.json"),
            &scheduler(vec![node("root", None, NodeStatusRecord::Planned)]),
        );
        write_json(
            &root.join("nodes").join("root").join("node.json"),
            &node("root", None, NodeStatusRecord::Succeeded),
        );
        write_json(
            &root.join("nodes").join("child").join("node.json"),
            &node("child", Some("root"), NodeStatusRecord::Running),
        );
        write_json(&root.join("branches.json"), &minimal_branch_registry());
        fs::write(
            root.join("transition-journal.jsonl"),
            format!("{}\nnot-json\n", minimal_journal_line()),
        )
        .expect("write journal");
        fs::write(
            root.join("history")
                .join("blocks")
                .join("segment-000000.jsonl"),
            r#"{"state":{},"entries":[{},{}]}"#,
        )
        .expect("write history");
        fs::write(
            root.join("nodes")
                .join("child")
                .join("channels")
                .join("runtime-1")
                .join("child-to-parent.jsonl"),
            format!("{}\n", minimal_channel_line()),
        )
        .expect("write channel");
        fs::create_dir_all(root.join("evaluations")).expect("create evaluations dir");
        fs::write(
            root.join("evaluations").join("branch-synthetic-1.json"),
            r#"{
  "baseline_campaign_id": "campaign-0",
  "branch_id": "branch-synthetic-1",
  "treatment_campaign_id": "campaign-1",
  "branch_registry_path": "branches.json",
  "evaluation_artifact_path": "evaluations/branch-synthetic-1.json",
  "treatment_campaign_manifest": "nodes/child/manifest.json",
  "treatment_closure_state_path": "nodes/child/closure-state.json",
  "overall_disposition": "keep",
  "reasons": [],
  "compared_instances": [
    {
      "instance_id": "instance-1",
      "baseline_metrics": {
        "tool_calls_total": 3,
        "tool_calls_failed": 0,
        "patch_attempted": false,
        "patch_apply_state": "no",
        "submission_artifact_state": "nonempty",
        "partial_patch_failures": 0,
        "same_file_patch_retry_count": 0,
        "same_file_patch_max_streak": 0,
        "aborted": false,
        "aborted_repair_loop": false,
        "nonempty_valid_patch": true,
        "convergence": true,
        "oracle_eligible": true
      },
      "treatment_metrics": {
        "tool_calls_total": 5,
        "tool_calls_failed": 1,
        "patch_attempted": true,
        "patch_apply_state": "yes",
        "submission_artifact_state": "empty",
        "partial_patch_failures": 0,
        "same_file_patch_retry_count": 1,
        "same_file_patch_max_streak": 1,
        "aborted": true,
        "aborted_repair_loop": false,
        "nonempty_valid_patch": false,
        "convergence": false,
        "oracle_eligible": false
      },
      "evaluation": {
        "disposition": "keep",
        "reasons": []
      },
      "status": "compared"
    }
  ]
}"#,
        )
        .expect("write synthetic evaluation");

        let parent_identity_path = root.join("parent_identity.json");
        write_json(&parent_identity_path, &parent_identity("root", 0));

        let forest = FsRunStore::new(&root)
            .with_parent_identity_path(&parent_identity_path)
            .load_forest()
            .expect("load forest");

        assert_eq!(forest.nodes.len(), 2);
        assert_eq!(forest.node("root").progress.phase, Phase::Completed);
        assert!(forest.node("root").evidence.iter().any(|evidence| {
            evidence.kind == EvidenceKind::ParentIdentity
                && evidence.authority == AuthorityLabel::TypedRecordEvidence
        }));
        assert_eq!(
            forest
                .passive_evidence
                .branch_registry
                .as_ref()
                .expect("branch evidence")
                .source_node_count,
            0
        );
        assert_eq!(
            forest
                .passive_evidence
                .transition_journal
                .as_ref()
                .expect("journal evidence")
                .parsed_count,
            1
        );
        assert_eq!(
            forest
                .passive_evidence
                .transition_journal
                .as_ref()
                .expect("journal evidence")
                .parse_error_count,
            1
        );
        assert_eq!(
            forest
                .passive_evidence
                .history
                .as_ref()
                .expect("history evidence")
                .admitted_entry_count,
            2
        );
        assert_eq!(
            forest
                .passive_evidence
                .channel_envelopes
                .as_ref()
                .expect("channel evidence")
                .parsed_count,
            1
        );
        let evaluations = forest
            .passive_evidence
            .evaluations
            .as_ref()
            .expect("evaluation evidence");
        assert_eq!(evaluations.summary.file_count, 1);
        assert_eq!(evaluations.summary.parsed_count, 1);
        assert_eq!(evaluations.summary.keep_count, 1);
        assert_eq!(evaluations.summary.reject_count, 0);
        let artifact = evaluations
            .index
            .get("branch-synthetic-1")
            .expect("branch-synthetic-1");
        let compared = artifact
            .compared_instances
            .first()
            .expect("one synthetic compared instance");
        assert_eq!(
            compared
                .baseline_metrics
                .as_ref()
                .expect("baseline metrics")
                .tool_calls_total,
            3
        );
        assert!(
            compared
                .treatment_metrics
                .as_ref()
                .expect("treatment metrics")
                .aborted
        );
        assert_eq!(
            compared
                .evaluation
                .as_ref()
                .expect("evaluation")
                .disposition,
            ploke_records::branch::Disposition::Keep
        );
        assert_no_sealed_history_authority(&forest);

        fs::remove_dir_all(root).expect("remove temp run");
    }

    #[test]
    #[ignore]
    fn fs_run_store_loads_real_campaign() {
        let run_root = std::env::var("PLOKE_TREE_RUN_ROOT")
            .expect("set PLOKE_TREE_RUN_ROOT to a prototype1 run root");
        let mut store = FsRunStore::new(run_root);

        if let Ok(path) = std::env::var("PLOKE_TREE_PARENT_IDENTITY_PATH") {
            store = store.with_parent_identity_path(path);
        } else if let Ok(parent_root) = std::env::var("PLOKE_TREE_PARENT_ROOT") {
            store = store.with_parent_root(parent_root);
        }

        let forest = store.load_forest().expect("load real campaign forest");
        let max_generation = forest
            .nodes
            .iter()
            .map(|node| node.generation)
            .max()
            .unwrap_or(0);
        let journal = forest
            .passive_evidence
            .transition_journal
            .as_ref()
            .expect("journal evidence present");
        let history = forest
            .passive_evidence
            .history
            .as_ref()
            .expect("history evidence present");

        println!(
            "nodes={} max_generation={} roots={} journal_lines={} journal_parsed={} journal_errors={} history_blocks={} history_entries={} history_record_errors={} history_json_errors={} branches={:?} channels={:?} evaluations={:?}",
            forest.nodes.len(),
            max_generation,
            forest.roots.len(),
            journal.line_count,
            journal.parsed_count,
            journal.parse_error_count,
            history.sealed_block_count,
            history.admitted_entry_count,
            history.record_parse_error_count,
            history.json_parse_error_count,
            forest.passive_evidence.branch_registry,
            forest.passive_evidence.channel_envelopes,
            forest.passive_evidence.evaluations.as_ref().map(|v| &v.summary),
        );

        assert!(
            forest.nodes.len() >= 37,
            "expected real node records to be loaded"
        );
        assert!(
            max_generation >= 8,
            "expected real generation range to be loaded"
        );
        assert!(journal.line_count > 0);
        assert!(history.sealed_block_count > 0);
        assert!(history.admitted_entry_count > 0);
        let branches = forest
            .passive_evidence
            .branch_registry
            .as_ref()
            .expect("branch registry evidence present");
        assert_eq!(branches.source_node_count, 11);
        assert_eq!(branches.branch_count, 36);
        assert_eq!(branches.active_target_count, 0);
        assert!(
            forest
                .passive_evidence
                .channel_envelopes
                .as_ref()
                .expect("channel evidence present")
                .parsed_count
                > 0
        );
        let evaluations = forest
            .passive_evidence
            .evaluations
            .as_ref()
            .expect("evaluation evidence present");
        assert_eq!(evaluations.summary.file_count, 36);
        assert_eq!(evaluations.summary.parsed_count, 36);
        assert_eq!(evaluations.summary.keep_count, 20);
        assert_eq!(evaluations.summary.reject_count, 16);

        let branch_keep = evaluations
            .index
            .get("branch-116821c1239b4022")
            .expect("branch-116821c1239b4022 artifact");
        assert_eq!(branch_keep.overall_disposition, ploke_records::branch::Disposition::Keep);
        let branch_keep_compared = branch_keep
            .compared_instances
            .first()
            .expect("branch-116821 has compared instance");
        assert_eq!(
            branch_keep_compared
                .baseline_metrics
                .as_ref()
                .expect("branch-116821 baseline metrics")
                .tool_calls_total,
            12
        );
        assert_eq!(
            branch_keep_compared
                .treatment_metrics
                .as_ref()
                .expect("branch-116821 treatment metrics")
                .tool_calls_total,
            17
        );
        assert!(
            branch_keep_compared
                .treatment_metrics
                .as_ref()
                .expect("branch-116821 treatment metrics")
                .aborted
        );
        assert_eq!(
            branch_keep_compared
                .evaluation
                .as_ref()
                .expect("branch-116821 evaluation")
                .disposition,
            ploke_records::branch::Disposition::Keep
        );

        let branch_reject = evaluations
            .index
            .get("branch-01187cd17226d1a4")
            .expect("branch-01187cd17226d1a4 artifact");
        assert_eq!(
            branch_reject.overall_disposition,
            ploke_records::branch::Disposition::Reject
        );
        let branch_reject_compared = branch_reject
            .compared_instances
            .first()
            .expect("branch-01187 has compared instance");
        assert_eq!(branch_reject_compared.status, "missing_baseline_record");
        assert!(branch_reject_compared.baseline_metrics.is_none());
        assert_no_sealed_history_authority(&forest);
    }

    trait ForestTestExt {
        fn node(&self, key: &str) -> &TreeNode;
    }

    impl ForestTestExt for RunForest {
        fn node(&self, key: &str) -> &TreeNode {
            self.nodes
                .iter()
                .find(|node| node.key == NodeKey::from(key))
                .expect("node exists")
        }
    }

    fn assert_no_sealed_history_authority(forest: &RunForest) {
        assert!(
            forest
                .nodes
                .iter()
                .all(|node| node.authority != AuthorityLabel::SealedVerifiedHistory)
        );
        assert!(forest.nodes.iter().all(|node| {
            node.evidence
                .iter()
                .all(|evidence| evidence.authority != AuthorityLabel::SealedVerifiedHistory)
        }));
    }

    fn scheduler(nodes: Vec<NodeRecord>) -> SchedulerStateRecord {
        SchedulerStateRecord {
            schema_version: "prototype1-scheduler.v1".to_owned(),
            campaign_id: CampaignId("campaign-1".to_owned()),
            updated_at: "2026-05-08T12:01:00Z".to_owned(),
            policy: Default::default(),
            frontier_node_ids: nodes
                .iter()
                .filter(|node| node.status == NodeStatusRecord::Running)
                .map(|node| node.node_id.clone())
                .collect(),
            completed_node_ids: nodes
                .iter()
                .filter(|node| node.status == NodeStatusRecord::Succeeded)
                .map(|node| node.node_id.clone())
                .collect(),
            failed_node_ids: nodes
                .iter()
                .filter(|node| node.status == NodeStatusRecord::Failed)
                .map(|node| node.node_id.clone())
                .collect(),
            last_continuation_decision: None,
            nodes,
        }
    }

    fn node(id: &str, parent: Option<&str>, status: NodeStatusRecord) -> NodeRecord {
        NodeRecord {
            schema_version: "prototype1-treatment-node.v1".to_owned(),
            node_id: SchedulerNodeId(id.to_owned()),
            parent_node_id: parent.map(|id| SchedulerNodeId(id.to_owned())),
            generation: if parent.is_some() { 1 } else { 0 },
            instance_id: InstanceId(format!("instance-{id}")),
            source_state_id: SourceStateId(format!("source-{id}")),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: parent.map(|id| BranchId(format!("branch-{id}"))),
            branch_id: BranchId(format!("branch-{id}")),
            candidate_id: CandidateId(format!("candidate-{id}")),
            target_relpath: PathBuf::from("crates/ploke-core/tool_text/request_code_context.md"),
            node_dir: PathBuf::from(format!("/tmp/prototype1/nodes/{id}")),
            workspace_root: PathBuf::from(format!("/tmp/worktrees/{id}")),
            binary_path: PathBuf::from(format!("/tmp/worktrees/{id}/target/debug/ploke")),
            runner_request_path: PathBuf::from(format!(
                "/tmp/prototype1/nodes/{id}/runner-request.json"
            )),
            runner_result_path: PathBuf::from(format!(
                "/tmp/prototype1/nodes/{id}/runner-result.json"
            )),
            status,
            created_at: "2026-05-08T12:00:00Z".to_owned(),
            updated_at: "2026-05-08T12:01:00Z".to_owned(),
        }
    }

    fn parent_identity(node_id: &str, generation: u32) -> ParentIdentityRecord {
        ParentIdentityRecord {
            schema_version: "prototype1-parent-identity.v1".to_owned(),
            campaign_id: "campaign-1".to_owned(),
            parent_id: node_id.to_owned(),
            node_id: node_id.to_owned(),
            generation,
            instance_id: Some(format!("instance-{node_id}")),
            previous_parent_id: None,
            parent_node_id: None,
            branch_id: format!("branch-{node_id}"),
            artifact_branch: Some(format!("artifact-{node_id}")),
            created_at: "2026-05-08T12:00:00Z".to_owned(),
        }
    }

    fn minimal_branch_registry() -> serde_json::Value {
        serde_json::json!({
            "schema_version": "prototype1-branch-registry.v1",
            "campaign_id": "campaign-1",
            "updated_at": "2026-05-08T12:00:00Z",
            "source_nodes": [],
            "active_targets": []
        })
    }

    fn minimal_journal_line() -> String {
        serde_json::json!({
            "kind": "resource",
            "recorded_at": 1770000000000i64,
            "campaign_id": "campaign-1",
            "parent_id": "root",
            "node_id": "root",
            "generation": 0,
            "subject": "cargo_target",
            "phase": "parent_start",
            "path": "/tmp/target",
            "status": "measured",
            "bytes": 1
        })
        .to_string()
    }

    fn minimal_channel_line() -> String {
        serde_json::json!({
            "schema_version": "prototype1-runtime-channel.v1",
            "direction": "child_to_parent",
            "campaign_id": "campaign-1",
            "node_id": "child",
            "runtime_id": "11111111-1111-1111-1111-111111111111",
            "message_id": "22222222-2222-2222-2222-222222222222",
            "recorded_at": 1770000000000i64,
            "body_hash": "abc123",
            "body": "ready"
        })
        .to_string()
    }

    fn write_json(path: &Path, value: &impl Serialize) {
        let json = serde_json::to_vec_pretty(value).expect("serialize fixture");
        fs::write(path, json).expect("write fixture");
    }

    fn temp_run_root(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time after epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("ploke-tree-{name}-{}-{nanos}", std::process::id()))
    }
}
