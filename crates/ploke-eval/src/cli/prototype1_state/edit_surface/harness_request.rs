use std::{
    fmt,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct BroadHarnessRequest {
    pub(crate) schema: BroadHarnessRequestSchema,
    pub(crate) parent_node_id: ParentNodeRef,
    pub(crate) target_repository: TargetRepository,
    pub(crate) edit_policy: BroadEditPolicy,
    pub(crate) child_budget: HarnessChildBudget,
    pub(crate) protected_core: ProtectedCorePointer,
    pub(crate) evaluation: EvaluationBrief,
    pub(crate) evidence_roots: Vec<EvidenceRoot>,
    pub(crate) instructions: Vec<HarnessInstruction>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BroadHarnessRequestSchema {
    V1,
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

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(transparent)]
pub(crate) struct TargetRepository(PathBuf);

impl TargetRepository {
    fn new(path: PathBuf) -> Self {
        Self(path)
    }

    fn display(&self) -> impl fmt::Display + '_ {
        self.0.display()
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
    ChildPlanOutput,
    HistoryBlocks,
    Evaluations,
    Nodes,
    ProtocolArtifacts,
    Oracle,
}

impl EvidenceRootKind {
    fn label(self) -> &'static str {
        match self {
            Self::ChildPlanOutput => "child plan output",
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
    WriteChildPlan,
}

impl HarnessInstruction {
    fn render(self, child_plan_path: &Path) -> String {
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
            Self::WriteChildPlan => format!(
                "Write the resulting child plan to {}.",
                child_plan_path.display()
            ),
        }
    }
}

impl BroadHarnessRequest {
    pub(crate) fn prototype1_workspace(
        parent_node_id: String,
        target_repository: PathBuf,
        child_budget: HarnessChildBudget,
        prototype_root: &Path,
        child_plan_path: &Path,
    ) -> Self {
        Self {
            schema: BroadHarnessRequestSchema::V1,
            parent_node_id: ParentNodeRef::new(parent_node_id),
            target_repository: TargetRepository::new(target_repository),
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
                    kind: EvidenceRootKind::ChildPlanOutput,
                    location: EvidenceRootLocation::File {
                        path: child_plan_path.to_path_buf(),
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
            instructions: vec![
                HarnessInstruction::InspectRepositoryAndEvidence,
                HarnessInstruction::TreatProtocolDiagnosticsAsGuidance,
                HarnessInstruction::EditBroadSurfaceOutsideProtectedCore,
                HarnessInstruction::ChooseLikelyDescendantImprovement,
                HarnessInstruction::WriteChildPlan,
            ],
        }
    }

    pub(crate) fn render_prompt(&self, child_plan_path: &Path) -> String {
        let mut prompt = String::new();
        prompt.push_str("# Prototype 1 broad edit harness request\n\n");
        prompt.push_str(&format!("Parent node: {}\n", self.parent_node_id.as_str()));
        prompt.push_str(&format!(
            "Repository: {}\n",
            self.target_repository.display()
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

        prompt.push_str("## Instructions\n\n");
        for instruction in &self.instructions {
            prompt.push_str(&format!("- {}\n", instruction.render(child_plan_path)));
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
