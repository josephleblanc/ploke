use crate::{Database, DbError, NodeType, QueryResult, TypedEmbedData};
use std::collections::BTreeMap;

use cozo::{DataValue, Num, ScriptMutability};
use itertools::Itertools;
use tracing::instrument;

use crate::database::HNSW_SUFFIX;
use crate::multi_embedding::hnsw_ext::HnswExt;

fn arr_to_float(arr: &[f32]) -> DataValue {
    DataValue::List(
        arr.iter()
            .map(|f| DataValue::Num(Num::Float(*f as f64)))
            .collect_vec(),
    )
}

type Embedding = (uuid::Uuid, String, DataValue);

pub fn hnsw_all_types(
    db: &Database,
    k: usize,
    ef: usize,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    let mut results = Vec::new();
    for ty in NodeType::primary_nodes() {
        let ty_ret: Vec<Embedding> = hnsw_of_type(db, ty, k, ef)?;
        results.extend(ty_ret);
    }
    Ok(results)
}

pub fn run_script_warn(
    db: &Database,
    script: &str,
    rel_params: BTreeMap<String, DataValue>,
    mutability: ScriptMutability,
) -> Result<cozo::NamedRows, ploke_error::Error> {
    match db.run_script(script, rel_params, mutability) {
        Ok(r) => Ok(r),
        Err(e) => {
            if e.to_string()
                .contains("Index hnsw_idx not found on relation")
            {
                Err(ploke_error::Error::Warning(
                    ploke_error::WarningError::PlokeDb(e.to_string()),
                ))
            } else {
                Err(DbError::Cozo(e.to_string()).into())
            }
        }
    }
}

// TODO:migrate-multi-embed-full
// Update the call sites to use new API
pub fn hnsw_of_type(
    db: &Database,
    ty: NodeType,
    k: usize,
    ef: usize,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active_embedding_set = db.with_active_set(|set| set.clone())?;
    db.hnsw_neighbors_for_type(ty, &active_embedding_set, k, ef)
}

// TODO:migrate-multi-embed-full
// Update the call sites to use new API
#[instrument(skip_all, fields(query_result))]
pub fn search_similar(
    db: &Database,
    vector_query: Vec<f32>,
    k: usize,
    ef: usize,
    ty: NodeType,
) -> Result<TypedEmbedData, ploke_error::Error> {
    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active_embedding_set = db.with_active_set(|set| set.clone())?;
    db.search_similar_for_set(&active_embedding_set, ty, vector_query, k, ef, 100, None)
        .map(|res| res.typed_data)
}

#[derive(Clone)]
pub struct SimilarArgs<'a> {
    pub db: &'a Database,
    pub vector_query: &'a Vec<f32>,
    pub k: usize,
    pub ef: usize,
    pub ty: NodeType,
    pub max_hits: usize,
    pub radius: f64,
}

#[instrument(skip_all, fields(query_result))]
pub fn search_similar_args(args: SimilarArgs) -> Result<EmbedDataVerbose, ploke_error::Error> {
    let SimilarArgs {
        db,
        vector_query,
        k,
        ef,
        ty,
        max_hits,
        radius,
    } = args;
    use tracing::info;
    info!("running search_similar args");

    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active_embedding_set = db.with_active_set(|set| set.clone())?;
    db.search_similar_for_set(
        &active_embedding_set,
        ty,
        vector_query.clone(),
        k,
        ef,
        max_hits,
        Some(radius),
    )
}

#[derive(Debug, Clone)]
pub struct EmbedDataVerbose {
    pub typed_data: TypedEmbedData,
    pub dist: Vec<f64>,
}

pub fn search_similar_test(
    db: &Database,
    k: usize,
    ef: usize,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    let mut params = std::collections::BTreeMap::new();
    params.insert("k".to_string(), DataValue::from(k as i64));
    params.insert("ef".to_string(), DataValue::from(ef as i64));

    let hnsw_script = [
        r#"
            ?[id, name, distance] := 
                *function{
                    id, 
                    name, 
                    embedding: v
                    @ 'NOW'
                },
                ~function"#,
        HNSW_SUFFIX,
        r#"{id, name| 
                    query: v, 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance
                }
            "#,
    ]
    .concat();
    let result = db
        .run_script(&hnsw_script, params, ScriptMutability::Immutable)
        .map_err(DbError::from)?;

    let mut results = Vec::new();
    for row in result.rows {
        tracing::trace!("{:?}", row);
        let id = if let DataValue::Uuid(cozo::UuidWrapper(id)) = row[0] {
            tracing::trace!("{:?}", id);
            id
        } else {
            uuid::Uuid::max()
        };
        let content = row[1].get_str().unwrap().to_string();
        results.push((id, content, row[2].clone()));
    }

    Ok(results)
}

