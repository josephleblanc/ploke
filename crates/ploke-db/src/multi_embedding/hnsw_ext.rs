use std::{collections::BTreeMap, ops::Deref as _};

use cozo::{DataValue, Num, ScriptMutability, UuidWrapper};
use itertools::Itertools;
use ploke_core::embeddings::EmbeddingSet;
use syn_parser::utils::LogStyle;
use tracing::{debug, info, instrument};
use uuid::Uuid;

use crate::{
    database::HNSW_SUFFIX,
    multi_embedding::{
        db_ext::EmbeddingExt,
        schema::{EmbeddingSetExt, EmbeddingVector},
    },
    Database, DbError, EmbedDataVerbose, NodeType, QueryResult, TypedEmbedData,
};

pub(crate) const HNSW_TARGET: &str = "hnsw-index";

pub trait HnswExt {
    fn ensure_embedding_relation(&self, embedding_set: &EmbeddingSet) -> Result<(), DbError>;
    fn create_embedding_index(&self, embedding_set: &EmbeddingSet) -> Result<(), DbError>;
    fn hnsw_neighbors_for_type(
        &self,
        node_type: NodeType,
        embedding_set: &EmbeddingSet,
        k: usize,
        ef: usize,
    ) -> Result<Vec<(Uuid, String, DataValue)>, ploke_error::Error>;
    fn search_similar_for_set(
        &self,
        embedding_set: &EmbeddingSet,
        node_type: NodeType,
        vector_query: Vec<f32>,
        k: usize,
        ef: usize,
        limit: usize,
        radius: Option<f64>,
    ) -> Result<EmbedDataVerbose, ploke_error::Error>;

    fn search_similar_for_set_test(
        &self,
        embedding_set: &EmbeddingSet,
        node_type: NodeType,
        vector_query: Vec<f32>,
        k: usize,
        ef: usize,
        limit: usize,
        radius: Option<f64>,
    ) -> Result<EmbedDataVerbose, ploke_error::Error>;

    fn vec_to_param(vector_query: Vec<f32>) -> DataValue {
        let as_list = vector_query
            .into_iter()
            .map(|fl| {
                if (fl as f64).is_subnormal() {
                    1.0
                } else {
                    fl as f64
                }
            })
            .map(|fl| DataValue::Num(Num::Float(fl)))
            .collect_vec();
        DataValue::List(as_list)
    }

    fn is_hnsw_index_registered(&self, embedding_set: &EmbeddingSet) -> Result<bool, DbError>;

    fn create_index_warn(&self, enbedding_set: &EmbeddingSet) -> Result<(), ploke_error::Error>;
}

