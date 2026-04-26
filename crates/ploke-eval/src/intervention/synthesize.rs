use std::collections::BTreeSet;
use std::convert::Infallible;
use std::path::PathBuf;

use ploke_core::tool_types::ToolName;
use ploke_protocol::llm::{
    JsonAdjudicationSpec, JsonAdjudicator, JsonChatPrompt, JsonLlmConfig, ProtocolLlmError,
};
use ploke_protocol::procedure::{
    FanOutError, MergeError, NamedProcedure, ObservedSubrequest, Procedure, ProcedureExt,
    SequenceError, SubrequestDescriptor,
};
use ploke_protocol::step::{MechanizedExecutor, MechanizedSpec, Step, StepSpec};
use serde::{Deserialize, Serialize};

use crate::intervention::issue::{IssueCase, IssueSelectionBasis};
use crate::intervention::spec::{
    ArtifactEdit, InterventionCandidate, InterventionCandidateSet, InterventionSpec,
    InterventionSynthesisInput, InterventionSynthesisOutput, ValidationPolicy,
    text_replacement_patch_id,
};

pub const INTERVENTION_SYNTHESIS_PROCEDURE: &str = "intervention_synthesis";

pub fn synthesize_intervention(
    input: &InterventionSynthesisInput,
) -> Option<InterventionSynthesisOutput> {
    let issue = &input.issue;
    match issue.selection_basis {
        IssueSelectionBasis::ProtocolReviewedIssueCalls => {
            Some(synthesize_reviewed_tool_target(input))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InterventionSynthesisContext {
    pub issue: IssueCase,
    pub source_state_id: String,
    pub source_content: String,
    pub target_relpath: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) operation_target: Option<crate::loop_graph::OperationTarget>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SynthesizedInterventionDraft {
    pub proposed_content: String,
    // Keep these as strings for Prototype 1, but later we likely want a small
    // enum or other typed label space here so cross-run aggregation can use
    // machine-readable categories instead of reading freeform reasoning.
    pub intended_effect: String,
    // Same direction as `intended_effect`: this is useful operator/debug text
    // right now, but long-term high-level summaries should not depend on
    // mining freeform rationale strings.
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, Default)]
struct ContextualizeInterventionSynthesis;

impl StepSpec for ContextualizeInterventionSynthesis {
    type InputState = InterventionSynthesisInput;
    type OutputState = InterventionSynthesisContext;

    fn step_id(&self) -> &'static str {
        "contextualize_intervention_synthesis"
    }

    fn step_name(&self) -> &'static str {
        "contextualize_intervention_synthesis"
    }
}

impl MechanizedSpec for ContextualizeInterventionSynthesis {
    type Error = Infallible;

    fn execute_mechanized(
        &self,
        input: Self::InputState,
    ) -> Result<Self::OutputState, Self::Error> {
        Ok(InterventionSynthesisContext {
            target_relpath: PathBuf::from(input.issue.target_tool.description_artifact_relpath()),
            issue: input.issue,
            source_state_id: input.source_state_id,
            source_content: input.source_content,
            operation_target: input.operation_target,
        })
    }
}

#[derive(Debug, Clone, Copy)]
struct ProposeToolGuidanceRewrite {
    step_id: &'static str,
    step_name: &'static str,
    branch_label: &'static str,
    strategy: &'static str,
}

impl StepSpec for ProposeToolGuidanceRewrite {
    type InputState = InterventionSynthesisContext;
    type OutputState = SynthesizedInterventionDraft;

    fn step_id(&self) -> &'static str {
        self.step_id
    }

    fn step_name(&self) -> &'static str {
        self.step_name
    }
}

