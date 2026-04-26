use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::spec::{
    InterventionApplyOutput, InterventionSynthesisOutput, operation_target_artifact_id,
    text_file_artifact_id, text_replacement_patch_id,
};
use crate::branch_evaluation::BranchDisposition;
use crate::loop_graph::{ArtifactId, Coordinate, OperationTarget, PatchId};
use crate::spec::PrepareError;

pub const PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION: &str = "prototype1-branch-registry.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TreatmentBranchStatus {
    Synthesized,
    Selected,
    Applied,
    Restored,
    Dropped,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreatmentBranchEvaluationSummary {
    pub baseline_campaign_id: String,
    pub treatment_campaign_id: String,
    pub compared_instances: usize,
    pub rejected_instances: usize,
    pub overall_disposition: BranchDisposition,
    pub evaluated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreatmentBranchNode {
    pub branch_id: String,
    pub candidate_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) patch_id: Option<PatchId>,
    pub branch_label: String,
    pub synthesized_spec_id: String,
    pub proposed_content: String,
    pub proposed_content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) generation_target: Option<OperationTarget>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) generation_coordinate: Option<Coordinate>,
    pub status: TreatmentBranchStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) derived_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_evaluation: Option<TreatmentBranchEvaluationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionSourceNode {
    pub source_state_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) operation_target: Option<OperationTarget>,
    pub instance_id: String,
    pub target_relpath: PathBuf,
    pub source_content: String,
    pub source_content_hash: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub selected_branch_id: Option<String>,
    pub branches: Vec<TreatmentBranchNode>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveInterventionTarget {
    pub target_relpath: PathBuf,
    pub source_state_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) active_patch_id: Option<PatchId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_apply_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) active_derived_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) active_operation_target: Option<OperationTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Prototype1BranchRegistry {
    pub schema_version: String,
    pub campaign_id: String,
    pub updated_at: String,
    #[serde(default)]
    pub source_nodes: Vec<InterventionSourceNode>,
    #[serde(default)]
    pub active_targets: Vec<ActiveInterventionTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionRestoreOutput {
    pub branch_id: String,
    pub source_state_id: String,
    pub target_relpath: PathBuf,
    pub restored_content_hash: String,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ActiveBranchSelection {
    pub target_relpath: PathBuf,
    pub source_state_id: String,
    pub branch: TreatmentBranchNode,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResolvedTreatmentBranch {
    pub instance_id: String,
    pub source_state_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_branch_id: Option<String>,
    pub target_relpath: PathBuf,
    pub source_content: String,
    pub source_content_hash: String,
    pub selected_branch_id: Option<String>,
    pub branch: TreatmentBranchNode,
}

fn sha256_hex(payload: &str) -> String {
    format!("{:x}", Sha256::digest(payload.as_bytes()))
}

fn operation_target_for_artifact(artifact_id: &ArtifactId) -> OperationTarget {
    OperationTarget::Artifact {
        artifact_id: artifact_id.clone(),
    }
}

fn branch_registry_dir(campaign_manifest_path: &Path) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}

pub fn prototype1_branch_registry_path(campaign_manifest_path: &Path) -> PathBuf {
    branch_registry_dir(campaign_manifest_path).join("branches.json")
}

pub fn treatment_branch_id(
    source_state_id: &str,
    target_relpath: &Path,
    candidate_id: &str,
) -> String {
    let raw = format!(
        "{}\n{}\n{}",
        source_state_id,
        target_relpath.display(),
        candidate_id
    );
    let digest = sha256_hex(&raw);
    format!("branch-{}", &digest[..16])
}

fn default_registry(campaign_id: &str) -> Prototype1BranchRegistry {
    Prototype1BranchRegistry {
        schema_version: PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION.to_string(),
        campaign_id: campaign_id.to_string(),
        updated_at: Utc::now().to_rfc3339(),
        source_nodes: Vec::new(),
        active_targets: Vec::new(),
    }
}

pub fn load_or_default_branch_registry(
    campaign_id: &str,
    campaign_manifest_path: &Path,
) -> Result<Prototype1BranchRegistry, PrepareError> {
    let path = prototype1_branch_registry_path(campaign_manifest_path);
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
            path: path.clone(),
            source,
        }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            Ok(default_registry(campaign_id))
        }
        Err(source) => Err(PrepareError::ReadManifest { path, source }),
    }
}

