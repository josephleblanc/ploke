use serde::{Deserialize, Serialize};

use crate::tool_descriptions::{
    ToolDescription, ToolDescriptionArtifactRelPath, tool_description,
    tool_description_artifact_relpath,
};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialOrd, PartialEq, Ord, Eq, Hash)]
pub enum ToolName {
    #[serde(rename = "request_code_context")]
    RequestCodeContext,
    #[serde(rename = "apply_code_edit")]
    ApplyCodeEdit,
    #[serde(rename = "insert_rust_item")]
    InsertRustItem,
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
    pub const ALL: [ToolName; 10] = [
        ToolName::RequestCodeContext,
        ToolName::ApplyCodeEdit,
        ToolName::InsertRustItem,
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
            InsertRustItem => "insert_rust_item",
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
    pub description: String,
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

impl ToolName {
    pub fn description(self) -> ToolDescription {
        tool_description(self)
    }

    pub fn description_artifact_relpath(self) -> ToolDescriptionArtifactRelPath {
        tool_description_artifact_relpath(self)
    }
}

#[cfg(test)]
mod tests {
    use super::{ToolFunctionDef, ToolName};
    use crate::tool_descriptions::tool_description;

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

    #[test]
    fn apply_code_edit_description_points_to_method_lookup() {
        let description = tool_description(ToolName::ApplyCodeEdit).to_lowercase();
        assert!(description.contains("node_type=method"));
        assert!(description.contains("semantic targets"));
        assert!(description.contains("code_item_lookup"));
        assert!(description.contains("non_semantic_patch"));
    }

    #[test]
    fn non_semantic_patch_description_warns_on_rust_files() {
        let description = tool_description(ToolName::NsPatch).to_lowercase();
        assert!(description.contains("non-rust files"));
        assert!(description.contains("rust files"));
        assert!(description.contains("semantic code edit tool"));
    }

    #[test]
    fn insert_rust_item_tool_name_serializes() {
        let name = serde_json::to_string(&ToolName::InsertRustItem).expect("serialize");
        assert_eq!(name, "\"insert_rust_item\"");
    }

    #[test]
    fn tool_function_def_serializes_description_as_string() {
        let function = ToolFunctionDef {
            name: ToolName::ApplyCodeEdit,
            description: tool_description(ToolName::ApplyCodeEdit).to_string(),
            parameters: serde_json::json!({"type": "object"}),
        };

        let value = serde_json::to_value(function).expect("serialize");
        assert_eq!(
            value.get("description").and_then(|v| v.as_str()),
            Some(tool_description(ToolName::ApplyCodeEdit))
        );
    }
}
