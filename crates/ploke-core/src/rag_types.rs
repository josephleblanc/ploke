use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPart {
    pub id: Uuid,
    pub file_path: NodeFilepath,
    pub canon_path: CanonPath,
    pub ranges: Vec<(usize, usize)>,
    pub kind: ContextPartKind,
    pub text: String,
    pub score: f32,
    pub modality: Modality,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextStats {
    pub total_tokens: usize,
    pub files: usize,
    pub parts: usize,
    pub truncated_parts: usize,
    pub dedup_removed: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AssembledContext {
    pub parts: Vec<ContextPart>,
    pub stats: ContextStats,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ContextPartKind {
    Code,
    Doc,
    Signature,
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum Modality {
    Dense,
    Sparse,
    HybridFused,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestCodeContextArgs {
    pub search_term: String,
    #[serde(default)]
    pub token_budget: Option< u32 >,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RequestCodeContextResult {
    pub ok: bool,
    pub search_term: String,
    pub top_k: usize,
    pub kind: ContextPartKind,
    pub context: Vec<ConciseContext>,
}

pub struct AssembledMeta {
    pub search_term: String,
    pub top_k: usize,
    pub kind: ContextPartKind,
}

impl RequestCodeContextResult {
    pub fn from_assembled(parts: Vec< ContextPart >, m: AssembledMeta) -> Self {
        let context: Vec<ConciseContext> = parts.into_iter().map(ConciseContext::from).collect();
        Self {
            ok: true,
            search_term: m.search_term,
            top_k: m.top_k,
            kind: m.kind,
            context,
        }
    }
}

impl From<ContextPart> for ConciseContext {
    fn from(value: ContextPart) -> Self {
        Self {
            file_path: value.file_path.clone(),
            canon_path: value.canon_path.clone(),
            snippet: value.text,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialOrd, Ord, Hash, PartialEq)]
#[serde(transparent)]
pub struct NodeFilepath(pub String);

impl AsRef<str> for NodeFilepath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl NodeFilepath {
    pub fn new(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq,  PartialOrd, Ord, Hash, PartialEq)]
#[serde(transparent)]
pub struct CanonPath(pub String);

impl AsRef<str> for CanonPath {
    fn as_ref(&self) -> &str {
        &self.0
    }
}

impl CanonPath {
    pub fn new(s: String) -> Self {
        Self(s)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Eq, PartialEq, PartialOrd, Ord)]
pub struct ConciseContext {
    pub file_path: NodeFilepath,
    pub canon_path: CanonPath,
    pub snippet: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetFileMetadataResult {
    pub ok: bool,
    pub file_path: String,
    pub exists: bool,
    pub byte_len: u64,
    pub modified_ms: Option<i64>,
    pub file_hash: String,
    pub tracking_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApplyCodeEditResult {
    pub ok: bool,
    /// Number of edits staged into an EditProposal
    pub staged: usize,
    /// Number of edits applied immediately (0 unless auto-confirm is enabled and synchronous)
    pub applied: usize,
    /// Display-friendly file paths included in this proposal
    pub files: Vec<String>,
    /// Preview mode used for the summary ("diff" or "codeblock")
    pub preview_mode: String,
    /// Whether auto-confirm is enabled in config (application may proceed asynchronously)
    pub auto_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateFileResult {
    pub ok: bool,
    /// Number of creations staged
    pub staged: usize,
    /// Number of creations applied immediately (0 unless auto-confirm is enabled and synchronous)
    pub applied: usize,
    /// Display-friendly file paths included in this proposal
    pub files: Vec<String>,
    /// Preview mode used for the summary ("diff" or "codeblock")
    pub preview_mode: String,
    /// Whether auto-confirm is enabled in config (application may proceed asynchronously)
    pub auto_confirmed: bool,
}