pub fn save_branch_registry(
    campaign_manifest_path: &Path,
    registry: &Prototype1BranchRegistry,
) -> Result<(), PrepareError> {
    let dir = branch_registry_dir(campaign_manifest_path);
    fs::create_dir_all(&dir).map_err(|source| PrepareError::WriteManifest {
        path: dir.clone(),
        source,
    })?;
    let path = prototype1_branch_registry_path(campaign_manifest_path);
    let json = serde_json::to_string_pretty(registry).map_err(PrepareError::Serialize)?;
    fs::write(&path, json).map_err(|source| PrepareError::WriteManifest { path, source })
}

pub fn record_synthesized_branches(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    instance_id: &str,
    synthesis: &InterventionSynthesisOutput,
    selected_candidate_id: Option<&str>,
    parent_branch_id: Option<&str>,
) -> Result<Prototype1BranchRegistry, PrepareError> {
    let mut registry = load_or_default_branch_registry(campaign_id, campaign_manifest_path)?;
    let source_content_hash = sha256_hex(&synthesis.candidate_set.source_content);
    // Preserve any real target provenance already supplied by synthesis.
    // Otherwise fall back to the current text-file surface identity. That
    // fallback is intentionally narrower than a worktree ArtifactId; see
    // `text_file_artifact_id` for the boundary.
    let source_artifact_id = synthesis
        .candidate_set
        .operation_target
        .as_ref()
        .and_then(operation_target_artifact_id)
        .cloned()
        .unwrap_or_else(|| {
            text_file_artifact_id(
                &synthesis.candidate_set.target_relpath,
                &synthesis.candidate_set.source_content,
            )
        });
    let operation_target = synthesis
        .candidate_set
        .operation_target
        .clone()
        .unwrap_or_else(|| operation_target_for_artifact(&source_artifact_id));
    let selected_branch_id = selected_candidate_id.map(|candidate_id| {
        treatment_branch_id(
            &synthesis.candidate_set.source_state_id,
            &synthesis.candidate_set.target_relpath,
            candidate_id,
        )
    });

    let source_node = if let Some(node) = registry.source_nodes.iter_mut().find(|node| {
        node.source_state_id == synthesis.candidate_set.source_state_id
            && node.target_relpath == synthesis.candidate_set.target_relpath
    }) {
        node
    } else {
        registry.source_nodes.push(InterventionSourceNode {
            source_state_id: synthesis.candidate_set.source_state_id.clone(),
            parent_branch_id: parent_branch_id.map(ToOwned::to_owned),
            source_artifact_id: Some(source_artifact_id.clone()),
            operation_target: Some(operation_target.clone()),
            instance_id: instance_id.to_string(),
            target_relpath: synthesis.candidate_set.target_relpath.clone(),
            source_content: synthesis.candidate_set.source_content.clone(),
            source_content_hash: source_content_hash.clone(),
            selected_branch_id: None,
            branches: Vec::new(),
        });
        registry
            .source_nodes
            .last_mut()
            .expect("newly pushed source node")
    };

    source_node.instance_id = instance_id.to_string();
    if let Some(parent_branch_id) = parent_branch_id {
        source_node.parent_branch_id = Some(parent_branch_id.to_string());
    }
    source_node.source_content = synthesis.candidate_set.source_content.clone();
    source_node.source_content_hash = source_content_hash.clone();
    source_node.source_artifact_id = Some(source_artifact_id.clone());
    source_node.operation_target = Some(operation_target.clone());
    source_node.selected_branch_id = selected_branch_id.clone();

    for candidate in &synthesis.candidate_set.candidates {
        let branch_id = treatment_branch_id(
            &synthesis.candidate_set.source_state_id,
            &synthesis.candidate_set.target_relpath,
            &candidate.candidate_id,
        );
        let proposed_content_hash = sha256_hex(&candidate.proposed_content);
        // Candidate patch ids are the first durable handle for probabilistic
        // patch generation. Legacy candidate ids and branch ids remain display
        // and registry handles, not patch provenance.
        let patch_id = candidate.patch_id.clone().unwrap_or_else(|| {
            text_replacement_patch_id(
                &synthesis.candidate_set.target_relpath,
                &synthesis.candidate_set.source_content,
                &candidate.proposed_content,
            )
        });
        match source_node
            .branches
            .iter_mut()
            .find(|branch| branch.branch_id == branch_id)
        {
            Some(branch) => {
                branch.patch_id = Some(patch_id);
                branch.branch_label = candidate.branch_label.clone();
                branch.synthesized_spec_id = candidate.spec.spec_id().to_string();
                branch.proposed_content = candidate.proposed_content.clone();
                branch.proposed_content_hash = proposed_content_hash;
                branch.generation_target = Some(operation_target.clone());
                if selected_candidate_id == Some(candidate.candidate_id.as_str()) {
                    branch.status = TreatmentBranchStatus::Selected;
                }
            }
            None => {
                source_node.branches.push(TreatmentBranchNode {
                    branch_id,
                    candidate_id: candidate.candidate_id.clone(),
                    patch_id: Some(patch_id),
                    branch_label: candidate.branch_label.clone(),
                    synthesized_spec_id: candidate.spec.spec_id().to_string(),
                    proposed_content: candidate.proposed_content.clone(),
                    proposed_content_hash,
                    generation_target: Some(operation_target.clone()),
                    generation_coordinate: None,
                    status: if selected_candidate_id == Some(candidate.candidate_id.as_str()) {
                        TreatmentBranchStatus::Selected
                    } else {
                        TreatmentBranchStatus::Synthesized
                    },
                    apply_id: None,
                    applied_content_hash: None,
                    derived_artifact_id: None,
                    latest_evaluation: None,
                });
            }
        }
    }

    registry.updated_at = Utc::now().to_rfc3339();
    save_branch_registry(campaign_manifest_path, &registry)?;
    Ok(registry)
}

