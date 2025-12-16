use std::collections::BTreeMap;
use std::sync::{Arc, Mutex, RwLock};
use std::{ops::Deref, panic::Location, path::Path};

use crate::bm25_index::{DocMeta, TOKENIZER_VERSION};
use crate::error::DbError;
use crate::multi_embedding::db_ext::EmbeddingExt;
use crate::multi_embedding::schema::{EmbeddingSetExt as _, EmbeddingVector};
use crate::NodeType;
use crate::QueryResult;
use cozo::{DataValue, Db, MemStorage, NamedRows, UuidWrapper, Vector};
use itertools::Itertools;
use lazy_static::lazy_static;
use ploke_core::{EmbeddingData, FileData, TrackingHash};
use ploke_error::Error as PlokeError;
use ploke_transform::schema::meta::Bm25MetaSchema;
use serde::{Deserialize, Serialize};
use syn_parser::parser::nodes::{AnyNodeId, ToCozoUuid};
use tracing::{debug, info, instrument, trace};
use uuid::Uuid;

// cfg items

use ploke_core::embeddings::{
    EmbRelName, EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};

pub(crate) const TRACKING_HASH_TARGET: &str = "tracking-hash";

lazy_static! {
    static ref DEFAULT_EMBEDDING_SET: EmbeddingSet = EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("local"),
        EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2"),
        EmbeddingShape::new_dims_default(384)
    );
}

// end cfg items

pub const HNSW_SUFFIX: &str = ":hnsw_idx";

/// Main database connection and query interface
// TODO:refactor:database improve state handling for active_embedding_set
// JL, 2025-12-06
// - [ ] Change the active_embedding_set to an option, and when we add ways to remove embeddings,
// ensure the active_embedding_set is changed to None.
// - [ ] Add an error type for when an embedding set is not found
// - [ ] Add convenience methods to map the option onto an error when we expect the
// active_embedding_set to be Some but find None, so we can ergonomically handle unwraps.
#[derive(Debug)]
pub struct Database {
    db: Db<MemStorage>,
    pub active_embedding_set: Arc<RwLock<EmbeddingSet>>,
}

#[derive(Debug, Clone, Copy)]
pub struct QueryContext {
    pub name: &'static str,
    pub script: &'static str,
}

