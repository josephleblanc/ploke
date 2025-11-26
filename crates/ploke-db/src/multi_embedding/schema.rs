use ploke_core::embeddings::{
    EmbRelName, EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingSetId,
    EmbeddingShape,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::DbError;

/// An embedding vector stored as a single row in the cozo database, containing minimal information
/// of the:
/// - node_id: The code item that was processed into the vector, using the start/end bytes of the
///   location in the underlying code base.
/// - vector: A vector of a length (unknown at compile time), processed by a vector embedding model.
/// - embedding_set_id: A hash of the contents of the embedding set used to create the vector. This
///   is essentially a pointer to another item in the database that contains the data on the model,
///   provider, etc used to create this vector.
#[derive(Clone, PartialEq, PartialOrd, Serialize, Deserialize, Debug)]
pub struct EmbeddingVector {
    /// The id of the node used to create this vector.
    pub node_id: Uuid,
    /// Vector embedding of length (unknown at compile time)
    pub vector: Vec<f64>,
    /// Hashed ID pointing to the embedding set that was used to create this vector.
    pub embedding_set_id: EmbeddingSetId,
}

pub trait EmbeddingSetExt {
    fn provider(&self) -> EmbeddingProviderSlug;
    fn model(&self) -> EmbeddingModelId;
    fn shape(&self) -> EmbeddingShape;
    fn hash_id(&self) -> EmbeddingSetId;
    fn rel_name(&self) -> EmbRelName;

    fn create_vector_with_node(&self, node_id: Uuid, vector: Vec<f64>) -> EmbeddingVector {
        EmbeddingVector {
            node_id,
            vector,
            embedding_set_id: self.hash_id(),
        }
    }
}
