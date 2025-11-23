use std::collections::BTreeMap;
use std::ops::Deref;

use crate::database::Database;
use crate::error::DbError;
use crate::multi_embedding::adapter::ExperimentalEmbeddingDbExt;
use crate::multi_embedding::schema::vector_dims::{vector_literal, VectorDimensionSpec};
use cozo::{Db, MemStorage, ScriptMutability};
use ploke_core::{ArcStr, EmbeddingModelId};
use uuid::Uuid;

#[derive(Clone, Eq, PartialEq, Ord, PartialOrd, Hash, Default, Debug)]
pub struct EmbeddingModel(ArcStr);

impl AsRef<str> for EmbeddingModel {
    fn as_ref(&self) -> &str {
        self.0.as_ref()
    }
}

#[derive(Clone, Debug)]
pub struct ExperimentalVectorRelation {
    dims: i64,
    embedding_model: EmbeddingModelId,
}

impl ExperimentalVectorRelation {
    pub fn new(dims: i64, embedding_model: EmbeddingModelId) -> Self {
        Self {
            dims,
            embedding_model,
        }
    }

    pub fn dims(&self) -> i64 {
        self.dims
    }

    pub fn relation_name(&self) -> String {
        let model = sanitize_relation_component(self.embedding_model.as_ref());
        format!("emb_{}_{}", model, self.dims)
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

fn sanitize_relation_component(raw: &str) -> String {
    let mut out = String::with_capacity(raw.len() + 4);
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() || out.starts_with(|c: char| c.is_ascii_digit()) {
        out.insert(0, 'v');
    }
    out
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
