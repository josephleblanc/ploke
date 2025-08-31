#![allow(clippy::needless_lifetimes)]

use std::{ops::Deref, path::PathBuf, sync::Arc};

use itertools::Itertools;
use ploke_core::rag_types::{ContextPart, ContextPartKind, Modality};
use ploke_rag::{RagService, RetrievalStrategy, TokenBudget};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use syn_parser::parser::nodes::NodePath;
use uuid::Uuid;
use tokio::sync::mpsc;
use once_cell::sync::OnceCell;
use crate::{
    rag::{
        tools::{
            apply_code_edit_tool,
            get_file_metadata_tool,
            handle_request_context
        },
        utils::ToolCallParams
    },
    system::SystemEvent, 
    AppEvent
};

mod request_code_context;
pub use request_code_context::{ RequestCodeContext, RequestCodeContextInput, RequestCodeContextOutput };
mod code_edit;
pub use code_edit::{ CodeEdit, CodeEditInput, CanonicalEdit };
mod get_file_metadata;
pub use get_file_metadata::{ GetFileMetadataTool, GetFileMetadataInput };

pub trait Tool {
    const NAME: &'static str;
    const DESCRIPTION: &'static str;

    type Params: for<'de> Deserialize<'de>;
    type Output: Serialize;

    fn run(self, params: Self::Params) -> impl std::future::Future<Output = Result<Self::Output, ploke_error::Error>> + Send;
    fn schema() -> &'static serde_json::Value;
    fn tool_def() -> ToolFunctionDef;
}

pub trait ToolFromParams: Tool {
    fn build(params: &ToolCallParams) -> Self;
}

#[tracing::instrument(skip(tool_call_params))]
pub async fn dispatch_tool<T>(tool_call_params: ToolCallParams)
where
    T: Tool + ToolFromParams,
{
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        name: _,
        arguments,
        call_id,
    } = tool_call_params.clone();

    // Parse typed params
    let parsed: T::Params = match serde_json::from_value(arguments.clone()) {
        Ok(v) => v,
        Err(e) => {
            let err = format!("Invalid payload for {}: {}", T::NAME, e);
            let _ = event_bus.realtime_tx.send(AppEvent::System(SystemEvent::ToolCallFailed {
                request_id,
                parent_id,
                call_id: call_id.clone(),
                error: err,
            }));
            return;
        }
    };

    // Build and run tool. Tools are responsible for emitting Completed events if needed.
    let tool = T::build(&tool_call_params);
    if let Err(e) = tool.run(parsed).await {
        let _ = event_bus.realtime_tx.send(AppEvent::System(SystemEvent::ToolCallFailed {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            error: e.to_string(),
        }));
    }
}


#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    RequestCodeContext,
    ApplyCodeEdit,
    GetFileMetadata,
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq)]
pub enum ToolDescr {
    #[serde(rename = "Request additional code context from the repository up to a token budget.")]
    RequestCodeContext,
    #[serde(rename = "Apply canonical code edits to one or more nodes identified by canonical path.")]
    ApplyCodeEdit,
    #[serde(rename = "Fetch current file metadata to obtain the expected_file_hash (tracking hash UUID) for safe edits.")]
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
#[derive(Clone, Debug)]
pub struct Ctx {
    pub state: Arc<crate::app_state::AppState>,
    pub event_bus: Arc<crate::EventBus>,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub call_id: Arc<str>,
}

#[derive(Clone, Debug)]
pub struct ToolCallRecord {
    pub name: ToolName,
    pub request_id: Uuid,
    pub parent_id: Uuid,
    pub call_id: String,
    pub params_json: Arc<String>,
    pub output_json: Arc<String>,
    pub created_at_ms: i64,
}

static TOOL_PERSIST_SENDER: OnceCell<mpsc::Sender<ToolCallRecord>> = OnceCell::new();

pub fn set_tool_persist_sender(tx: mpsc::Sender<ToolCallRecord>) {
    let _ = TOOL_PERSIST_SENDER.set(tx);
}

pub trait GatTool {
    type Output: Serialize + Send + 'static;
    type OwnedParams: Serialize + Send + 'static;
    type Params<'de>: Deserialize<'de> + Send;

