use serde::{Deserialize, Serialize};

use crate::{EmbeddingModelId, EmbeddingProviderSlug, EmbeddingShape};

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
}

impl EmbeddingSetId {
    /// Convenience constructor from components.
    pub fn new(
        provider: EmbeddingProviderSlug,
        model: EmbeddingModelId,
        shape: EmbeddingShape,
    ) -> Self {
        Self {
            provider,
            model,
            shape,
        }
    }

    /// Returns the embedding dimension for this set.
    pub fn dimension(&self) -> u32 {
        self.shape.dimension
    }
}
