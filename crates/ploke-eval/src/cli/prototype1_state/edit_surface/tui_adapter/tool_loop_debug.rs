use std::{
    path::{Path, PathBuf},
    sync::Mutex,
};

use ploke_llm::manager::RecordedResponse;
use ploke_records::{
    agent_turn::{ToolCompletedRecord, ToolFailedRecord, ToolRequestRecord},
    llm_response::RawFullResponseRecord,
    tool_contracts::ToolArgumentsJson,
};
use ploke_tui::llm::{ChatDebugSink, ChatDebugStep, ChatDebugToolResult};

use super::ModelSelection;
use crate::{
    cli::prototype1_state::edit_surface::harness_request::{
        EvidenceRoot, EvidenceRootKind, EvidenceRootLocation,
    },
    prelude::CampaignId,
    replay::tool_loop::{
        FsToolLoopStore, ToolLoopResult, ToolLoopResume, ToolLoopSession, ToolLoopStatus,
        ToolLoopStep, ToolLoopStore, WorkspaceState,
    },
    spec::PrepareError,
};

pub(super) fn install_for_attempt(
    workspace: &Path,
    model: Option<&ModelSelection>,
    evidence: &[EvidenceRoot],
) -> Option<ploke_tui::llm::ChatDebugSinkGuard> {
    let prototype_root = prototype_root_from_evidence(evidence)?;
    let sink = ToolLoopDebugSink::new(
        prototype_root.join("debug/tool-loop"),
        workspace.to_path_buf(),
        model.map(|model| model.model_id().to_string()),
        campaign_id_from_prototype_root(&prototype_root),
    );
    Some(ploke_tui::llm::install_chat_debug_sink(
        std::sync::Arc::new(sink),
    ))
}

struct ToolLoopDebugSink {
    store: FsToolLoopStore,
    workspace: PathBuf,
    model: Option<String>,
    campaign_id: Option<CampaignId>,
    last_error: Mutex<Option<String>>,
}

impl ToolLoopDebugSink {
    fn new(
        root: PathBuf,
        workspace: PathBuf,
        model: Option<String>,
        campaign_id: Option<CampaignId>,
    ) -> Self {
        Self {
            store: FsToolLoopStore::new(root),
            workspace,
            model,
            campaign_id,
            last_error: Mutex::new(None),
        }
    }

    fn persist(&self, step: ChatDebugStep) -> Result<(), PrepareError> {
        let session_id = step.session_id.to_string();
        let mut session = ToolLoopSession::new(&session_id, "headless-tui", self.workspace.clone());
        session.campaign_id = self.campaign_id.as_ref().map(ToString::to_string);
        session.model = self.model.clone();
        session.status = if step.terminal {
            ToolLoopStatus::Terminal
        } else {
            ToolLoopStatus::Paused
        };
        self.store.write_session(&session)?;

        let raw = RawFullResponseRecord {
            assistant_message_id: step.assistant_message_id,
            recorded_response: RecordedResponse::new(step.step_index, step.response),
        };
        let mut record = ToolLoopStep::new(
            &session_id,
            step.step_index,
            step.request_messages,
            raw,
            WorkspaceState::default(),
            WorkspaceState::default(),
        )?;
        let request_id = format!("{}:{}", step.session_id, step.step_index);
        let parent_id = step.parent_id.to_string();
        record.tool_requests = step
            .tool_calls
            .iter()
            .map(|call| ToolRequestRecord {
                request_id: request_id.clone(),
                parent_id: parent_id.clone(),
                call_id: call.call_id.to_string(),
                tool: call.function.name.as_str().to_string(),
                arguments: ToolArgumentsJson::from(call.function.arguments.clone()),
            })
            .collect();
        record.tool_results = step
            .tool_results
            .into_iter()
            .map(|result| result_record(result, &request_id, &parent_id))
            .collect();
        record.terminal = step.terminal;
        self.store.write_step(&record)?;

        let mut resume = ToolLoopResume::new(
            &session_id,
            step.assistant_message_id.to_string(),
            parent_id,
            request_id,
            step.final_messages,
        );
        resume.next_step = step.step_index.saturating_add(1);
        resume.terminal = step.terminal;
        self.store.write_resume(&resume)
    }
}

impl ChatDebugSink for ToolLoopDebugSink {
    fn record_step(&self, step: ChatDebugStep) -> Result<(), String> {
        self.persist(step).map_err(|error| {
            let message = error.to_string();
            if let Ok(mut last_error) = self.last_error.lock() {
                *last_error = Some(message.clone());
            }
            message
        })
    }
}

fn result_record(result: ChatDebugToolResult, request_id: &str, parent_id: &str) -> ToolLoopResult {
    match result {
        ChatDebugToolResult::Completed {
            call_id,
            tool,
            content,
            ..
        } => ToolLoopResult::Completed(ToolCompletedRecord {
            request_id: request_id.to_string(),
            parent_id: parent_id.to_string(),
            call_id: call_id.to_string(),
            tool: tool.unwrap_or_else(|| "unknown".to_string()),
            content,
            ui_payload: None,
            latency_ms: 0,
        }),
        ChatDebugToolResult::Failed {
            call_id,
            tool,
            error,
            ..
        } => ToolLoopResult::Failed(ToolFailedRecord {
            request_id: request_id.to_string(),
            parent_id: parent_id.to_string(),
            call_id: call_id.to_string(),
            tool,
            error,
            ui_payload: None,
            latency_ms: 0,
        }),
    }
}

