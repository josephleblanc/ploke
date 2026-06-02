use serde::{Deserialize, Serialize};

use crate::tool_types::ToolName;

use super::dto::{
    ApplyCodeEditResult, ApplyNsPatchResult, CargoToolParamsOwned, CargoToolResult,
    CodeEditParamsOwned, ConciseContext, CreateFileParamsOwned, CreateFileResult, EdgesParamsOwned,
    InsertRustItemParamsOwned, LegacySearchArguments, ListDirParamsOwned, ListDirResult,
    LookupParamsOwned, NsPatchParamsOwned, NsReadParamsOwned, NsReadResult,
    RequestCodeContextParamsOwned, RequestCodeContextResult, ToolArgumentParseFailure,
    ToolResultParseFailure,
};

/// Closed enum of currently owned tool argument DTOs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "arguments")]
pub enum ToolCallArguments {
    #[serde(rename = "request_code_context")]
    RequestCodeContext(RequestCodeContextParamsOwned),
    #[serde(rename = "apply_code_edit")]
    ApplyCodeEdit(CodeEditParamsOwned),
    #[serde(rename = "insert_rust_item")]
    InsertRustItem(InsertRustItemParamsOwned),
    #[serde(rename = "create_file")]
    CreateFile(CreateFileParamsOwned),
    #[serde(rename = "non_semantic_patch")]
    NsPatch(NsPatchParamsOwned),
    #[serde(rename = "read_file")]
    NsRead(NsReadParamsOwned),
    #[serde(rename = "code_item_lookup")]
    CodeItemLookup(LookupParamsOwned),
    #[serde(rename = "code_item_edges")]
    CodeItemEdges(EdgesParamsOwned),
    #[serde(rename = "cargo")]
    Cargo(CargoToolParamsOwned),
    #[serde(rename = "list_dir")]
    ListDir(ListDirParamsOwned),
    #[serde(rename = "search_code")]
    SearchCode(LegacySearchArguments),
    #[serde(rename = "search_symbols")]
    SearchSymbols(LegacySearchArguments),
    #[serde(rename = "query_codebase")]
    QueryCodebase(LegacySearchArguments),
}

impl ToolCallArguments {
    pub fn tool_name(&self) -> Option<ToolName> {
        match self {
            Self::RequestCodeContext(_) => Some(ToolName::RequestCodeContext),
            Self::ApplyCodeEdit(_) => Some(ToolName::ApplyCodeEdit),
            Self::InsertRustItem(_) => Some(ToolName::InsertRustItem),
            Self::CreateFile(_) => Some(ToolName::CreateFile),
            Self::NsPatch(_) => Some(ToolName::NsPatch),
            Self::NsRead(_) => Some(ToolName::NsRead),
            Self::CodeItemLookup(_) => Some(ToolName::CodeItemLookup),
            Self::CodeItemEdges(_) => Some(ToolName::CodeItemEdges),
            Self::Cargo(_) => Some(ToolName::Cargo),
            Self::ListDir(_) => Some(ToolName::ListDir),
            Self::SearchCode(_) | Self::SearchSymbols(_) | Self::QueryCodebase(_) => None,
        }
    }
}

/// Closed enum of currently owned tool result DTOs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "tool", content = "result")]
pub enum ToolResultContent {
    #[serde(rename = "request_code_context")]
    RequestCodeContext(RequestCodeContextResult),
    #[serde(rename = "apply_code_edit")]
    ApplyCodeEdit(ApplyCodeEditResult),
    #[serde(rename = "insert_rust_item")]
    InsertRustItem(ApplyCodeEditResult),
    #[serde(rename = "create_file")]
    CreateFile(CreateFileResult),
    #[serde(rename = "non_semantic_patch")]
    NsPatch(ApplyNsPatchResult),
    #[serde(rename = "read_file")]
    NsRead(NsReadResult),
    #[serde(rename = "code_item_lookup")]
    CodeItemLookup(ConciseContext),
    #[serde(rename = "cargo")]
    Cargo(CargoToolResult),
    #[serde(rename = "list_dir")]
    ListDir(ListDirResult),
}

impl ToolResultContent {
    pub fn tool_name(&self) -> ToolName {
        match self {
            Self::RequestCodeContext(_) => ToolName::RequestCodeContext,
            Self::ApplyCodeEdit(_) => ToolName::ApplyCodeEdit,
            Self::InsertRustItem(_) => ToolName::InsertRustItem,
            Self::CreateFile(_) => ToolName::CreateFile,
            Self::NsPatch(_) => ToolName::NsPatch,
            Self::NsRead(_) => ToolName::NsRead,
            Self::CodeItemLookup(_) => ToolName::CodeItemLookup,
            Self::Cargo(_) => ToolName::Cargo,
            Self::ListDir(_) => ToolName::ListDir,
        }
    }
}

/// Typed persisted view of a tool-call argument payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", content = "record", rename_all = "snake_case")]
pub enum PersistedToolCallArguments {
    Decoded(ToolCallArguments),
    ParseFailure(ToolArgumentParseFailure),
}

impl PersistedToolCallArguments {
    pub fn decoded(&self) -> Option<&ToolCallArguments> {
        match self {
            Self::Decoded(arguments) => Some(arguments),
            Self::ParseFailure(_) => None,
        }
    }

    pub fn parse_failure(&self) -> Option<&ToolArgumentParseFailure> {
        match self {
            Self::Decoded(_) => None,
            Self::ParseFailure(failure) => Some(failure),
        }
    }
}

/// Typed persisted view of a tool result `content` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", content = "record", rename_all = "snake_case")]
pub enum PersistedToolResultContent {
    Decoded(ToolResultContent),
    ParseFailure(ToolResultParseFailure),
}

impl PersistedToolResultContent {
    pub fn decoded(&self) -> Option<&ToolResultContent> {
        match self {
            Self::Decoded(result) => Some(result),
            Self::ParseFailure(_) => None,
        }
    }

    pub fn parse_failure(&self) -> Option<&ToolResultParseFailure> {
        match self {
            Self::Decoded(_) => None,
            Self::ParseFailure(failure) => Some(failure),
        }
    }
}
