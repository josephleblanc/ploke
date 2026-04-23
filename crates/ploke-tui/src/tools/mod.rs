#![allow(clippy::needless_lifetimes)]

// For any db-related issues, check cross-crate contract with db in:
//  `ploke/crates/ploke-tui/docs/crate-contracts/tool-to-ploke-db.md`
use crate::{
    tracing_setup::TOOL_CALL_TARGET,
    utils::{
        consts::DEBUG_TOOLS,
        path_scoping,
        se_de::{de_arc_str, se_arc_str},
    },
};
use std::{
    borrow::Cow,
    collections::HashMap,
    ops::Deref,
    path::{Path, PathBuf},
    sync::Arc,
};

use crate::{
    AppEvent,
    app_state::{AppState, events::SystemEvent},
    rag::utils::ToolCallParams,
};
use itertools::Itertools;
use once_cell::sync::OnceCell;
use ploke_core::{
    ArcStr,
    rag_types::{ContextPart, ContextPartKind, Modality},
};
use ploke_error::DomainError;
use ploke_io::path_policy::PathPolicy;
use ploke_rag::{RagService, RetrievalStrategy, TokenBudget};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use syn_parser::parser::nodes::NodePath;
use tokio::sync::mpsc;
use uuid::Uuid;

#[cfg(test)]
mod tool_tests;

pub mod request_code_context;
pub use request_code_context::{
    RequestCodeContext, RequestCodeContextGat, RequestCodeContextInput,
};
pub mod code_edit;
pub use code_edit::{CanonicalEdit, CodeEdit, CodeEditInput, GatCodeEdit};
pub mod cargo;
pub mod code_item_lookup;
pub mod create_file;
pub mod error;
pub mod get_code_edges;
pub mod insert_rust_item;
pub mod list_dir;
pub mod ns_patch;
pub mod ns_read;
pub mod ui;
pub mod validators;

pub use error::{
    Audience, ToolError, ToolErrorCode, ToolErrorWire, ToolInvocationError, allowed_tool_names,
    tool_io_error, tool_ui_error,
};
pub use ui::{ToolUiField, ToolUiPayload, ToolVerbosity};

// NOTE:ploke-llm
// Moved ToolName and its `as_str` implementation into `ploke-core` for now so `ploke-llm` and
// `ploke-tui` have shared access to the same underlying types.
pub use ploke_core::tool_types::ToolName;

// NOTE:ploke-llm
// tool descriptions now live in ploke-core as first-class text artifacts so both ploke-tui and
// ploke-llm can share the same guidance surface.
pub use ploke_core::tool_descriptions::ToolDescription;

// NOTE:ploke-llm
// moved into ploke-core for shared access
pub use ploke_core::tool_types::FunctionMarker;

// NOTE:ploke-llm
// moved into ploke-core for shared access
pub use ploke_core::tool_types::ToolDefinition;

// NOTE:ploke-llm
// moved into ploke-core for shared access
pub use ploke_core::tool_types::ToolFunctionDef;

// --- GAT-based tool dispatch (pilot) ---
#[derive(Debug)]
pub struct Ctx {
    pub state: Arc<crate::app_state::AppState>,
    pub event_bus: Arc<crate::EventBus>,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub call_id: ArcStr,
}

impl Clone for Ctx {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            event_bus: self.event_bus.clone(),
            request_id: self.request_id,
            parent_id: self.parent_id,
            call_id: self.call_id.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct ToolCallRecord {
    pub name: ToolName,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub call_id: ArcStr,
    pub params_json: Arc<String>,
    pub output_json: Arc<String>,
    pub created_at_ms: i64,
}

// NOTE:ploke-llm
// moved into ploke-llm, as it is mostly concerned with structuring the message to the API
// endpoint.
//
// #[derive(Deserialize, Serialize, Debug, Clone, PartialOrd, PartialEq)]
// /* pub */ struct ToolCall {
//     #[serde(
//         deserialize_with = "de_arc_str",
//         serialize_with = "se_arc_str",
//         rename = "id"
//     )]
//     pub call_id: ArcStr,
//
//     #[serde(rename = "type")]
//     pub call_type: FunctionMarker,
//     pub function: FunctionCall,
// }
//
// #[derive(Deserialize, Serialize, Debug, Clone, PartialOrd, PartialEq)]
// pub struct FunctionCall {
//     pub name: ToolName,
//     // Store raw JSON arguments - needs to be owned String for deserialization from OpenRouter
//     pub arguments: String,
// }
//
pub use ploke_llm::response::ToolCall;

// Tool result structure
#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub content: String,
    pub ui_payload: Option<ToolUiPayload>,
}

