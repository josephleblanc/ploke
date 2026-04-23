use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::spec::InterventionApplyOutput;
use super::spec::InterventionSynthesisOutput;
use crate::branch_evaluation::BranchDisposition;
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
    pub branch_label: String,
    pub synthesized_spec_id: String,
    pub proposed_content: String,
    pub proposed_content_hash: String,
    pub status: TreatmentBranchStatus,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub apply_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applied_content_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_evaluation: Option<TreatmentBranchEvaluationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionSourceNode {
    pub source_state_id: String,
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
    pub active_branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active_apply_id: Option<String>,
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
    pub target_relpath: PathBuf,
    pub source_content: String,
    pub source_content_hash: String,
    pub selected_branch_id: Option<String>,
    pub branch: TreatmentBranchNode,
}

fn sha256_hex(payload: &str) -> String {
    format!("{:x}", Sha256::digest(payload.as_bytes()))
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
) -> Result<Prototype1BranchRegistry, PrepareError> {
    let mut registry = load_or_default_branch_registry(campaign_id, campaign_manifest_path)?;
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
            instance_id: instance_id.to_string(),
            target_relpath: synthesis.candidate_set.target_relpath.clone(),
            source_content: synthesis.candidate_set.source_content.clone(),
            source_content_hash: sha256_hex(&synthesis.candidate_set.source_content),
            selected_branch_id: None,
            branches: Vec::new(),
        });
        registry
            .source_nodes
            .last_mut()
            .expect("newly pushed source node")
    };

    source_node.instance_id = instance_id.to_string();
    source_node.source_content = synthesis.candidate_set.source_content.clone();
    source_node.source_content_hash = sha256_hex(&synthesis.candidate_set.source_content);
    source_node.selected_branch_id = selected_branch_id.clone();

    for candidate in &synthesis.candidate_set.candidates {
        let branch_id = treatment_branch_id(
            &synthesis.candidate_set.source_state_id,
            &synthesis.candidate_set.target_relpath,
            &candidate.candidate_id,
        );
        let proposed_content_hash = sha256_hex(&candidate.proposed_content);
        match source_node
            .branches
            .iter_mut()
            .find(|branch| branch.branch_id == branch_id)
        {
            Some(branch) => {
                branch.branch_label = candidate.branch_label.clone();
                branch.synthesized_spec_id = candidate.spec.spec_id().to_string();
                branch.proposed_content = candidate.proposed_content.clone();
                branch.proposed_content_hash = proposed_content_hash;
                if selected_candidate_id == Some(candidate.candidate_id.as_str()) {
                    branch.status = TreatmentBranchStatus::Selected;
                }
            }
            None => {
                source_node.branches.push(TreatmentBranchNode {
                    branch_id,
                    candidate_id: candidate.candidate_id.clone(),
                    branch_label: candidate.branch_label.clone(),
                    synthesized_spec_id: candidate.spec.spec_id().to_string(),
                    proposed_content: candidate.proposed_content.clone(),
                    proposed_content_hash,
                    status: if selected_candidate_id == Some(candidate.candidate_id.as_str()) {
                        TreatmentBranchStatus::Selected
                    } else {
                        TreatmentBranchStatus::Synthesized
                    },
                    apply_id: None,
                    applied_content_hash: None,
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
    branch.status = TreatmentBranchStatus::Applied;
    branch.apply_id = Some(apply.treatment_state.apply_id.clone());
    branch.applied_content_hash = Some(apply.applied_content_hash.clone());
    source_node.selected_branch_id = Some(branch_id.clone());

    match registry
        .active_targets
        .iter_mut()
        .find(|entry| entry.target_relpath == target_relpath)
    {
        Some(active) => {
            active.source_state_id = source_state_id.clone();
            active.active_branch_id = Some(branch_id);
            active.active_apply_id = Some(apply.treatment_state.apply_id.clone());
        }
        None => registry.active_targets.push(ActiveInterventionTarget {
            target_relpath: target_relpath.to_path_buf(),
            source_state_id: source_state_id.clone(),
            active_branch_id: Some(branch_id),
            active_apply_id: Some(apply.treatment_state.apply_id.clone()),
        }),
    }

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
        let mut branch_found = false;
        for branch in &mut source_node.branches {
            if branch.branch_id == branch_id {
                branch_found = true;
                if branch.status == TreatmentBranchStatus::Synthesized {
                    branch.status = TreatmentBranchStatus::Selected;
                }
                source_node.selected_branch_id = Some(branch_id.to_string());
                selected = Some((source_state_id.clone(), target_relpath.clone()));
            }
        }
        if branch_found {
            break;
        }
    }

    let (source_state_id, target_relpath) =
        selected.ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!("unknown treatment branch '{}'", branch_id),
        })?;

    match registry
        .active_targets
        .iter_mut()
        .find(|entry| entry.target_relpath == target_relpath)
    {
        Some(active) => {
            active.source_state_id = source_state_id;
            active.active_branch_id = Some(branch_id.to_string());
            active.active_apply_id = None;
        }
        None => registry.active_targets.push(ActiveInterventionTarget {
            target_relpath,
            source_state_id,
            active_branch_id: Some(branch_id.to_string()),
            active_apply_id: None,
        }),
    }

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
                    active.active_apply_id = None;
                    active.source_state_id = source_node.source_state_id.clone();
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
                candidates: vec![
                    InterventionCandidate {
                        candidate_id: "candidate-1".to_string(),
                        branch_label: "minimal_rewrite".to_string(),
                        proposed_content: "new text a\n".to_string(),
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
        )
        .expect("record synthesis");

        assert_eq!(registry.source_nodes.len(), 1);
        let source = &registry.source_nodes[0];
        assert_eq!(source.instance_id, "clap-rs__clap-3670");
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
        )
        .expect("record synthesis");
        let branch_id = registry.source_nodes[0].branches[1].branch_id.clone();

        let resolved =
            resolve_treatment_branch("test-campaign", &manifest, &branch_id).expect("resolve");
        assert_eq!(resolved.instance_id, "clap-rs__clap-3670");
        assert_eq!(resolved.source_content, "old text\n");
        assert_eq!(resolved.branch.candidate_id, "candidate-2");
        assert_eq!(resolved.branch.proposed_content, "new text b\n");
    }
}
