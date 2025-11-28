#![cfg(feature = "multi_embedding_db")]

use std::{collections::BTreeMap, ops::Deref};

use cozo::{DataValue, NamedRows, ScriptMutability};
use itertools::Itertools;
use ploke_core::embeddings::{EmbRelName, EmbeddingSet, EmbeddingSetId};
use ploke_error::Error as PlokeError;
use tracing::info;
use uuid::Uuid;

use crate::{multi_embedding::schema::{EmbeddingSetExt, EmbeddingVector}, query::builder::EMBEDDABLE_NODES_NOW, Database, DbError, NodeType, QueryResult};

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

    // TODO: After we get this to work, try removing the async (I don't know if it really wins us
    // anything here)
async fn update_embeddings_batch(&self, updates: Vec<(Uuid, Vec<f64>)>, embedding_set: &EmbeddingSet) -> Result<(), DbError>;
}

impl EmbeddingExt for cozo::Db<cozo::MemStorage> {
    fn count_pending_embeddings(&self, embedding_set: &EmbeddingSet) -> Result<usize, DbError> {
        let pending_nodes_rhs = self.script_pending_nodes_rhs(embedding_set);

        let query = format!("?[count(id)] := {}", pending_nodes_rhs);

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
!*{embed_rel}{{node_id: id}}

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

        result
            .rows
            .first()
            .and_then(|row| row.first())
            .map(|v| v.get_str().is_some())
            .ok_or(DbError::NotFound)
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

        result
            .rows
            .first()
            .and_then(|row| row.first())
            .map(|v| v.get_str().is_some())
            .ok_or(DbError::NotFound)
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

    async fn update_embeddings_batch(&self, updates: Vec<(Uuid, Vec<f64>)>, embedding_set: &EmbeddingSet) -> Result<(), DbError> {
        // NOTE: original function returns on empty input, but we may not want to do that, seems
        // error-prone
        
        // ensure no vectors are empty
        let updates_data = updates.into_iter().map(|(node_id, vector)| 
            embedding_set.new_vector_with_node(node_id, vector)
        )
            .map(|ev| ev.validate_embedding_vec().map(|_| ev))
            .map_ok(EmbeddingVector::into_cozo_datavalue)
            .try_collect()?;

        let params = BTreeMap::from([( "updates".to_string(), DataValue::List(updates_data) )]);
        Ok(())
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
            $db.run_script(script, BTreeMap::new(), $mutability).map_err(DbError::from)
        }};
    }

    macro_rules! run_script_params {
        ($db:expr, $mutability:expr, $label:expr, $name:expr, $script:expr, $params:expr) => {{
            let script = $script;
            log_script!($label, $name, script);
            $db.run_script(script, $params, $mutability).map_err(DbError::from)
        }};
    }

    fn setup_db() -> Result<cozo::Db<MemStorage>, ploke_error::Error> {
        ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")
    }
    use crate::multi_embedding::schema::CozoEmbeddingSetExt;

    use ploke_error::Error as PlokeError;

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
            test_vecs.push(( Uuid::new_v5(&test_namespace, &[ n; 8 ]), test_vector.clone() ));
        }

        // create underlying embedding set relation (has metadata for embedding model)
        let script_create_embedding_set = EmbeddingSet::script_create();
        run_script!(empty_db, cozo::ScriptMutability::Mutable, "Testing Script:", "create embedding_set", 
            script_create_embedding_set)?;

        // put the embedding set data
        let script_put_embedding_set = embedding_set.script_put();
        run_script!(empty_db, cozo::ScriptMutability::Mutable, "Testing Script:", "put_embedding_set", 
            &script_put_embedding_set)?;

        // create relation for the given vector embedding
        let script_create_vector_embedding = EmbeddingVector::script_create_from_set(&embedding_set);
        run_script!(empty_db, cozo::ScriptMutability::Mutable, "Testing Script:", "create vector embedding rel", 
            &script_create_vector_embedding)?;

        let updates_data = test_vecs.into_iter().map(|(node_id, vector)| 
            embedding_set.new_vector_with_node(node_id, vector)
        )
            .map(|ev| ev.validate_embedding_vec().map(|_| ev))
            .map_ok(EmbeddingVector::into_cozo_datavalue)
            .try_collect()?;

        let params = BTreeMap::from([( "updates".to_string(), DataValue::List(updates_data) )]);
        let put_vectors_batch = embedding_set.script_put_vector_with_param_batch();
        // put the embedding set data
        run_script_params!(empty_db, cozo::ScriptMutability::Mutable, "Testing Script:", "put vector embeddings batch with params", 
            &put_vectors_batch, params)?;

        let get_vector_rows = embedding_set.script_get_vector_rows();
        let named_rows = run_script!(empty_db, cozo::ScriptMutability::Immutable, "Testing Script:", "get vector rows", 
            &get_vector_rows)?;

        let counted_vectors = named_rows.into_iter().count();
        assert_eq!(n_vectors, counted_vectors);

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
        let create_rel_script = EmbeddingVector::script_create_from_set(&embedding_set);

        let relation_name = embedding_set.relation_name();
        let db_result = run_script!(empty_db, ScriptMutability::Mutable, "Testing Script:", relation_name, &create_rel_script);
        assert!(db_result.is_err(), "Expect an EmbeddingModelId with a slash '/' in the name to error");

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

        let db_result = run_script!(empty_db, ScriptMutability::Mutable, "Testing Script:", relation_name, &create_rel_script);
        assert!(db_result.is_ok(), "Should be valid create script");

        let script_put = embedding_set.script_put();
        let db_result = run_script!(empty_db, ScriptMutability::Mutable, "Testing Script:", relation_name, &script_put);
        assert!(db_result.is_ok(), "Having a slash '/' in a cozo String field should be fine.");

        Ok(())
    }

    #[test]
    fn multi_pending_embeddings_count_basic() -> Result<(), ploke_error::Error> {
        // ploke_test_utils::init_test_tracing(Level::DEBUG);
        // let db = Database::new(ploke_test_utils::setup_db_full("fixture_nodes")?);
        // let db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")?;
        let db = setup_db()?;
        //
        let common_nodes_result = db.get_common_nodes()?;
        let common_nodes_count = common_nodes_result.rows.len();

        info!(?common_nodes_count);

        let embedding_set = EmbeddingSet::new(
            EmbeddingProviderSlug::new_from_str("test_provider/with_slash_in_name"),
            EmbeddingModelId::new_from_str("test_model"),
            EmbeddingShape::new_dims_default(123),
        );
        let create_rel_script = EmbeddingSet::script_create();

        info!(
            "{} {}\n{}",
            "Testing Script:".log_step(),
            "create embedding_set relation".log_name(),
            "Running script:\n{create_rel_script}"
        );
        let db_result = db
            .run_script(
                create_rel_script,
                BTreeMap::new(),
                ScriptMutability::Mutable,
            )
            .map_err(DbError::from)?;
        info!(?db_result.rows);

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
        let step_msg = format!( "create {} relation", embedding_set.relation_name() );
        run_script!(db, cozo::ScriptMutability::Mutable, "Testing Script:", &step_msg, &create_vector_script)?;
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
        let count =
            <cozo::Db<MemStorage> as EmbeddingExt>::count_pending_embeddings(&db, &embedding_set)?;
        info!("Total nodes found without embeddings using new method:\n\t{count}");
        assert_eq!(
            136, count,
            "Expect all nodes present (flaky, add better count later)"
        );

        panic!();

        Ok(())
    }
}


