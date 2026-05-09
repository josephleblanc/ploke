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
mod tests;
