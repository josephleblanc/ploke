//! Passive provider-response replay records.
//!
//! `llm-full-responses.jsonl` is a sidecar trace of normalized provider
//! response envelopes. Runtime crates decide when to emit or replay these
//! records; this module only owns the persisted line shape.

use std::{error::Error, fmt};

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

/// Parse failure for one physical JSONL record.
#[derive(Debug)]
pub struct DecodeError {
    line: usize,
    source: serde_json::Error,
}

/// One decoded response paired with its one-based physical JSONL line.
#[derive(Debug, Clone)]
pub struct FullResponseLine {
    line: usize,
    record: RawFullResponseRecord,
}

impl FullResponseLine {
    pub const fn line(&self) -> usize {
        self.line
    }

    pub fn record(&self) -> &RawFullResponseRecord {
        &self.record
    }

    pub fn into_record(self) -> RawFullResponseRecord {
        self.record
    }
}

impl DecodeError {
    pub const fn line(&self) -> usize {
        self.line
    }

    pub fn into_source(self) -> serde_json::Error {
        self.source
    }
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.source)
    }
}

impl Error for DecodeError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        Some(&self.source)
    }
}

/// Decode the canonical JSONL provider-response sidecar without imposing a
/// consumer-specific filter or ordering policy.
///
/// Blank lines are ignored, but every nonblank physical line must contain
/// exactly one complete record. This preserves the persisted JSONL contract
/// and reports the source line for present-but-invalid evidence.
pub fn decode_full_response_records(text: &str) -> Result<Vec<RawFullResponseRecord>, DecodeError> {
    Ok(decode_full_response_lines(text)?
        .into_iter()
        .map(FullResponseLine::into_record)
        .collect())
}

/// Decode the canonical sidecar while retaining each record's physical line.
pub fn decode_full_response_lines(text: &str) -> Result<Vec<FullResponseLine>, DecodeError> {
    text.lines()
        .enumerate()
        .filter(|(_, line)| !line.trim().is_empty())
        .map(|(index, line)| {
            let line_number = index + 1;
            serde_json::from_str(line)
                .map(|record| FullResponseLine {
                    line: line_number,
                    record,
                })
                .map_err(|source| DecodeError {
                    line: line_number,
                    source,
                })
        })
        .collect()
}

impl Record for RawFullResponseRecord {
    const FAMILY: RecordFamily = RecordFamily::LlmFullResponseTrace;
    const SCHEMA: &'static str = FULL_RESPONSE_TRACE_SCHEMA;
    const FORMAT: RecordFormat = RecordFormat::JsonLines;
}

#[cfg(test)]
mod tests {
    use super::*;

    const RESPONSE: &str = r#"{"assistant_message_id":"8e32b33b-6de5-4e1c-9fa1-14bc2059913f","response_index":0,"response":{"id":"chatcmpl-fixture","choices":[{"index":0,"finish_reason":"stop","message":{"role":"assistant","content":"done"}}],"created":0,"model":"test/model","object":"chat.completion"}}"#;

    #[test]
    fn raw_full_response_record_roundtrips_current_sidecar_shape() {
        let record: RawFullResponseRecord =
            serde_json::from_str(RESPONSE).expect("deserialize full response record");
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

    #[test]
    fn full_response_decoder_preserves_file_order_and_blank_lines() {
        let second = RESPONSE.replace("\"response_index\":0", "\"response_index\":1");
        let records = decode_full_response_records(&format!("\n{second}\n\n{RESPONSE}\n"))
            .expect("decode JSONL sidecar");

        assert_eq!(records.len(), 2);
        assert_eq!(records[0].response_index().get(), 1);
        assert_eq!(records[1].response_index().get(), 0);
    }

    #[test]
    fn full_response_line_decoder_preserves_physical_coordinates() {
        let second = RESPONSE.replace("\"response_index\":0", "\"response_index\":1");
        let lines = decode_full_response_lines(&format!("\n{second}\n\n{RESPONSE}\n"))
            .expect("decode JSONL lines");

        assert_eq!(lines[0].line(), 2);
        assert_eq!(lines[0].record().response_index().get(), 1);
        assert_eq!(lines[1].line(), 4);
        assert_eq!(lines[1].record().response_index().get(), 0);
    }

    #[test]
    fn full_response_decoder_reports_the_invalid_source_line() {
        let error = decode_full_response_records(&format!("{RESPONSE}\n{{not-json}}\n"))
            .expect_err("invalid JSONL line must fail");

        assert_eq!(error.line(), 2);
    }

    #[test]
    fn full_response_decoder_rejects_two_records_on_one_line() {
        let error = decode_full_response_records(&format!("{RESPONSE}{RESPONSE}\n"))
            .expect_err("one JSONL line cannot contain two records");

        assert_eq!(error.line(), 1);
    }
}
