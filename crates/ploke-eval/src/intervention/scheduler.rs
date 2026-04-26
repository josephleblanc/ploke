use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ResolvedTreatmentBranch;
use crate::loop_graph::{ArtifactId, OperationTarget, PatchId};
use crate::spec::PrepareError;

pub const PROTOTYPE1_SCHEDULER_SCHEMA_VERSION: &str = "prototype1-scheduler.v1";
pub const PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION: &str = "prototype1-treatment-node.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Prototype1SearchPolicy {
    pub max_generations: u32,
    pub max_total_nodes: u32,
    pub stop_on_first_keep: bool,
    pub require_keep_for_continuation: bool,
}

impl Default for Prototype1SearchPolicy {
    fn default() -> Self {
        Self {
            max_generations: 1,
            max_total_nodes: 32,
            stop_on_first_keep: false,
            require_keep_for_continuation: true,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1ContinuationDisposition {
    ContinueReady,
    StopMaxGenerations,
    StopMaxTotalNodes,
    StopNoSelectedBranch,
    StopOnFirstKeepSatisfied,
    StopSelectedBranchRejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[must_use = "continuation decisions must be inspected before advancing or stopping the prototype loop"]
pub struct Prototype1ContinuationDecision {
    pub disposition: Prototype1ContinuationDisposition,
    pub selected_next_branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_branch_disposition: Option<String>,
    pub next_generation: u32,
    pub total_nodes_after_continue: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1NodeStatus {
    Planned,
    WorkspaceStaged,
    BinaryBuilt,
    Running,
    Succeeded,
    Failed,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Prototype1RunnerDisposition {
    Succeeded,
    CompileFailed,
    TreatmentFailed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Prototype1NodeRecord {
    pub schema_version: String,
    pub node_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_node_id: Option<String>,
    pub generation: u32,
    pub instance_id: String,
    pub source_state_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) operation_target: Option<OperationTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) base_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) patch_id: Option<PatchId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) derived_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_branch_id: Option<String>,
    pub branch_id: String,
    pub candidate_id: String,
    pub target_relpath: PathBuf,
    pub node_dir: PathBuf,
    pub workspace_root: PathBuf,
    pub binary_path: PathBuf,
    pub runner_request_path: PathBuf,
    pub runner_result_path: PathBuf,
    pub status: Prototype1NodeStatus,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[must_use = "runner results must be checked so node failures are not silently ignored"]
pub struct Prototype1RunnerResult {
    pub schema_version: String,
    pub campaign_id: String,
    pub node_id: String,
    pub generation: u32,
    pub branch_id: String,
    pub status: Prototype1NodeStatus,
    pub disposition: Prototype1RunnerDisposition,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub treatment_campaign_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub evaluation_artifact_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stdout_excerpt: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr_excerpt: Option<String>,
    pub recorded_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Prototype1RunnerRequest {
    pub schema_version: String,
    pub campaign_id: String,
    pub node_id: String,
    pub generation: u32,
    pub instance_id: String,
    pub source_state_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) operation_target: Option<OperationTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) base_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) patch_id: Option<PatchId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) derived_artifact_id: Option<ArtifactId>,
    pub branch_id: String,
    pub target_relpath: PathBuf,
    pub workspace_root: PathBuf,
    pub binary_path: PathBuf,
    #[serde(default)]
    pub stop_on_error: bool,
    pub runner_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Prototype1SchedulerState {
    pub schema_version: String,
    pub campaign_id: String,
    pub updated_at: String,
    #[serde(default)]
    pub policy: Prototype1SearchPolicy,
    #[serde(default)]
    pub frontier_node_ids: Vec<String>,
    #[serde(default)]
    pub completed_node_ids: Vec<String>,
    #[serde(default)]
    pub failed_node_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_continuation_decision: Option<Prototype1ContinuationDecision>,
    #[serde(default)]
    pub nodes: Vec<Prototype1NodeRecord>,
}

fn sha256_hex(payload: &str) -> String {
    format!("{:x}", Sha256::digest(payload.as_bytes()))
}

fn scheduler_dir(campaign_manifest_path: &Path) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}

fn operation_target_base_artifact_id(target: &OperationTarget) -> Option<&ArtifactId> {
    match target {
        OperationTarget::Artifact { artifact_id } => Some(artifact_id),
        OperationTarget::PatchSet {
            base_artifact_id, ..
        } => Some(base_artifact_id),
        OperationTarget::ArtifactSet {
            base_artifact_id, ..
        } => base_artifact_id.as_ref(),
    }
}

pub fn prototype1_scheduler_path(campaign_manifest_path: &Path) -> PathBuf {
    scheduler_dir(campaign_manifest_path).join("scheduler.json")
}

fn prototype1_nodes_dir(campaign_manifest_path: &Path) -> PathBuf {
    scheduler_dir(campaign_manifest_path).join("nodes")
}

pub fn prototype1_node_dir(campaign_manifest_path: &Path, node_id: &str) -> PathBuf {
    prototype1_nodes_dir(campaign_manifest_path).join(node_id)
}

pub fn prototype1_node_record_path(campaign_manifest_path: &Path, node_id: &str) -> PathBuf {
    prototype1_node_dir(campaign_manifest_path, node_id).join("node.json")
}

pub fn prototype1_runner_request_path(campaign_manifest_path: &Path, node_id: &str) -> PathBuf {
    prototype1_node_dir(campaign_manifest_path, node_id).join("runner-request.json")
}

pub fn prototype1_runner_result_path(campaign_manifest_path: &Path, node_id: &str) -> PathBuf {
    prototype1_node_dir(campaign_manifest_path, node_id).join("runner-result.json")
}

pub fn prototype1_node_id(branch_id: &str, generation: u32) -> String {
    let raw = format!("{branch_id}\n{generation}");
    let digest = sha256_hex(&raw);
    format!("node-{}", &digest[..16])
}

fn default_scheduler_state(campaign_id: &str) -> Prototype1SchedulerState {
    Prototype1SchedulerState {
        schema_version: PROTOTYPE1_SCHEDULER_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.to_string(),
        updated_at: Utc::now().to_rfc3339(),
        policy: Prototype1SearchPolicy::default(),
        frontier_node_ids: Vec::new(),
        completed_node_ids: Vec::new(),
        failed_node_ids: Vec::new(),
        last_continuation_decision: None,
        nodes: Vec::new(),
    }
}

pub fn load_or_default_scheduler_state(
    campaign_id: &str,
    campaign_manifest_path: &Path,
) -> Result<Prototype1SchedulerState, PrepareError> {
    let path = prototype1_scheduler_path(campaign_manifest_path);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
            path: path.clone(),
            source,
        }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(default_scheduler_state(campaign_id))
        }
        Err(source) => Err(PrepareError::ReadManifest { path, source }),
    }
}

