use std::collections::BTreeMap;

use cozo::{DataValue, Num, ScriptMutability};
use itertools::Itertools;
use tracing::instrument;

use crate::database::HNSW_SUFFIX;

use super::*;

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
    let mut script = String::from(
        r#"
            ?[id, name, distance] := "#,
    );

    let mut rel_params = std::collections::BTreeMap::new();
    for (i, ty) in NodeType::primary_nodes().iter().enumerate() {
        let rel = ty.relation_str();
        let k_for_ty = format!("{}{}", rel, k);
        let ef_for_ty = format!("{}{}", rel, ef);
        rel_params.insert(k_for_ty.clone(), DataValue::from(k as i64));
        rel_params.insert(ef_for_ty.clone(), DataValue::from(ef as i64));
        rel_params.insert(rel.to_string(), DataValue::from(rel));

        let rel_rhs = [
            rel,
            r#"{
                    id, 
                    name, 
                    embedding: v
                },
                ~"#,
            rel,
            HNSW_SUFFIX, r#"{id, name| 
                    query: v, 
                    k: $"#,
            k_for_ty.as_str(),
            r#", 
                    ef: $ef,
                    bind_distance: distance
                }
            "#,
        ]
        .into_iter();
        script.extend(rel_rhs);
        if !i > NodeType::primary_nodes().len() {
            script.push_str(" or ");
        }
    }

    let result = run_script_warn(db, &script, rel_params, ScriptMutability::Immutable)?;
    let mut results = Vec::new();
    use cozo::Vector;
    for row in result.rows.into_iter() {
        tracing::info!("{:?}", row);
        let id = if let DataValue::Uuid(cozo::UuidWrapper(id)) = row[0] {
            tracing::info!("{:?}", id);
            id
        } else {
            uuid::Uuid::max()
        };
        let content = row[1].get_str().unwrap().to_string();
        results.push((id, content, row[2].to_owned()));
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
                .contains("Index hnsw_idx not found on relation const")
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
                },
                ~"#,
        rel,
        HNSW_SUFFIX, r#"{id, name| 
                    query: v, 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance
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
        tracing::info!("{:?}", row);
        let id = if let DataValue::Uuid(cozo::UuidWrapper(id)) = row[0] {
            tracing::info!("{:?}", id);
            id
        } else {
            uuid::Uuid::max()
        };
        let content = row[1].get_str().unwrap().to_string();
        results.push((id, content, row[2].to_owned()));
    }

    Ok(results)
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

    ?[id, name, file_path, file_hash, hash, span, namespace] := 
        batch[id, name, file_path, file_hash, hash, span, namespace]
     "#;
    let hnsw_script = [r#"
            ?[id, name, distance] := 
                *function{
                    id, 
                    name, 
                    @ 'NOW'
                },
                ~function"#, HNSW_SUFFIX, r#"{id, name| 
                    query: vec($vector_query), 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance
                },
            "#];
    let limit_param = ":limit $limit";

    let rel = ty.relation_str();
    script.push_str(base_script_start);
    script.push_str(rel);
    script.push_str(base_script_end);

    tracing::info!("script for similarity search is: {}", script);
    let query_result = db
        .run_script(&script, params, cozo::ScriptMutability::Immutable)
        .inspect_err(|e| tracing::error!("{e}"))
        .map_err(|e| DbError::Cozo(e.to_string()))?;

    let less_flat_row = query_result.rows.first();
    let count_less_flat = query_result.rows.len();
    if let Some(lfr) = less_flat_row {
        tracing::info!("\n{:=^80}\n== less_flat: {count_less_flat} ==\n== less_flat: {less_flat_row:?} ==\n", rel);
    }
    let v = QueryResult::from(query_result).to_embedding_nodes()?;
    let ty_embed = TypedEmbedData { v, ty };
    Ok(ty_embed)
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
                ~function"#, HNSW_SUFFIX, r#"{id, name| 
                    query: v, 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance
                }
            "#,
    ].concat();
    let result = db
        .run_script(
            &hnsw_script,
            params,
            ScriptMutability::Immutable,
        )
        .map_err(DbError::from)?;

    let mut results = Vec::new();
    for row in result.rows {
        tracing::info!("{:?}", row);
        let id = if let DataValue::Uuid(cozo::UuidWrapper(id)) = row[0] {
            tracing::info!("{:?}", id);
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
    // Create documents table
    // Create HNSW index on embeddings
    let script = [
        r#"
            ::hnsw create "#,
        ty.relation_str(),
        HNSW_SUFFIX, r#" {
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

pub fn create_index_warn(db: &Database, ty: NodeType) -> Result<(), ploke_error::Error> {
    // Create documents table
    // Create HNSW index on embeddings
    let script = [
        r#"
            ::hnsw create "#,
        ty.relation_str(),
        HNSW_SUFFIX, r#" {
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
        HNSW_SUFFIX, r#" {
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

/// Performs batch semantic search on prompts from a file and returns results
///
/// This function reads prompts from a file, generates embeddings for each prompt,
/// performs semantic search against the database, and returns the results in a
/// structured format suitable for serialization.
///
/// # Arguments
///
/// * `db` - Database instance for performing semantic search
/// * `prompt_file` - Path to file containing prompts (one per line)
/// * `out_file` - Path to output file for results (JSON format)
/// * `max_hits` - Maximum number of similar snippets to return per prompt
/// * `threshold` - Optional similarity threshold for filtering results
///
/// # Returns
///
/// Returns a vector of batch results containing prompt indices, original prompts,
/// and their corresponding code snippets found through semantic search.
pub async fn batch_prompt_search(
    db: &Database,
    prompt_file: String,
    out_file: String,
    max_hits: Option<usize>,
    threshold: Option<f32>,
) -> Result<Vec<BatchResult>, ploke_error::Error> {
    use std::fs;
    use ploke_embed::indexer::EmbeddingProcessor;
    
    let prompts = fs::read_to_string(&prompt_file)
        .map_err(|e| DbError::Io(e.to_string()))?;
    
    let prompts: Vec<String> = prompts
        .lines()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .collect();
    
    if prompts.is_empty() {
        return Ok(Vec::new());
    }
    
    // Create a simple embedder for batch processing
    let embedder = EmbeddingProcessor::new(ploke_embed::indexer::EmbeddingSource::Local(
        ploke_embed::local::LocalEmbedder::new(ploke_embed::local::EmbeddingConfig::default())
            .map_err(|e| DbError::Internal(e.to_string()))?
    ));
    
    let max_hits = max_hits.unwrap_or(10);
    let _threshold = threshold.unwrap_or(0.0);
    
    let mut results = Vec::new();
    
    for (prompt_idx, prompt) in prompts.iter().enumerate() {
        tracing::info!("Processing prompt {}: {}", prompt_idx, prompt);
        
        let embeddings = embedder
            .generate_embeddings(vec![prompt.clone()])
            .await
            .map_err(|e| DbError::Internal(e.to_string()))?;
        
        if let Some(embedding) = embeddings.into_iter().next() {
            let ty_embed_data = search_similar(
                db,
                embedding,
                max_hits,
                200,
                NodeType::Function,
            )?;
            
            let snippets = ty_embed_data
                .v
                .into_iter()
                .map(|data| data.name)
                .collect::<Vec<String>>();
            
            results.push(BatchResult {
                prompt_idx,
                prompt: prompt.clone(),
                snippets,
            });
        }
    }
    
    // Write results to file
    let json_content = serde_json::to_string_pretty(&results)
        .map_err(|e| DbError::Internal(e.to_string()))?;
    
    fs::write(&out_file, json_content)
        .map_err(|e| DbError::Io(e.to_string()))?;
    
    Ok(results)
}

/// Result structure for batch prompt search operations
#[derive(Serialize, Deserialize)]
pub struct BatchResult {
    pub prompt_idx: usize,
    pub prompt: String,
    pub snippets: Vec<String>,
}
