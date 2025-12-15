use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ToolName {
    RequestCodeContext,
    ApplyCodeEdit,
    CreateFile,
    NsPatch,
    NsRead,
}

impl ToolName {
    pub fn as_str(self) -> &'static str {
        use ToolName::*;
        match self {
            RequestCodeContext => "request_code_context",
            ApplyCodeEdit => "apply_code_edit",
            CreateFile => "create_file",
            NsPatch => "non_semantic_patch",
            NsRead => "read_file",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialOrd, PartialEq, Eq)]
pub struct FunctionMarker;

impl FunctionMarker {
    pub const VALUE: &'static str = "function";
}

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
#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, Eq)]
pub struct ToolDefinition {
    #[serde(rename = "type")]
    pub r#type: FunctionMarker,
    pub function: ToolFunctionDef,
}

#[derive(Serialize, Debug, Clone, Deserialize, PartialEq, Eq)]
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
    pub parameters: serde_json::Value, // JSON Schema
}

 impl From<ToolFunctionDef> for ToolDefinition {
     fn from(val: ToolFunctionDef) -> Self {
         ToolDefinition {
             r#type: FunctionMarker,
             function: val,
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
        rename = "Create a new Rust source file atomically within the workspace, staging for approval."
    )]
    CreateFile,
    #[serde(
        rename = r#"Apply a non-semantic code edit. This tool is most useful in two cases:

1. You need to read/edit non-rust files
    - While this application as a whole is focused on Rust code, the user may ask you to read or even edit non-Rust files.
    - This `non_semantic_patch` tool can be used to patch non-Rust files.

2. The parser that allows for semantic edits fails on the target directory
    - usually because there is an error in the target crate (e.g. a missing closing bracket).
    - In this case, this `non_semantic_patch tool can be used to apply a code edit.
    - DO NOT use this tool on Rust files (*.rs) before trying to use the semantic code edit tool first.
"#
    )]
    NsPatch,
    #[serde(
        rename = "Read workspace files before editing. Supports optional line ranges and truncation limits to keep responses concise."
    )]
    NsRead,
}
