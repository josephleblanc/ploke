//! Durable checkpoint records for response-stepped LLM/tool-loop debugging.
//!
//! These records are debugger evidence, not Prototype 1 History or child
//! admission authority. A `ToolLoopSession` records the outer `walk` edge that
//! opened a nested harness/tool-loop frame. Each `ToolLoopStep` records one
//! provider response after the full tool batch from that response has executed
//! and settled. `ToolLoopResume` stores the next request state so a debugger can
//! continue without re-running completed tool effects.

#![allow(dead_code)]

use std::{
    fs,
    path::{Path, PathBuf},
};

use ploke_llm::{
    manager::{RequestMessage, parse_chat_outcome},
    response::FinishReason,
};
use ploke_records::{
    agent_turn::{
        ObservedTurnEventRecord, ToolCompletedRecord, ToolFailedRecord, ToolRequestRecord,
    },
    llm_response::RawFullResponseRecord,
};
use serde::{Deserialize, Serialize};

use crate::spec::PrepareError;

pub(crate) const TOOL_LOOP_SESSION_SCHEMA: &str = "ploke-eval-tool-loop-session.v1";
pub(crate) const TOOL_LOOP_STEP_SCHEMA: &str = "ploke-eval-tool-loop-step.v1";
pub(crate) const TOOL_LOOP_RESUME_SCHEMA: &str = "ploke-eval-tool-loop-resume.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct ToolLoopSession {
    pub(crate) schema: String,
    pub(crate) session_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) campaign_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) parent_node_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) branch_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) generation: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) fanout_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) lane_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) request_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) outer_phase: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) outer_edge: Option<String>,
    pub(crate) harness: String,
    pub(crate) workspace: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<String>,
    pub(crate) status: ToolLoopStatus,
}

impl ToolLoopSession {
    pub(crate) fn new(
        session_id: impl Into<String>,
        harness: impl Into<String>,
        workspace: PathBuf,
    ) -> Self {
        Self {
            schema: TOOL_LOOP_SESSION_SCHEMA.to_owned(),
            session_id: session_id.into(),
            campaign_id: None,
            parent_node_id: None,
            branch_id: None,
            generation: None,
            fanout_id: None,
            lane_id: None,
            request_path: None,
            outer_phase: None,
            outer_edge: None,
            harness: harness.into(),
            workspace,
            model: None,
            status: ToolLoopStatus::Active,
        }
    }

    fn validate_schema(&self) -> Result<(), PrepareError> {
        validate_schema(
            "read_tool_loop_session",
            "session",
            &self.schema,
            TOOL_LOOP_SESSION_SCHEMA,
        )
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub(crate) enum ToolLoopStatus {
    Active,
    Paused,
    Terminal,
    Abandoned,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ToolLoopStep {
    pub(crate) schema: String,
    pub(crate) session_id: String,
    pub(crate) step_index: usize,
    pub(crate) request_messages: Vec<RequestMessage>,
    pub(crate) response: RawFullResponseRecord,
    pub(crate) outcome: ToolLoopOutcome,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tool_requests: Vec<ToolRequestRecord>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) tool_results: Vec<ToolLoopResult>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) events: Vec<ObservedTurnEventRecord>,
    pub(crate) workspace_before: WorkspaceState,
    pub(crate) workspace_after: WorkspaceState,
    pub(crate) terminal: bool,
}

impl ToolLoopStep {
    pub(crate) fn new(
        session_id: impl Into<String>,
        step_index: usize,
        request_messages: Vec<RequestMessage>,
        response: RawFullResponseRecord,
        workspace_before: WorkspaceState,
        workspace_after: WorkspaceState,
    ) -> Result<Self, PrepareError> {
        let outcome = ToolLoopOutcome::from_response(&response)?;
        Ok(Self {
            schema: TOOL_LOOP_STEP_SCHEMA.to_owned(),
            session_id: session_id.into(),
            step_index,
            request_messages,
            response,
            outcome,
            tool_requests: Vec::new(),
            tool_results: Vec::new(),
            events: Vec::new(),
            workspace_before,
            workspace_after,
            terminal: false,
        })
    }

