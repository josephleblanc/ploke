use std::collections::BTreeMap;
use std::ops::Deref;

use crate::database::Database;
use crate::error::DbError;
use crate::multi_embedding::adapter::ExperimentalEmbeddingDbExt;
use crate::multi_embedding::schema::vector_dims::{
    supported_dimension_set, vector_literal, VectorDimensionSpec,
};
use cozo::{Db, MemStorage, ScriptMutability};
use uuid::Uuid;

#[derive(Copy, Clone, Debug)]
pub struct ExperimentalVectorRelation {
    dims: i64,
    relation_base: &'static str,
}

impl ExperimentalVectorRelation {
    pub fn try_new(dims: i64, relation_base: &'static str) -> Result<Self, DbError> {
        if supported_dimension_set().contains(&dims) {
            Ok(Self {
                dims,
                relation_base,
            })
        } else {
            Err(DbError::UnsupportedEmbeddingDimension { dims })
        }
    }

    pub fn new(dims: i64, relation_base: &'static str) -> Self {
        Self::try_new(dims, relation_base).expect("dimension must be supported")
    }

    pub fn dims(&self) -> i64 {
        self.dims
    }

    pub fn relation_name(&self) -> String {
        format!("{}_{}", self.relation_base, self.dims)
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
            self.dims
        )
    }

    pub fn insert_row(
        &self,
        db: &Database,
        node_id: Uuid,
        dim_spec: &VectorDimensionSpec,
    ) -> Result<(), DbError> {
        insert_vector_row(db.deref(), self, node_id, dim_spec)
    }

    /// Inserts the provided embedding vector into the runtime relation, replacing any previous row.
    pub fn upsert_vector_values(
        &self,
        db: &Database,
        node_id: Uuid,
        dim_spec: &VectorDimensionSpec,
        vector: &[f32],
    ) -> Result<(), DbError> {
        if vector.len() != self.dims as usize {
            return Err(DbError::ExperimentalVectorLengthMismatch {
                expected: self.dims as usize,
                actual: vector.len(),
            });
        }
        if dim_spec.dims() != self.dims {
            return Err(DbError::ExperimentalVectorLayoutMismatch {
                relation: self.relation_name(),
                dims: dim_spec.dims(),
            });
        }
        let identity = self.script_identity();
        let vector_literal = literal_from_values(vector);
        let script = format!(
            r#"
?[node_id, embedding_model, provider, at, embedding_dims, vector] <- [[
    to_uuid("{node_id}"),
    "{embedding_model}",
    "{provider}",
    'ASSERT',
    {embedding_dims},
    {vector_literal}
]] :put {identity}
"#,
            node_id = node_id,
            embedding_model = dim_spec.embedding_model(),
            provider = dim_spec.provider(),
            embedding_dims = dim_spec.dims(),
            vector_literal = vector_literal,
            identity = identity,
        );
        db.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
            .map_err(|err| DbError::ExperimentalScriptFailure {
                action: "insert_vector_row",
                relation: self.relation_name(),
                details: err.to_string(),
            })?;
        Ok(())
    }

    pub fn ensure_registered(&self, db: &Database) -> Result<(), DbError> {
        match db.ensure_relation_registered(&self.relation_name()) {
            Ok(()) => Ok(()),
            Err(DbError::ExperimentalRelationMissing { .. }) => db
                .run_script(
                    &self.script_create(),
                    BTreeMap::new(),
                    ScriptMutability::Mutable,
                )
                .map(|_| ())
                .map_err(|err| DbError::ExperimentalScriptFailure {
                    action: "vector_relation_create",
                    relation: self.relation_name(),
                    details: err.to_string(),
                }),
            Err(other) => Err(other),
        }
    }
}

fn insert_vector_row(
    db: &Db<MemStorage>,
    relation: &ExperimentalVectorRelation,
    node_id: Uuid,
    dim_spec: &VectorDimensionSpec,
) -> Result<(), DbError> {
    let identity = relation.script_identity();
    let literal = vector_literal(dim_spec.dims() as usize, dim_spec.offset());
    let script = format!(
        r#"
?[node_id, embedding_model, provider, at, embedding_dims, vector] <- [[
    to_uuid("{node_id}"),
    "{embedding_model}",
    "{provider}",
    'ASSERT',
    {embedding_dims},
    {vector_literal}
]] :put {identity}
"#,
        node_id = node_id,
        embedding_model = dim_spec.embedding_model(),
        provider = dim_spec.provider(),
        embedding_dims = dim_spec.dims(),
        vector_literal = literal,
        identity = identity,
    );
    db.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
        .map_err(|err| DbError::ExperimentalScriptFailure {
            action: "insert_vector_row",
            relation: relation.relation_name(),
            details: err.to_string(),
        })?;
    Ok(())
}

pub fn literal_from_values(vector: &[f32]) -> String {
    let values = vector
        .iter()
        .map(|value| format!("{:.10}", value))
        .collect::<Vec<_>>()
        .join(", ");
    format!("vec([{values}])")
}
