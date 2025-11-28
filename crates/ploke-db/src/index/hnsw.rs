use crate::{Database, DbError, NodeType, QueryResult, TypedEmbedData};
use std::collections::BTreeMap;

use cozo::{DataValue, Num, ScriptMutability};
use itertools::Itertools;
use tracing::instrument;

use crate::database::HNSW_SUFFIX;
#[cfg(feature = "multi_embedding_db")]
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

pub fn hnsw_of_type(
    db: &Database,
    ty: NodeType,
    k: usize,
    ef: usize,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    #[cfg(feature = "multi_embedding_db")]
    {
        return db.hnsw_neighbors_for_type(ty, &db.active_embedding_set, k, ef);
    }

    #[cfg(not(feature = "multi_embedding_db"))]
    {
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
            results.push((id, content, row[2].to_owned()));
        }

        Ok(results)
    }
}

#[instrument(skip_all, fields(query_result))]
pub fn search_similar(
    db: &Database,
    vector_query: Vec<f32>,
    k: usize,
    ef: usize,
    ty: NodeType,
) -> Result<TypedEmbedData, ploke_error::Error> {
    #[cfg(feature = "multi_embedding_db")]
    {
        return db
            .search_similar_for_set(&db.active_embedding_set, ty, vector_query, k, ef, 100, None)
            .map(|res| res.typed_data);
    }

    #[cfg(not(feature = "multi_embedding_db"))]
    {
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
    #[cfg(feature = "multi_embedding_db")]
    {
        return db.search_similar_for_set(
            &db.active_embedding_set,
            ty,
            vector_query.clone(),
            k,
            ef,
            max_hits,
            Some(radius),
        );
    }
    #[cfg(not(feature = "multi_embedding_db"))]
    {
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

pub fn create_index(db: &Database, ty: NodeType) -> Result<(), DbError> {
    #[cfg(feature = "multi_embedding_db")]
    {
        let _ = ty;
        return db.create_embedding_index(&db.active_embedding_set);
    }

    #[cfg(not(feature = "multi_embedding_db"))]
    {
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
}

pub fn create_index_primary(db: &Database) -> Result<(), DbError> {
    #[cfg(feature = "multi_embedding_db")]
    {
        use crate::multi_embedding::hnsw_ext::HnswExt;

        return db.create_embedding_index(&db.active_embedding_set);
    }

    #[cfg(not(feature = "multi_embedding_db"))]
    {
        for ty in NodeType::primary_nodes() {
            create_index(db, ty)?;
        }
        Ok(())
    }
}

#[cfg(feature = "multi_embedding_db")]
/// Temporary wrapper function to replace current API
pub fn create_index_warn(db: &Database) -> Result<(), ploke_error::Error> {
    #[cfg(feature = "multi_embedding_db")]
    {
        use crate::multi_embedding::hnsw_ext::HnswExt;

        db
            .create_embedding_index(&db.active_embedding_set)
            .map_err(ploke_error::Error::from)
    }
}

#[cfg(not(feature = "multi_embedding_db"))]
pub fn create_index_warn(db: &Database, ty: NodeType) -> Result<(), ploke_error::Error> {
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
    #[cfg(feature = "multi_embedding_db")]
    {
        use crate::multi_embedding::hnsw_ext::HnswExt;

        let _ = ty;
        return db
            .create_embedding_index(&db.active_embedding_set)
            .map_err(ploke_error::Error::from);
    }

    #[cfg(not(feature = "multi_embedding_db"))]
    {
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
