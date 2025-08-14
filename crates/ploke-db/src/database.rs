use std::collections::BTreeMap;
use std::collections::HashMap;
use std::ops::Deref;
use std::path::Path;

use crate::bm25_index::DocMeta;
use crate::bm25_index::TOKENIZER_VERSION;
use crate::error::DbError;
use crate::query::builder::EMBEDDABLE_NODES;
use crate::query::builder::EMBEDDABLE_NODES_NOW;
use crate::NodeType;
use crate::QueryResult;
use cozo::DataValue;
use cozo::Db;
use cozo::MemStorage;
use cozo::NamedRows;
use cozo::UuidWrapper;
use itertools::concat;
use itertools::Itertools;
use ploke_core::EmbeddingData;
use ploke_core::FileData;
use ploke_core::TrackingHash;
use ploke_transform::schema::meta::Bm25MetaSchema;
use rayon::iter::ParallelBridge;
use rayon::iter::ParallelIterator;
use serde::Deserialize;
use tracing::instrument;
use uuid::Uuid;

pub const HNSW_SUFFIX: &str = ":hnsw_idx";

/// Main database connection and query interface
#[derive(Debug)]
pub struct Database {
    db: Db<MemStorage>,
}

#[derive(Deserialize)]
struct CrateRow {
    name: String,
    id: String, // the UUID already arrives as a string
}

impl std::ops::Deref for Database {
    type Target = Db<MemStorage>;

    fn deref(&self) -> &Self::Target {
        &self.db
    }
}

/// Safely converts a Cozo DataValue to a Uuid.
pub fn to_uuid(val: &DataValue) -> Result<uuid::Uuid, DbError> {
    if let DataValue::Uuid(UuidWrapper(uuid)) = val {
        Ok(*uuid)
    } else {
        Err(DbError::Cozo(format!("Expected Uuid, found {:?}", val)))
    }
}

/// Safely converts a Cozo DataValue to a String.
pub fn to_string(val: &DataValue) -> Result<String, DbError> {
    if let DataValue::Str(s) = val {
        Ok(s.to_string())
    } else {
        Err(DbError::Cozo(format!("Expected String, found {:?}", val)))
    }
}

/// Safely converts a Cozo DataValue to a usize.
pub fn to_usize(val: &DataValue) -> Result<usize, DbError> {
    if let DataValue::Num(cozo::Num::Int(n)) = val {
        // Cozo stores numbers that can be i64, u64, or f64. Safest to try as i64 for span.
        usize::try_from(*n).map_err(|e| {
            DbError::Cozo(format!(
                "Could not convert Num::Int to i64 for usize: {:?}, original error {}",
                n, e
            ))
        })
    } else {
        Err(DbError::Cozo(format!("Expected Number, found {:?}", val)))
    }
}

#[derive(Debug, Clone)]
pub struct TypedEmbedData {
    pub v: Vec<EmbeddingData>,
    pub ty: NodeType,
}

impl Deref for TypedEmbedData {
    type Target = Vec<EmbeddingData>;

    fn deref(&self) -> &Self::Target {
        &self.v
    }
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    module_name: String,
    module_id: Uuid,
    crate_name: String,
    namespace: Uuid,
    file_path: String,
}

impl Database {
    const ANCESTOR_RULES: &str = r#"
    parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains"}

