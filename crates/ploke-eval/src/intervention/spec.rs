use std::path::{Path, PathBuf};

use ploke_core::tool_types::ToolName;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

use crate::intervention::issue::IssueCase;
use crate::loop_graph::{ArtifactId, OperationTarget, PatchId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InterventionKind {
    ToolGuidanceMutation,
    PolicyConfigMutation,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum ArtifactEdit {
    ReplaceWholeText {
        new_text: String,
    },
    AppendText {
        text: String,
    },
    ReplaceSection {
        start_marker: String,
        end_marker: String,
        replacement: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationPolicy {
    pub allowed_relpaths: Vec<PathBuf>,
    pub require_target_exists: bool,
    pub require_nonempty_result: bool,
    pub require_utf8: bool,
    pub require_content_change: bool,
    pub require_markers_after_apply: Vec<String>,
    pub require_cargo_check: bool,
}

impl ValidationPolicy {
    pub fn for_tool_description_target(tool: ToolName) -> Self {
        Self {
            allowed_relpaths: vec![PathBuf::from(tool.description_artifact_relpath())],
            require_target_exists: true,
            require_nonempty_result: true,
            require_utf8: true,
            require_content_change: true,
            require_markers_after_apply: Vec::new(),
            require_cargo_check: false,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum InterventionSpec {
    ToolGuidanceMutation {
        spec_id: String,
        evidence_basis: String,
        intended_effect: String,
        tool: ToolName,
        edit: ArtifactEdit,
        validation_policy: ValidationPolicy,
    },
    PolicyConfigMutation {
        spec_id: String,
        evidence_basis: String,
        intended_effect: String,
        relpath: PathBuf,
        edit: ArtifactEdit,
        validation_policy: ValidationPolicy,
    },
}

impl InterventionSpec {
    pub fn kind(&self) -> InterventionKind {
        match self {
            Self::ToolGuidanceMutation { .. } => InterventionKind::ToolGuidanceMutation,
            Self::PolicyConfigMutation { .. } => InterventionKind::PolicyConfigMutation,
        }
    }

    pub fn spec_id(&self) -> &str {
        match self {
            Self::ToolGuidanceMutation { spec_id, .. }
            | Self::PolicyConfigMutation { spec_id, .. } => spec_id,
        }
    }

    pub fn edit(&self) -> &ArtifactEdit {
        match self {
            Self::ToolGuidanceMutation { edit, .. } | Self::PolicyConfigMutation { edit, .. } => {
                edit
            }
        }
    }

    pub fn validation_policy(&self) -> &ValidationPolicy {
        match self {
            Self::ToolGuidanceMutation {
                validation_policy, ..
            }
            | Self::PolicyConfigMutation {
                validation_policy, ..
            } => validation_policy,
        }
    }

    pub fn target_relpath(&self) -> &Path {
        match self {
            Self::ToolGuidanceMutation { tool, .. } => {
                Path::new(tool.description_artifact_relpath())
            }
            Self::PolicyConfigMutation { relpath, .. } => relpath.as_path(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionSynthesisInput {
    pub issue: IssueCase,
    pub source_state_id: String,
    pub source_content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) operation_target: Option<OperationTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionCandidate {
    pub candidate_id: String,
    pub branch_label: String,
    pub proposed_content: String,
    pub spec: InterventionSpec,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) patch_id: Option<PatchId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionCandidateSet {
    pub source_state_id: String,
    pub target_relpath: PathBuf,
    pub source_content: String,
    pub candidates: Vec<InterventionCandidate>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) operation_target: Option<OperationTarget>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionSynthesisOutput {
    pub candidate_set: InterventionCandidateSet,
}

impl InterventionSynthesisOutput {
    pub fn primary_candidate(&self) -> Option<&InterventionCandidate> {
        self.candidate_set.candidates.first()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TreatmentStateRef {
    pub source_state_id: String,
    pub apply_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionApplyInput {
    pub source_state_id: String,
    pub candidate: InterventionCandidate,
    pub target_relpath: PathBuf,
    pub expected_source_content: String,
    pub repo_root: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) base_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) patch_id: Option<PatchId>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionApplyOutput {
    pub treatment_state: TreatmentStateRef,
    pub candidate_id: String,
    pub target_relpath: PathBuf,
    pub absolute_path: PathBuf,
    pub changed: bool,
    pub source_content_hash: String,
    pub applied_content_hash: String,
    pub validation: ValidationResult,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) base_artifact_id: Option<ArtifactId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) patch_id: Option<PatchId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) derived_artifact_id: Option<ArtifactId>,
}

pub(crate) fn sha256_hex(payload: &str) -> String {
    format!("{:x}", Sha256::digest(payload.as_bytes()))
}

pub(crate) fn operation_target_artifact_id(target: &OperationTarget) -> Option<&ArtifactId> {
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

/// Content-derived identity for the current text-file prototype surface.
///
/// This names the target file relpath plus text content. It is intentionally
/// not a whole-worktree ArtifactId, so dirty worktrees are not silently treated
/// as durable graph nodes. Callers such as
/// [`crate::intervention::apply::execute_intervention_apply`] use this only as
/// a fallback until the live backend can provide durable git/tree/manifest
/// artifact ids.
pub(crate) fn text_file_artifact_id(target_relpath: &Path, content: &str) -> ArtifactId {
    let mut hasher = Sha256::new();
    hasher.update(target_relpath.display().to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(content.as_bytes());
    ArtifactId::new(format!("text-file-sha256:{:x}", hasher.finalize()))
}

/// Content-derived identity for a generated full-text replacement proposal.
///
/// This is a patch/proposal identity for the current text-file surface. It is
/// not a branch id and should not be used as a durable artifact id. The branch
/// registry records it through
/// [`crate::intervention::branch_registry::record_synthesized_branches`] so the
/// current prototype does not lose patch-attempt provenance while broader
/// runtime coordinates are still being wired in.
pub(crate) fn text_replacement_patch_id(
    target_relpath: &Path,
    source_content: &str,
    proposed_content: &str,
) -> PatchId {
    let mut hasher = Sha256::new();
    hasher.update(target_relpath.display().to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(source_content.as_bytes());
    hasher.update(b"\0");
    hasher.update(proposed_content.as_bytes());
    PatchId::new(format!("text-replace-sha256:{:x}", hasher.finalize()))
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionExecutionInput {
    pub repo_root: PathBuf,
    pub spec: InterventionSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionExecutionOutput {
    pub spec_id: String,
    pub kind: InterventionKind,
    pub target_relpath: PathBuf,
    pub absolute_path: PathBuf,
    pub changed: bool,
    pub applied: AppliedEdit,
    pub validation: ValidationResult,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AppliedEdit {
    pub absolute_path: PathBuf,
    pub changed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ValidationResult {
    pub ok: bool,
    pub checks: Vec<String>,
}

#[derive(Debug, Error)]
pub enum InterventionSpecError {
    #[error("unsupported intervention kind for this adapter: {0:?}")]
    UnsupportedKind(InterventionKind),
    #[error("target path is not allowed by validation policy: {0}")]
    TargetNotAllowed(String),
    #[error("target path is missing: {0}")]
    MissingTarget(PathBuf),
    #[error("replace_section markers not found for target {target}")]
    ReplaceSectionMarkersMissing { target: String },
    #[error("validation failed: {0}")]
    ValidationFailed(String),
    #[error(
        "source content mismatch for apply target {target}: expected synthesized source state content"
    )]
    SourceContentMismatch { target: String },
    #[error("i/o error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("unimplemented capability: {0}")]
    Unimplemented(&'static str),
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn validation_policy_json() -> serde_json::Value {
        json!({
            "allowed_relpaths": ["crates/ploke-core/tool_text/request_code_context.md"],
            "require_target_exists": true,
            "require_nonempty_result": true,
            "require_utf8": true,
            "require_content_change": true,
            "require_markers_after_apply": [],
            "require_cargo_check": false
        })
    }

    fn tool_spec_json() -> serde_json::Value {
        json!({
            "kind": "tool_guidance_mutation",
            "spec_id": "reviewed-tool:request_code_context:minimal_rewrite",
            "evidence_basis": "basis",
            "intended_effect": "effect",
            "tool": "request_code_context",
            "edit": {
                "kind": "replace_whole_text",
                "new_text": "new text\n"
            },
            "validation_policy": validation_policy_json()
        })
    }

    #[test]
    fn legacy_candidate_deserializes_without_patch_id() {
        let candidate: InterventionCandidate = serde_json::from_value(json!({
            "candidate_id": "candidate-1",
            "branch_label": "minimal_rewrite",
            "proposed_content": "new text\n",
            "spec": tool_spec_json()
        }))
        .expect("deserialize legacy candidate");

        assert_eq!(candidate.patch_id, None);
        let serialized = serde_json::to_value(&candidate).expect("serialize candidate");
        assert!(serialized.get("patch_id").is_none());
    }

    #[test]
    fn legacy_apply_output_deserializes_without_graph_provenance() {
        let output: InterventionApplyOutput = serde_json::from_value(json!({
            "treatment_state": {
                "source_state_id": "baseline-run-1",
                "apply_id": "apply-1"
            },
            "candidate_id": "candidate-1",
            "target_relpath": "crates/ploke-core/tool_text/request_code_context.md",
            "absolute_path": "/tmp/repo/crates/ploke-core/tool_text/request_code_context.md",
            "changed": true,
            "source_content_hash": "source-hash",
            "applied_content_hash": "applied-hash",
            "validation": {
                "ok": true,
                "checks": []
            }
        }))
        .expect("deserialize legacy apply output");

        assert_eq!(output.base_artifact_id, None);
        assert_eq!(output.patch_id, None);
        assert_eq!(output.derived_artifact_id, None);
        let serialized = serde_json::to_value(&output).expect("serialize apply output");
        assert!(serialized.get("base_artifact_id").is_none());
        assert!(serialized.get("patch_id").is_none());
        assert!(serialized.get("derived_artifact_id").is_none());
    }
}