impl QueryContext {
    pub const fn new(name: &'static str, script: &'static str) -> Self {
        Self { name, script }
    }
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

pub trait ImmutQuery {
    fn raw_query(&self, query: &str) -> Result<NamedRows, DbError>;
}

impl ImmutQuery for Database {
    fn raw_query(&self, query: &str) -> Result<NamedRows, DbError> {
        self.run_script(query, BTreeMap::new(), cozo::ScriptMutability::Immutable)
            .map_err(DbError::from)
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

pub fn to_vector(val: &DataValue, embedding_set: &EmbeddingSet) -> Result<Vec<f64>, DbError> {
    let dims = embedding_set.dims();
    if let DataValue::Vec(Vector::F32(v)) = val {
        let real_v: Vec<f64> = v.into_iter().map(|i| *i as f64).collect_vec();
        Ok(real_v)
    } else {
        panic!();
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

/// Safely converts a Cozo DataValue to a usize.
pub fn to_u64(val: &DataValue) -> Result<u64, DbError> {
    if let DataValue::Num(cozo::Num::Int(n)) = val {
        // Cozo stores numbers that can be i64, u64, or f64. Safest to try as i64 for span.
        u64::try_from(*n).map_err(|e| {
            DbError::Cozo(format!(
                "Could not convert Num::Int to i64 for usize: {:?}, original error {}",
                n, e
            ))
        })
    } else {
        Err(DbError::Cozo(format!("Expected Number, found {:?}", val)))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
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
    /// Read-only access via closure (keeps lock scope tiny).
    pub fn with_active_set<R>(&self, f: impl FnOnce(&EmbeddingSet) -> R) -> Result<R, DbError> {
        let guard = self
            .active_embedding_set
            .read()
            .map_err(|_| DbError::ActiveSetPoisoned)?;
        Ok(f(&*guard))
    }

    pub fn active_model_id(&self) -> Result<EmbeddingModelId, DbError> {
        self.with_active_set(|s| s.model.clone())
    }

    /// Mutate via closure (single write lock, tiny scope).
    pub fn update_active_set<R>(
        &self,
        f: impl FnOnce(&mut EmbeddingSet) -> R,
    ) -> Result<R, DbError> {
        let mut guard = self
            .active_embedding_set
            .write()
            .map_err(|_| DbError::ActiveSetPoisoned)?;
        Ok(f(&mut *guard))
    }

    /// Common convenience: replace the whole set.
    pub fn set_active_set(&self, new_set: EmbeddingSet) -> Result<(), DbError> {
        self.update_active_set(|slot| *slot = new_set).map(|_| ())
    }

    pub fn rel_names_with_tracking_hash<'a>(&'a self) -> Result<Vec<String>, DbError> {
        fn filter_is_th(db: &Database, rel_name: &str) -> Result<bool, DbError> {
            let script_th_col = format!("::columns {rel_name}");
            let is_th = db
                .raw_query(&script_th_col)
                .inspect(|qr| tracing::debug!(target: TRACKING_HASH_TARGET, relation_name = %rel_name, dbg_string = %qr.debug_string_all()))?
                .rows
                .into_iter()
                .flat_map(|r| r.into_iter())
                .any(|cell| cell.get_str() == Some("tracking_hash"));
            Ok(is_th)
        }

        let script_rels = "::relations";
        let v: Vec<String> = self
            .raw_query(script_rels)
            .inspect(|qr| tracing::debug!(target: TRACKING_HASH_TARGET, dbg_string = %qr.debug_string_all()))?
            .iter_col("name")
            .ok_or(DbError::NotFound)?
            .map(to_string)
            .filter_ok(|maybe_name| {
                let x = filter_is_th(self, maybe_name.as_str());
                x == Ok(true)
            })
            .try_collect()?;
        tracing::debug!(target: TRACKING_HASH_TARGET, rels_with_th = %format_args!("{v:#?}"));
        Ok(v)
    }

    pub fn get_anynode_th(&self, node_id: Uuid) -> Option<Uuid> {
        fn to_script(script_rhs: &str, vec_rel: &str, node_id: Uuid) {
            let script = format!(
                r#"?[tracking_hash] := ({script_rhs}),
                *{vec_rel} {{ node_id @ 'NOW' }},
                node_id = to_uuid("{node_id}")"#
            );
        }
        // let active_embedding_set = self.active_embedding_set.lock();
        let vec_rel: EmbRelName = self
            .with_active_set(EmbeddingSet::vector_relation_name)
            .ok()?;
        let mut debug_rel_rhs: Vec<(String, String)> = Vec::new();
        let rels_with_th_rhs = self
            .rel_names_with_tracking_hash()
            .inspect(|rel_names| tracing::debug!(target: TRACKING_HASH_TARGET,  relation_names_with_th = ?rel_names ))
            .ok()?
            .iter_mut()
            .map(|node_rel| {
                (
                    node_rel.clone(),
                    format!(r#"*{node_rel} {{ id: node_id, tracking_hash @ 'NOW' }}"#),
                )
            })
            .inspect(|(node_rel, script)| {
                tracing::debug!(target: TRACKING_HASH_TARGET,  script_adding_or_statement = %script, %node_rel);
                debug_rel_rhs.push((node_rel.clone(), script.clone()));
            })
            .map(|(_node_rel, script)| script)
            .join(" or ");

        let rels_with_th = self.rel_names_with_tracking_hash().ok()?;
        for (node_rel, script_rhs) in debug_rel_rhs {
            let script_cols = format!("::columns {node_rel}");
            let cols = self
                .raw_query(&script_cols)
                .inspect(|qr| {
                    tracing::debug!(target: TRACKING_HASH_TARGET,
                        relation_name = %node_rel,
                        dbg_string = %qr.debug_string_all()
                    )
                })
                .ok()?;

            let script = format!(
                r#"?[tracking_hash] := ({script_rhs}),
                node_id == to_uuid("{node_id}")"#
            );
            let script_result = self
                .raw_query(&script)
                .inspect(|th_expected| tracing::debug!(target: "tracking-hash", maybe_th_row = ?th_expected))
                .ok()?;
        }
        let script = format!(
            r#"?[tracking_hash] := ({rels_with_th_rhs}),
            node_id = to_uuid("{node_id}")"#
        );
        tracing::debug!(target: TRACKING_HASH_TARGET, script_find_tracking_hashes = %script);
        let node_info = self;
        let query_result = self
            .raw_query(&script)
            .inspect(|qr| tracing::debug!(target: TRACKING_HASH_TARGET, dbg_string = %qr.debug_string_all()))
            .inspect_err(|e| tracing::error!("{e:?}"))
            .ok()?;
        let tracking_hash_uuid: &DataValue = query_result.iter_col("tracking_hash")?.next()?;
        to_uuid(tracking_hash_uuid).ok()
    }

    const ANCESTOR_RULES: &str = r#"
    parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains"}

    ancestor[desc, asc] := parent_of[desc, asc]
    ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]
    is_root_module[id] := *module{id}, *file_mod {owner_id: id}
    "#;

    /// Get the embedding data from the canonical path of a given node, find the node in the
    /// database and return the `EmbeddingData` so the snippet for the node can be found from a
    /// file read.
    /// Returns the database error or the embedding data for the item.
    pub fn get_node_from_canon(
        &self,
        canonical_path: &[&str],
    ) -> Result<EmbeddingData, PlokeError> {
        todo!()
    }

    /// Create new database connection
    pub fn new(db: Db<MemStorage>) -> Self {
        Self {
            db,
            // TODO:migrate-multi-embed-full
            // Update to not use hardcoded active_embedding_set in new and update callsites.
            // Replace those taht use default model with call to `default()` instead
            active_embedding_set: Arc::new(RwLock::new(DEFAULT_EMBEDDING_SET.clone())),
        }
    }
    pub fn new_init() -> Result<Self, PlokeError> {
        let db = Db::new(MemStorage::default()).map_err(|e| DbError::Cozo(e.to_string()))?;
        db.initialize().map_err(|e| DbError::Cozo(e.to_string()))?;
        // TODO:migrate-multi-embed-full
        // Update to not use hardcoded active_embedding_set in new and update callsites.
        // Replace those taht use default model with call to `default()` instead
        Ok(Self {
            db,
            active_embedding_set: Arc::new(RwLock::new(DEFAULT_EMBEDDING_SET.clone())),
        })
    }

    pub fn init_with_schema() -> Result<Self, PlokeError> {
        let db = Db::new(MemStorage::default()).map_err(|e| DbError::Cozo(e.to_string()))?;
        db.initialize().map_err(|e| DbError::Cozo(e.to_string()))?;

        // Create the schema
        ploke_transform::schema::create_schema_all(&db)?;

        Ok(Self {
            db,
            active_embedding_set: Arc::new(RwLock::new(DEFAULT_EMBEDDING_SET.clone())),
        })
    }

    // Gets all the file data in the same namespace as the crate name given as argument.
    // This is useful when you want to compare which files have changed since the database was
    // last updated.
    //
    // The query is essentially starting with the given crate name and then:
    //
    // 1. from crate name get crate contex
    //     - WARN: Assumes that the crate names are unique.
    //     - TODO: Add a test that checks what happens when we have two items with the same crate
    //         name. my guess is that this query will return all files in both namespaces, but it
    //         will be possible to distinguish between the two items by namespace.
    //
    // 2. use the crate namespace to find file-mod node.
    //
    // 3. use file-mod node (via owner_id) to get module (for tracking_hash)
    //
    // So ultimately, this is returning all the information on the file, and then the tracking hash
    // of the node.
    //
    // NOTE:refactor:file_hash 2025-12-03
    //  What we could do instead of matching on the TrackngHash here is add a field to the the
    //  file_mod node in the database, which is the same FileHash type we have started adopting in
    //  ploke-io and ploke-core for a faster hash of the entire module.
    pub fn get_crate_files(&self, crate_name: &str) -> Result<Vec<FileData>, PlokeError> {
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

    // pub fn retract_embedded_files(
    //     &self,
    //     file_mod: Uuid,
    //     ty: NodeType,
    // ) -> Result<QueryResult, PlokeError> {
    //     let rel_name = ty.relation_str();
    //     let keys = ty.keys().join(", ");
    //     let vals = ty.vals().join(", ");
    //     debug!(%rel_name, %keys, %vals);
    //     let embedding_set = &self.active_embedding_set;
    //     let set_id = embedding_set.hash_id;
    //     let vector_set_name = embedding_set.vector_relation_name();
    //     let script = format!(
    //         "parent_of[child, parent] := *syntax_edge{{
    //             source_id: parent,
    //             target_id: child,
    //             relation_kind: \"Contains\"
    //         }}
    //
    //         ancestor[desc, asc] := parent_of[desc, asc]
    //         ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]
    //
    //         to_retract[{keys}, at, {vals}] := *{rel_name} {{ {keys}, {vals}  @ 'NOW'}},
    //             *file_mod {{ owner_id: file_mod }},
    //             ancestor[id, file_mod],
    //             file_mod = \"{file_mod}\",
    //             set_id = {set_id},
    //             not *{vector_set_name} {{ node_id: id, embedding_set_id: set_id @ 'NOW' }},
    //             at = 'RETRACT'
    //
    //         ?[{keys}, at, {vals}] := to_retract[{keys}, at, {vals}]
    //             :put {rel_name} {{ {keys}, at => {vals} }}
    //             :returning
    //         "
    //     );
    //
    //     self.raw_query_mut(&script)
    //         .inspect_err(|_| {
    //             tracing::error!("using script:\n {}", script);
    //         })
    //         .map_err(PlokeError::from)
    // }

    pub fn retract_embedded_files(
        &self,
        file_mod: Uuid,
        ty: NodeType,
    ) -> Result<QueryResult, PlokeError> {
        let ty_rel_name = ty.relation_str();
        let keys = ty.keys().join(", ");
        let vals = ty.vals().join(", ");
        trace!(%ty_rel_name, %keys, %vals);
        let set_id = self.with_active_set(|set| set.hash_id().into_inner() as i64)?;
        let vector_set_name = self.with_active_set(|set| set.vector_relation_name())?;
        let vector_fields = EmbeddingVector::script_fields();
        let _script_old = format!(
            "
            {{
            parent_of[child, parent] := *syntax_edge{{
                source_id: parent, 
                target_id: child, 
                relation_kind: \"Contains\"
            }}

            ancestor[desc, asc] := parent_of[desc, asc]
            ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

            embedded_vec[node_id, embedding_set_id, vector] := *{vector_set_name} {{ 
                    node_id, 
                    embedding_set_id, 
                    vector @ 'NOW' 
                }},
                embedding_set_id == {set_id}

            to_retract[{vector_fields}] := *{ty_rel_name} {{ id: node_id  @ 'NOW'}},
                *file_mod {{ owner_id: file_mod }},
                ancestor[node_id, file_mod],
                file_mod = \"{file_mod}\",
                embedded_vec[node_id, embedding_set_id, vector],
                at = 'RETRACT'

            ?[{vector_fields}] := to_retract[{vector_fields}]
                :put {vector_set_name} {{ {vector_fields} }}
                :returning
            }}
            "
        );
        let script = format!(
            "
            {{
            parent_of[child, parent] := *syntax_edge{{
                source_id: parent, 
                target_id: child, 
                relation_kind: \"Contains\"
            }}

            ancestor[desc, asc] := parent_of[desc, asc]
            ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

            has_file_ancestor[node_id, mod_id] := *{ty_rel_name} {{id: node_id @ 'NOW'}},
                *file_mod {{ owner_id: mod_id }},
                *module {{ id: mod_id}},
                ancestor[node_id, mod_id]

            embedded_vec[node_id, embedding_set_id, vector, at] := *{vector_set_name} {{ 
                    node_id, 
                    embedding_set_id, 
                    vector @ 'NOW'
                }},
                embedding_set_id == to_int({set_id}),
                has_file_ancestor[node_id, mod_id],
                mod_id = to_uuid(\"{file_mod}\"),
                at = 'RETRACT'

            ?[{vector_fields}] := embedded_vec[node_id, embedding_set_id, vector, at]
            :put {vector_set_name}
            }}
            "
        );
        // info!(retract_embedded_files = %script);

        self.raw_query_mut(&script)
            .inspect_err(|_| {
                tracing::error!("using script:\n {}", script);
            })
            .map_err(PlokeError::from)
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
    pub async fn clear_relations(&self) -> Result<(), PlokeError> {
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
    pub async fn clear_hnsw_idx(&self) -> Result<(), PlokeError> {
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
            .filter(|n| n.ends_with(HNSW_SUFFIX));

        for index in rels {
            tracing::debug!(?index);
            let mut script = String::from("::hnsw drop ");
            script.extend(index.chars().filter(|c| *c == '\"'));
            let dropped_hnsw = self
                .db
                .run_script(&script, BTreeMap::new(), cozo::ScriptMutability::Mutable)
                .map_err(DbError::from)?;
            tracing::debug!(?dropped_hnsw);
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
    pub async fn count_relations(&self) -> Result<usize, PlokeError> {
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

    /// Execute a CozoScript query and preserve the Rust callsite + logical query name on errors.
    #[track_caller]
    pub fn raw_query_with_context(&self, ctx: QueryContext) -> Result<QueryResult, DbError> {
        let caller = Location::caller();
        let result = self.db.run_script(
            ctx.script,
            BTreeMap::new(),
            cozo::ScriptMutability::Immutable,
        );

        match result {
            Ok(rows) => Ok(QueryResult::from(rows)),
            Err(err) => Err(DbError::cozo_with_callsite(
                ctx.name,
                err.to_string(),
                caller,
            )),
        }
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
    pub async fn create_new_backup_default(path: impl AsRef<Path>) -> Result<Database, PlokeError> {
        let new_db = cozo::new_cozo_mem().map_err(DbError::from)?;
        new_db.restore_backup(&path).map_err(DbError::from)?;
        Ok(Self {
            db: new_db,
            active_embedding_set: Arc::new(RwLock::new(DEFAULT_EMBEDDING_SET.clone())),
        })
    }
    pub async fn create_new_backup(active_embedding_set: EmbeddingSet, path: impl AsRef<Path>) -> Result<Database, PlokeError> {
        let new_db = cozo::new_cozo_mem().map_err(DbError::from)?;
        new_db.restore_backup(&path).map_err(DbError::from)?;
        Ok(Self {
            db: new_db,
            active_embedding_set: Arc::new(RwLock::new(active_embedding_set)),
        })
    }

    pub fn iter_relations(&self) -> Result<impl IntoIterator<Item = String>, PlokeError> {
        let output = self.raw_query("::relations")?;
        Ok(output.rows.into_iter().filter_map(|r| {
            r.first()
                .into_iter()
                .filter_map(|c| c.get_str().iter().map(|s| s.to_string()).next())
                .next()
        }))
    }
    pub fn relations_vec(&self) -> Result<Vec<String>, PlokeError> {
        let vector = Vec::from_iter(self.iter_relations()?);
        Ok(vector)
    }

    pub fn relations_vec_no_hnsw(&self) -> Result<Vec<String>, PlokeError> {
        let filtered_rels = self
            .iter_relations()?
            .into_iter()
            .filter(|s| s.ends_with(HNSW_SUFFIX));
        let vector = Vec::from_iter(filtered_rels);
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
    pub fn get_path_info(&self, path: &str) -> Result<QueryResult, PlokeError> {
        let ty = NodeType::Module;
        let rel = ty.relation_str();
        let keys: String = ty.keys().join(", ");
        let vals: String = ty.vals().join(", ");
        let script = format!("?[target_path, {keys}, {vals}] := *file_mod{{owner_id: id, file_path: target_path, @ 'NOW' }},
                        *module{{ {keys}, {vals} @ 'NOW' }},
                        target_path = \"{path}\"
        ");
        tracing::info!(target: "file_hashes", "using script\n{}", &script);
        let res = self.raw_query(&script)?;
        Ok(res)
    }
    pub fn get_mod_info(&self, mod_id: Uuid) -> Result<QueryResult, PlokeError> {
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

    pub fn update_embeddings_batch(
        &self,
        updates: Vec<(uuid::Uuid, Vec<f32>)>,
    ) -> Result<(), DbError> {
        if updates.is_empty() {
            return Ok(());
        }
        // Use multi-embedding path; active_embedding_set is validated at Database construction.
        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = self.with_active_set(|set| set.clone())?;
        self.deref().update_embeddings_batch(
            updates
                .into_iter()
                .map(|(id, v)| (id, v.into_iter().map(f32::into).collect::<Vec<f64>>()))
                .collect(),
            &active_embedding_set,
        )
    }

    /// Validate that an embedding vector is non-empty
    // #[cfg( not(feature = "multi_embedding") )]
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
    ) -> Result<Vec<TypedEmbedData>, PlokeError> {
        let mut unembedded_data = Vec::new();
        let mut count = 0;

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = self.with_active_set(|set| set.clone())?;
        // TODO: Awkward. Improve this.
        for t in NodeType::primary_nodes() {
            let nodes_of_type = self.deref().get_unembed_rel(
                t,
                limit.saturating_sub(count),
                cursor,
                active_embedding_set.clone(),
            )?;
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
    ) -> Result<Vec<TypedEmbedData>, PlokeError> {
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
    pub fn get_file_data(&self) -> Result<Vec<FileData>, PlokeError> {
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
        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = self.with_active_set(|set| set.clone())?;
        self.deref().count_unembedded_files(&embedding_set)
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
    // TODO:migrate-multi-embed-full
    // Update callsites to use new API without relying on wrapper function
    pub fn get_nodes_ordered(&self, nodes: Vec<Uuid>) -> Result<Vec<EmbeddingData>, PlokeError> {

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = self.with_active_set(|set| set.clone())?;
        self.deref()
            .get_nodes_ordered_for_set(nodes, &active_embedding_set)
    }

    // TODO:migrate-multi-embed-full
    // Update callsites to use new API without relying on wrapper function
    pub fn get_unembed_rel(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: usize,
    ) -> Result<TypedEmbedData, PlokeError> {
        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = self.with_active_set(|set| set.clone())?;
        self.deref()
            .get_unembed_rel(node_type, limit, cursor, active_embedding_set)
    }

    pub fn get_embed_rel(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: usize,
    ) -> Result<TypedEmbedData, PlokeError> {
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

    // TODO:migrate-multi-embed-full
    // Update callsites to use new API without relying on wrapper function
    pub fn get_rel_with_cursor(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: Uuid,
    ) -> Result<TypedEmbedData, PlokeError> {
        tracing::debug!(
            target: "ploke-db::database",
            rel = %node_type.relation_str(),
            limit,
            cursor = %cursor,
            "delegating get_rel_with_cursor to multi_embedding_db impl",
        );
        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = self.with_active_set(|set| set.clone())?;
        self.deref()
            .get_rel_with_cursor(node_type, limit, cursor, &active_embedding_set)
    }

    /// Gets the primary node typed embed data needed to update the nodes in the database
    /// that are within the given file.
    /// Note that this does not include the module nodes for the files themselves.
    /// This is useful when doing a partial update of the database following change detection in
    /// previously parsed and inserted files.
    // WARN: This needs to be tested
    #[allow(unreachable_code)]
    // TODO:migrate-multi-embed-full
    // Decide if this needs to exist or if it has been fully supplanted
    pub fn get_nodes_by_file_with_cursor(
        &self,
        node_type: NodeType,
        limit: usize,
        cursor: Uuid,
    ) -> Result<TypedEmbedData, PlokeError> {
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

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = self.with_active_set(|set| set.clone())?;
        self.deref()
            .count_unembedded_nonfiles(&active_embedding_set)
    }

    pub fn count_pending_embeddings(&self) -> Result<usize, DbError> {
        use crate::multi_embedding::db_ext::EmbeddingExt;

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = self.with_active_set(|set| set.clone())?;
        let inner_db = &self.db;
        inner_db.count_pending_embeddings(&embedding_set)
    }

    pub fn into_usize(named_rows: NamedRows) -> Result<usize, DbError> {
        named_rows
            .rows
            .first()
            .and_then(|row| row.first())
            .and_then(|v| v.get_int())
            .inspect(|v| trace!("the value in first row, first cell is: {:?}", v))
            .map(|n| n as usize)
            .ok_or(DbError::NotFound)
    }

    pub fn get_pending_test(&self) -> Result<NamedRows, DbError> {
        use crate::multi_embedding::db_ext::EmbeddingExt;
        let mut rows = Vec::new();
        let limit: usize = 10_000;

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = self.with_active_set(|set| set.clone())?;
        for node_type in NodeType::primary_nodes() {
            let typed = self
                .deref()
                .get_rel_with_cursor(node_type, limit, Uuid::nil(), &active_embedding_set)
                .map_err(|e| DbError::QueryExecution(e.to_string()))?;
            for emb in typed.v {
                // Order: [id, at (placeholder), name] to match the indexer's expectations.
                rows.push(vec![
                    DataValue::Uuid(UuidWrapper(emb.id)),
                    DataValue::Str("NOW".into()), // placeholder for 'at' timestamp
                    DataValue::Str(emb.name.clone().into()),
                ]);
            }
        }
        Ok(NamedRows {
            headers: vec!["id".into(), "at".into(), "name".into()],
            rows,
            next: None,
        })
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

    /// Print counts of various information relevent to a given embedding set.
    ///
    /// The printed information shows the debug print, and will not error if the relation is not
    /// found in the database.
    ///
    /// There is also a helper function `debug_print_counts_active` to for convencience to instead
    /// print the database's currently active embedding set.
    pub fn debug_print_counts(&self, embedding_set: &EmbeddingSet) {
        let unembedded_files = self.count_unembedded_files();
        let unembedded_non_files = self.count_unembedded_nonfiles();
        let all_common_nodes = self.count_common_nodes();
        let all_pending = self.count_pending_embeddings();
        let all_emb_complete = self.count_complete_embeddings(embedding_set);
        let all_embedded_rows = self.count_embeddings_for_set(embedding_set);
        debug!(
            r#"Counts:
    unembedded_files = {unembedded_files:?}
    unembedded_non_files = {unembedded_non_files:?}
    all_common_nodes = {all_common_nodes:?}
    all_pending = {all_pending:?}
    all_emb_complete = {all_emb_complete:?}
    all_embedded_rows = {all_embedded_rows:?}

    embedded (all) / non-embedded (common) = {all_embedded_rows:?}/{all_common_nodes:?}"#
        );
    }

    /// Helper function `debug_print_counts_active` to for convencience to instead print the
    /// database's currently active embedding set.
    ///
    /// See `debug_print_counts` for the more general version that accepts a given embedding set.
    pub fn debug_print_counts_active(&self) {
        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let active_embedding_set = self.with_active_set(|set| set.clone()).unwrap();
        self.debug_print_counts(&active_embedding_set);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::bm25_index::DocData;
    use crate::multi_embedding::schema::CozoEmbeddingSetExt;
    use crate::Database;
    use crate::DbError;
    use cozo::{Db, MemStorage, ScriptMutability};
    use ploke_transform::schema::create_schema_all;
    use tracing::error;
    use tracing::info;
    use tracing::trace;
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

    #[test]
    fn update_embeddings_batch_empty() -> Result<(), DbError> {
        let db = setup_db();
        db.update_embeddings_batch(vec![])?;
        // Should not panic/error with empty input
        Ok(())
    }

    #[tokio::test]
    // ASDF
    async fn test_get_file_data() -> Result<(), PlokeError> {
        // Initialize the logger to see output from Cozo
        let db = Database::new(ploke_test_utils::setup_db_full_multi_embedding(
            "fixture_nodes",
        )?);

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
    async fn test_count_nodes_for_embedding() -> Result<(), PlokeError> {
        // ploke_test_utils::init_test_tracing_with_target("cozo-script", Level::DEBUG);
        let db = Database::new(ploke_test_utils::setup_db_full_multi_embedding(
            "fixture_nodes",
        )?);
        let count = db.count_pending_embeddings()?;
        tracing::info!("Found {} nodes without embeddings", count);
        Ok(())
    }

    #[tokio::test]
    async fn test_get_nodes_two() -> Result<(), PlokeError> {
        // Initialize the logger to see output from Cozo
        // ploke_test_utils::init_test_tracing_with_target("cozo-script", Level::INFO);
        let db = Database::new(ploke_test_utils::setup_db_full_multi_embedding(
            "fixture_nodes",
        )?);

        let count1 = db.count_pending_embeddings()?;
        tracing::debug!("Found {} nodes without embeddings", count1);
        assert_ne!(0, count1);

        Ok(())
    }

    #[tokio::test]
    async fn test_get_nodes_for_embedding() -> Result<(), PlokeError> {
        // ploke_test_utils::init_test_tracing(Level::ERROR);
        // Initialize the logger to see output from Cozo
        let db = Database::new(ploke_test_utils::setup_db_full_multi_embedding(
            "fixture_nodes",
        )?);
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
    /// Test the presence of tracking hashes for different groups of nodes.
    ///
    /// Expects all common node items to individually have tracking hashes
    async fn test_get_anynode_th() -> Result<(), PlokeError> {
        use crate::multi_embedding::debug::DebugAll;
        // ploke_test_utils::init_test_tracing(Level::INFO);

        let db = Database::new(ploke_test_utils::setup_db_full_multi_embedding(
            "fixture_nodes",
        )?);
        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let emb_set = db.with_active_set(|set| set.clone())?;
        db.is_embedding_info_all(&emb_set)?;
        let common_query_result = db.get_common_nodes()?;
        let common_nodes_count = common_query_result.rows.len();
        let tracking_hashes: Vec<Uuid> = common_query_result
            .iter_col("tracking_hash")
            .expect("expect nodes have tracking hash")
            .map(to_uuid)
            .try_collect()?;
        let node_ids: Vec<Uuid> = common_query_result
            .iter_col("id")
            .expect("expect common nodes to have a node id (id)")
            .map(to_uuid)
            .try_collect()?;
        let mut unique_common_hashes: HashSet<Uuid> = HashSet::new();
        for node_id in node_ids {
            let anynode_th = db
                .get_anynode_th(node_id)
                .expect("all common nodes should have a tracking hash");
            let is_unique = unique_common_hashes.insert(anynode_th);
            assert!(is_unique, "expect all common hashes to be unique");
        }
        let unique_common_count = unique_common_hashes.len();
        assert_eq!(
            common_nodes_count, unique_common_count,
            "Expect common items to all have tracking hashes"
        );
        info!(target: TRACKING_HASH_TARGET, "All {unique_common_count}/{common_nodes_count} common nodes are unique and have tracking_hash as expected");

        let rels_with_th_rhs = db.rel_names_with_tracking_hash()?;
        info!(target: TRACKING_HASH_TARGET, "relations with tracking_hash: {rels_with_th_rhs:#?}");
        Ok(())
    }

    #[tokio::test]
    async fn test_unembedded_counts() -> Result<(), PlokeError> {
        // ploke_test_utils::init_test_tracing(Level::TRACE);
        // Initialize the logger to see output from Cozo
        let db = Database::new(ploke_test_utils::setup_db_full_multi_embedding(
            "fixture_nodes",
        )?);
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

        db.update_embeddings_batch(vec![(id, embedding.clone())])?;

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
    // #[ignore = "Needs to use new callback method"]
    async fn test_update_embeddings_batch() -> Result<(), PlokeError> {
        // ploke_test_utils::init_test_tracing(Level::DEBUG);
        // 1. Setup the database with a fixture
        let db = Database::new(ploke_test_utils::setup_db_full_multi_embedding(
            "fixture_nodes",
        )?);

        // 2. Get initial state
        let initial_count = db.count_unembedded_nonfiles()?;
        tracing::info!(?initial_count);
        assert!(initial_count > 0, "Fixture should have unembedded nodes");

        // 3. Get a batch of nodes to update
        let nodes_to_update = db.get_unembedded_node_data(10, 0)?;
        let nodes_to_update = nodes_to_update
            .iter()
            .flat_map(|emb| emb.v.iter())
            .collect_vec();
        let update_count = nodes_to_update.len();
        tracing::info!(?update_count);
        assert!(update_count > 0, "Should retrieve some nodes to update");
        assert!(update_count <= 10);

        // 4. Create mock embeddings for the batch
        let updates: Vec<(uuid::Uuid, Vec<f64>)> = nodes_to_update
            .into_iter()
            .map(|node| (node.id, vec![1.0; 384]))
            .collect();

        // 5. Call the function to update the batch

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = db.with_active_set(|set| set.clone())?;
        let relation_name = embedding_set.rel_name.clone();
        if !db.deref().is_relation_registered(&relation_name)? {
            use crate::multi_embedding::schema::EmbeddingVector;

            let create_rel_script = EmbeddingVector::script_create_from_set(&embedding_set);
            db.run_script(
                &create_rel_script,
                BTreeMap::new(),
                ScriptMutability::Mutable,
            )
            .map_err(DbError::from)?;
        }
        db.deref()
            .update_embeddings_batch(updates, &embedding_set)?;
        // assert_eq!(update_count, updated_ct);

        // 6. Verify the update
        let final_count = db.count_unembedded_nonfiles()?;
        tracing::info!(?initial_count);
        tracing::info!(?update_count);
        tracing::info!(?final_count);
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
                "?[id, tracking_hash, tokenizer_version, token_length] := *bm25_doc_meta{id, tracking_hash, tokenizer_version, token_length}
                    :sort token_length",
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

        // Verify the second document
        eprintln!("{:?}", result.rows[0]);
        if let DataValue::Num(cozo::Num::Int(token_length)) = result.rows[0][3] {
            assert_eq!(token_length, 42);
        } else {
            panic!("Expected Int DataValue for token_length");
        }

        eprintln!("{:?}", result.rows[1]);
        if let DataValue::Num(cozo::Num::Int(token_length)) = result.rows[1][3] {
            assert_eq!(token_length, 128);
        } else {
            panic!("Expected Int DataValue for token_length");
        }

        Ok(())
    }

    fn debug_print_counts(db: &Database) -> Result<(), PlokeError> {
        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = db.with_active_set(|set| set.clone())?;

        let unembedded_files = db.count_unembedded_files()?;
        let unembedded_non_files = db.count_unembedded_nonfiles()?;
        let all_pending = db.count_pending_embeddings()?;
        let all_emb_complete = db.count_complete_embeddings(&embedding_set)?;
        let all_embedded_rows = db.count_embeddings_for_set(&embedding_set)?;
        debug!(
            "Counts:
            unembedded_files = {unembedded_files}
            unembedded_non_files = {unembedded_non_files}
            all_pending = {all_pending}
            all_emb_complete = {all_emb_complete}
            all_embedded_rows = {all_embedded_rows}
        "
        );
        Ok(())
    }
    #[tokio::test]
    async fn test_retract_embedding_single() -> Result<(), PlokeError> {
        // ploke_test_utils::init_test_tracing_with_target("cozo-script", Level::ERROR);
        let cozo_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")?;
        let db = Database::new(cozo_db);
        crate::multi_embedding::db_ext::load_db(&db, "fixture_nodes".to_string()).await?;
        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = db.with_active_set(|set| set.clone())?;

        debug_print_counts(&db)?;

        crate::create_index_primary(&db)?;
        let emb_rows_script = embedding_set.script_get_vector_rows();
        info!(%emb_rows_script);
        let nr = db
            .run_script(
                &emb_rows_script,
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .map_err(DbError::from)?;
        info!(?nr.headers);
        // let printable_all_rows = rows.rows.into_iter().map(|r| format!("{r:?}")).join("\n");
        // info!(%printable_all_rows);

        let row = nr.rows.first().unwrap();
        info!(?row);
        let id = row.first().unwrap().clone();
        info!(?id);
        let uid = to_uuid(&id)?;
        let embedding_rel_name = embedding_set.rel_name();
        let retract_script = format!(
            r#"
    ?[node_id, embedding_set_id, vector, at] := *{embedding_rel_name}{{node_id, embedding_set_id, vector}},
        node_id = to_uuid("{uid}"), at = 'RETRACT'

        :put {embedding_rel_name}{{node_id, embedding_set_id, vector, at}}
"#
        );
        info!(%retract_script);
        db.run_script(&retract_script, BTreeMap::new(), ScriptMutability::Mutable)
            .map_err(DbError::from)?;

        debug_print_counts(&db)?;

        Ok(())
    }
    #[tokio::test]
    async fn test_retract_embeddings_full() -> Result<(), PlokeError> {
        // ploke_test_utils::init_test_tracing_with_target("cozo-script", Level::ERROR);
        let cozo_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")?;
        let db = Database::new(cozo_db);
        crate::multi_embedding::db_ext::load_db(&db, "fixture_nodes".to_string()).await?;
        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = db.with_active_set(|set| set.clone())?;

        // debug_print_counts(&db)?;
        crate::create_index_primary(&db)?;
        // debug_print_counts(&db)?;

        let initial_embedded = db.count_embeddings_for_set(&embedding_set)?;
        assert!(
            initial_embedded > 0,
            "expect all embeddings present initially"
        );

        let file_data: Vec<FileData> = db.get_file_data()?;
        info!("Files count with db.get_file_data: {}", file_data.len());
        info!("Loop through file data");

        for (i, file_mod_id) in file_data.iter().map(|f| f.id).enumerate() {
            info!("Looping {i}");
            for node_ty in NodeType::primary_nodes() {
                trace!("Looping {i} -- node_ty: {}", node_ty.relation_str());
                let query_res = db
                    .retract_embedded_files(file_mod_id, node_ty)
                    .inspect_err(|e| error!("Error in retract_embed_files: {e}"))?;
                trace!("Raw return of retract_embedded_files:\n{:?}", query_res);
                if !query_res.rows.is_empty() {
                    let to_print = query_res
                        .rows
                        .iter()
                        .map(|r| r.iter().join(" | "))
                        .join("\n");
                    // info!("Return of retract_embedded_files:\n{}", to_print);
                }
            }
        }

        let final_embedded = db.count_embeddings_for_set(&embedding_set)?;
        assert!(
            final_embedded == 0,
            "expect no registered embeddings after retracting all of them"
        );

        // debug_print_counts(&db)?;
        Ok(())
    }

    #[tokio::test]
    async fn test_retract_embeddings_partial() -> Result<(), PlokeError> {
        // crate::multi_embedding::hnsw_ext::init_tracing_once("cozo-script", Level::DEBUG);
        let cozo_db = ploke_test_utils::setup_db_full_multi_embedding("fixture_nodes")?;
        let db = Database::new(cozo_db);
        crate::multi_embedding::db_ext::load_db(&db, "fixture_nodes".to_string()).await?;

        // TODO:active-embedding-set 2025-12-15
        // update the active embedding set functions to correctly use Arc<RwLock<>> within these
        // functions.
        let embedding_set = db.with_active_set(|set| set.clone())?;

        debug_print_counts(&db)?;
        db.clear_hnsw_idx().await?;
        debug_print_counts(&db)?;
        crate::create_index_primary_with_index(&db)?;
        debug_print_counts(&db)?;

        let initial_embedded = db.count_embeddings_for_set(&embedding_set)?;
        assert!(
            initial_embedded > 0,
            "expect all embeddings present initially"
        );

        let file_data: Vec<FileData> = db.get_file_data()?;
        debug!("Files count with db.get_file_data: {}", file_data.len());
        debug!("Loop through file data");

        for (i, file_mod_id) in file_data.iter().map(|f| f.id).enumerate().step_by(2) {
            debug!("Looping {i}");
            for node_ty in NodeType::primary_nodes() {
                trace!("Looping {i} -- node_ty: {}", node_ty.relation_str());
                let query_res = db
                    .retract_embedded_files(file_mod_id, node_ty)
                    .inspect_err(|e| error!("Error in retract_embed_files: {e}"))?;
                trace!("Raw return of retract_embedded_files:\n{:?}", query_res);
                if !query_res.rows.is_empty() {
                    let to_print = query_res
                        .rows
                        .iter()
                        .map(|r| r.iter().join(" | "))
                        .join("\n");
                    // debug!("Return of retract_embedded_files:\n{}", to_print);
                }
            }
        }

        let partial_embedded = db.count_embeddings_for_set(&embedding_set)?;
        assert!(
            initial_embedded > partial_embedded,
            "expect fewer embeddings after retracting some of them"
        );
        assert!(partial_embedded > 0, "expect some embeddings remain");

        for (i, file_mod_id) in file_data
            .iter()
            .map(|f| f.id)
            .enumerate()
            .skip(1)
            .step_by(2)
        {
            debug!("Looping {i}");
            for node_ty in NodeType::primary_nodes() {
                trace!("Looping {i} -- node_ty: {}", node_ty.relation_str());
                let query_res = db
                    .retract_embedded_files(file_mod_id, node_ty)
                    .inspect_err(|e| error!("Error in retract_embed_files: {e}"))?;
                trace!("Raw return of retract_embedded_files:\n{:?}", query_res);
                if !query_res.rows.is_empty() {
                    let to_print = query_res
                        .rows
                        .iter()
                        .map(|r| r.iter().join(" | "))
                        .join("\n");
                    // debug!("Return of retract_embedded_files:\n{}", to_print);
                }
            }
        }

        let ending_embedded = db.count_embeddings_for_set(&embedding_set)?;
        assert!(
            initial_embedded > ending_embedded,
            "expect fewer embeddings after retracting some of them"
        );
        assert!(
            partial_embedded > ending_embedded,
            "expect fewer embeddings after retracting more of them"
        );
        assert!(
            ending_embedded == 0,
            "expect no embeddings remain after retracting all"
        );

        // debug_print_counts(&db)?;
        Ok(())
    }
}
