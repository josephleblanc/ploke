#![allow(clippy::needless_lifetimes)]

// For any db-related issues, check cross-crate contract with db in:
//  `ploke/crates/ploke-tui/docs/crate-contracts/tool-to-ploke-db.md`
use crate::utils::{
    consts::DEBUG_TOOLS,
    path_scoping,
    se_de::{de_arc_str, se_arc_str},
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
use ploke_rag::{RagService, RetrievalStrategy, TokenBudget};
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use serde_json::{Value, json};
use syn_parser::parser::nodes::NodePath;
use tokio::sync::mpsc;
use uuid::Uuid;

pub mod request_code_context;
pub use request_code_context::{
    RequestCodeContext, RequestCodeContextGat, RequestCodeContextInput,
};
pub mod code_edit;
pub use code_edit::{CanonicalEdit, CodeEdit, CodeEditInput, GatCodeEdit};
pub mod code_item_lookup;
pub mod create_file;
pub mod error;
pub mod ns_patch;
pub mod ns_read;
pub mod validators;

pub use error::{Audience, ToolError, ToolErrorCode, ToolErrorWire, ToolInvocationError};

// NOTE:ploke-llm
// Moved ToolName and its `as_str` implementation into `ploke-core` for now so `ploke-llm` and
// `ploke-tui` have shared access to the same underlying types.
pub use ploke_core::tool_types::ToolName;

// NOTE:ploke-llm
// moved ToolDescr into ploke-core to make available to `ploke-llm` as we refactor ploke-tui::llm
// into ploke-llm
pub use ploke_core::tool_types::ToolDescr;

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
    let args = tool_call.function.arguments;
    tracing::debug!(target: DEBUG_TOOLS,
        request_id = %ctx.request_id,
        call_id = ?tool_call.call_id,
        args = ?args,
        tool = ?tool_call.function.name,
        "Processing tool"
    );
    match tool_call.function.name {
        ToolName::RequestCodeContext => {
            let params = request_code_context::RequestCodeContextGat::deserialize_params(&args)
                .map_err(|err| {
                    let terr = request_code_context::RequestCodeContextGat::adapt_error(err);
                    request_code_context::RequestCodeContextGat::emit_err(
                        &ctx,
                        terr.to_wire_string(),
                    );
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult { content } =
                request_code_context::RequestCodeContextGat::execute(params, ctx.clone())
                    .await
                    .map_err(|e| {
                        let terr = request_code_context::RequestCodeContextGat::adapt_error(
                            ToolInvocationError::Exec(e),
                        );
                        RequestCodeContextGat::emit_err(&ctx, terr.to_wire_string());
                        color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                    })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            request_code_context::RequestCodeContextGat::emit_completed(&ctx, content);
            Ok(())
        }
        ToolName::ApplyCodeEdit => {
            let params = code_edit::GatCodeEdit::deserialize_params(&args).map_err(|err| {
                let terr = code_edit::GatCodeEdit::adapt_error(err);
                code_edit::GatCodeEdit::emit_err(&ctx, terr.to_wire_string());
                color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
            })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult { content } = code_edit::GatCodeEdit::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = code_edit::GatCodeEdit::adapt_error(ToolInvocationError::Exec(e));
                    GatCodeEdit::emit_err(&ctx, terr.to_wire_string());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            code_edit::GatCodeEdit::emit_completed(&ctx, content);
            Ok(())
        }
        ToolName::CreateFile => {
            let params = create_file::CreateFile::deserialize_params(&args).map_err(|err| {
                let terr = create_file::CreateFile::adapt_error(err);
                create_file::CreateFile::emit_err(&ctx, terr.to_wire_string());
                color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
            })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult { content } = create_file::CreateFile::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = create_file::CreateFile::adapt_error(ToolInvocationError::Exec(e));
                    create_file::CreateFile::emit_err(&ctx, terr.to_wire_string());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            create_file::CreateFile::emit_completed(&ctx, content);
            Ok(())
        }
        ToolName::NsPatch => {
            let params = ns_patch::NsPatch::deserialize_params(&args).map_err(|err| {
                let terr = ns_patch::NsPatch::adapt_error(err);
                ns_patch::NsPatch::emit_err(&ctx, terr.to_wire_string());
                color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
            })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult { content } = ns_patch::NsPatch::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = ns_patch::NsPatch::adapt_error(ToolInvocationError::Exec(e));
                    ns_patch::NsPatch::emit_err(&ctx, terr.to_wire_string());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            ns_patch::NsPatch::emit_completed(&ctx, content);
            Ok(())
        }
        ToolName::NsRead => {
            let params = ns_read::NsRead::deserialize_params(&args).map_err(|err| {
                let terr = ns_read::NsRead::adapt_error(err);
                ns_read::NsRead::emit_err(&ctx, terr.to_wire_string());
                color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
            })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult { content } = ns_read::NsRead::execute(params, ctx.clone())
                .await
                .map_err(|e| {
                    let terr = ns_read::NsRead::adapt_error(ToolInvocationError::Exec(e));
                    ns_read::NsRead::emit_err(&ctx, terr.to_wire_string());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            ns_read::NsRead::emit_completed(&ctx, content);
            Ok(())
        }
        ToolName::CodeItemLookup => {
            let params =
                code_item_lookup::CodeItemLookup::deserialize_params(&args).map_err(|err| {
                    let terr = code_item_lookup::CodeItemLookup::adapt_error(err);
                    code_item_lookup::CodeItemLookup::emit_err(&ctx, terr.to_wire_string());
                    color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult { content } =
                code_item_lookup::CodeItemLookup::execute(params, ctx.clone())
                    .await
                    .map_err(|e| {
                        let terr = code_item_lookup::CodeItemLookup::adapt_error(
                            ToolInvocationError::Exec(e),
                        );
                        code_item_lookup::CodeItemLookup::emit_err(&ctx, terr.to_wire_string());
                        color_eyre::eyre::eyre!(terr.format_for_audience(Audience::System))
                    })?;
            tracing::debug!(target: DEBUG_TOOLS,
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            code_item_lookup::CodeItemLookup::emit_completed(&ctx, content);
            Ok(())
        }
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
    fn description() -> ToolDescr;
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
            description: Self::description(),
            parameters: Self::schema().clone(),
        }
        .into()
    }

    fn emit_completed(ctx: &Ctx, output_json: String) {
        let _ = ctx.event_bus.realtime_tx.send(crate::AppEvent::System(
            SystemEvent::ToolCallCompleted {
                request_id: ctx.request_id,
                parent_id: ctx.parent_id,
                call_id: ctx.call_id.clone(),
                content: output_json,
            },
        ));
    }

    fn emit_err(ctx: &Ctx, error: String) {
        let _ =
            ctx.event_bus
                .realtime_tx
                .send(crate::AppEvent::System(SystemEvent::ToolCallFailed {
                    request_id: ctx.request_id,
                    parent_id: ctx.parent_id,
                    call_id: ctx.call_id.clone(),
                    error,
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
    fn validate_to_abs_path<T: AsRef<Path>>(
        &self,
        crate_root: T,
    ) -> Result<PathBuf, ploke_error::Error> {
        path_scoping::resolve_in_crate_root(self.get_file_path(), &crate_root).map_err(|err| {
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
        assert_eq!(ToolName::CreateFile.as_str(), "create_file");
    }

    #[test]
    fn tool_definition_serializes_expected_shape() {
        let def = ToolDefinition {
            r#type: FunctionMarker,
            function: ToolFunctionDef {
                name: ToolName::ApplyCodeEdit,
                description: ToolDescr::ApplyCodeEdit,
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
}