pub fn mark_treatment_branch_applied(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    target_relpath: &Path,
    apply: &InterventionApplyOutput,
) -> Result<Prototype1BranchRegistry, PrepareError> {
    let mut registry = load_or_default_branch_registry(campaign_id, campaign_manifest_path)?;
    let source_state_id = &apply.treatment_state.source_state_id;

    let source_node = registry
        .source_nodes
        .iter_mut()
        .find(|node| {
            node.source_state_id == *source_state_id && node.target_relpath == target_relpath
        })
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "missing source node for apply source_state_id='{}' target='{}'",
                source_state_id,
                target_relpath.display()
            ),
        })?;

    let source_artifact_id = apply
        .base_artifact_id
        .clone()
        .or_else(|| source_node.source_artifact_id.clone())
        .unwrap_or_else(|| text_file_artifact_id(target_relpath, &source_node.source_content));
    let operation_target = source_node
        .operation_target
        .clone()
        .unwrap_or_else(|| operation_target_for_artifact(&source_artifact_id));
    source_node.source_artifact_id = Some(source_artifact_id.clone());
    source_node.operation_target = Some(operation_target.clone());

    let branch_id = treatment_branch_id(source_state_id, target_relpath, &apply.candidate_id);
    let branch = source_node
        .branches
        .iter_mut()
        .find(|branch| branch.branch_id == branch_id)
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "missing branch '{}' for apply candidate '{}'",
                branch_id, apply.candidate_id
            ),
        })?;
    let patch_id = apply
        .patch_id
        .clone()
        .or_else(|| branch.patch_id.clone())
        .unwrap_or_else(|| {
            text_replacement_patch_id(
                target_relpath,
                &source_node.source_content,
                &branch.proposed_content,
            )
        });
    let derived_artifact_id = apply
        .derived_artifact_id
        .clone()
        .unwrap_or_else(|| text_file_artifact_id(target_relpath, &branch.proposed_content));
    branch.patch_id = Some(patch_id.clone());
    branch.generation_target = Some(operation_target.clone());
    branch.status = TreatmentBranchStatus::Applied;
    branch.apply_id = Some(apply.treatment_state.apply_id.clone());
    branch.applied_content_hash = Some(apply.applied_content_hash.clone());
    branch.derived_artifact_id = Some(derived_artifact_id.clone());
    source_node.selected_branch_id = Some(branch_id.clone());

    match registry
        .active_targets
        .iter_mut()
        .find(|entry| entry.target_relpath == target_relpath)
    {
        Some(active) => {
            active.source_state_id = source_state_id.clone();
            active.source_artifact_id = Some(source_artifact_id.clone());
            active.active_branch_id = Some(branch_id);
            active.active_patch_id = Some(patch_id.clone());
            active.active_apply_id = Some(apply.treatment_state.apply_id.clone());
            active.active_derived_artifact_id = Some(derived_artifact_id.clone());
            active.active_operation_target = Some(operation_target.clone());
        }
        None => registry.active_targets.push(ActiveInterventionTarget {
            target_relpath: target_relpath.to_path_buf(),
            source_state_id: source_state_id.clone(),
            source_artifact_id: Some(source_artifact_id),
            active_branch_id: Some(branch_id),
            active_patch_id: Some(patch_id),
            active_apply_id: Some(apply.treatment_state.apply_id.clone()),
            active_derived_artifact_id: Some(derived_artifact_id),
            active_operation_target: Some(operation_target),
        }),
    };

    registry.updated_at = Utc::now().to_rfc3339();
    save_branch_registry(campaign_manifest_path, &registry)?;
    Ok(registry)
}

