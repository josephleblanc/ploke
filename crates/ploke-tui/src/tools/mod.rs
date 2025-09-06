#![allow(clippy::needless_lifetimes)]

// For any db-related issues, check cross-crate contract with db in:
//  `ploke/crates/ploke-tui/docs/crate-contracts/tool-to-ploke-db.md`
use crate::utils::{consts::DEBUG_TOOLS, se_de::{ de_arc_str, se_arc_str }};
use std::{borrow::Cow, collections::HashMap, ops::Deref, path::PathBuf, sync::Arc};

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
pub mod get_file_metadata;
pub use get_file_metadata::{GetFileMetadata, GetFileMetadataInput};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    RequestCodeContext,
    ApplyCodeEdit,
    GetFileMetadata,
}

impl ToolName {
    pub fn as_str(self) -> &'static str {
        use ToolName::*;
        match self {
            RequestCodeContext => "request_code_context",
            ApplyCodeEdit => "apply_code_edit",
            GetFileMetadata => "get_file_metadata",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq)]
pub enum ToolDescr {
    #[serde(rename = "Request additional code context from the repository up to a token budget.")]
    RequestCodeContext,
    #[serde(
        rename = "Apply canonical code edits to one or more nodes identified by canonical path."
    )]
    ApplyCodeEdit,
    #[serde(
        rename = "Fetch current file metadata to obtain the expected_file_hash (tracking hash UUID) for safe edits."
    )]
    GetFileMetadata,
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq)]
pub struct FunctionMarker;

impl FunctionMarker {
    pub const VALUE: &'static str = "function";
}

// Serialize/deserialize always produces/expects the string "function".
impl Serialize for FunctionMarker {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(Self::VALUE)
    }
}

impl<'de> Deserialize<'de> for FunctionMarker {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct V;
        impl serde::de::Visitor<'_> for V {
            type Value = FunctionMarker;
            fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
                write!(f, "\"{}\"", FunctionMarker::VALUE)
            }
            fn visit_str<E>(self, s: &str) -> Result<Self::Value, E>
            where
                E: serde::de::Error,
            {
                if s == FunctionMarker::VALUE {
                    Ok(FunctionMarker)
                } else {
                    Err(E::invalid_value(serde::de::Unexpected::Str(s), &self))
                }
            }
        }
        deserializer.deserialize_str(V)
    }
}

// OpenAI tool/function definition (for request payload)
#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub r#type: FunctionMarker,
    pub function: ToolFunctionDef,
}

impl From<ToolFunctionDef> for ToolDefinition {
    fn from(val: ToolFunctionDef) -> Self {
        ToolDefinition {
            r#type: FunctionMarker,
            function: val,
        }
    }
}

#[derive(Serialize, Debug, Clone, Deserialize)]
pub struct ToolFunctionDef {
    pub name: ToolName,
    pub description: ToolDescr,
    // TODO: We want to make this something more type-safe, e.g. instead of Value, it should be
    // generic over the types that implement a tool-calling trait
    // - DO NOT use dynamic dispatch
    // - DO use static dispatch
    // - will likely require changing stuct definition to include a generic type e.g.
    // `ToolFunctionDef<T>`
    // - zero-alloc and zero-copy wherever possible, tools are hot path
    pub parameters: Value, // JSON Schema
}

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

#[derive(Deserialize, Serialize, Debug, Clone, PartialOrd, PartialEq)]
pub struct ToolCall {
    #[serde(deserialize_with = "de_arc_str", serialize_with="se_arc_str", rename = "id")]
    pub call_id: ArcStr,

    #[serde(rename = "type")]
    pub call_type: FunctionMarker,
    pub function: FunctionCall,
}

#[derive(Deserialize, Serialize, Debug, Clone, PartialOrd, PartialEq)]
pub struct FunctionCall {
    pub name: ToolName,
    // Store raw JSON arguments - needs to be owned String for deserialization from OpenRouter
    pub arguments: String,
}

// Tool result structure
#[derive(Debug, Serialize)]
pub struct ToolResult {
    pub content: String,
}

// Tool errors
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Deserialization Error: {0}")]
    DeserializationError(String),
    #[error("Execution Error: {0}")]
    ExecutionError(String),
}

