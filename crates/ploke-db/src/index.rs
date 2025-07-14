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

type Embedding = (i32, String, f64);

pub fn search_similar(
    db: &Database,
    k: usize,
    ef: usize,
) -> Result<Vec<Embedding>, Box<dyn std::error::Error>> {
    let mut params = std::collections::BTreeMap::new();
    params.insert("k".to_string(), DataValue::from(k as i64));
    params.insert("ef".to_string(), DataValue::from(ef as i64));

    let result = db.run_script(
        r#"
            ?[id, name, distance] := 
                *function{name: "func_one", embedding: v},
                ~function:embedding{id, name| 
                    query: v, 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance
                }
            "#,
        params,
        ScriptMutability::Immutable,
    )?;

    let mut results = Vec::new();
    for row in result.rows {
        let id = row[0].get_int().unwrap() as i32;
        let content = row[1].get_str().unwrap().to_string();
        let distance = row[2].get_float().unwrap();
        results.push((id, content, distance));
    }

    Ok(results)
}

    pub fn create_index(
    db: &Database,
    ty: NodeType,
) -> Result<(), Box<dyn std::error::Error>> {
        // Create documents table
        // Create HNSW index on embeddings
        db.run_script(
            r#"
            ::hnsw create function:embedding {
                fields: [embedding],
                dim: 384,
                dtype: F32,
                m: 32,
                ef_construction: 200,
                distance: L2
            }
            "#,
            std::collections::BTreeMap::new(),
            ScriptMutability::Mutable,
        )?;

        Ok(())
    }
