//! Passive persisted protocol artifact DTOs.
//!
//! These records mirror persisted files under run-local
//! `protocol-artifacts/*.json` directories.
//! They do not evaluate procedures or enforce runtime authority.

use std::path::PathBuf;

use ploke_protocol::{
    FanOutArtifact, ForkState, LocalAnalysisAssessment, LocalAnalysisContext, ProcedureArtifact,
    SegmentReviewSubject, SegmentationJudgment, SegmentedToolCallSequence, SequenceArtifact,
    SequenceReviewContext, StepArtifact, ToolCallNeighborhood, ToolCallSequence,
};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

pub const SCHEMA_V1: &str = "protocol-artifact.v1";
pub const TOOL_CALL_INTENT_SEGMENTATION: &str = "tool_call_intent_segmentation";
pub const TOOL_CALL_REVIEW: &str = "tool_call_review";
pub const TOOL_CALL_SEGMENT_REVIEW: &str = "tool_call_segment_review";

#[derive(Debug, Clone, PartialEq)]
pub struct Artifact {
    pub schema_version: String,
    pub procedure_name: String,
    pub subject_id: String,
    pub run_id: String,
    pub created_at_ms: u64,
    pub model_id: Option<String>,
    pub provider_slug: Option<String>,
    pub body: ArtifactBody,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ArtifactFile {
    pub path: PathBuf,
    pub artifact: Artifact,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MechanizedProvenanceMirror {
    pub strategy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct JsonLlmProvenanceMirror {
    pub model_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider_slug: Option<String>,
    pub raw_content: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<String>,
    pub response: OpenAiResponseMirror,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OpenAiResponseMirror {
    #[serde(default)]
    pub id: String,
    #[serde(default)]
    pub choices: Vec<ChoiceMirror>,
    #[serde(default)]
    pub created: i64,
    #[serde(default)]
    pub model: String,
    #[serde(default)]
    pub object: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub system_fingerprint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub usage: Option<TokenUsageMirror>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ChoiceMirror {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub native_finish_reason: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub index: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<ResponseMessageMirror>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ResponseMessageMirror {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub content: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct TokenUsageMirror {
    pub prompt_tokens: u32,
    pub completion_tokens: u32,
    pub total_tokens: u32,
}

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

#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactBody {
    ToolCallIntentSegmentation(IntentSegmentationPayload),
    ToolCallReview(ToolCallReviewPayload),
    ToolCallSegmentReview(ToolCallSegmentReviewPayload),
}

impl Artifact {
    pub fn body(&self) -> Option<ArtifactBody> {
        Some(self.body.clone())
    }

    pub fn typed_payload(&self) -> Option<ArtifactBody> {
        self.body()
    }
}

impl Serialize for Artifact {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut state = serializer.serialize_struct("Artifact", 11)?;
        state.serialize_field("schema_version", &self.schema_version)?;
        state.serialize_field("procedure_name", &self.procedure_name)?;
        state.serialize_field("subject_id", &self.subject_id)?;
        state.serialize_field("run_id", &self.run_id)?;
        state.serialize_field("created_at_ms", &self.created_at_ms)?;
        if let Some(model_id) = &self.model_id {
            state.serialize_field("model_id", model_id)?;
        }
        if let Some(provider_slug) = &self.provider_slug {
            state.serialize_field("provider_slug", provider_slug)?;
        }
        match &self.body {
            ArtifactBody::ToolCallIntentSegmentation(payload) => {
                state.serialize_field("input", &payload.input)?;
                state.serialize_field("output", &payload.output)?;
                state.serialize_field("artifact", &payload.artifact)?;
            }
            ArtifactBody::ToolCallReview(payload) => {
                state.serialize_field("input", &payload.input)?;
                state.serialize_field("output", &payload.output)?;
                state.serialize_field("artifact", &payload.artifact)?;
            }
            ArtifactBody::ToolCallSegmentReview(payload) => {
                state.serialize_field("input", &payload.input)?;
                state.serialize_field("output", &payload.output)?;
                state.serialize_field("artifact", &payload.artifact)?;
            }
        }
        state.end()
    }
}

impl<'de> Deserialize<'de> for Artifact {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawArtifact {
            schema_version: String,
            procedure_name: String,
            subject_id: String,
            run_id: String,
            created_at_ms: u64,
            #[serde(default)]
            model_id: Option<String>,
            #[serde(default)]
            provider_slug: Option<String>,
            input: serde_json::Value,
            output: serde_json::Value,
            artifact: serde_json::Value,
        }
        let raw = RawArtifact::deserialize(deserializer)?;
        let body = decode_payload_from_values(
            raw.procedure_name.as_str(),
            raw.input,
            raw.output,
            raw.artifact,
        )
        .map_err(serde::de::Error::custom)?;
        Ok(Self {
            schema_version: raw.schema_version,
            procedure_name: raw.procedure_name,
            subject_id: raw.subject_id,
            run_id: raw.run_id,
            created_at_ms: raw.created_at_ms,
            model_id: raw.model_id,
            provider_slug: raw.provider_slug,
            body,
        })
    }
}

fn decode_payload_from_values(
    procedure_name: &str,
    input: serde_json::Value,
    output: serde_json::Value,
    artifact: serde_json::Value,
) -> Result<ArtifactBody, String> {
    #[derive(Deserialize)]
    struct RawPayload<TIn, TOut, TArtifact> {
        input: TIn,
        output: TOut,
        artifact: TArtifact,
    }

    let payload_value = serde_json::json!({
        "input": input,
        "output": output,
        "artifact": artifact
    });
    match procedure_name {
        TOOL_CALL_INTENT_SEGMENTATION => serde_json::from_value::<
            RawPayload<
                ToolCallSequence,
                SegmentedToolCallSequence,
                IntentSegmentationArtifactMirror,
            >,
        >(payload_value)
        .map(|payload| {
            ArtifactBody::ToolCallIntentSegmentation(IntentSegmentationPayload {
                input: payload.input,
                output: payload.output,
                artifact: payload.artifact,
            })
        })
        .map_err(|err| format!("decode {TOOL_CALL_INTENT_SEGMENTATION}: {err}")),
        TOOL_CALL_REVIEW => serde_json::from_value::<
            RawPayload<ToolCallNeighborhood, LocalAnalysisAssessment, ToolCallReviewArtifactMirror>,
        >(payload_value)
        .map(|payload| {
            ArtifactBody::ToolCallReview(ToolCallReviewPayload {
                input: payload.input,
                output: payload.output,
                artifact: payload.artifact,
            })
        })
        .map_err(|err| format!("decode {TOOL_CALL_REVIEW}: {err}")),
        TOOL_CALL_SEGMENT_REVIEW => serde_json::from_value::<
            RawPayload<
                SegmentReviewSubject,
                LocalAnalysisAssessment,
                ToolCallSegmentReviewArtifactMirror,
            >,
        >(payload_value)
        .map(|payload| {
            ArtifactBody::ToolCallSegmentReview(ToolCallSegmentReviewPayload {
                input: payload.input,
                output: payload.output,
                artifact: payload.artifact,
            })
        })
        .map_err(|err| format!("decode {TOOL_CALL_SEGMENT_REVIEW}: {err}")),
        other => Err(format!("unknown procedure_name '{other}'")),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::fs;
    use std::path::{Path, PathBuf};

    use ploke_protocol::{Confidence, IntentLabel, OverallVerdict, SegmentStatus};

    use super::*;

    #[test]
    #[ignore]
    fn all_artifacts_roundtrip() {
        let dir = protocol_artifacts_dir();
        let mut paths = fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("read protocol-artifacts dir {}: {err}", dir.display()))
            .map(|entry| entry.expect("read protocol entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        paths.sort();

        let mut counts = BTreeMap::<String, usize>::new();

        for path in &paths {
            let original = read_json_value(path);
            let artifact: Artifact =
                serde_json::from_value(original.clone()).unwrap_or_else(|err| {
                    panic!("deserialize protocol artifact {}: {err}", path.display())
                });
            let serialized = serde_json::to_value(&artifact).unwrap_or_else(|err| {
                panic!("serialize protocol artifact {}: {err}", path.display())
            });
            assert_eq!(
                serialized,
                original,
                "round-trip mismatch for {}",
                path.display()
            );

            *counts.entry(artifact.procedure_name.clone()).or_default() += 1;
        }

        assert_eq!(paths.len(), 16, "unexpected protocol artifact count");
        assert_eq!(
            counts
                .get(TOOL_CALL_INTENT_SEGMENTATION)
                .copied()
                .unwrap_or(0),
            1
        );
        assert_eq!(counts.get(TOOL_CALL_REVIEW).copied().unwrap_or(0), 10);
        assert_eq!(
            counts.get(TOOL_CALL_SEGMENT_REVIEW).copied().unwrap_or(0),
            5
        );
    }

    #[test]
    #[ignore]
    fn typed_payloads() {
        let paths = protocol_artifact_paths();
        let segmentation = paths
            .iter()
            .find(|path| {
                path.to_string_lossy()
                    .contains(TOOL_CALL_INTENT_SEGMENTATION)
            })
            .expect("segmentation artifact path");
        let review = paths
            .iter()
            .find(|path| path.to_string_lossy().contains(TOOL_CALL_REVIEW))
            .expect("review artifact path");

        let segmentation_value = read_json_value(segmentation);
        let segmentation_artifact: Artifact = serde_json::from_value(segmentation_value)
            .unwrap_or_else(|err| {
                panic!("deserialize segmentation {}: {err}", segmentation.display())
            });

        assert_eq!(segmentation_artifact.schema_version, SCHEMA_V1);
        assert_eq!(
            segmentation_artifact.procedure_name,
            TOOL_CALL_INTENT_SEGMENTATION
        );
        let segmentation_payload = match segmentation_artifact.body() {
            Some(ArtifactBody::ToolCallIntentSegmentation(payload)) => payload,
            other => panic!("expected segmentation payload, got {other:?}"),
        };
        assert_eq!(segmentation_payload.input.total_calls_in_run, 10);
        assert_eq!(segmentation_payload.output.coverage.total_calls, 10);
        assert_eq!(segmentation_payload.output.segments.len(), 5);
        assert_eq!(
            segmentation_payload.artifact.procedure_name,
            TOOL_CALL_INTENT_SEGMENTATION
        );

        let review_value = read_json_value(review);
        let review_artifact: Artifact = serde_json::from_value(review_value)
            .unwrap_or_else(|err| panic!("deserialize review {}: {err}", review.display()));
        assert_eq!(review_artifact.procedure_name, TOOL_CALL_REVIEW);
        let review_payload = match review_artifact.typed_payload() {
            Some(ArtifactBody::ToolCallReview(payload)) => payload,
            other => panic!("expected review payload, got {other:?}"),
        };
        assert_eq!(review_payload.output.overall, OverallVerdict::Mixed);
        assert_eq!(review_payload.output.overall_confidence, Confidence::High);
        assert_eq!(review_payload.output.packet.total_calls_in_run, 10);
        assert_eq!(review_payload.output.packet.total_calls_in_scope, 3);
        assert_eq!(review_payload.output.packet.calls.len(), 3);
        assert_eq!(review_payload.artifact.procedure_name, TOOL_CALL_REVIEW);
        let branch_artifacts = &review_payload.artifact.artifact.second.branches;
        assert_eq!(
            branch_artifacts.left.left.provenance.model_id,
            "x-ai/grok-4-fast"
        );
        assert_eq!(
            branch_artifacts.left.left.provenance.response.usage,
            Some(TokenUsageMirror {
                prompt_tokens: 944,
                completion_tokens: 665,
                total_tokens: 1609,
            })
        );

        let segment_review = paths
            .iter()
            .find(|path| path.to_string_lossy().contains(TOOL_CALL_SEGMENT_REVIEW))
            .expect("segment review artifact path");
        let segment_review_value = read_json_value(segment_review);
        let segment_review_artifact: Artifact = serde_json::from_value(segment_review_value)
            .unwrap_or_else(|err| {
                panic!(
                    "deserialize segment review {}: {err}",
                    segment_review.display()
                )
            });
        let segment_review_payload = match segment_review_artifact.body() {
            Some(ArtifactBody::ToolCallSegmentReview(payload)) => payload,
            other => panic!("expected segment-review payload, got {other:?}"),
        };
        assert_eq!(segment_review_payload.input.segment.start_index, 0);
        assert_eq!(segment_review_payload.input.segment.end_index, 1);
        assert_eq!(segment_review_payload.output.packet.segment_index, Some(0));
        assert_eq!(
            segment_review_payload.output.packet.segment_status,
            Some(SegmentStatus::Labeled)
        );
        assert_eq!(
            segment_review_payload.output.packet.segment_label,
            Some(IntentLabel::InspectCandidate)
        );
        assert_eq!(
            segment_review_payload.artifact.procedure_name,
            TOOL_CALL_SEGMENT_REVIEW
        );
    }

    #[test]
    #[ignore]
    fn print_segmentation_roundtrip() {
        let path = protocol_artifact_paths()
            .into_iter()
            .find(|path| {
                path.to_string_lossy()
                    .contains(TOOL_CALL_INTENT_SEGMENTATION)
            })
            .expect("segmentation artifact path");
        let original = read_json_value(&path);

        println!(
            "before deserialize:\n{}",
            serde_json::to_string_pretty(&segmentation_probe_from_value(&original))
                .expect("format original probe")
        );

        let artifact: Artifact = serde_json::from_value(original.clone()).unwrap_or_else(|err| {
            panic!(
                "deserialize segmentation artifact {}: {err}",
                path.display()
            )
        });
        println!(
            "after deserialize:\n{}",
            serde_json::to_string_pretty(&segmentation_probe_from_artifact(&artifact))
                .expect("format artifact probe")
        );

        let serialized = serde_json::to_value(&artifact).expect("serialize segmentation artifact");
        println!(
            "after serialize again:\n{}",
            serde_json::to_string_pretty(&segmentation_probe_from_value(&serialized))
                .expect("format serialized probe")
        );

        assert_eq!(serialized, original);
    }

    fn segmentation_probe_from_value(value: &serde_json::Value) -> serde_json::Value {
        serde_json::json!({
            "schema_version": value.get("schema_version"),
            "procedure_name": value.get("procedure_name"),
            "input": {
                "total_calls_in_run": value.get("input").and_then(|input| input.get("total_calls_in_run")),
            },
            "output": {
                "coverage_total_calls": value.get("output")
                    .and_then(|output| output.get("coverage"))
                    .and_then(|coverage| coverage.get("total_calls")),
                "segments_len": value.get("output")
                    .and_then(|output| output.get("segments"))
                    .and_then(serde_json::Value::as_array)
                    .map(Vec::len),
            }
        })
    }

    fn segmentation_probe_from_artifact(artifact: &Artifact) -> serde_json::Value {
        let payload = match artifact.body() {
            Some(ArtifactBody::ToolCallIntentSegmentation(payload)) => payload,
            other => panic!("expected segmentation payload, got {other:?}"),
        };
        serde_json::json!({
            "schema_version": artifact.schema_version,
            "procedure_name": artifact.procedure_name,
            "input": {
                "total_calls_in_run": payload.input.total_calls_in_run,
            },
            "output": {
                "coverage_total_calls": payload.output.coverage.total_calls,
                "segments_len": payload.output.segments.len(),
            }
        })
    }

    fn protocol_artifact_paths() -> Vec<PathBuf> {
        let dir = protocol_artifacts_dir();
        let mut paths = fs::read_dir(&dir)
            .unwrap_or_else(|err| panic!("read protocol-artifacts dir {}: {err}", dir.display()))
            .map(|entry| entry.expect("read protocol entry").path())
            .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("json"))
            .collect::<Vec<_>>();
        paths.sort();
        paths
    }

    fn protocol_artifacts_dir() -> PathBuf {
        std::env::var_os("PLOKE_RECORDS_REAL_PROTOCOL_ARTIFACTS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from(
                    "/home/brasides/.ploke-eval/instances/prototype1/p1-edit-surface-history-long-20260508-1/treatments/branch-01187cd17226d1a4/instances/BurntSushi__ripgrep-2209/runs/run-1778270575083-structured-current-policy-7a8e5b98/protocol-artifacts",
                )
            })
    }

    fn read_json_value(path: &Path) -> serde_json::Value {
        let text = fs::read_to_string(path).unwrap_or_else(|err| {
            panic!(
                "read protocol artifact {} (set PLOKE_RECORDS_REAL_PROTOCOL_ARTIFACTS_DIR to override): {err}",
                path.display()
            )
        });
        serde_json::from_str(&text).expect("parse protocol artifact JSON value")
    }
}
