use super::*;
use ploke_core::PROJECT_NAMESPACE_UUID;
use ploke_db::NodeType;

pub(crate) fn calc_top_k_for_budget(token_budget: u32) -> usize {
    let top_k = (token_budget / 200) as usize;
    top_k.clamp(5, 20)
}

// Strongly-typed request for apply_code_edit
#[derive(Debug, Serialize, Deserialize)]
pub struct ApplyCodeEditRequest {
    pub edits: Vec<Edit>,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Edit {
    Canonical {
        file: String,
        canon: String,
        node_type: NodeType,
        code: String,
    },
    Splice {
        file_path: String,
        expected_file_hash: ploke_core::TrackingHash,
        start_byte: u32,
        end_byte: u32,
        replacement: String,
        #[serde(default = "default_namespace")]
        namespace: uuid::Uuid,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Function,
    Const,
    Enum,
    Impl,
    Import,
    Macro,
    Module,
    Static,
    Struct,
    Trait,
    TypeAlias,
    Union,
}

impl NodeKind {
    pub fn as_relation(&self) -> &'static str {
        match self {
            NodeKind::Function => "function",
            NodeKind::Const => "const",
            NodeKind::Enum => "enum",
            NodeKind::Impl => "impl",
            NodeKind::Import => "import",
            NodeKind::Macro => "macro",
            NodeKind::Module => "module",
            NodeKind::Static => "static",
            NodeKind::Struct => "struct",
            NodeKind::Trait => "trait",
            NodeKind::TypeAlias => "type_alias",
            NodeKind::Union => "union",
        }
    }
}

fn default_namespace() -> uuid::Uuid { PROJECT_NAMESPACE_UUID }

// Temporary migration shim to accept legacy direct splice payloads without the tagged enum.
// This is used to map tests and older callers into the typed Edit::Splice variant.
#[derive(Debug, Deserialize)]
pub struct LegacyApplyDirect {
    pub edits: Vec<LegacySpliceInput>,
    #[serde(default)]
    pub confidence: Option<f32>,
    #[serde(default)]
    pub namespace: Option<uuid::Uuid>,
}

#[derive(Debug, Deserialize)]
pub struct LegacySpliceInput {
    pub file_path: String,
    pub expected_file_hash: ploke_core::TrackingHash,
    pub start_byte: u64,
    pub end_byte: u64,
    pub replacement: String,
    #[serde(default)]
    pub namespace: Option<uuid::Uuid>,
}

#[derive(Clone, Debug)]
pub struct ToolCallParams<'a> {
    pub state: &'a Arc<AppState>,
    pub event_bus: &'a Arc<EventBus>,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub name: String,
    pub arguments: serde_json::Value,
    pub call_id: String,
}

impl<'a> ToolCallParams<'a> {
    pub(super) fn tool_call_failed(&self, error: String) {
        let _ = self
            .event_bus
            .realtime_tx
            .send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: self.request_id,
                parent_id: self.parent_id,
                call_id: self.call_id.clone(),
                error,
            }));
    }
    pub(super) fn tool_call_err(&self, error: String) -> SystemEvent {
        SystemEvent::ToolCallFailed {
            request_id: self.request_id,
            parent_id: self.parent_id,
            call_id: self.call_id.clone(),
            error,
        }
    }
}
