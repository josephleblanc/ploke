use std::path::PathBuf;

use itertools::Itertools;
use ploke_core::rag_types::{ContextPart, ContextPartKind, Modality};
use ploke_rag::{RagService, RetrievalStrategy};
use serde_json::{json, Value};
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use syn_parser::parser::nodes::NodePath;

pub trait Tool {
    /// Static strings so they can be used in const contexts.
    const NAME: &'static str;
    const DESCRIPTION: &'static str;

    type Params: for<'de> Deserialize<'de>;
    type Output: Serialize;

    async fn run(self, params: Self::Params) -> Result<Self::Output, ploke_error::Error>;

    /// JSON schema built at compile time via `schemars` or `serde_json!`.
    fn schema() -> &'static serde_json::Map<String, Value>;
}
 // crates/ploke-tui/src/tools/apply_code_edit.rs
 use super::*;

 pub struct RequstCodeContext {
    rag: Arc<RagService>,
    top_k: u16,
    budget: TokenBudget,
    strategy: RetrievalStrategy,
}

#[derive(Clone, PartialOrd, PartialEq, Deserialize)]
pub struct RequestCodeContextInput {
    hint: String,
    top_k: Option<u16>,
}

#[derive(Clone, PartialOrd, PartialEq, Serialize)]
pub struct RequestCodeContextOutput {
    code: Vec<CodeSnippet>,
    meta: Vec<SnippetMeta>
}

#[derive(Debug, Clone, PartialOrd, PartialEq, Serialize, Ord, Eq)]
pub struct CodeSnippet {
    file_path: String,
    snippet: String,
    // canonical_path: NodePath
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Serialize)]
pub struct SnippetMeta {
    id: Uuid,
    kind: ContextPartKind,
    score: f32,
    modality: Modality
}

impl SnippetMeta {
    fn extract_meta(cp: &ContextPart) -> Self {
        SnippetMeta {
            id: cp.id, 
            kind: cp.kind, 
            score: cp.score, 
            modality: cp.modality
        }
    }
}

lazy_static::lazy_static! {
    static ref REQUEST_CODE_CONTEXT_PARAMETERS: serde_json::Value = json!({
        "type": "object",
        "properties": {
            "search_term": {
                "type": "string",
                "description": "The text used to perform a dense vector similarity and bm25 hybrid search of the code base."
            },
            "top_k": {
                "type": "integer",
                "minimum": 1,
                "description": "Optional suggestion for number of results to return."
            }
        },
        "required": ["hint"],
        "additionalProperties": false
    });
}
impl Tool for RequstCodeContext {
    const NAME: &'static str = "apply_code_edit";
    const DESCRIPTION: &'static str = "Apply a diff-style edit to a source file.";

    type Params = RequestCodeContextInput;
    type Output = RequestCodeContextOutput;

    async fn run(self, p: Self::Params) -> Result<Self::Output, ploke_error::Error> { 
        let query = &p.hint;
        let top_k = p.top_k.unwrap_or(self.top_k);
        let budget = self.budget;
        let strategy = self.strategy;
        let assembed_context = self.rag.get_context(query, top_k as usize, &budget, &strategy).await?;

        let stats = assembed_context.stats;
        let parts = assembed_context.parts;
        let (snippets, metadata): (Vec<CodeSnippet>, Vec<SnippetMeta>) = parts.into_iter().map(|cp| {
            let meta = SnippetMeta {id: cp.id, kind: cp.kind, score: cp.score, modality: cp.modality};
            let snippet = CodeSnippet {
                file_path: cp.file_path, snippet: cp.text
                // , canonical_path
            };
            (snippet, meta)
        }).unzip();
        let all_snippets = RequestCodeContextOutput {
            code: snippets,
            meta: metadata
        };
        Ok(all_snippets)
    }

    fn schema() -> &'static serde_json::Map<String, Value>{
        let x = REQUEST_CODE_CONTEXT_PARAMETERS.as_object();
        match x {
            Some(map) => map,
            None => panic!("Tool schema must be well-formed json object")
        }
    }
}

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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    CodeContextRequested
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq)]
pub enum ToolDescr {
    #[serde(rename = "Request additional code context from the repository up to a token budget.")] 
    CodeContextRequested,
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
    pub parameters: Value, // JSON Schema
}

pub trait ToolParams {
    fn name() -> ToolName;
    fn description() -> ToolDescr;
    fn parameters() -> serde_json::Value;
}


#[tracing::instrument(skip(tool_call_params),
    fields( 
        request_id = %tool_call_params.request_id,
        parent_id = %tool_call_params.parent_id,
        call_id = %tool_call_params.call_id
    )
)]
pub async fn handle_tool_call_requested<'a>(tool_call_params: ToolCallParams<'a>) {
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
        arguments: arguments.clone(),
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
