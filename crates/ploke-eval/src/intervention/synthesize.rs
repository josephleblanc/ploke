use ploke_core::tool_types::ToolName;

use crate::intervention::issue::{IssueCase, IssueSelectionBasis};
use crate::intervention::spec::{
    ArtifactEdit, InterventionSpec, InterventionSynthesisInput, InterventionSynthesisOutput,
    ValidationPolicy,
};

pub fn synthesize_intervention(
    input: &InterventionSynthesisInput,
) -> Option<InterventionSynthesisOutput> {
    let issue = &input.issue;
    match issue.selection_basis {
        IssueSelectionBasis::ProtocolReviewedIssueCalls => Some(InterventionSynthesisOutput {
            selected_spec: synthesize_reviewed_tool_target(issue),
        }),
    }
}

fn synthesize_reviewed_tool_target(issue: &IssueCase) -> InterventionSpec {
    let tool = issue.target_tool;

    let tool_name = tool.as_str();
    let marker = format!("Protocol review targeting note for `{tool_name}`.");
    let mut validation_policy = ValidationPolicy::for_tool_description_target(tool);
    validation_policy
        .require_markers_after_apply
        .push(marker.clone());

    InterventionSpec::ToolGuidanceMutation {
        spec_id: format!("reviewed-tool:{}", tool_name),
        evidence_basis: format!(
            "protocol-reviewed tool target {} (issue_calls={}, reviewed_calls={}, nearby_segment_labels={:?}, concerns={:?})",
            tool_name,
            issue.evidence.reviewed_issue_call_count,
            issue.evidence.reviewed_call_count,
            issue.evidence.protocol.nearby_segment_labels,
            issue.evidence.protocol.candidate_concerns
        ),
        intended_effect: format!(
            "reduce repeated reviewed-issue calls involving {}",
            tool_name
        ),
        tool,
        edit: ArtifactEdit::AppendText {
            text: protocol_review_guidance_block(tool),
        },
        validation_policy,
    }
}

fn protocol_review_guidance_block(tool: ToolName) -> String {
    let tool_name = tool.as_str();
    format!(
        "\n\nProtocol review targeting note for `{tool_name}`.\n- This tool appeared repeatedly in protocol-reviewed issue calls.\n- Before repeating `{tool_name}` in the same local context, reassess whether it is still the right tool.\n- If protocol review concerns recur, gather more context or switch to a more appropriate tool instead of repeating the same call pattern.\n"
    )
}