fn prototype_root_from_evidence(evidence: &[EvidenceRoot]) -> Option<PathBuf> {
    evidence
        .iter()
        .find_map(|root| match (&root.kind, &root.location) {
            (
                EvidenceRootKind::Evaluations | EvidenceRootKind::Nodes,
                EvidenceRootLocation::Directory { path },
            ) => path.parent().map(Path::to_path_buf),
            (EvidenceRootKind::HistoryBlocks, EvidenceRootLocation::Directory { path }) => {
                path.parent().and_then(Path::parent).map(Path::to_path_buf)
            }
            (
                EvidenceRootKind::ProtocolArtifacts,
                EvidenceRootLocation::NodeScopedDirectory { nodes_root, .. },
            ) => nodes_root.parent().map(Path::to_path_buf),
            _ => None,
        })
}

fn campaign_id_from_prototype_root(prototype_root: &Path) -> Option<CampaignId> {
    prototype_root
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .map(CampaignId::from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    use ploke_core::ArcStr;
    use ploke_llm::manager::RequestMessage;
    use uuid::Uuid;

    use crate::cli::prototype1_state::edit_surface::harness_request::{EvidenceRole, EvidenceRoot};

    fn response_with_tool_call() -> ploke_llm::response::OpenAiResponse {
        serde_json::from_value(serde_json::json!({
            "id": "chatcmpl-tool-loop-debug-test",
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
            "created": 1,
            "model": "test/model",
            "object": "chat.completion"
        }))
        .expect("response json")
    }

    fn list_dir_call() -> ploke_llm::response::ToolCall {
        serde_json::from_value(serde_json::json!({
            "id": "call-1",
            "type": "function",
            "function": {
                "name": "list_dir",
                "arguments": "{\"dir\":\".\",\"max_entries\":3}"
            }
        }))
        .expect("tool call json")
    }

    #[test]
    fn derives_checkpoint_root_from_nodes_evidence() {
        let root = EvidenceRoot {
            kind: EvidenceRootKind::Nodes,
            location: EvidenceRootLocation::Directory {
                path: PathBuf::from("/tmp/campaign/prototype1/nodes"),
            },
            role: EvidenceRole::RuntimeEvidence,
        };
        assert_eq!(
            prototype_root_from_evidence(&[root]),
            Some(PathBuf::from("/tmp/campaign/prototype1"))
        );
    }

    #[test]
    fn persists_chat_debug_step_to_tool_loop_store() {
        let tmp = tempfile::tempdir().expect("tempdir");
        let root = tmp.path().join("debug/tool-loop");
        let workspace = tmp.path().join("workspace");
        fs::create_dir_all(&workspace).expect("workspace dir");
        let sink = ToolLoopDebugSink::new(
            root.clone(),
            workspace.clone(),
            Some("test/model".to_string()),
            Some(CampaignId::from("campaign-debug-test")),
        );
        let session_id = Uuid::from_u128(0xcccccccc_cccc_cccc_cccc_cccccccccccc);
        let assistant_id = Uuid::from_u128(0xdddddddd_dddd_dddd_dddd_dddddddddddd);
        let parent_id = Uuid::from_u128(0xeeeeeeee_eeee_eeee_eeee_eeeeeeeeeeee);

        sink.record_step(ChatDebugStep {
            session_id,
            parent_id,
            assistant_message_id: assistant_id,
            step_index: 0,
            request_messages: vec![RequestMessage::new_user("hello".to_string())],
            response: response_with_tool_call(),
            tool_calls: vec![list_dir_call()],
            tool_results: vec![ChatDebugToolResult::Completed {
                call_id: ArcStr::from("call-1"),
                tool: Some("list_dir".to_string()),
                content: "{\"ok\":true}".to_string(),
                ui_payload: None,
            }],
            final_messages: vec![
                RequestMessage::new_user("hello".to_string()),
                RequestMessage::new_tool("{\"ok\":true}".to_string(), ArcStr::from("call-1")),
            ],
            terminal: false,
        })
        .expect("record step");

        let store = FsToolLoopStore::new(root);
        let session = store
            .read_session(&session_id.to_string())
            .expect("read session");
        assert_eq!(session.status, ToolLoopStatus::Paused);
        assert_eq!(session.model.as_deref(), Some("test/model"));
        let step = store
            .read_step(&session_id.to_string(), 0)
            .expect("read step");
        assert_eq!(step.tool_requests.len(), 1);
        assert_eq!(step.tool_results.len(), 1);
        let resume = store
            .read_resume(&session_id.to_string())
            .expect("read resume");
        assert_eq!(resume.next_step, 1);
        assert!(!resume.terminal);
    }
}
