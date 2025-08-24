use serde::{Deserialize, Serialize};
use uuid::Uuid;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPart {
    pub id: Uuid,
    pub file_path: String,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextPartKind {
    #[serde(rename = "code")]
    Code,
    #[serde(rename = "doc")]
    Doc,
    #[serde(rename = "signature")]
    Signature,
    #[serde(rename = "metadata")]
    Metadata,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Modality {
    #[serde(rename = "dense")]
    Dense,
    #[serde(rename = "sparse")]
    Sparse,
    #[serde(rename = "hybrid_fused")]
    HybridFused,
}