impl HnswExt for cozo::Db<cozo::MemStorage> {
    fn ensure_embedding_relation(&self, embedding_set: &EmbeddingSet) -> Result<(), DbError> {
        if self.is_relation_registered(&embedding_set.rel_name)? {
            return Ok(());
        }

        let script = EmbeddingVector::script_create_from_set(embedding_set);
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|err| DbError::EmbeddingScriptFailure {
                action: "create_embedding_relation",
                relation: embedding_set.rel_name.clone(),
                details: err.to_string(),
            })
    }

    fn create_embedding_index(&self, embedding_set: &EmbeddingSet) -> Result<(), DbError> {
        self.ensure_embedding_relation(embedding_set)?;

        let rel_name = embedding_set.rel_name.as_ref().replace('-', "_");
        let hnsw_suffix = HNSW_SUFFIX;

        let hnsw_index_rel_name = embedding_set.hnsw_rel_name();
        let dim = embedding_set.dims();

        // Only skip creation if the HNSW index relation already exists.
        if self
            .is_hnsw_index_registered(embedding_set)
            .map_err(|e| DbError::Cozo(e.to_string()))?
        {
            return Ok(());
        }
        tracing::info!(target: "cozo-script",
            "create new relation: '{hnsw_index_rel_name}'\nrel_name = '{rel_name}'\nhnsw_suffix = '{hnsw_suffix}'",
        );
        let script = format!(
            r#"
::hnsw create {hnsw_index_rel_name} {{
    fields: [vector],
    dim: {dim},
    dtype: F32,
    m: 32,
    ef_construction: 200,
    distance: L2
}}
"#
        );

        tracing::debug!(target: "cozo-script", "create_embedding_index script:\n{script}");

        self.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|err| {
                tracing::error!(
                    target: "cozo-script",
                    "create_embedding_index error: {err}; script:\n{script}"
                );
                DbError::Cozo(err.to_string())
            })
    }

    fn hnsw_neighbors_for_type(
        &self,
        node_type: NodeType,
        embedding_set: &EmbeddingSet,
        k: usize,
        ef: usize,
    ) -> Result<Vec<(Uuid, String, DataValue)>, ploke_error::Error> {
        let rel_name = node_type.relation_str();
        let embed_rel = embedding_set.rel_name.as_ref().replace('-', "_");

        let mut params = BTreeMap::new();
        params.insert("k".into(), DataValue::from(k as i64));
        params.insert("ef".into(), DataValue::from(ef as i64));
        params.insert(
            "embedding_set_id".into(),
            DataValue::from(embedding_set.hash_id().into_inner() as i64),
        );

        let script = format!(
            r#"
?[result_id, name, distance] :=
    *{embed_rel}{{ node_id: id, vector: v, embedding_set_id: set_id @ 'NOW' }},
    set_id = $embedding_set_id,
    ~{embed_rel}{hnsw_suffix}{{ node_id: result_id, embedding_set_id: idx_set |
        query: v,
        k: $k,
        ef: $ef,
        bind_distance: distance
    }},
    idx_set = $embedding_set_id,
    *{rel_name}{{ id: result_id, name @ 'NOW' }}
"#,
            embed_rel = embed_rel,
            hnsw_suffix = HNSW_SUFFIX,
            rel_name = rel_name,
        );

        tracing::debug!(
            target: "cozo-script",
            script = %script,
            rel = %rel_name,
            embed_rel = %embed_rel,
            k,
            ef,
            set_id = embedding_set.hash_id().into_inner()
        );
        let result = match self.run_script(&script, params, ScriptMutability::Immutable) {
            Ok(rows) => Ok(rows),
            Err(err) => {
                tracing::error!(
                    target: "cozo-script",
                    "hnsw_neighbors_for_type error: {err}; script:\n{script}"
                );
                if err.to_string().contains("hnsw_idx") {
                    Err(ploke_error::Error::Warning(
                        ploke_error::WarningError::PlokeDb(err.to_string()),
                    ))
                } else {
                    Err(DbError::Cozo(err.to_string()).into())
                }
            }
        }?;

        let mut neighbors = Vec::new();
        for row in result.rows {
            let id = match row.get(0) {
                Some(DataValue::Uuid(UuidWrapper(uuid))) => *uuid,
                _ => continue,
            };
            let name = row
                .get(1)
                .and_then(|v| v.get_str())
                .unwrap_or_default()
                .to_string();
            let dist = row.get(2).cloned().unwrap_or(DataValue::Null);
            neighbors.push((id, name, dist));
        }
        Ok(neighbors)
    }

    // TODO: Delete once this is actually working
    fn search_similar_for_set_test(
        &self,
        embedding_set: &EmbeddingSet,
        node_type: NodeType,
        vector_query: Vec<f32>,
        k: usize,
        ef: usize,
        limit: usize,
        radius: Option<f64>,
    ) -> Result<EmbedDataVerbose, ploke_error::Error> {
        let mut params = BTreeMap::new();
        params.insert("k".into(), DataValue::from(k as i64));
        params.insert("ef".into(), DataValue::from(ef as i64));
        params.insert("limit".into(), DataValue::from(limit as i64));
        params.insert("vector_query".into(), Self::vec_to_param(vector_query));
        params.insert(
            "embedding_set_id".into(),
            DataValue::from(embedding_set.hash_id().into_inner() as i64),
        );
        if let Some(radius) = radius {
            params.insert("radius".into(), DataValue::Num(Num::Float(radius)));
        }

        let rel = node_type.relation_str();
        let embed_rel = embedding_set.rel_name.as_ref().replace('-', "_");
        let radius_clause = if radius.is_some() {
            ",\n        radius: $radius,"
        } else {
            ","
        };
        let hnsw_rel = embedding_set.hnsw_rel_name();
        let p = |script: &str| -> Result<(), DbError> {
            let s = format!("::indices {}", script);
            info!("trying basic script:\n\t{}", s.log_magenta());
            let out = self
                .run_script(&s, BTreeMap::new(), ScriptMutability::Immutable)
                .map_err(DbError::from)
                .map(|r| format!("{r:?}\n"))?;
            debug!(%out);
            Ok(())
        };
        let pp = |script: &str| -> Result<(), DbError> {
            let s = script;
            info!("trying basic script:\n\t{}", s.log_magenta());
            let out = self
                .run_script(&s, BTreeMap::new(), ScriptMutability::Immutable)
                .map_err(DbError::from)
                .map(|r| format!("{r:?}\n"))?;
            debug!(%out);
            Ok(())
        };
        p(hnsw_rel.as_ref())?;
        p(embed_rel.as_ref())?;
        let const_with_embed = format!(
            r#"?[id, name, vector] := *const {{ name, id }}, *{embed_rel} {{node_id: id, vector }}"#
        );
        pp(&const_with_embed)?;
        let count_pending = self.count_pending_embeddings(embedding_set)?;
        info!(?count_pending);
        let script_count_vecs = format!("?[id] := *{embed_rel} {{node_id: id }}");
        pp(&script_count_vecs)?;
        pp("::relations")?;
        let script = format!(
            r#"
from_index[node_id, distance] :=
    ~{embed_rel}{hnsw_suffix}{{ node_id, embedding_set_id: set_id |
        query: vec($vector_query),
        k: $k,
        ef: $ef{radius_clause}
        bind_distance: distance
    }},
    set_id = $embedding_set_id

has_embedding[id, name, distance] :=
    from_index[node_id, distance],
    *{rel}{{ id, name, tracking_hash: hash, span @ 'NOW' }},
    id == node_id

?[id, name, distance] :=
    has_embedding[id, name, distance]
:limit $limit
"#,
            embed_rel = embed_rel,
            hnsw_suffix = HNSW_SUFFIX,
            rel = rel,
            radius_clause = radius_clause,
        );
        debug!(hnsw_script = %script);

        let query_result = match self.run_script(&script, params, ScriptMutability::Immutable) {
            Ok(rows) => Ok(QueryResult::from(rows)),
            Err(err) => {
                if err.to_string().contains("hnsw_idx") {
                    Err(ploke_error::Error::Warning(
                        ploke_error::WarningError::PlokeDb(err.to_string()),
                    ))
                } else {
                    Err(DbError::Cozo(err.to_string()).into())
                }
            }
        };
        debug!(hnsw_query_result = ?query_result);
        let query_result = query_result?;

        let mut dist_vec = Vec::new();
        if let Some(dist_idx) = query_result.headers.iter().position(|h| h == "distance") {
            dist_vec.extend(
                query_result
                    .rows
                    .iter()
                    .filter_map(|row| row.get(dist_idx).and_then(|v| v.get_float())),
            );
        }

        let v = query_result.to_embedding_nodes()?;
        let typed_data = TypedEmbedData { v, ty: node_type };
        Ok(EmbedDataVerbose {
            typed_data,
            dist: dist_vec,
        })
    }

    fn search_similar_for_set(
        &self,
        embedding_set: &EmbeddingSet,
        node_type: NodeType,
        vector_query: Vec<f32>,
        k: usize,
        ef: usize,
        limit: usize,
        radius: Option<f64>,
    ) -> Result<EmbedDataVerbose, ploke_error::Error> {
        let mut params = BTreeMap::new();
        params.insert("k".into(), DataValue::from(k as i64));
        params.insert("ef".into(), DataValue::from(ef as i64));
        params.insert("limit".into(), DataValue::from(limit as i64));
        params.insert("vector_query".into(), Self::vec_to_param(vector_query));
        params.insert(
            "embedding_set_id".into(),
            DataValue::from(embedding_set.hash_id().into_inner() as i64),
        );
        if let Some(radius) = radius {
            params.insert("radius".into(), DataValue::Num(Num::Float(radius)));
        }

        let rel = node_type.relation_str();
        let embed_rel = embedding_set.rel_name.as_ref().replace('-', "_");
        let radius_clause = if radius.is_some() {
            ",\n        radius: $radius,"
        } else {
            ","
        };
        let script = format!(
            r#"
parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}}

ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

