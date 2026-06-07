//! Owned tool parameter and result DTOs.

use serde::{Deserialize, Serialize};

use crate::file_hash::FileHash;
pub use crate::rag_types::{
    ApplyCodeEditResult, ConciseContext, CreateFileResult, RequestCodeContextResult,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct RequestCodeContextParamsOwned {
    #[serde(default, alias = "token_budget")]
    pub token_budget_per_result: Option<u32>,
    #[serde(default)]
    pub token_budget_total: Option<u32>,
    pub search_term: Option<String>,
}

pub use crate::NodeType;

/// Graph node type label carried on semantic code-edit tool arguments.
///
/// Wire-compatible with `ploke_db::NodeType` without pulling database crates into wasm builds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphNodeType {
    Function,
    Struct,
    Enum,
    Trait,
    Module,
    Const,
    Impl,
    Import,
    Macro,
    Static,
    TypeAlias,
    Union,
    Method,
    Param,
    Variant,
    Field,
    Attribute,
    GenericType,
    GenericLifetime,
    GenericConst,
    NamedType,
    ReferenceType,
    SliceType,
    ArrayType,
    TupleType,
    FunctionType,
    NeverType,
    InferredType,
    RawPointerType,
    TraitObjectType,
    ImplTraitType,
    ParenType,
    MacroType,
    UnknownType,
    SyntaxEdge,
}

impl From<crate::NodeType> for GraphNodeType {
    fn from(value: crate::NodeType) -> Self {
        match value {
            crate::NodeType::Function => Self::Function,
            crate::NodeType::Struct => Self::Struct,
            crate::NodeType::Enum => Self::Enum,
            crate::NodeType::Trait => Self::Trait,
            crate::NodeType::Module => Self::Module,
            crate::NodeType::Const => Self::Const,
            crate::NodeType::Impl => Self::Impl,
            crate::NodeType::Import => Self::Import,
            crate::NodeType::Macro => Self::Macro,
            crate::NodeType::Static => Self::Static,
            crate::NodeType::TypeAlias => Self::TypeAlias,
            crate::NodeType::Union => Self::Union,
            crate::NodeType::Method => Self::Method,
            crate::NodeType::Param => Self::Param,
            crate::NodeType::Variant => Self::Variant,
            crate::NodeType::Field => Self::Field,
            crate::NodeType::Attribute => Self::Attribute,
            crate::NodeType::GenericType => Self::GenericType,
            crate::NodeType::GenericLifetime => Self::GenericLifetime,
            crate::NodeType::GenericConst => Self::GenericConst,
            crate::NodeType::NamedType => Self::NamedType,
            crate::NodeType::ReferenceType => Self::ReferenceType,
            crate::NodeType::SliceType => Self::SliceType,
            crate::NodeType::ArrayType => Self::ArrayType,
            crate::NodeType::TupleType => Self::TupleType,
            crate::NodeType::FunctionType => Self::FunctionType,
            crate::NodeType::NeverType => Self::NeverType,
            crate::NodeType::InferredType => Self::InferredType,
            crate::NodeType::RawPointerType => Self::RawPointerType,
            crate::NodeType::TraitObjectType => Self::TraitObjectType,
            crate::NodeType::ImplTraitType => Self::ImplTraitType,
            crate::NodeType::ParenType => Self::ParenType,
            crate::NodeType::MacroType => Self::MacroType,
            crate::NodeType::UnknownType => Self::UnknownType,
            crate::NodeType::SyntaxEdge => Self::SyntaxEdge,
        }
    }
}