pub fn load_scheduler_state(
    campaign_manifest_path: &Path,
) -> Result<Prototype1SchedulerState, PrepareError> {
    let path = prototype1_scheduler_path(campaign_manifest_path);
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest { path, source })
}

fn save_scheduler_state(
    campaign_manifest_path: &Path,
    scheduler: &Prototype1SchedulerState,
) -> Result<(), PrepareError> {
    let dir = scheduler_dir(campaign_manifest_path);
    fs::create_dir_all(&dir).map_err(|source| PrepareError::WriteManifest {
        path: dir.clone(),
        source,
    })?;
    let path = prototype1_scheduler_path(campaign_manifest_path);
    let bytes = serde_json::to_vec_pretty(scheduler).map_err(PrepareError::Serialize)?;
    fs::write(&path, bytes).map_err(|source| PrepareError::WriteManifest { path, source })
}

fn save_node_record(record: &Prototype1NodeRecord) -> Result<(), PrepareError> {
    if let Some(parent) = record.node_dir.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir_all(&record.node_dir).map_err(|source| PrepareError::WriteManifest {
        path: record.node_dir.clone(),
        source,
    })?;
    let path = record
        .runner_request_path
        .parent()
        .unwrap_or(&record.node_dir);
    fs::create_dir_all(path).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let bytes = serde_json::to_vec_pretty(record).map_err(PrepareError::Serialize)?;
    let record_path = record.node_dir.join("node.json");
    fs::write(&record_path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: record_path,
        source,
    })
}

