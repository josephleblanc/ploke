#![allow(clippy::result_large_err)]
#![cfg(feature = "multi_embedding_db")]

use std::{collections::BTreeMap, ops::Deref};

use cozo::{DataValue, NamedRows, Num, ScriptMutability, UuidWrapper};
use itertools::Itertools;
use ploke_core::embeddings::{
    EmbRelName, EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingSetId,
    EmbeddingShape, HnswRelName,
};
use ploke_core::EmbeddingData;
use ploke_error::Error as PlokeError;
use syn_parser::utils::LogStyle as _;
use tracing::info;
use uuid::Uuid;

use crate::{
    database::HNSW_SUFFIX,
    multi_embedding::schema::{CozoEmbeddingSetExt, EmbeddingSetExt, EmbeddingVector},
    query::builder::EMBEDDABLE_NODES_NOW,
    Database, DbError, EmbedDataVerbose, NodeType, QueryResult, TypedEmbedData,
};

/// Trait used to extend the database with embeddings-aware methods
pub trait EmbeddingExt {
    /// Counts the code primary node code items that have not yet been embedded.
    ///
    /// Queries the underlying database to determine which nodes have been embedded or not by the
    /// presence/absence of an associated vector for the given embedding set (identified by the
    /// embedding_set_id).
    ///
    /// In the case of nodes being processed into vector embeddings, this function can be used to
    /// determine which nodes have not yet been embedded, while some may already have been
    /// embedded.
    ///
    /// Useful in `ploke-embed` when processing vector embeddings.
    fn count_pending_embeddings(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError>;

    /// Helper function to specifically count unembedded non-files.
    ///
    /// Similar to `count_pending_embeddings`, it is useful when processing vector embeddings in
    /// `ploke-embed`.
    fn count_unembedded_nonfiles(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError>;

    /// Helper function to specifically count unembedded files.
    ///
    // Similar to `count_pending_embeddings`, it is useful when processing vector embeddings in
    // `ploke-embed`
    fn count_unembedded_files(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError>;

    /// Checks for the presence of the embedding info for a given embedding set.
    fn is_embedding_present(&self, embedding_set_id: &EmbeddingSet) -> Result<bool, DbError>;

    fn is_embedding_id_present(&self, embedding_set_id: EmbeddingSetId) -> Result<bool, DbError>;

    fn get_embedding_rel_name(
        &self,
        embedding_set_id: EmbeddingSetId,
    ) -> Result<EmbRelName, DbError>;

    /// Checks whether or not a given vector relation name is already registered in the database.
    ///
    /// Useful when checking that the database has registered the relation for and embedding model.
    ///
    /// e.g. If the relation is present in the database, then the database is ready to register
    /// the embedding vectors for code snippets.
    fn is_relation_registered(&self, relation_name: &EmbRelName) -> Result<bool, DbError>;

    /// Checks whether the 'embedding_set' relation has been created in the database.
    ///
    /// Note that this is separate from using a `put` to add a new row for that relation - this is
    /// just to check that the spec for the row itself has been registered in the database, and
    /// this should have been done during setup for the database.
    ///
    /// Useful when we want to ensure that `embedding_set` exists to prevent the failure of other
    /// database methods.
    fn is_embedding_set_registered(&self) -> Result<bool, DbError>;

    fn script_pending_nodes_rhs(&self, embedding_set: &EmbeddingSet) -> String;

    /// These nodes are also filtered in the function `embeddable_nodes_now` (cfg-gated) behind the
    /// lazy static for the rhs script, `EMBEDDABLE_NODES_NOW`, and form the set of nodes that we
    /// use for embeddings.
    fn script_embeddable_nodes_now_rhs() -> &'static str;

    /// The script used to get the common nodes in `get_common_nodes`
    fn script_get_common_nodes(&self) -> Result<String, DbError>;

    /// Get the rows for nodes that we often use for embeddings or other similar functions, which
    /// are grouped in this function by the common fields they share, namely:
    ///
    /// id, name, tracking_hash, span
    ///
    /// These nodes are also filtered in the function `embeddable_nodes_now` (cfg-gated) behind the
    /// lazy static for the rhs script, `EMBEDDABLE_NODES_NOW`, and form the set of nodes that we
    /// use for embeddings.
    ///
    /// See also `script_embeddable_nodes_now_rhs` above
    fn get_common_nodes(&self) -> Result<QueryResult, PlokeError>;

    /// Retrieves ordered embedding data for the given ids constrained to a specific embedding set.
    fn get_nodes_ordered_for_set(
        &self,
        nodes: Vec<Uuid>,
        embedding_set: &EmbeddingSet,
    ) -> Result<Vec<EmbeddingData>, PlokeError>;

    // TODO: After we get this to work, try removing the async (I don't know if it really wins us
    // anything here)
    async fn update_embeddings_batch(
        &self,
        updates: Vec<(Uuid, Vec<f64>)>,
        embedding_set: &EmbeddingSet,
    ) -> Result<(), DbError>;

    fn get_unembed_rel(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: usize,
        embedding_set: EmbeddingSet,
    ) -> Result<TypedEmbedData, PlokeError>;

    fn ensure_default_relation_set(&self) -> Result<(), PlokeError>;

    fn replace_embedding_set_relation(&self, embedding_set: EmbeddingSet)
        -> Result<(), PlokeError>;

    fn create_embedding_set_relation(&self) -> Result<(), PlokeError>;

    fn put_embedding_set(&self, embedding_set: &EmbeddingSet) -> Result<(), PlokeError>;

    fn is_hnsw_relation_registered(&self, relation_name: &HnswRelName) -> Result<bool, DbError>;

    fn create_vector_embedding_relation(&self, embedding_set: &EmbeddingSet) -> Result<(), PlokeError>;
}

impl EmbeddingExt for cozo::Db<cozo::MemStorage> {
    fn count_pending_embeddings(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError> {
        let pending_nodes_rhs = self.script_pending_nodes_rhs(embedding_set);

        let query = format!("?[count(id)] := {}", pending_nodes_rhs);
        tracing::debug!(target: "cozo-script", ?query);

        let result = self
            .run_script(
                &query,
                Default::default(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        Database::into_usize(result)
    }

    fn count_unembedded_nonfiles(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError> {
        let nodes = self.count_pending_embeddings(embedding_set)?;
        let files = self.count_unembedded_files(embedding_set)?;
        let count = nodes.checked_sub(files).ok_or_else(|| {
            DbError::QueryExecution(
                "Invariant violated: more unembedded files than unembedded nodes".into(),
            )
        })?;
        Ok(count)
    }

    fn count_unembedded_files(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError> {
        let query = format!(
            r#"
unembedded_file_mod[id] := *module{{id}}, *file_mod{{owner_id: id}}, 
not *{embed_rel}{{node_id: id}}

?[count(id)] := unembedded_file_mod[id]
"#,
            embed_rel = embedding_set.rel_name.to_string().replace('-', "_")
        );

        let result = self
            .run_script(
                &query,
                Default::default(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        Database::into_usize(result)
    }

    fn is_embedding_present(&self, embedding_set: &EmbeddingSet) -> Result<bool, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "embedding_set_id".into(),
            cozo::DataValue::from(embedding_set.hash_id.into_inner() as i64),
        );

        let get_rel_name_script =
            "?[rel_name] := *embedding_set{id: $embedding_set_id, rel_name @ 'NOW'}";
        let result = self
            .run_script(
                get_rel_name_script,
                params,
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        let is_present = result
            .rows
            .first()
            .and_then(|row| row.first())
            .map(|v| v.get_str().is_some())
            .unwrap_or(false);
        Ok(is_present)
    }

    fn is_embedding_id_present(&self, embedding_set_id: EmbeddingSetId) -> Result<bool, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "embedding_set_id".into(),
            cozo::DataValue::from(embedding_set_id.into_inner() as i64),
        );

        let get_rel_name_script =
            "?[rel_name] := *embedding_set{id: $embedding_set_id, rel_name @ 'NOW'}";
        let result = self
            .run_script(
                get_rel_name_script,
                params,
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        let is_present = result
            .rows
            .first()
            .and_then(|row| row.first())
            .map(|v| v.get_str().is_some())
            .unwrap_or(false);
        Ok(is_present)
    }

    fn get_embedding_rel_name(
        &self,
        embedding_set_id: EmbeddingSetId,
    ) -> Result<EmbRelName, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "embedding_set_id".into(),
            cozo::DataValue::from(embedding_set_id.into_inner() as i64),
        );

        let get_rel_name_script =
            "?[rel_name] := *embedding_set{id: $embedding_set_id, rel_name @ 'NOW'}";
        let result = self
            .run_script(
                get_rel_name_script,
                params,
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        result
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|v| v.get_str())
            .map(EmbRelName::new_from_str)
            .ok_or(DbError::NotFound)
    }

    fn is_embedding_set_registered(&self) -> Result<bool, DbError> {
        let rows = self
            .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::EmbeddingSetScriptFailure {
                action: "relations_lookup",
                details: err.to_string(),
            })?;
        let is_found = rows.rows.iter().any(|row| {
            row.iter().any(|value| {
                value
                    .get_str()
                    .map(|name| name == "embedding_set")
                    .unwrap_or(false)
            })
        });
        Ok(is_found)
    }

    fn is_relation_registered(&self, relation_name: &EmbRelName) -> Result<bool, DbError> {
        let rows = self
            .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|err| DbError::EmbeddingScriptFailure {
                action: "relations_lookup",
                relation: relation_name.clone(),
                details: err.to_string(),
            })?;
        let is_found = rows.rows.iter().any(|row| {
            row.iter().any(|value| {
                value
                    .get_str()
                    .map(|name| name == relation_name.as_ref())
                    .unwrap_or(false)
            })
        });
        Ok(is_found)
    }

    fn script_pending_nodes_rhs(&self, embedding_set: &EmbeddingSet) -> String {
        let rel_name = &embedding_set.rel_name;
        NodeType::primary_nodes()
            .iter()
            .map(|node_type| {
                format!(
                    "(*{node_rel}{{id}}, not *{embed_rel}{{node_id: id}})",
                    node_rel = node_type.relation_str(),
                    embed_rel = &rel_name
                )
            })
            .join(" or ")
    }

    fn script_embeddable_nodes_now_rhs() -> &'static str {
        EMBEDDABLE_NODES_NOW.as_str()
    }

    fn script_get_common_nodes(&self) -> Result<String, DbError> {
        let embeddable_nodes_rule = format!(
            "embeddable[id, name, tracking_hash, span] := {}",
            Self::script_embeddable_nodes_now_rhs()
        );
        let script = format!(
            r#"
        {embeddable_nodes_rule}

        ?[id, name, tracking_hash, span] := embeddable[id, name, tracking_hash, span]
        "#
        );
        Ok(script)
    }

    fn get_common_nodes(&self) -> Result<QueryResult, PlokeError> {
        use tracing::debug;

        let script = self.script_get_common_nodes()?;

        debug!(target: "common_nodes", ?script);
        self.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable)
            .map(QueryResult::from)
            .map_err(DbError::from)
            .map_err(PlokeError::from)
    }

    fn get_nodes_ordered_for_set(
        &self,
        nodes: Vec<Uuid>,
        embedding_set: &EmbeddingSet,
    ) -> Result<Vec<EmbeddingData>, PlokeError> {
        if nodes.is_empty() {
            return Ok(Vec::new());
        }

        let embed_rel = embedding_set.rel_name.as_ref().replace('-', "_");
        let ancestor_rules = r#"
parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}

ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]
"#;
        let has_embedding_rule = NodeType::primary_nodes()
            .iter()
            .map(|ty| {
                format!(
                    r#"
has_embedding[id, name, hash, span] :=
    *{rel}{{id, name, tracking_hash: hash, span @ 'NOW'}},
    *{embed_rel}{{ node_id: id, embedding_set_id: set_id @ 'NOW' }},
    set_id = $embedding_set_id
"#,
                    rel = ty.relation_str(),
                    embed_rel = embed_rel
                )
            })
            .join("\n");

        let script = format!(
            r#"
target_ids[id, ordering] <- $data

{ancestor_rules}

{has_embedding_rule}

batch[id, name, file_path, file_hash, hash, span, namespace, ordering] :=
    has_embedding[id, name, hash, span],
    ancestor[id, mod_id],
    *module{{id: mod_id, tracking_hash: file_hash @ 'NOW'}},
    *file_mod {{ owner_id: mod_id, file_path, namespace @ 'NOW'}},
    target_ids[id, ordering]

?[id, name, file_path, file_hash, hash, span, namespace, ordering] :=
    batch[id, name, file_path, file_hash, hash, span, namespace, ordering]
:sort ordering
"#,
            ancestor_rules = ancestor_rules,
            has_embedding_rule = has_embedding_rule
        );

        let ids_data: Vec<DataValue> = nodes
            .into_iter()
            .enumerate()
            .map(|(i, id)| {
                DataValue::List(vec![
                    DataValue::Uuid(UuidWrapper(id)),
                    DataValue::from(i as i64),
                ])
            })
            .collect();

        let mut params = BTreeMap::new();
        params.insert("data".into(), DataValue::List(ids_data));
        params.insert(
            "embedding_set_id".into(),
            DataValue::from(embedding_set.hash_id().into_inner() as i64),
        );

        let query_result = self
            .run_script(&script, params, ScriptMutability::Immutable)
            .map(QueryResult::from)
            .map_err(DbError::from)
            .map_err(PlokeError::from)?;

        query_result.to_embedding_nodes()
    }