/// Trait used to extend the database with embeddings-aware methods
impl EmbeddingExt for Database {
    fn count_pending_embeddings(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError> {
        self.deref().count_pending_embeddings(embedding_set_id)
    }

    /// Helper function to specifically count unembedded non-files.
    ///
    /// Similar to `count_pending_embeddings`, it is useful when processing vector embeddings in
    /// `ploke-embed`.
    fn count_unembedded_nonfiles(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError> {
        self.deref().count_unembedded_nonfiles(embedding_set_id)
    }

    /// Helper function to specifically count unembedded files.
    ///
    // Similar to `count_pending_embeddings`, it is useful when processing vector embeddings in
    // `ploke-embed`
    fn count_unembedded_files(&self, embedding_set_id: &EmbeddingSet) -> Result<usize, DbError> {
        self.deref().count_unembedded_files(embedding_set_id)
    }

    /// Checks for the presence of the embedding info for a given embedding set.
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

    /// Checks whether or not a given vector relation name is already registered in the database.
    ///
    /// Useful when checking that the database has registered the relation for and embedding model.
    ///
    /// e.g. If the relation is present in the database, then the database is ready to register
    /// the embedding vectors for code snippets.
    fn is_relation_registered(&self, relation_name: &EmbRelName) -> Result<bool, DbError> {
        self.deref().is_relation_registered(relation_name)
    }

    fn script_pending_nodes_rhs(&self, embedding_set: &EmbeddingSet) -> String {
        self.deref().script_pending_nodes_rhs(embedding_set)
    }

    /// These nodes are also filtered in the function `embeddable_nodes_now` (cfg-gated) behind the
    /// lazy static for the rhs script, `EMBEDDABLE_NODES_NOW`, and form the set of nodes that we
    /// use for embeddings.
    fn script_embeddable_nodes_now_rhs() -> &'static str {
        self.deref().script_embeddable_nodes_now_rhs()
    }

    /// The script used to get the common nodes in `get_common_nodes`
    fn script_get_common_nodes(&self) -> Result<String, DbError> {
        self.deref().script_get_common_nodes()
    }

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
    fn get_common_nodes(&self) -> Result<QueryResult, PlokeError> {
        self.deref().get_common_nodes()
    }

    // TODO: After we get this to work, try removing the async (I don't know if it really wins us
    // anything here)
    async fn update_embeddings_batch(&self, updates: Vec<(Uuid, Vec<f64>)>, embedding_set: &EmbeddingSet) -> Result<(), DbError> {
        self.deref().update_embeddings_batch(updates, embedding_set).await
    }
}