fn save_runner_request(request: &Prototype1RunnerRequest, path: &Path) -> Result<(), PrepareError> {
    if let Some(parent) = request.binary_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::create_dir_all(&request.workspace_root).map_err(|source| PrepareError::WriteManifest {
        path: request.workspace_root.clone(),
        source,
    })?;
    let bytes = serde_json::to_vec_pretty(request).map_err(PrepareError::Serialize)?;
    fs::write(path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn save_runner_result(result: &Prototype1RunnerResult, path: &Path) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(result).map_err(PrepareError::Serialize)?;
    fs::write(path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

pub fn write_runner_result_at(
    path: &Path,
    result: &Prototype1RunnerResult,
) -> Result<(), PrepareError> {
    save_runner_result(result, path)
}

pub fn load_node_record(
    campaign_manifest_path: &Path,
    node_id: &str,
) -> Result<Prototype1NodeRecord, PrepareError> {
    let path = prototype1_node_record_path(campaign_manifest_path, node_id);
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest { path, source })
}

pub fn load_runner_request(
    campaign_manifest_path: &Path,
    node_id: &str,
) -> Result<Prototype1RunnerRequest, PrepareError> {
    let path = prototype1_runner_request_path(campaign_manifest_path, node_id);
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest { path, source })
}

pub fn load_runner_result(
    campaign_manifest_path: &Path,
    node_id: &str,
) -> Result<Prototype1RunnerResult, PrepareError> {
    let path = prototype1_runner_result_path(campaign_manifest_path, node_id);
    load_runner_result_at(&path)
}

pub fn load_runner_result_at(path: &Path) -> Result<Prototype1RunnerResult, PrepareError> {
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn is_not_found(err: &PrepareError) -> bool {
    matches!(
        err,
        PrepareError::ReadManifest { source, .. }
            if source.kind() == std::io::ErrorKind::NotFound
    )
}

pub fn update_scheduler_policy(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    policy: Prototype1SearchPolicy,
) -> Result<Prototype1SchedulerState, PrepareError> {
    let mut scheduler = load_or_default_scheduler_state(campaign_id, campaign_manifest_path)?;
    scheduler.policy = policy;
    scheduler.updated_at = Utc::now().to_rfc3339();
    save_scheduler_state(campaign_manifest_path, &scheduler)?;
    Ok(scheduler)
}

pub fn update_node_status(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    node_id: &str,
    status: Prototype1NodeStatus,
) -> Result<(Prototype1SchedulerState, Prototype1NodeRecord), PrepareError> {
    let mut scheduler = load_or_default_scheduler_state(campaign_id, campaign_manifest_path)?;
    let now = Utc::now().to_rfc3339();
    let Some(node) = scheduler
        .nodes
        .iter_mut()
        .find(|node| node.node_id == node_id)
    else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!("prototype1 node '{node_id}' not found in scheduler"),
        });
    };
    node.status = status;
    node.updated_at = now.clone();
    let record = node.clone();
    save_node_record(&record)?;

    if matches!(status, Prototype1NodeStatus::Succeeded) {
        if !scheduler.completed_node_ids.iter().any(|id| id == node_id) {
            scheduler.completed_node_ids.push(node_id.to_string());
        }
        scheduler.failed_node_ids.retain(|id| id != node_id);
        scheduler.frontier_node_ids.retain(|id| id != node_id);
    } else if matches!(status, Prototype1NodeStatus::Failed) {
        if !scheduler.failed_node_ids.iter().any(|id| id == node_id) {
            scheduler.failed_node_ids.push(node_id.to_string());
        }
        scheduler.completed_node_ids.retain(|id| id != node_id);
        scheduler.frontier_node_ids.retain(|id| id != node_id);
    } else {
        if !scheduler.frontier_node_ids.iter().any(|id| id == node_id) {
            scheduler.frontier_node_ids.push(node_id.to_string());
        }
        scheduler.completed_node_ids.retain(|id| id != node_id);
        scheduler.failed_node_ids.retain(|id| id != node_id);
    }

    scheduler.updated_at = now;
    save_scheduler_state(campaign_manifest_path, &scheduler)?;
    Ok((scheduler, record))
}

pub fn update_node_workspace_root(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    node_id: &str,
    workspace_root: PathBuf,
) -> Result<
    (
        Prototype1SchedulerState,
        Prototype1NodeRecord,
        Prototype1RunnerRequest,
    ),
    PrepareError,
> {
    let mut scheduler = load_or_default_scheduler_state(campaign_id, campaign_manifest_path)?;
    let now = Utc::now().to_rfc3339();
    let Some(node) = scheduler
        .nodes
        .iter_mut()
        .find(|node| node.node_id == node_id)
    else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!("prototype1 node '{node_id}' not found in scheduler"),
        });
    };
    node.workspace_root = workspace_root.clone();
    node.updated_at = now.clone();
    let record = node.clone();
    save_node_record(&record)?;

    let mut request = load_runner_request(campaign_manifest_path, node_id)?;
    request.workspace_root = workspace_root;
    save_runner_request(&request, &record.runner_request_path)?;

    scheduler.updated_at = now;
    save_scheduler_state(campaign_manifest_path, &scheduler)?;
    Ok((scheduler, record, request))
}