    async fn update_embeddings_batch(
        &self,
        updates: Vec<(Uuid, Vec<f64>)>,
        embedding_set: &EmbeddingSet,
    ) -> Result<(), DbError> {
        if updates.is_empty() {
            return Ok(());
        }

        // Convert updates into cozo-friendly values after validation.
        let mut updates_data = Vec::with_capacity(updates.len());
        for (node_id, vector) in updates {
            let vector = embedding_set.new_vector_with_node(node_id, vector);
            vector.validate_embedding_vec()?;
            updates_data.push(vector.into_cozo_datavalue());
        }

        let params = BTreeMap::from([("updates".to_string(), DataValue::List(updates_data))]);
        let script = embedding_set.script_put_vector_with_param_batch();

        self.run_script(&script, params, ScriptMutability::Mutable)
            .map_err(DbError::from)?;

        Ok(())
    }

    fn get_unembed_rel(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: usize,
        embedding_set: EmbeddingSet,
    ) -> Result<TypedEmbedData, PlokeError> {
        let node_relation_name = node_type.relation_str();
        let embed_rel = embedding_set.relation_name();
        let script = format!(
            r#"
    parent_of[child, parent] := *syntax_edge{{ source_id: parent, target_id: child, relation_kind: "Contains" }}

    ancestor[desc, asc] := parent_of[desc, asc]
    ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

    needs_embedding[id, name, hash, span] := *{node_relation_name} {{
            id, 
            name, 
            tracking_hash: hash, 
            span, 
         }}, not *{embed_rel} {{node_id: id}}

    is_root_module[id] := *module{{ id }}, *file_mod {{owner_id: id}}

    batch[id, name, file_path, file_hash, hash, span, namespace] := 
        needs_embedding[id, name, hash, span],
        ancestor[id, mod_id],
        is_root_module[mod_id],
        *module{{ id: mod_id, tracking_hash: file_hash }},
        *file_mod {{  owner_id: mod_id, file_path, namespace  }},

    ?[id, name, file_path, file_hash, hash, span, namespace] := 
        batch[id, name, file_path, file_hash, hash, span, namespace]
        :sort id
        :limit $limit
     "#
        );

        // Create parameters map
        let mut params = BTreeMap::new();
        params.insert("limit".into(), DataValue::from(limit as i64));
        params.insert("cursor".into(), DataValue::from(cursor as i64));

        let query_result = self
            .run_script(&script, params, cozo::ScriptMutability::Immutable)
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        let count_more_flat = query_result.rows.iter().flatten().count();
        let count_less_flat = query_result.rows.len();
        tracing::info!("== more_flat: {count_more_flat} | less_flat: {count_less_flat} ==");
        let more_flat_row = query_result.rows.iter().flatten().next();
        let less_flat_row = query_result.rows.first();
        tracing::info!("== \nmore_flat: {more_flat_row:?}\nless_flat: {less_flat_row:?}\n ==");
        let mut v = QueryResult::from(query_result).to_embedding_nodes()?;
        v.truncate(limit.min(count_less_flat));
        let ty_embed = TypedEmbedData { v, ty: node_type };
        Ok(ty_embed)
    }