// TODO:migrate-multi-embed-full
// Update the call sites to use new API
pub fn create_index(db: &Database, ty: NodeType) -> Result<(), DbError> {
    let _ = ty;

    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active_embedding_set = db.with_active_set(|set| set.clone())?;
    db.create_embedding_index(&active_embedding_set)
}

// TODO:docs Add doc comments
pub fn create_index_primary(db: &Database) -> Result<(), DbError> {
    use crate::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt};

    let r1 = db.ensure_embedding_set_relation();
    tracing::info!(create_embedding_set_relation = ?r1);
    r1.unwrap_or_else(|_| panic!());

    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active_embedding_set = db.with_active_set(|set| set.clone())?;
    let r2 = db.ensure_embedding_relation(&active_embedding_set);
    tracing::info!(ensure_embedding_relation = ?r2);
    r2.unwrap_or_else(|_| panic!());

    let r3 = db.ensure_vector_embedding_relation(&active_embedding_set);
    tracing::info!(ensure_vector_embedding_relation = ?r3);
    r3.unwrap_or_else(|_| panic!());
    Ok(())
}

// TODO:docs Add doc comments
// TODO:ploke-db 2025-12-16
// Replace this function with a Database method
pub fn create_index_primary_with_index(db: &Database) -> Result<(), DbError> {
    use crate::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt};

    create_index_primary(db)?;
    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active_embedding_set = db.with_active_set(|set| set.clone())?;
    tracing::debug!(?active_embedding_set);
    db.create_embedding_index(&active_embedding_set.clone())
}

/// Temporary wrapper function to replace current API
// TODO:migrate-multi-embed-full
// Update the call sites to use new API
pub fn create_index_warn(db: &Database) -> Result<(), ploke_error::Error> {
    use crate::multi_embedding::hnsw_ext::HnswExt;

    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active_embedding_set = db.with_active_set(|set| set.clone())?;
    db.create_embedding_index(&active_embedding_set)
        .map_err(ploke_error::Error::from)
}

// TODO:migrate-multi-embed-full
// Update the call sites to use new API
pub fn replace_index_warn(db: &Database, ty: NodeType) -> Result<(), ploke_error::Error> {
    use crate::multi_embedding::hnsw_ext::HnswExt;

    let _ = ty;
    // TODO:active-embedding-set 2025-12-15
    // update the active embedding set functions to correctly use Arc<RwLock<>> within these
    // functions.
    let active_embedding_set = db.with_active_set(|set| set.clone())?;
    db.create_embedding_index(&active_embedding_set)
        .map_err(ploke_error::Error::from)
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::create_index_primary;
    use crate::hnsw_all_types;
    use crate::utils::test_utils::TEST_DB_NODES;
    use crate::DbError;
    use ploke_test_utils::workspace_root;
    use tokio::sync::Mutex;

    use lazy_static::lazy_static;
    use ploke_error::Error;
    use tokio_test::assert_err;

    use crate::Database;

    #[tokio::test]
    async fn test_hnsw_init_from_backup() -> Result<(), Error> {
        let db = Database::init_with_schema()?;

        let mut target_file = workspace_root();
        target_file.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
        eprintln!("Loading backup db from file at:\n{}", target_file.display());
        let prior_rels_vec = db.relations_vec()?;
        db.import_from_backup(&target_file, &prior_rels_vec)
            .map_err(DbError::from)
            .map_err(ploke_error::Error::from)?;

        super::create_index_primary_with_index(&db)?;

        let k = 20;
        let ef = 40;
        hnsw_all_types(&db, k, ef)?;
        let unembedded = db.count_unembedded_nonfiles()?;
        println!("unembedded: {unembedded}");
        let embedded = db.count_pending_embeddings()?;
        println!("embedded: {embedded}");
        Ok(())
    }

    #[tokio::test]
    async fn test_hnsw_init_from_backup_error() -> Result<(), Error> {
        let db = Database::init_with_schema()?;

        let mut target_file = workspace_root();
        target_file.push("tests/backup_dbs/fixture_nodes_bfc25988-15c1-5e58-9aa8-3d33b5e58b92");
        eprintln!("Loading backup db from file at:\n{}", target_file.display());
        let prior_rels_vec = db.relations_vec()?;
        db.import_from_backup(&target_file, &prior_rels_vec)
            .map_err(DbError::from)
            .map_err(ploke_error::Error::from)?;

        // Note: purposefully commented out to cause failure.
        // super::create_index_primary(&db)?;

        let k = 20;
        let ef = 40;
        let e = hnsw_all_types(&db, k, ef);
        assert_err!(e.clone());
        let err_msg = String::from("Database error: Index hnsw_idx not found on relation function");
        let expect_err = ploke_error::Error::Warning(ploke_error::WarningError::PlokeDb(err_msg));
        let actual_err = e.clone().expect_err("expect error");
        assert!(matches!(actual_err, ploke_error::Error::Warning(_)));
        Ok(())
    }
}