pub fn record_runner_result(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    result: Prototype1RunnerResult,
) -> Result<(Prototype1SchedulerState, Prototype1NodeRecord), PrepareError> {
    let path = prototype1_runner_result_path(campaign_manifest_path, &result.node_id);
    save_runner_result(&result, &path)?;
    update_node_status(
        campaign_id,
        campaign_manifest_path,
        &result.node_id,
        result.status,
    )
}

pub fn clear_runner_result(
    campaign_manifest_path: &Path,
    node_id: &str,
) -> Result<bool, PrepareError> {
    let path = prototype1_runner_result_path(campaign_manifest_path, node_id);
    match fs::remove_file(&path) {
        Ok(()) => Ok(true),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(source) => Err(PrepareError::WriteManifest { path, source }),
    }
}

pub fn decide_continuation(
    scheduler: &Prototype1SchedulerState,
    current_generation: u32,
    selected_next_branch_id: Option<&str>,
    selected_branch_disposition: Option<&str>,
) -> Prototype1ContinuationDecision {
    let next_generation = current_generation.saturating_add(1);
    let total_nodes_after_continue = scheduler.nodes.len() as u32;

    let disposition = if selected_next_branch_id.is_none() {
        Prototype1ContinuationDisposition::StopNoSelectedBranch
    } else if scheduler.policy.require_keep_for_continuation
        && selected_branch_disposition.is_some_and(|value| value != "keep")
    {
        Prototype1ContinuationDisposition::StopSelectedBranchRejected
    } else if scheduler.policy.stop_on_first_keep
        && selected_branch_disposition.is_some_and(|value| value == "keep")
    {
        Prototype1ContinuationDisposition::StopOnFirstKeepSatisfied
    } else if next_generation > scheduler.policy.max_generations {
        Prototype1ContinuationDisposition::StopMaxGenerations
    } else if total_nodes_after_continue >= scheduler.policy.max_total_nodes {
        Prototype1ContinuationDisposition::StopMaxTotalNodes
    } else {
        Prototype1ContinuationDisposition::ContinueReady
    };

    Prototype1ContinuationDecision {
        disposition,
        selected_next_branch_id: selected_next_branch_id.map(ToOwned::to_owned),
        selected_branch_disposition: selected_branch_disposition.map(ToOwned::to_owned),
        next_generation,
        total_nodes_after_continue,
    }
}

pub fn decide_node_successor_continuation(
    scheduler: &Prototype1SchedulerState,
    node: &Prototype1NodeRecord,
    selected_branch_disposition: Option<&str>,
) -> Prototype1ContinuationDecision {
    decide_continuation(
        scheduler,
        node.generation,
        Some(&node.branch_id),
        selected_branch_disposition,
    )
}

pub fn record_continuation_decision(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    decision: Prototype1ContinuationDecision,
) -> Result<Prototype1SchedulerState, PrepareError> {
    let mut scheduler = load_or_default_scheduler_state(campaign_id, campaign_manifest_path)?;
    scheduler.last_continuation_decision = Some(decision);
    scheduler.updated_at = Utc::now().to_rfc3339();
    save_scheduler_state(campaign_manifest_path, &scheduler)?;
    Ok(scheduler)
}

pub fn register_treatment_evaluation_node(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    branch: &ResolvedTreatmentBranch,
    generation: u32,
    repo_root: &Path,
    stop_on_error: bool,
) -> Result<
    (
        Prototype1SchedulerState,
        Prototype1NodeRecord,
        Prototype1RunnerRequest,
    ),
    PrepareError,