#[derive(Debug, Clone)]
pub struct ToolCallPreflightError {
    pub call_id: ArcStr,
    pub tool_name: ToolName,
    pub error: ToolError,
}

pub fn validate_and_sanitize_tool_calls(
    tool_calls: &[ToolCall],
) -> Result<Vec<ToolCall>, ToolCallPreflightError> {
    tool_calls
        .iter()
        .map(validate_and_sanitize_tool_call)
        .collect()
}

pub fn validate_and_sanitize_tool_call(
    tool_call: &ToolCall,
) -> Result<ToolCall, ToolCallPreflightError> {
    let mut sanitized = tool_call.clone();
    sanitized.function.arguments = sanitize_tool_args(&sanitized.function.arguments);
    validate_tool_args(sanitized.function.name, &sanitized.function.arguments).map_err(
        |error| ToolCallPreflightError {
            call_id: sanitized.call_id.clone(),
            tool_name: sanitized.function.name,
            error,
        },
    )?;
    Ok(sanitized)
}

fn validate_tool_args(tool_name: ToolName, args: &str) -> Result<(), ToolError> {
    match tool_name {
        ToolName::RequestCodeContext => validate_tool_args_with::<RequestCodeContextGat>(args),
        ToolName::ApplyCodeEdit => validate_tool_args_with::<GatCodeEdit>(args),
        ToolName::InsertRustItem => {
            validate_tool_args_with::<insert_rust_item::InsertRustItem>(args)
        }
        ToolName::CreateFile => validate_tool_args_with::<create_file::CreateFile>(args),
        ToolName::NsPatch => validate_tool_args_with::<ns_patch::NsPatch>(args),
        ToolName::NsRead => validate_tool_args_with::<ns_read::NsRead>(args),
        ToolName::CodeItemLookup => {
            validate_tool_args_with::<code_item_lookup::CodeItemLookup>(args)
        }
        ToolName::CodeItemEdges => validate_tool_args_with::<get_code_edges::CodeItemEdges>(args),
        ToolName::Cargo => validate_tool_args_with::<cargo::CargoTool>(args),
        ToolName::ListDir => validate_tool_args_with::<list_dir::ListDir>(args),
    }
}

fn validate_tool_args_with<T: Tool>(args: &str) -> Result<(), ToolError> {
    T::deserialize_params(args)
        .map(|_| ())
        .map_err(T::adapt_error)
}

