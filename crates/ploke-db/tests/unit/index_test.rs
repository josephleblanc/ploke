#![allow(clippy::type_complexity, dead_code)]
use itertools::Itertools;

use cozo::*;
use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt, schema::EmbeddingSetExt};
use std::collections::HashMap;
use uuid::Uuid;

#[derive(Debug)]
struct VectorDocument {
    id: i32,
    content: String,
    embedding: Vec<f32>,
}

struct VectorIndex {
    db: DbInstance,
}

impl VectorIndex {
    fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let db = DbInstance::new("mem", "", "")?;
        Ok(Self { db })
    }

    fn create_tables(&self) -> Result<(), Box<dyn std::error::Error>> {
        // Create documents table
        self.db.run_script(
            r#"
            :create documents {
                id: Int,
                content: String,
                embedding: <F32; 384>
            }
            "#,
            std::collections::BTreeMap::new(),
            ScriptMutability::Mutable,
        )?;

        // Create HNSW index on embeddings
        self.db.run_script(
            r#"
            ::hnsw create documents:embedding {
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

    fn insert_document(&self, doc: &VectorDocument) -> Result<(), Box<dyn std::error::Error>> {
        let mut params = std::collections::BTreeMap::new();
        params.insert("id".to_string(), DataValue::from(doc.id as i64));
        params.insert("content".to_string(), DataValue::from(doc.content.clone()));
        params.insert("embedding".to_string(), to_cozo_float(doc));

        self.db.run_script(
            r#"
            ?[id, content, embedding] <- [[$id, $content, $embedding]]
            :put documents {id, content, embedding}
            "#,
            params,
            ScriptMutability::Mutable,
        )?;

        Ok(())
    }

    fn search_similar(
        &self,
        query_embedding: &[f32],
        k: usize,
        ef: usize,
    ) -> Result<Vec<(i32, String, f64)>, Box<dyn std::error::Error>> {
        let mut params = std::collections::BTreeMap::new();
        params.insert("query_embedding".to_string(), arr_to_float(query_embedding));
        params.insert("k".to_string(), DataValue::from(k as i64));
        params.insert("ef".to_string(), DataValue::from(ef as i64));

        let result = self.db.run_script(
            r#"
            ?[id, content, distance] := 
                ~documents:embedding{id, content | 
                    query: q, 
                    k: $k, 
                    ef: $ef,
                    bind_distance: distance
                }, q = vec($query_embedding)
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

    fn rebuild_index(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.db.run_script(
            r#"
            ::hnsw rebuild documents:embedding
            "#,
            std::collections::BTreeMap::new(),
            ScriptMutability::Mutable,
        )?;

        Ok(())
    }

    fn get_index_stats(
        &self,
    ) -> Result<HashMap<String, serde_json::Value>, Box<dyn std::error::Error>> {
        let result = self.db.run_script(
            r#"
            ::hnsw stat documents:embedding
            "#,
            std::collections::BTreeMap::new(),
            ScriptMutability::Immutable,
        )?;

        let mut stats = HashMap::new();
        for row in result.rows {
            let key = row[0].get_str().unwrap().to_string();
            let value = match &row[1] {
                DataValue::Num(Num::Int(i)) => serde_json::json!(*i),
                DataValue::Num(Num::Float(f)) => serde_json::json!(*f),
                DataValue::Str(s) => serde_json::json!(s),
                _ => serde_json::json!(null),
            };
            stats.insert(key, value);
        }

        Ok(stats)
    }
}

fn to_cozo_float(doc: &VectorDocument) -> DataValue {
    DataValue::List(
        doc.embedding
            .clone()
            .iter()
            .map(|f| DataValue::Num(Num::Float(*f as f64)))
            .collect_vec(),
    )
}

fn arr_to_float(arr: &[f32]) -> DataValue {
    DataValue::List(
        arr.iter()
            .map(|f| DataValue::Num(Num::Float(*f as f64)))
            .collect_vec(),
    )
}

fn current_schema_embedding_set() -> EmbeddingSet {
    EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("local-test"),
        EmbeddingModelId::new_from_str("deterministic-vector-smoke-db"),
        EmbeddingShape::f32_raw(3),
    )
}

fn current_schema_vector_param(vector: Vec<f32>) -> DataValue {
    DataValue::List(
        vector
            .into_iter()
            .map(|value| DataValue::Num(Num::Float(value as f64)))
            .collect(),
    )
}

fn setup_current_schema_vector_index() -> (Db<MemStorage>, EmbeddingSet, Vec<(Uuid, Vec<f32>)>) {
    let db = Db::new(MemStorage::default()).expect("in-memory Cozo DB should initialize");
    let embedding_set = current_schema_embedding_set();
    let fixtures = vec![
        (
            Uuid::from_u128(0x00000000000000000000000000000101),
            vec![1.0, 0.0, 0.0],
        ),
        (
            Uuid::from_u128(0x00000000000000000000000000000102),
            vec![0.9, 0.1, 0.0],
        ),
        (
            Uuid::from_u128(0x00000000000000000000000000000103),
            vec![0.0, 1.0, 0.0],
        ),
    ];

    db.ensure_embedding_set_relation()
        .expect("embedding_set relation should be created");
    db.put_embedding_set(&embedding_set)
        .expect("embedding_set row should be inserted");
    db.ensure_vector_embedding_relation(&embedding_set)
        .expect("vector embedding relation should be created from current schema");
    db.update_embeddings_batch(
        fixtures
            .iter()
            .map(|(id, vector)| (*id, vector.iter().map(|value| *value as f64).collect()))
            .collect(),
        &embedding_set,
    )
    .expect("embedding vectors should be inserted through production batch API");
    db.create_embedding_index(&embedding_set)
        .expect("HNSW index should be created for current embedding relation");

    (db, embedding_set, fixtures)
}

fn search_current_schema_index(
    db: &Db<MemStorage>,
    embedding_set: &EmbeddingSet,
    query_vector: Vec<f32>,
    limit: usize,
) -> Vec<(Uuid, f64)> {
    let hnsw_rel = embedding_set.hnsw_rel_name();
    let params = std::collections::BTreeMap::from([
        (
            "query_vector".to_string(),
            current_schema_vector_param(query_vector),
        ),
        ("k".to_string(), DataValue::from(limit as i64)),
        ("ef".to_string(), DataValue::from(16)),
        ("limit".to_string(), DataValue::from(limit as i64)),
        (
            "embedding_set_id".to_string(),
            DataValue::from(embedding_set.hash_id().into_inner() as i64),
        ),
    ]);
    let script = format!(
        r#"
        ?[node_id, distance] :=
            ~{hnsw_rel}{{ node_id, embedding_set_id: set_id |
                query: vec($query_vector),
                k: $k,
                ef: $ef,
                bind_distance: distance
            }},
            set_id = $embedding_set_id
        :order distance
        :limit $limit
        "#,
    );

    db.run_script(&script, params, ScriptMutability::Immutable)
        .expect("current-schema HNSW search should run")
        .rows
        .into_iter()
        .map(|row| {
            let id = match row.first() {
                Some(DataValue::Uuid(UuidWrapper(id))) => *id,
                other => panic!("expected UUID node id in HNSW result, got {other:?}"),
            };
            let distance = row
                .get(1)
                .and_then(DataValue::get_float)
                .expect("HNSW distance should be a float");
            (id, distance)
        })
        .collect()
}

fn current_schema_hnsw_index_rows(db: &Db<MemStorage>, embedding_set: &EmbeddingSet) -> NamedRows {
    let base_rel = embedding_set.rel_name().as_ref().replace('-', "_");
    let script = format!("::indices {base_rel}");
    db.run_script(
        &script,
        std::collections::BTreeMap::new(),
        ScriptMutability::Immutable,
    )
    .expect("HNSW index metadata should be listed for current-schema relation")
}

fn current_schema_vector_count(db: &Db<MemStorage>, embedding_set: &EmbeddingSet) -> usize {
    let rel = embedding_set.rel_name().as_ref().replace('-', "_");
    let params = std::collections::BTreeMap::from([(
        "embedding_set_id".to_string(),
        DataValue::from(embedding_set.hash_id().into_inner() as i64),
    )]);
    let script = format!(
        r#"
        ?[node_id] :=
            *{rel}{{ node_id, embedding_set_id: set_id @ 'NOW' }},
            set_id = $embedding_set_id
        "#,
    );

    db.run_script(&script, params, ScriptMutability::Immutable)
        .expect("current-schema vector relation should be queryable")
        .rows
        .len()
}

// Helper function to generate mock embeddings
fn generate_mock_embedding(seed: u64, dim: usize) -> Vec<f32> {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};

    let mut hasher = DefaultHasher::new();
    seed.hash(&mut hasher);
    let hash = hasher.finish();

    (0..dim)
        .map(|i| {
            let mut h = DefaultHasher::new();
            (hash + i as u64).hash(&mut h);
            let val = h.finish() as f64 / u64::MAX as f64;
            ((val - 0.5) * 2.0) as f32 // Normalize to [-1, 1]
        })
        .collect()
}

// Example usage
fn main() -> Result<(), Box<dyn std::error::Error>> {
    println!("Creating CozoDB HNSW Vector Index Example");

    let index = VectorIndex::new()?;
    index.create_tables()?;

    // Insert sample documents
    let documents = vec![
        VectorDocument {
            id: 1,
            content: "Rust is a systems programming language".to_string(),
            embedding: generate_mock_embedding(1, 384),
        },
        VectorDocument {
            id: 2,
            content: "CozoDB is a graph database".to_string(),
            embedding: generate_mock_embedding(2, 384),
        },
        VectorDocument {
            id: 3,
            content: "Vector databases enable semantic search".to_string(),
            embedding: generate_mock_embedding(3, 384),
        },
    ];

    for doc in &documents {
        index.insert_document(doc)?;
    }

    // Perform similarity search
    let query_embedding = generate_mock_embedding(1, 384);
    let results = index.search_similar(&query_embedding, 2, 2)?;

    println!("Search results:");
    for (id, content, distance) in results {
        println!(
            "ID: {}, Content: {}, Distance: {:.4}",
            id, content, distance
        );
    }

    // Get index statistics
    let stats = index.get_index_stats()?;
    println!("\nIndex statistics:");
    for (key, value) in stats {
        println!("{}: {}", key, value);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_index_and_insert() {
        let index = VectorIndex::new().expect("Failed to create index");
        index.create_tables().expect("Failed to create tables");

        let doc = VectorDocument {
            id: 1,
            content: "Hello world".to_string(),
            embedding: generate_mock_embedding(1, 384),
        };

        index
            .insert_document(&doc)
            .expect("Failed to insert document");

        // Verify the document was inserted by searching
        let results = index
            .search_similar(&doc.embedding, 1, 1)
            .expect("Failed to search");

        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, 1);
        assert_eq!(results[0].1, "Hello world");
        assert!(results[0].2 < 0.1); // Distance should be very small (nearly identical)
    }

    #[test]
    fn test_similarity_search() {
        let index = VectorIndex::new().expect("Failed to create index");
        index.create_tables().expect("Failed to create tables");

        // Insert multiple documents
        let documents = vec![
            VectorDocument {
                id: 1,
                content: "Machine learning is fascinating".to_string(),
                embedding: generate_mock_embedding(1, 384),
            },
            VectorDocument {
                id: 2,
                content: "Deep learning and neural networks".to_string(),
                embedding: generate_mock_embedding(2, 384),
            },
            VectorDocument {
                id: 3,
                content: "Natural language processing".to_string(),
                embedding: generate_mock_embedding(3, 384),
            },
            VectorDocument {
                id: 4,
                content: "Computer vision applications".to_string(),
                embedding: generate_mock_embedding(4, 384),
            },
        ];

        for doc in &documents {
            index
                .insert_document(doc)
                .expect("Failed to insert document");
        }

        // Search for similar documents
        let query_embedding = generate_mock_embedding(1, 384); // Similar to first document
        let results = index
            .search_similar(&query_embedding, 2, 2)
            .expect("Failed to search");

        assert_eq!(results.len(), 2);
        // First result should be the most similar (document 1)
        assert_eq!(results[0].0, 1);
        assert!(results[0].2 < results[1].2); // First result should have smaller distance
    }

    #[test]
    #[ignore = "current-schema deterministic HNSW smoke"]
    fn test_index_rebuild() {
        let (db, embedding_set, fixtures) = setup_current_schema_vector_index();
        let query_vector = fixtures[0].1.clone();
        let before = search_current_schema_index(&db, &embedding_set, query_vector.clone(), 3);
        let hnsw_rel = embedding_set.hnsw_rel_name();

        db.run_script(
            &format!("::hnsw drop {hnsw_rel}"),
            std::collections::BTreeMap::new(),
            ScriptMutability::Mutable,
        )
        .expect("dropping current-schema HNSW index should succeed");
        assert!(
            !db.is_hnsw_index_registered(&embedding_set)
                .expect("dropped HNSW index registration check should run"),
            "HNSW index should not be registered after drop"
        );

        db.create_embedding_index(&embedding_set)
            .expect("production HNSW creation entrypoint should rebuild dropped index");
        let after = search_current_schema_index(&db, &embedding_set, query_vector, 3);

        assert_eq!(
            after.len(),
            before.len(),
            "rebuilt index should return the same result count"
        );
        assert_eq!(
            after.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            before.iter().map(|(id, _)| *id).collect::<Vec<_>>(),
            "rebuilt index should preserve deterministic nearest-neighbor ranking"
        );
        for ((_, before_distance), (_, after_distance)) in before.iter().zip(after.iter()) {
            assert!(
                (before_distance - after_distance).abs() < 1e-6,
                "rebuilt index should preserve distances, before={before:?}, after={after:?}"
            );
        }
    }

    #[test]
    #[ignore = "current-schema deterministic HNSW smoke"]
    fn test_index_stats() {
        let (db, embedding_set, fixtures) = setup_current_schema_vector_index();
        let hnsw_rel = embedding_set.hnsw_rel_name().to_string();
        let rows = current_schema_hnsw_index_rows(&db, &embedding_set);

        assert_eq!(
            rows.headers,
            vec!["name", "type", "relations", "config"],
            "::indices output should expose stable metadata columns"
        );
        let hnsw_row = rows
            .rows
            .iter()
            .find(|row| {
                row.get(1).and_then(DataValue::get_str) == Some("hnsw")
                    && row.get(2).and_then(|relations| match relations {
                        DataValue::List(values) => Some(
                            values
                                .iter()
                                .any(|value| value.get_str() == Some(hnsw_rel.as_str())),
                        ),
                        _ => None,
                    }) == Some(true)
            })
            .unwrap_or_else(|| panic!("expected HNSW metadata row for {hnsw_rel}, got {rows:?}"));

        let config = match hnsw_row.get(3) {
            Some(DataValue::Json(JsonData(config))) => config,
            other => panic!("expected HNSW config JSON metadata, got {other:?}"),
        };
        assert_eq!(
            config.get("vec_dim").and_then(serde_json::Value::as_i64),
            Some(3),
            "HNSW metadata should report the current embedding dimension"
        );
        assert_eq!(
            config
                .get("ef_construction")
                .and_then(serde_json::Value::as_i64),
            Some(200),
            "HNSW metadata should report production ef_construction"
        );
        assert_eq!(
            config
                .get("m_neighbours")
                .and_then(serde_json::Value::as_i64),
            Some(32),
            "HNSW metadata should report production m_neighbours"
        );
        assert_eq!(
            config.get("distance").and_then(serde_json::Value::as_str),
            Some("L2"),
            "HNSW metadata should report production distance metric"
        );
        assert_eq!(
            current_schema_vector_count(&db, &embedding_set),
            fixtures.len(),
            "stats smoke should verify indexed relation contains the seeded vectors"
        );
    }

    #[test]
    fn test_empty_search() {
        let index = VectorIndex::new().expect("Failed to create index");
        index.create_tables().expect("Failed to create tables");

        let query_embedding = generate_mock_embedding(1, 384);
        let results = index
            .search_similar(&query_embedding, 5, 5)
            .expect("Failed to search empty index");

        assert_eq!(results.len(), 0);
    }

    #[test]
    fn test_different_embedding_dimensions() {
        // This test verifies that our mock function works with different dimensions
        let embedding_128 = generate_mock_embedding(1, 128);
        let embedding_384 = generate_mock_embedding(1, 384);
        let embedding_512 = generate_mock_embedding(1, 512);

        assert_eq!(embedding_128.len(), 128);
        assert_eq!(embedding_384.len(), 384);
        assert_eq!(embedding_512.len(), 512);

        // Same seed should produce same values for same positions
        let embedding_128_again = generate_mock_embedding(1, 128);
        assert_eq!(embedding_128, embedding_128_again);
    }
}
