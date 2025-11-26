use std::collections::BTreeMap;

use cozo::DataValue;
use itertools::Itertools as _;
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

impl EmbeddingVector {
    pub fn script_create_from_set(embedding_set: &EmbeddingSet) -> String {
        format!(
            ":create {rel_name} {{
    node_id: Uuid,
    at: Validity,
    =>
    vector: <F32; {dims}>,
    embedding_set_id: Int
}}",
            rel_name = embedding_set.rel_name,
            dims = embedding_set.dims()
        )
    }
}

impl EmbeddingSetExt for EmbeddingSet {
    fn provider(&self) -> EmbeddingProviderSlug {
        // TODO: check if this is Arc::clone
        self.provider.clone()
    }

    fn model(&self) -> EmbeddingModelId {
        // TODO: check if this is Arc::clone
        self.model.clone()
    }

    fn shape(&self) -> EmbeddingShape {
        self.shape
    }

    fn hash_id(&self) -> EmbeddingSetId {
        self.hash_id
    }

    fn rel_name(&self) -> EmbRelName {
        // TODO: check if this is Arc::clone
        self.rel_name.clone()
    }
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

pub trait CozoEmbeddingSetExt: EmbeddingSetExt {
    fn script_create() -> &'static str {
        ":create embedding_set { 
id: Int,
at: Validity,
=>
provider: String, 
model: String, 
dims: Int,
embedding_dtype: String
rel_name: String
}"
    }

    fn script_put(&self) -> String {
        format!(
            ":put embedding_set {{
id: {hash_id},
at: 'ASSERT',
provider: {provider}, 
model: {model}, 
dims: {shape_dims},
embedding_dtype: {shape_embedding_dtype},
rel_name: {rel_name}
}}",
            hash_id = self.hash_id(),
            provider = self.provider(),
            model = self.model(),
            shape_dims = self.shape().dimension,
            shape_embedding_dtype = self.shape().dtype_tag(),
            rel_name = self.rel_name()
        )
    }

    fn script_put_vector_with_param(&self) -> String {
        let script = format!(
            r#"
?[node_id, at, vector] <- [[
    $node_id,
    'ASSERT',
    $vector
]] :put {vector_identity}
"#,
            vector_identity = self.script_vector_identity()
        );
        script
    }

    fn param_put_vector(
        &self,
        vector: Vec<f64>,
        node_id: Uuid,
    ) -> BTreeMap<String, cozo::DataValue> {
        let to_cozo_uuid = |u: Uuid| -> DataValue { DataValue::Uuid(cozo::UuidWrapper(u)) };
        let vector_datavalue = vector.iter().map(|v| DataValue::from(*v)).collect_vec();

        BTreeMap::from([
            (
                "id".to_string(),
                DataValue::from(self.hash_id().into_inner() as i64),
            ),
            ("node_id".to_string(), to_cozo_uuid(node_id)),
            ("vector".to_string(), DataValue::List(vector_datavalue)),
        ])
    }

    fn script_vector_identity(&self) -> String {
        format!("{} {{ node_id, at => vector }}", self.rel_name())
    }
}