// potential alternative for static dispatch, might be helpful for macro
pub(crate) async fn process_tool(tool_call: ToolCall, ctx: Ctx) -> color_eyre::Result<()> {
    // TODO: Implement this as the Clone method for Ctx
    // let new_ctx = Ctx {
    //     state: ctx.state.clone(),
    //     event_bus: ctx.event_bus.clone(),
    //     request_id: ctx.request_id,
    //     parent_id: ctx.parent_id,
    //     call_id: ctx.call_id.clone(),
    // };
    let name = tool_call.function.name;
    let args = sanitize_tool_args(&tool_call.function.arguments);
    tracing::debug!(target: DEBUG_TOOLS,
        request_id = %ctx.request_id,
        call_id = ?tool_call.call_id,
        args = ?args,
        tool = ?tool_call.function.name,
        "Processing tool"
    );
    tracing::info!(target: TOOL_CALL_TARGET,
        tool = %name.as_str(),
        request_id = %ctx.request_id,
        parent_id = %ctx.parent_id,
        call_id = %tool_call.call_id,
        args = %args,
        "tool_call_request"
    );
    match tool_call.function.name {
        ToolName::RequestCodeContext => {
            let params = request_code_context::RequestCodeContextGat::deserialize_params(&args)
                .map_err(|err| {
                    let terr = request_code_context::RequestCodeContextGat::adapt_error(err);
                    request_code_context::RequestCodeContextGat::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult {
                content,
                ui_payload,
            } = request_code_context::RequestCodeContextGat::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = request_code_context::RequestCodeContextGat::adapt_error(
                        ToolInvocationError::Exec(e),
                    );
                    RequestCodeContextGat::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            request_code_context::RequestCodeContextGat::emit_completed(&ctx, content, ui_payload);
            Ok(())
        }
        ToolName::ApplyCodeEdit => {
            let params = code_edit::GatCodeEdit::deserialize_params(&args).map_err(|err| {
                let terr = code_edit::GatCodeEdit::adapt_error(err);
                code_edit::GatCodeEdit::emit_err(&ctx, terr.clone());
                color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
            })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult {
                content,
                ui_payload,
            } = code_edit::GatCodeEdit::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = code_edit::GatCodeEdit::adapt_error(ToolInvocationError::Exec(e));
                    GatCodeEdit::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            code_edit::GatCodeEdit::emit_completed(&ctx, content, ui_payload);
            Ok(())
        }
        ToolName::InsertRustItem => {
            let params =
                insert_rust_item::InsertRustItem::deserialize_params(&args).map_err(|err| {
                    let terr = insert_rust_item::InsertRustItem::adapt_error(err);
                    insert_rust_item::InsertRustItem::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult {
                content,
                ui_payload,
            } = insert_rust_item::InsertRustItem::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr =
                        insert_rust_item::InsertRustItem::adapt_error(ToolInvocationError::Exec(e));
                    insert_rust_item::InsertRustItem::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            insert_rust_item::InsertRustItem::emit_completed(&ctx, content, ui_payload);
            Ok(())
        }
        ToolName::CreateFile => {
            let params = create_file::CreateFile::deserialize_params(&args).map_err(|err| {
                let terr = create_file::CreateFile::adapt_error(err);
                create_file::CreateFile::emit_err(&ctx, terr.clone());
                color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
            })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult {
                content,
                ui_payload,
            } = create_file::CreateFile::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = create_file::CreateFile::adapt_error(ToolInvocationError::Exec(e));
                    create_file::CreateFile::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            create_file::CreateFile::emit_completed(&ctx, content, ui_payload);
            Ok(())
        }
        ToolName::NsPatch => {
            let params = ns_patch::NsPatch::deserialize_params(&args).map_err(|err| {
                let terr = ns_patch::NsPatch::adapt_error(err);
                ns_patch::NsPatch::emit_err(&ctx, terr.clone());
                color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
            })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult {
                content,
                ui_payload,
            } = ns_patch::NsPatch::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = ns_patch::NsPatch::adapt_error(ToolInvocationError::Exec(e));
                    ns_patch::NsPatch::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            ns_patch::NsPatch::emit_completed(&ctx, content, ui_payload);
            Ok(())
        }
        ToolName::NsRead => {
            let params = ns_read::NsRead::deserialize_params(&args).map_err(|err| {
                let terr = ns_read::NsRead::adapt_error(err);
                ns_read::NsRead::emit_err(&ctx, terr.clone());
                color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
            })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult {
                content,
                ui_payload,
            } = ns_read::NsRead::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = ns_read::NsRead::adapt_error(ToolInvocationError::Exec(e));
                    ns_read::NsRead::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            ns_read::NsRead::emit_completed(&ctx, content, ui_payload);
            Ok(())
        }
        ToolName::CodeItemLookup => {
            let params =
                code_item_lookup::CodeItemLookup::deserialize_params(&args).map_err(|err| {
                    let terr = code_item_lookup::CodeItemLookup::adapt_error(err);
                    code_item_lookup::CodeItemLookup::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult {
                content,
                ui_payload,
            } = code_item_lookup::CodeItemLookup::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr =
                        code_item_lookup::CodeItemLookup::adapt_error(ToolInvocationError::Exec(e));
                    code_item_lookup::CodeItemLookup::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            code_item_lookup::CodeItemLookup::emit_completed(&ctx, content, ui_payload);
            Ok(())
        }
        ToolName::CodeItemEdges => {
            let params =
                get_code_edges::CodeItemEdges::deserialize_params(&args).map_err(|err| {
                    let terr = get_code_edges::CodeItemEdges::adapt_error(err);
                    get_code_edges::CodeItemEdges::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult {
                content,
                ui_payload,
            } = get_code_edges::CodeItemEdges::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr =
                        get_code_edges::CodeItemEdges::adapt_error(ToolInvocationError::Exec(e));
                    get_code_edges::CodeItemEdges::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            get_code_edges::CodeItemEdges::emit_completed(&ctx, content, ui_payload);
            Ok(())
        }
        ToolName::ListDir => {
            let params = list_dir::ListDir::deserialize_params(&args).map_err(|err| {
                let terr = list_dir::ListDir::adapt_error(err);
                list_dir::ListDir::emit_err(&ctx, terr.clone());
                color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
            })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult {
                content,
                ui_payload,
            } = list_dir::ListDir::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = list_dir::ListDir::adapt_error(ToolInvocationError::Exec(e));
                    list_dir::ListDir::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            list_dir::ListDir::emit_completed(&ctx, content, ui_payload);
            Ok(())
        }
        ToolName::Cargo => {
            let params = cargo::CargoTool::deserialize_params(&args).map_err(|err| {
                let terr = cargo::CargoTool::adapt_error(err);
                cargo::CargoTool::emit_err(&ctx, terr.clone());
                color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
            })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult {
                content,
                ui_payload,
            } = cargo::CargoTool::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = cargo::CargoTool::adapt_error(ToolInvocationError::Exec(e));
                    cargo::CargoTool::emit_err(&ctx, terr.clone());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            cargo::CargoTool::emit_completed(&ctx, content, ui_payload);
            Ok(())
        }
    }
}

const TOOL_ARG_SUFFIXES: [&str; 1] = ["<|tool_call_end|>"];

fn sanitize_tool_args(args: &str) -> String {
    let mut cutoff: Option<usize> = None;
    for token in TOOL_ARG_SUFFIXES {
        if let Some(idx) = args.find(token) {
            cutoff = Some(cutoff.map_or(idx, |current| current.min(idx)));
        }
    }
    match cutoff {
        Some(idx) => args[..idx].trim().to_string(),
        None => args.trim().to_string(),
    }
}

static TOOL_PERSIST_SENDER: OnceCell<mpsc::Sender<ToolCallRecord>> = OnceCell::new();

pub fn set_tool_persist_sender(tx: mpsc::Sender<ToolCallRecord>) {
    let _ = TOOL_PERSIST_SENDER.set(tx);
}

pub trait Tool {
    type Output: Serialize + Send;
    type OwnedParams: Serialize + Send;
    type Params<'de>: Deserialize<'de> + Send
    where
        Self: 'de;

    fn name() -> ToolName;
    fn description() -> ToolDescription;
    fn schema() -> &'static serde_json::Value;

    fn build(ctx: &Ctx) -> Self
    where
        Self: Sized;

    fn into_owned<'de>(params: &Self::Params<'de>) -> Self::OwnedParams;

    /// Map transport/validation/exec errors into a structured ToolError.
    fn adapt_error(err: ToolInvocationError) -> ToolError {
        err.into_tool_error(Self::name())
    }

    fn tool_def() -> ToolDefinition {
        ToolFunctionDef {
            name: Self::name(),
            description: Self::description().to_string(),
            parameters: Self::schema().clone(),
        }
        .into()
    }

    fn emit_completed(ctx: &Ctx, output_json: String, ui_payload: Option<ToolUiPayload>) {
        let _ = ctx.event_bus.realtime_tx.send(crate::AppEvent::System(
            SystemEvent::ToolCallCompleted {
                request_id: ctx.request_id,
                parent_id: ctx.parent_id,
                call_id: ctx.call_id.clone(),
                content: output_json,
                ui_payload,
            },
        ));
    }

    fn emit_err(ctx: &Ctx, error: ToolError) {
        let ui_payload = Some(ToolUiPayload::from_error(ctx.call_id.clone(), &error));
        let _ =
            ctx.event_bus
                .realtime_tx
                .send(crate::AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id: ctx.request_id,
                    parent_id: ctx.parent_id,
                    call_id: ctx.call_id.clone(),
                    error: error.to_wire_string(),
                    ui_payload,
                }));
    }

    // Helper for deserializing the arguments JSON string
    fn deserialize_params<'a>(json: &'a str) -> Result<Self::Params<'a>, ToolInvocationError> {
        serde_json::from_str(json).map_err(|e| ToolInvocationError::Deserialize {
            source: e,
            raw: Some(json.to_string()),
        })
    }

    fn execute<'de>(
        params: Self::Params<'de>,
        ctx: Ctx,
    ) -> impl std::future::Future<Output = Result<ToolResult, ploke_error::Error>> + Send;
}

pub trait ValidatesAbolutePath {
    fn get_file_path(&self) -> impl AsRef<Path>;
    fn validate_to_abs_path(
        &self,
        primary_root: &Path,
        policy: &PathPolicy,
    ) -> Result<PathBuf, ploke_error::Error> {
        path_scoping::resolve_tool_path(self.get_file_path().as_ref(), primary_root, policy)
            .map_err(|err| {
                ploke_error::Error::Domain(DomainError::Io {
                    message: format!("invalid path: {err}"),
                })
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::borrow::Cow;
    use std::sync::Arc;
    use tokio::time::{Duration, timeout};

    #[test]
    fn function_marker_roundtrip() {
        let fm = FunctionMarker;
        let v = serde_json::to_value(fm).expect("serialize");
        assert_eq!(v, serde_json::Value::String("function".to_string()));

        let de: FunctionMarker = serde_json::from_value(v).expect("deserialize 'function'");
        let bad = serde_json::from_str::<FunctionMarker>("\"not-a-function\"");
        assert!(bad.is_err(), "unexpectedly accepted non-'function' value");
        let _ = de; // silence unused
    }

    #[test]
    fn tool_name_as_str_mappings() {
        assert_eq!(
            ToolName::RequestCodeContext.as_str(),
            "request_code_context"
        );
        assert_eq!(ToolName::ApplyCodeEdit.as_str(), "apply_code_edit");
        assert_eq!(ToolName::InsertRustItem.as_str(), "insert_rust_item");
        assert_eq!(ToolName::CreateFile.as_str(), "create_file");
    }

    #[test]
    fn tool_definition_serializes_expected_shape() {
        let def = ToolDefinition {
            r#type: FunctionMarker,
            function: ToolFunctionDef {
                name: ToolName::ApplyCodeEdit,
                description: ToolName::ApplyCodeEdit.description().to_string(),
                parameters: json!({"type": "object"}),
            },
        };
        let v = serde_json::to_value(&def).expect("serialize");
        let obj = v.as_object().expect("obj");
        assert_eq!(obj.get("type").and_then(|v| v.as_str()), Some("function"));
        let f = obj
            .get("function")
            .and_then(|f| f.as_object())
            .expect("function obj");
        assert_eq!(
            f.get("name").and_then(|n| n.as_str()),
            Some("apply_code_edit")
        );
        assert!(f.contains_key("parameters"));
    }

    #[test]
    fn validate_and_sanitize_tool_call_strips_suffix_tokens() {
        let tool_call = ToolCall {
            call_id: ArcStr::from("call_suffix"),
            call_type: FunctionMarker,
            function: ploke_llm::response::FunctionCall {
                name: ToolName::ListDir,
                arguments: "{\"dir\":\"src\",\"max_entries\":5}<|tool_call_end|>".to_string(),
            },
        };

        let validated = validate_and_sanitize_tool_call(&tool_call).expect("validated tool call");

        assert_eq!(
            validated.function.arguments,
            "{\"dir\":\"src\",\"max_entries\":5}"
        );
    }

    #[test]
    fn validate_and_sanitize_tool_call_rejects_malformed_json() {
        let tool_call = ToolCall {
            call_id: ArcStr::from("call_bad_json"),
            call_type: FunctionMarker,
            function: ploke_llm::response::FunctionCall {
                name: ToolName::NsRead,
                arguments: "{\"file\":\"src/lib.rs\",\"start_line\":".to_string(),
            },
        };

        let err = validate_and_sanitize_tool_call(&tool_call).expect_err("malformed json");

        assert_eq!(err.tool_name, ToolName::NsRead);
        assert!(err.error.message.contains("failed to parse tool arguments"));
    }

    #[test]
    fn validate_and_sanitize_tool_call_rejects_schema_invalid_args() {
        let tool_call = ToolCall {
            call_id: ArcStr::from("call_bad_schema"),
            call_type: FunctionMarker,
            function: ploke_llm::response::FunctionCall {
                name: ToolName::Cargo,
                arguments: "{\"command\":\"fmt\",\"scope\":\"workspace\"}".to_string(),
            },
        };

        let err = validate_and_sanitize_tool_call(&tool_call).expect_err("schema invalid args");

        assert_eq!(err.tool_name, ToolName::Cargo);
        assert!(err.error.message.contains("unknown variant"));
    }
}