impl JsonAdjudicationSpec for ProposeToolGuidanceRewrite {
    fn build_prompt(&self, input: &Self::InputState) -> JsonChatPrompt {
        let tool_name = input.issue.target_tool.as_str();
        let user = format!(
            "You are generating a treatment-branch candidate rewrite for the tool description file `{}`.\n\
The intervention target has already been selected programmatically. Do not choose a different tool.\n\
\n\
Branch label: {}\n\
Rewrite strategy: {}\n\
\n\
Protocol evidence summary:\n\
- target_tool: {}\n\
- reviewed_issue_calls: {}\n\
- reviewed_calls: {}\n\
- nearby_segment_labels: {:?}\n\
- candidate_concerns: {:?}\n\
\n\
Return JSON with exactly these string fields:\n\
- proposed_content\n\
- intended_effect\n\
- rationale\n\
\n\
Requirements:\n\
- `proposed_content` must be the full replacement text for the file, not a patch and not an appended note.\n\
- Keep the rewrite bounded to this one file.\n\
- Preserve the basic purpose of the tool while making the description more robust against the reviewed issue pattern.\n\
- Do not mention branch labels or internal protocol machinery in the rewritten file text.\n\
\n\
Current file contents:\n```md\n{}\n```",
            input.target_relpath.display(),
            self.branch_label,
            self.strategy,
            tool_name,
            input.issue.evidence.reviewed_issue_call_count,
            input.issue.evidence.reviewed_call_count,
            input.issue.evidence.protocol.nearby_segment_labels,
            input.issue.evidence.protocol.candidate_concerns,
            input.source_content
        );

        JsonChatPrompt {
            system: "Return valid JSON only. You are proposing one candidate full replacement text for a bounded tool-description artifact.".to_string(),
            user,
        }
    }
}

type DraftPair = ploke_protocol::core::ForkState<
    InterventionSynthesisContext,
    SynthesizedInterventionDraft,
    SynthesizedInterventionDraft,
>;
type DraftTriplet = ploke_protocol::core::ForkState<
    InterventionSynthesisContext,
    DraftPair,
    SynthesizedInterventionDraft,
>;

#[derive(Debug, Clone, Copy, Default)]
struct AssembleInterventionCandidates;

impl StepSpec for AssembleInterventionCandidates {
    type InputState = DraftTriplet;
    type OutputState = InterventionSynthesisOutput;

    fn step_id(&self) -> &'static str {
        "assemble_intervention_candidates"
    }

    fn step_name(&self) -> &'static str {
        "assemble_intervention_candidates"
    }
}

impl MechanizedSpec for AssembleInterventionCandidates {
    type Error = Infallible;

    fn execute_mechanized(
        &self,
        input: Self::InputState,
    ) -> Result<Self::OutputState, Self::Error> {
        let context = input.source;
        let minimal = input.left.left;
        let decision_rule = input.left.right;
        let stronger = input.right;

        let mut seen = BTreeSet::new();
        let mut candidates = Vec::new();
        for (candidate_id, branch_label, draft) in [
            ("candidate-1", "minimal_rewrite", minimal),
            ("candidate-2", "decision_rule_rewrite", decision_rule),
            ("candidate-3", "stronger_rewrite", stronger),
        ] {
            if !seen.insert(draft.proposed_content.clone()) {
                continue;
            }
            candidates.push(build_candidate_from_draft(
                &context,
                candidate_id,
                branch_label,
                draft,
            ));
        }

        Ok(InterventionSynthesisOutput {
            candidate_set: InterventionCandidateSet {
                source_state_id: context.source_state_id,
                target_relpath: context.target_relpath,
                source_content: context.source_content,
                candidates,
                operation_target: context.operation_target,
            },
        })
    }
}

fn build_candidate_from_draft(
    context: &InterventionSynthesisContext,
    candidate_id: &str,
    branch_label: &str,
    draft: SynthesizedInterventionDraft,
) -> InterventionCandidate {
    let tool = context.issue.target_tool;
    let validation_policy = ValidationPolicy::for_tool_description_target(tool);
    let tool_name = tool.as_str();
    let spec = InterventionSpec::ToolGuidanceMutation {
        spec_id: format!("reviewed-tool:{}:{}", tool_name, branch_label),
        evidence_basis: format!(
            "protocol-reviewed tool target {} (issue_calls={}, reviewed_calls={}, nearby_segment_labels={:?}, concerns={:?}, source_state_id={}, branch_label={}, rationale={})",
            tool_name,
            context.issue.evidence.reviewed_issue_call_count,
            context.issue.evidence.reviewed_call_count,
            context.issue.evidence.protocol.nearby_segment_labels,
            context.issue.evidence.protocol.candidate_concerns,
            context.source_state_id,
            branch_label,
            draft.rationale
        ),
        intended_effect: draft.intended_effect.clone(),
        tool,
        edit: ArtifactEdit::ReplaceWholeText {
            new_text: draft.proposed_content.clone(),
        },
        validation_policy,
    };

    InterventionCandidate {
        candidate_id: candidate_id.to_string(),
        branch_label: branch_label.to_string(),
        patch_id: Some(text_replacement_patch_id(
            &context.target_relpath,
            &context.source_content,
            &draft.proposed_content,
        )),
        proposed_content: draft.proposed_content,
        spec,
    }
}

