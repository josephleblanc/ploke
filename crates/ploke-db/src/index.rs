use std::collections::BTreeMap;

use cozo::{DataValue, Num, ScriptMutability};
use itertools::Itertools;

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
            r#":hnsw_idx{id, name| 
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
        r#":hnsw_idx{id, name| 
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

pub fn search_similar_test(
    db: &Database,
    k: usize,
    ef: usize,
) -> Result<Vec<Embedding>, ploke_error::Error> {
    let mut params = std::collections::BTreeMap::new();
    params.insert("k".to_string(), DataValue::from(k as i64));
    params.insert("ef".to_string(), DataValue::from(ef as i64));

    let result = db
        .run_script(
            r#"
            ?[id, name, distance] := 
                *function{
                    id, 
                    name, 
                    embedding: v
                },
                ~function:hnsw_idx{id, name| 
                    query: v, 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance
                }
            "#,
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
        r#":hnsw_idx {
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

pub fn create_index_warn(
    db: &Database,
    ty: NodeType,
    mutability: ScriptMutability,
) -> Result<(), ploke_error::Error> {
    // Create documents table
    // Create HNSW index on embeddings
    let script = [
        r#"
            ::hnsw create "#,
        ty.relation_str(),
        r#":hnsw_idx {
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
    run_script_warn(db, &script, std::collections::BTreeMap::new(), mutability)?;
    Ok(())
}
