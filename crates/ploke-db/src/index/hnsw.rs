use crate::database::{HNSW_SUFFIX, to_uuid};
use crate::{Database, DbError, NodeType, QueryResult, TypedEmbedData};
use std::collections::BTreeMap;

use cozo::{DataValue, NamedRows, Num, ScriptMutability};
use itertools::Itertools;
use tracing::instrument;

#[cfg(feature = "multi_embedding_db")]
use crate::multi_embedding::HnswDistance;
#[cfg(feature = "multi_embedding_db")]
use crate::multi_embedding::adapter::ExperimentalEmbeddingDatabaseExt;
#[cfg(feature = "multi_embedding_db")]
use crate::multi_embedding::schema::metadata::ExperimentalRelationSchemaDbExt;
#[cfg(feature = "multi_embedding_db")]
use crate::multi_embedding::schema::node_specs::experimental_spec_for_node;
#[cfg(feature = "multi_embedding_db")]
use crate::multi_embedding::schema::vector_dims::vector_dimension_specs;
#[cfg(feature = "multi_embedding_db")]
use crate::multi_embedding::vectors::ExperimentalVectorRelation;

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
            let db_err = DbError::from(e);
            let message = db_err.to_string();
            if message.contains("Index") && message.contains("not found") {
                Err(ploke_error::Error::Warning(
                    ploke_error::WarningError::PlokeDb(message),
                ))
            } else {
                Err(ploke_error::Error::from(db_err))
            }
        }
    }
}

pub fn hnsw_of_type(
    db: &Database,
    ty: NodeType,
    k: usize,
    ef: usize,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    #[cfg(feature = "multi_embedding_db")]
    if db.multi_embedding_db_enabled() {
        return multi_embedding_hnsw_of_type(db, ty, k, ef);
    }
    let mut params = std::collections::BTreeMap::new();
    let rel = ty.relation_str();
    params.insert("k".to_string(), DataValue::from(k as i64));
    params.insert("ef".to_string(), DataValue::from(ef as i64));
    params.insert("rel".to_string(), DataValue::from(rel));

    let result = [
        r#"
            ?[id, name, distance] := 
                *"#,
        rel,
        r#"{
                    id, 
                    name, 
                    embedding: v
                    @ 'NOW'
                },
                !is_null(v),
                ~"#,
        rel,
        HNSW_SUFFIX,
        r#"{id, name | 
                    query: v, 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance,
                }
            "#,
    ]
    .join("");
    let result = match db.run_script(&result, params, ScriptMutability::Immutable) {
        Ok(r) => Ok(r),
        Err(e) => {
            if e.to_string()
                .contains("Index hnsw_idx not found on relation const")
            {
                Err(ploke_error::Error::Warning(
                    ploke_error::WarningError::PlokeDb(e.to_string()),
                ))
            } else {
                Err(DbError::Cozo(e.to_string()).into())
            }
        }
    }?;

    Ok(named_rows_to_embeddings(result))
}