type InterventionSynthesisInner = ploke_protocol::procedure::Sequence<
    Step<ContextualizeInterventionSynthesis, MechanizedExecutor>,
    ploke_protocol::procedure::Merge<
        ploke_protocol::procedure::FanOut<
            ploke_protocol::procedure::FanOut<
                ObservedSubrequest<Step<ProposeToolGuidanceRewrite, JsonAdjudicator>>,
                ObservedSubrequest<Step<ProposeToolGuidanceRewrite, JsonAdjudicator>>,
            >,
            ObservedSubrequest<Step<ProposeToolGuidanceRewrite, JsonAdjudicator>>,
        >,
        Step<AssembleInterventionCandidates, MechanizedExecutor>,
    >,
>;

type InterventionSynthesisArtifact =
    ploke_protocol::core::ProcedureArtifact<<InterventionSynthesisInner as Procedure>::Artifact>;
type InterventionSynthesisError = SequenceError<
    Infallible,
    MergeError<
        FanOutError<FanOutError<ProtocolLlmError, ProtocolLlmError>, ProtocolLlmError>,
        Infallible,
    >,
>;

#[derive(Debug, Clone)]
struct InterventionSynthesisProcedure {
    inner: NamedProcedure<InterventionSynthesisInner>,
}

impl InterventionSynthesisProcedure {
    fn new(adjudicator: JsonAdjudicator) -> Self {
        let context = Step::new(ContextualizeInterventionSynthesis, MechanizedExecutor);
        let minimal = ObservedSubrequest::new(
            Step::new(
                ProposeToolGuidanceRewrite {
                    step_id: "propose_minimal_tool_guidance_rewrite",
                    step_name: "propose_minimal_tool_guidance_rewrite",
                    branch_label: "minimal_rewrite",
                    strategy: "Make the smallest coherent full rewrite that clarifies when to stop repeating the tool and gather more context.",
                },
                adjudicator.clone(),
            ),
            SubrequestDescriptor {
                label: "minimal_rewrite",
                request_index: 1,
                request_total: 3,
            },
        );
        let decision_rule = ObservedSubrequest::new(
            Step::new(
                ProposeToolGuidanceRewrite {
                    step_id: "propose_decision_rule_tool_guidance_rewrite",
                    step_name: "propose_decision_rule_tool_guidance_rewrite",
                    branch_label: "decision_rule_rewrite",
                    strategy: "Rewrite the description to include explicit decision rules and stop conditions before repeating the tool in the same local context.",
                },
                adjudicator.clone(),
            ),
            SubrequestDescriptor {
                label: "decision_rule_rewrite",
                request_index: 2,
                request_total: 3,
            },
        );
        let stronger = ObservedSubrequest::new(
            Step::new(
                ProposeToolGuidanceRewrite {
                    step_id: "propose_stronger_tool_guidance_rewrite",
                    step_name: "propose_stronger_tool_guidance_rewrite",
                    branch_label: "stronger_rewrite",
                    strategy: "Produce a stronger full rewrite that more aggressively narrows when this tool should be used and emphasizes alternatives when repeated issue patterns recur.",
                },
                adjudicator,
            ),
            SubrequestDescriptor {
                label: "stronger_rewrite",
                request_index: 3,
                request_total: 3,
            },
        );
        let assemble = Step::new(AssembleInterventionCandidates, MechanizedExecutor);

        Self {
            inner: context
                .then(
                    minimal
                        .fan_out_named("minimal_rewrite", "decision_rule_rewrite", decision_rule)
                        .fan_out_named("candidate_pair", "stronger_rewrite", stronger)
                        .merge(assemble),
                )
                .named(INTERVENTION_SYNTHESIS_PROCEDURE),
        }
    }

