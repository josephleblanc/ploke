//! Typed passive payloads for known protocol artifact procedures.

use ploke_protocol::{
    FanOutArtifact, ForkState, LocalAnalysisAssessment, LocalAnalysisContext, ProcedureArtifact,
    SegmentReviewSubject, SegmentationJudgment, SegmentedToolCallSequence, SequenceArtifact,
    SequenceReviewContext, StepArtifact, ToolCallNeighborhood, ToolCallSequence,
};
use serde::{Deserialize, Serialize};

use super::provenance::{JsonLlmProvenanceMirror, MechanizedProvenanceMirror};

pub type IntentSegmentationArtifactMirror = ProcedureArtifact<
    SequenceArtifact<
        StepArtifact<ToolCallSequence, SequenceReviewContext, MechanizedProvenanceMirror>,
        ploke_protocol::MergeArtifact<
            FanOutArtifact<
                SequenceReviewContext,
                StepArtifact<
                    SequenceReviewContext,
                    SequenceReviewContext,
                    MechanizedProvenanceMirror,
                >,
                StepArtifact<SequenceReviewContext, SegmentationJudgment, JsonLlmProvenanceMirror>,
            >,
            StepArtifact<
                ForkState<SequenceReviewContext, SequenceReviewContext, SegmentationJudgment>,
                SegmentedToolCallSequence,
                MechanizedProvenanceMirror,
            >,
        >,
    >,
>;

pub type ToolCallReviewArtifactMirror = ProcedureArtifact<
    SequenceArtifact<
        StepArtifact<ToolCallNeighborhood, LocalAnalysisContext, MechanizedProvenanceMirror>,
        ploke_protocol::MergeArtifact<
            FanOutArtifact<
                LocalAnalysisContext,
                FanOutArtifact<
                    LocalAnalysisContext,
                    StepArtifact<
                        LocalAnalysisContext,
                        ploke_protocol::UsefulnessAssessment,
                        JsonLlmProvenanceMirror,
                    >,
                    StepArtifact<
                        LocalAnalysisContext,
                        ploke_protocol::RedundancyAssessment,
                        JsonLlmProvenanceMirror,
                    >,
                >,
                StepArtifact<
                    LocalAnalysisContext,
                    ploke_protocol::RecoverabilityAssessment,
                    JsonLlmProvenanceMirror,
                >,
            >,
            StepArtifact<
                ForkState<
                    LocalAnalysisContext,
                    ForkState<
                        LocalAnalysisContext,
                        ploke_protocol::UsefulnessAssessment,
                        ploke_protocol::RedundancyAssessment,
                    >,
                    ploke_protocol::RecoverabilityAssessment,
                >,
                LocalAnalysisAssessment,
                MechanizedProvenanceMirror,
            >,
        >,
    >,
>;

pub type ToolCallSegmentReviewArtifactMirror = ProcedureArtifact<
    SequenceArtifact<
        StepArtifact<SegmentReviewSubject, LocalAnalysisContext, MechanizedProvenanceMirror>,
        ploke_protocol::MergeArtifact<
            FanOutArtifact<
                LocalAnalysisContext,
                FanOutArtifact<
                    LocalAnalysisContext,
                    StepArtifact<
                        LocalAnalysisContext,
                        ploke_protocol::UsefulnessAssessment,
                        JsonLlmProvenanceMirror,
                    >,
                    StepArtifact<
                        LocalAnalysisContext,
                        ploke_protocol::RedundancyAssessment,
                        JsonLlmProvenanceMirror,
                    >,
                >,
                StepArtifact<
                    LocalAnalysisContext,
                    ploke_protocol::RecoverabilityAssessment,
                    JsonLlmProvenanceMirror,
                >,
            >,
            StepArtifact<
                ForkState<
                    LocalAnalysisContext,
                    ForkState<
                        LocalAnalysisContext,
                        ploke_protocol::UsefulnessAssessment,
                        ploke_protocol::RedundancyAssessment,
                    >,
                    ploke_protocol::RecoverabilityAssessment,
                >,
                LocalAnalysisAssessment,
                MechanizedProvenanceMirror,
            >,
        >,
    >,
>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct IntentSegmentationPayload {
    pub input: ToolCallSequence,
    pub output: SegmentedToolCallSequence,
    pub artifact: IntentSegmentationArtifactMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallReviewPayload {
    pub input: ToolCallNeighborhood,
    pub output: LocalAnalysisAssessment,
    pub artifact: ToolCallReviewArtifactMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ToolCallSegmentReviewPayload {
    pub input: SegmentReviewSubject,
    pub output: LocalAnalysisAssessment,
    pub artifact: ToolCallSegmentReviewArtifactMirror,
}
