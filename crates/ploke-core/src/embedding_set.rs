use serde::{Deserialize, Serialize};

use crate::{ArcStr, EmbeddingModelId, EmbeddingProviderSlug, EmbeddingShape};

/// Strongly-typed identifier for an embedding set.
///
/// An embedding set represents a specific provider+model+shape combination
/// (e.g., \"openai\" / \"text-embedding-ada-002\" / `<F32; 1536>`). This type
/// is intended to align with the runtime-owned multi-embedding schema and
/// will eventually correspond to rows in the `embedding_sets` relation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingSetId {
    pub provider: EmbeddingProviderSlug,
    pub model: EmbeddingModelId,
    pub shape: EmbeddingShape,
    /// The name created by {model}_{dims}, used as the relation name in the database for the
    /// vector embeddings generated from this embedding model.
    pub rel_name: EmbRelName
}

#[repr(transparent)]
#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Serialize, Deserialize, Debug)]
pub struct EmbRelName(ArcStr);

impl AsRef<str> for EmbRelName {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

impl EmbRelName {
    pub fn new_from_str(model_id: &str) -> Self {
        Self(ArcStr::from(model_id))
    }

    pub fn new_from_string(model_id: String) -> Self {
        // No extra copy, underlying ArcStr changes ownership of String (handled differently than
        // &str despite same semantics)
        Self(ArcStr::from(model_id))
    }
}

impl std::fmt::Display for EmbRelName {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl EmbeddingSetId {
    /// Convenience constructor from components.
    pub fn new(
        provider: EmbeddingProviderSlug,
        model: EmbeddingModelId,
        shape: EmbeddingShape,
    ) -> Self {
        let dims = shape.dimension;
        let rel_name = EmbRelName::new_from_string(format!("{model}_{dims}"));
        Self {
            provider,
            model,
            shape,
            rel_name
        }
    }

    /// Returns the embedding dimension for this set.
    pub fn dimension(&self) -> u32 {
        self.shape.dimension
    }
}
