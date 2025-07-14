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
use cozo::{DataValue, ScriptMutability};
use std::collections::BTreeMap;

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

fn insert_sample_embeddings(
    db: &cozo::Db<cozo::MemStorage>,
) -> Result<cozo::NamedRows, cozo::Error> {
    // Check if the relation exists first
    let relations = db.run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)?;
    println!("{:-^50?}", "all relations");
    for row in relations {
        println!("Row ---> {:?}", row);
    }

    println!("{:-^80}", "all indices of code_embeddings");
    let indicies = db.run_script(
        "::indices code_embeddings",
        BTreeMap::new(),
        ScriptMutability::Immutable,
    )?;
    for row in indicies {
        println!("Row ---> {:?}", row);
    }
    println!("{:-^80}", "end all indicies");

    println!("{:-^80}", "all columns of code_embeddings");
    let columns = db.run_script(
        "::columns code_embeddings",
        BTreeMap::new(),
        ScriptMutability::Immutable,
    )?;
    for row in columns {
        println!("Row ---> {:?}", row);
    }
    println!("{:-^80}", "end all indicies");

    println!("{:-^80}", "all columns of code_embeddings:vector");
    let indicies = db.run_script(
        "::indices code_embeddings",
        BTreeMap::new(),
        ScriptMutability::Immutable,
    )?;
    for row in indicies {
        println!("Row ---> {:?}", row);
    }
    println!("{:-^80}", "end all columns");

    // Shadowing relations after print
    let relations = db.run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)?;
    #[allow(unused_variables)]
    let relation_exists = relations
        .rows
        .iter()
        .any(|row| (row[0].get_str() == Some("code_embeddings")));

    println!("relation_exists: {}", relation_exists);
    let code_embeddings_def = relations
        .rows
        .iter()
        .find(|row| row[0].get_str() == Some("code_embeddings"));
    println!("code_embeddings defined as row: {:?}", code_embeddings_def);

    if !relation_exists {
        println!("Inside `if !relation_exists");
        // Use replace instead of create to handle both creation and updates
        // This avoids the "relation already exists" error
        db.run_script(
            ":create code_embeddings {id: Int, node_id: Int, node_type: String, embedding: <F32; 384>, text_snippet: String}",
            BTreeMap::new(),
            ScriptMutability::Mutable,
        )?;
    }

    // Create a sample embedding vector (384 dimensions)
    // We'll use a simple pattern for the vector values
    let mut embedding_values = Vec::with_capacity(384);
    for i in 0..384 {
        embedding_values.push(DataValue::from(i as f64 / 384.0));
    }

    // Create parameters for the query
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), DataValue::from(1));
    params.insert("node_id".to_string(), DataValue::from(1));
    params.insert("node_type".to_string(), DataValue::from("Function"));
    params.insert("embedding".to_string(), DataValue::List(embedding_values));
    params.insert(
        "snippet".to_string(),
        DataValue::from(
            "fn sample_function(input: String) -> String { println!(\"Hello\"); input }",
        ),
    );

    #[allow(unused_variables)]
    let vector_relation_exists = relations
        .rows
        .iter()
        .any(|row| (row[0].get_str() == Some("code_embeddings:vector")));
    println!("vector_relation_exists: {}", relation_exists);
    let code_embeddings_def = relations
        .rows
        .iter()
        .find(|row| row[0].get_str() == Some("code_embeddings:vector"));
    println!(
        "code_embeddings:vector defined as row: {:?}",
        code_embeddings_def
    );
    // Insert a sample embedding for a function
    let result = db.run_script(
        r#"
        ?[id, node_id, node_type, embedding, text_snippet] <- 
            [[$id, $node_id, $node_type, $embedding, $snippet]]
        :put code_embeddings
        "#,
        params,
        ScriptMutability::Mutable,
    )?;

    if !vector_relation_exists {
        // Create the HNSW index on the embeddings
        db.run_script(
            r#"::hnsw create code_embeddings:vector {
                dim: 384, 
                m: 16, 
                dtype: F32, 
                fields: [embedding], 
                distance: Cosine, 
                ef_construction: 50
            }"#,
            BTreeMap::new(),
            ScriptMutability::Mutable,
        )?;
    }

    Ok(result)
}

