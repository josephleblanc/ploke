//! Load recorded provider responses from eval run artifacts.
//!
//! This module reads the passive sidecar owned by `ploke-records` and converts
//! it into the `ploke-llm` tape used by replay-capable chat sessions. It does
//! not replay TUI events or synthesize tool results.

use std::{
    collections::BTreeSet,
    fs,
    path::{Path, PathBuf},
};

use ploke_llm::manager::{
    ChatStepOutcome, RecordedResponseTape, ResponseIndex, parse_chat_outcome,
};
use ploke_records::llm_response::{FULL_RESPONSE_TRACE_FILE, RawFullResponseRecord};
use uuid::Uuid;

use crate::spec::PrepareError;

#[derive(Debug, Clone)]
pub struct LoadedResponseTape {
    path: PathBuf,
    assistant_message_id: Uuid,
    records: Vec<RawFullResponseRecord>,
}

#[derive(Debug, Clone)]
pub struct ResponseTapeInspection {
    path: PathBuf,
    assistant_message_id: Uuid,
    records: Vec<RawFullResponseRecord>,
}

impl LoadedResponseTape {
    pub fn load(run_dir: &Path, assistant_message_id: &str) -> Result<Self, PrepareError> {
        let loaded = load_response_tape_records(run_dir, assistant_message_id)?;
        loaded.reject_missing_response_indices()?;
        Ok(loaded)
    }

    pub fn load_for_inspection(
        run_dir: &Path,
        assistant_message_id: &str,
    ) -> Result<ResponseTapeInspection, PrepareError> {
        load_response_tape_records(run_dir, assistant_message_id).map(ResponseTapeInspection::from)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn assistant_message_id(&self) -> Uuid {
        self.assistant_message_id
    }

    pub fn records(&self) -> &[RawFullResponseRecord] {
        &self.records
    }

    pub fn record_count(&self) -> usize {
        self.records.len()
    }

    pub fn last_response_index(&self) -> Option<ResponseIndex> {
        self.records
            .last()
            .map(RawFullResponseRecord::response_index)
    }

    pub fn empty_prefix(&self) -> Self {
        Self {
            path: self.path.clone(),
            assistant_message_id: self.assistant_message_id,
            records: Vec::new(),
        }
    }

    pub fn prefix_through_response_index(
        &self,
        response_index: ResponseIndex,
    ) -> Result<Self, PrepareError> {
        let mut found = false;
        let records = self
            .records
            .iter()
            .filter_map(|record| {
                let include = record.response_index() <= response_index;
                found |= record.response_index() == response_index;
                include.then(|| record.clone())
            })
            .collect::<Vec<_>>();

        if !found {
            return Err(PrepareError::DatabaseSetup {
                phase: "slice_llm_replay",
                detail: format!(
                    "assistant message '{}' does not have recorded response_index {} in '{}'",
                    self.assistant_message_id,
                    response_index,
                    self.path.display()
                ),
            });
        }

        Ok(Self {
            path: self.path.clone(),
            assistant_message_id: self.assistant_message_id,
            records,
        })
    }

    pub fn with_appended_records(
        &self,
        records: &[RawFullResponseRecord],
    ) -> Result<Self, PrepareError> {
        let mut merged = self.records.clone();
        let mut seen = merged
            .iter()
            .map(RawFullResponseRecord::response_index)
            .collect::<BTreeSet<_>>();
        for record in records {
            if !record.matches_assistant_message(self.assistant_message_id) {
                return Err(PrepareError::DatabaseSetup {
                    phase: "append_llm_replay_branch",
                    detail: format!(
                        "branch response assistant_message_id '{}' does not match replay assistant_message_id '{}'",
                        record.assistant_message_id, self.assistant_message_id
                    ),
                });
            }
            if !seen.insert(record.response_index()) {
                return Err(PrepareError::DatabaseSetup {
                    phase: "append_llm_replay_branch",
                    detail: format!(
                        "branch response_index {} duplicates an installed replay response",
                        record.response_index()
                    ),
                });
            }
            merged.push(record.clone());
        }
        let loaded = Self {
            path: self.path.clone(),
            assistant_message_id: self.assistant_message_id,
            records: merged,
        };
        loaded.reject_missing_response_indices()?;
        Ok(loaded)
    }

    pub fn response_index_for_tool_call(
        &self,
        call_id: &str,
    ) -> Result<Option<ResponseIndex>, PrepareError> {
        for record in &self.records {
            let body = serde_json::to_string(record.response()).map_err(PrepareError::Serialize)?;
            let step = parse_chat_outcome(&body).map_err(|source| PrepareError::DatabaseSetup {
                phase: "inspect_llm_replay_tool_calls",
                detail: source.to_string(),
            })?;
            if let ChatStepOutcome::ToolCalls { calls, .. } = step.outcome {
                if calls.iter().any(|call| call.call_id.as_ref() == call_id) {
                    return Ok(Some(record.response_index()));
                }
            }
        }
        Ok(None)
    }

    pub fn missing_response_indices(&self) -> Vec<ResponseIndex> {
        missing_response_indices(&self.records)
    }

    fn reject_missing_response_indices(&self) -> Result<(), PrepareError> {
        let missing = self.missing_response_indices();
        if missing.is_empty() {
            return Ok(());
        }

        let missing_list = missing
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>()
            .join(", ");
        Err(PrepareError::DatabaseSetup {
            phase: "load_llm_replay",
            detail: format!(
                "recorded provider response sidecar '{}' for assistant message '{}' is incomplete; missing response_index values: {missing_list}. Use load_for_inspection for forensic inspection only; replay admission requires a contiguous tape.",
                self.path.display(),
                self.assistant_message_id
            ),
        })
    }

    pub fn into_recorded_response_tape(self) -> RecordedResponseTape {
        let responses = self
            .records
            .into_iter()
            .map(RawFullResponseRecord::into_recorded_response)
            .collect();
        RecordedResponseTape::new(responses)
    }
}

impl ResponseTapeInspection {
    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn assistant_message_id(&self) -> Uuid {
        self.assistant_message_id
    }