    fn ensure_default_relation_set(&self) -> Result<(), PlokeError> {

        let embedding_set = EmbeddingSet::default();


        // Check that the relation "embedding_set" is registered
        if !self.is_embedding_set_registered()? {
            self.create_embedding_set_relation()?;
        }

        // Check that the given relation for the spectific "embedding_set" relation has already
        // been "put" into the database
        if !self.is_embedding_present(&embedding_set)? {
            self.put_embedding_set(&embedding_set)?;
            tracing::info!("{}: put default embedding set", "Db".log_step());
        }

        // Check that the given relation already has a specific relation_name in the database.
        let vector_rel_name = embedding_set.relation_name();
        if !self.is_relation_registered(vector_rel_name)? {
            EmbeddingVector::script_create_from_set(&embedding_set);
        }
        Ok(())
    }
    fn create_vector_embedding_relation(&self, embedding_set: &EmbeddingSet) -> Result<(), PlokeError> {
        let script_create_vector_rel = EmbeddingVector::script_create_from_set(embedding_set);
        self.run_script(&script_create_vector_rel, BTreeMap::new(), ScriptMutability::Mutable)
            .map_err(DbError::from)?;
        Ok(())
    }

    fn replace_embedding_set_relation(
        &self,
        embedding_set: EmbeddingSet,
    ) -> Result<(), PlokeError> {
        let replace_rel_script = EmbeddingSet::script_replace();
        let relation_name = EmbeddingSet::embedding_set_relation_name();
        let db_result = self
            .run_script(
                replace_rel_script,
                BTreeMap::new(),
                ScriptMutability::Mutable,
            )
            .map_err(DbError::from)?;
        Ok(())
    }

