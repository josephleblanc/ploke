use uuid::Uuid;

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

use crate::{tools::{Ctx, ToolCall, ToolName}, ArcStr, ModelId, UiError};

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
    },
    ToolCallFailed {
        request_id: Uuid,
        parent_id: Uuid,
        call_id: ArcStr,
        error: String,
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
        crate_name: String,
        #[serde(skip)]
        file_dir: Option<Arc<std::path::PathBuf>>,
        is_success: bool,
        error: Option<&'static str>,
    },
    ReIndex {
        workspace: String,
    },
    #[cfg(all(feature = "test_harness", feature = "live_api_tests"))]
    TestHarnessApiResponse {
        request_id: Uuid,
        response_body: String,
        model: String,
        use_tools: bool,
    },
}