from_index[node_id, distance] :=
    ~{embed_rel}{hnsw_suffix}{{ node_id, embedding_set_id: set_id |
        query: vec($vector_query),
        k: $k,
        ef: $ef{radius_clause}
        bind_distance: distance
    }},
    set_id = $embedding_set_id

has_embedding[id, name, hash, span, distance] :=
    from_index[node_id, distance],
    *{rel}{{ id, name, tracking_hash: hash, span @ 'NOW' }},
    id == node_id

is_root_module[id] := *module{{id @ 'NOW' }}, *file_mod {{owner_id: id @ 'NOW'}}

batch[id, name, file_path, file_hash, hash, span, namespace, distance] :=
    has_embedding[id, name, hash, span, distance],
    ancestor[id, mod_id],
    is_root_module[mod_id],
    *module{{id: mod_id, tracking_hash: file_hash @ 'NOW'}},
    *file_mod {{ owner_id: mod_id, file_path, namespace @ 'NOW'}}

?[id, name, file_path, file_hash, hash, span, namespace, distance] :=
    batch[id, name, file_path, file_hash, hash, span, namespace, distance]
:order distance
:limit $limit
"#,
            embed_rel = embed_rel,
            hnsw_suffix = HNSW_SUFFIX,
            rel = rel,
            radius_clause = radius_clause,
        );
        debug!(target: "cozo-script", hnsw_script = %script);

        let query_result = match self.run_script(&script, params, ScriptMutability::Immutable) {
            Ok(rows) => Ok(QueryResult::from(rows)),
            Err(err) => {
                if err.to_string().contains("hnsw_idx") {
                    Err(ploke_error::Error::Warning(
                        ploke_error::WarningError::PlokeDb(err.to_string()),
                    ))
                } else {
                    Err(DbError::Cozo(err.to_string()).into())
                }
            }
        };
        debug!(hnsw_query_result = ?query_result);
        let query_result = query_result?;

        let mut dist_vec = Vec::new();
        if let Some(dist_idx) = query_result.headers.iter().position(|h| h == "distance") {
            dist_vec.extend(
                query_result
                    .rows
                    .iter()
                    .filter_map(|row| row.get(dist_idx).and_then(|v| v.get_float())),
            );
        }

        let v = query_result.to_embedding_nodes()?;
        let typed_data = TypedEmbedData { v, ty: node_type };
        Ok(EmbedDataVerbose {
            typed_data,
            dist: dist_vec,
        })
    }

    #[instrument(
        skip(self),
        fields(?embedding_set, script_check_indices),
        level = "debug",
        ret
    )]
    fn is_hnsw_index_registered(&self, embedding_set: &EmbeddingSet) -> Result<bool, DbError> {
        // TODO: introspect ::indices output to avoid repeated creation; currently we
        // always attempt to create the index, but we still run the query to ensure the
        // script is valid under the current embedding schema.
        let hnsw_rel_name = embedding_set.hnsw_rel_name();
        let script_check_indices = format!("::indices {hnsw_rel_name}");
        let db_res = self
            .run_script(
                &script_check_indices,
                BTreeMap::new(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(DbError::from);
        let expect_err_msg = format!("Cannot find requested stored relation '{hnsw_rel_name}'");
        match db_res {
            // the script succeeds, finding the item in the database
            Ok(_) => Ok(true),
            // the script fails, with a message that the item is not found in the database
            Err(e) if e == DbError::Cozo(expect_err_msg) => Ok(false),
            // the script fails for some other reason
            Err(e) => Err(e),
        }
    }

    fn create_index_warn(&self, enbedding_set: &EmbeddingSet) -> Result<(), ploke_error::Error> {
        todo!()
    }
}

impl HnswExt for Database {
    fn ensure_embedding_relation(&self, embedding_set: &EmbeddingSet) -> Result<(), DbError> {
        self.deref().ensure_embedding_relation(embedding_set)
    }

    fn create_embedding_index(&self, embedding_set: &EmbeddingSet) -> Result<(), DbError> {
        self.deref().create_embedding_index(embedding_set)
    }

    fn hnsw_neighbors_for_type(
        &self,
        node_type: NodeType,
        embedding_set: &EmbeddingSet,
        k: usize,
        ef: usize,
    ) -> Result<Vec<(Uuid, String, DataValue)>, ploke_error::Error> {
        self.deref()
            .hnsw_neighbors_for_type(node_type, embedding_set, k, ef)
    }

    fn search_similar_for_set(
        &self,
        embedding_set: &EmbeddingSet,
        node_type: NodeType,
        vector_query: Vec<f32>,
        k: usize,
        ef: usize,
        limit: usize,
        radius: Option<f64>,
    ) -> Result<EmbedDataVerbose, ploke_error::Error> {
        self.deref().search_similar_for_set(
            embedding_set,
            node_type,
            vector_query,
            k,
            ef,
            limit,
            radius,
        )
    }

    fn is_hnsw_index_registered(&self, embedding_set: &EmbeddingSet) -> Result<bool, DbError> {
        self.deref().is_hnsw_index_registered(embedding_set)
    }

    fn create_index_warn(&self, enbedding_set: &EmbeddingSet) -> Result<(), ploke_error::Error> {
        todo!()
    }

    fn search_similar_for_set_test(
        &self,
        embedding_set: &EmbeddingSet,
        node_type: NodeType,
        vector_query: Vec<f32>,
        k: usize,
        ef: usize,
        limit: usize,
        radius: Option<f64>,
    ) -> Result<EmbedDataVerbose, ploke_error::Error> {
        self.deref().search_similar_for_set_test(
            embedding_set,
            node_type,
            vector_query,
            k,
            ef,
            limit,
            radius,
        )
    }
}

#[cfg(test)]
pub(crate) use tests::init_tracing_once;

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::{Arc, Once},
    };

    use super::*;
    use cozo::NamedRows;
    use ploke_error::Error;
    use syn_parser::utils::LogStyle;
    use tracing::info;

    use crate::{
        database::ImmutQuery,
        log_script,
        multi_embedding::{
            schema::CozoEmbeddingSetExt,
            test_utils::{eprint_relations, setup_db, setup_empty_db},
        },
        run_script_params,
    };

    static TEST_TRACING: Once = Once::new();
    pub(crate) fn init_tracing_once(target: &'static str, level: tracing::Level) {
        TEST_TRACING.call_once(|| {
            ploke_test_utils::init_test_tracing_with_target(target, level);
        });
    }

    fn log_step(header: &'static str, relation_name: &str) {
        tracing::info!(
            target: "cozo-script",
            "{}\n\t{}: {}",
            header.log_header(),
            "relation".log_step(),
            relation_name.log_magenta()
        );
    }

    /// Seed a handful of vectors for functions so HNSW queries can run against real data.
    fn seed_function_vectors(
        db: &Database,
        embedding_set: &EmbeddingSet,
        limit: usize,
    ) -> Result<Vec<Uuid>, Error> {
        let fetch_ids = format!(
            "?[id, name] := *function{{ id, name @ 'NOW' }} :limit {limit}",
            limit = limit
        );
        let rows = db
            .run_script(&fetch_ids, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(DbError::from)?;
        let ids: Vec<Uuid> = rows
            .rows
            .iter()
            .filter_map(|r| {
                if let DataValue::Uuid(UuidWrapper(u)) = r[0] {
                    Some(u)
                } else {
                    None
                }
            })
            .collect();
        assert!(
            !ids.is_empty(),
            "fixture should contain function ids for HNSW neighbors test"
        );

        let updates = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let val = (i as f64) + 0.1;
                embedding_set
                    .new_vector_with_node(*id, vec![val; embedding_set.dims() as usize])
                    .into_cozo_datavalue()
            })
            .collect::<Vec<_>>();
        let mut params = BTreeMap::new();
        params.insert("updates".to_string(), DataValue::List(updates));
        let put_vectors = embedding_set.script_put_vector_with_param_batch();
        db.run_script(&put_vectors, params, ScriptMutability::Mutable)
            .map_err(DbError::from)?;

        Ok(ids)
    }

    #[test]
    // WARNING: This test verifies that if we rely on the old version of the embeddings stored in
    // the database under the "multi_embedding_*" flags we cannot load the old database due to the
    // way we do not expose the old chema under the new flags. Under the cfg flag combinations it
    // is impossible to load the old schema.
    //
    // NOTE: Keeping this just so we don't try to do the same thing again in the future.
    fn test_is_hnsw_index_registered() -> Result<(), Error> {
        // init_tracing_once("cozo-script", tracing::Level::INFO);
        let cozo_db = setup_db()?;
        let db = Database::new(cozo_db);

        let embedding_set = EmbeddingSet::default();
        let db_res = db.is_hnsw_index_registered(&embedding_set);
        eprint_relations(&db)?;

        let relation = embedding_set.rel_name().as_ref();
        log_step("create vector-unique relation", relation);
        let db_ret = db.create_embedding_index(&embedding_set);
        info!(create_vector_rel = ?db_ret);

        log_step("check for previous hnsw indices per type", "primary nodes");

        let legacy_node_script = script_get_legacy_node_embeddings();
        log_script!(
            "get vector embeddings from 'embedding' field",
            "primary nodes",
            legacy_node_script
        );
        let rows_result = db
            .run_script(
                &legacy_node_script,
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .map_err(DbError::from);
        let expected_err =
            DbError::Cozo("stored relation 'function' does not have field 'embedding'".to_string());
        let rows = match rows_result {
            // unexpectedly passing (at time of writing, we do not migrate and use cfg flags to
            // keep the legacy vs multi_embedding schema isolated in different builds)
            Ok(r) => r,
            // expected error, doesn't find legacy "embedding" field with new "multi_embedding"
            // cfg active
            Err(e) if e == expected_err => return Ok(()),
            // unexpected error
            Err(e) => return Err(Error::from(e)),
        };

        let mut updates_data: Vec<DataValue> = Vec::new();
        for row in rows.into_iter() {
            let id = row[0].to_owned();
            let ev = row[1].to_owned();
            if !matches!(id, DataValue::Uuid(_)) {
                tracing::error!("expected id to be of DataValue::Uuid(_) type, found: {id:?}");
                panic!();
            } else if !matches!(ev, DataValue::Vec(_)) {
                tracing::error!("expected ev to be of DataValue::Vec(_) type, found: {ev:?}");
                panic!();
            }
            let set_id = DataValue::Num(Num::Int(embedding_set.hash_id().into_inner() as i64));
            updates_data.push(DataValue::List(vec![id, set_id, ev]));
        }
        let params = BTreeMap::from([("updates".to_string(), DataValue::List(updates_data))]);
        let put_vectors_batch = embedding_set.script_put_vector_with_param_batch();
        run_script_params!(
            db,
            cozo::ScriptMutability::Mutable,
            "Testing Script:",
            "put vector embeddings batch with params",
            &put_vectors_batch,
            params
        )?;

        Ok(())
    }

    #[test]
    fn hnsw_neighbors_for_type_roundtrip() -> Result<(), Error> {
        // init_tracing_once("cozo-script", tracing::Level::DEBUG);
        // Use fixture DB with multi-embedding schema and default embedding set.
        let cozo_db = setup_db()?;
        let db = Database::new(cozo_db);

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = db.with_active_set(|set| set.clone())?;

        // Ensure vector relation exists.
        db.ensure_embedding_relation(&embedding_set)?;

        // Take a couple of function ids to seed vectors.
        let fetch_ids = r#"?[id, name] := *function{ id, name @ 'NOW' } :limit 3"#;
        let rows = db
            .run_script(fetch_ids, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(DbError::from)?;
        let ids: Vec<Uuid> = rows
            .rows
            .iter()
            .filter_map(|r| {
                if let DataValue::Uuid(UuidWrapper(u)) = r[0] {
                    Some(u)
                } else {
                    None
                }
            })
            .collect();
        assert!(
            !ids.is_empty(),
            "fixture should contain function ids for HNSW neighbors test"
        );

        let db_res = db.is_hnsw_index_registered(&embedding_set)?;
        info!(is_hnsw_index_registered_before = ?db_res);
        assert!(!db_res, "Expect hnsw index is not registered initially.");

        // Insert vectors for these ids.
        let updates = ids
            .iter()
            .enumerate()
            .map(|(i, id)| {
                let val = (i as f64) + 0.1;
                embedding_set
                    .new_vector_with_node(*id, vec![val; embedding_set.dims() as usize])
                    .into_cozo_datavalue()
            })
            .collect::<Vec<_>>();
        let mut params = BTreeMap::new();
        params.insert("updates".to_string(), DataValue::List(updates));
        let put_vectors = embedding_set.script_put_vector_with_param_batch();
        db.run_script(&put_vectors, params, ScriptMutability::Mutable)
            .map_err(DbError::from)?;

        // Create the HNSW index and query neighbors.
        // Create HNSW index manually to capture parse errors.
        let rel_name_index = embedding_set.rel_name.as_ref().replace('-', "_");
        let index_script = format!(
            r#"
::hnsw create {rel_name}{hnsw_suffix} {{
    fields: [vector],
    dim: {dim},
    dtype: F32,
    m: 32,
    ef_construction: 200,
    distance: L2
}}
"#,
            rel_name = rel_name_index,
            hnsw_suffix = HNSW_SUFFIX,
            dim = embedding_set.dims()
        );
        db.run_script(&index_script, BTreeMap::new(), ScriptMutability::Mutable)
            .unwrap_or_else(|e| {
                panic!("failed to create hnsw index: {e}\nscript:\n{index_script}")
            });

        let db_res = db.is_hnsw_index_registered(&embedding_set)?;
        info!(is_hnsw_index_registered_after = ?db_res);
        assert!(
            db_res,
            "Expect hnsw index is registered after running create script"
        );

        // Manually run the neighbors script to surface parser errors directly.
        let rel_name = NodeType::Function.relation_str();
        let embed_rel = embedding_set.rel_name.as_ref().replace('-', "_");
        let mut params = BTreeMap::new();
        params.insert("k".into(), DataValue::from(2i64));
        params.insert("ef".into(), DataValue::from(10i64));
        params.insert(
            "embedding_set_id".into(),
            DataValue::from(embedding_set.hash_id().into_inner() as i64),
        );
        let script = format!(
            r#"
?[result_id, name, distance] :=
    *{embed_rel}{{ node_id: id, vector: v, embedding_set_id: set_id @ 'NOW' }},
    set_id = $embedding_set_id,
    ~{embed_rel}{hnsw_suffix}{{ node_id: result_id, embedding_set_id: idx_set |
        query: v,
        k: $k,
        ef: $ef,
        bind_distance: distance
    }},
    idx_set = $embedding_set_id,
    *{rel_name}{{ id: result_id, name @ 'NOW' }}
"#,
            embed_rel = embed_rel,
            hnsw_suffix = HNSW_SUFFIX,
            rel_name = rel_name
        );
        let rows = match db.run_script(&script, params, ScriptMutability::Immutable) {
            Ok(r) => r,
            Err(e) => panic!("hnsw neighbor script failed: {e}\nscript:\n{script}"),
        };

        assert!(
            !rows.rows.is_empty(),
            "expected neighbors for functions with inserted vectors"
        );

        Ok(())
    }

    #[test]
    fn hnsw_neighbors_for_type_api_roundtrip() -> Result<(), Error> {
        // init_tracing_once("cozo-script", tracing::Level::DEBUG);
        let cozo_db = setup_db()?;
        let db = Database::new(cozo_db);
        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = db.with_active_set(|set| set.clone())?;

        // Prepare embeddings and index using the public API.
        db.ensure_embedding_relation(&embedding_set)?;
        tracing::info!(target: "cozo-script", "seed vectors for functions");
        let seeded_ids = seed_function_vectors(&db, &embedding_set, 4)?;
        tracing::info!(target: "cozo-script", "seeded ids: {:?}", seeded_ids);
        tracing::info!(target: "cozo-script", "create hnsw index");
        db.create_embedding_index(&embedding_set)?;
        tracing::info!(target: "cozo-script", "invoke hnsw_neighbors_for_type");

        let neighbors = match db.hnsw_neighbors_for_type(NodeType::Function, &embedding_set, 2, 10)
        {
            Ok(n) => n,
            Err(err) => {
                // Mirror the method's script so failures print a runnable query.
                let rel_name = NodeType::Function.relation_str();
                let embed_rel = embedding_set.rel_name.as_ref().replace('-', "_");
                let mut params = BTreeMap::new();
                params.insert("k".into(), DataValue::from(2i64));
                params.insert("ef".into(), DataValue::from(10i64));
                params.insert(
                    "embedding_set_id".into(),
                    DataValue::from(embedding_set.hash_id().into_inner() as i64),
                );
                let script = format!(
                    r#"
?[result_id, name, distance] :=
    *{embed_rel}{{ node_id: id, vector: v, embedding_set_id: set_id @ 'NOW' }},
    set_id = $embedding_set_id,
    ~{embed_rel}{hnsw_suffix}{{ node_id: result_id, embedding_set_id: idx_set |
        query: v,
        k: $k,
        ef: $ef,
        bind_distance: distance
    }},
    idx_set = $embedding_set_id,
    *{rel_name}{{ id: result_id, name @ 'NOW' }}
"#,
                    embed_rel = embed_rel,
                    hnsw_suffix = HNSW_SUFFIX,
                    rel_name = rel_name
                );
                let manual = db.run_script(&script, params, ScriptMutability::Immutable);
                panic!(
                        "hnsw_neighbors_for_type failed: {err:?}\nscript:\n{script}\nmanual run result: {manual:?}"
                    );
            }
        };

        let seeded: HashSet<Uuid> = seeded_ids.into_iter().collect();
        assert!(
            !neighbors.is_empty(),
            "expected at least one neighbor row from hnsw_neighbors_for_type"
        );
        assert!(
            neighbors.iter().any(|(id, _, _)| seeded.contains(id)),
            "neighbor ids should come from seeded function vectors"
        );

        Ok(())
    }

    #[test]
    fn search_similar_for_set_returns_expected_neighbors() -> Result<(), Error> {
        // init_tracing_once("cozo-script", tracing::Level::DEBUG);
        let cozo_db = setup_db()?;
        let db = Database::new(cozo_db);
        let embedding_set = db.active_embedding_set.clone();

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = db.with_active_set(|set| set.clone())?;
        db.ensure_embedding_relation(&embedding_set)?;
        let seeded_ids = seed_function_vectors(&db, &embedding_set, 4)?;

        let before_is_index_registered = db.is_hnsw_index_registered(&embedding_set)?;
        info!(?before_is_index_registered);

        db.create_embedding_index(&embedding_set)?;

        let after_is_index_registered = db.is_hnsw_index_registered(&embedding_set)?;
        info!(?after_is_index_registered);

        let dims = embedding_set.dims() as usize;
        let query_vector = vec![0.1f32; dims];
        let k = 3;
        let ef = 10;
        let limit = 3;

        let result = db.search_similar_for_set(
            &embedding_set,
            NodeType::Function,
            query_vector.clone(),
            k,
            ef,
            limit,
            None,
        )?;

        let seeded: HashSet<Uuid> = seeded_ids.iter().copied().collect();
        assert!(
            !result.typed_data.v.is_empty(),
            "expected search_similar_for_set to return neighbors"
        );
        assert_eq!(
            result.typed_data.ty,
            NodeType::Function,
            "typed_data should indicate function nodes"
        );
        assert_eq!(
            result.typed_data.v.len(),
            result.dist.len(),
            "distance vector should align with returned nodes"
        );
        assert!(result.dist.len() <= limit, "results should respect limit");
        assert!(
            result
                .typed_data
                .v
                .iter()
                .all(|row| seeded.contains(&row.id)),
            "returned ids should come from seeded vectors"
        );
        assert!(
            result.dist.iter().copied().fold(f64::INFINITY, f64::min) < 1e-6,
            "closest neighbor should match the query vector"
        );

        let radius = Some(0.5);
        let radius_result = db.search_similar_for_set(
            &embedding_set,
            NodeType::Function,
            query_vector,
            k,
            ef,
            limit,
            radius,
        )?;

        assert!(
            radius_result.typed_data.v.len() == 1,
            "radius filter should reduce to the exact match neighbor"
        );
        assert!(
            radius_result.dist.first().map(|d| d.abs()).unwrap_or(1.0) < 1e-6,
            "radius-filtered neighbor should have near-zero distance"
        );

        Ok(())
    }

    fn script_get_legacy_node_embeddings() -> String {
        // let legacy_embeddings_script_rhs = EMBEDDABLE_NODES_NOW_LEGACY_RHS.clone();
        // let legacy_fields = NodeType::LEGACY_EMBEDDABLE_NODE_FIELDS;
        // let legacy_nodes_rule = format_args!("legacy_nodes[id, embedding] := ( {legacy_embeddings_script_rhs} ), !is_null(embedding)");
        let script = format!(
            r#"
 legacy_nodes[id, embedding] := (  *function {{id, name, span, tracking_hash, embedding  @ 'NOW' }} or  *const {{id, name, span,
tracking_hash @ 'NOW' }} or  *enum {{id, name, span, tracking_hash, embedding  @ 'NOW' }} or  *macro
{{id, name, span, tracking_hash, embedding  @ 'NOW' }} or  *module {{id, name, span, tracking_hash,
embedding  @ 'NOW' }} or  *static {{id, name, span, tracking_hash, embedding  @ 'NOW' }} or  *struct
{{id, name, span, tracking_hash, embedding  @ 'NOW' }} or  *trait {{id, name, span, tracking_hash,
embedding  @ 'NOW' }} or  *type_alias {{id, name, span, tracking_hash, embedding  @ 'NOW' }} or *union
{{id, name, span, tracking_hash, embedding  @ 'NOW' }} ), !is_null(embedding)

        ?[id, embedding] := legacy_nodes[id, embedding]
        "#
        );
        script
    }
    #[test]
    fn test_load_db() -> Result<(), Error> {
        use crate::create_index_primary;
        use crate::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt};
        use ploke_test_utils::workspace_root;
        use tracing::{error, Level};

        // init_tracing_once(HNSW_TARGET, Level::TRACE);
        info!(target: HNSW_TARGET, "starting test: test_load_db");

        let db = Database::init_with_schema()?;
        // use default active embedding set of sentence-transformers...

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = db.with_active_set(|set| set.clone())?;

        // clear hnsw relations before importing from backup (as specified by cozo docs)
        // TODO: needs citation for cozo docs
        let hnsw_rel = embedding_set.hnsw_rel_name();
        let script_clear_hnsw = format!("::hnsw drop {hnsw_rel}");

        let remove_hnsw_result = db.raw_query_mut(&script_clear_hnsw);
        info!(target: HNSW_TARGET, ?remove_hnsw_result);
        if let Err(err) = remove_hnsw_result {
            let err_msg = err.to_string();
            let expected_missing = err_msg.contains("Cannot find requested stored relation");
            let expected_read_only = err_msg.contains("Cannot remove index in read-only mode");
            if expected_missing || expected_read_only {
                info!(
                    target: HNSW_TARGET,
                    "skipping drop failure, expected condition: {err_msg}"
                );
            } else {
                return Err(err.into());
            }
        }

        let mut target_file = workspace_root();
        target_file.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
        let prior_rels_vec = db
            .relations_vec()
            .inspect_err(|e| error!(target: HNSW_TARGET, "{e:#?}"))?;
        let prior_rels_string = format!("{prior_rels_vec:#?}");
        info!(target: HNSW_TARGET, %prior_rels_string);
        db.import_from_backup(&target_file, &prior_rels_vec)
            .expect("the database to be imported without errors");
        // .map_err(DbError::from)
        // .map_err(ploke_error::Error::from)?;

        // TODO: put the embeddings setup into create_index_primary, then make create_index_primary
        // a method on the database (if possible, weird with Arc)
        create_index_primary(&db)?;

        let embedding_set = ploke_core::embeddings::EmbeddingSet::default();

        // let r1 = db.create_embedding_set_relation();
        // tracing::info!(create_embedding_set_relation = ?r1);
        // r1?;

        let r1 = db.ensure_embedding_set_relation();
        tracing::info!(create_embedding_set_relation = ?r1);
        r1?;

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = db.with_active_set(|set| set.clone())?;
        let r2 = db.ensure_embedding_relation(&active_embedding_set);
        tracing::info!(ensure_embedding_relation = ?r2);
        r2?;

        let r3 = db.create_embedding_index(&active_embedding_set);
        tracing::info!(create_embedding_index = ?r3);
        r3?;

        Ok(())
    }

    fn helper_load_db() -> Result<Database, Error> {
        use crate::create_index_primary;
        use crate::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt};
        use ploke_test_utils::workspace_root;

        let db = Database::init_with_schema()?;
        let mut target_file = workspace_root();
        target_file.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
        let prior_rels_vec = db.relations_vec()?;
        db.import_from_backup(&target_file, &prior_rels_vec)
            .map_err(DbError::from)
            .map_err(ploke_error::Error::from)?;

        // TODO: put the embeddings setup into create_index_primary, then make create_index_primary
        // a method on the database (if possible, weird with Arc)
        create_index_primary(&db)?;
        Ok(db)
    }

    #[test]
    fn test_something() -> Result<(), Error> {
        // init_tracing_once("cozo-script", tracing::Level::INFO);
        let db = helper_load_db()?;

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = db.with_active_set(|set| set.clone())?;

        let rel = NodeType::Const.relation_str();
        let embed_rel = embedding_set.hnsw_rel_name();
        let run_query_with_count = |script: &str| -> Result<(), Error> {
            info!(%script);
            let r = db.raw_query(script)?;
            let count = r.rows.len();
            let headers = &r.headers;
            info!(%count, ?headers);
            Ok(())
        };

        let basic_fields = "id, name, hash, span";
        let basic_batch = format!(
            "batch[{basic_fields}] := *{rel} {{ id, name, tracking_hash: hash, span @ 'NOW'  }}"
        );
        let last_line = format!("?[{basic_fields}] := batch[{basic_fields}]");

        let basic_query = format!(
            r#"
    {basic_batch}
    {last_line}
    "#
        );
        // try basic query
        run_query_with_count(&basic_query)?;

        let ancestor_script = format!(
            r#"
 parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}}

 ancestor[desc, asc] := parent_of[desc, asc]
 ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]
    "#
        );
        let is_root_module_script = format!(
            r#"
 is_root_module[id] := *module{{id @ 'NOW' }}, *file_mod {{owner_id: id @ 'NOW'}}
    "#
        );
        // let basic_with_ancestor = format!();

        let batch_with_ancestor = format!(
            r#"
 batch[id, name, file_path, file_hash, hash, span, namespace, distance] :=
*{rel}{{ id, name, tracking_hash: hash, span @ 'NOW' }},
ancestor[id, mod_id],
is_root_module[mod_id],
*module{{id: mod_id, tracking_hash: file_hash @ 'NOW'}},
*file_mod {{ owner_id: mod_id, file_path, namespace @ 'NOW'}}
    "#
        );

        let basic_batch_with_anc = format!(
            r#"
    {ancestor_script}
    {is_root_module_script}
    {basic_batch}
    {last_line}
    "#
        );
        run_query_with_count(&basic_batch_with_anc)?;

        Ok(())
    }
}
