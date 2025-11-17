use super::*;
use super::specs::*;
use super::definitions::*;

use std::collections::{BTreeMap, HashSet};

use crate::database::Database;
use crate::error::DbError;
use crate::NodeType;
use cozo::{self, DataValue, Db, MemStorage, NamedRows, Num, ScriptMutability, UuidWrapper};
use itertools::Itertools;
use lazy_static::lazy_static;
use std::ops::Deref;
use uuid::Uuid;


    fn seed_metadata_relation(
        spec: &ExperimentalNodeSpec,
    ) -> Result<(Database, SampleNodeData), DbError> {
        let db = init_db();
        let relation_name = spec.base.metadata_schema.relation().to_string();
        db.run_script(
            &spec.base.metadata_schema.script_create(),
            BTreeMap::new(),
            ScriptMutability::Mutable,
        )
        .map_err(|err| DbError::ExperimentalScriptFailure {
            action: "schema_create",
            relation: relation_name,
            details: err.to_string(),
        })?;

        let sample = (spec.sample_builder)();
        insert_metadata_sample(&db, spec, &sample)?;
        Ok((db, sample))
    }

    fn seed_vector_relation_for_node(
        db: &Database,
        spec: &ExperimentalNodeSpec,
        node_id: Uuid,
        dim_spec: &VectorDimensionSpec,
    ) -> Result<ExperimentalVectorRelation, DbError> {
        let vector_relation =
            ExperimentalVectorRelation::new(dim_spec.dims, spec.base.vector_relation_base);
        let relation_name = vector_relation.relation_name();
        match db.ensure_relation_registered(&relation_name) {
            Ok(()) => {}
            Err(DbError::ExperimentalRelationMissing { .. }) => {
                let create_script = vector_relation.script_create();
                db.run_script(&create_script, BTreeMap::new(), ScriptMutability::Mutable)
                    .map_err(|err| DbError::ExperimentalScriptFailure {
                        action: "vector_relation_create",
                        relation: relation_name.clone(),
                        details: err.to_string(),
                    })?;
            }
            Err(other) => return Err(other),
        }
        insert_vector_row(db, &vector_relation, node_id, dim_spec)?;
        Ok(vector_relation)
    }

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
}

