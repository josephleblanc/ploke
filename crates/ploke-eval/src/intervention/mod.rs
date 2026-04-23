//! Prototype-local intervention execution shim.
//!
//! This module currently owns a narrow local edit path for intervention
//! experiments inside `ploke-eval`. It intentionally does not define the
//! long-term shared editing substrate for the project. The staged/apply
//! lifecycle below is an implementation detail for Prototype 1 and should be
//! treated as replaceable once intervention execution is rewired to delegate to
//! `ploke-tui`.

mod execute;
mod issue;
mod spec;
mod synthesize;

pub use execute::execute_tool_text_intervention;
pub use issue::{
    INTERVENTION_ISSUE_DETECTION_PROCEDURE, IssueCase, IssueDetectionArtifactInput,
    IssueDetectionInput, IssueDetectionOutput, IssueEvidence, IssueProtocolEvidence,
    IssueSelectionBasis, detect_issue_cases, issue_detection_artifact_input, select_primary_issue,
};
pub use spec::{
    AppliedEdit, ArtifactEdit, InterventionExecutionInput, InterventionExecutionOutput,
    InterventionKind, InterventionSpec, InterventionSpecError, InterventionSynthesisInput,
    InterventionSynthesisOutput, ValidationPolicy, ValidationResult,
};
pub use synthesize::synthesize_intervention;

#[cfg(test)]
mod tests;
