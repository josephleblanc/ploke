use super::*;
use crate::{EventBus, app_state::core::PreviewMode};
use ploke_core::rag_types::ApplyCodeEditResult;
use ploke_db::NodeType;

/// CodeEdit tool: applies one or more canonical edits to source files.
///
/// - Calls into the existing `rag::tools::apply_code_edit_tool` path (no behavior changes),
///   using the Canonical edit mode exclusively.
/// - Returns an `ApplyCodeEditResult` summarizing the staged proposal.
pub struct GatCodeEdit;
pub struct CodeEdit {
    state: Arc<AppState>,
    event_bus: Arc<EventBus>,
    parent_id: Uuid,
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

impl super::Tool for GatCodeEdit {
    type Output = ApplyCodeEditResult;
    type OwnedParams = CodeEditParamsOwned;
    type Params<'de> = CodeEditParams<'de>;

    fn name() -> super::ToolName {
        super::ToolName::ApplyCodeEdit
    }
    fn description() -> super::ToolDescr {
        super::ToolDescr::ApplyCodeEdit
    }
    fn schema() -> &'static serde_json::Value {
        CODE_EDIT_PARAMETERS.deref()
    }

    fn build(ctx: &super::Ctx) -> Self {
        Self
    }

    // TODO:refactor
    // Check if this is actually being used.
    // Might be smarter to require + implement From<Self::Params<'a>> for Edit or something
    fn into_owned<'a>(params: &Self::Params<'a>) -> Self::OwnedParams {
        CodeEditParamsOwned {
            confidence: params.confidence,
            edits: params
                .edits
                .iter()
                .map(|e| CanonicalEditOwned {
                    file: e.file.clone().into_owned(),
                    canon: e.canon.clone().into_owned(),
                    node_type: e.node_type,
                    code: e.code.clone().into_owned(),
                })
                .collect(),
        }
    }

    async fn execute<'a>(
        params: Self::Params<'a>,
        ctx: super::Ctx,
    ) -> Result<ToolResult, ploke_error::Error> {
        use crate::rag::tools::apply_code_edit_tool;
        use crate::rag::utils::{ApplyCodeEditRequest, Edit, ToolCallParams};
        // Convert borrowed -> owned typed request
        let typed_req = ApplyCodeEditRequest {
            confidence: params.confidence,
            edits: params
                .edits
                .iter()
                .map(|e| Edit::Canonical {
                    file: e.file.clone().into_owned(),
                    canon: e.canon.clone().into_owned(),
                    node_type: e.node_type,
                    code: e.code.clone().into_owned(),
                })
                .collect(),
        };
        // Execute legacy staging path; rely on proposal store for result
        let request_id = ctx.request_id;
        let call_id = ctx.call_id.clone();
        let params_env = ToolCallParams {
            state: Arc::clone(&ctx.state),
            event_bus: Arc::clone(&ctx.event_bus),
            request_id,
            parent_id: ctx.parent_id,
            name: Self::name(),
            typed_req,
            call_id,
        };
        apply_code_edit_tool(params_env).await;
        // Build typed result deterministically from proposal registry
        print_code_edit_results(&ctx, request_id, ToolName::ApplyCodeEdit).await
    }
}

pub async fn print_code_edit_results(
    ctx: &Ctx,
    request_id: Uuid,
    tool_name: ToolName,
) -> Result<ToolResult, ploke_error::Error> {
    let proposal_opt = { ctx.state.proposals.read().await.get(&request_id).cloned() };
    if let Some(prop) = proposal_opt {
        let primary_root = ctx
            .state
            .with_system_read(|sys| sys.tool_path_context().map(|(r, _)| r.clone()))
            .await;
        let staged = if matches!(tool_name, ToolName::NsPatch) {
            prop.edits_ns.len()
        } else {
            prop.edits.len()
        };
        let files: Vec<std::path::PathBuf> = if !prop.files.is_empty() {
            prop.files.clone()
        } else if matches!(tool_name, ToolName::NsPatch) {
            prop.edits_ns.iter().map(|e| e.file_path.clone()).collect()
        } else {
            Vec::new()
        };
        let display_files: Vec<String> = files
            .iter()
            .map(|p| {
                if let Some(ref root) = primary_root {
                    p.strip_prefix(root)
                        .map(|rp| rp.display().to_string())
                        .unwrap_or_else(|_| p.display().to_string())
                } else {
                    p.display().to_string()
                }
            })
            .collect();
        let editing_cfg = { ctx.state.config.read().await.editing.clone() };
        let preview_mode = match editing_cfg.preview_mode {
            PreviewMode::Diff => "diff".to_string(),
            PreviewMode::CodeBlock => "codeblock".to_string(),
        };
        let structured_result = ApplyCodeEditResult {
            ok: true,
            staged,
            applied: 0,
            files: display_files,
            preview_mode,
            auto_confirmed: editing_cfg.auto_confirm_edits,
        };
        let summary = format!(
            "Staged {} edits across {} files",
            structured_result.staged,
            structured_result.files.len()
        );
        let ui_payload = super::ToolUiPayload::new(tool_name, ctx.call_id.clone(), summary)
            .with_request_id(request_id)
            .with_field("status", "pending")
            .with_field("staged", structured_result.staged.to_string())
            .with_field("applied", structured_result.applied.to_string())
            .with_field("files", structured_result.files.len().to_string())
            .with_field("preview_mode", structured_result.preview_mode.as_str());
        let serialized_str =
            serde_json::to_string(&structured_result).expect("Incorrect deserialization");
        Ok(ToolResult {
            content: serialized_str,
            ui_payload: Some(ui_payload),
        })
    } else {
        Err(ploke_error::Error::Internal(
            ploke_error::InternalError::CompilerError(
                "apply_code_edit failed to stage proposal (see ToolCallFailed)".to_string(),
            ),
        ))
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
                        "canon": { "type": "string", "description": "Canonical path to a semantic target, e.g. crate::module::Item or crate::module::Type::method." },
                        "node_type": semantic_node_type_schema_property(),
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
    });
}

