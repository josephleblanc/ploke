use serde::{Deserialize, Serialize};

use crate::ArcStr;

/// Strongly-typed identifier for an embedding model.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Debug, Deserialize, Serialize)]
pub struct EmbeddingModelId(ArcStr);

impl AsRef<str> for EmbeddingModelId {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl EmbeddingModelId {
    pub fn new_from_str(model_id: &str) -> Self {
        Self(ArcStr::from(model_id))
    }
}

impl std::fmt::Display for EmbeddingModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Strongly-typed identifier for an embedding provider (e.g., "openai", "local-transformers").
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Debug, Deserialize, Serialize)]
pub struct EmbeddingProviderSlug(ArcStr);

impl AsRef<str> for EmbeddingProviderSlug {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl EmbeddingProviderSlug {
    pub fn new_from_str(model_id: &str) -> Self {
        Self(ArcStr::from(model_id))
    }
}

impl std::fmt::Display for EmbeddingModelId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

// AI: Add a test for the Display implementation, to verify that we are or are not printing with quotes
// around the text in EmbeddingModelId AI!

/// Data type for elements in an embedding vector.
///
/// Cozo only allows for vectors to be F32 or F64
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingDType {
    F32,
    F64,
}

/// Encoding format for serialized embeddings.
///
/// For now we only use raw floating-point vectors; additional formats can be added later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingEncoding {
    /// Raw dense vector representation (e.g., `<F32; dims>` columns in Cozo).
    RawVector,
    /// Byte-encoded form (e.g., for transport or compressed storage).
    Bytes,
    /// Base64-encoded representation.
    Base64,
}

/// Describes the shape of an embedding vector: its dimension, data type, and encoding.
///
/// Planning references:
/// - `EmbeddingShape` in `required-groundwork.md`
/// - Remote embedding trait/system design reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingShape {
    pub dimension: u32,
    pub dtype: EmbeddingDType,
    pub encoding: EmbeddingEncoding,
}

impl EmbeddingShape {
    /// Convenience constructor.
    pub const fn new(dimension: u32, dtype: EmbeddingDType, encoding: EmbeddingEncoding) -> Self {
        Self {
            dimension,
            dtype,
            encoding,
        }
    }

    /// Shape for an in-memory `<F32; dims>` vector, matching current Cozo relations.
    pub const fn f32_raw(dimension: u32) -> Self {
        Self {
            dimension,
            dtype: EmbeddingDType::F32,
            encoding: EmbeddingEncoding::RawVector,
        }
    }
}