    fn validate_schema(&self) -> Result<(), PrepareError> {
        validate_schema(
            "read_tool_loop_step",
            "step",
            &self.schema,
            TOOL_LOOP_STEP_SCHEMA,
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum ToolLoopOutcome {
    ToolCalls {
        count: usize,
        finish_reason: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_preview: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning_preview: Option<String>,
    },
    Content {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        content_preview: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        reasoning_preview: Option<String>,
    },
}

impl ToolLoopOutcome {
    pub(crate) fn from_response(response: &RawFullResponseRecord) -> Result<Self, PrepareError> {
        let body = serde_json::to_string(response.response()).map_err(PrepareError::Serialize)?;
        let step = parse_chat_outcome(&body).map_err(|source| PrepareError::DatabaseSetup {
            phase: "tool_loop_step_outcome",
            detail: source.to_string(),
        })?;
        Ok(match step.outcome {
            ploke_llm::manager::ChatStepOutcome::ToolCalls {
                calls,
                content,
                reasoning,
                finish_reason,
            } => Self::ToolCalls {
                count: calls.len(),
                finish_reason: finish_reason_label(&finish_reason),
                content_preview: content.as_deref().map(preview),
                reasoning_preview: reasoning.as_deref().map(preview),
            },
            ploke_llm::manager::ChatStepOutcome::Content { content, reasoning } => Self::Content {
                content_preview: content.as_deref().map(preview),
                reasoning_preview: reasoning.as_deref().map(preview),
            },
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "status", rename_all = "snake_case")]
pub(crate) enum ToolLoopResult {
    Completed(ToolCompletedRecord),
    Failed(ToolFailedRecord),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub(crate) struct WorkspaceState {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) dirty_paths: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub(crate) struct ToolLoopResume {
    pub(crate) schema: String,
    pub(crate) session_id: String,
    pub(crate) next_step: usize,
    pub(crate) assistant_message_id: String,
    pub(crate) parent_id: String,
    pub(crate) request_id: String,
    pub(crate) request_messages: Vec<RequestMessage>,
    pub(crate) attempts: u32,
    pub(crate) terminal: bool,
}

impl ToolLoopResume {
    pub(crate) fn new(
        session_id: impl Into<String>,
        assistant_message_id: impl Into<String>,
        parent_id: impl Into<String>,
        request_id: impl Into<String>,
        request_messages: Vec<RequestMessage>,
    ) -> Self {
        Self {
            schema: TOOL_LOOP_RESUME_SCHEMA.to_owned(),
            session_id: session_id.into(),
            next_step: 0,
            assistant_message_id: assistant_message_id.into(),
            parent_id: parent_id.into(),
            request_id: request_id.into(),
            request_messages,
            attempts: 0,
            terminal: false,
        }
    }

    fn validate_schema(&self) -> Result<(), PrepareError> {
        validate_schema(
            "read_tool_loop_resume",
            "resume",
            &self.schema,
            TOOL_LOOP_RESUME_SCHEMA,
        )
    }
}

pub(crate) trait ToolLoopStore {
    fn write_session(&self, session: &ToolLoopSession) -> Result<(), PrepareError>;
    fn read_session(&self, session_id: &str) -> Result<ToolLoopSession, PrepareError>;
    fn write_step(&self, step: &ToolLoopStep) -> Result<(), PrepareError>;
    fn read_step(&self, session_id: &str, step_index: usize) -> Result<ToolLoopStep, PrepareError>;
    fn write_resume(&self, resume: &ToolLoopResume) -> Result<(), PrepareError>;
    fn read_resume(&self, session_id: &str) -> Result<ToolLoopResume, PrepareError>;
    fn list_sessions(&self) -> Result<Vec<ToolLoopSession>, PrepareError>;
}

#[derive(Debug, Clone)]
pub(crate) struct FsToolLoopStore {
    root: PathBuf,
}

impl FsToolLoopStore {
    pub(crate) fn new(root: PathBuf) -> Self {
        Self { root }
    }

    pub(crate) fn root(&self) -> &Path {
        &self.root
    }

    pub(crate) fn session_dir(&self, session_id: &str) -> PathBuf {
        self.root.join(session_id)
    }

    fn session_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("session.json")
    }

    fn resume_path(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("resume.json")
    }

    fn steps_dir(&self, session_id: &str) -> PathBuf {
        self.session_dir(session_id).join("steps")
    }

    fn step_path(&self, session_id: &str, step_index: usize) -> PathBuf {
        self.steps_dir(session_id)
            .join(format!("{step_index:04}.json"))
    }

    pub(crate) fn latest_session(&self) -> Result<Option<ToolLoopSession>, PrepareError> {
        let mut sessions = self.list_sessions()?;
        sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        Ok(sessions.pop())
    }

    pub(crate) fn step_indices(&self, session_id: &str) -> Result<Vec<usize>, PrepareError> {
        let dir = self.steps_dir(session_id);
        if !dir.is_dir() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&dir).map_err(|source| PrepareError::ReadManifest {
            path: dir.clone(),
            source,
        })?;
        let mut indices = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| PrepareError::ReadManifest {
                path: dir.clone(),
                source,
            })?;
            let path = entry.path();
            if path.extension().is_some_and(|ext| ext == "json") {
                if let Some(stem) = path.file_stem().and_then(|stem| stem.to_str()) {
                    if let Ok(index) = stem.parse::<usize>() {
                        indices.push(index);
                    }
                }
            }
        }
        indices.sort_unstable();
        Ok(indices)
    }

    pub(crate) fn latest_step_index(
        &self,
        session_id: &str,
    ) -> Result<Option<usize>, PrepareError> {
        Ok(self.step_indices(session_id)?.pop())
    }
}

impl ToolLoopStore for FsToolLoopStore {
    fn write_session(&self, session: &ToolLoopSession) -> Result<(), PrepareError> {
        session.validate_schema()?;
        write_json(&self.session_path(&session.session_id), session)
    }

    fn read_session(&self, session_id: &str) -> Result<ToolLoopSession, PrepareError> {
        let session: ToolLoopSession = read_json(&self.session_path(session_id))?;
        session.validate_schema()?;
        Ok(session)
    }

    fn write_step(&self, step: &ToolLoopStep) -> Result<(), PrepareError> {
        step.validate_schema()?;
        write_json(&self.step_path(&step.session_id, step.step_index), step)
    }

    fn read_step(&self, session_id: &str, step_index: usize) -> Result<ToolLoopStep, PrepareError> {
        let step: ToolLoopStep = read_json(&self.step_path(session_id, step_index))?;
        step.validate_schema()?;
        Ok(step)
    }

    fn write_resume(&self, resume: &ToolLoopResume) -> Result<(), PrepareError> {
        resume.validate_schema()?;
        write_json(&self.resume_path(&resume.session_id), resume)
    }

    fn read_resume(&self, session_id: &str) -> Result<ToolLoopResume, PrepareError> {
        let resume: ToolLoopResume = read_json(&self.resume_path(session_id))?;
        resume.validate_schema()?;
        Ok(resume)
    }

    fn list_sessions(&self) -> Result<Vec<ToolLoopSession>, PrepareError> {
        if !self.root.is_dir() {
            return Ok(Vec::new());
        }
        let entries = fs::read_dir(&self.root).map_err(|source| PrepareError::ReadManifest {
            path: self.root.clone(),
            source,
        })?;
        let mut sessions = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|source| PrepareError::ReadManifest {
                path: self.root.clone(),
                source,
            })?;
            let path = entry.path().join("session.json");
            if path.is_file() {
                let session: ToolLoopSession = read_json(&path)?;
                session.validate_schema()?;
                sessions.push(session);
            }
        }
        sessions.sort_by(|left, right| left.session_id.cmp(&right.session_id));
        Ok(sessions)
    }
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let body = serde_json::to_vec_pretty(value).map_err(PrepareError::Serialize)?;
    fs::write(path, body).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T, PrepareError> {
    let body = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    serde_json::from_str(&body).map_err(|source| PrepareError::DatabaseSetup {
        phase: "read_tool_loop_json",
        detail: format!("failed to parse '{}': {source}", path.display()),
    })
}

fn validate_schema(
    phase: &'static str,
    label: &str,
    actual: &str,
    expected: &str,
) -> Result<(), PrepareError> {
    if actual == expected {
        return Ok(());
    }
    Err(PrepareError::DatabaseSetup {
        phase,
        detail: format!("unsupported tool-loop {label} schema '{actual}', expected '{expected}'"),
    })
}

fn finish_reason_label(reason: &FinishReason) -> String {
    match reason {
        FinishReason::Stop => "stop".to_owned(),
        FinishReason::Length => "length".to_owned(),
        FinishReason::ContentFilter => "content_filter".to_owned(),
        FinishReason::ToolCalls => "tool_calls".to_owned(),
        FinishReason::MalformedFunctionCall => "malformed_function_call".to_owned(),
        FinishReason::UnexpectedToolCall => "unexpected_tool_call".to_owned(),
        FinishReason::Timeout => "timeout".to_owned(),
        FinishReason::Error(error) => format!("error:{error}"),
    }
}

fn preview(text: &str) -> String {
    const MAX: usize = 240;
    if text.chars().count() <= MAX {
        return text.to_owned();
    }
    let mut out = text.chars().take(MAX.saturating_sub(3)).collect::<String>();
    out.push_str("...");
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_llm::manager::{RecordedResponse, ResponseIndex};
    use uuid::Uuid;

    fn content_response(index: usize) -> RawFullResponseRecord {
        let response = serde_json::from_value(serde_json::json!({
            "id": format!("chatcmpl-tool-loop-{index}"),
            "choices": [{
                "index": 0,
                "finish_reason": "stop",
                "message": {
                    "role": "assistant",
                    "content": "done"
                }
            }],
            "created": index,
            "model": "test/model",
            "object": "chat.completion"
        }))
        .expect("response json");
        RawFullResponseRecord {
            assistant_message_id: Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa),
            recorded_response: RecordedResponse {
                response_index: ResponseIndex::new(index),
                response,
            },
        }
    }