> {
    let mut scheduler = load_or_default_scheduler_state(campaign_id, campaign_manifest_path)?;
    let node_id = prototype1_node_id(&branch.branch.branch_id, generation);
    let node_dir = prototype1_node_dir(campaign_manifest_path, &node_id);
    let workspace_root = repo_root.to_path_buf();
    let binary_path = node_dir.join("bin/ploke-eval");
    let runner_request_path = prototype1_runner_request_path(campaign_manifest_path, &node_id);
    let runner_result_path = prototype1_runner_result_path(campaign_manifest_path, &node_id);
    let parent_node_id = branch
        .parent_branch_id
        .as_deref()
        .and_then(|parent_branch_id| {
            generation
                .checked_sub(1)
                .map(|g| prototype1_node_id(parent_branch_id, g))
        });
    let now = Utc::now().to_rfc3339();

    // Only copy graph provenance already carried by the branch registry. The
    // scheduler is not the authority that decides what counts as a durable
    // Artifact, so it must not mint ArtifactIds from branch names,
    // source_state_id, or dirty worktree state. As runtime coordinates become
    // available upstream, this is the entry point that should preserve them on
    // node and runner-request records.
    let operation_target = branch.branch.generation_target.clone().or_else(|| {
        branch
            .branch
            .generation_coordinate
            .as_ref()
            .map(|coordinate| coordinate.target.clone())
    });
    let base_artifact_id = operation_target
        .as_ref()
        .and_then(operation_target_base_artifact_id)
        .cloned();
    let patch_id = branch.branch.patch_id.clone();
    let derived_artifact_id = branch.branch.derived_artifact_id.clone();

    let record = Prototype1NodeRecord {
        schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        node_id: node_id.clone(),
        parent_node_id,
        generation,
        instance_id: branch.instance_id.clone(),
        source_state_id: branch.source_state_id.clone(),
        operation_target: operation_target.clone(),
        base_artifact_id: base_artifact_id.clone(),
        patch_id: patch_id.clone(),
        derived_artifact_id: derived_artifact_id.clone(),
        parent_branch_id: branch.parent_branch_id.clone(),
        branch_id: branch.branch.branch_id.clone(),
        candidate_id: branch.branch.candidate_id.clone(),
        target_relpath: branch.target_relpath.clone(),
        node_dir: node_dir.clone(),
        workspace_root: workspace_root.clone(),
        binary_path: binary_path.clone(),
        runner_request_path: runner_request_path.clone(),
        runner_result_path: runner_result_path.clone(),
        status: scheduler
            .nodes
            .iter()
            .find(|node| node.node_id == node_id)
            .map(|node| node.status)
            .unwrap_or(Prototype1NodeStatus::Planned),
        created_at: scheduler
            .nodes
            .iter()
            .find(|node| node.node_id == node_id)
            .map(|node| node.created_at.clone())
            .unwrap_or_else(|| now.clone()),
        updated_at: now.clone(),
    };

    let request = Prototype1RunnerRequest {
        schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.to_string(),
        node_id: node_id.clone(),
        generation,
        instance_id: branch.instance_id.clone(),
        source_state_id: branch.source_state_id.clone(),
        operation_target,
        base_artifact_id,
        patch_id,
        derived_artifact_id,
        branch_id: branch.branch.branch_id.clone(),
        target_relpath: branch.target_relpath.clone(),
        workspace_root,
        binary_path,
        stop_on_error,
        runner_args: vec![
            "loop".to_string(),
            "prototype1-runner".to_string(),
            "--campaign".to_string(),
            campaign_id.to_string(),
            "--node-id".to_string(),
            node_id.clone(),
            "--execute".to_string(),
            "--stop-on-error".to_string(),
            stop_on_error.to_string(),
            "--format".to_string(),
            "json".to_string(),
        ],
    };

    fs::create_dir_all(node_dir.join("bin")).map_err(|source| PrepareError::WriteManifest {
        path: node_dir.join("bin"),
        source,
    })?;
    save_node_record(&record)?;
    save_runner_request(&request, &runner_request_path)?;

    match scheduler
        .nodes
        .iter_mut()
        .find(|node| node.node_id == node_id)
    {
        Some(existing) => *existing = record.clone(),
        None => scheduler.nodes.push(record.clone()),
    }
    if !scheduler.frontier_node_ids.iter().any(|id| id == &node_id) {
        scheduler.frontier_node_ids.push(node_id.clone());
    }
    scheduler.completed_node_ids.retain(|id| id != &node_id);
    scheduler.failed_node_ids.retain(|id| id != &node_id);
    scheduler.last_continuation_decision = None;
    scheduler.updated_at = now;
    save_scheduler_state(campaign_manifest_path, &scheduler)?;

    Ok((scheduler, record, request))
}

