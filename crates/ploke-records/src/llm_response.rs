//! Passive provider-response replay records.
//!
//! `llm-full-responses.jsonl` is a sidecar trace of normalized provider
//! response envelopes. Runtime crates decide when to emit or replay these
//! records; this module only owns the persisted line shape.

use ploke_llm::manager::{RecordedResponse, ResponseIndex};
use ploke_llm::response::OpenAiResponse;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::record::{Record, RecordFamily, RecordFormat};

pub const FULL_RESPONSE_TRACE_FILE: &str = "llm-full-responses.jsonl";
pub const FULL_RESPONSE_TRACE_SCHEMA: &str = "llm-full-response-trace.v1";

/// One line in `llm-full-responses.jsonl`.
///
/// The response payload is the normalized OpenAI/OpenRouter envelope that
/// `ploke-llm` can parse back into a `ChatStepData`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawFullResponseRecord {
    /// Assistant message node being updated by this response.
    pub assistant_message_id: Uuid,

    /// Recorded provider response payload and response-chain coordinate.
    #[serde(flatten)]
    pub recorded_response: RecordedResponse,
}

impl RawFullResponseRecord {
    pub fn new(assistant_message_id: Uuid, recorded_response: RecordedResponse) -> Self {
        Self {
            assistant_message_id,
            recorded_response,
        }
    }

    pub fn matches_assistant_message(&self, assistant_message_id: Uuid) -> bool {
        self.assistant_message_id == assistant_message_id
    }

    pub fn response_index(&self) -> ResponseIndex {
        self.recorded_response.response_index
    }

    pub fn response(&self) -> &OpenAiResponse {
        &self.recorded_response.response
    }

    pub fn into_recorded_response(self) -> RecordedResponse {
        self.recorded_response
    }
}

impl Record for RawFullResponseRecord {
    const FAMILY: RecordFamily = RecordFamily::LlmFullResponseTrace;
    const SCHEMA: &'static str = FULL_RESPONSE_TRACE_SCHEMA;
    const FORMAT: RecordFormat = RecordFormat::JsonLines;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn raw_full_response_record_roundtrips_current_sidecar_shape() {
        let json = r#"{
            "assistant_message_id": "8e32b33b-6de5-4e1c-9fa1-14bc2059913f",
            "response_index": 0,
            "response": {
                "id": "chatcmpl-fixture",
                "choices": [{
                    "index": 0,
                    "finish_reason": "stop",
                    "message": {
                        "role": "assistant",
                        "content": "done"
                    }
                }],
                "created": 0,
                "model": "test/model",
                "object": "chat.completion"
            }
        }"#;

        let record: RawFullResponseRecord =
            serde_json::from_str(json).expect("deserialize full response record");
        let assistant_id =
            Uuid::parse_str("8e32b33b-6de5-4e1c-9fa1-14bc2059913f").expect("assistant uuid");
        assert!(record.matches_assistant_message(assistant_id));
        assert_eq!(record.response_index().get(), 0);
        assert_eq!(record.response().id, "chatcmpl-fixture");

        let encoded = serde_json::to_string(&record).expect("serialize full response record");
        let decoded: RawFullResponseRecord =
            serde_json::from_str(&encoded).expect("deserialize encoded record");
        assert_eq!(decoded.assistant_message_id, record.assistant_message_id);
        assert_eq!(decoded.response_index(), record.response_index());
        assert_eq!(decoded.response().model, "test/model");
    }
}
