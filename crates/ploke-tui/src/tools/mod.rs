use std::{ops::Deref, path::PathBuf, sync::Arc};

use itertools::Itertools;
use ploke_core::rag_types::{ContextPart, ContextPartKind, Modality};
use ploke_rag::{RagService, RetrievalStrategy, TokenBudget};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use syn_parser::parser::nodes::NodePath;
use uuid::Uuid;
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

pub trait Tool {
    /// Static strings so they can be used in const contexts.
    const NAME: &'static str;
    const DESCRIPTION: &'static str;

    type Params: for<'de> Deserialize<'de>;
    type Output: Serialize;

    fn run(self, params: Self::Params) -> impl std::future::Future<Output = Result<Self::Output, ploke_error::Error>> + Send;

    /// JSON schema built at compile time via `schemars` or `serde_json!`.
    fn schema() -> &'static serde_json::Value;

    fn tool_def() -> ToolFunctionDef;
}


#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    RequestCodeContext,
    ApplyCodeEdit,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq)]
pub enum ToolDescr {
    #[serde(rename = "Request additional code context from the repository up to a token budget.")]
    RequestCodeContext,
    #[serde(rename = "Apply canonical code edits to one or more nodes identified by canonical path.")]
    ApplyCodeEdit,
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

#[tracing::instrument(skip(tool_call_params),
    fields( 
        request_id = %tool_call_params.request_id,
        parent_id = %tool_call_params.parent_id,
        call_id = %tool_call_params.call_id
    )
)]
pub async fn handle_tool_call_requested(tool_call_params: ToolCallParams<'_>) {
    let ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
 
        name,
        arguments,
        call_id,
    } = tool_call_params.clone();
    tracing::info!(
        request_id = %request_id,
        parent_id = %parent_id,
        call_id = %call_id,
 
        name = %name,
        "handle_tool_call_requested"
    );
    let tool_call_failed = |error| {
        AppEvent::System(SystemEvent::ToolCallFailed {
            request_id,
            parent_id,
            call_id: call_id.clone(),
            error,
        })
    };

    let tool_call_params = ToolCallParams {
        state,
        event_bus,
        request_id,
        parent_id,
 
        name: name.clone(),
        arguments,
        call_id: call_id.clone(),
    };
    match name.as_str() {
        "apply_code_edit" => apply_code_edit_tool(tool_call_params).await,
        // New: get_file_metadata tool for fetching current file hash and basic metadata
        "get_file_metadata" => get_file_metadata_tool(tool_call_params).await,
        // "request_code_context" => handle_request_context(tool_call_params).await,
        _ => {
            tracing::warn!("Unsupported tool call: {}", name);
            let err = format!("Unsupported tool: {}", name);
            let _ = event_bus.realtime_tx.send(tool_call_failed(err.clone()));
            return;
        }
    }
}
