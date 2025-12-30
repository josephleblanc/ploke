use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq, Hash)]
pub enum ToolName {
    #[serde(rename = "request_code_context")]
    RequestCodeContext,
    #[serde(rename = "apply_code_edit")]
    ApplyCodeEdit,
    #[serde(rename = "create_file")]
    CreateFile,
    #[serde(rename = "non_semantic_patch", alias = "ns_patch")]
    NsPatch,
    #[serde(rename = "read_file", alias = "ns_read")]
    NsRead,
    #[serde(rename = "code_item_lookup")]
    CodeItemLookup,
    #[serde(rename = "code_item_edges")]
    CodeItemEdges,
    #[serde(rename = "cargo")]
    Cargo,
    #[serde(rename = "list_dir")]
    ListDir,
}

impl ToolName {
    pub const ALL: [ToolName; 9] = [
        ToolName::RequestCodeContext,
        ToolName::ApplyCodeEdit,
        ToolName::CreateFile,
        ToolName::NsPatch,
        ToolName::NsRead,
        ToolName::CodeItemLookup,
        ToolName::CodeItemEdges,
        ToolName::Cargo,
        ToolName::ListDir,
    ];

    pub fn as_str(self) -> &'static str {
        use ToolName::*;
        match self {
            RequestCodeContext => "request_code_context",
            ApplyCodeEdit => "apply_code_edit",
            CreateFile => "create_file",
            NsPatch => "non_semantic_patch",
            NsRead => "read_file",
            CodeItemLookup => "code_item_lookup",
            CodeItemEdges => "code_item_edges",
            Cargo => "cargo",
            ListDir => "list_dir",
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
        rename = "Read focused-crate files before editing. Paths must be absolute or crate-root-relative. Supports optional line ranges and truncation limits to keep responses concise."
    )]
    NsRead,
    #[serde(
        rename = r#"Find the definition of a known code item. Better than grep. 

Returns the code snippet of the item if it exists, and provides positive proof if the item does not exist.

Use this tool when you want to look up a given code item. 

Pro tip: use it with parallel tool calls to look up as many code items as you want.
"#
    )]
    CodeItemLookup,
    #[serde(
        rename = r#"Shows all edges for the target item. Useful for discovering nearby code items."#
    )]
    CodeItemEdges,
    #[serde(rename = "Run cargo check or cargo test with JSON diagnostics output.")]
    Cargo,
    #[serde(
        rename = "List files in a directory (crate-root scoped) without shell access. Returns a structured entry list with names, kinds, and optional size/mtime metadata."
    )]
    ListDir,
}

#[cfg(test)]
mod tests {
    use super::ToolName;

    #[test]
    fn tool_name_serializes_to_canonical_strings() {
        let read = serde_json::to_string(&ToolName::NsRead).expect("serialize");
        let patch = serde_json::to_string(&ToolName::NsPatch).expect("serialize");

        assert_eq!(read, "\"read_file\"");
        assert_eq!(patch, "\"non_semantic_patch\"");
    }

    #[test]
    fn tool_name_accepts_aliases() {
        let read_alias: ToolName = serde_json::from_str("\"ns_read\"").expect("alias");
        let patch_alias: ToolName = serde_json::from_str("\"ns_patch\"").expect("alias");

        assert_eq!(read_alias, ToolName::NsRead);
        assert_eq!(patch_alias, ToolName::NsPatch);
    }
}