    #[tracing::instrument]
    fn create_embedding_set_relation(&self) -> Result<(), PlokeError> {
        let create_rel_script = EmbeddingSet::script_create();
        let db_result = self
            .run_script(
                create_rel_script,
                BTreeMap::new(),
                ScriptMutability::Mutable,
            )
            .map_err(DbError::from)?;
        Ok(())
    }

    fn put_embedding_set(&self, embedding_set: &EmbeddingSet) -> Result<(), PlokeError> {
        let script_put = embedding_set.script_put();
        let db_result = self
            .run_script(&script_put, BTreeMap::new(), ScriptMutability::Mutable)
            .map_err(DbError::from)?;
        // tracing::info!(put_embedding_set = ?db_result.rows);
        Ok(())
    }

    fn is_hnsw_relation_registered(&self, relation_name: &HnswRelName) -> Result<bool, DbError> {
        let rows = self
            .run_script(
                "::hnsw relations",
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .map_err(|err| DbError::HnswEmbeddingScriptFailure {
                action: "hnsw relations lookup",
                relation: relation_name.clone(),
                details: err.to_string(),
            })?;
        let is_found = rows.rows.iter().any(|row| {
            row.iter().any(|value| {
                value
                    .get_str()
                    .map(|name| name == relation_name.as_ref())
                    .unwrap_or(false)
            })
        });
        Ok(is_found)
    }
}

/// Trait used to extend the database with embeddings-aware methods
impl EmbeddingExt for Database {
    fn count_pending_embeddings(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError> {
        self.deref().count_pending_embeddings(embedding_set_id)
    }

    fn count_unembedded_nonfiles(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError> {
        self.deref().count_unembedded_nonfiles(embedding_set_id)
    }

    fn count_unembedded_files(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError> {
        self.deref().count_unembedded_files(embedding_set_id)
    }

    fn is_embedding_present(&self, embedding_set_id: &EmbeddingSet) -> Result<bool, DbError> {
        self.deref().is_embedding_present(embedding_set_id)
    }

    fn is_embedding_id_present(&self, embedding_set_id: EmbeddingSetId) -> Result<bool, DbError> {
        self.deref().is_embedding_id_present(embedding_set_id)
    }

    fn get_embedding_rel_name(
        &self,
        embedding_set_id: EmbeddingSetId,
    ) -> Result<EmbRelName, DbError> {
        self.deref().get_embedding_rel_name(embedding_set_id)
    }

    fn is_relation_registered(&self, relation_name: &EmbRelName) -> Result<bool, DbError> {
        self.deref().is_relation_registered(relation_name)
    }

    fn script_pending_nodes_rhs(&self, embedding_set: &EmbeddingSet) -> String {
        self.deref().script_pending_nodes_rhs(embedding_set)
    }

    fn script_embeddable_nodes_now_rhs() -> &'static str {
        // WARNING: This is not really good practice, and is just copied from the above
        // implementation for cozo::Db.
        // Revisit this once we are ready to do a better refactor for wrapper databases
        EMBEDDABLE_NODES_NOW.as_str()
    }

    fn script_get_common_nodes(&self) -> Result<String, DbError> {
        self.deref().script_get_common_nodes()
    }

    fn get_common_nodes(&self) -> Result<QueryResult, PlokeError> {
        self.deref().get_common_nodes()
    }

    fn get_nodes_ordered_for_set(
        &self,
        nodes: Vec<Uuid>,
        embedding_set: &EmbeddingSet,
    ) -> Result<Vec<EmbeddingData>, PlokeError> {
        self.deref().get_nodes_ordered_for_set(nodes, embedding_set)
    }

    // TODO: After we get this to work, try removing the async (I don't know if it really wins us
    // anything here)
    async fn update_embeddings_batch(
        &self,
        updates: Vec<(Uuid, Vec<f64>)>,
        embedding_set: &EmbeddingSet,
    ) -> Result<(), DbError> {
        self.deref()
            .update_embeddings_batch(updates, embedding_set)
            .await
    }

    fn get_unembed_rel(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: usize,
        embedding_set: EmbeddingSet,
    ) -> Result<TypedEmbedData, PlokeError> {
        self.deref()
            .get_unembed_rel(node_type, limit, cursor, self.active_embedding_set.clone())
    }

    fn ensure_default_relation_set(&self) -> Result<(), PlokeError> {
        self.deref().ensure_default_relation_set()
    }

    fn replace_embedding_set_relation(
        &self,
        embedding_set: EmbeddingSet,
    ) -> Result<(), PlokeError> {
        self.deref().replace_embedding_set_relation(embedding_set)
    }

    fn create_embedding_set_relation(&self) -> Result<(), PlokeError> {
        self.deref().create_embedding_set_relation()
    }

    fn put_embedding_set(&self, embedding_set: &EmbeddingSet) -> Result<(), PlokeError> {
        self.deref().put_embedding_set(embedding_set)
    }

    fn is_hnsw_relation_registered(&self, relation_name: &HnswRelName) -> Result<bool, DbError> {
        self.deref().is_hnsw_relation_registered(relation_name)
    }

    fn is_embedding_set_registered(&self) -> Result<bool, DbError> {
        self.deref().is_embedding_set_registered()
    }

    fn create_vector_embedding_relation(&self, embedding_set: &EmbeddingSet) -> Result<(), PlokeError> {
        self.deref().create_vector_embedding_relation(embedding_set)
    }
}

pub fn into_usize(named_rows: QueryResult) -> Result<usize, DbError> {
    named_rows
        .rows
        .first()
        .and_then(|row| row.first())
        .and_then(|v| v.get_int())
        .inspect(|v| tracing::info!("the value in first row, first cell is: {:?}", v))
        .map(|n| n as usize)
        .ok_or(DbError::NotFound)
}

/// Test helper to print all the relations found in the database.
///
/// Useful when you don't find the expected relation in the database, or to help manually inspect
/// to just poke around and get more familiar with database structure.
pub fn print_all_relations(db: &cozo::Db<cozo::MemStorage>) -> Result<(), DbError> {
    let rows = db
        .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
        .map_err(DbError::from)?;
    for row in rows.rows.iter() {
        for value in row.iter() {
            let found_relation = value.get_str().unwrap_or("non-string row");
            println!("{found_relation}");
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use lazy_static::lazy_static;
    use std::collections::BTreeMap;
    use syn_parser::utils::LogStyle;

    use super::*;
    use cozo::{MemStorage, ScriptMutability};
    use ploke_core::embeddings::{
        EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
    };
    use tracing::Level;

    use crate::{
        multi_embedding::{
            db_ext::EmbeddingExt,
            hnsw_ext::HnswExt,
            schema::{EmbeddingSetExt, EmbeddingVector},
        },
        Database,
    };

    lazy_static! {
        /// Convenience/speed struct for a test db that can be used in tests that do not require
        /// mutable access to the underlying database.
        ///
        /// This helps speed up testing a bit, since we don't need to re-parse the target fixture
        /// each time for tests that only need immutable access anyways.
        ///
        /// Note that cozo::Db implements Arc::clone under the hood, so cloning this static ref is
        /// cheap.
        static ref TEST_DB_IMMUTABLE: cozo::Db<cozo::MemStorage> =
            ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
                .expect("database must be set up correctly");
    }
    /// Helper macro to reduce boilderplate for printing logging statements when printing
    /// cozoscript in tests, see `run_script!` macro.
    macro_rules! log_script {
        ($label:expr, $name:expr, $script:expr) => {
            info!(
                target: "cozo-script",
                step = %$label.log_step(),
                name = %$name.log_name(),
                script = %$script.log_magenta()
            );
        };
    }

    /// Arguments:
    /// $db:expr, $mutability:expr, $label:expr, $name:expr, $script:expr
    ///
    /// Example usage:
    /// ```
    /// let empty_db = cozo::Db::new(cozo::MemStorage::default()).map_err(DbError::from)?;
    /// empty_db.initialize().map_err(DbError::from)?;
    ///
    /// let create_rel_script = r#"
    ///     :create example_relation { field_one: Int, field_two: String }
    /// "#;
    ///
    /// // run_script! uses log_script! to log the tracing call
    /// let db_result = run_script!(empty_db, cozo::ScriptMutability::Mutable, "Testing Script:", "create
    /// example_relation relation", create_rel_script)?;.
    /// ```
    macro_rules! run_script {
        ($db:expr, $mutability:expr, $label:expr, $name:expr, $script:expr) => {{
            let script = $script;
            log_script!($label, $name, script);
            $db.run_script(script, BTreeMap::new(), $mutability)
                .map_err(DbError::from)
        }};
    }

    macro_rules! run_script_params {
        ($db:expr, $mutability:expr, $label:expr, $name:expr, $script:expr, $params:expr) => {{
            let script = $script;
            log_script!($label, $name, script);
            $db.run_script(script, $params, $mutability)
                .map_err(DbError::from)
        }};
    }

    fn setup_db() -> Result<cozo::Db<MemStorage>, ploke_error::Error> {
        ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
    }
    use crate::multi_embedding::schema::CozoEmbeddingSetExt;

    use ploke_error::Error as PlokeError;

    fn eprint_relations(fixture_db: cozo::Db<MemStorage>) -> Result<(), PlokeError> {
        let script = "::relations";
        let list_relations = fixture_db
            .run_script(script, BTreeMap::new(), ScriptMutability::Mutable)
            .map_err(DbError::from)?;
        for rel in list_relations {
            eprintln!("{rel:?}");
        }
        Ok(())
    }

    #[test]
    fn test_hnsw_rel_name_script() -> Result<(), PlokeError> {
        let empty_db = cozo::Db::new(MemStorage::default()).map_err(DbError::from)?;
        empty_db.initialize().map_err(DbError::from)?;

        let hnsw_rel_name = HnswRelName::new_from_str(
            "emb_sentence_transformers_slash_all_MiniLM_L6_v2_384:hnsw_idx",
        );
        // empty_db.is_hnsw_relation_registered(&hnsw_rel_name)?;

        let fixture_db = setup_db()?;
        eprint_relations(fixture_db.clone())?;

        fixture_db.ensure_default_relation_set()?;
        eprint_relations(fixture_db)?;

        Ok(())
    }

    #[test]
    fn test_put_vector_with_params_batch() -> Result<(), PlokeError> {
        // ploke_test_utils::init_test_tracing_with_target("cozo-script", Level::INFO);
        let empty_db = cozo::Db::new(MemStorage::default()).map_err(DbError::from)?;
        empty_db.initialize().map_err(DbError::from)?;

        let n_vectors = 100;
        const VECTOR_LENGTH: usize = 64;
        let embedding_set = EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("test_provider"),
            EmbeddingModelId::new_from_str("test_model"),
            EmbeddingShape::new_dims_default(VECTOR_LENGTH as u32),
        );

        let test_vector: Vec<f64> = Vec::from([0.2; VECTOR_LENGTH]);
        let mut test_vecs: Vec<(Uuid, Vec<f64>)> = Vec::new();
        let test_namespace = Uuid::nil();
        for n in 0..100 {
            test_vecs.push((Uuid::new_v5(&test_namespace, &[n; 8]), test_vector.clone()));
        }

        // create underlying embedding set relation (has metadata for embedding model)
        let script_create_embedding_set = EmbeddingSet::script_create();
        run_script!(
            empty_db,
            cozo::ScriptMutability::Mutable,
            "Testing Script:",
            "create embedding_set",
            script_create_embedding_set
        )?;

        // put the embedding set data
        let script_put_embedding_set = embedding_set.script_put();
        run_script!(
            empty_db,
            cozo::ScriptMutability::Mutable,
            "Testing Script:",
            "put_embedding_set",
            &script_put_embedding_set
        )?;

        // create relation for the given vector embedding
        let script_create_vector_embedding =
            EmbeddingVector::script_create_from_set(&embedding_set);
        run_script!(
            empty_db,
            cozo::ScriptMutability::Mutable,
            "Testing Script:",
            "create vector embedding rel",
            &script_create_vector_embedding
        )?;

        let updates_data = test_vecs
            .into_iter()
            .map(|(node_id, vector)| embedding_set.new_vector_with_node(node_id, vector))
            .map(|ev| ev.validate_embedding_vec().map(|_| ev))
            .map_ok(EmbeddingVector::into_cozo_datavalue)
            .try_collect()?;

        let params = BTreeMap::from([("updates".to_string(), DataValue::List(updates_data))]);
        let put_vectors_batch = embedding_set.script_put_vector_with_param_batch();
        // put the embedding set data
        run_script_params!(
            empty_db,
            cozo::ScriptMutability::Mutable,
            "Testing Script:",
            "put vector embeddings batch with params",
            &put_vectors_batch,
            params
        )?;

        let get_vector_rows = embedding_set.script_get_vector_rows();
        let named_rows = run_script!(
            empty_db,
            cozo::ScriptMutability::Immutable,
            "Testing Script:",
            "get vector rows",
            &get_vector_rows
        )?;

        let counted_vectors = named_rows.into_iter().count();
        assert_eq!(n_vectors, counted_vectors);

        Ok(())
    }

    #[test]
    fn get_nodes_ordered_for_set_preserves_order() -> Result<(), PlokeError> {
        let db = TEST_DB_IMMUTABLE.clone();
        let embedding_set = EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("local"),
            EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2"),
            EmbeddingShape::new_dims_default(384),
        );
        db.ensure_embedding_relation(&embedding_set)?;

        // Take a couple of node ids from the fixture functions and insert vectors for our embedding set.
        let rows = run_script!(
            db,
            ScriptMutability::Immutable,
            "Testing Script:",
            "fetch function ids",
            "?[id] := *function { id @ 'NOW' } :limit 2"
        )?;
        let ids: Vec<Uuid> = rows
            .rows
            .iter()
            .filter_map(|row| row.first())
            .filter_map(|v| {
                if let DataValue::Uuid(UuidWrapper(u)) = v {
                    Some(*u)
                } else {
                    None
                }
            })
            .collect();
        assert!(!ids.is_empty(), "fixture must contain function ids");

        let updates = ids
            .iter()
            .map(|id| embedding_set.new_vector_with_node(*id, vec![0.1; 384]))
            .map(EmbeddingVector::into_cozo_datavalue)
            .collect_vec();
        let mut params = BTreeMap::new();
        params.insert("updates".to_string(), DataValue::List(updates));
        let put_vectors = embedding_set.script_put_vector_with_param_batch();
        run_script_params!(
            db,
            ScriptMutability::Mutable,
            "Testing Script:",
            "insert vectors for ordered nodes",
            &put_vectors,
            params
        )?;

        let ordered = db.get_nodes_ordered_for_set(ids.clone(), &embedding_set)?;
        let returned_ids: Vec<Uuid> = ordered.iter().map(|e| e.id).collect();
        assert_eq!(returned_ids, ids);

        Ok(())
    }

    #[test]
    fn hnsw_index_filters_embedding_set() -> Result<(), PlokeError> {
        let db = cozo::Db::new(MemStorage::default()).map_err(DbError::from)?;
        db.initialize().map_err(DbError::from)?;

        let set_a = EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("prov_a"),
            EmbeddingModelId::new_from_str("test_model"),
            EmbeddingShape::new_dims_default(3),
        );
        let set_b = EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("prov_b"),
            EmbeddingModelId::new_from_str("test_model"),
            EmbeddingShape::new_dims_default(3),
        );

        db.ensure_embedding_relation(&set_a)?;

        let create_fn_rel = ":create function { id: Uuid, at: Validity => name: String }";
        db.run_script(
            create_fn_rel,
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )
        .map_err(DbError::from)?;

        let id_a1 = Uuid::new_v4();
        let id_a2 = Uuid::new_v4();
        let id_b = Uuid::new_v4();
        let insert_functions = format!(
            "?[id, at, name] <- [[ '{id_a1}', 'ASSERT', 'a1' ], [ '{id_a2}', 'ASSERT', 'a2' ], [ '{id_b}', 'ASSERT', 'b' ]]\n:put function {{ id, at => name }}",
        );
        db.run_script(
            &insert_functions,
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )
        .map_err(DbError::from)?;

        let updates = vec![
            set_a
                .new_vector_with_node(id_a1, vec![1.0, 0.0, 0.0])
                .into_cozo_datavalue(),
            set_a
                .new_vector_with_node(id_a2, vec![0.0, 1.0, 0.0])
                .into_cozo_datavalue(),
            set_b
                .new_vector_with_node(id_b, vec![0.0, 0.0, 1.0])
                .into_cozo_datavalue(),
        ];
        let mut params = BTreeMap::new();
        params.insert("updates".to_string(), DataValue::List(updates));
        let put_vectors = set_a.script_put_vector_with_param_batch();
        db.run_script(&put_vectors, params, cozo::ScriptMutability::Mutable)
            .map_err(DbError::from)?;

        db.create_embedding_index(&set_a)?;

        let neighbors = db.hnsw_neighbors_for_type(NodeType::Function, &set_a, 2, 10)?;
        assert!(
            !neighbors.is_empty(),
            "expected neighbors to be returned for set_a"
        );
        assert!(
            neighbors.iter().all(|(id, _, _)| *id != id_b),
            "hnsw results should ignore other embedding_set_id rows"
        );

        Ok(())
    }