#[instrument(skip_all, fields(query_result))]
pub fn search_similar(
    db: &Database,
    vector_query: Vec<f32>,
    k: usize,
    ef: usize,
    ty: NodeType,
) -> Result<TypedEmbedData, ploke_error::Error> {
    let mut params = std::collections::BTreeMap::new();
    params.insert("k".to_string(), DataValue::from(k as i64));
    params.insert("ef".to_string(), DataValue::from(ef as i64));
    params.insert("limit".to_string(), DataValue::from(100_i64));
    params.insert(
        "vector_query".to_string(),
        DataValue::List(
            vector_query
                .into_iter()
                .map(|fl| {
                    if (fl as f64).is_subnormal() {
                        1.0
                    } else {
                        fl as f64
                    }
                })
                .map(|fl| DataValue::Num(Num::Float(fl)))
                .collect_vec(),
        ),
    );

    let mut script = String::new();
    let base_script_start = r#"
    parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}

    ancestor[desc, asc] := parent_of[desc, asc]
    ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

    has_embedding[id, name, hash, span] := *"#;
    let base_script_end = r#" {id, name, tracking_hash: hash, span, embedding @ 'NOW' }, !is_null(embedding)

    is_root_module[id] := *module{id @ 'NOW' }, *file_mod {owner_id: id @ 'NOW'}

    batch[id, name, file_path, file_hash, hash, span, namespace] := 
        has_embedding[id, name, hash, span],
        ancestor[id, mod_id],
        is_root_module[mod_id],
        *module{id: mod_id, tracking_hash: file_hash @ 'NOW'},
        *file_mod { owner_id: mod_id, file_path, namespace @ 'NOW'},

    ?[id, name, file_path, file_hash, hash, span, namespace, distance] := 
        batch[id, name, file_path, file_hash, hash, span, namespace],
     "#;
    let hnsw_script = [
        r#"
            ?[id, name, distance] := 
                *function{
                    id, 
                    name, 
                    @ 'NOW'
                },
                ~function"#,
        HNSW_SUFFIX,
        r#"{id, name| 
                    query: vec($vector_query), 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance
                }
            "#,
    ];
    let limit_param = ":limit $limit";

    let rel = ty.relation_str();
    script.push_str(base_script_start);
    script.push_str(rel);
    script.push_str(base_script_end);

    tracing::trace!("script for similarity search is: {}", script);
    let query_result = db
        .run_script(&script, params, cozo::ScriptMutability::Immutable)
        .inspect_err(|e| tracing::error!("{e}"))
        .map_err(|e| DbError::Cozo(e.to_string()))?;

    let less_flat_row = query_result.rows.first();
    let count_less_flat = query_result.rows.len();
    if let Some(lfr) = less_flat_row {
        tracing::trace!(
            "\n{:=^80}\n== less_flat: {count_less_flat} ==\n== less_flat: {less_flat_row:?} ==\n",
            rel
        );
    }
    let v = QueryResult::from(query_result).to_embedding_nodes()?;
    let ty_embed = TypedEmbedData { v, ty };
    Ok(ty_embed)
}

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
    let mut params = std::collections::BTreeMap::new();
    params.insert("k".to_string(), DataValue::from(k as i64));
    params.insert("ef".to_string(), DataValue::from(ef as i64));
    params.insert("limit".to_string(), DataValue::from(max_hits as i64));
    params.insert("radius".to_string(), DataValue::from(radius));
    params.insert(
        "vector_query".to_string(),
        DataValue::List(
            vector_query
                .iter()
                // .into_iter()
                .map(|fl| {
                    if (*fl as f64).is_subnormal() {
                        0.0
                    } else {
                        *fl as f64
                    }
                })
                .map(|fl| DataValue::Num(Num::Float(fl)))
                .collect_vec(),
        ),
    );
    let rel = ty.relation_str();

    let mut script = String::new();
    let base_script_start = r#"
    parent_of[child, parent] := *syntax_edge{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}

    ancestor[desc, asc] := parent_of[desc, asc]
    ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

    has_embedding[id, name, hash, span] := *"#;
    let base_script_end = r#" {id, name, tracking_hash: hash, span, embedding @ 'NOW' }, !is_null(embedding)

    is_root_module[id] := *module{id @ 'NOW' }, *file_mod {owner_id: id @ 'NOW'}

    batch[id, name, file_path, file_hash, hash, span, namespace] := 
        has_embedding[id, name, hash, span],
        ancestor[id, mod_id],
        is_root_module[mod_id],
        *module{id: mod_id, tracking_hash: file_hash @ 'NOW'},
        *file_mod { owner_id: mod_id, file_path, namespace @ 'NOW'},

    ?[id, name, file_path, file_hash, hash, span, namespace, distance] := 
        batch[id, name, file_path, file_hash, hash, span, namespace],
     "#;
    // ?[id, name, distance] :=
    let hnsw_script = [
        r#"
                *"#,
        rel,
        r#"{
                    id, 
                    name, 
                    @ 'NOW'
                },
                ~"#,
        rel,
        HNSW_SUFFIX,
        r#"{id, name| 
                    query: vec($vector_query), 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance,
                    radius: $radius
                },
                :order distance
            "#,
    ];
    let limit_param = ":limit $limit";

    script.push_str(base_script_start);
    script.push_str(rel);
    script.push_str(base_script_end);
    script.push_str(&hnsw_script.into_iter().collect::<String>());
    script.push_str(limit_param);

    tracing::trace!("script for similarity search is: {}", script);
    let query_result = db
        .run_script(&script, params, cozo::ScriptMutability::Immutable)
        .inspect_err(|e| tracing::error!("{e}"))
        .map_err(|e| DbError::Cozo(e.to_string()))?;

    let less_flat_row = query_result.rows.first();
    let count_less_flat = query_result.rows.len();
    if let Some(lfr) = less_flat_row {
        tracing::trace!(
            "\n{:=^80}\n== less_flat: {count_less_flat} ==\n== less_flat: {less_flat_row:?} ==\n",
            rel
        );
    }
    let mut dist_vec = Vec::new();
    if !query_result.rows.is_empty() {
        tracing::trace!("query_result.headers: {:?}", query_result.headers);
        let dist_idx = query_result
            .headers
            .iter()
            .enumerate()
            .find(|(idx, s)| *s == "distance")
            .map(|(idx, _)| idx)
            .expect("Must return `distance` in database return values");
        let dist_floats = query_result
            .rows
            .iter()
            .filter_map(|r| r[dist_idx].get_float());
        dist_vec.extend(dist_floats);
    }
    let v = QueryResult::from(query_result).to_embedding_nodes()?;
    let ty_embed = TypedEmbedData { v, ty };
    let verbose_embed = EmbedDataVerbose {
        typed_data: ty_embed,
        dist: dist_vec,
    };
    Ok(verbose_embed)
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

