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

use std::borrow::Cow;

#[derive(Debug, Clone, Deserialize)]
pub struct CodeEditParams<'a> {
    #[serde(borrow)]
    pub edits: Vec<CanonicalEditBorrowed<'a>>,
    #[serde(default)]
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CanonicalEditBorrowed<'a> {
    #[serde(borrow)]
    pub file: Cow<'a, str>,
    #[serde(borrow)]
    pub canon: Cow<'a, str>,
    pub node_type: NodeType,
    #[serde(borrow)]
    pub code: Cow<'a, str>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CodeEditParamsOwned {
    pub edits: Vec<CanonicalEditOwned>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CanonicalEditOwned {
    pub file: String,
    pub canon: String,
    pub node_type: NodeType,
    pub code: String,
}

impl super::GatTool for CodeEdit {
    type Output = ApplyCodeEditResult;
    type OwnedParams = CodeEditParamsOwned;
    type Params<'de> = CodeEditParams<'de>;

    fn name() -> super::ToolName { super::ToolName::ApplyCodeEdit }
    fn description() -> super::ToolDescr { super::ToolDescr::ApplyCodeEdit }
    fn schema() -> &'static serde_json::Value { CODE_EDIT_PARAMETERS.deref() }

    fn build(ctx: &super::Ctx) -> Self {
        Self {
            state: Arc::clone(&ctx.state),
            event_bus: Arc::clone(&ctx.event_bus),
            parent_id: ctx.parent_id,
        }
    }

    fn into_owned<'a>(params: &Self::Params<'a>) -> Self::OwnedParams {
        CodeEditParamsOwned {
            confidence: params.confidence,
            edits: params.edits.iter().map(|e| CanonicalEditOwned {
                file: e.file.clone().into_owned(),
                canon: e.canon.clone().into_owned(),
                node_type: e.node_type,
                code: e.code.clone().into_owned(),
            }).collect(),
        }
    }

    async fn run<'a>(self, params: &Self::Params<'a>, _ctx: super::Ctx) -> Result<Self::Output, ploke_error::Error> {
        use crate::rag::utils::{ApplyCodeEditRequest, Edit, ToolCallParams};
        use crate::rag::tools::apply_code_edit_tool;
        // Convert borrowed -> owned typed request
        let typed_req = ApplyCodeEditRequest {
            confidence: params.confidence,
            edits: params.edits.iter().map(|e| Edit::Canonical {
                file: e.file.clone().into_owned(),
                canon: e.canon.clone().into_owned(),
                node_type: e.node_type,
                code: e.code.clone().into_owned(),
            }).collect(),
        };
        let arguments = serde_json::to_value(typed_req)
            .map_err(|e| ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(format!(
                "Failed to serialize CodeEditParams: {}", e
            ))))?;
        // Execute legacy staging path; rely on proposal store for result
        let request_id = Uuid::new_v4();
        let call_id = Uuid::new_v4().to_string();
        let params_env = ToolCallParams {
            state: Arc::clone(&self.state),
            event_bus: Arc::clone(&self.event_bus),
            request_id,
            parent_id: self.parent_id,
            name: "apply_code_edit".to_string(),
            arguments,
            call_id,
        };
        apply_code_edit_tool(params_env).await;
        // Build typed result deterministically from proposal registry
        let proposal_opt = { self.state.proposals.read().await.get(&request_id).cloned() };
        if let Some(prop) = proposal_opt {
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
}

impl super::ToolFromParams for CodeEdit {
    fn build(params: &crate::rag::utils::ToolCallParams) -> Self {
        Self {
            state: Arc::clone(&params.state),
            event_bus: Arc::clone(&params.event_bus),
            parent_id: params.parent_id,
        }
    }
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
            state: Arc::clone(&self.state),
            event_bus: Arc::clone(&self.event_bus),
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
            parameters: <CodeEdit as super::Tool>::schema().clone(),
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tool_def_serializes_expected_shape() {
        let def = <CodeEdit as Tool>::tool_def();
        let v = serde_json::to_value(&def).expect("serialize");
        let func = v.as_object().expect("def obj");
        assert_eq!(func.get("name").and_then(|n| n.as_str()), Some("apply_code_edit"));
        let params = func.get("parameters").and_then(|p| p.as_object()).expect("params obj");
        let props = params.get("properties").and_then(|p| p.as_object()).expect("props obj");
        assert!(props.contains_key("edits"));
        let edits = props.get("edits").and_then(|e| e.as_object()).expect("edits obj");
        assert!(edits.contains_key("items"));
    }

    #[test]
    fn canonical_mapping_includes_mode() {
        use crate::rag::utils::{ApplyCodeEditRequest, Edit};

        let input = CodeEditInput {
            edits: vec![CanonicalEdit {
                file: "src/lib.rs".to_string(),
                canon: "crate::x::y".to_string(),
                node_type: ploke_db::NodeType::Function,
                code: "fn y(){}".to_string(),
            }],
            confidence: Some(0.5),
        };
        let typed = ApplyCodeEditRequest {
            confidence: input.confidence,
            edits: input
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
        let payload = serde_json::to_value(&typed).expect("serialize typed");
        let first = payload
            .get("edits").and_then(|e| e.as_array()).and_then(|a| a.first())
            .and_then(|e| e.as_object()).expect("obj");
        assert_eq!(first.get("mode").and_then(|m| m.as_str()), Some("canonical"));
    }
}