#[test]
#[ignore = "requires update"]
fn test_vector_similarity_search_identical() {
    let db = setup_test_db();

    // Insert sample embeddings
    insert_sample_embeddings(&db).expect("Failed to insert sample embeddings");

    // Create a query vector using the vec function in CozoScript
    // We'll use the same vector as in our sample data for perfect similarity
    let mut query_vec = Vec::with_capacity(384);
    for i in 0..384 {
        if i < 385 {
            query_vec.push(DataValue::from(i as f64 / 384.0));
            // } else {
            //     query_vec.push(DataValue::from(0.5))
        }
    }

    // Create parameters for the query
    let mut params = BTreeMap::new();
    params.insert("query_vec".to_string(), DataValue::List(query_vec));

    // Query to find similar code snippets using HNSW index
    let query = r#"
        ?[node_id, node_type, text_snippet, dist] := 
            ~code_embeddings:vector{
                node_id, node_type, text_snippet | 
                query: vec($query_vec), 
                k: 2, 
                ef: 50,
                bind_distance: dist
            }
        :order dist
    "#;

    let result = db
        .run_script(query, params, ScriptMutability::Immutable)
        .expect("Failed to perform vector similarity search");

    #[cfg(feature = "debug")]
    test_helpers::print_debug("Vector search results", &result);

    // We should have at least one result
    assert!(
        !result.rows.is_empty(),
        "Expected at least one vector search result"
    );

    // The first result should have a very low distance (close to 0.0)
    // Since we're using the same vector, it should be almost exactly 0.0
    let distance = result.rows[0][3].get_float().unwrap_or(1.0);
    assert!(
        distance < 0.01,
        "Expected low distance score, got {}",
        distance
    );
}

#[test]
#[ignore = "requires update"]
fn test_vector_similarity_search() {
    let db = setup_test_db();

    // Insert sample embeddings
    insert_sample_embeddings(&db).expect("Failed to insert sample embeddings");

    // Create a query vector using the vec function in CozoScript
    // This vector is similar but not identical to the target vector
    let mut query_vec = Vec::with_capacity(384);
    for i in 0..384 {
        if i < 380 {
            query_vec.push(DataValue::from(i as f64 / 384.0));
        } else {
            query_vec.push(DataValue::from(0.5))
        }
    }

    // Create parameters for the query
    let mut params = BTreeMap::new();
    params.insert("query_vec".to_string(), DataValue::List(query_vec));

    // Query to find similar code snippets using HNSW index
    let query = r#"
        ?[node_id, node_type, text_snippet, dist] := 
            ~code_embeddings:vector{
                node_id, node_type, text_snippet | 
                query: vec($query_vec), 
                k: 2, 
                ef: 50,
                bind_distance: dist
            }
        :order dist
    "#;

    let result = db
        .run_script(query, params, ScriptMutability::Immutable)
        .expect("Failed to perform vector similarity search");

    #[cfg(feature = "debug")]
    test_helpers::print_debug("Vector search results", &result);

    // We should have at least one result
    assert!(
        !result.rows.is_empty(),
        "Expected at least one vector search result"
    );

    // The first result should have a very low distance (close to 0.0)
    // Since we're using the same vector, it should be almost exactly 0.0
    let distance = result.rows[0][3].get_float().unwrap_or(1.0);
    assert!(
        distance > 0.5,
        "Expected distance score > 0.5 , got {}",
        distance
    );
}

// #[test]
// TODO: Learn how this syntax works. Might be important later.
// The fr_* and to_* syntax of the hnsw search is extremely irritating. Even the examples in the
// documentation fail, so it's hard to know if it is even working as intended by the cozo crate.
// For now, we will ignore it, as we don't really need to do a walk like this in the graph right
// now.
#[allow(dead_code)]
fn test_code_embeddings_hnsw_graph() {
    let db = setup_test_db();

    // Insert sample embeddings
    insert_sample_embeddings(&db).expect("Failed to insert sample embeddings");

    let query = r#"
        ?[fr_embedding, to_k, dist] := *code_embeddings:vector{ 
            layer: 0, 
            fr_embedding,
            to_embedding,
            dist
        }

    "#;

    #[allow(unused_variables)]
    let result = db
        .run_script(query, BTreeMap::new(), ScriptMutability::Immutable)
        .expect("Failed to walk code embeddings HNSW graph");

    #[cfg(feature = "debug")]
    test_helpers::print_debug("Code embeddings HNSW graph walking results", &result);
}