pub fn select_treatment_branch(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    branch_id: &str,
) -> Result<Prototype1BranchRegistry, PrepareError> {
    let mut registry = load_or_default_branch_registry(campaign_id, campaign_manifest_path)?;
    let mut selected = None;

    for source_node in &mut registry.source_nodes {
        let source_state_id = source_node.source_state_id.clone();
        let target_relpath = source_node.target_relpath.clone();
        let source_artifact_id = source_node
            .source_artifact_id
            .clone()
            .unwrap_or_else(|| text_file_artifact_id(&target_relpath, &source_node.source_content));
        let operation_target = source_node
            .operation_target
            .clone()
            .unwrap_or_else(|| operation_target_for_artifact(&source_artifact_id));
        source_node.source_artifact_id = Some(source_artifact_id.clone());
        source_node.operation_target = Some(operation_target.clone());
        let mut branch_found = false;
        for branch in &mut source_node.branches {
            if branch.branch_id == branch_id {
                let patch_id = branch.patch_id.clone().unwrap_or_else(|| {
                    text_replacement_patch_id(
                        &target_relpath,
                        &source_node.source_content,
                        &branch.proposed_content,
                    )
                });
                branch.patch_id = Some(patch_id.clone());
                branch.generation_target = Some(operation_target.clone());
                branch_found = true;
                if branch.status == TreatmentBranchStatus::Synthesized {
                    branch.status = TreatmentBranchStatus::Selected;
                }
                source_node.selected_branch_id = Some(branch_id.to_string());
                selected = Some((
                    source_state_id.clone(),
                    target_relpath.clone(),
                    source_artifact_id.clone(),
                    patch_id,
                    branch.derived_artifact_id.clone(),
                    operation_target.clone(),
                ));
            }
        }
        if branch_found {
            break;
        }
    }

    let (
        source_state_id,
        target_relpath,
        source_artifact_id,
        active_patch_id,
        active_derived_artifact_id,
        active_operation_target,
    ) = selected.ok_or_else(|| PrepareError::InvalidBatchSelection {
        detail: format!("unknown treatment branch '{}'", branch_id),
    })?;

    match registry
        .active_targets
        .iter_mut()
        .find(|entry| entry.target_relpath == target_relpath)
    {
        Some(active) => {
            active.source_state_id = source_state_id;
            active.source_artifact_id = Some(source_artifact_id.clone());
            active.active_branch_id = Some(branch_id.to_string());
            active.active_patch_id = Some(active_patch_id.clone());
            active.active_apply_id = None;
            active.active_derived_artifact_id = active_derived_artifact_id.clone();
            active.active_operation_target = Some(active_operation_target.clone());
        }
        None => registry.active_targets.push(ActiveInterventionTarget {
            target_relpath,
            source_state_id,
            source_artifact_id: Some(source_artifact_id),
            active_branch_id: Some(branch_id.to_string()),
            active_patch_id: Some(active_patch_id),
            active_apply_id: None,
            active_derived_artifact_id,
            active_operation_target: Some(active_operation_target),
        }),
    };

    registry.updated_at = Utc::now().to_rfc3339();
    save_branch_registry(campaign_manifest_path, &registry)?;
    Ok(registry)
}