    async fn run(
        &self,
        subject: InterventionSynthesisInput,
    ) -> Result<
        ploke_protocol::core::ProcedureRun<
            InterventionSynthesisOutput,
            InterventionSynthesisArtifact,
        >,
        InterventionSynthesisError,
    > {
        self.inner.run(subject).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RecordedInterventionSynthesisRun {
    pub procedure_name: String,
    pub output: InterventionSynthesisOutput,
    pub artifact: serde_json::Value,
}

pub(crate) async fn synthesize_intervention_with_llm(
    input: InterventionSynthesisInput,
    cfg: JsonLlmConfig,
) -> Result<RecordedInterventionSynthesisRun, String> {
    let client = reqwest::Client::new();
    let procedure = InterventionSynthesisProcedure::new(JsonAdjudicator::new(client, cfg));
    let run = procedure.run(input).await.map_err(|err| err.to_string())?;
    let artifact =
        serde_json::to_value(&run.artifact).map_err(|err| format!("serialize artifact: {err}"))?;
    Ok(RecordedInterventionSynthesisRun {
        procedure_name: run.procedure_name,
        output: run.output,
        artifact,
    })
}

fn synthesize_reviewed_tool_target(
    input: &InterventionSynthesisInput,
) -> InterventionSynthesisOutput {
    let issue = &input.issue;
    let tool = issue.target_tool;

    let tool_name = tool.as_str();
    let marker = format!("Protocol review targeting rewrite for `{tool_name}`.");
    let mut validation_policy = ValidationPolicy::for_tool_description_target(tool);
    validation_policy
        .require_markers_after_apply
        .push(marker.clone());

    let proposed_content = protocol_review_guidance_rewrite(input.source_content.trim_end(), tool);
    let spec = InterventionSpec::ToolGuidanceMutation {
        spec_id: format!("reviewed-tool:{}:candidate-1", tool_name),
        evidence_basis: format!(
            "protocol-reviewed tool target {} (issue_calls={}, reviewed_calls={}, nearby_segment_labels={:?}, concerns={:?}, source_state_id={})",
            tool_name,
            issue.evidence.reviewed_issue_call_count,
            issue.evidence.reviewed_call_count,
            issue.evidence.protocol.nearby_segment_labels,
            issue.evidence.protocol.candidate_concerns,
            input.source_state_id
        ),
        intended_effect: format!(
            "replace tool guidance for {} with a revised description intended to reduce repeated reviewed-issue calls",
            tool_name
        ),
        tool,
        edit: ArtifactEdit::ReplaceWholeText {
            new_text: proposed_content.clone(),
        },
        validation_policy,
    };

    InterventionSynthesisOutput {
        candidate_set: InterventionCandidateSet {
            source_state_id: input.source_state_id.clone(),
            target_relpath: spec.target_relpath().to_path_buf(),
            source_content: input.source_content.clone(),
            operation_target: input.operation_target.clone(),
            candidates: vec![InterventionCandidate {
                candidate_id: "candidate-1".to_string(),
                branch_label: "rewrite_v1".to_string(),
                patch_id: Some(text_replacement_patch_id(
                    spec.target_relpath(),
                    &input.source_content,
                    &proposed_content,
                )),
                proposed_content,
                spec,
            }],
        },
    }
}

fn protocol_review_guidance_rewrite(source_content: &str, tool: ToolName) -> String {
    let tool_name = tool.as_str();
    let base = if source_content.is_empty() {
        format!("# {tool_name}\n")
    } else {
        source_content.to_string()
    };
    format!(
        "{base}\n\nProtocol review targeting rewrite for `{tool_name}`.\n- This description is being revised as a treatment-branch candidate.\n- Protocol review found repeated issue-bearing calls involving `{tool_name}`.\n- The intended effect is to make the tool choice threshold stricter before repeating this tool in the same local context.\n"
    )
}