#[allow(clippy::from_over_into)]
impl Into<ploke_error::Error> for ToolError {
    fn into(self) -> ploke_error::Error {
        use ToolError::*;
        match self {
            DeserializationError(s) => {
                ploke_error::Error::Internal(ploke_error::InternalError::NotImplemented(s))
            }
            ExecutionError(s) => {
                ploke_error::Error::Internal(ploke_error::InternalError::NotImplemented(s))
            }
        }
    }
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
            let params = request_code_context::RequestCodeContextGat::deserialize_params(&args)?;
            tracing::debug!(target: DEBUG_TOOLS, 
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult { content } =
                request_code_context::RequestCodeContextGat::execute(params, ctx.clone())
                    .await
                    .inspect_err(|e| RequestCodeContextGat::emit_err(&ctx, e.to_string()))?;
            tracing::debug!(target: DEBUG_TOOLS, 
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            request_code_context::RequestCodeContextGat::emit_completed(&ctx, content);
            Ok(())
        }
        ToolName::ApplyCodeEdit => {
            let params = code_edit::GatCodeEdit::deserialize_params(&args)?;
            tracing::debug!(target: DEBUG_TOOLS, 
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult { content } = code_edit::GatCodeEdit::execute(params, ctx.clone())
                .await
                .inspect_err(|e| GatCodeEdit::emit_err(&ctx, e.to_string()))?;
            tracing::debug!(target: DEBUG_TOOLS, 
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            code_edit::GatCodeEdit::emit_completed(&ctx, content);
            Ok(())
        }
        ToolName::GetFileMetadata => {
            let params = get_file_metadata::GetFileMetadata::deserialize_params(&args)?;
            tracing::debug!(target: DEBUG_TOOLS, 
                "params: {}\n",
                format_args!("{:#?}", &params),
            );
            let ToolResult { content } =
                get_file_metadata::GetFileMetadata::execute(params, ctx.clone())
                    .await
                    .inspect_err(|e| GetFileMetadata::emit_err(&ctx, e.to_string()))?;
            tracing::debug!(target: DEBUG_TOOLS, 
                "content: {}\n",
                format_args!("{:#?}", &content),
            );
            get_file_metadata::GetFileMetadata::emit_completed(&ctx, content);
            Ok(())
        } // more here
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
    fn deserialize_params<'a>(json: &'a str) -> Result<Self::Params<'a>, ToolError> {
        serde_json::from_str(json).map_err(|e| ToolError::DeserializationError(e.to_string()))
    }

    fn execute<'de>(
        params: Self::Params<'de>,
        ctx: Ctx,
    ) -> impl std::future::Future<Output = Result<ToolResult, ploke_error::Error>> + Send;
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
        assert_eq!(ToolName::GetFileMetadata.as_str(), "get_file_metadata");
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

    #[tokio::test]
    async fn process_tool_emits_completed_for_get_file_metadata() {
        use crate::test_utils::mock::create_mock_app_state;
        use crate::{AppEvent, EventBus, event_bus::EventBusCaps};

        // Prepare ctx
        let state = Arc::new(create_mock_app_state());
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let call_id = ArcStr::from("test-call");
        let ctx = Ctx {
            state: Arc::clone(&state),
            event_bus: Arc::clone(&event_bus),
            request_id: Uuid::new_v4(),
            parent_id: Uuid::new_v4(),
            call_id: call_id.clone(),
        };

        // Create a temporary file
        let tmp = std::env::temp_dir().join(format!("gat_meta_{}.txt", Uuid::new_v4()));
        std::fs::write(&tmp, b"abc").expect("write temp");

        // Build ToolCall
        let args = json!({"file_path": tmp.display().to_string()}).to_string();
        let tc = ToolCall {
            call_id: ArcStr::from("test-call"),
            call_type: FunctionMarker,
            function: FunctionCall {
                name: ToolName::GetFileMetadata,
                arguments: args,
            },
        };

        let mut rx = event_bus.realtime_tx.subscribe();
        process_tool(tc, ctx.clone()).await.expect("process ok");

        // Expect a ToolCallCompleted with matching call_id
        let got = timeout(Duration::from_secs(1), async move {
            loop {
                match rx.recv().await {
                    Ok(AppEvent::System(SystemEvent::ToolCallCompleted {
                        call_id,
                        content,
                        ..
                    })) => {
                        if call_id == ctx.call_id {
                            break Some(content);
                        }
                    }
                    Ok(_) => continue,
                    Err(_) => break None,
                }
            }
        })
        .await
        .expect("timeout waiting for event");

        assert!(got.is_some(), "no ToolCallCompleted captured");
    }
}
