use std::{
    ffi::OsString,
    fmt, fs,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use sha2::{Digest as _, Sha256};

use crate::cli::prototype1_state::backend::EditSurfaceAdmission;
use crate::loop_graph::{ArtifactId, Coordinate, OperationTarget, RuntimeId};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BroadHarnessRequest {
    pub(crate) schema: BroadHarnessRequestSchema,
    pub(crate) parent_node_id: ParentNodeRef,
    pub(crate) workspace: HarnessWorkspace,
    pub(crate) edit_policy: BroadEditPolicy,
    pub(crate) child_budget: HarnessChildBudget,
    pub(crate) protected_core: ProtectedCorePointer,
    pub(crate) evaluation: EvaluationBrief,
    pub(crate) evidence_roots: Vec<EvidenceRoot>,
    pub(crate) return_evidence: ReturnEvidenceContract,
    pub(crate) instructions: Vec<HarnessInstruction>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct PublishedBroadHarnessRequest {
    pub(crate) schema: PublishedBroadHarnessRequestSchema,
    pub(crate) request_id: String,
    pub(crate) request_hash: String,
    pub(crate) request_path: PathBuf,
    pub(crate) prompt_path: PathBuf,
    pub(crate) submitted_result_path: PathBuf,
    pub(crate) admission_binding: RequestAdmissionBinding,
    pub(crate) request: BroadHarnessRequest,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BroadHarnessRequestSchema {
    V1,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PublishedBroadHarnessRequestSchema {
    V1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Publication {
    sequence: u32,
}

impl Publication {
    fn allocate(
        request_path: &Path,
        prompt_path: &Path,
        submitted_result_path: &Path,
        workspace_path: &Path,
    ) -> Self {
        let mut sequence = 1;
        loop {
            let publication = Self { sequence };
            if publication.is_available(
                request_path,
                prompt_path,
                submitted_result_path,
                workspace_path,
            ) {
                return publication;
            }
            sequence += 1;
        }
    }

    fn request_id(self, parent_node_id: &str) -> String {
        match self.sequence {
            1 => format!("broad-harness-request:{parent_node_id}"),
            sequence => format!("broad-harness-request:{parent_node_id}:r{sequence}"),
        }
    }

    fn prompt_path(self, path: &Path) -> PathBuf {
        match self.sequence {
            1 => path.to_path_buf(),
            sequence => suffixed_file_path(path, sequence),
        }
    }

    fn request_path(self, path: &Path) -> PathBuf {
        match self.sequence {
            1 => path.to_path_buf(),
            sequence => suffixed_file_path(path, sequence),
        }
    }

    fn submitted_result_path(self, path: &Path) -> PathBuf {
        match self.sequence {
            1 => path.to_path_buf(),
            sequence => suffixed_file_path(path, sequence),
        }
    }

    fn workspace_path(self, path: &Path) -> PathBuf {
        match self.sequence {
            1 => path.to_path_buf(),
            sequence => suffixed_component_path(path, sequence),
        }
    }

    fn is_available(
        self,
        request_path: &Path,
        prompt_path: &Path,
        submitted_result_path: &Path,
        workspace_path: &Path,
    ) -> bool {
        !path_exists(&self.request_path(request_path))
            && !path_exists(&self.prompt_path(prompt_path))
            && !path_exists(&self.submitted_result_path(submitted_result_path))
            && !path_exists(&self.workspace_path(workspace_path))
    }
}

fn path_exists(path: &Path) -> bool {
    fs::metadata(path).is_ok()
}

fn suffixed_file_path(path: &Path, sequence: u32) -> PathBuf {
    let mut name = path
        .file_stem()
        .map(OsString::from)
        .or_else(|| path.file_name().map(OsString::from))
        .expect("published request path should end with a filename");
    name.push(format!("-r{sequence}"));
    if let Some(extension) = path.extension() {
        name.push(".");
        name.push(extension);
    }
    join_parent(path, name)
}

fn suffixed_component_path(path: &Path, sequence: u32) -> PathBuf {
    let mut name = path
        .file_name()
        .map(OsString::from)
        .expect("published request path should end with a path component");
    name.push(format!("-r{sequence}"));
    join_parent(path, name)
}

fn join_parent(path: &Path, name: OsString) -> PathBuf {
    match path.parent() {
        Some(parent) => parent.join(name),
        None => PathBuf::from(name),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub(crate) struct ParentNodeRef(String);

impl ParentNodeRef {
    pub(crate) fn new(value: String) -> Self {
        Self(value)
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

/// Request-time authority identity that later admission must match before backend checks.
///
/// This is binding evidence only. It is not itself admission.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct RequestAdmissionBinding {
    pub(crate) coordinate: Coordinate,
    pub(crate) target_artifact_id: ArtifactId,
    pub(crate) policy_id: RequestAdmissionPolicyId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RequestAdmissionBindingError {
    CoordinateTargetMismatch {
        expected_target_artifact_id: ArtifactId,
        actual_target: OperationTarget,
    },
}

impl RequestAdmissionBinding {
    pub(crate) fn new(
        coordinate: Coordinate,
        target_artifact_id: ArtifactId,
        policy_id: impl Into<String>,
    ) -> Result<Self, RequestAdmissionBindingError> {
        match &coordinate.target {
            OperationTarget::Artifact { artifact_id } if artifact_id == &target_artifact_id => {
                Ok(Self {
                    coordinate,
                    target_artifact_id,
                    policy_id: RequestAdmissionPolicyId::new(policy_id),
                })
            }
            actual_target => Err(RequestAdmissionBindingError::CoordinateTargetMismatch {
                expected_target_artifact_id: target_artifact_id,
                actual_target: actual_target.clone(),
            }),
        }
    }

    pub(crate) fn from_admission(
        admission: &EditSurfaceAdmission,
    ) -> Result<Self, RequestAdmissionBindingError> {
        let coordinate = admission.coordinate().clone();
        let target_artifact_id = match &coordinate.target {
            OperationTarget::Artifact { artifact_id } => artifact_id.clone(),
            actual_target => {
                return Err(RequestAdmissionBindingError::CoordinateTargetMismatch {
                    expected_target_artifact_id: ArtifactId::new(
                        "<admission target artifact>".to_string(),
                    ),
                    actual_target: actual_target.clone(),
                });
            }
        };
        Self::new(coordinate, target_artifact_id, admission.policy().as_str())
    }

    fn prototype1_workspace(source_repository_path: &Path, edit_policy: BroadEditPolicy) -> Self {
        let target_artifact_id = ArtifactId::new(source_repository_path.display().to_string());
        let coordinate = Coordinate {
            runtime_id: RuntimeId::new(),
            target: OperationTarget::Artifact {
                artifact_id: target_artifact_id.clone(),
            },
        };
        Self::new(coordinate, target_artifact_id, edit_policy.label())
            .expect("prototype workspace admission binding should match its target artifact")
    }

    pub(crate) fn coordinate(&self) -> &Coordinate {
        &self.coordinate
    }

    pub(crate) fn target_artifact_id(&self) -> &ArtifactId {
        &self.target_artifact_id
    }

    pub(crate) fn base_artifact_id(&self) -> &ArtifactId {
        &self.target_artifact_id
    }

    pub(crate) fn policy_id(&self) -> &RequestAdmissionPolicyId {
        &self.policy_id
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub(crate) struct RequestAdmissionPolicyId(String);

impl RequestAdmissionPolicyId {
    pub(crate) fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct HarnessWorkspace {
    pub(crate) source_repository_path: PathBuf,
    pub(crate) candidate_workspace_path: PathBuf,
}

impl HarnessWorkspace {
    fn new(source_repository_path: PathBuf, candidate_workspace_path: PathBuf) -> Self {
        Self {
            source_repository_path,
            candidate_workspace_path,
        }
    }

    pub(crate) fn source_repository_path(&self) -> &Path {
        &self.source_repository_path
    }

    pub(crate) fn candidate_workspace_path(&self) -> &Path {
        &self.candidate_workspace_path
    }

    fn display_source_repository(&self) -> impl fmt::Display + '_ {
        self.source_repository_path.display()
    }

    fn display_candidate_workspace(&self) -> impl fmt::Display + '_ {
        self.candidate_workspace_path.display()
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct HarnessChildBudget {
    pub(crate) min_children: u32,
    pub(crate) max_children: u32,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BroadEditPolicy {
    WorkspaceExceptPlokeEval,
}

impl BroadEditPolicy {
    fn label(self) -> &'static str {
        match self {
            Self::WorkspaceExceptPlokeEval => "workspace except ploke-eval",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ProtectedCorePointer {
    pub(crate) anchor: ProtectedCoreAnchor,
    pub(crate) policy: ProtectedCorePolicy,
    pub(crate) consequence: ProtectedCoreConsequence,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ProtectedCoreAnchor {
    AuthorityConstant {
        code_path: PathBuf,
        symbol: ProtectedCoreSymbol,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProtectedCoreSymbol {
    EvalCoreSurfaceRoot,
}

impl ProtectedCoreSymbol {
    fn label(self) -> &'static str {
        match self {
            Self::EvalCoreSurfaceRoot => "EVAL_CORE_SURFACE_ROOT",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProtectedCorePolicy {
    WorkspaceExceptPlokeEvalAuthoritySet,
}

impl ProtectedCorePolicy {
    fn label(self) -> &'static str {
        match self {
            Self::WorkspaceExceptPlokeEvalAuthoritySet => {
                "workspace-except-ploke-eval authority prefixes and filenames"
            }
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ProtectedCoreConsequence {
    RejectBeforeAdmissionOrInvalidDescendant,
}

impl ProtectedCoreConsequence {
    fn label(self) -> &'static str {
        match self {
            Self::RejectBeforeAdmissionOrInvalidDescendant => {
                "edits to protected authority are rejected before admission or prevent a valid descendant from starting"
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EvaluationBrief {
    pub(crate) scope: EvaluationScope,
    pub(crate) selection: SelectionAuthority,
    pub(crate) guidance: GuidancePolicy,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvaluationScope {
    Prototype1DescendantPerformance,
}

impl EvaluationScope {
    fn label(self) -> &'static str {
        match self {
            Self::Prototype1DescendantPerformance => "Prototype 1 descendant performance",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SelectionAuthority {
    HistoryBackedSuccessorSelection,
}

impl SelectionAuthority {
    fn label(self) -> &'static str {
        match self {
            Self::HistoryBackedSuccessorSelection => "History-backed successor selection",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GuidancePolicy {
    ProtocolDiagnosticsAreContext,
}

impl GuidancePolicy {
    fn label(self) -> &'static str {
        match self {
            Self::ProtocolDiagnosticsAreContext => {
                "protocol diagnoses are guidance, not hard file targets"
            }
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct EvidenceRoot {
    pub(crate) kind: EvidenceRootKind,
    pub(crate) location: EvidenceRootLocation,
    pub(crate) role: EvidenceRole,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceRootKind {
    SubmittedResultOutput,
    HistoryBlocks,
    Evaluations,
    Nodes,
    ProtocolArtifacts,
    Oracle,
}

impl EvidenceRootKind {
    fn label(self) -> &'static str {
        match self {
            Self::SubmittedResultOutput => "submitted result output",
            Self::HistoryBlocks => "History blocks",
            Self::Evaluations => "evaluations",
            Self::Nodes => "node records",
            Self::ProtocolArtifacts => "protocol artifacts",
            Self::Oracle => "oracle reports",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum EvidenceRootLocation {
    Directory {
        path: PathBuf,
    },
    File {
        path: PathBuf,
    },
    NodeScopedDirectory {
        nodes_root: PathBuf,
        child_relpath: PathBuf,
    },
    AttachedReport {
        report: AttachedReport,
    },
}

impl EvidenceRootLocation {
    fn render(&self) -> String {
        match self {
            Self::Directory { path } => path.display().to_string(),
            Self::File { path } => path.display().to_string(),
            Self::NodeScopedDirectory {
                nodes_root,
                child_relpath,
            } => format!(
                "{}/<node>/{}",
                nodes_root.display(),
                child_relpath.display()
            ),
            Self::AttachedReport { report } => report.label().to_string(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AttachedReport {
    FinalReportJson,
}

impl AttachedReport {
    fn label(self) -> &'static str {
        match self {
            Self::FinalReportJson => "final_report.json when present",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum EvidenceRole {
    OutputBox,
    SealedHistory,
    EvaluationPayloads,
    RuntimeEvidence,
    GuidanceOnly,
    OracleSummary,
}

impl EvidenceRole {
    fn label(self) -> &'static str {
        match self {
            Self::OutputBox => "write destination",
            Self::SealedHistory => "sealed run history",
            Self::EvaluationPayloads => "evaluation evidence",
            Self::RuntimeEvidence => "runtime evidence",
            Self::GuidanceOnly => "guidance only",
            Self::OracleSummary => "oracle summary",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum HarnessInstruction {
    InspectRepositoryAndEvidence,
    TreatProtocolDiagnosticsAsGuidance,
    EditBroadSurfaceOutsideProtectedCore,
    ChooseLikelyDescendantImprovement,
    WriteSubmittedResult,
}

impl HarnessInstruction {
    fn render(self, submitted_result_path: &Path) -> String {
        match self {
            Self::InspectRepositoryAndEvidence => {
                "Inspect the repository and the listed evidence before choosing edits.".to_string()
            }
            Self::TreatProtocolDiagnosticsAsGuidance => {
                "Use protocol diagnoses as context about possible tool or workflow issues, not as hard file targets.".to_string()
            }
            Self::EditBroadSurfaceOutsideProtectedCore => {
                "You may edit any useful part of the allowed workspace surface outside the protected ploke-eval authority core.".to_string()
            }
            Self::ChooseLikelyDescendantImprovement => {
                "Choose edits that you judge most likely to improve descendant performance under the evaluation and successor-selection loop.".to_string()
            }
            Self::WriteSubmittedResult => format!(
                "Write the typed submitted-result evidence to {}. This submission is evidence only; ploke-eval later checks it and may mint a ChildPlan after admission.",
                submitted_result_path.display()
            ),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ReturnEvidenceContract {
    pub(crate) authority_boundary: SubmissionAuthorityBoundary,
    pub(crate) fields: Vec<ReturnEvidenceField>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ReturnEvidenceField {
    ChangeSummary,
    GuidingEvidence,
    ImprovementRationale,
    SuggestedChecks,
}

impl ReturnEvidenceField {
    fn label(self) -> &'static str {
        match self {
            Self::ChangeSummary => "what files you changed",
            Self::GuidingEvidence => "what evidence guided the choice",
            Self::ImprovementRationale => "why the change should help future evaluations",
            Self::SuggestedChecks => "how the change should be checked",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct SubmissionAuthorityBoundary {
    pub(crate) admission: SubmissionAuthorityClaim,
    pub(crate) grant: SubmissionAuthorityClaim,
    pub(crate) child_plan: SubmissionAuthorityClaim,
}

impl SubmissionAuthorityBoundary {
    pub(crate) fn submitted_evidence_only() -> Self {
        Self {
            admission: SubmissionAuthorityClaim::NotClaimed,
            grant: SubmissionAuthorityClaim::NotClaimed,
            child_plan: SubmissionAuthorityClaim::NotClaimed,
        }
    }

    fn render(&self) -> String {
        format!(
            "admission={}, grant={}, child_plan={}",
            self.admission.label(),
            self.grant.label(),
            self.child_plan.label()
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum SubmissionAuthorityClaim {
    NotClaimed,
}

impl SubmissionAuthorityClaim {
    fn label(self) -> &'static str {
        match self {
            Self::NotClaimed => "not claimed",
        }
    }
}

impl BroadHarnessRequest {
    pub(crate) fn prototype1_workspace(
        parent_node_id: String,
        source_repository: PathBuf,
        child_budget: HarnessChildBudget,
        candidate_workspace_path: PathBuf,
        prototype_root: &Path,
        submitted_result_path: &Path,
    ) -> Self {
        Self {
            schema: BroadHarnessRequestSchema::V1,
            parent_node_id: ParentNodeRef::new(parent_node_id),
            workspace: HarnessWorkspace::new(source_repository, candidate_workspace_path),
            edit_policy: BroadEditPolicy::WorkspaceExceptPlokeEval,
            child_budget,
            protected_core: ProtectedCorePointer {
                anchor: ProtectedCoreAnchor::AuthorityConstant {
                    code_path: PathBuf::from(
                        "crates/ploke-eval/src/cli/prototype1_state/backend.rs",
                    ),
                    symbol: ProtectedCoreSymbol::EvalCoreSurfaceRoot,
                },
                policy: ProtectedCorePolicy::WorkspaceExceptPlokeEvalAuthoritySet,
                consequence: ProtectedCoreConsequence::RejectBeforeAdmissionOrInvalidDescendant,
            },
            evaluation: EvaluationBrief {
                scope: EvaluationScope::Prototype1DescendantPerformance,
                selection: SelectionAuthority::HistoryBackedSuccessorSelection,
                guidance: GuidancePolicy::ProtocolDiagnosticsAreContext,
            },
            evidence_roots: vec![
                EvidenceRoot {
                    kind: EvidenceRootKind::SubmittedResultOutput,
                    location: EvidenceRootLocation::File {
                        path: submitted_result_path.to_path_buf(),
                    },
                    role: EvidenceRole::OutputBox,
                },
                EvidenceRoot {
                    kind: EvidenceRootKind::HistoryBlocks,
                    location: EvidenceRootLocation::Directory {
                        path: prototype_root.join("history/blocks"),
                    },
                    role: EvidenceRole::SealedHistory,
                },
                EvidenceRoot {
                    kind: EvidenceRootKind::Evaluations,
                    location: EvidenceRootLocation::Directory {
                        path: prototype_root.join("evaluations"),
                    },
                    role: EvidenceRole::EvaluationPayloads,
                },
                EvidenceRoot {
                    kind: EvidenceRootKind::Nodes,
                    location: EvidenceRootLocation::Directory {
                        path: prototype_root.join("nodes"),
                    },
                    role: EvidenceRole::RuntimeEvidence,
                },
                EvidenceRoot {
                    kind: EvidenceRootKind::ProtocolArtifacts,
                    location: EvidenceRootLocation::NodeScopedDirectory {
                        nodes_root: prototype_root.join("nodes"),
                        child_relpath: PathBuf::from("protocol-artifacts"),
                    },
                    role: EvidenceRole::GuidanceOnly,
                },
                EvidenceRoot {
                    kind: EvidenceRootKind::Oracle,
                    location: EvidenceRootLocation::AttachedReport {
                        report: AttachedReport::FinalReportJson,
                    },
                    role: EvidenceRole::OracleSummary,
                },
            ],
            return_evidence: ReturnEvidenceContract {
                authority_boundary: SubmissionAuthorityBoundary::submitted_evidence_only(),
                fields: vec![
                    ReturnEvidenceField::ChangeSummary,
                    ReturnEvidenceField::GuidingEvidence,
                    ReturnEvidenceField::ImprovementRationale,
                    ReturnEvidenceField::SuggestedChecks,
                ],
            },
            instructions: vec![
                HarnessInstruction::InspectRepositoryAndEvidence,
                HarnessInstruction::TreatProtocolDiagnosticsAsGuidance,
                HarnessInstruction::EditBroadSurfaceOutsideProtectedCore,
                HarnessInstruction::ChooseLikelyDescendantImprovement,
                HarnessInstruction::WriteSubmittedResult,
            ],
        }
    }

    pub(crate) fn render_prompt(&self, submitted_result_path: &Path) -> String {
        let mut prompt = String::new();
        prompt.push_str("# Prototype 1 broad edit harness request\n\n");
        prompt.push_str(&format!("Parent node: {}\n", self.parent_node_id.as_str()));
        prompt.push_str(&format!(
            "Source repository snapshot: {}\n",
            self.workspace.display_source_repository()
        ));
        prompt.push_str(&format!(
            "Mutable candidate workspace: {}\n",
            self.workspace.display_candidate_workspace()
        ));
        prompt.push_str(&format!("Edit policy: {}\n", self.edit_policy.label()));
        prompt.push_str(&format!(
            "Child budget: {} to {}\n\n",
            self.child_budget.min_children, self.child_budget.max_children
        ));

        prompt.push_str("## Evaluation\n\n");
        prompt.push_str(&format!("- Scope: {}\n", self.evaluation.scope.label()));
        prompt.push_str(&format!(
            "- Selection: {}\n",
            self.evaluation.selection.label()
        ));
        prompt.push_str(&format!(
            "- Guidance: {}\n\n",
            self.evaluation.guidance.label()
        ));

        prompt.push_str("## Protected Core\n\n");
        prompt.push_str(&self.render_protected_core());
        prompt.push('\n');

        prompt.push_str("## Evidence\n\n");
        for root in &self.evidence_roots {
            prompt.push_str(&format!(
                "- {}: {} ({})\n",
                root.kind.label(),
                root.location.render(),
                root.role.label()
            ));
        }
        prompt.push('\n');

        prompt.push_str("## Return Evidence\n\n");
        prompt.push_str(&format!(
            "- Authority boundary: {}\n",
            self.return_evidence.authority_boundary.render()
        ));
        for field in &self.return_evidence.fields {
            prompt.push_str(&format!("- {}\n", field.label()));
        }
        prompt.push('\n');

        prompt.push_str("## Instructions\n\n");
        for instruction in &self.instructions {
            prompt.push_str(&format!(
                "- {}\n",
                instruction.render(submitted_result_path)
            ));
        }
        prompt
    }

    fn render_protected_core(&self) -> String {
        match &self.protected_core.anchor {
            ProtectedCoreAnchor::AuthorityConstant { code_path, symbol } => format!(
                "- Anchor: {}:{}\n- Policy: {}\n- Consequence: {}\n",
                code_path.display(),
                symbol.label(),
                self.protected_core.policy.label(),
                self.protected_core.consequence.label()
            ),
        }
    }
}

impl PublishedBroadHarnessRequest {
    pub(crate) fn prototype1_workspace(
        parent_node_id: String,
        source_repository: PathBuf,
        child_budget: HarnessChildBudget,
        prototype_root: &Path,
        request_path: PathBuf,
        prompt_path: PathBuf,
        submitted_result_path: PathBuf,
        admission_binding: RequestAdmissionBinding,
    ) -> Self {
        let workspace_path = prototype_root
            .join("workspaces/edit-harness")
            .join(&parent_node_id);
        let publication = Publication::allocate(
            request_path.as_path(),
            prompt_path.as_path(),
            submitted_result_path.as_path(),
            workspace_path.as_path(),
        );
        let request_path = publication.request_path(request_path.as_path());
        let prompt_path = publication.prompt_path(prompt_path.as_path());
        let submitted_result_path =
            publication.submitted_result_path(submitted_result_path.as_path());
        let candidate_workspace_path = publication.workspace_path(workspace_path.as_path());
        let request = BroadHarnessRequest::prototype1_workspace(
            parent_node_id.clone(),
            source_repository,
            child_budget,
            candidate_workspace_path,
            prototype_root,
            &submitted_result_path,
        );
        let mut published = Self {
            schema: PublishedBroadHarnessRequestSchema::V1,
            request_id: publication.request_id(&parent_node_id),
            request_hash: String::new(),
            request_path,
            prompt_path,
            submitted_result_path,
            admission_binding,
            request,
        };
        published.request_hash = published.compute_request_hash();
        published
    }

    pub(crate) fn request_id(&self) -> &str {
        &self.request_id
    }

    pub(crate) fn request_hash(&self) -> &str {
        &self.request_hash
    }

    pub(crate) fn request_path(&self) -> &Path {
        &self.request_path
    }

    pub(crate) fn prompt_path(&self) -> &Path {
        &self.prompt_path
    }

    pub(crate) fn submitted_result_path(&self) -> &Path {
        &self.submitted_result_path
    }

    pub(crate) fn admission_binding(&self) -> &RequestAdmissionBinding {
        &self.admission_binding
    }

    pub(crate) fn workspace_path(&self) -> &Path {
        self.request.workspace.candidate_workspace_path()
    }

    pub(crate) fn request(&self) -> &BroadHarnessRequest {
        &self.request
    }

    pub(crate) fn with_admission_binding(
        mut self,
        admission_binding: RequestAdmissionBinding,
    ) -> Self {
        self.admission_binding = admission_binding;
        self.request_hash = self.compute_request_hash();
        self
    }

    fn compute_request_hash(&self) -> String {
        let preimage = PublishedBroadHarnessRequestPreimage {
            request_id: &self.request_id,
            request_path: &self.request_path,
            prompt_path: &self.prompt_path,
            submitted_result_path: &self.submitted_result_path,
            admission_binding: self.admission_binding(),
            request: &self.request,
        };
        let bytes = serde_json::to_vec(&preimage)
            .expect("published broad harness request preimage should serialize");
        format!("{:x}", Sha256::digest(bytes))
    }
}

#[derive(Serialize)]
struct PublishedBroadHarnessRequestPreimage<'a> {
    request_id: &'a str,
    request_path: &'a PathBuf,
    prompt_path: &'a PathBuf,
    submitted_result_path: &'a PathBuf,
    admission_binding: &'a RequestAdmissionBinding,
    request: &'a BroadHarnessRequest,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::prototype1_state::backend::EditSurfaceAdmission;
    use tempfile::TempDir;

    struct Fixture {
        _temp: TempDir,
        prototype_root: PathBuf,
        request_path: PathBuf,
        prompt_path: PathBuf,
        submitted_result_path: PathBuf,
    }

    impl Fixture {
        fn new() -> Self {
            let temp = tempfile::tempdir().expect("tempdir");
            let prototype_root = temp.path().join("prototype1");
            let prompt_dir = prototype_root.join("messages/edit-harness-request");
            let result_dir = prototype_root.join("messages/edit-harness-result");
            fs::create_dir_all(&prompt_dir).expect("create prompt dir");
            fs::create_dir_all(&result_dir).expect("create result dir");
            Self {
                _temp: temp,
                prototype_root,
                request_path: prompt_dir.join("parent-node-7.json"),
                prompt_path: prompt_dir.join("parent-node-7.md"),
                submitted_result_path: result_dir.join("parent-node-7.json"),
            }
        }

        fn published_request(&self) -> PublishedBroadHarnessRequest {
            let admission_binding = self.request_admission_binding(
                "artifact:/repo/live-parent",
                "workspace except ploke-eval",
            );
            PublishedBroadHarnessRequest::prototype1_workspace(
                "parent-node-7".to_string(),
                PathBuf::from("/repo/live-parent"),
                HarnessChildBudget {
                    min_children: 1,
                    max_children: 3,
                },
                &self.prototype_root,
                self.request_path.clone(),
                self.prompt_path.clone(),
                self.submitted_result_path.clone(),
                admission_binding,
            )
        }

        fn request_admission_binding(
            &self,
            artifact_id: &str,
            policy_id: &str,
        ) -> RequestAdmissionBinding {
            let _ = self;
            let coordinate = Coordinate {
                runtime_id: RuntimeId::new(),
                target: OperationTarget::Artifact {
                    artifact_id: ArtifactId::new(artifact_id),
                },
            };
            let admission = EditSurfaceAdmission::new(
                coordinate,
                crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId::new(
                    policy_id,
                ),
            );
            RequestAdmissionBinding::from_admission(&admission)
                .expect("request admission binding should project from admission")
        }
    }

    #[test]
    fn published_request_binds_isolated_workspace_and_submitted_result() {
        let fixture = Fixture::new();
        let published = fixture.published_request();

        let json =
            serde_json::to_string(&published).expect("serialize published broad harness request");
        let decoded = serde_json::from_str::<PublishedBroadHarnessRequest>(&json)
            .expect("deserialize published broad harness request");

        assert_eq!(decoded, published);
        assert_eq!(
            decoded.request.workspace.source_repository_path(),
            Path::new("/repo/live-parent")
        );
        assert_eq!(
            decoded.workspace_path(),
            fixture
                .prototype_root
                .join("workspaces/edit-harness/parent-node-7")
                .as_path()
        );
        assert_eq!(decoded.request_path(), fixture.request_path.as_path());
        assert_eq!(
            decoded.submitted_result_path(),
            fixture.submitted_result_path.as_path()
        );
        assert_eq!(
            decoded.request.return_evidence.authority_boundary,
            SubmissionAuthorityBoundary::submitted_evidence_only()
        );
        assert_eq!(
            decoded.request.return_evidence.fields,
            vec![
                ReturnEvidenceField::ChangeSummary,
                ReturnEvidenceField::GuidingEvidence,
                ReturnEvidenceField::ImprovementRationale,
                ReturnEvidenceField::SuggestedChecks,
            ]
        );
        assert_eq!(
            decoded.admission_binding().base_artifact_id(),
            &ArtifactId::new("artifact:/repo/live-parent")
        );
        assert_eq!(
            decoded.admission_binding().policy_id().as_str(),
            "workspace except ploke-eval"
        );
    }

    #[test]
    fn prompt_names_submitted_result_instead_of_child_plan() {
        let fixture = Fixture::new();
        let published = fixture.published_request();

        let prompt = published
            .request()
            .render_prompt(published.submitted_result_path());

        let workspace_line = format!(
            "Mutable candidate workspace: {}",
            fixture
                .prototype_root
                .join("workspaces/edit-harness/parent-node-7")
                .display()
        );
        assert!(prompt.contains(&workspace_line));
        assert!(prompt.contains(
            "Authority boundary: admission=not claimed, grant=not claimed, child_plan=not claimed"
        ));
        let output_line = format!(
            "Write the typed submitted-result evidence to {}.",
            fixture.submitted_result_path.display()
        );
        assert!(prompt.contains(&output_line));
        assert!(!prompt.contains("Write the resulting child plan"));
    }

    #[test]
    fn repeated_publication_for_same_parent_gets_request_scoped_identity() {
        let fixture = Fixture::new();
        let first = fixture.published_request();
        fs::write(first.prompt_path(), "published prompt").expect("write first prompt");

        let second = fixture.published_request();

        assert_eq!(first.request_id(), "broad-harness-request:parent-node-7");
        assert_eq!(
            second.request_id(),
            "broad-harness-request:parent-node-7:r2"
        );
        assert_eq!(
            second.request_path(),
            fixture
                .prototype_root
                .join("messages/edit-harness-request/parent-node-7-r2.json")
                .as_path()
        );
        assert_ne!(first.request_hash(), second.request_hash());
        assert_ne!(first.workspace_path(), second.workspace_path());
        assert_ne!(
            first.submitted_result_path(),
            second.submitted_result_path()
        );
        assert_eq!(
            second.prompt_path(),
            fixture
                .prototype_root
                .join("messages/edit-harness-request/parent-node-7-r2.md")
                .as_path()
        );
        assert_eq!(
            second.submitted_result_path(),
            fixture
                .prototype_root
                .join("messages/edit-harness-result/parent-node-7-r2.json")
                .as_path()
        );
        assert_eq!(
            second.workspace_path(),
            fixture
                .prototype_root
                .join("workspaces/edit-harness/parent-node-7-r2")
                .as_path()
        );
    }

    #[test]
    fn request_json_alone_reserves_publication_identity() {
        let fixture = Fixture::new();
        fs::write(&fixture.request_path, "existing request").expect("write existing request");

        let published = fixture.published_request();

        assert_eq!(
            published.request_id(),
            "broad-harness-request:parent-node-7:r2"
        );
        assert_eq!(
            published.request_path(),
            fixture
                .prototype_root
                .join("messages/edit-harness-request/parent-node-7-r2.json")
                .as_path()
        );
    }

    #[test]
    fn admission_binding_round_trips_through_published_request() {
        let fixture = Fixture::new();
        let binding =
            fixture.request_admission_binding("artifact:broad-base", "policy:broad-boundary");
        let published = fixture
            .published_request()
            .with_admission_binding(binding.clone());

        let json =
            serde_json::to_string(&published).expect("serialize published broad harness request");
        let decoded = serde_json::from_str::<PublishedBroadHarnessRequest>(&json)
            .expect("deserialize published broad harness request");

        assert_eq!(decoded.admission_binding(), &binding);
        assert_eq!(
            decoded.admission_binding().base_artifact_id(),
            &ArtifactId::new("artifact:broad-base")
        );
        assert_eq!(
            decoded.admission_binding().policy_id().as_str(),
            "policy:broad-boundary"
        );
    }

    #[test]
    fn published_request_rejects_missing_admission_binding() {
        let fixture = Fixture::new();
        let published = fixture.published_request();
        let mut json = serde_json::to_value(&published).expect("serialize published request");

        json.as_object_mut()
            .expect("published request is a JSON object")
            .remove("admission_binding");

        assert!(
            serde_json::from_value::<PublishedBroadHarnessRequest>(json).is_err(),
            "missing authority binding should not deserialize"
        );
    }

    #[test]
    fn request_hash_changes_when_admission_binding_changes() {
        let fixture = Fixture::new();
        let published = fixture.published_request();
        let first = published.clone().with_admission_binding(
            fixture.request_admission_binding("artifact:broad-base", "policy:first"),
        );
        let second = published.with_admission_binding(
            fixture.request_admission_binding("artifact:other-base", "policy:first"),
        );

        assert_ne!(first.request_hash(), second.request_hash());
    }
}