pub fn load_or_register_treatment_evaluation_node(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    branch: &ResolvedTreatmentBranch,
    generation: u32,
    repo_root: &Path,
    stop_on_error: bool,
) -> Result<
    (
        Prototype1SchedulerState,
        Prototype1NodeRecord,
        Prototype1RunnerRequest,
    ),
    PrepareError,
> {
    let node_id = prototype1_node_id(&branch.branch.branch_id, generation);
    let mut scheduler = load_or_default_scheduler_state(campaign_id, campaign_manifest_path)?;

    let node = match load_node_record(campaign_manifest_path, &node_id) {
        Ok(node) => node,
        Err(err) if is_not_found(&err) => {
            return register_treatment_evaluation_node(
                campaign_id,
                campaign_manifest_path,
                branch,
                generation,
                repo_root,
                stop_on_error,
            );
        }
        Err(err) => return Err(err),
    };
    let request = match load_runner_request(campaign_manifest_path, &node_id) {
        Ok(request) => request,
        Err(err) if is_not_found(&err) => {
            return register_treatment_evaluation_node(
                campaign_id,
                campaign_manifest_path,
                branch,
                generation,
                repo_root,
                stop_on_error,
            );
        }
        Err(err) => return Err(err),
    };

    if node.branch_id != branch.branch.branch_id
        || node.generation != generation
        || node.instance_id != branch.instance_id
        || node.source_state_id != branch.source_state_id
        || node.target_relpath != branch.target_relpath
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "existing prototype1 node '{}' does not match requested branch/generation identity",
                node_id
            ),
        });
    }
    if request.node_id != node.node_id
        || request.branch_id != node.branch_id
        || request.generation != node.generation
        || request.instance_id != node.instance_id
        || request.source_state_id != node.source_state_id
        || request.target_relpath != node.target_relpath
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "existing prototype1 runner request for node '{}' does not match persisted node identity",
                node_id
            ),
        });
    }

    match scheduler
        .nodes
        .iter_mut()
        .find(|existing| existing.node_id == node_id)
    {
        Some(existing) => *existing = node.clone(),
        None => scheduler.nodes.push(node.clone()),
    }
    if !scheduler.frontier_node_ids.iter().any(|id| id == &node_id)
        && !scheduler.completed_node_ids.iter().any(|id| id == &node_id)
        && !scheduler.failed_node_ids.iter().any(|id| id == &node_id)
    {
        scheduler.frontier_node_ids.push(node_id);
    }
    scheduler.updated_at = Utc::now().to_rfc3339();
    save_scheduler_state(campaign_manifest_path, &scheduler)?;

    Ok((scheduler, node, request))
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::super::{TreatmentBranchNode, TreatmentBranchStatus};
    use super::*;

    fn campaign_manifest_path(tmp: &Path) -> PathBuf {
        let campaign_dir = tmp.join("campaigns/test-campaign");
        fs::create_dir_all(&campaign_dir).expect("campaign dir");
        campaign_dir.join("campaign.json")
    }

    fn resolved_branch() -> ResolvedTreatmentBranch {
        ResolvedTreatmentBranch {
            instance_id: "clap-rs__clap-3670".to_string(),
            source_state_id: "source-1".to_string(),
            parent_branch_id: Some("branch-parent".to_string()),
            target_relpath: PathBuf::from("crates/ploke-core/tool_text/request_code_context.md"),
            source_content: "old text\n".to_string(),
            source_content_hash: "old-hash".to_string(),
            selected_branch_id: Some("branch-selected".to_string()),
            branch: TreatmentBranchNode {
                branch_id: "branch-123".to_string(),
                candidate_id: "candidate-1".to_string(),
                patch_id: None,
                branch_label: "minimal_rewrite".to_string(),
                synthesized_spec_id: "spec-1".to_string(),
                proposed_content: "new text\n".to_string(),
                proposed_content_hash: "new-hash".to_string(),
                generation_target: None,
                generation_coordinate: None,
                status: TreatmentBranchStatus::Synthesized,
                apply_id: None,
                applied_content_hash: None,
                derived_artifact_id: None,
                latest_evaluation: None,
            },
        }
    }

    fn resolved_branch_with_graph() -> ResolvedTreatmentBranch {
        let mut resolved = resolved_branch();
        let source_artifact = ArtifactId::new("git-tree:source");
        resolved.branch.patch_id = Some(PatchId::new("patch:attempt-1"));
        resolved.branch.generation_target = Some(OperationTarget::Artifact {
            artifact_id: source_artifact,
        });
        resolved.branch.derived_artifact_id = Some(ArtifactId::new("git-commit:derived"));
        resolved
    }

    #[test]
    fn register_treatment_node_persists_scheduler_and_runner_request() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());

        let (scheduler, node, request) = register_treatment_evaluation_node(
            "test-campaign",
            &manifest,
            &resolved_branch(),
            2,
            tmp.path(),
            false,
        )
        .expect("register node");

        assert_eq!(scheduler.nodes.len(), 1);
        assert_eq!(scheduler.policy, Prototype1SearchPolicy::default());
        assert_eq!(scheduler.frontier_node_ids, vec![node.node_id.clone()]);
        assert_eq!(node.status, Prototype1NodeStatus::Planned);
        let expected_parent = prototype1_node_id("branch-parent", 1);
        assert_eq!(
            node.parent_node_id.as_deref(),
            Some(expected_parent.as_str())
        );
        assert_eq!(node.workspace_root, tmp.path());
        assert!(node.binary_path.parent().expect("bin parent").exists());
        assert_eq!(request.runner_args[0], "loop");
        assert_eq!(request.runner_args[1], "prototype1-runner");

        let loaded_scheduler =
            load_or_default_scheduler_state("test-campaign", &manifest).expect("load scheduler");
        let loaded_node = load_node_record(&manifest, &node.node_id).expect("load node");
        let loaded_request =
            load_runner_request(&manifest, &node.node_id).expect("load runner request");

        assert_eq!(loaded_scheduler.nodes[0].node_id, node.node_id);
        assert_eq!(loaded_node.branch_id, "branch-123");
        assert_eq!(loaded_request.node_id, node.node_id);
        assert_eq!(loaded_request.binary_path, node.binary_path);
    }

    #[test]
    fn register_treatment_node_propagates_visible_graph_provenance() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());

        let (_scheduler, node, request) = register_treatment_evaluation_node(
            "test-campaign",
            &manifest,
            &resolved_branch_with_graph(),
            2,
            tmp.path(),
            false,
        )
        .expect("register node");

        assert_eq!(node.operation_target, request.operation_target);
        assert_eq!(node.base_artifact_id, request.base_artifact_id);
        assert_eq!(node.patch_id, request.patch_id);
        assert_eq!(node.derived_artifact_id, request.derived_artifact_id);
        assert_eq!(
            node.base_artifact_id,
            Some(ArtifactId::new("git-tree:source"))
        );
        assert_eq!(node.patch_id, Some(PatchId::new("patch:attempt-1")));
        assert_eq!(
            node.derived_artifact_id,
            Some(ArtifactId::new("git-commit:derived"))
        );
    }

    #[test]
    fn graph_provenance_fields_are_backward_compatible_json() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());

        let (_scheduler, node, request) = register_treatment_evaluation_node(
            "test-campaign",
            &manifest,
            &resolved_branch(),
            2,
            tmp.path(),
            false,
        )
        .expect("register node");

        let node_json = serde_json::to_value(&node).expect("serialize node");
        assert!(node_json.get("operation_target").is_none());
        assert!(node_json.get("base_artifact_id").is_none());
        assert!(node_json.get("patch_id").is_none());
        assert!(node_json.get("derived_artifact_id").is_none());
        let node_round_trip: Prototype1NodeRecord =
            serde_json::from_value(node_json).expect("deserialize node without provenance");
        assert_eq!(node_round_trip.operation_target, None);
        assert_eq!(node_round_trip.base_artifact_id, None);
        assert_eq!(node_round_trip.patch_id, None);
        assert_eq!(node_round_trip.derived_artifact_id, None);

        let request_json = serde_json::to_value(&request).expect("serialize request");
        assert!(request_json.get("operation_target").is_none());
        assert!(request_json.get("base_artifact_id").is_none());
        assert!(request_json.get("patch_id").is_none());
        assert!(request_json.get("derived_artifact_id").is_none());
        let request_round_trip: Prototype1RunnerRequest =
            serde_json::from_value(request_json).expect("deserialize request without provenance");
        assert_eq!(request_round_trip.operation_target, None);
        assert_eq!(request_round_trip.base_artifact_id, None);
        assert_eq!(request_round_trip.patch_id, None);
        assert_eq!(request_round_trip.derived_artifact_id, None);
    }

    #[test]
    fn graph_provenance_fields_serialize_when_present() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());

        let (_scheduler, mut node, mut request) = register_treatment_evaluation_node(
            "test-campaign",
            &manifest,
            &resolved_branch(),
            2,
            tmp.path(),
            false,
        )
        .expect("register node");

        let source_artifact = ArtifactId::new("git-tree:source");
        let patch = PatchId::new("patch:attempt-1");
        let derived_artifact = ArtifactId::new("git-commit:derived");
        node.operation_target = Some(OperationTarget::Artifact {
            artifact_id: source_artifact.clone(),
        });
        node.base_artifact_id = Some(source_artifact.clone());
        node.patch_id = Some(patch.clone());
        node.derived_artifact_id = Some(derived_artifact.clone());
        request.operation_target = node.operation_target.clone();
        request.base_artifact_id = Some(source_artifact);
        request.patch_id = Some(patch);
        request.derived_artifact_id = Some(derived_artifact);

        let node_json = serde_json::to_value(&node).expect("serialize node");
        assert_eq!(
            node_json["operation_target"],
            serde_json::json!({
                "kind": "artifact",
                "artifact_id": "git-tree:source",
            })
        );
        assert_eq!(node_json["base_artifact_id"], "git-tree:source");
        assert_eq!(node_json["patch_id"], "patch:attempt-1");
        assert_eq!(node_json["derived_artifact_id"], "git-commit:derived");

        let request_json = serde_json::to_value(&request).expect("serialize request");
        assert_eq!(request_json["base_artifact_id"], "git-tree:source");
        assert_eq!(request_json["patch_id"], "patch:attempt-1");
        assert_eq!(request_json["derived_artifact_id"], "git-commit:derived");
        assert_eq!(
            request_json["operation_target"],
            node_json["operation_target"]
        );
    }

    #[test]
    fn continuation_decision_stops_on_generation_limit_and_rejects_non_keep() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());
        let policy = Prototype1SearchPolicy {
            max_generations: 2,
            max_total_nodes: 8,
            stop_on_first_keep: false,
            require_keep_for_continuation: true,
        };
        let scheduler = update_scheduler_policy("test-campaign", &manifest, policy.clone())
            .expect("persist policy");

        let reject = decide_continuation(&scheduler, 1, Some("branch-1"), Some("reject"));
        assert_eq!(
            reject.disposition,
            Prototype1ContinuationDisposition::StopSelectedBranchRejected
        );

        let generation_limit = decide_continuation(&scheduler, 2, Some("branch-1"), Some("keep"));
        assert_eq!(
            generation_limit.disposition,
            Prototype1ContinuationDisposition::StopMaxGenerations
        );

        let continue_ready = decide_continuation(&scheduler, 1, Some("branch-1"), Some("keep"));
        assert_eq!(
            continue_ready.disposition,
            Prototype1ContinuationDisposition::ContinueReady
        );
        assert_eq!(continue_ready.next_generation, 2);
        assert_eq!(
            continue_ready.selected_next_branch_id.as_deref(),
            Some("branch-1")
        );
        assert_eq!(
            continue_ready.selected_branch_disposition.as_deref(),
            Some("keep")
        );

        let stop_on_first_keep_scheduler = update_scheduler_policy(
            "test-campaign",
            &manifest,
            Prototype1SearchPolicy {
                stop_on_first_keep: true,
                ..policy
            },
        )
        .expect("persist stop-on-first-keep policy");
        let stop_on_keep = decide_continuation(
            &stop_on_first_keep_scheduler,
            1,
            Some("branch-1"),
            Some("keep"),
        );
        assert_eq!(
            stop_on_keep.disposition,
            Prototype1ContinuationDisposition::StopOnFirstKeepSatisfied
        );
    }

    #[test]
    fn load_or_register_reuses_existing_node_request() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());
        let resolved = resolved_branch();

        let (_scheduler, node, request) = register_treatment_evaluation_node(
            "test-campaign",
            &manifest,
            &resolved,
            2,
            tmp.path(),
            false,
        )
        .expect("register node");

        let (loaded_scheduler, loaded_node, loaded_request) =
            load_or_register_treatment_evaluation_node(
                "test-campaign",
                &manifest,
                &resolved,
                2,
                tmp.path(),
                true,
            )
            .expect("load existing node");

        assert_eq!(loaded_scheduler.nodes.len(), 1);
        assert_eq!(loaded_node, node);
        assert_eq!(loaded_request, request);
    }
}
