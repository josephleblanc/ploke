//! Passive persisted protocol artifact DTOs.
//!
//! These records mirror persisted files under run-local
//! `protocol-artifacts/*.json` directories.
//! They do not evaluate procedures or enforce runtime authority.

use std::path::PathBuf;

use ploke_protocol::{
    LocalAnalysisAssessment, SegmentReviewSubject, SegmentedToolCallSequence, ToolCallNeighborhood,
    ToolCallSequence,
};
use serde::de::IgnoredAny;
use serde::ser::SerializeStruct;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use crate::record::{Record, RecordFamily, RecordFormat};

mod artifacts;
pub mod provenance;

pub use artifacts::{
    IntentSegmentationArtifactMirror, IntentSegmentationPayload, ToolCallReviewArtifactMirror,
    ToolCallReviewPayload, ToolCallSegmentReviewArtifactMirror, ToolCallSegmentReviewPayload,
};
pub use provenance::{
    ChoiceMirror, JsonLlmProvenanceMirror, MechanizedProvenanceMirror, OpenAiResponseMirror,
    ResponseMessageMirror, TokenUsageMirror,
};

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
pub struct ArtifactCoordinate {
    pub schema_version: String,
    pub procedure_name: String,
    pub subject_id: String,
    pub run_id: String,
    pub created_at_ms: u64,
    pub model_id: Option<String>,
    pub provider_slug: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ArtifactDecodeFailureRecord {
    pub path: Option<PathBuf>,
    pub coordinate: Option<ArtifactCoordinate>,
    pub expected_payload_kind: Option<ArtifactPayloadKind>,
    pub error: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactPayloadKind {
    ToolCallIntentSegmentation,
    ToolCallReview,
    ToolCallSegmentReview,
}

#[derive(Debug, Clone, PartialEq)]
pub enum ArtifactBody {
    ToolCallIntentSegmentation(IntentSegmentationPayload),
    ToolCallReview(ToolCallReviewPayload),
    ToolCallSegmentReview(ToolCallSegmentReviewPayload),
}

impl Record for Artifact {
    const FAMILY: RecordFamily = RecordFamily::ProtocolArtifact;
    const SCHEMA: &'static str = SCHEMA_V1;
    const FORMAT: RecordFormat = RecordFormat::Json;
}

impl Artifact {
    pub fn body(&self) -> Option<ArtifactBody> {
        Some(self.body.clone())
    }

    pub fn typed_payload(&self) -> Option<ArtifactBody> {
        self.body()
    }
}

pub fn decode_artifact_str(
    source: &str,
    path: Option<PathBuf>,
) -> Result<Artifact, ArtifactDecodeFailureRecord> {
    serde_json::from_str(source).map_err(|err| ArtifactDecodeFailureRecord {
        path,
        coordinate: artifact_coordinate(source).ok(),
        expected_payload_kind: expected_payload_kind(source),
        error: err.to_string(),
    })
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
        struct RawIntentSegmentationArtifact {
            schema_version: String,
            subject_id: String,
            run_id: String,
            created_at_ms: u64,
            #[serde(default)]
            model_id: Option<String>,
            #[serde(default)]
            provider_slug: Option<String>,
            input: ToolCallSequence,
            output: SegmentedToolCallSequence,
            artifact: IntentSegmentationArtifactMirror,
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawToolCallReviewArtifact {
            schema_version: String,
            subject_id: String,
            run_id: String,
            created_at_ms: u64,
            #[serde(default)]
            model_id: Option<String>,
            #[serde(default)]
            provider_slug: Option<String>,
            input: ToolCallNeighborhood,
            output: LocalAnalysisAssessment,
            artifact: ToolCallReviewArtifactMirror,
        }

        #[derive(Deserialize)]
        #[serde(deny_unknown_fields)]
        struct RawToolCallSegmentReviewArtifact {
            schema_version: String,
            subject_id: String,
            run_id: String,
            created_at_ms: u64,
            #[serde(default)]
            model_id: Option<String>,
            #[serde(default)]
            provider_slug: Option<String>,
            input: SegmentReviewSubject,
            output: LocalAnalysisAssessment,
            artifact: ToolCallSegmentReviewArtifactMirror,
        }

        #[derive(Deserialize)]
        #[serde(tag = "procedure_name")]
        enum RawArtifact {
            #[serde(rename = "tool_call_intent_segmentation")]
            ToolCallIntentSegmentation(RawIntentSegmentationArtifact),
            #[serde(rename = "tool_call_review")]
            ToolCallReview(RawToolCallReviewArtifact),
            #[serde(rename = "tool_call_segment_review")]
            ToolCallSegmentReview(RawToolCallSegmentReviewArtifact),
        }

        match RawArtifact::deserialize(deserializer)? {
            RawArtifact::ToolCallIntentSegmentation(raw) => Ok(Self {
                schema_version: raw.schema_version,
                procedure_name: TOOL_CALL_INTENT_SEGMENTATION.to_string(),
                subject_id: raw.subject_id,
                run_id: raw.run_id,
                created_at_ms: raw.created_at_ms,
                model_id: raw.model_id,
                provider_slug: raw.provider_slug,
                body: ArtifactBody::ToolCallIntentSegmentation(IntentSegmentationPayload {
                    input: raw.input,
                    output: raw.output,
                    artifact: raw.artifact,
                }),
            }),
            RawArtifact::ToolCallReview(raw) => Ok(Self {
                schema_version: raw.schema_version,
                procedure_name: TOOL_CALL_REVIEW.to_string(),
                subject_id: raw.subject_id,
                run_id: raw.run_id,
                created_at_ms: raw.created_at_ms,
                model_id: raw.model_id,
                provider_slug: raw.provider_slug,
                body: ArtifactBody::ToolCallReview(ToolCallReviewPayload {
                    input: raw.input,
                    output: raw.output,
                    artifact: raw.artifact,
                }),
            }),
            RawArtifact::ToolCallSegmentReview(raw) => Ok(Self {
                schema_version: raw.schema_version,
                procedure_name: TOOL_CALL_SEGMENT_REVIEW.to_string(),
                subject_id: raw.subject_id,
                run_id: raw.run_id,
                created_at_ms: raw.created_at_ms,
                model_id: raw.model_id,
                provider_slug: raw.provider_slug,
                body: ArtifactBody::ToolCallSegmentReview(ToolCallSegmentReviewPayload {
                    input: raw.input,
                    output: raw.output,
                    artifact: raw.artifact,
                }),
            }),
        }
    }
}

fn expected_payload_kind(source: &str) -> Option<ArtifactPayloadKind> {
    artifact_coordinate(source).ok().and_then(|coordinate| {
        match coordinate.procedure_name.as_str() {
            TOOL_CALL_INTENT_SEGMENTATION => Some(ArtifactPayloadKind::ToolCallIntentSegmentation),
            TOOL_CALL_REVIEW => Some(ArtifactPayloadKind::ToolCallReview),
            TOOL_CALL_SEGMENT_REVIEW => Some(ArtifactPayloadKind::ToolCallSegmentReview),
            _ => None,
        }
    })
}

fn artifact_coordinate(source: &str) -> Result<ArtifactCoordinate, serde_json::Error> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct CoordinateProbe {
        schema_version: String,
        procedure_name: String,
        subject_id: String,
        run_id: String,
        created_at_ms: u64,
        #[serde(default)]
        model_id: Option<String>,
        #[serde(default)]
        provider_slug: Option<String>,
        #[serde(rename = "input")]
        _input: IgnoredAny,
        #[serde(rename = "output")]
        _output: IgnoredAny,
        #[serde(rename = "artifact")]
        _artifact: IgnoredAny,
    }

    let probe: CoordinateProbe = serde_json::from_str(source)?;
    Ok(ArtifactCoordinate {
        schema_version: probe.schema_version,
        procedure_name: probe.procedure_name,
        subject_id: probe.subject_id,
        run_id: probe.run_id,
        created_at_ms: probe.created_at_ms,
        model_id: probe.model_id,
        provider_slug: probe.provider_slug,
    })
}

#[cfg(test)]
mod tests;