    fn tool_response(index: usize) -> RawFullResponseRecord {
        let response = serde_json::from_value(serde_json::json!({
            "id": format!("chatcmpl-tool-loop-{index}"),
            "choices": [{
                "index": 0,
                "finish_reason": "tool_calls",
                "message": {
                    "role": "assistant",
                    "tool_calls": [{
                        "id": "call-1",
                        "type": "function",
                        "function": {
                            "name": "list_dir",
                            "arguments": "{\"dir\":\".\",\"max_entries\":3}"
                        }
                    }]
                }
            }],
            "created": index,
            "model": "test/model",
            "object": "chat.completion"
        }))
        .expect("response json");
        RawFullResponseRecord {
            assistant_message_id: Uuid::from_u128(0xaaaaaaaa_aaaa_aaaa_aaaa_aaaaaaaaaaaa),
            recorded_response: RecordedResponse {
                response_index: ResponseIndex::new(index),
                response,
            },
        }
    }

    #[test]
    fn tool_loop_step_projects_content_outcome() {
        let step = ToolLoopStep::new(
            "session-1",
            0,
            vec![RequestMessage::new_user("hello".to_owned())],
            content_response(0),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");

        match step.outcome {
            ToolLoopOutcome::Content {
                content_preview, ..
            } => {
                assert_eq!(content_preview.as_deref(), Some("done"));
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn tool_loop_step_projects_tool_call_outcome() {
        let step = ToolLoopStep::new(
            "session-1",
            0,
            vec![RequestMessage::new_user("list".to_owned())],
            tool_response(0),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");

        match step.outcome {
            ToolLoopOutcome::ToolCalls {
                count,
                finish_reason,
                ..
            } => {
                assert_eq!(count, 1);
                assert_eq!(finish_reason, "tool_calls");
            }
            other => panic!("unexpected outcome: {other:?}"),
        }
    }

    #[test]
    fn fs_tool_loop_store_roundtrips_records() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FsToolLoopStore::new(dir.path().join("debug/tool-loop"));
        let mut session = ToolLoopSession::new(
            "session-1",
            "broad-headless-tui",
            PathBuf::from("/tmp/workspace"),
        );
        session.outer_phase = Some("r10".to_owned());
        session.outer_edge = Some("r10_to_r11".to_owned());
        session.status = ToolLoopStatus::Paused;
        store.write_session(&session).expect("write session");

        let step = ToolLoopStep::new(
            "session-1",
            0,
            vec![RequestMessage::new_user("hello".to_owned())],
            content_response(0),
            WorkspaceState::default(),
            WorkspaceState::default(),
        )
        .expect("step");
        store.write_step(&step).expect("write step");

        let mut resume = ToolLoopResume::new(
            "session-1",
            "assistant-1",
            "parent-1",
            "request-1",
            vec![RequestMessage::new_user("hello".to_owned())],
        );
        resume.next_step = 1;
        store.write_resume(&resume).expect("write resume");

        let loaded_session = store.read_session("session-1").expect("read session");
        assert_eq!(loaded_session.session_id, "session-1");
        assert_eq!(loaded_session.outer_phase.as_deref(), Some("r10"));
        assert_eq!(loaded_session.status, ToolLoopStatus::Paused);
        let sessions = store.list_sessions().expect("list sessions");
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session_id, "session-1");
        let latest = store
            .latest_session()
            .expect("latest session")
            .expect("session exists");
        assert_eq!(latest.session_id, "session-1");

        let loaded_step = store.read_step("session-1", 0).expect("read step");
        assert_eq!(loaded_step.step_index, 0);
        assert_eq!(loaded_step.request_messages.len(), 1);
        assert_eq!(loaded_step.response.response_index().get(), 0);

        let loaded_resume = store.read_resume("session-1").expect("read resume");
        assert_eq!(loaded_resume.next_step, 1);
        assert_eq!(loaded_resume.request_messages.len(), 1);
    }

    #[test]
    fn fs_tool_loop_store_rejects_wrong_schema() {
        let dir = tempfile::tempdir().expect("tempdir");
        let store = FsToolLoopStore::new(dir.path().join("debug/tool-loop"));
        let mut session = ToolLoopSession::new(
            "session-1",
            "broad-headless-tui",
            PathBuf::from("/tmp/workspace"),
        );
        session.schema = "old-schema".to_owned();
        let path = store.session_path("session-1");
        write_json(&path, &session).expect("write raw old schema");

        let error = store
            .read_session("session-1")
            .expect_err("wrong schema should fail");
        assert!(
            error
                .to_string()
                .contains("unsupported tool-loop session schema"),
            "unexpected error: {error}"
        );
    }
}
