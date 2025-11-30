use std::collections::BTreeMap;

use cozo::{DataValue, UuidWrapper};
use itertools::Itertools as _;
use ploke_core::embeddings::{
    EmbRelName, EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingSetId,
    EmbeddingShape, HnswRelName,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{database::HNSW_SUFFIX, DbError};

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

macro_rules! cozo_script_embedding_set {
    ($db_op: literal, $rel:literal) => {
        concat!(
            $db_op,
            " ",
            $rel,
            " {\n",
            "id: String,\n",
            "at: Validity\n",
            "=>\n",
            "provider: String,\n",
            "model: String,\n",
            "dims: Int,\n",
            "embedding_dtype: String,\n",
            "rel_name: String\n",
            "}"
        )
    };
}

impl CozoEmbeddingSetExt for EmbeddingSet {}

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

    pub fn script_fields() -> &'static str {
        "node_id, at, vector, embedding_set_id"
    }

    /// Validate that an embedding vector is non-empty
    pub fn validate_embedding_vec(&self) -> Result<(), DbError> {
        if self.vector.is_empty() {
            Err(DbError::QueryExecution(
                "Embedding vector must not be empty".into(),
            ))
        } else {
            Ok(())
        }
    }

    /// Convert to DataValue format - as a list of [node_id, embedding_set_id, embedding]
    pub fn into_cozo_datavalue(self) -> cozo::DataValue {
        let id_val = DataValue::Uuid(UuidWrapper(self.node_id));
        let embedding_val = DataValue::List(
            self.vector
                .into_iter()
                .map(|f| DataValue::Num(cozo::Num::Float(f)))
                .collect(),
        );
        let embedding_set_id =
            DataValue::Num(cozo::Num::Int(self.embedding_set_id.into_inner() as i64));
        // Each update is a list containing [node_id, embedding_set_id, embedding]
        DataValue::List(vec![id_val, embedding_set_id, embedding_val])
    }
}

impl EmbeddingSetExt for EmbeddingSet {
    type Vector = EmbeddingVector;
    fn provider(&self) -> &EmbeddingProviderSlug {
        // TODO: check if this is Arc::clone
        &self.provider
    }

    fn model(&self) -> &EmbeddingModelId {
        // TODO: check if this is Arc::clone
        &self.model
    }

    fn shape(&self) -> EmbeddingShape {
        self.shape
    }

    fn hash_id(&self) -> EmbeddingSetId {
        self.hash_id
    }

    fn rel_name(&self) -> &EmbRelName {
        // TODO: check if this is Arc::clone
        &self.rel_name
    }
}

pub trait EmbeddingSetExt {
    type Vector: Into<EmbeddingVector>;
    fn provider(&self) -> &EmbeddingProviderSlug;
    fn model(&self) -> &EmbeddingModelId;
    fn shape(&self) -> EmbeddingShape;
    fn hash_id(&self) -> EmbeddingSetId;
    fn rel_name(&self) -> &EmbRelName;
    fn hnsw_rel_name(&self) -> HnswRelName {
        let rel_name = self.rel_name();
        let hnsw_suffix = HNSW_SUFFIX;
        let hnsw_string = format!("{rel_name}{hnsw_suffix}");
        HnswRelName::new_from_str(&hnsw_string)
    }

    fn new_vector_with_node(&self, node_id: Uuid, vector: Vec<f64>) -> EmbeddingVector {
        EmbeddingVector {
            node_id,
            vector,
            embedding_set_id: self.hash_id(),
        }
    }
}

pub trait CozoEmbeddingSetExt: EmbeddingSetExt {
    const REL_NAME: &'static str = "embedding_set";

    fn embedding_set_relation_name() -> &'static str {
        Self::REL_NAME
    }

    fn script_create() -> &'static str {
        cozo_script_embedding_set!(":create", "embedding_set")
    }

    fn script_replace() -> &'static str {
        cozo_script_embedding_set!(":replace", "embedding_set")
    }

    fn script_identity(&self) -> &'static str {
        "id, at, => provider, model, dims, embedding_dtype, rel_name"
    }

    fn script_fields(&self) -> &'static str {
        "id, at, provider, model, dims, embedding_dtype, rel_name"
    }

    fn script_put(&self) -> String {
        format!(
            r#"?[{fields}] <- [[ '{hash_id}', 'ASSERT', '{provider}', '{model}', {shape_dims}, '{shape_embedding_dtype}', '{rel_name}' ]]

:put embedding_set {{
{identity}
}}"#,
            identity = self.script_identity(),
            fields = self.script_fields(),
            hash_id = self.hash_id(),
            provider = self.provider(),
            model = self.model(),
            shape_dims = self.shape().dimension,
            shape_embedding_dtype = self.shape().dtype_tag(),
            rel_name = self.rel_name()
        )
    }

    fn script_put_vector_with_param_batch(&self) -> String {
        let vector_fields = EmbeddingVector::script_fields();

        let script = format!(
            r#"
{{
    input[node_id, embedding_set_id, vector] <- $updates

    ?[node_id, embedding_set_id, vector, at] := 
        input[node_id, embedding_set_id, vector],
        at = 'ASSERT'

    :put {embedding_rel_name} {{ node_id, embedding_set_id, vector, at }}
}}
"#,
            embedding_rel_name = self.rel_name()
        );
        script
    }

    fn script_get_vector_rows(&self) -> String {
        let fields = EmbeddingVector::script_fields();
        let vector_rel_name = self.rel_name();
        format!("?[{fields}] := *{vector_rel_name} {{ {fields} }}")
    }

    fn script_count_vector_rows(&self) -> String {
        format!("{} {}", self.script_get_vector_rows(), " :count")
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
                "embedding_set_id".to_string(),
                DataValue::from(self.hash_id().into_inner() as i64),
            ),
            ("node_id".to_string(), to_cozo_uuid(node_id)),
            ("vector".to_string(), DataValue::List(vector_datavalue)),
        ])
    }

    fn script_vector_identity(&self) -> String {
        format!("{} {{ node_id, at, vector, embedding_set_id }}", self.rel_name())
    }
}
