use std::path::{Path, PathBuf};

use ploke_core::tool_types::ToolName;
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::intervention::issue::IssueCase;

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
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionCandidate {
    pub candidate_id: String,
    pub branch_label: String,
    pub proposed_content: String,
    pub spec: InterventionSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct InterventionCandidateSet {
    pub source_state_id: String,
    pub target_relpath: PathBuf,
    pub source_content: String,
    pub candidates: Vec<InterventionCandidate>,
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
