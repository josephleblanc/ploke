use std::path::PathBuf;

use ploke_core::CrateId;
use uuid::Uuid;

use super::core::WorkspaceFreshness;

/// Typed mutations for `SystemStatus`. All system state changes flow through
/// these variants, applied via `SystemStatus::apply`. This centralizes state
/// transitions and prevents ad-hoc field mutation.
#[derive(Debug, Clone)]
pub enum SystemMutation {
    LoadWorkspace {
        workspace_root: PathBuf,
        member_roots: Vec<PathBuf>,
        focused_root: Option<PathBuf>,
    },
    LoadStandaloneCrate {
        crate_root: PathBuf,
    },
    RecordParseSuccess,
    RecordParseFailure {
        target_dir: PathBuf,
        message: String,
    },
    SetWorkspaceFreshness {
        crate_id: CrateId,
        freshness: WorkspaceFreshness,
    },
    RecordIndexComplete {
        crate_id: CrateId,
    },
    InitPwd {
        pwd: PathBuf,
    },
}

#[derive(Debug, Clone, Copy)]
pub struct MessageUpdatedEvent(pub Uuid);

impl MessageUpdatedEvent {
    pub fn new(message_id: Uuid) -> Self {
        Self(message_id)
    }
}

impl From<MessageUpdatedEvent> for crate::AppEvent {
    fn from(event: MessageUpdatedEvent) -> Self {
        crate::AppEvent::MessageUpdated(event)
    }
}

use std::{borrow::Cow, sync::Arc};

use ploke_db::TypedEmbedData;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    ArcStr, ModelId, UiError,
    tools::{Ctx, ToolCall, ToolName, ToolUiPayload},
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum SystemEvent {
    SaveRequested(Vec<u8>), // Serialized content
    HistorySaved {
        file_path: String,
    },
    MutationFailed(UiError),
    CommandDropped(&'static str),
    ReadSnippet(TypedEmbedData),
    CompleteReadSnip(Vec<String>),
    ModelSwitched(ModelId),
    ToolCallRequested {
        // request_id: Uuid,
        tool_call: ToolCall,
        // ctx: Ctx
        parent_id: Uuid,
        request_id: Uuid,
        // name: ToolName,
        // arguments: Value,
        // call_id: ArcStr,
    },
    ToolCallCompleted {
        request_id: Uuid,
        parent_id: Uuid,
        call_id: ArcStr,
        content: String,
        #[serde(default)]
        ui_payload: Option<ToolUiPayload>,
    },
    ToolCallFailed {
        request_id: Uuid,
        parent_id: Uuid,
        call_id: ArcStr,
        error: String,
        #[serde(default)]
        ui_payload: Option<ToolUiPayload>,
    },
    ReadQuery {
        file_name: String,
        query_name: String,
    },
    WriteQuery {
        query_name: String,
        query_content: String,
    },
    BackupDb {
        file_dir: String,
        is_success: bool,
        error: Option<String>,
    },
    LoadDb {
        workspace_ref: String,
        #[serde(skip)]
        file_dir: Option<Arc<std::path::PathBuf>>,
        #[serde(skip)]
        root_path: Option<Arc<std::path::PathBuf>>,
        is_success: bool,
        error: Option<String>,
    },
    ReIndex {
        workspace: String,
    },
    /// Working directory changed - components should update their cached pwd
    PwdChanged(PathBuf),
    #[cfg(all(feature = "test_harness", feature = "live_api_tests"))]
    TestHarnessApiResponse {
        request_id: Uuid,
        response_body: String,
        model: String,
        use_tools: bool,
    },
}