fn named_rows_to_embeddings(rows: NamedRows) -> Vec<Embedding> {
    rows.rows
        .into_iter()
        .filter_map(|row| {
            let id = to_uuid(&row[0]).ok()?;
            let name = row[1].get_str().unwrap_or_default().to_string();
            Some((id, name, row[2].clone()))
        })
        .collect()
}

#[cfg(feature = "multi_embedding_db")]
fn multi_embedding_hnsw_of_type(
    db: &Database,
    ty: NodeType,
    k: usize,
    ef: usize,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    let Some(spec) = experimental_spec_for_node(ty) else {
        return Ok(Vec::new());
    };
    spec.metadata_schema
        .ensure_registered(db)
        .map_err(ploke_error::Error::from)?;
    let mut aggregated = Vec::new();
    for dim_spec in vector_dimension_specs() {
        let relation = ExperimentalVectorRelation::new(dim_spec.dims(), spec.vector_relation_base);
        relation
            .ensure_registered(db)
            .map_err(ploke_error::Error::from)?;
        let script = format!(
            r#"
?[node_id, name, distance] :=
    *{vector_rel}{{ node_id, vector: v @ 'NOW' }},
    ~{vector_rel}:vector_idx{{ node_id |
        query: v,
        k: {k},
        ef: {ef},
        bind_distance: distance
    }},
    *{node_rel}{{ id: node_id, name @ 'NOW' }}
"#,
            vector_rel = relation.relation_name(),
            node_rel = ty.relation_str(),
            k = k,
            ef = ef,
        );
        match db.run_script(&script, BTreeMap::new(), ScriptMutability::Immutable) {
            Ok(rows) => aggregated.extend(named_rows_to_embeddings(rows)),
            Err(err) => {
                let db_err = DbError::from(err);
                let message = db_err.to_string();
                if message.contains("Index") && message.contains("not found") {
                    continue;
                } else {
                    return Err(ploke_error::Error::from(db_err));
                }
            }
        }
    }
    Ok(aggregated)
}

#[cfg(feature = "multi_embedding_db")]
fn create_multi_embedding_indexes_for_type(db: &Database, ty: NodeType) -> Result<(), DbError> {
    if !db.multi_embedding_db_enabled() {
        return Ok(());
    }
    let Some(spec) = experimental_spec_for_node(ty) else {
        return Ok(());
    };
    spec.metadata_schema.ensure_registered(db)?;
    for dim_spec in vector_dimension_specs() {
        let relation = ExperimentalVectorRelation::new(dim_spec.dims(), spec.vector_relation_base);
        relation.ensure_registered(db)?;
        if let Err(err) = db.create_idx(
            &relation.relation_name(),
            dim_spec.dims(),
            dim_spec.hnsw_m(),
            dim_spec.hnsw_ef_construction(),
            HnswDistance::L2,
        ) {
            let msg = err.to_string();
            if msg.contains("already exists") {
                continue;
            } else {
                return Err(err);
            }
        }
    }
    Ok(())
}

pub fn create_index(db: &Database, ty: NodeType) -> Result<(), DbError> {
    #[cfg(feature = "multi_embedding_db")]
    create_multi_embedding_indexes_for_type(db, ty)?;

    // Create documents table
    // Create HNSW index on embeddings
    let script = [
        r#"
            ::hnsw create "#,
        ty.relation_str(),
        HNSW_SUFFIX,
        r#" {
                fields: [embedding],
                dim: 384,
                dtype: F32,
                m: 32,
                ef_construction: 200,
                distance: L2
            }
            "#,
    ]
    .join("");
    db.run_script(
        &script,
        std::collections::BTreeMap::new(),
        ScriptMutability::Mutable,
    )?;

    Ok(())
}

pub fn create_index_primary(db: &Database) -> Result<(), DbError> {
    for ty in NodeType::primary_nodes() {
        create_index(db, ty)?;
    }
    Ok(())
}

