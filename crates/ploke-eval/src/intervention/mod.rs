//! Prototype-local intervention execution shim.
//!
//! This module currently owns a narrow local edit path for intervention
//! experiments inside `ploke-eval`. It intentionally does not define the
//! long-term shared editing substrate for the project. The staged/apply
//! lifecycle below is an implementation detail for Prototype 1 and should be
//! treated as replaceable once intervention execution is rewired to delegate to
//! `ploke-tui`.

mod apply;
mod branch_registry;
mod execute;
mod issue;
mod scheduler;
mod spec;
mod synthesize;

pub use apply::{INTERVENTION_APPLY_PROCEDURE, execute_intervention_apply};
pub use branch_registry::{
    ActiveBranchSelection, ActiveInterventionTarget, InterventionRestoreOutput,
    InterventionSourceNode, PROTOTYPE1_BRANCH_REGISTRY_SCHEMA_VERSION, Prototype1BranchRegistry,
    ResolvedTreatmentBranch, TreatmentBranchEvaluationSummary, TreatmentBranchNode,
    TreatmentBranchStatus, active_branch_selection_for_target, load_or_default_branch_registry,
    mark_treatment_branch_applied, prototype1_branch_registry_path, record_synthesized_branches,
    record_treatment_branch_evaluation, resolve_treatment_branch, restore_treatment_branch,
    select_treatment_branch, treatment_branch_id,
};
pub use execute::execute_tool_text_intervention;
pub use issue::{
    INTERVENTION_ISSUE_DETECTION_PROCEDURE, IssueCase, IssueDetectionArtifactInput,
    IssueDetectionInput, IssueDetectionOutput, IssueEvidence, IssueProtocolEvidence,
    IssueSelectionBasis, detect_issue_cases, issue_detection_artifact_input, select_primary_issue,
};
pub use scheduler::{
    PROTOTYPE1_SCHEDULER_SCHEMA_VERSION, PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION,
    Prototype1NodeRecord, Prototype1NodeStatus, Prototype1RunnerRequest, Prototype1SchedulerState,
    load_node_record, load_or_default_scheduler_state, load_runner_request, prototype1_node_dir,
    prototype1_node_id, prototype1_node_record_path, prototype1_runner_request_path,
    prototype1_runner_result_path, prototype1_scheduler_path, register_treatment_evaluation_node,
};
pub use spec::{
    AppliedEdit, ArtifactEdit, InterventionApplyInput, InterventionApplyOutput,
    InterventionCandidate, InterventionCandidateSet, InterventionExecutionInput,
    InterventionExecutionOutput, InterventionKind, InterventionSpec, InterventionSpecError,
    InterventionSynthesisInput, InterventionSynthesisOutput, TreatmentStateRef, ValidationPolicy,
    ValidationResult,
};
pub(crate) use synthesize::synthesize_intervention_with_llm;
pub use synthesize::{INTERVENTION_SYNTHESIS_PROCEDURE, synthesize_intervention};

#[cfg(test)]
mod tests;
