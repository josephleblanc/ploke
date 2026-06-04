//! Tests for vector functionality in CozoDB
//!
//! This file tests CozoDB's vector storage and similarity search capabilities:
//! 1. Basic vector storage and retrieval
//! 2. HNSW index creation and vector similarity search
//! 3. Direct HNSW graph traversal
//! 4. Code embeddings with higher dimensionality
//!
//! Note: When creating relations in tests that may be run multiple times,
//! we use `:replace` instead of `:create` to avoid "relation already exists" errors.

use crate::test_helpers::setup_test_db;
use cozo::{DataValue, Num, ScriptMutability, UuidWrapper};
use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt, schema::EmbeddingSetExt};
use std::collections::BTreeMap;
use uuid::Uuid;

mod test_helpers;

#[test]
fn test_basic_vector_functionality() {
    let db = setup_test_db();

    // Create a simple relation with vector field
    db.run_script(
        ":create vector_test {id: Int => vec_data: <F32; 3>}",
        BTreeMap::new(),
        ScriptMutability::Mutable,
    )
    .expect("Failed to create vector_test relation");

    // Insert a few vectors
    db.run_script(
        r#"
        ?[id, vec_data] <- [
            [1, vec([1.0, 0.0, 0.0])],
            [2, vec([0.0, 1.0, 0.0])],
            [3, vec([0.0, 0.0, 1.0])],
            [4, vec([0.5, 0.5, 0.5])]
        ] :put vector_test
        "#,
        BTreeMap::new(),
        ScriptMutability::Mutable,
    )
    .expect("Failed to insert vectors");

    // Create HNSW index on the vector field
    db.run_script(
        "::hnsw create vector_test:vector_idx {dim: 3, m: 10, ef_construction: 20, fields: [vec_data]}",
        BTreeMap::new(),
        ScriptMutability::Mutable,
    ).expect("Failed to create HNSW index");

    // Query all vectors to verify insertion
    let result = db
        .run_script(
            "?[id, vec_data] := *vector_test[id, vec_data]",
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to query vectors");

    assert_eq!(
        result.rows.len(),
        4,
        "Expected 4 vectors in the test relation"
    );

    // Test vector similarity search
    let result = db
        .run_script(
            r#"
        ?[id, dist] :=
            ~vector_test:vector_idx{id |
                query: vec([1.0, 0.0, 0.0]),
                k: 2,
                ef: 10,
                bind_distance: dist
            }
        :order dist
        "#,
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to perform vector search");

    assert!(
        !result.rows.is_empty(),
        "Expected at least one result from vector search"
    );

    // The first result should be id 1 (exact match) with distance close to 0
    let first_id = result.rows[0][0].get_int().unwrap_or(-1);
    let first_dist = result.rows[0][1].get_float().unwrap_or(1.0);

    assert_eq!(first_id, 1, "First result should be id 1 (exact match)");
    assert!(
        first_dist < 0.01,
        "Distance for exact match should be close to 0"
    );
}

#[test]
fn test_hnsw_graph_walking() {
    let db = setup_test_db();

    // Create and populate the test relation
    db.run_script(
        ":create vector_test {id: Int => vec_data: <F32; 3>}",
        BTreeMap::new(),
        ScriptMutability::Mutable,
    )
    .expect("Failed to create vector_test relation");

    db.run_script(
        r#"
        ?[id, vec_data] <- [
            [1, vec([1.0, 0.0, 0.0])],
            [2, vec([0.0, 1.0, 0.0])],
            [3, vec([0.0, 0.0, 1.0])],
            [4, vec([0.5, 0.5, 0.5])]
        ] :put vector_test
        "#,
        BTreeMap::new(),
        ScriptMutability::Mutable,
    )
    .expect("Failed to insert vectors");

    db.run_script(
        "::hnsw create vector_test:vector_idx {dim: 3, m: 10, ef_construction: 20, fields: [vec_data]}",
        BTreeMap::new(),
        ScriptMutability::Mutable,
    ).expect("Failed to create HNSW index");

    // Test walking the HNSW graph directly
    #[allow(unused_variables)]
    let result = db
        .run_script(
            r#"
        ?[fr_id, to_id, dist] :=
            *vector_test:vector_idx{layer: 0, fr_id, to_id, dist}
        :limit 10
        "#,
            BTreeMap::new(),
            ScriptMutability::Immutable,
        )
        .expect("Failed to walk HNSW graph");

    // The graph should have some connections
    #[cfg(feature = "debug")]
    test_helpers::print_debug("HNSW graph connections", &result);
}

fn test_embedding_set() -> EmbeddingSet {
    EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("local-test"),
        EmbeddingModelId::new_from_str("deterministic-vector-smoke"),
        EmbeddingShape::f32_raw(3),
    )
}

fn vector_param(vector: Vec<f32>) -> DataValue {
    DataValue::List(
        vector
            .into_iter()
            .map(|value| DataValue::Num(Num::Float(value as f64)))
            .collect(),
    )
}

fn insert_current_schema_embeddings(
    db: &cozo::Db<cozo::MemStorage>,
) -> (EmbeddingSet, Vec<(Uuid, Vec<f32>)>) {
    let embedding_set = test_embedding_set();
    let fixtures = vec![
        (
            Uuid::from_u128(0x00000000000000000000000000000001),
            vec![1.0, 0.0, 0.0],
        ),
        (
            Uuid::from_u128(0x00000000000000000000000000000002),
            vec![0.9, 0.1, 0.0],
        ),
        (
            Uuid::from_u128(0x00000000000000000000000000000003),
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

    (embedding_set, fixtures)
}

fn search_current_schema_embeddings(
    db: &cozo::Db<cozo::MemStorage>,
    embedding_set: &EmbeddingSet,
    query_vector: Vec<f32>,
    limit: usize,
) -> Vec<(Uuid, f64)> {
    let mut params = BTreeMap::new();
    params.insert("query_vector".to_string(), vector_param(query_vector));
    params.insert("k".to_string(), DataValue::from(limit as i64));
    params.insert("ef".to_string(), DataValue::from(16));
    params.insert("limit".to_string(), DataValue::from(limit as i64));
    params.insert(
        "embedding_set_id".to_string(),
        DataValue::from(embedding_set.hash_id().into_inner() as i64),
    );

    let hnsw_rel = embedding_set.hnsw_rel_name();
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

    let result = db
        .run_script(&script, params, ScriptMutability::Immutable)
        .expect("current-schema HNSW search should run");

    result
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

#[test]
#[ignore = "current-schema deterministic HNSW smoke"]
fn test_vector_similarity_search_identical() {
    let db = setup_test_db();
    let (embedding_set, fixtures) = insert_current_schema_embeddings(&db);

    let exact_id = fixtures[0].0;
    let results = search_current_schema_embeddings(&db, &embedding_set, fixtures[0].1.clone(), 3);

    assert_eq!(
        results.len(),
        3,
        "expected all seeded vectors in search results"
    );
    assert_eq!(
        results[0].0, exact_id,
        "identical query should rank the exact vector first"
    );
    assert!(
        results[0].1.abs() < 1e-6,
        "identical query should have near-zero distance, got {}",
        results[0].1
    );
    assert!(
        results.windows(2).all(|pair| pair[0].1 <= pair[1].1),
        "results should be ordered by ascending distance: {results:?}"
    );
}

#[test]
#[ignore = "current-schema deterministic HNSW smoke"]
fn test_vector_similarity_search() {
    let db = setup_test_db();
    let (embedding_set, fixtures) = insert_current_schema_embeddings(&db);

    let exact_id = fixtures[0].0;
    let similar_id = fixtures[1].0;
    let orthogonal_id = fixtures[2].0;
    let results = search_current_schema_embeddings(&db, &embedding_set, vec![0.88, 0.12, 0.0], 3);

    assert_eq!(
        results.len(),
        3,
        "expected all seeded vectors in search results"
    );
    assert_eq!(
        results[0].0, similar_id,
        "nearby query should rank the deliberately similar vector first"
    );
    assert_eq!(
        results[1].0, exact_id,
        "exact x-axis vector should be second for the offset query"
    );
    assert_eq!(
        results[2].0, orthogonal_id,
        "orthogonal vector should be farthest from the offset query"
    );
    assert!(
        results[0].1 < results[1].1 && results[1].1 < results[2].1,
        "expected strict distance ordering similar < exact < orthogonal, got {results:?}"
    );
}