    fn name() -> ToolName;
    fn description() -> ToolDescr;
    fn schema() -> &'static serde_json::Value;

    fn build(ctx: &Ctx) -> Self
    where
        Self: Sized;

    fn into_owned<'a>(params: &Self::Params<'a>) -> Self::OwnedParams;

    fn tool_def() -> ToolFunctionDef {
        ToolFunctionDef {
            name: Self::name(),
            description: Self::description(),
            parameters: Self::schema().clone(),
        }
    }

    fn emit_completed(ctx: &Ctx, output_json: String) {
        let _ = ctx.event_bus.realtime_tx.send(crate::AppEvent::System(crate::system::SystemEvent::ToolCallCompleted {
            request_id: ctx.request_id,
            parent_id: ctx.parent_id,
            call_id: ctx.call_id.to_string(),
            content: output_json,
        }));
    }

    fn run<'a>(self, params: &Self::Params<'a>, ctx: Ctx)
        -> impl std::future::Future<Output = Result<Self::Output, ploke_error::Error>> + Send;
}

pub async fn dispatch_tool_gat<T>(tool_call_params: ToolCallParams)
where
    T: GatTool,
{
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
        name: _name,
        arguments,
        call_id,
    } = tool_call_params.clone();

    let ctx = Ctx {
        state: Arc::clone(&state),
        event_bus: Arc::clone(&event_bus),
        request_id,
        parent_id,
        call_id: Arc::from(call_id.as_str()),
    };

    let args_str = match serde_json::to_string(&arguments) {
        Ok(s) => s,
        Err(e) => {
            let err = format!("Failed to serialize tool arguments: {}", e);
            let _ = ctx.event_bus.realtime_tx.send(crate::AppEvent::System(crate::system::SystemEvent::ToolCallFailed {
                request_id: ctx.request_id,
                parent_id: ctx.parent_id,
                call_id: ctx.call_id.to_string(),
                error: err,
            }));
            return;
        }
    };
    let args_arc: Arc<str> = Arc::from(args_str);

    let params: T::Params<'_> = match serde_json::from_str(&args_arc) {
        Ok(v) => v,
        Err(e) => {
            let err = format!("Invalid payload for {:?}: {}", T::name(), e);
            let _ = ctx.event_bus.realtime_tx.send(crate::AppEvent::System(crate::system::SystemEvent::ToolCallFailed {
                request_id: ctx.request_id,
                parent_id: ctx.parent_id,
                call_id: ctx.call_id.to_string(),
                error: err,
            }));
            return;
        }
    };

    let tool = T::build(&ctx);
    match tool.run(&params, ctx.clone()).await {
        Ok(output) => {
            let output_json = serde_json::to_string(&output).unwrap_or_else(|_| "{}".to_string());
            T::emit_completed(&ctx, output_json.clone());
            let owned_params = T::into_owned(&params);
            let params_json: Arc<String> = Arc::new(serde_json::to_string(&owned_params).unwrap_or_else(|_| "{}".to_string()));
            let output_json: Arc<String> = Arc::new(output_json);
            if let Some(tx) = TOOL_PERSIST_SENDER.get() {
                let _ = tx.try_send(ToolCallRecord {
                    name: T::name(),
                    request_id: ctx.request_id,
                    parent_id: ctx.parent_id,
                    call_id: ctx.call_id.to_string(),
                    params_json: params_json.clone(),
                    output_json: output_json.clone(),
                    created_at_ms: chrono::Utc::now().timestamp_millis(),
                });
            }

        }
        Err(e) => {
            let _ = ctx.event_bus.realtime_tx.send(crate::AppEvent::System(crate::system::SystemEvent::ToolCallFailed {
                request_id: ctx.request_id,
                parent_id: ctx.parent_id,
                call_id: ctx.call_id.to_string(),
                error: e.to_string(),
            }));
        }
    }
}