    ancestor[desc, asc] := parent_of[desc, asc]
    ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]
    is_root_module[id] := *module{id}, *file_mod {owner_id: id}
    "#;
    /// Create new database connection
    pub fn new(db: Db<MemStorage>) -> Self {
        Self { db }
    }
    pub fn new_init() -> Result<Self, ploke_error::Error> {
        let db = Db::new(MemStorage::default()).map_err(|e| DbError::Cozo(e.to_string()))?;
        db.initialize().map_err(|e| DbError::Cozo(e.to_string()))?;
        Ok(Self { db })
    }

    pub fn init_with_schema() -> Result<Self, ploke_error::Error> {
        let db = Db::new(MemStorage::default()).map_err(|e| DbError::Cozo(e.to_string()))?;
        db.initialize().map_err(|e| DbError::Cozo(e.to_string()))?;

        // Create the schema
        ploke_transform::schema::create_schema_all(&db)?;

        Ok(Self { db })
    }

    /// Gets all the file data in the same namespace as the crate name given as argument.
    /// This is useful when you want to compare which files have changed since the database was
    /// last updated.
    pub fn get_crate_files(&self, crate_name: &str) -> Result<Vec<FileData>, ploke_error::Error> {
        let script = format!(
            "{} \"{}\"",
            r#"?[id, tracking_hash, namespace, file_path] := 
    *module { id, tracking_hash @ 'NOW' },
    *file_mod { file_path, namespace, owner_id: id @ 'NOW' },
    *crate_context { name: crate_name, namespace @ 'NOW' },
    crate_name = "#,
            crate_name
        );
        let ret = self.raw_query(&script)?;
        tracing::info!("get_crate_files output: {:#?}", ret);
        ret.try_into_file_data()
    }

    pub fn retract_embedded_files(
        &self,
        file_mod: Uuid,
        ty: NodeType,
    ) -> Result<QueryResult, ploke_error::Error> {
        let rel_name = ty.relation_str();
        let keys = ty.keys().join(", ");
        let vals = ty.vals().join(", ");
        let script = format!(
            "parent_of[child, parent] := *syntax_edge{{
                source_id: parent, 
                target_id: child, 
                relation_kind: \"Contains\"
            }}

            ancestor[desc, asc] := parent_of[desc, asc]
            ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

            to_retract[{keys}, at, {vals}] := *{rel_name} {{ {keys}, {vals}  @ 'NOW'}},
                *file_mod {{ owner_id: file_mod }},
                ancestor[id, file_mod],
                file_mod = \"{file_mod}\",
                !is_null(embedding),
                at = 'RETRACT'

            ?[{keys}, at, {vals}] := to_retract[{keys}, at, {vals}]
                :put {rel_name} {{ {keys}, at => {vals} }}
                :returning
            "
        );
        self.raw_query_mut(&script)
            .inspect_err(|_| {
                tracing::error!("using script:\n {}", script);
            })
            .map_err(ploke_error::Error::from)
    }

    /// Clears all user-defined relations from the database.
    ///
    /// This method removes all relations that were created by the application,
    /// excluding system relations that contain ":". It's useful for resetting
    /// the database state during testing or when reprocessing data.
    ///
    /// # Examples
    ///
    /// ```
    /// # tokio_test::block_on(async {
    ///
    /// use ploke_db::Database;
    /// use cozo::ScriptMutability;
    ///
    /// // Initialize database with schema
    /// let db = Database::init_with_schema().unwrap();
    ///
    /// // Get initial relations
    /// let initial_relations = db.run_script("::relations", Default::default(), ScriptMutability::Immutable).unwrap();
    /// let initial_count = initial_relations.rows.len();
    /// assert!(initial_count > 0, "Should have some relations after schema creation");
    ///
    /// // Clear all user relations
    /// db.clear_relations().await.unwrap();
    ///
    /// // Verify no user relations remain
    /// let remaining_relations = db.run_script("::relations", Default::default(), ScriptMutability::Immutable).unwrap();
    /// let user_relations: Vec<_> = remaining_relations.rows
    ///     .into_iter()
    ///     .filter(|row| {
    ///         if let cozo::DataValue::Str(name) = &row[0] {
    ///             !name.starts_with("::")
    ///         } else {
    ///             false
    ///         }
    ///     })
    ///     .collect();
    ///
    /// assert_eq!(user_relations.len(), 0, "Should have no user relations after clearing");
    /// # })
    /// ```
    /// - JL, Reviewed and edited Jul 30, 2025
    pub async fn clear_relations(&self) -> Result<(), ploke_error::Error> {
        let rels = self
            .db
            .run_script(
                "::relations",
                BTreeMap::new(),
                cozo::ScriptMutability::Mutable,
            )
            .map_err(DbError::from)?
            .rows
            .into_iter()
            .map(|r| r[0].to_string())
            .filter(|n| !n.contains(":"))
            .join(", "); // keep only user relations

        let mut script = String::from("::remove ");
        script.extend(rels.split("\""));
        self.db
            .run_script(&script, BTreeMap::new(), cozo::ScriptMutability::Mutable)
            .map_err(DbError::from)?;
        Ok(())
    }

    /// Clears all HNSW indices from the database.
    ///
    /// This method removes all HNSW (Hierarchical Navigable Small World) indices that were created
    /// for embedding similarity search. These indices have names ending with ":hnsw_idx", e.g.
    /// `functions:hnsw_idx` and are separate from regular database relations. Unlike regular
    /// relations which can be removed with "::remove", indices must be dropped using the "::index
    /// drop" command.
    ///
    /// The choice of naming for the HNSW indices as "hnsw_idx" is arbitrary, and could have been
    /// named "whatever_noxd", but is named "hnsw_idx" for consistency
    ///
    /// This is useful when you need to reset the embedding indices, such as during testing or
    /// when rebuilding indices with new parameters.
    ///
    /// It is also used when clearing all relations in the database in preparation for a database
    /// restore from backup, as cozo requires the database must be empty before a restore.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// # tokio_test::block_on(async {
    ///     use ploke_db::Database;
    ///     use cozo::ScriptMutability;
    ///     
    ///     // Initialize database with schema
    ///     let mut db = Database::init_with_schema().expect("Could not init database with schema");
    ///     
    ///     // Create some HNSW indices for testing
    ///     // WARN: This doesn't actually work because we don't have anything to index yet, we
    ///     // need a better test that uses a lazily loaded database that already contains embeddings.
    ///     db.index_embeddings(ploke_db::NodeType::Function, 384).await
    ///         .expect("Error indexing embeddings");
    ///     
    ///     // Count initial relations (including indices)
    ///     let initial_relations = db.run_script("::relations", Default::default(), ScriptMutability::Immutable).unwrap();
    ///     let hnsw_indices: Vec<_> = initial_relations.rows
    ///         .into_iter()
    ///         .filter(|row| {
    ///             if let cozo::DataValue::Str(name) = &row[0] {
    ///                 name.ends_with(":hnsw_idx")
    ///             } else {
    ///                 false
    ///             }
    ///         })
    ///         .collect();
    ///     
    ///     // Should have some HNSW indices after creating them
    ///     assert!(hnsw_indices.len() > 0, "Should have HNSW indices after creation");
    ///     
    ///     // Clear all HNSW indices
    ///     db.clear_hnsw_idx().await.expect("Error clearing hnsw indicies from database");
    ///     
    ///     // Verify no HNSW indices remain
    ///     let remaining_relations = db.run_script("::relations", Default::default(), ScriptMutability::Immutable).unwrap();
    ///     let remaining_hnsw: Vec<_> = remaining_relations.rows
    ///         .into_iter()
    ///         .filter(|row| {
    ///             if let cozo::DataValue::Str(name) = &row[0] {
    ///                 name.ends_with(":hnsw_idx")
    ///             } else {
    ///                 false
    ///             }
    ///         })
    ///         .collect();
    ///     
    ///     assert_eq!(remaining_hnsw.len(), 0, "Should have no HNSW indices after clearing");
    /// # })
    /// ```
    /// - JL, Reviewed and edited Jul 30, 2025
    pub async fn clear_hnsw_idx(&self) -> Result<(), ploke_error::Error> {
        let rels = self
            .db
            .run_script(
                "::relations",
                BTreeMap::new(),
                cozo::ScriptMutability::Mutable,
            )
            .map_err(DbError::from)?
            .rows
            .into_iter()
            .map(|r| r[0].to_string())
            .filter(|n| n.ends_with(":hnsw_idx"));

        for index in rels {
            let mut script = String::from("::index drop ");
            script.extend(index.chars().filter(|c| *c == '\"'));
            self.db
                .run_script(&script, BTreeMap::new(), cozo::ScriptMutability::Mutable)
                .map_err(DbError::from)?;
        }
        Ok(())
    }

    /// Counts the total number of relations in the database.
    ///
    /// This method returns the count of all relations in the database, including
    /// both system relations (containing ":") and user-defined relations.
    ///
    /// # Examples
    ///
    /// ```
    /// # tokio_test::block_on(async {
    /// use ploke_db::Database;
    /// use cozo::ScriptMutability;
    ///
    /// // Initialize database with schema
    /// let db = Database::init_with_schema().unwrap();
    ///
    /// // Count initial relations
    /// let initial_count = db.count_relations().await.unwrap();
    /// assert!(initial_count > 0, "Should have some relations after schema creation");
    ///
    /// // Verify count matches ::relations output
    /// let relations_result = db.run_script("::relations", Default::default(), ScriptMutability::Immutable).unwrap();
    /// assert_eq!(initial_count, relations_result.rows.len());
    /// # })
    /// ```
    /// - JL, Reviewed and edited Jul 30, 2025
    pub async fn count_relations(&self) -> Result<usize, ploke_error::Error> {
        let rel_count = self
            .db
            .run_script_read_only("::relations", BTreeMap::new())
            .map_err(DbError::from)?
            .rows
            .len();
        Ok(rel_count)
    }
    // NOTE: the goal of the following todo items is to be able to provide quick and easy calls to
    // the database to present more simple and increasingly granular information to the user. We
    // want it to be easy and intuitive to explore the data of their code.
    // - For example, we might show something simple like, "X relations created in the code graph",
    // where the "X" is in bold with a colored background, and maybe pulses or something, or
    // otherwise invites the user to click on it (maybe have a grey-text "click me" pointing to the
    // text or something that is only included once or until the user clicks on it for the first
    // time).
    // - When the user clicks on the number of relations created, it drops down (running the query
    // in the background) with each of the relations and the numbers for each.
    //  - A similar similar color/text style is used on each of these, numbers, and when they click
    //  on those... you get the idea. Think Matrioshka
    //
    // TODO: Add a way to count the number of hnsw indices loaded.

    // TODO: Add a way to return the number of items in a given relation.

    // TODO: Add a way to see the last time a relation was changed (given that we implement time
    // travel)

    // TODO: Add a way to return all the members of a given relation.

    /// Execute a raw CozoScript query
    pub fn raw_query(&self, script: &str) -> Result<QueryResult, DbError> {
        let result = self
            .db
            .run_script(
                script,
                std::collections::BTreeMap::new(),
                cozo::ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        Ok(QueryResult::from(result))
    }

    pub fn raw_query_mut(&self, script: &str) -> Result<QueryResult, DbError> {
        let result = self
            .db
            .run_script(
                script,
                std::collections::BTreeMap::new(),
                cozo::ScriptMutability::Mutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        Ok(QueryResult::from(result))
    }

    pub async fn mock_get_nodes_for_embedding(&self) -> Result<Vec<EmbeddingData>, DbError> {
        // TODO: The CozoScript query needs to be validated and might require adjustments
        // based on the final schema. For now, we'll return mock data.
        let mock_nodes = vec![
            // Example node. In a real scenario, this would come from the database.
            // EmbeddingData {
            //     id: Uuid::new_v4(),
            //     path: PathBuf::from("/path/to/your/file.rs"),
            //     content_hash: 123456789,
            //     start_byte: 100,
            //     end_byte: 500,
            // },
        ];
        Ok(mock_nodes)
    }
    pub async fn create_new_backup(path: impl AsRef<Path>) -> Result<Database, ploke_error::Error> {
        let new_db = cozo::new_cozo_mem().map_err(DbError::from)?;
        new_db.restore_backup(&path).map_err(DbError::from)?;
        Ok(Self { db: new_db })
    }

    pub fn iter_relations(&self) -> Result<impl IntoIterator<Item = String>, ploke_error::Error> {
        let output = self.raw_query("::relations")?;
        Ok(output.rows.into_iter().filter_map(|r| {
            r.first()
                .into_iter()
                .filter_map(|c| c.get_str().iter().map(|s| s.to_string()).next())
                .next()
        }))
    }
    pub fn relations_vec(&self) -> Result<Vec<String>, ploke_error::Error> {
        let vector = Vec::from_iter(self.iter_relations()?);
        Ok(vector)
    }
    pub fn get_crate_name_id(&self, crate_name: &str) -> Result<String, DbError> {
        use serde_json::Value;

        let rows = self.raw_query("?[name, id] := *crate_context {id, name}")?;

        // Unwrap row 0
        let row = rows.rows.first().expect("no rows returned");

        // Pull the two columns out as strings
        let name = match &row[0] {
            DataValue::Str(s) => s.clone(),
            _ => panic!("Invariant Violated: name is not a string"),
        };

        let id = match &row[1] {
            DataValue::Uuid(UuidWrapper(uuid)) => uuid.to_string(), // fallback
            _ => panic!("Invariant Violated: id is not a Uuid"),
        };

        // Build the filename
        let name_id = format!("{}_{}", name, id);
        Ok(name_id)
    }
    pub fn get_path_info(&self, path: &str) -> Result<QueryResult, ploke_error::Error> {
        let ty = NodeType::Module;
        let rel = ty.relation_str();
        let keys: String = ty.keys().join(", ");
        let vals: String = ty.vals().join(", ");
        let script = format!("?[target_path, {keys}, {vals}] := *file_mod{{owner_id: id, file_path: target_path, @ 'NOW' }},
                        *module{{ {keys}, {vals} @ 'NOW' }},
                        target_path = \"{path}\",
                        is_embedding_null = is_null(embedding)
        ");
        tracing::info!(target: "file_hashes", "using script\n{}", &script);
        let res = self.raw_query(&script)?;
        Ok(res)
    }
    pub fn get_mod_info(&self, mod_id: Uuid) -> Result<QueryResult, ploke_error::Error> {
        let ty = NodeType::Module;
        let rel = ty.relation_str();
        let keys: String = ty.keys().filter(|s| *s != "id").join(", ");
        let vals: String = ty.vals().join(", ");
        let script = format!(
            "?[file_path, {keys}, {vals}] := *file_mod{{owner_id: id, file_path, @ 'NOW' }},
                        *module{{ {keys}, {vals} @ 'NOW' }},
                        is_embedding_null = is_null(embedding)
        "
        );
        let res = self.raw_query(&script)?;
        Ok(res)
    }

    pub async fn index_embeddings(
        &mut self,
        node_type: NodeType,
        dim: usize,
    ) -> Result<(), DbError> {
        // TODO: This is so dirty.
        // I know there is a better way to do this with cozo using the BTreeMap approach.
        // Figure it out.
        let dim_string = dim.to_string();
        let embedding_query: [&str; 8] = [
            r#"::hnsw create"#,
            node_type.relation_str(),
            r#":embedding_idx { "#,
            dim_string.as_str(),
            r#": "#,
            r#"fields: [embedding"#,
            // embedding
            r#"],
        distance: Cosine,
        filter: !is_null(embedding
        "#,
            // embedding
            r#"
        )
}
"#,
        ];
        let query_string = embedding_query.concat();
        self.run_script(
            query_string.as_str(),
            BTreeMap::new(),
            cozo::ScriptMutability::Mutable,
        )
        .map_err(|e| DbError::Cozo(e.to_string()))?;

        Ok(())
    }

    pub async fn update_embeddings_batch(
        &self,
        updates: Vec<(uuid::Uuid, Vec<f32>)>,
    ) -> Result<(), DbError> {
        if updates.is_empty() {
            return Ok(());
        }

        // Validate embeddings before processing
        for (_, embedding) in &updates {
            Self::validate_embedding_vec(embedding)?;
        }

        // Convert updates to DataValue format - as a list of [id, embedding] pairs
        let updates_data: Vec<DataValue> = updates
            .into_iter()
            .map(|(id, embedding)| {
                let id_val = DataValue::Uuid(UuidWrapper(id));
                let embedding_val = DataValue::List(
                    embedding
                        .into_iter()
                        .map(|f| DataValue::Num(cozo::Num::Float(f as f64)))
                        .collect(),
                );
                // Each update is a list containing [id, embedding]
                DataValue::List(vec![id_val, embedding_val])
            })
            .collect();

        let mut params = BTreeMap::new();
        params.insert("updates".to_string(), DataValue::List(updates_data));

        for node_type in NodeType::primary_nodes() {
            let rel_name = node_type.relation_str();
            let keys_iter = node_type.keys();
            // Filter out "embedding" so there isn't a conflict in the returned values from the
            // database vs the added values in the `put`
            let vals_iter = node_type.vals().filter(|v| *v != "embedding");
            let key_vals_string = keys_iter.chain(vals_iter).join(", ");
            let rel_identity = node_type.identity();

            // A bit convoluted, but should ultimately come out to something like:
            //
            // {
            //     ?[new_id, new_embedding] <- $updates
            //     :replace _new {new_id, new_embedding}
            // }
            // {
            //     ?[at, embedding, id, name, docstring, vis_kind, vis_path, span, tracking_hash,
            //              cfgs, return_type_id, body, module_id]
            //      :=
            //         *_new{new_id: id, new_embedding: embedding},
            //         at = 'ASSERT',
            //         *function {id, name, docstring, vis_kind, vis_path, span, tracking_hash,
            //              cfgs, return_type_id, body, module_id}
            //     :put function {id, at => name, docstring, vis_kind, vis_path, span, tracking_hash,
            //              cfgs, return_type_id, body, module_id, embedding}
            // }
            let script2_first_block = [r#"
{
    ?[new_id, new_embedding] <- $updates 
    :replace _new {new_id, new_embedding} 
}"#]
            .into_iter();
            let script2_second_block = [
                r#"
{ 
    ?[at, embedding, "#,
                &key_vals_string,
                r#"] := *_new{new_id: id, new_embedding: embedding}, 
        at = 'ASSERT',
        *"#,
                rel_name,
                " { ",
            ]
            .into_iter();
            let mut script2 = String::from_iter(script2_first_block.chain(script2_second_block));
            script2.push_str(&key_vals_string);
            script2.push_str("}\n :put ");
            script2.push_str(&rel_identity);
            script2.push_str("\n}");

            let result = self
                .run_script(&script2, params.clone(), cozo::ScriptMutability::Mutable)
                .map_err(|e| {
                    let error_json = cozo::format_error_as_json(e, None);
                    let error_str = serde_json::to_string_pretty(&error_json).unwrap();
                    tracing::error!("{}", error_str);
                    DbError::Cozo(error_str)
                })
                .inspect_err(|e| {
                    tracing::error!("{}", e);
                    tracing::error!("script2:\n{}", &script2)
                });
            if result.is_err() {
                tracing::error!("full_result: {:#?}", result);
            }
            result?;
        }

        Ok(())
    }

    /// Validate that an embedding vector is non-empty
    fn validate_embedding_vec(embedding: &[f32]) -> Result<(), DbError> {
        if embedding.is_empty() {
            Err(DbError::QueryExecution(
                "Embedding vector must not be empty".into(),
            ))
        } else {
            Ok(())
        }
    }

    /// Fetches all primary nodes that do not yet have an embedding.
    ///
    /// This query retrieves the necessary information to fetch the node's content
    /// and later associate the generated embedding with the correct node.
    pub fn get_unembedded_node_data(
        &self,
        limit: usize,
        cursor: usize,
    ) -> Result<Vec<TypedEmbedData>, ploke_error::Error> {
        let mut unembedded_data = Vec::new();
        let mut count = 0;
        // TODO: Awkward. Improve this.
        for t in NodeType::primary_nodes() {
            let nodes_of_type = self.get_unembed_rel(t, limit.saturating_sub(count), cursor)?;
            count += nodes_of_type.len();
            tracing::info!("=== {count} ===");
            unembedded_data.push(nodes_of_type);
        }
        Ok(unembedded_data)
    }

    /// Fetches all primary nodes that already have an embedding.
    pub fn get_embedded_node_data(
        &self,
        limit: usize,
        cursor: usize,
    ) -> Result<Vec<TypedEmbedData>, ploke_error::Error> {
        let mut unembedded_data = Vec::new();
        let mut count = 0;
        // TODO: Awkward. Improve this.
        for t in NodeType::primary_nodes() {
            let nodes_of_type = self.get_embed_rel(t, limit.saturating_sub(count), cursor)?;
            count += nodes_of_type.len();
            tracing::info!("=== {count} ===");
            unembedded_data.push(nodes_of_type);
        }
        Ok(unembedded_data)
    }

    // TODO: finish integrating get_file_data into the batch embedding process.
    // Most likely this will involve repalcing the Vec<EmbeddingData> with a hashmap.
    pub fn get_file_data(&self) -> Result<Vec<FileData>, ploke_error::Error> {
        let script = r#"
            ?[id, tracking_hash, namespace, file_path] := 
                *module { id, tracking_hash },
                *file_mod { owner_id: id, namespace, file_path },
                *crate_context { namespace }
        "#;

        let named_rows = self
            .db
            .run_script(script, BTreeMap::new(), cozo::ScriptMutability::Immutable)
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        let query_result = QueryResult::from(named_rows);
        query_result.try_into_file_data()
    }

    pub fn count_unembedded_files(&self) -> Result<usize, DbError> {
        let script = r#"
            ?[count( id )] := 
                *module { id, tracking_hash @ 'NOW' },
                *file_mod { owner_id: id, namespace, file_path @ 'NOW' },
                *crate_context { namespace @ 'NOW' }
        "#;

        let result = self
            .db
            .run_script(script, BTreeMap::new(), cozo::ScriptMutability::Immutable)
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        // Ok(named_rows.flatten().len())
        Self::into_usize(result)
    }

    /// Retrieves ordered embedding data for a list of target nodes.
    ///
    /// This method fetches the embedding data for a specific set of nodes identified by their UUIDs,
    /// returning the results in the same order as the input IDs. It includes file path, namespace,
    /// and other metadata needed for code understanding.
    ///
    /// # Arguments
    ///
    /// * `nodes` - A vector of UUIDs representing the nodes to retrieve
    ///
    /// # Returns
    ///
    /// A result containing a vector of `EmbeddingData` structs in the same order as the input UUIDs,
    /// or an error if the query fails.
    /// This is useful for retrieving the `EmbeddingData` required to retrieve code snippets from
    /// files after finding the Ids via a search method (dense embedding search, bm25 search)
    pub fn get_nodes_ordered(
        &self,
        nodes: Vec<Uuid>,
    ) -> Result<Vec<EmbeddingData>, ploke_error::Error> {
        let ancestor_rules = Self::ANCESTOR_RULES;
        let has_embedding_rule = NodeType::primary_nodes().iter().map(|ty| {
            let rel = ty.relation_str();
            format!(r#"
            has_embedding[id, name, hash, span] := *{rel}{{id, name, tracking_hash: hash, span, embedding @ 'NOW' }}, !is_null(embedding)
            "#)
        }).join("\n");

        let script = format!(
            r#"
        target_ids[node_id, ordering] <- $data

        {ancestor_rules}

        {has_embedding_rule}

    batch[id, name, file_path, file_hash, hash, span, namespace, ordering] := 
        has_embedding[id, name, hash, span],
        ancestor[id, mod_id],
        is_root_module[mod_id],
        *module{{id: mod_id, tracking_hash: file_hash}},
        *file_mod {{ owner_id: mod_id, file_path, namespace }},
        target_ids[id, ordering]

    ?[id, name, file_path, file_hash, hash, span, namespace, ordering] := 
        batch[id, name, file_path, file_hash, hash, span, namespace, ordering]
        :sort ordering
     "#
        );

        let ids_data: Vec<DataValue> = nodes
            .into_iter()
            .enumerate()
            .map(|(i, id)| {
                DataValue::List(vec![
                    DataValue::from(i as i64),
                    DataValue::Uuid(UuidWrapper(id)),
                ])
            })
            .collect();
        let limit = ids_data.len();

        // Create parameters map
        let mut params = BTreeMap::new();
        params.insert("data".into(), DataValue::List(ids_data));

        let query_result = self
            .db
            .run_script(&script, params, cozo::ScriptMutability::Immutable)
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        let embedding_data = QueryResult::from(query_result).to_embedding_nodes()?;
        Ok(embedding_data)
    }

    pub fn get_unembed_rel(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: usize,
    ) -> Result<TypedEmbedData, ploke_error::Error> {
        let mut base_script = String::new();
        // TODO: Add pre-registered fixed rules to the system.
        let base_script_start = r#"
    parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains"}

    ancestor[desc, asc] := parent_of[desc, asc]
    ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

    needs_embedding[id, name, hash, span] := *"#;
        let base_script_end = r#" {id, name, tracking_hash: hash, span, embedding}, is_null(embedding)

    is_root_module[id] := *module{id}, *file_mod {owner_id: id}

    batch[id, name, file_path, file_hash, hash, span, namespace] := 
        needs_embedding[id, name, hash, span],
        ancestor[id, mod_id],
        is_root_module[mod_id],
        *module{id: mod_id, tracking_hash: file_hash},
        *file_mod { owner_id: mod_id, file_path, namespace },

    ?[id, name, file_path, file_hash, hash, span, namespace] := 
        batch[id, name, file_path, file_hash, hash, span, namespace]
        :sort id
        :limit $limit
     "#;
        let rel_name = node_type.relation_str();

        base_script.push_str(base_script_start);
        base_script.push_str(rel_name);
        base_script.push_str(base_script_end);

        // Create parameters map
        let mut params = BTreeMap::new();
        params.insert("limit".into(), DataValue::from(limit as i64));
        params.insert("cursor".into(), DataValue::from(cursor as i64));

        let query_result = self
            .db
            .run_script(&base_script, params, cozo::ScriptMutability::Immutable)
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

    pub fn get_embed_rel(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: usize,
    ) -> Result<TypedEmbedData, ploke_error::Error> {
        let mut base_script = String::new();
        // TODO: Add pre-registered fixed rules to the system.
        let base_script_start = r#"
    parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains"}

    ancestor[desc, asc] := parent_of[desc, asc]
    ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

    has_embedding[id, name, hash, span] := *"#;
        let base_script_end = r#" {id, name, tracking_hash: hash, span, embedding}, !is_null(embedding)

    is_root_module[id] := *module{id}, *file_mod {owner_id: id}

    batch[id, name, file_path, file_hash, hash, span, namespace] := 
        has_embedding[id, name, hash, span],
        ancestor[id, mod_id],
        is_root_module[mod_id],
        *module{id: mod_id, tracking_hash: file_hash},
        *file_mod { owner_id: mod_id, file_path, namespace },

    ?[id, name, file_path, file_hash, hash, span, namespace] := 
        batch[id, name, file_path, file_hash, hash, span, namespace]
        :sort id
        :limit $limit
     "#;
        let rel_name = node_type.relation_str();

        base_script.push_str(base_script_start);
        base_script.push_str(rel_name);
        base_script.push_str(base_script_end);

        // Create parameters map
        let mut params = BTreeMap::new();
        params.insert("limit".into(), DataValue::from(limit as i64));
        params.insert("cursor".into(), DataValue::from(cursor as i64));

        let query_result = self
            .db
            .run_script(&base_script, params, cozo::ScriptMutability::Immutable)
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

    #[instrument(target = "specific_target", skip_all, fields(limit = 0))]
    pub fn get_rel_with_cursor(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: Uuid,
    ) -> Result<TypedEmbedData, ploke_error::Error> {
        let mut base_script = String::new();
        // TODO: Add pre-registered fixed rules to the system.
        let base_script_start = r#"
    parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW' }

    ancestor[desc, asc] := parent_of[desc, asc]
    ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

    needs_embedding[id, name, hash, span] := *"#;
        let base_script_end = r#" {id, name, tracking_hash: hash, span, embedding @ 'NOW' }, is_null(embedding)

    is_root_module[id] := *module{id}, *file_mod {owner_id: id}

    batch[id, name, file_path, file_hash, hash, span, namespace, string_id] := 
        needs_embedding[id, name, hash, span],
        ancestor[id, mod_id],
        is_root_module[mod_id],
        *module{id: mod_id, tracking_hash: file_hash @ 'NOW' },
        *file_mod { owner_id: mod_id, file_path, namespace @ 'NOW' },
        to_string(id) > to_string($cursor),
        string_id = to_string(id)

    ?[id, name, file_path, file_hash, hash, span, namespace, string_id] := 
        batch[id, name, file_path, file_hash, hash, span, namespace, string_id]
        :sort string_id
        :limit $limit
     "#;
        let rel_name = node_type.relation_str();

        base_script.push_str(base_script_start);
        base_script.push_str(rel_name);
        base_script.push_str(base_script_end);

        // Create parameters map
        let mut params = BTreeMap::new();
        params.insert("limit".into(), DataValue::from(limit as i64));
        params.insert("cursor".into(), DataValue::Uuid(UuidWrapper(cursor)));

        let query_result = self
            .db
            .run_script(&base_script, params, cozo::ScriptMutability::Immutable)
            .inspect_err(|e| tracing::error!("{e}"))
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        let less_flat_row = query_result.rows.first();
        let count_less_flat = query_result.rows.len();
        if let Some(lfr) = less_flat_row {
            tracing::info!("\n{:=^80}\n== less_flat: {count_less_flat} ==\n== less_flat: {less_flat_row:?} ==\nlimit: {limit}", rel_name);
        }
        let mut v = QueryResult::from(query_result).to_embedding_nodes()?;
        v.truncate(limit.min(count_less_flat));
        if !v.is_empty() {
            tracing::info!(
                "\n== after truncated, {} remain: {:?} ==\n{:=^80}",
                v.len(),
                v.iter().map(|c| &c.name).join(" | "),
                ""
            );
        }
        let ty_embed = TypedEmbedData { v, ty: node_type };
        Ok(ty_embed)
    }

    /// Gets the primary node typed embed data needed to update the nodes in the database
    /// that are within the given file.
    /// Note that this does not include the module nodes for the files themselves.
    /// This is useful when doing a partial update of the database following change detection in
    /// previously parsed and inserted files.
    // WARN: This needs to be tested
    pub fn get_nodes_by_file_with_cursor(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: Uuid,
    ) -> Result<TypedEmbedData, ploke_error::Error> {
        todo!();
        let ancestor_rule = r#"
parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains"}

ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc],
    *file_mod{owner_id: asc}
        "#;

        let mut query: String = String::new();
        query.push_str(ancestor_rule);

        let needs_update_start = r#"
needs_update[id, name, hash, span] :=
        "#;
        query.push_str(needs_update_start);

        let rel_name = node_type.relation_str();

        // TODO: Change this function to apply to all types at once, rather than the per-type
        // approach we are using right now. This requires that we somehow encode the type of the
        // node within the relation - if possible, use the node relation name for this, if that is
        // not possible (due to cozo rules or something, add a new field to the relations, probably
        // using the discriminant of the enum PrimaryNodeType)
        // let primary_nodes = NodeType::primary_nodes();
        // for (i, primary_node) in primary_nodes.iter().enumerate() {
        //     query.push_str(&format!(
        //     "*{} {{ id, tracking_hash, span }}",
        //         primary_node.relation_str()
        //     ));
        //     if i + 1 < primary_nodes.len() {
        //         query.push_str(" or ")
        //     }
        // }

        let batch_rule = r#"
batch[id, name, target_file, file_hash, hash, span, namespace, string_id] :=
    needs_update[id, name, hash, span],
    ancestor[id, mod_id],
    is_root_module[mod_id],
    *module{id: mod_id, tracking_hash: file_hash },
    *file_mod {owner_id: mod_id, file_path: target_file, namespace },
    target_file = "crates/ploke-tui/src/lib.rs",
    to_string(id) > to_string($cursor),
    string_id = to_string(id)
        "#;
        query.push_str(batch_rule);

        let final_query = r#"
?[id, name, target_file, file_hash, hash, span, namespace, string_id] :=
    batch[id, name, target_file, file_hash, hash, span, namespace, string_id]
    :sort string_id
    :limit $limit
        "#;
        query.push_str(final_query);

        let rel_name = node_type.relation_str();

        let mut params: BTreeMap<String, DataValue> = BTreeMap::new();
        params.insert("limit".into(), DataValue::from(limit as i64));
        params.insert("cursor".into(), DataValue::Uuid(UuidWrapper(cursor)));

        let query_result = self
            .db
            .run_script(&query, BTreeMap::new(), cozo::ScriptMutability::Immutable)
            .inspect_err(|e| tracing::error!("{e}"))
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        let less_flat_row = query_result.rows.first();
        let count_less_flat = query_result.rows.len();
        if let Some(lfr) = less_flat_row {
            tracing::info!("\n{:=^80}\n== less_flat: {count_less_flat} ==\n== less_flat: {less_flat_row:?} ==\nlimit: {limit}", rel_name);
        }
        let mut v = QueryResult::from(query_result).to_embedding_nodes()?;
        v.truncate(limit.min(count_less_flat));
        if !v.is_empty() {
            tracing::info!(
                "\n== after truncated, {} remain: {:?} ==\n{:=^80}",
                v.len(),
                v.iter().map(|c| &c.name).join(" | "),
                ""
            );
        }
        let ty_embed = TypedEmbedData { v, ty: node_type };
        Ok(ty_embed)
    }

    pub fn count_unembedded_nonfiles(&self) -> Result<usize, DbError> {
        let nodes = self.count_pending_embeddings()?;
        let files = self.count_unembedded_files()?;
        let count = nodes.checked_sub(files).expect(
            "Invariant: There must be more nodes than files, since files are a subset of nodes",
        );
        Ok(count)
    }

    pub fn count_pending_embeddings(&self) -> Result<usize, DbError> {
        let lhs = r#"?[count(id)] := 
        "#;
        let mut query: String = String::new();

        query.push_str(lhs);
        for (i, primary_node) in NodeType::primary_nodes().iter().enumerate() {
            query.push_str(&format!(
                "*{} {{ id, embedding: null, tracking_hash, span @ 'NOW' }}",
                primary_node.relation_str()
            ));
            if i + 1 < NodeType::primary_nodes().len() {
                query.push_str(" or ")
            }
        }
        tracing::info!("count nodes with query:\n{}", query);
        let result = self
            .db
            .run_script_read_only(&query, Default::default())
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        tracing::info!("result of query:\n{:?}", result);

        Self::into_usize(result)
    }

    pub fn into_usize(named_rows: NamedRows) -> Result<usize, DbError> {
        named_rows
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|v| v.get_int())
            .inspect(|v| tracing::info!("the value in first row, first cell is: {:?}", v))
            .map(|n| n as usize)
            .ok_or(DbError::NotFound)
    }

    pub fn get_pending_test(&self) -> Result<NamedRows, DbError> {
        let lhs = r#"?[ at, name, id] := 
        "#;
        let mut query2: String = String::new();
        query2.push_str(lhs);
        for (i, primary_node) in NodeType::primary_nodes().iter().enumerate() {
            query2.push_str(&format!(
                "*{} {{ id, at, embedding: null, tracking_hash, span, name @ 'NOW' }}",
                primary_node.relation_str()
            ));
            if i + 1 < NodeType::primary_nodes().len() {
                query2.push_str(" or ")
            }
        }
        let result2 = self
            .db
            .run_script_read_only(&query2, Default::default())
            .map_err(|e| DbError::Cozo(e.to_string()))?;
        Ok(result2)
    }

    /// Upsert BM25 document metadata in a batch transaction
    ///
    /// This method inserts or updates BM25 document metadata for multiple documents in a single
    /// database transaction. Each document is identified by its UUID and contains metadata needed
    /// for BM25 scoring including a tracking hash, tokenizer version, and token length.
    pub fn upsert_bm25_doc_meta_batch(
        &self,
        docs: impl IntoIterator<Item = (Uuid, DocMeta)>,
    ) -> Result<(), DbError> {
        // Convert docs to DataValue format
        let docs_data: Vec<DataValue> = docs
            .into_iter()
            .map(|(id, doc_meta)| {
                let DocMeta {
                    token_length,
                    tracking_hash,
                } = doc_meta;
                DataValue::List(vec![
                    DataValue::Uuid(UuidWrapper(id)),
                    DataValue::Uuid(UuidWrapper(tracking_hash.0)),
                    DataValue::Str(TOKENIZER_VERSION.into()),
                    DataValue::Num(cozo::Num::Int(token_length as i64)),
                ])
            })
            .collect();

        let mut params = BTreeMap::new();
        params.insert("docs".to_string(), DataValue::List(docs_data));

        let script = r#"
            # Upsert document metadata
            docs_data[id, tracking_hash, tokenizer_version, token_length] <- $docs
            ?[id, tracking_hash, tokenizer_version, token_length, at] := 
                docs_data[id, tracking_hash, tokenizer_version, token_length],
                at = 'ASSERT'

            :put bm25_doc_meta { id => tracking_hash, tokenizer_version, token_length, at }
        "#;

        self.run_script(script, params, cozo::ScriptMutability::Mutable)
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bm25_index::DocData;
    use crate::Database;
    use crate::DbError;
    use cozo::{Db, MemStorage, ScriptMutability};
    use ploke_transform::schema::create_schema_all;
    use tracing::Level;
    use tracing_subscriber::layer::SubscriberExt;
    use tracing_subscriber::util::SubscriberInitExt;
    use uuid::Uuid;

    fn setup_db() -> Database {
        let db = Db::new(MemStorage::default()).unwrap();
        db.initialize().unwrap();
        create_schema_all(&db).unwrap();
        Database::new(db)
    }

    #[tokio::test]
    async fn update_embeddings_batch_empty() -> Result<(), DbError> {
        let db = setup_db();
        db.update_embeddings_batch(vec![]).await?;
        // Should not panic/error with empty input
        Ok(())
    }

    #[tokio::test]
    async fn test_get_file_data() -> Result<(), ploke_error::Error> {
        // Initialize the logger to see output from Cozo
        let db = Database::new(ploke_test_utils::setup_db_full("fixture_nodes")?);
        let count1 = db.count_pending_embeddings()?;
        tracing::debug!("Found {} nodes without embeddings", count1);
        assert_ne!(0, count1);

        let limit = 100;
        let cursor = 0;

        let unembedded_data = db.get_unembedded_node_data(limit, cursor)?;
        let unembedded_data = unembedded_data
            .iter()
            .flat_map(|emb| emb.v.iter())
            .collect_vec();
        for node in unembedded_data.iter() {
            tracing::trace!("{}", node.id);
        }

        let count2 = unembedded_data.len();
        assert_ne!(0, count2);
        tracing::debug!("Retrieved {} nodes without embeddings", count2);

        let file_data = db.get_file_data()?;
        eprintln!("{:#?}", file_data);
        assert_eq!(10, file_data.len());
        for node in unembedded_data.iter() {
            assert!(
                file_data.iter().any(|f| f.namespace == node.namespace),
                "No node with identical tracking hash to file: {:#?}",
                node
            );
        }

        Ok(())
    }

    #[tokio::test]
    async fn test_count_nodes_for_embedding() -> Result<(), ploke_error::Error> {
        let db = Database::new(ploke_test_utils::setup_db_full("fixture_nodes")?);
        let count = db.count_pending_embeddings()?;
        tracing::info!("Found {} nodes without embeddings", count);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_nodes_two() -> Result<(), ploke_error::Error> {
        // ploke_test_utils::init_test_tracing(Level::INFO);
        // Initialize the logger to see output from Cozo
        let db = Database::new(ploke_test_utils::setup_db_full("fixture_nodes")?);

        let count1 = db.count_pending_embeddings()?;
        tracing::debug!("Found {} nodes without embeddings", count1);
        assert_ne!(0, count1);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_nodes_for_embedding() -> Result<(), ploke_error::Error> {
        // ploke_test_utils::init_test_tracing(Level::ERROR);
        // Initialize the logger to see output from Cozo
        let db = Database::new(ploke_test_utils::setup_db_full("fixture_nodes")?);
        let count1 = db.count_pending_embeddings()?;
        tracing::debug!("Found {} nodes without embeddings", count1);
        assert_ne!(0, count1);

        let limit = 100;
        let cursor = 0;

        let unembedded_data = db.get_unembedded_node_data(limit, cursor)?;
        let unembedded_data = unembedded_data
            .iter()
            .flat_map(|emb| emb.v.iter())
            .collect_vec();
        for node in unembedded_data.iter() {
            tracing::trace!("{}", node.id);
        }

        let count2 = unembedded_data.len();
        assert_ne!(0, count2);
        tracing::debug!("Retrieved {} nodes without embeddings", count2);
        assert!(count1 > count2);
        Ok(())
    }

    #[tokio::test]
    async fn test_unembedded_counts() -> Result<(), ploke_error::Error> {
        ploke_test_utils::init_test_tracing(Level::TRACE);
        // Initialize the logger to see output from Cozo
        let db = Database::new(ploke_test_utils::setup_db_full("fixture_nodes")?);
        let all_pending = db.count_pending_embeddings()?;
        assert_ne!(0, all_pending);

        // Check that there are at least as many files as nodes
        let non_file_pending = db.count_unembedded_nonfiles()?;
        assert!(non_file_pending <= all_pending);

        // Check it all adds up.
        let file_pending = db.count_unembedded_files()?;
        assert!((non_file_pending + file_pending) == all_pending);

        // NOTE: If the limit is under around 129-ish, then this test will fail.
        // I can't tell if this is the desired result or not. Depends on how we want to design the
        // file and node counting functions.
        let limit = 200;
        let cursor = 0;

        let unembedded_data = db.get_unembedded_node_data(limit, cursor)?;
        let unembedded_data = unembedded_data
            .iter()
            .flat_map(|emb| emb.v.iter())
            .collect_vec();
        assert_eq!(non_file_pending, unembedded_data.len());
        for node in unembedded_data.iter() {
            tracing::trace!("{}", node.id);
        }
        Ok(())
    }

    #[tokio::test]
    #[ignore = "test needs refactoring"]
    async fn update_embeddings_batch_single() -> Result<(), DbError> {
        let db = setup_db();
        let id = Uuid::new_v4();
        let embedding = vec![1.0, 2.0, 3.0];

        // Insert initial record with null embedding
        let insert_script = r#"
            ?[id] <- [[$id]]
            :put embedding_nodes { id => embedding: null }
        "#;
        let mut params = BTreeMap::new();
        params.insert("id".to_string(), DataValue::Uuid(UuidWrapper(id)));
        db.db
            .run_script(insert_script, params, cozo::ScriptMutability::Mutable)
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        db.update_embeddings_batch(vec![(id, embedding.clone())])
            .await?;

        // Verify embedding was saved
        let result = db
            .db
            .run_script(
                "?[id, embedding] := *embedding_nodes{id, embedding}",
                std::collections::BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        assert_eq!(result.rows.len(), 1);
        if let DataValue::Uuid(uuid_wrapper) = &result.rows[0][0] {
            assert_eq!(uuid_wrapper.0, id);
        } else {
            panic!("Expected Uuid DataValue");
        }
        if let DataValue::List(list) = &result.rows[0][1] {
            assert_eq!(list.len(), 3);
            if let DataValue::Num(cozo::Num::Float(f)) = list[0] {
                assert_eq!(f, 1.0);
            } else {
                panic!("Expected Float DataValue");
            }
            if let DataValue::Num(cozo::Num::Float(f)) = list[1] {
                assert_eq!(f, 2.0);
            } else {
                panic!("Expected Float DataValue");
            }
            if let DataValue::Num(cozo::Num::Float(f)) = list[2] {
                assert_eq!(f, 3.0);
            } else {
                panic!("Expected Float DataValue");
            }
        } else {
            panic!("Expected List DataValue");
        }

        Ok(())
    }

    #[tokio::test]
    #[ignore = "Needs to use new callback method"]
    async fn test_update_embeddings_batch() -> Result<(), ploke_error::Error> {
        // ploke_test_utils::init_test_tracing(Level::DEBUG);
        // 1. Setup the database with a fixture
        let db = Database::new(ploke_test_utils::setup_db_full("fixture_nodes")?);

        // 2. Get initial state
        let initial_count = db.count_unembedded_nonfiles()?;
        assert!(initial_count > 0, "Fixture should have unembedded nodes");

        // 3. Get a batch of nodes to update
        let nodes_to_update = db.get_unembedded_node_data(10, 0)?;
        let nodes_to_update = nodes_to_update
            .iter()
            .flat_map(|emb| emb.v.iter())
            .collect_vec();
        let update_count = nodes_to_update.len();
        assert!(update_count > 0, "Should retrieve some nodes to update");
        assert!(update_count <= 10);

        // 4. Create mock embeddings for the batch
        let updates: Vec<(uuid::Uuid, Vec<f32>)> = nodes_to_update
            .into_iter()
            .map(|node| (node.id, vec![1.0; 384]))
            .collect();

        // 5. Call the function to update the batch
        db.update_embeddings_batch(updates).await?;
        // assert_eq!(update_count, updated_ct);

        // 6. Verify the update
        let final_count = db.count_unembedded_nonfiles()?;
        assert_eq!(
            final_count,
            initial_count - update_count,
            "The number of pending embeddings should decrease by the number of updated nodes, which is {}",
            update_count
        );

        Ok(())
    }

    #[tokio::test]
    async fn test_upsert_bm25_doc_meta_batch() -> Result<(), DbError> {
        let db = setup_db();
        let docdata_one = DocMeta {
            token_length: 42,
            tracking_hash: TrackingHash(Uuid::new_v4()),
        };
        let docdata_two = DocMeta {
            token_length: 128,
            tracking_hash: TrackingHash(Uuid::new_v4()),
        };

        let docs = vec![(Uuid::new_v4(), docdata_one), (Uuid::new_v4(), docdata_two)];

        db.upsert_bm25_doc_meta_batch(docs.into_iter()).unwrap();

        // Verify data was inserted
        let result = db
            .db
            .run_script(
                "?[id, tracking_hash, tokenizer_version, token_length] := *bm25_doc_meta{id, tracking_hash, tokenizer_version, token_length}",
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        assert_eq!(result.rows.len(), 2);

        // Verify the first document
        if let DataValue::Uuid(uuid_wrapper) = &result.rows[0][0] {
            // ID is correct
        } else {
            panic!("Expected Uuid DataValue for id");
        }

        eprintln!("{:?}", result.rows[0]);
        if let DataValue::Num(cozo::Num::Int(token_length)) = result.rows[0][3] {
            assert_eq!(token_length, 128);
        } else {
            panic!("Expected Int DataValue for token_length");
        }

        // Verify the second document
        eprintln!("{:?}", result.rows[1]);
        if let DataValue::Num(cozo::Num::Int(token_length)) = result.rows[1][3] {
            assert_eq!(token_length, 42);
        } else {
            panic!("Expected Int DataValue for token_length");
        }

        Ok(())
    }
}
