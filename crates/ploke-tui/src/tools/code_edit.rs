use super::*;
use ploke_core::rag_types::ApplyCodeEditResult;
use crate::{app_state::core::PreviewMode, EventBus};
use ploke_db::NodeType;

/// CodeEdit tool: applies one or more canonical edits to source files.
///
/// - Calls into the existing `rag::tools::apply_code_edit_tool` path (no behavior changes),
///   using the Canonical edit mode exclusively.
/// - Returns an `ApplyCodeEditResult` summarizing the staged proposal.
pub struct CodeEdit {
    pub state: Arc<crate::app_state::AppState>,
    pub event_bus: Arc<EventBus>,
    /// Parent message id for correlation in the conversation tree.
    pub parent_id: Uuid,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CodeEditInput {
    pub edits: Vec<CanonicalEdit>,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct CanonicalEdit {
    pub file: String,
    pub canon: String,
    pub node_type: NodeType,
    pub code: String,
}

lazy_static::lazy_static! {
    static ref CODE_EDIT_PARAMETERS: serde_json::Value = json!({
        "type": "object",
        "properties": {
            "edits": {
                "type": "array",
                "items": {
                    "type": "object",
                    "properties": {
                        "file": { "type": "string", "description": "Absolute or workspace-relative file path." },
                        "canon": { "type": "string", "description": "Canonical path to the node, e.g. crate::module::Item" },
                        "node_type": { "type": "string", "description": "Node type (function|struct|enum|...)." },
                        "code": { "type": "string", "description": "Replacement code for the node." }
                    },
                    "required": ["file", "canon", "node_type", "code"],
                    "additionalProperties": false
                }
            },
            "confidence": {
                "type": "number",
                "minimum": 0.0,
                "maximum": 1.0,
                "description": "Optional confidence indicator for the edit proposal."
            }
        },
        "required": ["edits"],
        "additionalProperties": false
    });
}

impl Tool for CodeEdit {
    const NAME: &'static str = "apply_code_edit";
    const DESCRIPTION: &'static str = "Apply canonical code edits to one or more nodes identified by canonical path.";

    type Params = CodeEditInput;
    type Output = ApplyCodeEditResult;

    async fn run(self, p: Self::Params) -> Result<Self::Output, ploke_error::Error> {
        use crate::rag::utils::{ApplyCodeEditRequest, Edit, ToolCallParams};
        use crate::rag::tools::apply_code_edit_tool;

        // Convert to the strongly-typed request expected by the dispatcher/tool handler
        let typed_req = ApplyCodeEditRequest {
            confidence: p.confidence,
            edits: p
                .edits
                .into_iter()
                .map(|e| Edit::Canonical {
                    file: e.file,
                    canon: e.canon,
                    node_type: e.node_type,
                    code: e.code,
                })
                .collect(),
        };

        let arguments = serde_json::to_value(typed_req)
            .map_err(|e| ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(format!(
                "Failed to serialize CodeEditInput: {}",
                e
            ))))?;

        // Prepare tool call envelope
        let request_id = Uuid::new_v4();
        let call_id = Uuid::new_v4().to_string();

        // Execute the existing tool handler (no behavior change)
        let rx = self.event_bus.realtime_tx.subscribe();
        let params = ToolCallParams {
            state: &self.state,
            event_bus: &self.event_bus,
            request_id,
            parent_id: self.parent_id,
            name: Self::NAME.to_string(),
            arguments,
            call_id: call_id.clone(),
        };

        // Run the handler to stage edits and emit events
        apply_code_edit_tool(params).await;

        // Attempt to read the staged proposal and construct a typed result deterministically.
        // If not present (e.g. validation error), emit a structured error.
        let proposal_opt = { self.state.proposals.read().await.get(&request_id).cloned() };
        if let Some(prop) = proposal_opt {
            // Derive file display list and preview mode in the same spirit as the handler
            let crate_root = { self.state.system.read().await.crate_focus.clone() };
            let display_files: Vec<String> = prop
                .files
                .iter()
                .map(|p| {
                    if let Some(root) = crate_root.as_ref() {
                        p.strip_prefix(root)
                            .map(|rp| rp.display().to_string())
                            .unwrap_or_else(|_| p.display().to_string())
                    } else {
                        p.display().to_string()
                    }
                })
                .collect();

            let editing_cfg = { self.state.config.read().await.editing.clone() };
            let preview_mode = match editing_cfg.preview_mode {
                PreviewMode::Diff => "diff".to_string(),
                PreviewMode::CodeBlock => "codeblock".to_string(),
            };

            Ok(ApplyCodeEditResult {
                ok: true,
                staged: prop.edits.len(),
                applied: 0,
                files: display_files,
                preview_mode,
                auto_confirmed: editing_cfg.auto_confirm_edits,
            })
        } else {
            Err(ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(
                "apply_code_edit failed to stage proposal (see ToolCallFailed)".to_string(),
            )))
        }
    }

    fn schema() -> &'static serde_json::Value {
        CODE_EDIT_PARAMETERS.deref()
    }

    fn tool_def() -> ToolFunctionDef {
        ToolFunctionDef {
            name: ToolName::ApplyCodeEdit,
            description: ToolDescr::ApplyCodeEdit,
            parameters: Self::schema().clone(),
        }
    }
}
