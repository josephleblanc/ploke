use serde::{Deserialize, Serialize};

use crate::{arcstr_wrapper, ArcStr};

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
    pub rel_name: EmbRelName,
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
            rel_name,
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

/// Describes the shape of an embedding vector: its dimension, data type, and encoding.
///
/// Planning references:
/// - `EmbeddingShape` in `required-groundwork.md`
/// - Remote embedding trait/system design reports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EmbeddingShape {
    pub dimension: u32,
    pub dtype: EmbeddingDType,
}

impl EmbeddingShape {
    /// Convenience constructor.
    pub const fn new(dimension: u32, dtype: EmbeddingDType) -> Self {
        Self { dimension, dtype }
    }

    /// Shape for an in-memory `<F32; dims>` vector, matching current Cozo relations.
    pub const fn f32_raw(dimension: u32) -> Self {
        Self {
            dimension,
            dtype: EmbeddingDType::F32,
        }
    }

    /// Convenience constructor to create new dimension vec with defaults (containing
    /// Cozo-compatable values) otherwise.
    pub fn new_dims_default(dimension: u32) -> Self {
        Self {
            dimension,
            dtype: Default::default(),
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