pub fn create_index_warn(db: &Database, ty: NodeType) -> Result<(), ploke_error::Error> {
    #[cfg(feature = "multi_embedding_db")]
    create_multi_embedding_indexes_for_type(db, ty).map_err(ploke_error::Error::from)?;

    // Create documents table
    // Create HNSW index on embeddings
    let script = [
        r#"
            ::hnsw create "#,
        ty.relation_str(),
        HNSW_SUFFIX,
        r#" {
                fields: [embedding],
                dim: 384,
                dtype: F32,
                m: 32,
                ef_construction: 200,
                distance: L2
            }
            "#,
    ]
    .join("");
    run_script_warn(
        db,
        &script,
        std::collections::BTreeMap::new(),
        ScriptMutability::Mutable,
    )?;
    Ok(())
}

pub fn replace_index_warn(db: &Database, ty: NodeType) -> Result<(), ploke_error::Error> {
    // Create documents table
    // Create HNSW index on embeddings
    let script = [
        r#"
            ::hnsw replace "#,
        ty.relation_str(),
        HNSW_SUFFIX,
        r#" {
                fields: [embedding],
                dim: 384,
                dtype: F32,
                m: 32,
                ef_construction: 200,
                distance: L2
            }
            "#,
    ]
    .join("");
    run_script_warn(
        db,
        &script,
        std::collections::BTreeMap::new(),
        ScriptMutability::Mutable,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use crate::{
        DbError, NodeType, create_index, create_index_primary, hnsw_all_types, hnsw_of_type,
        utils::test_utils::{TEST_DB_NODES, fixture_db_backup_path},
    };
    use tokio::sync::Mutex;

    use lazy_static::lazy_static;
    use ploke_error::Error;
    use tokio_test::assert_err;

    use crate::Database;
    #[cfg(feature = "multi_embedding_db")]
    use crate::MultiEmbeddingRuntimeConfig;
    #[cfg(feature = "multi_embedding_db")]
    use crate::multi_embedding::schema::vector_dims::vector_dimension_specs;

    #[tokio::test]
    async fn test_hnsw_init_from_backup() -> Result<(), Error> {
        let db = Database::init_with_schema()?;

        let target_file = fixture_db_backup_path();
        eprintln!("Loading backup db from file at:\n{}", target_file.display());
        let prior_rels_vec = db.relations_vec()?;
        db.import_from_backup(&target_file, &prior_rels_vec)
            .map_err(DbError::from)
            .map_err(ploke_error::Error::from)?;

        super::create_index_primary(&db)?;

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

        let target_file = fixture_db_backup_path();
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

    #[cfg(feature = "multi_embedding_db")]
    #[tokio::test]
    async fn multi_embedding_hnsw_index_and_search() -> Result<(), ploke_error::Error> {
        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Database::with_multi_embedding_config(raw_db, config);
        let batches = db.get_unembedded_node_data(1, 0)?;
        let (node_type, node) = batches
            .into_iter()
            .find_map(|typed| typed.v.into_iter().next().map(|entry| (typed.ty, entry)))
            .ok_or(DbError::NotFound)?;
        let dim_spec = vector_dimension_specs().first().expect("dimension spec");
        let vector = vec![0.5; dim_spec.dims() as usize];
        db.update_embeddings_batch(vec![(node.id, vector)]).await?;
        create_index(&db, node_type).map_err(ploke_error::Error::from)?;
        let hits = hnsw_of_type(
            &db,
            node_type,
            1,
            dim_spec.hnsw_search_ef() as usize,
        )?;
        assert!(
            hits.iter().any(|(id, _, _)| *id == node.id),
            "expected HNSW search to yield runtime embedding rows for seeded node"
        );
        Ok(())
    }

    #[cfg(feature = "multi_embedding_db")]
    #[tokio::test]
    async fn multi_embedding_hnsw_returns_empty_without_vectors() -> Result<(), ploke_error::Error>
    {
        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Database::with_multi_embedding_config(raw_db, config);

        // Ensure multi-embedding indexes exist but do not seed any runtime vectors.
        create_index_primary(&db).map_err(ploke_error::Error::from)?;

        let ty = NodeType::primary_nodes()
            .get(0)
            .copied()
            .expect("at least one primary node type");
        let k = 5;
        let ef = 16;
        let hits = hnsw_of_type(&db, ty, k, ef)?;
        assert!(
            hits.is_empty(),
            "expected no HNSW hits when no runtime vectors have been written"
        );

        Ok(())
    }
}