impl From<GraphNodeType> for crate::NodeType {
    fn from(value: GraphNodeType) -> Self {
        match value {
            GraphNodeType::Function => Self::Function,
            GraphNodeType::Struct => Self::Struct,
            GraphNodeType::Enum => Self::Enum,
            GraphNodeType::Trait => Self::Trait,
            GraphNodeType::Module => Self::Module,
            GraphNodeType::Const => Self::Const,
            GraphNodeType::Impl => Self::Impl,
            GraphNodeType::Import => Self::Import,
            GraphNodeType::Macro => Self::Macro,
            GraphNodeType::Static => Self::Static,
            GraphNodeType::TypeAlias => Self::TypeAlias,
            GraphNodeType::Union => Self::Union,
            GraphNodeType::Method => Self::Method,
            GraphNodeType::Param => Self::Param,
            GraphNodeType::Variant => Self::Variant,
            GraphNodeType::Field => Self::Field,
            GraphNodeType::Attribute => Self::Attribute,
            GraphNodeType::GenericType => Self::GenericType,
            GraphNodeType::GenericLifetime => Self::GenericLifetime,
            GraphNodeType::GenericConst => Self::GenericConst,
            GraphNodeType::NamedType => Self::NamedType,
            GraphNodeType::ReferenceType => Self::ReferenceType,
            GraphNodeType::SliceType => Self::SliceType,
            GraphNodeType::ArrayType => Self::ArrayType,
            GraphNodeType::TupleType => Self::TupleType,
            GraphNodeType::FunctionType => Self::FunctionType,
            GraphNodeType::NeverType => Self::NeverType,
            GraphNodeType::InferredType => Self::InferredType,
            GraphNodeType::RawPointerType => Self::RawPointerType,
            GraphNodeType::TraitObjectType => Self::TraitObjectType,
            GraphNodeType::ImplTraitType => Self::ImplTraitType,
            GraphNodeType::ParenType => Self::ParenType,
            GraphNodeType::MacroType => Self::MacroType,
            GraphNodeType::UnknownType => Self::UnknownType,
            GraphNodeType::SyntaxEdge => Self::SyntaxEdge,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CodeEditParamsOwned {
    pub edits: Vec<CanonicalEditOwned>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CanonicalEditOwned {
    pub file: String,
    pub canon: String,
    pub node_type: GraphNodeType,
    pub code: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InsertRustContainerKind {
    File,
    Module,
    Trait,
    Impl,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ToolItemKind {
    Function,
    Method,
    Const,
    Enum,
    Impl,
    Import,
    Macro,
    Module,
    Static,
    Struct,
    Trait,
    TypeAlias,
    Union,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InsertRustItemParamsOwned {
    pub file: String,
    pub container_kind: InsertRustContainerKind,
    pub container_canon: Option<String>,
    pub item_kind: ToolItemKind,
    pub code: String,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFileParamsOwned {
    pub file_path: String,
    pub content: String,
    pub on_exists: Option<String>,
    pub create_parents: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NsPatchParamsOwned {
    pub patches: Vec<NsPatchOwned>,
    pub confidence: Option<f32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NsPatchOwned {
    pub file: String,
    pub diff: String,
    pub reasoning: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyNsPatchResult {
    pub ok: bool,
    pub staged: usize,
    pub applied: usize,
    pub files: Vec<String>,
    pub preview_mode: String,
    pub auto_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NsReadParamsOwned {
    pub file: String,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub max_bytes: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NsReadResult {
    pub ok: bool,
    pub file_path: String,
    pub exists: bool,
    pub byte_len: Option<u64>,
    pub start_line: Option<u32>,
    pub end_line: Option<u32>,
    pub truncated: bool,
    pub content: Option<String>,
    pub file_hash: Option<FileHash>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LookupParamsOwned {
    pub item_name: String,
    pub file_path: String,
    pub node_kind: String,
    pub module_path: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EdgesParamsOwned {
    pub item_name: String,
    pub file_path: String,
    pub node_kind: String,
    pub module_path: String,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CargoCommand {
    Test,
    Check,
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CargoScope {
    Focused,
    Workspace,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoToolParamsOwned {
    pub command: CargoCommand,
    pub scope: CargoScope,
    pub package: Option<String>,
    pub features: Option<Vec<String>>,
    pub all_features: bool,
    pub no_default_features: bool,
    pub target: Option<String>,
    pub profile: Option<String>,
    pub release: bool,
    pub lib: bool,
    pub tests: bool,
    pub bins: bool,
    pub examples: bool,
    pub benches: bool,
    pub test_args: Option<Vec<String>>,
    #[serde(default)]
    pub include_warnings: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoToolResult {
    pub ok: bool,
    pub status_reason: CargoStatusReason,
    pub command: CargoCommand,
    pub scope: CargoScope,
    pub manifest_path: String,
    pub exit_code: Option<i32>,
    pub duration_ms: u64,
    pub summary: CargoSummary,
    pub diagnostics: Vec<CargoDiagnostic>,
    pub stderr_tail: Vec<String>,
    pub non_json_stdout_tail: Vec<String>,
    pub json_parse_errors_tail: Vec<String>,
    pub raw_messages_truncated: bool,
}

#[derive(Debug, Clone, Serialize, Default, Deserialize)]
pub struct CargoSummary {
    pub errors: u32,
    pub warnings: u32,
    pub notes: u32,
    pub artifacts: u32,
    pub other_messages: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoDiagnostic {
    pub level: String,
    pub message: String,
    pub code: Option<String>,
    pub spans: Vec<CargoSpan>,
    pub rendered: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CargoSpan {
    pub file_name: String,
    pub line_start: u32,
    pub line_end: u32,
    pub column_start: u32,
    pub column_end: u32,
    pub is_primary: bool,
}

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CargoStatusReason {
    Success,
    CompileFailed,
    TestsFailedOrRuntime,
    CargoFailedOrInvalidArgs,
    Timeout,
    Canceled,
    Killed,
}

impl CargoStatusReason {
    pub fn as_str(self) -> &'static str {
        match self {
            CargoStatusReason::Success => "success",
            CargoStatusReason::CompileFailed => "compile_failed",
            CargoStatusReason::TestsFailedOrRuntime => "tests_failed_or_runtime",
            CargoStatusReason::CargoFailedOrInvalidArgs => "cargo_failed_or_invalid_args",
            CargoStatusReason::Timeout => "timeout",
            CargoStatusReason::Canceled => "canceled",
            CargoStatusReason::Killed => "killed",
        }
    }
}

impl CargoCommand {
    pub fn as_str(self) -> &'static str {
        match self {
            CargoCommand::Test => "test",
            CargoCommand::Check => "check",
        }
    }
}

impl CargoScope {
    pub fn as_str(self) -> &'static str {
        match self {
            CargoScope::Focused => "focused",
            CargoScope::Workspace => "workspace",
        }
    }
}
impl Default for CargoScope {
    fn default() -> Self {
        CargoScope::Focused
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDirParamsOwned {
    pub dir: String,
    pub include_hidden: bool,
    pub sort: Option<String>,
    pub max_entries: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDirEntry {
    pub name: String,
    pub path: String,
    pub kind: String,
    pub size_bytes: Option<u64>,
    pub modified_ms: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListDirResult {
    pub ok: bool,
    pub dir: String,
    pub exists: bool,
    pub truncated: bool,
    pub entries: Vec<ListDirEntry>,
}

/// Historical search tool argument shape kept for replay/protocol context.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LegacySearchArguments {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_term: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub query: Option<String>,
}

/// Typed record emitted when a persisted/provider argument string cannot be decoded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolArgumentParseFailure {
    pub tool: String,
    pub raw_arguments: String,
    pub error: ToolArgumentDecodeError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolArgumentDecodeError {
    UnknownTool { tool: String },
    InvalidJson { message: String },
}

/// Typed record emitted when a persisted tool result string cannot be decoded.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ToolResultParseFailure {
    pub tool: String,
    pub raw_content: String,
    pub error: ToolResultDecodeError,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum ToolResultDecodeError {
    UnknownTool { tool: String },
    UnsupportedToolResult { tool: String },
    InvalidJson { message: String },
}
