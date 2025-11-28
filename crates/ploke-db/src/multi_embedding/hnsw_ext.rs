use std::{collections::BTreeMap, ops::Deref as _};

use cozo::{DataValue, Num, ScriptMutability, UuidWrapper};
use itertools::Itertools;
use ploke_core::embeddings::EmbeddingSet;
use uuid::Uuid;

use crate::{
    database::HNSW_SUFFIX,
    multi_embedding::{
        db_ext::EmbeddingExt,
        schema::{EmbeddingSetExt, EmbeddingVector},
    },
    Database, DbError, EmbedDataVerbose, NodeType, QueryResult, TypedEmbedData,
};

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

    fn vec_to_param(vector_query: Vec<f32>) -> DataValue {
        let as_list = vector_query
            .into_iter()
            .map(|fl| {
                if (fl as f64).is_subnormal() {
                    0.0
                } else {
                    fl as f64
                }
            })
            .map(|fl| DataValue::Num(Num::Float(fl)))
            .collect_vec();
        DataValue::List(as_list)
    }

    fn is_hnsw_index_registered(
        &self,
        embedding_set: &EmbeddingSet,
    ) -> Result<bool, ploke_error::Error>;

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

        let hnsw_index_rel_name = format!("{rel_name}{hnsw_suffix}");

        if self.is_relation_registered(&embedding_set.rel_name)? {
            return Ok(());
        }
        tracing::info!(target: "cozo-script",
            "create new relation: '{rel_name}{hnsw_suffix}'\nrel_name = '{rel_name}'\nhnsw_suffix = '{hnsw_suffix}'",
        );
        let script = format!(
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
            rel_name = rel_name,
            hnsw_suffix = HNSW_SUFFIX,
            dim = embedding_set.dims(),
        );

        self.run_script(&script, BTreeMap::new(), ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|err| DbError::Cozo(err.to_string()))
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

        let result = match self.run_script(&script, params, ScriptMutability::Immutable) {
            Ok(rows) => Ok(rows),
            Err(err) => {
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
        }?;

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

    fn is_hnsw_index_registered(
        &self,
        embedding_set: &EmbeddingSet,
    ) -> Result<bool, ploke_error::Error> {
        let vec_rel_name = embedding_set.rel_name();
        let script_check_indices = "::indices {vec_rel_name}";
        let db_ret = self
            .run_script(
                script_check_indices,
                BTreeMap::new(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(DbError::from)?;
        for item in db_ret.rows {
            eprintln!("{item:?}");
        }

        Ok(false)
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

    fn is_hnsw_index_registered(
        &self,
        embedding_set: &EmbeddingSet,
    ) -> Result<bool, ploke_error::Error> {
        self.deref().is_hnsw_index_registered(embedding_set)
    }

    fn create_index_warn(&self, enbedding_set: &EmbeddingSet) -> Result<(), ploke_error::Error> {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Once;

    use super::*;
    use cozo::NamedRows;
    use ploke_error::Error;
    use syn_parser::utils::LogStyle;
    use tracing::info;

    use crate::{
        multi_embedding::{schema::CozoEmbeddingSetExt, test_utils::{eprint_relations, setup_db, setup_empty_db}},
        query::builder::EMBEDDABLE_NODES_NOW_LEGACY_RHS, { run_script_params, log_script },
    };

    static TEST_TRACING: Once = Once::new();
    fn init_tracing_once(target: &'static str, level: tracing::Level) {
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

    #[test]
    // WARNING: This test cannot pass, because it relies on the old version of the embeddings
    // stored in the database, which are not loaded here. Under the cfg flag combinations it is
    // impossible to load the old schema.
    //
    // NOTE: Keeping this just so we don't try to do the same thing again in the future.
    fn test_is_hnsw_index_registered() -> Result<(), Error> {
        // init_tracing_once("cozo-script", tracing::Level::INFO);
        let cozo_db = setup_db()?;
        let db = Database::new(cozo_db);

        let embedding_set = EmbeddingSet::default();
        let db_res = db.is_hnsw_index_registered(&embedding_set);
        eprint_relations(&db)?;

        let relation = embedding_set.relation_name().as_ref();
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
        let rows = db
            .run_script(
                &legacy_node_script,
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .map_err(DbError::from)?;

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
            let set_id = DataValue::Num( Num::Int( embedding_set.hash_id().into_inner() as i64 ) );
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

        panic!();

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
}