pub fn active_branch_selection_for_target(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    target_relpath: &Path,
) -> Result<Option<ActiveBranchSelection>, PrepareError> {
    let registry = load_or_default_branch_registry(campaign_id, campaign_manifest_path)?;
    let Some(active) = registry
        .active_targets
        .iter()
        .find(|entry| entry.target_relpath == target_relpath)
    else {
        return Ok(None);
    };
    let Some(branch_id) = active.active_branch_id.as_deref() else {
        return Ok(None);
    };
    let Some(source_node) = registry.source_nodes.iter().find(|node| {
        node.source_state_id == active.source_state_id && node.target_relpath == target_relpath
    }) else {
        return Ok(None);
    };
    let Some(branch) = source_node
        .branches
        .iter()
        .find(|branch| branch.branch_id == branch_id)
        .cloned()
    else {
        return Ok(None);
    };
    Ok(Some(ActiveBranchSelection {
        target_relpath: target_relpath.to_path_buf(),
        source_state_id: source_node.source_state_id.clone(),
        branch,
    }))
}

pub fn resolve_treatment_branch(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    branch_id: &str,
) -> Result<ResolvedTreatmentBranch, PrepareError> {
    let registry = load_or_default_branch_registry(campaign_id, campaign_manifest_path)?;
    for source_node in registry.source_nodes {
        if let Some(branch) = source_node
            .branches
            .iter()
            .find(|branch| branch.branch_id == branch_id)
            .cloned()
        {
            return Ok(ResolvedTreatmentBranch {
                instance_id: source_node.instance_id,
                source_state_id: source_node.source_state_id,
                parent_branch_id: source_node.parent_branch_id,
                target_relpath: source_node.target_relpath,
                source_content: source_node.source_content,
                source_content_hash: source_node.source_content_hash,
                selected_branch_id: source_node.selected_branch_id,
                branch,
            });
        }
    }
    Err(PrepareError::InvalidBatchSelection {
        detail: format!("unknown treatment branch '{}'", branch_id),
    })
}

pub fn restore_treatment_branch(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    branch_id: &str,
    repo_root: &Path,
) -> Result<InterventionRestoreOutput, PrepareError> {
    let mut registry = load_or_default_branch_registry(campaign_id, campaign_manifest_path)?;
    let mut restored = None;

    for source_node in &mut registry.source_nodes {
        if let Some(branch) = source_node
            .branches
            .iter_mut()
            .find(|branch| branch.branch_id == branch_id)
        {
            let absolute_path = repo_root.join(&source_node.target_relpath);
            let current = fs::read_to_string(&absolute_path).map_err(|source| {
                PrepareError::ReadManifest {
                    path: absolute_path.clone(),
                    source,
                }
            })?;
            let changed = current != source_node.source_content;
            if changed {
                fs::write(&absolute_path, &source_node.source_content).map_err(|source| {
                    PrepareError::WriteManifest {
                        path: absolute_path.clone(),
                        source,
                    }
                })?;
            }
            branch.status = TreatmentBranchStatus::Restored;
            if source_node.selected_branch_id.as_deref() == Some(branch_id) {
                source_node.selected_branch_id = None;
            }
            if let Some(active) = registry
                .active_targets
                .iter_mut()
                .find(|entry| entry.target_relpath == source_node.target_relpath)
            {
                if active.active_branch_id.as_deref() == Some(branch_id) {
                    active.active_branch_id = None;
                    active.active_patch_id = None;
                    active.active_apply_id = None;
                    active.active_derived_artifact_id = None;
                    active.active_operation_target = None;
                    active.source_state_id = source_node.source_state_id.clone();
                    active.source_artifact_id = source_node.source_artifact_id.clone();
                }
            }
            restored = Some(InterventionRestoreOutput {
                branch_id: branch_id.to_string(),
                source_state_id: source_node.source_state_id.clone(),
                target_relpath: source_node.target_relpath.clone(),
                restored_content_hash: source_node.source_content_hash.clone(),
                changed,
            });
            break;
        }
    }

    let restored = restored.ok_or_else(|| PrepareError::InvalidBatchSelection {
        detail: format!("unknown treatment branch '{}'", branch_id),
    })?;
    registry.updated_at = Utc::now().to_rfc3339();
    save_branch_registry(campaign_manifest_path, &registry)?;
    Ok(restored)
}

pub fn record_treatment_branch_evaluation(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    branch_id: &str,
    summary: TreatmentBranchEvaluationSummary,
) -> Result<Prototype1BranchRegistry, PrepareError> {
    let mut registry = load_or_default_branch_registry(campaign_id, campaign_manifest_path)?;
    let mut found = false;

    for source_node in &mut registry.source_nodes {
        if let Some(branch) = source_node
            .branches
            .iter_mut()
            .find(|branch| branch.branch_id == branch_id)
        {
            branch.latest_evaluation = Some(summary.clone());
            found = true;
            break;
        }
    }

    if !found {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!("unknown treatment branch '{}'", branch_id),
        });
    }

    registry.updated_at = Utc::now().to_rfc3339();
    save_branch_registry(campaign_manifest_path, &registry)?;
    Ok(registry)
}

