use crate::database::{to_uuid, HNSW_SUFFIX};
use crate::{Database, DbError, NodeType, QueryResult, TypedEmbedData};
use std::collections::BTreeMap;

use cozo::{DataValue, NamedRows, Num, ScriptMutability};
use itertools::Itertools;
use ploke_core::EmbeddingModelId;
use tracing::instrument;

#[cfg(feature = "multi_embedding")]
use crate::multi_embedding::adapter::ExperimentalEmbeddingDatabaseExt;
#[cfg(feature = "multi_embedding")]
use crate::multi_embedding::schema::metadata::ExperimentalRelationSchemaDbExt;
#[cfg(feature = "multi_embedding")]
use crate::multi_embedding::schema::vector_dims::sample_vector_dimension_specs;
#[cfg(feature = "multi_embedding")]
use crate::multi_embedding::vectors::ExperimentalVectorRelation;
#[cfg(feature = "multi_embedding")]
use crate::multi_embedding::HnswDistance;
#[cfg(feature = "multi_embedding")]
use ploke_core::EmbeddingSetId;

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
    emb_id: EmbeddingModelId,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    let mut results = Vec::new();
    for ty in NodeType::primary_nodes() {
        let ty_ret: Vec<Embedding> = hnsw_of_type(db, ty, k, ef, emb_id.clone())?;
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
    emb_model: EmbeddingModelId,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    #[cfg(not(feature = "multi_embedding"))]
    let _ = emb_model;
    #[cfg(feature = "multi_embedding")]
    return multi_embedding_hnsw_of_type(db, ty, k, ef, emb_model);

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

/// Arguments for set-aware semantic search using multi-embedding relations.
///
/// This mirrors `SimilarArgs` but adds an `EmbeddingSetId` so callers can
/// direct HNSW search to the correct per-dimension vector relation when
/// `multi_embedding_db` is enabled.
#[cfg(feature = "multi_embedding")]
pub struct SimilarArgsForSet<'a> {
    pub db: &'a Database,
    pub vector_query: &'a Vec<f32>,
    pub k: usize,
    pub ef: usize,
    pub ty: NodeType,
    pub max_hits: usize,
    pub radius: f64,
    pub set_id: &'a EmbeddingSetId,
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

/// Multi-embedding aware variant of `search_similar_args`.
///
/// When `multi_embedding_db` is enabled, callers that know the active
/// `EmbeddingSetId` should prefer this helper so that HNSW search is
/// constrained to the per-dimension vector relation associated with the
/// set. The result layout (headers/rows) matches `search_similar_args`
/// so it can be fed into `QueryResult::to_embedding_nodes` unchanged.
#[cfg(feature = "multi_embedding")]
#[instrument(skip_all, fields(query_result))]
pub fn search_similar_args_for_set(
    args: SimilarArgsForSet,
) -> Result<EmbedDataVerbose, ploke_error::Error> {
    use crate::multi_embedding::{
        schema::metadata::experimental_spec_for_node, ExperimentalEmbeddingDbExt as _,
    };

    let SimilarArgsForSet {
        db,
        vector_query,
        k,
        ef,
        ty,
        max_hits,
        radius,
        set_id,
    } = args;
    let emb_id = set_id.model.clone();

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

    // Resolve the node + vector specs for this set so we can target the
    // correct per-dimension relation and reuse the same batch/file-path
    // shaping as the legacy search.
    let Some(spec) = experimental_spec_for_node(ty) else {
        return Ok(EmbedDataVerbose {
            typed_data: TypedEmbedData { v: Vec::new(), ty },
            dist: Vec::new(),
        });
    };
    spec.ensure_registered(db)
        .map_err(ploke_error::Error::from)?;
    let relation = ExperimentalVectorRelation::new(vector_query.len() as i64, emb_id);
    let emb_rel_name = relation.relation_name();

    // Check if vector embedding is already in the database, no-op if not found + return error.
    db.ensure_relation_registered(&emb_rel_name)
        .map_err(ploke_error::Error::from)?;
    let vector_rel = relation.relation_name();

    let mut script = String::new();
    let has_embedding_script = format!(
        r#"
    parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}}

    ancestor[desc, asc] := parent_of[desc, asc]
    ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

    has_embedding[id, name, hash, span] := *{rel} {{id, name, tracking_hash: hash, span, embedding_vec @ 'NOW' }},
        {vector_rel} {{ id @ 'NOW' }}

    is_root_module[id] := *module{{id @ 'NOW' }}, *file_mod {{owner_id: id @ 'NOW'}}

    batch[id, name, file_path, file_hash, hash, span, namespace] := 
        has_embedding[id, name, hash, span],
        ancestor[id, mod_id],
        is_root_module[mod_id],
        *module{{id: mod_id, tracking_hash: file_hash @ 'NOW'}},
        *file_mod {{ owner_id: mod_id, file_path, namespace @ 'NOW'}},

    ?[id, name, file_path, file_hash, hash, span, namespace, distance] := 
        batch[id, name, file_path, file_hash, hash, span, namespace],
     "#
    );

    // HNSW over the per-dimension vector relation backing this embedding set.
    let hnsw_script = format!(
        r#"
                *{vector_rel}{{ node_id, vector: v @ 'NOW' }},
                ~{vector_rel}:vector_idx{{ node_id |
                    query: vec($vector_query), 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance,
                    radius: $radius
                }},
                *{rel}{{ id: node_id, name @ 'NOW' }},
                :order distance
            "#,
        vector_rel = vector_rel,
        rel = rel,
    );
    let limit_param = ":limit $limit";

    script.push_str(&has_embedding_script);
    script.push_str(&hnsw_script);
    script.push_str(limit_param);

    tracing::trace!("script for set-aware similarity search is: {}", script);
    let query_result = db
        .run_script(&script, params, cozo::ScriptMutability::Immutable)
        .inspect_err(|e| tracing::error!("{e}"))
        .map_err(|e| DbError::Cozo(e.to_string()))?;

    let less_flat_row = query_result.rows.first();
    let count_less_flat = query_result.rows.len();
    if let Some(lfr) = less_flat_row {
        tracing::trace!(
            "\n{:=^80}\n== less_flat (set-aware): {count_less_flat} ==\n== less_flat: {less_flat_row:?} ==\n",
            rel
        );
    }
    let mut dist_vec = Vec::new();
    if !query_result.rows.is_empty() {
        tracing::trace!(
            "query_result.headers (set-aware): {:?}",
            query_result.headers
        );
        let dist_idx = query_result
            .headers
            .iter()
            .enumerate()
            .find(|(_, s)| *s == "distance")
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

#[cfg(feature = "multi_embedding")]
fn multi_embedding_hnsw_of_type(
    db: &Database,
    ty: NodeType,
    k: usize,
    ef: usize,
    emb_model: EmbeddingModelId,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    use crate::multi_embedding::schema::metadata::experimental_spec_for_node;

    let Some(spec) = experimental_spec_for_node(ty) else {
        return Ok(Vec::new());
    };
    spec.ensure_registered(db)
        .map_err(ploke_error::Error::from)?;
    let mut aggregated = Vec::new();
    for dim_spec in sample_vector_dimension_specs() {
        let relation = ExperimentalVectorRelation::new(dim_spec.dims(), emb_model.clone());
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

/// Runs HNSW search for a specific embedding set.
///
/// This helper narrows the multi-embedding search to the per-dimension
/// relation associated with `set_id`, using the dimension spec table to
/// locate the correct relation name and HNSW parameters. Callers that have
/// an `EmbeddingSetId` (e.g., runtime search flows) should prefer this
/// helper over the dimension-agnostic `hnsw_of_type` when the set identity
/// matters.
#[cfg(feature = "multi_embedding")]
pub fn multi_embedding_hnsw_for_set(
    db: &Database,
    ty: NodeType,
    set_id: &EmbeddingSetId,
    k: usize,
    ef: usize,
    emb_model: EmbeddingModelId,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    use crate::multi_embedding::schema::metadata::experimental_spec_for_node;

    let Some(spec) = experimental_spec_for_node(ty) else {
        return Ok(Vec::new());
    };

    spec.ensure_registered(db)
        .map_err(ploke_error::Error::from)?;

    // Resolve the dimension spec for this set (provider/model/dims) and
    // use it to select the correct per-dimension relation.
    let dims = set_id.dimension();
    let relation = ExperimentalVectorRelation::new(dims as i64, emb_model);
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
        Ok(rows) => Ok(named_rows_to_embeddings(rows)),
        Err(err) => {
            let db_err = DbError::from(err);
            let message = db_err.to_string();
            if message.contains("Index") && message.contains("not found") {
                Ok(Vec::new())
            } else {
                Err(ploke_error::Error::from(db_err))
            }
        }
    }
}

#[cfg(feature = "multi_embedding")]
fn create_multi_embedding_indexes_for_type(
    db: &Database,
    ty: NodeType,
    emb_model: EmbeddingModelId,
) -> Result<(), DbError> {
    use crate::multi_embedding::schema::metadata::experimental_spec_for_node;

    let relation = ExperimentalVectorRelation::new(dim_spec.dims(), emb_model.clone());
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
    Ok(())
}

// Previously we thought we would need to re-create this index after reloading the database from
// the backup. However, it seems that this is not the case, and we can create the hnsw index or
// re-create when needed, but we run into an error if we try to just create the index after
// reloading the database.
// TODO: Add a test to show the correct way to re-index the database, which will involve removing
// the current hnsw index before adding the replacement. If we try to just create the same hnsw
// index again after it is already present in the database, we get a panic-level runtime error.
// Instead, we want to either:
// 1. check if the hnsw index is present before we try to create it again, and then let the create
//    command be idempotent on attempting the second time with either
//      - an emitted warning if we have an Ext trait for the error type that can be emit an event
//      that will be correctly handled in the ultimate caller in `ploke-tui`
//      - return with an error that is specifically handled by the caller, which would allow the
//      caller to be idempotent or panic as the caller determines.
// 2. (probably should be another command for our database API) check if the hnsw index currently
//    exists for the named database, and then if it does exist, remove the current hnsw index and
//    replace it with the new hnsw index with the same name.
pub fn create_index(
    db: &Database,
    ty: NodeType,
    #[cfg(feature = "multi_embedding")] emb_model: EmbeddingModelId,
) -> Result<(), DbError> {
    // short-circuit with create_multi_embedding_indexes_for_type when new feature flag enabled
    if cfg!(feature = "multi_embedding") {
        #[cfg(feature = "multi_embedding")]
        create_multi_embedding_indexes_for_type(db, ty, emb_model)?;
        return Ok(());
    }

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

pub fn create_index_primary(
    db: &Database,
    #[cfg(feature = "multi_embedding")] emb_model: EmbeddingModelId,
) -> Result<(), DbError> {
    for ty in NodeType::primary_nodes() {
        #[cfg(not(feature = "multi_embedding"))]
        create_index(db, ty)?;
        #[cfg(feature = "multi_embedding")]
        create_index(db, ty, emb_model.clone())?;
    }
    Ok(())
}

pub fn create_index_warn(
    db: &Database,
    ty: NodeType,
    #[cfg(feature = "multi_embedding")] emb_model: EmbeddingModelId,
) -> Result<(), ploke_error::Error> {
    #[cfg(feature = "multi_embedding")]
    create_multi_embedding_indexes_for_type(db, ty, emb_model).map_err(ploke_error::Error::from)?;

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
        create_index, create_index_primary, hnsw_all_types, hnsw_of_type,
        utils::test_utils::{fixture_db_backup_path, TEST_DB_NODES},
        DbError, NodeType,
    };
    use ploke_core::{ArcStr, EmbeddingModelId};
    use tokio::sync::Mutex;

    use lazy_static::lazy_static;
    use ploke_error::Error;
    use tokio_test::assert_err;

    #[cfg(feature = "multi_embedding")]
    use crate::index::hnsw::multi_embedding_hnsw_for_set;
    #[cfg(feature = "multi_embedding")]
    use crate::multi_embedding::schema::vector_dims::sample_vector_dimension_specs;
    use crate::Database;
    #[cfg(feature = "multi_embedding")]
    use crate::MultiEmbeddingRuntimeConfig;
    #[cfg(feature = "multi_embedding")]
    use ploke_core::{EmbeddingProviderSlug, EmbeddingSetId, EmbeddingShape};

    #[tokio::test]
    async fn test_hnsw_init_from_backup() -> Result<(), Error> {
        let db = Database::init_with_schema()?;
        let emb_id = EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2");

        let target_file = fixture_db_backup_path();
        eprintln!("Loading backup db from file at:\n{}", target_file.display());
        let prior_rels_vec = db.relations_vec()?;
        db.import_from_backup(&target_file, &prior_rels_vec)
            .map_err(DbError::from)
            .map_err(ploke_error::Error::from)?;

        #[cfg(feature = "multi_embedding")]
        super::create_index_primary(&db, emb_id.clone())?;
        #[cfg(not(feature = "multi_embedding"))]
        super::create_index_primary(&db)?;

        let k = 20;
        let ef = 40;
        hnsw_all_types(&db, k, ef, emb_id)?;
        let unembedded = db.count_unembedded_nonfiles()?;
        println!("unembedded: {unembedded}");
        let embedded = db.count_pending_embeddings()?;
        println!("embedded: {embedded}");
        Ok(())
    }

    #[tokio::test]
    async fn test_hnsw_init_from_backup_error() -> Result<(), Error> {
        let db = Database::init_with_schema()?;
        let emb_id = EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2");

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
        let e = hnsw_all_types(&db, k, ef, emb_id);
        assert_err!(e.clone());
        let err_msg = String::from("Database error: Index hnsw_idx not found on relation function");
        let expect_err = ploke_error::Error::Warning(ploke_error::WarningError::PlokeDb(err_msg));
        let actual_err = e.clone().expect_err("expect error");
        assert!(matches!(actual_err, ploke_error::Error::Warning(_)));
        Ok(())
    }

    #[cfg(feature = "multi_embedding")]
    #[tokio::test]
    async fn multi_embedding_hnsw_index_and_search() -> Result<(), ploke_error::Error> {
        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let emb_model = EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2");

        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Database::with_multi_embedding_config(raw_db, config);

        let batches = db.get_unembedded_node_data(1, 0)?;
        let (node_type, node) = batches
            .into_iter()
            .find_map(|typed| typed.v.into_iter().next().map(|entry| (typed.ty, entry)))
            .ok_or(DbError::NotFound)?;

        let dim_spec = sample_vector_dimension_specs()
            .first()
            .expect("dimension spec");
        let vector = vec![0.5; dim_spec.dims() as usize];

        db.update_embeddings_batch(vec![(node.id, vector)]).await?;
        create_index(&db, node_type, emb_model.clone()).map_err(ploke_error::Error::from)?;
        let hits = hnsw_of_type(
            &db,
            node_type,
            1,
            dim_spec.hnsw_search_ef() as usize,
            emb_model,
        )?;
        assert!(
            hits.iter().any(|(id, _, _)| *id == node.id),
            "expected HNSW search to yield runtime embedding rows for seeded node"
        );
        Ok(())
    }

    #[cfg(feature = "multi_embedding")]
    #[tokio::test]
    async fn multi_embedding_hnsw_returns_empty_without_vectors() -> Result<(), ploke_error::Error>
    {
        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Database::with_multi_embedding_config(raw_db, config);

        // Ensure multi-embedding indexes exist but do not seed any runtime vectors.
        let emb_model = EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2");
        create_index_primary(&db, emb_model.clone()).map_err(ploke_error::Error::from)?;

        let ty = NodeType::primary_nodes()
            .get(0)
            .copied()
            .expect("at least one primary node type");
        let k = 5;
        let ef = 16;
        let hits = hnsw_of_type(&db, ty, k, ef, emb_model)?;
        assert!(
            hits.is_empty(),
            "expected no HNSW hits when no runtime vectors have been written"
        );

        Ok(())
    }

    #[cfg(feature = "multi_embedding")]
    #[tokio::test]
    async fn vector_spec_for_set_rejects_mismatched_provider_model(
    ) -> Result<(), ploke_error::Error> {
        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Database::with_multi_embedding_config(raw_db, config);

        let dim_spec = sample_vector_dimension_specs()
            .first()
            .expect("at least one vector dimension spec");
        let shape = EmbeddingShape::f32_raw(dim_spec.dims() as u32);
        let set_id = EmbeddingSetId::new(
            EmbeddingProviderSlug::new_from_str("wrong-provider"),
            EmbeddingModelId::new_from_str("wrong-model"),
            shape,
        );

        let err = db
            .vector_spec_for_set(&set_id)
            .expect_err("expected provider/model mismatch to error");
        match err {
            DbError::QueryExecution(msg) => {
                assert!(
                    msg.contains("does not match"),
                    "expected detailed mismatch message, got: {msg}"
                );
            }
            other => panic!("expected QueryExecution error, got {other:?}"),
        }

        Ok(())
    }

    #[cfg(feature = "multi_embedding")]
    #[tokio::test]
    async fn vector_spec_for_set_rejects_unsupported_dimension() -> Result<(), ploke_error::Error> {
        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Database::with_multi_embedding_config(raw_db, config);

        let shape = EmbeddingShape::f32_raw(999);
        let set_id = EmbeddingSetId::new(
            EmbeddingProviderSlug::new_from_str("local-transformers"),
            EmbeddingModelId::new_from_str("dummy-model"),
            shape,
        );

        let err = db
            .vector_spec_for_set(&set_id)
            .expect_err("expected unsupported dimension to error");
        match err {
            DbError::UnsupportedEmbeddingDimension { dims } => {
                assert_eq!(dims, 999);
            }
            other => panic!("expected UnsupportedEmbeddingDimension error, got {other:?}"),
        }

        Ok(())
    }

    #[cfg(feature = "multi_embedding")]
    #[tokio::test]
    async fn multi_embedding_hnsw_for_set_returns_seeded_node() -> Result<(), ploke_error::Error> {
        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Database::with_multi_embedding_config(raw_db, config);
        let emb_model = EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2");

        // Seed a single runtime embedding for a pending node.
        let batches = db.get_unembedded_node_data(1, 0)?;
        let (node_type, node) = batches
            .into_iter()
            .find_map(|typed| typed.v.into_iter().next().map(|entry| (typed.ty, entry)))
            .ok_or(DbError::NotFound)?;

        let dim_spec = sample_vector_dimension_specs()
            .first()
            .expect("at least one vector dimension spec");
        let vector = vec![0.5_f32; dim_spec.dims() as usize];
        db.update_embeddings_batch(vec![(node.id, vector)]).await?;

        // Ensure multi-embedding indexes exist for this type.
        create_index(&db, node_type, emb_model.clone()).map_err(ploke_error::Error::from)?;

        // Build an EmbeddingSetId that matches the seeded dimension spec.
        let shape = EmbeddingShape::f32_raw(dim_spec.dims() as u32);
        let set_id = EmbeddingSetId::new(
            dim_spec.provider().clone(),
            dim_spec.embedding_model().clone(),
            shape,
        );

        let hits = multi_embedding_hnsw_for_set(
            &db,
            node_type,
            &set_id,
            1,
            dim_spec.hnsw_search_ef() as usize,
            emb_model,
        )?;

        assert!(
            hits.iter().any(|(id, _, _)| *id == node.id),
            "expected HNSW search for set to yield runtime embedding rows for seeded node"
        );

        Ok(())
    }

    #[cfg(feature = "multi_embedding")]
    #[tokio::test]
    async fn search_similar_args_for_set_returns_seeded_node() -> Result<(), ploke_error::Error> {
        use crate::index::hnsw::{search_similar_args_for_set, SimilarArgsForSet};

        let raw_db = ploke_test_utils::setup_db_full("fixture_nodes")?;
        let config = MultiEmbeddingRuntimeConfig::from_env().enable_multi_embedding_db();
        let db = Database::with_multi_embedding_config(raw_db, config);
        let emb_model = EmbeddingModelId::new_from_str("sentence-transformers/all-MiniLM-L6-v2");

        // Seed a single runtime embedding for a pending node.
        let batches = db.get_unembedded_node_data(1, 0)?;
        let (node_type, node) = batches
            .into_iter()
            .find_map(|typed| typed.v.into_iter().next().map(|entry| (typed.ty, entry)))
            .ok_or(DbError::NotFound)?;

        let dim_spec = sample_vector_dimension_specs()
            .first()
            .expect("at least one vector dimension spec");
        let vector = vec![0.5_f32; dim_spec.dims() as usize];
        db.update_embeddings_batch(vec![(node.id, vector.clone())])
            .await?;

        // Ensure multi-embedding indexes exist for this type.
        create_index(&db, node_type, emb_model).map_err(ploke_error::Error::from)?;

        // Build an EmbeddingSetId that matches the seeded dimension spec.
        let shape = EmbeddingShape::f32_raw(dim_spec.dims() as u32);
        let set_id = EmbeddingSetId::new(
            dim_spec.provider().clone(),
            dim_spec.embedding_model().clone(),
            shape,
        );

        let args = SimilarArgsForSet {
            db: &db,
            vector_query: &vector,
            k: 1,
            ef: dim_spec.hnsw_search_ef() as usize,
            ty: node_type,
            max_hits: 1,
            radius: 10.0,
            set_id: &set_id,
        };

        let result = search_similar_args_for_set(args)?;
        assert!(
            result.typed_data.v.iter().any(|entry| entry.id == node.id),
            "expected set-aware semantic search to yield seeded node"
        );
        assert!(
            !result.dist.is_empty(),
            "expected at least one distance value from set-aware search"
        );

        Ok(())
    }

    #[cfg(feature = "multi_embedding")]
    #[tokio::test]
    async fn search_similar_args_for_set_errors_when_feature_disabled(
    ) -> Result<(), ploke_error::Error> {
        use crate::index::hnsw::{search_similar_args_for_set, SimilarArgsForSet};

        // Database without multi_embedding_db enabled.
        let db = Database::init_with_schema()?;

        let vector = vec![0.0_f32; 4];
        let shape = EmbeddingShape::f32_raw(4);
        let set_id = EmbeddingSetId::new(
            EmbeddingProviderSlug::new_from_str("local-transformers"),
            EmbeddingModelId::new_from_str("dummy-model"),
            shape,
        );

        let args = SimilarArgsForSet {
            db: &db,
            vector_query: &vector,
            k: 1,
            ef: 8,
            ty: NodeType::Function,
            max_hits: 1,
            radius: 10.0,
            set_id: &set_id,
        };

        let err = search_similar_args_for_set(args).expect_err(
            "expected search_similar_args_for_set to error when multi_embedding_db is disabled",
        );
        let msg = format!("{err}");
        assert!(
            msg.contains("requires multi_embedding_db"),
            "expected clear error about missing multi_embedding_db, got: {msg}"
        );

        Ok(())
    }
}