    pub fn records(&self) -> &[RawFullResponseRecord] {
        &self.records
    }

    pub fn missing_response_indices(&self) -> Vec<ResponseIndex> {
        missing_response_indices(&self.records)
    }
}

impl From<LoadedResponseTape> for ResponseTapeInspection {
    fn from(loaded: LoadedResponseTape) -> Self {
        Self {
            path: loaded.path,
            assistant_message_id: loaded.assistant_message_id,
            records: loaded.records,
        }
    }
}

pub fn load_recorded_response_tape(
    run_dir: &Path,
    assistant_message_id: &str,
) -> Result<RecordedResponseTape, PrepareError> {
    Ok(LoadedResponseTape::load(run_dir, assistant_message_id)?.into_recorded_response_tape())
}

pub fn install_tui_recorded_response_tape(
    run_dir: &Path,
    assistant_message_id: &str,
) -> Result<LoadedResponseTape, PrepareError> {
    let loaded = LoadedResponseTape::load(run_dir, assistant_message_id)?;
    ploke_tui::llm::install_recorded_response_tape(loaded.clone().into_recorded_response_tape());
    Ok(loaded)
}

pub fn install_tui_recorded_response_prefix_then_live(
    run_dir: &Path,
    assistant_message_id: &str,
) -> Result<LoadedResponseTape, PrepareError> {
    let loaded = LoadedResponseTape::load(run_dir, assistant_message_id)?;
    ploke_tui::llm::install_recorded_response_prefix_then_live(
        loaded.clone().into_recorded_response_tape(),
    );
    Ok(loaded)
}

fn load_response_tape_records(
    run_dir: &Path,
    assistant_message_id: &str,
) -> Result<LoadedResponseTape, PrepareError> {
    let assistant_message_id = parse_assistant_message_id(assistant_message_id)?;
    let path = run_dir.join(FULL_RESPONSE_TRACE_FILE);
    let records = load_full_response_records_for_assistant(&path, assistant_message_id)?;
    if records.is_empty() {
        return Err(PrepareError::DatabaseSetup {
            phase: "load_llm_replay",
            detail: format!(
                "no recorded provider responses in '{}' for assistant message '{}'",
                path.display(),
                assistant_message_id
            ),
        });
    }

    Ok(LoadedResponseTape {
        path,
        assistant_message_id,
        records,
    })
}

fn load_full_response_records_for_assistant(
    path: &Path,
    assistant_message_id: Uuid,
) -> Result<Vec<RawFullResponseRecord>, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let mut responses = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let record: RawFullResponseRecord =
            serde_json::from_str(trimmed).map_err(|source| PrepareError::ParseManifest {
                path: path.to_path_buf(),
                source,
            })?;
        if record.matches_assistant_message(assistant_message_id) {
            responses.push(record);
        }
    }
    responses.sort_by_key(|record| record.response_index());
    Ok(responses)
}

fn missing_response_indices(records: &[RawFullResponseRecord]) -> Vec<ResponseIndex> {
    let Some(last) = records.last().map(|record| record.response_index().get()) else {
        return Vec::new();
    };
    let present = records
        .iter()
        .map(|record| record.response_index().get())
        .collect::<BTreeSet<_>>();
    (0..=last)
        .filter(|index| !present.contains(index))
        .map(ResponseIndex::new)
        .collect()
}