fn semantic_node_type_schema_property() -> serde_json::Value {
    let values = NodeType::primary_and_assoc_nodes()
        .iter()
        .map(|node_type| node_type.relation_str())
        .collect::<Vec<_>>();
    serde_json::json!({
        "type": "string",
        "enum": values,
        "description": format!(
            "Semantic node type accepted by apply_code_edit. Must be one of: {}",
            values.join(", ")
        ),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .get("edits")
            .and_then(|e| e.as_array())
            .and_then(|a| a.first())
            .and_then(|e| e.as_object())
            .expect("obj");
        assert_eq!(
            first.get("mode").and_then(|m| m.as_str()),
            Some("canonical")
        );
    }

    #[test]
    fn schema_guidance_mentions_method_targets() {
        let schema = &*CODE_EDIT_PARAMETERS;
        let node_type = schema
            .get("properties")
            .and_then(|props| props.get("edits"))
            .and_then(|edits| edits.get("items"))
            .and_then(|items| items.get("properties"))
            .and_then(|props| props.get("node_type"))
            .and_then(|value| value.get("description"))
            .and_then(|desc| desc.as_str())
            .expect("node_type schema description");
        let canon = schema
            .get("properties")
            .and_then(|props| props.get("edits"))
            .and_then(|edits| edits.get("items"))
            .and_then(|items| items.get("properties"))
            .and_then(|props| props.get("canon"))
            .and_then(|value| value.get("description"))
            .and_then(|desc| desc.as_str())
            .expect("canon schema description");

        assert!(
            node_type
                .to_lowercase()
                .contains("methods are valid direct targets")
        );
        assert!(!node_type.to_lowercase().contains("not direct targets"));
        assert!(canon.contains("Type::method"));
    }
}

#[cfg(test)]
mod gat_tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn params_deserialize_and_into_owned() {
        let raw = r#"{
            "edits": [
                {
                    "file": "src/lib.rs",
                    "canon": "crate::foo::bar",
                    "node_type": "function",
                    "code": "fn bar(){}"
                }
            ],
            "confidence": 0.42
        }"#;
        let params = GatCodeEdit::deserialize_params(raw).expect("parse");
        assert_eq!(params.confidence, Some(0.42));
        assert_eq!(params.edits.len(), 1);
        let owned = GatCodeEdit::into_owned(&params);
        assert_eq!(owned.confidence, Some(0.42));
        assert_eq!(owned.edits[0].file, "src/lib.rs");
        assert_eq!(owned.edits[0].canon, "crate::foo::bar");
        assert_eq!(owned.edits[0].code, "fn bar(){}");
    }

    #[test]
    fn name_desc_and_schema_present() {
        assert!(matches!(GatCodeEdit::name(), ToolName::ApplyCodeEdit));
        assert!(matches!(
            GatCodeEdit::description(),
            ToolDescr::ApplyCodeEdit
        ));
        let schema = GatCodeEdit::schema();
        assert!(schema.as_object().unwrap().contains_key("properties"));
    }

    #[tokio::test]
    async fn execute_errors_when_proposal_not_staged() {
        use crate::event_bus::EventBusCaps;
        let state = Arc::new(crate::test_utils::mock::create_mock_app_state());
        let event_bus = Arc::new(crate::EventBus::new(EventBusCaps::default()));
        let ctx = super::Ctx {
            state,
            event_bus,
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            call_id: ArcStr::from("code-edit-test"),
        };
        let params = CodeEditParams {
            edits: vec![CanonicalEditBorrowed {
                file: Cow::Borrowed("src/lib.rs"),
                canon: Cow::Borrowed("crate::nope::missing"),
                node_type: ploke_db::NodeType::Function,
                code: Cow::Borrowed("fn missing(){}"),
            }],
            confidence: Some(0.1),
        };
        let res = GatCodeEdit::execute(params, ctx).await;
        assert!(res.is_err(), "expected failure when no proposal was staged");
    }
    #[test]
    fn de_to_value() -> color_eyre::Result<()> {
        let schema = GatCodeEdit::schema();
        let v = serde_json::to_value(schema).expect("serialize");
        let node_type = v
            .get("properties")
            .and_then(|p| p.get("edits"))
            .and_then(|e| e.get("items"))
            .and_then(|i| i.get("properties"))
            .and_then(|p| p.get("node_type"))
            .and_then(|n| n.as_object())
            .expect("node_type schema");
        let enum_vals = node_type
            .get("enum")
            .and_then(|v| v.as_array())
            .expect("enum values");
        let enum_vals: Vec<&str> = enum_vals
            .iter()
            .map(|v| v.as_str().expect("string enum value"))
            .collect();
        let expected: Vec<&str> = NodeType::primary_and_assoc_nodes()
            .iter()
            .map(|ty| ty.relation_str())
            .collect();
        assert_eq!(enum_vals, expected);
        assert!(enum_vals.contains(&"method"));

        Ok(())
    }
}
