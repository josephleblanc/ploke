use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::ResolvedTreatmentBranch;
use crate::spec::PrepareError;

pub const PROTOTYPE1_SCHEDULER_SCHEMA_VERSION: &str = "prototype1-scheduler.v1";
pub const PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION: &str = "prototype1-treatment-node.v1";

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
pub struct Prototype1RunnerRequest {
    pub schema_version: String,
    pub campaign_id: String,
    pub node_id: String,
    pub generation: u32,
    pub instance_id: String,
    pub source_state_id: String,
    pub branch_id: String,
    pub target_relpath: PathBuf,
    pub workspace_root: PathBuf,
    pub binary_path: PathBuf,
    pub runner_args: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Prototype1SchedulerState {
    pub schema_version: String,
    pub campaign_id: String,
    pub updated_at: String,
    #[serde(default)]
    pub frontier_node_ids: Vec<String>,
    #[serde(default)]
    pub completed_node_ids: Vec<String>,
    #[serde(default)]
    pub failed_node_ids: Vec<String>,
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
        frontier_node_ids: Vec::new(),
        completed_node_ids: Vec::new(),
        failed_node_ids: Vec::new(),
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

fn save_runner_request(request: &Prototype1RunnerRequest) -> Result<(), PrepareError> {
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
    let path = request
        .workspace_root
        .parent()
        .unwrap_or(&request.workspace_root)
        .join("runner-request.json");
    let bytes = serde_json::to_vec_pretty(request).map_err(PrepareError::Serialize)?;
    fs::write(&path, bytes).map_err(|source| PrepareError::WriteManifest { path, source })
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

pub fn register_treatment_evaluation_node(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    branch: &ResolvedTreatmentBranch,
    generation: u32,
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
    let workspace_root = node_dir.join("workspace");
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

    let record = Prototype1NodeRecord {
        schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
        node_id: node_id.clone(),
        parent_node_id,
        generation,
        instance_id: branch.instance_id.clone(),
        source_state_id: branch.source_state_id.clone(),
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
        branch_id: branch.branch.branch_id.clone(),
        target_relpath: branch.target_relpath.clone(),
        workspace_root,
        binary_path,
        runner_args: vec![
            "loop".to_string(),
            "prototype1-runner".to_string(),
            "--campaign".to_string(),
            campaign_id.to_string(),
            "--node-id".to_string(),
            node_id.clone(),
            "--format".to_string(),
            "json".to_string(),
        ],
    };

    fs::create_dir_all(node_dir.join("bin")).map_err(|source| PrepareError::WriteManifest {
        path: node_dir.join("bin"),
        source,
    })?;
    fs::create_dir_all(node_dir.join("workspace")).map_err(|source| {
        PrepareError::WriteManifest {
            path: node_dir.join("workspace"),
            source,
        }
    })?;
    save_node_record(&record)?;
    save_runner_request(&request)?;

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
    scheduler.updated_at = now;
    save_scheduler_state(campaign_manifest_path, &scheduler)?;

    Ok((scheduler, record, request))
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
                branch_label: "minimal_rewrite".to_string(),
                synthesized_spec_id: "spec-1".to_string(),
                proposed_content: "new text\n".to_string(),
                proposed_content_hash: "new-hash".to_string(),
                status: TreatmentBranchStatus::Synthesized,
                apply_id: None,
                applied_content_hash: None,
                latest_evaluation: None,
            },
        }
    }

    #[test]
    fn register_treatment_node_persists_scheduler_and_runner_request() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());

        let (scheduler, node, request) =
            register_treatment_evaluation_node("test-campaign", &manifest, &resolved_branch(), 2)
                .expect("register node");

        assert_eq!(scheduler.nodes.len(), 1);
        assert_eq!(scheduler.frontier_node_ids, vec![node.node_id.clone()]);
        assert_eq!(node.status, Prototype1NodeStatus::Planned);
        let expected_parent = prototype1_node_id("branch-parent", 1);
        assert_eq!(
            node.parent_node_id.as_deref(),
            Some(expected_parent.as_str())
        );
        assert!(node.workspace_root.exists());
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
}