#[cfg(test)]
mod tests {
    use tempfile::tempdir;

    use super::super::apply::execute_intervention_apply;
    use super::super::spec::{
        ArtifactEdit, InterventionApplyInput, InterventionCandidate, InterventionCandidateSet,
        InterventionSpec, ValidationPolicy,
    };
    use super::*;
    use ploke_core::tool_types::ToolName;

    fn campaign_manifest_path(tmp: &Path) -> PathBuf {
        let campaign_dir = tmp.join("campaigns/test-campaign");
        fs::create_dir_all(&campaign_dir).expect("campaign dir");
        campaign_dir.join("campaign.json")
    }

    fn synthesis_output() -> InterventionSynthesisOutput {
        InterventionSynthesisOutput {
            candidate_set: InterventionCandidateSet {
                source_state_id: "baseline-run-1".to_string(),
                target_relpath: PathBuf::from(
                    "crates/ploke-core/tool_text/request_code_context.md",
                ),
                source_content: "old text\n".to_string(),
                operation_target: None,
                candidates: vec![
                    InterventionCandidate {
                        candidate_id: "candidate-1".to_string(),
                        branch_label: "minimal_rewrite".to_string(),
                        proposed_content: "new text a\n".to_string(),
                        patch_id: None,
                        spec: InterventionSpec::ToolGuidanceMutation {
                            spec_id: "reviewed-tool:request_code_context:minimal_rewrite"
                                .to_string(),
                            evidence_basis: "basis".to_string(),
                            intended_effect: "effect".to_string(),
                            tool: ToolName::RequestCodeContext,
                            edit: ArtifactEdit::ReplaceWholeText {
                                new_text: "new text a\n".to_string(),
                            },
                            validation_policy: ValidationPolicy::for_tool_description_target(
                                ToolName::RequestCodeContext,
                            ),
                        },
                    },
                    InterventionCandidate {
                        candidate_id: "candidate-2".to_string(),
                        branch_label: "decision_rule_rewrite".to_string(),
                        proposed_content: "new text b\n".to_string(),
                        patch_id: None,
                        spec: InterventionSpec::ToolGuidanceMutation {
                            spec_id: "reviewed-tool:request_code_context:decision_rule_rewrite"
                                .to_string(),
                            evidence_basis: "basis".to_string(),
                            intended_effect: "effect".to_string(),
                            tool: ToolName::RequestCodeContext,
                            edit: ArtifactEdit::ReplaceWholeText {
                                new_text: "new text b\n".to_string(),
                            },
                            validation_policy: ValidationPolicy::for_tool_description_target(
                                ToolName::RequestCodeContext,
                            ),
                        },
                    },
                ],
            },
        }
    }

