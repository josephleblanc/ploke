use serde::{Deserialize, Serialize};

use crate::{ArcStr, EmbeddingModelId, EmbeddingProviderSlug, EmbeddingShape};

/// Strongly-typed identifier for an embedding set.
///
/// An embedding set represents a specific provider+model+shape combination
/// (e.g., \"openai\" / \"text-embedding-ada-002\" / `<F32; 1536>`). This type
/// is intended to align with the runtime-owned multi-embedding schema and
/// will eventually correspond to rows in the `embedding_sets` relation.
///
/// The `embedding_sets` relation will be the database-stored basic info on an embedding model that
/// is used to create some set of vector embeddings, and will exist alongside `HnswEmbedInfo`,
/// which will also have the `rel_name` to reference the `EmbeddingSetId`.
///
/// We want to have an EmbeddingSetId, separate from the data on hnsw indices in `HnswEmbedInfo`,
/// to track the embedding model and provider themselves, which may map to multiple `HnswEmbedInfo`
/// (e.g. different hnsw search settings, same underlying embedding model).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingSetId {
    pub provider: EmbeddingProviderSlug,
    pub model: EmbeddingModelId,
    pub shape: EmbeddingShape,
    /// The name created by {model}_{dims}, used as the relation name in the database for the
    /// vector embeddings generated from this embedding model, which will be a relation with only
    /// the vector and the related node_id it points towards.
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
        let rel_name = EmbRelName::new_from_string(format!("emb_{model}_{dims}"));
        Self {
            provider,
            model,
            shape,
            rel_name
        }
    }

    pub fn dims(&self) -> u32 {
        self.shape.dimension
    }

    pub fn relation_name(&self) -> &EmbRelName {
        // NOTE: I'm not sure if we need this sanitization or not.
        // For now, going to comment it out, and just use the string itself, then bring back the
        // sanitization if we run into issues with tests trying to insert rows into cozo.
        // let model = sanitize_relation_component(self.embedding_model.as_ref());
        // format!("emb_{}_{}", model, self.dims())
        &self.rel_name
    }

    pub fn script_identity(&self) -> String {
        format!(
            "{} {{ node_id, embedding_model, provider, at => embedding_dims, vector }}",
            self.relation_name()
        )
    }

    pub fn script_create(&self) -> String {
        format!(
            ":create {} {{ node_id: Uuid, embedding_model: String, provider: String, at: Validity => embedding_dims: Int, vector: <F32; {}> }}",
            self.relation_name(),
            self.dims()
        )
    }

}
