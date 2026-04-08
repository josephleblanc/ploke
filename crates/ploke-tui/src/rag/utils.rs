use crate::tools::{ToolError, ToolErrorCode, ToolName, ToolUiPayload};

use super::*;
use ploke_core::{ArcStr, PROJECT_NAMESPACE_UUID};
use ploke_db::NodeType;

pub(crate) fn calc_top_k_for_budget(token_budget: u32) -> usize {
    let budget = token_budget;
    let top_k = (budget / 200) as usize;
    top_k.clamp(5, 20)
}

// Strongly-typed request for apply_code_edit
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyCodeEditRequest {
    pub edits: Vec<Edit>,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(tag = "mode", rename_all = "snake_case")]
pub enum Edit {
    // edit applied internally
    Canonical {
        file: String,
        canon: String,
        node_type: NodeType,
        code: String,
    },
    // edit applied internally
    // TODO:cleanup verify we still do/don't use this
    Splice {
        file_path: String,
        expected_file_hash: ploke_core::TrackingHash,
        start_byte: u32,
        end_byte: u32,
        replacement: String,
        #[serde(default = "default_namespace")]
        namespace: uuid::Uuid,
    },
    // arguments passed to mpatch::parse eventually
    // TODO:refactor
    // Try making these ArcStr instead maybe?
    // Or possibly leave them as Cow, depending on how we handle them re: threads
    Patch {
        file: String,
        diff: String,
        reasoning: String,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
pub enum NodeKind {
    Function,
    Method,
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
    pub const ALL: [Self; 13] = [
        Self::Function,
        Self::Method,
        Self::Const,
        Self::Enum,
        Self::Impl,
        Self::Import,
        Self::Macro,
        Self::Module,
        Self::Static,
        Self::Struct,
        Self::Trait,
        Self::TypeAlias,
        Self::Union,
    ];

    pub fn as_str(&self) -> &'static str {
        self.as_relation()
    }

    pub fn as_relation(&self) -> &'static str {
        match self {
            NodeKind::Function => "function",
            NodeKind::Method => "method",
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

    pub fn allowed_values() -> [&'static str; 13] {
        Self::ALL.map(|kind| kind.as_relation())
    }

    pub fn schema_description() -> String {
        let values = Self::allowed_values().join(", ");
        format!(
            "The kind of code item this is. Use `method` for items defined inside `impl` or `trait` blocks and `function` for free functions. Must be one of: {values}"
        )
    }

    pub fn schema_property() -> serde_json::Value {
        serde_json::json!({
            "type": "string",
            "enum": Self::allowed_values(),
            "description": Self::schema_description(),
        })
    }

    pub fn lookup_hint(self) -> Option<&'static str> {
        match self {
            NodeKind::Function => Some(
                "Hint: if this item is defined inside an `impl` or `trait`, retry with node_kind=method.",
            ),
            NodeKind::Method => {
                Some("Hint: if this item is a free function, retry with node_kind=function.")
            }
            _ => None,
        }
    }
}

impl std::str::FromStr for NodeKind {
    type Err = &'static str;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "function" => Ok(Self::Function),
            "method" => Ok(Self::Method),
            "const" => Ok(Self::Const),
            "enum" => Ok(Self::Enum),
            "impl" => Ok(Self::Impl),
            "import" => Ok(Self::Import),
            "macro" => Ok(Self::Macro),
            "module" => Ok(Self::Module),
            "static" => Ok(Self::Static),
            "struct" => Ok(Self::Struct),
            "trait" => Ok(Self::Trait),
            "type_alias" => Ok(Self::TypeAlias),
            "union" => Ok(Self::Union),
            _ => Err("invalid node kind"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::NodeKind;

    #[test]
    fn node_kind_includes_method() {
        assert_eq!(NodeKind::Method.as_relation(), "method");
        assert!(NodeKind::allowed_values().contains(&"method"));

        let parsed = "method".parse::<NodeKind>().expect("parse method");
        assert!(matches!(parsed, NodeKind::Method));
    }

    #[test]
    fn node_kind_lookup_hint_mentions_method_for_function() {
        let hint = NodeKind::Function.lookup_hint().expect("function hint");
        assert!(hint.contains("node_kind=method"));
    }
}

fn default_namespace() -> uuid::Uuid {
    PROJECT_NAMESPACE_UUID
}

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
pub struct ToolCallParams {
    pub state: Arc<AppState>,
    pub event_bus: Arc<EventBus>,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub name: ToolName,
    pub typed_req: ApplyCodeEditRequest,
    pub call_id: ArcStr,
}

impl ToolCallParams {
    fn tool_error_from_message(&self, message: impl Into<String>) -> ToolError {
        ToolError::new(self.name, ToolErrorCode::InvalidFormat, message)
    }

    pub(super) fn tool_call_failed(&self, error: String) {
        let err = self.tool_error_from_message(error);
        self.tool_call_failed_error(err);
    }

    pub(super) fn tool_call_failed_error(&self, error: ToolError) {
        let _ = self
            .event_bus
            .realtime_tx
            .send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id: self.request_id,
                parent_id: self.parent_id,
                call_id: self.call_id.clone(),
                error: error.to_wire_string(),
                ui_payload: Some(ToolUiPayload::from_error(self.call_id.clone(), &error)),
            }));
    }

    pub(super) fn tool_call_err(&self, error: String) -> SystemEvent {
        let err = self.tool_error_from_message(error);
        self.tool_call_err_from_error(err)
    }

    pub(super) fn tool_call_err_from_error(&self, error: ToolError) -> SystemEvent {
        SystemEvent::ToolCallFailed {
            request_id: self.request_id,
            parent_id: self.parent_id,
            call_id: self.call_id.clone(),
            error: error.to_wire_string(),
            ui_payload: Some(ToolUiPayload::from_error(self.call_id.clone(), &error)),
        }
    }
}