    #[test]
    fn record_synthesis_creates_source_node_and_selected_branch() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());
        let synthesis = synthesis_output();

        let registry = record_synthesized_branches(
            "test-campaign",
            &manifest,
            "clap-rs__clap-3670",
            &synthesis,
            Some("candidate-2"),
            None,
        )
        .expect("record synthesis");

        assert_eq!(registry.source_nodes.len(), 1);
        let source = &registry.source_nodes[0];
        assert_eq!(source.instance_id, "clap-rs__clap-3670");
        assert_eq!(source.parent_branch_id, None);
        assert_eq!(source.branches.len(), 2);
        assert!(source.selected_branch_id.is_some());
        let selected = source
            .branches
            .iter()
            .find(|branch| branch.candidate_id == "candidate-2")
            .expect("selected branch");
        assert_eq!(selected.status, TreatmentBranchStatus::Selected);
    }

    #[test]
    fn branch_registry_deserializes_legacy_records_without_graph_provenance() {
        let legacy = serde_json::json!({
            "schema_version": PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION,
            "campaign_id": "test-campaign",
            "updated_at": "2026-04-26T00:00:00Z",
            "source_nodes": [{
                "source_state_id": "baseline-run-1",
                "parent_branch_id": "branch-parent",
                "instance_id": "clap-rs__clap-3670",
                "target_relpath": "crates/ploke-core/tool_text/request_code_context.md",
                "source_content": "old text\n",
                "source_content_hash": "old-hash",
                "selected_branch_id": "branch-selected",
                "branches": [{
                    "branch_id": "branch-selected",
                    "candidate_id": "candidate-1",
                    "branch_label": "minimal_rewrite",
                    "synthesized_spec_id": "spec-1",
                    "proposed_content": "new text\n",
                    "proposed_content_hash": "new-hash",
                    "status": "selected"
                }]
            }],
            "active_targets": [{
                "target_relpath": "crates/ploke-core/tool_text/request_code_context.md",
                "source_state_id": "baseline-run-1",
                "active_branch_id": "branch-selected"
            }]
        });

        let registry: Prototype1BranchRegistry =
            serde_json::from_value(legacy).expect("deserialize legacy registry");
        let source = &registry.source_nodes[0];
        assert_eq!(source.source_artifact_id, None);
        assert_eq!(source.operation_target, None);
        assert_eq!(source.branches[0].patch_id, None);
        assert_eq!(source.branches[0].generation_target, None);
        assert_eq!(source.branches[0].generation_coordinate, None);
        assert_eq!(source.branches[0].derived_artifact_id, None);
        assert_eq!(registry.active_targets[0].source_artifact_id, None);
        assert_eq!(registry.active_targets[0].active_patch_id, None);
        assert_eq!(registry.active_targets[0].active_derived_artifact_id, None);
        assert_eq!(registry.active_targets[0].active_operation_target, None);

        let roundtrip = serde_json::to_value(&registry).expect("serialize registry");
        let branch = &roundtrip["source_nodes"][0]["branches"][0];
        assert!(branch.get("patch_id").is_none());
        assert!(branch.get("generation_target").is_none());
        assert!(branch.get("generation_coordinate").is_none());
        assert!(branch.get("derived_artifact_id").is_none());
        assert!(
            roundtrip["active_targets"][0]
                .get("active_patch_id")
                .is_none()
        );
    }

    #[test]
    fn record_synthesis_preserves_base_artifact_target_and_patch_identity() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());
        let synthesis = synthesis_output();

        let registry = record_synthesized_branches(
            "test-campaign",
            &manifest,
            "clap-rs__clap-3670",
            &synthesis,
            Some("candidate-1"),
            None,
        )
        .expect("record synthesis");

        let source = &registry.source_nodes[0];
        let expected_artifact_id = text_file_artifact_id(
            &synthesis.candidate_set.target_relpath,
            &synthesis.candidate_set.source_content,
        );
        assert_eq!(
            source.source_artifact_id.as_ref(),
            Some(&expected_artifact_id)
        );
        assert_eq!(
            source.operation_target.as_ref(),
            Some(&operation_target_for_artifact(&expected_artifact_id))
        );

        let branch = source
            .branches
            .iter()
            .find(|branch| branch.candidate_id == "candidate-1")
            .expect("candidate branch");
        let patch_id = branch.patch_id.as_ref().expect("patch id");
        assert!(patch_id.as_str().starts_with("text-replace-sha256:"));
        assert_ne!(patch_id.as_str(), branch.branch_id);
        assert_ne!(patch_id.as_str(), branch.candidate_id);
        assert_eq!(
            branch.generation_target.as_ref(),
            Some(&operation_target_for_artifact(&expected_artifact_id))
        );
        assert_eq!(branch.generation_coordinate, None);
        assert_eq!(branch.derived_artifact_id, None);
    }

    #[test]
    fn apply_marks_active_branch_and_restore_writes_source_content() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());
        let repo_root = tmp.path().join("repo");
        let target_relpath = PathBuf::from("crates/ploke-core/tool_text/request_code_context.md");
        let absolute_target = repo_root.join(&target_relpath);
        fs::create_dir_all(absolute_target.parent().expect("parent")).expect("target dir");
        fs::write(&absolute_target, "old text\n").expect("seed");

        let synthesis = synthesis_output();
        let registry = record_synthesized_branches(
            "test-campaign",
            &manifest,
            "clap-rs__clap-3670",
            &synthesis,
            Some("candidate-1"),
            None,
        )
        .expect("record synthesis");
        let source = &registry.source_nodes[0];
        let branch_id = source.selected_branch_id.clone().expect("selected branch");
        let candidate = synthesis
            .candidate_set
            .candidates
            .iter()
            .find(|candidate| candidate.candidate_id == "candidate-1")
            .expect("candidate")
            .clone();

        let apply_output = execute_intervention_apply(&InterventionApplyInput {
            source_state_id: synthesis.candidate_set.source_state_id.clone(),
            candidate,
            target_relpath: target_relpath.clone(),
            expected_source_content: synthesis.candidate_set.source_content.clone(),
            repo_root: repo_root.clone(),
            base_artifact_id: None,
            patch_id: None,
        })
        .expect("apply");
        let registry = mark_treatment_branch_applied(
            "test-campaign",
            &manifest,
            &target_relpath,
            &apply_output,
        )
        .expect("mark applied");
        assert_eq!(registry.active_targets.len(), 1);
        assert_eq!(
            registry.active_targets[0].active_branch_id.as_deref(),
            Some(branch_id.as_str())
        );
        let active = &registry.active_targets[0];
        assert!(active.source_artifact_id.is_some());
        assert!(active.active_patch_id.is_some());
        assert_eq!(
            active.active_apply_id.as_deref(),
            Some(apply_output.treatment_state.apply_id.as_str())
        );
        assert_eq!(
            active.active_derived_artifact_id.as_ref(),
            apply_output.derived_artifact_id.as_ref()
        );
        assert!(active.active_operation_target.is_some());
        let applied_branch = registry.source_nodes[0]
            .branches
            .iter()
            .find(|branch| branch.branch_id == branch_id)
            .expect("applied branch");
        assert_eq!(
            applied_branch.derived_artifact_id.as_ref(),
            apply_output.derived_artifact_id.as_ref()
        );

        let restored = restore_treatment_branch("test-campaign", &manifest, &branch_id, &repo_root)
            .expect("restore");
        assert!(restored.changed);
        assert_eq!(
            fs::read_to_string(&absolute_target).expect("restored target"),
            "old text\n"
        );
        let registry =
            load_or_default_branch_registry("test-campaign", &manifest).expect("reload registry");
        assert_eq!(registry.active_targets[0].active_branch_id, None);
        let restored_branch = registry.source_nodes[0]
            .branches
            .iter()
            .find(|branch| branch.branch_id == branch_id)
            .expect("restored branch");
        assert_eq!(restored_branch.status, TreatmentBranchStatus::Restored);
    }

    #[test]
    fn select_branch_marks_active_target_without_applying() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());
        let synthesis = synthesis_output();
        let registry = record_synthesized_branches(
            "test-campaign",
            &manifest,
            "clap-rs__clap-3670",
            &synthesis,
            Some("candidate-1"),
            None,
        )
        .expect("record synthesis");
        let branch_id = registry.source_nodes[0].branches[1].branch_id.clone();

        let registry =
            select_treatment_branch("test-campaign", &manifest, &branch_id).expect("select");
        assert_eq!(
            registry.active_targets[0].active_branch_id.as_deref(),
            Some(branch_id.as_str())
        );
        let selected = active_branch_selection_for_target(
            "test-campaign",
            &manifest,
            &synthesis.candidate_set.target_relpath,
        )
        .expect("active selection")
        .expect("selection present");
        assert_eq!(selected.branch.branch_id, branch_id);
        assert_eq!(selected.branch.status, TreatmentBranchStatus::Selected);
    }

    #[test]
    fn resolve_branch_returns_source_and_branch_content() {
        let tmp = tempdir().expect("tmp");
        let manifest = campaign_manifest_path(tmp.path());
        let synthesis = synthesis_output();
        let registry = record_synthesized_branches(
            "test-campaign",
            &manifest,
            "clap-rs__clap-3670",
            &synthesis,
            Some("candidate-2"),
            Some("branch-parent"),
        )
        .expect("record synthesis");
        let branch_id = registry.source_nodes[0].branches[1].branch_id.clone();

        let resolved =
            resolve_treatment_branch("test-campaign", &manifest, &branch_id).expect("resolve");
        assert_eq!(resolved.instance_id, "clap-rs__clap-3670");
        assert_eq!(resolved.parent_branch_id.as_deref(), Some("branch-parent"));
        assert_eq!(resolved.source_content, "old text\n");
        assert_eq!(resolved.branch.candidate_id, "candidate-2");
        assert_eq!(resolved.branch.proposed_content, "new text b\n");
    }
}