    #[test]
    fn test_get_common_nodes() -> Result<(), PlokeError> {
        let db = TEST_DB_IMMUTABLE.clone();

        let common_nodes_result = db.get_common_nodes()?;
        info!(
            "{} {}\n{}",
            "Testing Script:".log_step(),
            "create embedding_set relation".log_name(),
            format_args!("Running script:\n{}", db.script_get_common_nodes()?)
        );
        let script_get_common_nodes = db.script_get_common_nodes()?;
        let db_result = db
            .run_script(
                &script_get_common_nodes,
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .map_err(DbError::from)?;
        eprintln!("db_result.rows: {:#?}", db_result.rows);
        let count_common_nodes = common_nodes_result.rows.len();

        assert_eq!(
            136, count_common_nodes,
            r#"
Should match the number of expected nodes (more means the syn_parser has likely become more
sensitive/accurate, less is likely bad)\nTotal count was: {count_common_nodes}"#
        );
        Ok(())
    }

    #[test]
    fn test_slash_model_invalid_cozoscript() -> Result<(), PlokeError> {
        // ploke_test_utils::init_test_tracing_with_target("cozo-script", Level::INFO);
        let empty_db = cozo::Db::new(cozo::MemStorage::default()).map_err(DbError::from)?;
        empty_db.initialize().map_err(DbError::from)?;

        // This should give us an invalid script:
        //
        //:create emb_test_model/with_slash_123 {
        // node_id: Uuid,
        // at: Validity,
        // =>
        // vector: <F32; 123>,
        // embedding_set_id: Int
        // }
        let embedding_set = EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("test_provider"),
            EmbeddingModelId::new_from_str("test_model/with_slash"),
            EmbeddingShape::new_dims_default(123),
        );

        // manually rewriting the cozoscript here because now we sanitize it out in the
        // constructor for embedding_set
        let relation_name = "test_model/with_slash";
        let create_rel_script_bad = format!(
            r#":create {relation_name} {{
    node_id: Uuid,
    at: Validity,
    =>
    vector: <F32; {dims}>,
    embedding_set_id: Int }}"#,
            dims = embedding_set.dims()
        );
        let db_result = run_script!(
            empty_db,
            ScriptMutability::Mutable,
            "Testing Script:",
            relation_name,
            &create_rel_script_bad
        );
        assert!(
            db_result.is_err(),
            "Expect an EmbeddingModelId with a slash '/' in the name to error"
        );