fn parse_assistant_message_id(assistant_message_id: &str) -> Result<Uuid, PrepareError> {
    Uuid::parse_str(assistant_message_id).map_err(|source| PrepareError::DatabaseSetup {
        phase: "load_llm_replay",
        detail: format!("invalid assistant_message_id '{assistant_message_id}': {source}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn load_recorded_response_tape_filters_and_sorts_by_assistant_message() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join(FULL_RESPONSE_TRACE_FILE);
        let assistant_a = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        let assistant_b = Uuid::from_u128(0xbbbbbbbb_bbbb_bbbb_bbbb_bbbbbbbbbbbb);
        let first = response_line(assistant_a, 2, "second");
        let other = response_line(assistant_b, 0, "ignored");
        let second = response_line(assistant_a, 1, "first");
        let zeroth = response_line(assistant_a, 0, "zeroth");
        fs::write(&path, format!("{first}\n{other}\n{second}\n{zeroth}\n")).expect("write sidecar");

        let loaded =
            LoadedResponseTape::load(root.path(), &assistant_a.to_string()).expect("load tape");

        assert_eq!(loaded.path(), path.as_path());
        assert_eq!(loaded.assistant_message_id(), assistant_a);
        let indexes = loaded
            .records()
            .iter()
            .map(|record| record.response_index().get())
            .collect::<Vec<_>>();
        assert_eq!(indexes, vec![0, 1, 2]);
        assert_eq!(loaded.missing_response_indices(), Vec::new());
    }

    #[test]
    fn loaded_response_tape_rejects_non_contiguous_response_indexes_by_default() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join(FULL_RESPONSE_TRACE_FILE);
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        let first = response_line(assistant, 0, "first");
        let third = response_line(assistant, 2, "third");
        fs::write(&path, format!("{first}\n{third}\n")).expect("write sidecar");

        let err = LoadedResponseTape::load(root.path(), &assistant.to_string())
            .expect_err("default replay admission should reject gapped tapes");
        let message = err.to_string();
        assert!(
            message.contains("missing response_index values: 1"),
            "expected missing index in error, got {message}"
        );
        assert!(
            message.contains("load_for_inspection"),
            "expected forensic inspection hint in error, got {message}"
        );
    }

    #[test]
    fn loaded_response_tape_can_slice_prefix_through_response_index() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join(FULL_RESPONSE_TRACE_FILE);
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        let first = response_line(assistant, 0, "first");
        let second = response_line(assistant, 1, "second");
        let third = response_line(assistant, 2, "third");
        fs::write(&path, format!("{third}\n{first}\n{second}\n")).expect("write sidecar");

        let loaded =
            LoadedResponseTape::load(root.path(), &assistant.to_string()).expect("load tape");
        let prefix = loaded
            .prefix_through_response_index(ResponseIndex::new(1))
            .expect("slice prefix");

        let indexes = prefix
            .records()
            .iter()
            .map(|record| record.response_index().get())
            .collect::<Vec<_>>();
        assert_eq!(indexes, vec![0, 1]);
    }

    #[test]
    fn loaded_response_tape_can_append_branch_records() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join(FULL_RESPONSE_TRACE_FILE);
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        let first = response_line(assistant, 0, "first");
        fs::write(&path, format!("{first}\n")).expect("write sidecar");
        let loaded =
            LoadedResponseTape::load(root.path(), &assistant.to_string()).expect("load tape");
        let branch = response_record(assistant, 1, "branch");

        let merged = loaded
            .with_appended_records(&[branch])
            .expect("append branch response");

        let indexes = merged
            .records()
            .iter()
            .map(|record| record.response_index().get())
            .collect::<Vec<_>>();
        assert_eq!(indexes, vec![0, 1]);
    }

    #[test]
    fn loaded_response_tape_allows_non_contiguous_response_indexes_for_inspection() {
        let root = tempfile::tempdir().expect("tempdir");
        let path = root.path().join(FULL_RESPONSE_TRACE_FILE);
        let assistant = Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa);
        let second = response_line(assistant, 1, "second");
        let third = response_line(assistant, 2, "third");
        fs::write(&path, format!("{second}\n{third}\n")).expect("write sidecar");

        let loaded = LoadedResponseTape::load_for_inspection(root.path(), &assistant.to_string())
            .expect("inspection load should allow gapped tape");

        let missing = loaded
            .missing_response_indices()
            .into_iter()
            .map(ResponseIndex::get)
            .collect::<Vec<_>>();
        assert_eq!(missing, vec![0]);
    }

    fn response_line(assistant_message_id: Uuid, response_index: usize, content: &str) -> String {
        serde_json::json!({
            "assistant_message_id": assistant_message_id,
            "response_index": response_index,
            "response": {
                "id": format!("response-{response_index}"),
                "choices": [{
                    "index": 0,
                    "finish_reason": "stop",
                    "message": {
                        "role": "assistant",
                        "content": content
                    }
                }],
                "created": 0,
                "model": "test/model",
                "object": "chat.completion"
            }
        })
        .to_string()
    }

    fn response_record(
        assistant_message_id: Uuid,
        response_index: usize,
        content: &str,
    ) -> RawFullResponseRecord {
        serde_json::from_str(&response_line(
            assistant_message_id,
            response_index,
            content,
        ))
        .expect("response record")
    }
}
