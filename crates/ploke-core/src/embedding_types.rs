use serde::{Deserialize, Serialize};

use crate::{arcstr_wrapper, ArcStr};

/// Typed wrapper for an embedding model.
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Debug, Deserialize, Serialize)]
pub struct EmbeddingModelId(ArcStr);

arcstr_wrapper!(EmbeddingModelId);

/// Typed wrapper for an embedding provider (e.g., "openai", "local-transformers").
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Debug, Deserialize, Serialize)]
pub struct EmbeddingProviderSlug(ArcStr);

arcstr_wrapper!(EmbeddingProviderSlug);

/// Data type for elements in an embedding vector.
///
/// Cozo only allows for vectors to be F32 or F64
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingDType {
    #[default]
    F32,
    F64,
}

/// Encoding format for serialized embeddings.
///
/// For now we only use raw floating-point vectors; additional formats can be added later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum EmbeddingEncoding {
    /// Raw dense vector representation (e.g., `<F32; dims>` columns in Cozo).
    #[default]
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

    /// Convenience constructor to create new dimension vec with defaults (containing
    /// Cozo-compatable values) otherwise.
    pub fn new_dims_default(dimension: u32) -> Self {
        Self {
            dimension,
            dtype: Default::default(),
            encoding: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_embedding_model_id_display() {
        let model_id = EmbeddingModelId::new_from_str("text-embedding-3-small");
        let display = format!("{}", model_id);
        // Should not have quotes around the text
        assert_ne!(display, r#""text-embedding-3-small""#);
        assert_eq!(display, "text-embedding-3-small");
    }

    #[test]
    fn test_embedding_provider_display() {
        let provider = EmbeddingProviderSlug::new_from_str("openai");
        let display = format!("{}", provider);
        // Should not have quotes around the text
        assert_ne!(display, r#""openai""#);
        assert_eq!(display, "openai");
    }
}