        // now testing the sanitizer is catching the forward-slash `/`
        let create_rel_script_good = EmbeddingVector::script_create_from_set(&embedding_set);

        let relation_name = embedding_set.relation_name();
        let db_result = run_script!(
            empty_db,
            ScriptMutability::Mutable,
            "Testing Script:",
            relation_name,
            &create_rel_script_good
        );
        assert!(
            db_result.is_ok(),
            "Expect an EmbeddingModelId with a slash '/' in the name to error"
        );

        Ok(())
    }

    #[test]
    fn test_slash_provider_field_valid_cozoscript() -> Result<(), PlokeError> {
        // ploke_test_utils::init_test_tracing_with_target("cozo-script", Level::INFO);
        let empty_db = cozo::Db::new(cozo::MemStorage::default()).map_err(DbError::from)?;
        empty_db.initialize().map_err(DbError::from)?;

        let embedding_set = EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("test_provider/with_slash_in_name"),
            EmbeddingModelId::new_from_str("test_model"),
            EmbeddingShape::new_dims_default(123),
        );
        let create_rel_script = EmbeddingSet::script_create();
        let relation_name = EmbeddingSet::embedding_set_relation_name();

        let db_result = run_script!(
            empty_db,
            ScriptMutability::Mutable,
            "Testing Script:",
            relation_name,
            &create_rel_script
        );
        assert!(db_result.is_ok(), "Should be valid create script");

        let script_put = embedding_set.script_put();
        let db_result = run_script!(
            empty_db,
            ScriptMutability::Mutable,
            "Testing Script:",
            relation_name,
            &script_put
        );
        assert!(
            db_result.is_ok(),
            "Having a slash '/' in a cozo String field should be fine."
        );

        Ok(())
    }

    #[test]
    fn multi_pending_embeddings_count_basic() -> Result<(), ploke_error::Error> {
        ploke_test_utils::init_test_tracing_with_target("cozo-script", Level::DEBUG);
        // let db = Database::new(ploke_test_utils::setup_db_full("fixture_nodes")?);
        // let db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")?;

        // setup function takes care of:
        // - create embedding_set relation
        let db = setup_db()?;

        let common_nodes_result = db.get_common_nodes()?;
        let common_nodes_count = common_nodes_result.rows.len();

        info!(?common_nodes_count);

        let embedding_set = EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("test_provider/with_slash_in_name"),
            EmbeddingModelId::new_from_str("test_model"),
            EmbeddingShape::new_dims_default(123),
        );
        let create_rel_script = EmbeddingSet::script_create();

        let put_rel_script = embedding_set.script_put();

        info!(
            "{} {}\n{}",
            "Testing Script:".log_step(),
            "put embedding_set relation".log_name(),
            format_args!("Running script:\n{put_rel_script}")
        );
        let db_result = db
            .run_script(&put_rel_script, BTreeMap::new(), ScriptMutability::Mutable)
            .map_err(DbError::from)?;
        info!(?db_result.rows);

        let vector_relation_name = embedding_set.relation_name();
        let create_vector_script = EmbeddingVector::script_create_from_set(&embedding_set);
        let step_msg = format!("create {} relation", embedding_set.relation_name());
        run_script!(
            db,
            cozo::ScriptMutability::Mutable,
            "Testing Script:",
            &step_msg,
            &create_vector_script
        )?;
        // info!(
        //     "{} {}\n{}\n{}",
        //     "Testing Script:".log_step(),
        //     "create vector_script for relation".log_name(),
        //     format_args!(
        //         "embedding relation name: {}",
        //         vector_relation_name.log_name()
        //     ),
        //     format_args!("Running script:\n{create_vector_script}")
        // );
        // let db_result = db
        //     .run_script(
        //         &create_vector_script,
        //         BTreeMap::new(),
        //         ScriptMutability::Mutable,
        //     )
        //     .map_err(DbError::from)?;
        info!(create_vector_script_result = ?db_result.rows);

        // check that the relation for the embedding vector has been registered in the database. If
        // true then the database is prepared to receive vector embedding `put` commands.
        let relation_name = embedding_set.relation_name();
        info!(
            "{}\n{}",
            "Listing Relations".log_step(),
            "Using ::relations to list relations and filter for expected embedding relation name"
                .log_name(),
        );
        let is_relation_registered =
            db.is_relation_registered(relation_name).inspect_err(|_e| {
                if let Err(e) = print_all_relations(&db) {
                    tracing::error!("Issue printing all relations: {e}");
                }
            })?;
        info!(?is_relation_registered);
        assert!(
            is_relation_registered,
            "Expect the relation to be registered after running create_rel_script"
        );

        info!(?db_result.rows);
        let count_all_embeddable =
            <cozo::Db<MemStorage> as EmbeddingExt>::count_pending_embeddings(&db, &embedding_set)?;
        info!(target: "cozo-script",
            "{}: {}",
            "count_pending_embeddings".log_step(), "Total nodes found without embeddings using new method:\n\t{count}");
        assert_eq!(
            136, count_all_embeddable,
            "Expect all nodes present (flaky, add better count later)"
        );

        let count_unembedded_files =
            <cozo::Db<MemStorage> as EmbeddingExt>::count_unembedded_files(&db, &embedding_set)?;
        info!(target: "cozo-script",
            "{}: {}",
            "count_unembedded_files".log_step(), "Total nodes found without embeddings using new method:\n\t{count}");
        assert_eq!(10, count_unembedded_files, "Expect all nodes present");

        let count_unembedded_nonfiles =
            <cozo::Db<MemStorage> as EmbeddingExt>::count_unembedded_nonfiles(&db, &embedding_set)?;
        info!(target: "cozo-script",
            "{}: {}",
            "count_unembedded_nonfiles".log_step(), "Total nodes found without embeddings using new method:\n\t{count}");
        assert_eq!(126, count_unembedded_nonfiles, "Expect all nodes present");

        assert!(
            count_all_embeddable == (count_unembedded_nonfiles + count_unembedded_files),
            "totals should line up"
        );

        Ok(())
    }
}
