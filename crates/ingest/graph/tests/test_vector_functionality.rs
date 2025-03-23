//! Tests for vector functionality in CozoDB

use cozo::{DataValue, ScriptMutability};
use std::collections::BTreeMap;
use crate::test_helpers::setup_test_db;

mod test_helpers;

#[test]
fn test_basic_vector_functionality() {
    let db = setup_test_db();
    
    // Create a simple relation with vector field
    db.run_script(
        ":create vector_test {id: Int => vec_data: <F32; 3>}",
        BTreeMap::new(),
        ScriptMutability::Mutable,
    ).expect("Failed to create vector_test relation");

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
    ).expect("Failed to insert vectors");

    // Create HNSW index on the vector field
    db.run_script(
        "::hnsw create vector_test:vector_idx {dim: 3, m: 10, ef_construction: 20, fields: [vec_data]}",
        BTreeMap::new(),
        ScriptMutability::Mutable,
    ).expect("Failed to create HNSW index");

    // Query all vectors to verify insertion
    let _result = db.run_script(
        "?[id, vec_data] := *vector_test[id, vec_data]",
        BTreeMap::new(),
        ScriptMutability::Immutable,
    ).expect("Failed to query vectors");

    assert_eq!(
        _result.rows.len(),
        4,
        "Expected 4 vectors in the test relation"
    );

    // Test vector similarity search
    let result = db.run_script(
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
    ).expect("Failed to perform vector search");

    assert!(
        result.rows.len() >= 1,
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
    ).expect("Failed to create vector_test relation");

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
    ).expect("Failed to insert vectors");

    db.run_script(
        "::hnsw create vector_test:vector_idx {dim: 3, m: 10, ef_construction: 20, fields: [vec_data]}",
        BTreeMap::new(),
        ScriptMutability::Mutable,
    ).expect("Failed to create HNSW index");

    // Test walking the HNSW graph directly
    let result = db.run_script(
        r#"
        ?[fr_id, to_id, dist] := 
            *vector_test:vector_idx{layer: 0, fr_id, to_id, dist}
        :limit 10
        "#,
        BTreeMap::new(),
        ScriptMutability::Immutable,
    ).expect("Failed to walk HNSW graph");

    // The graph should have some connections
    #[cfg(feature = "debug")]
    test_helpers::print_debug("HNSW graph connections", &result);
}

fn insert_sample_embeddings(db: &cozo::Db<cozo::MemStorage>) -> Result<cozo::NamedRows, cozo::Error> {
    // First create the relation
    db.run_script(
        ":create code_embeddings {id: Int, node_id: Int, node_type: String, embedding: <F32; 384>, text_snippet: String}",
        BTreeMap::new(),
        ScriptMutability::Mutable,
    )?;
    
    // Create a sample embedding vector (384 dimensions)
    // We'll use a simple pattern for the vector values
    let mut embedding_values = Vec::with_capacity(384);
    for i in 0..384 {
        embedding_values.push(DataValue::from(i as f32 / 384.0));
    }
    
    // Create parameters for the query
    let mut params = BTreeMap::new();
    params.insert("id".to_string(), DataValue::from(1));
    params.insert("node_id".to_string(), DataValue::from(1));
    params.insert("node_type".to_string(), DataValue::from("Function"));
    params.insert("embedding".to_string(), DataValue::from(embedding_values));
    params.insert("snippet".to_string(), DataValue::from("fn sample_function(input: String) -> String { println!(\"Hello\"); input }"));
    
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
    
    // Create the HNSW index on the embeddings
    db.run_script(
        r#"
        ::hnsw create code_embeddings:vector {
            dim: 384,
            m: 16,
            dtype: F32,
            fields: [embedding],
            distance: Cosine,
            ef_construction: 50
        }
        "#,
        BTreeMap::new(),
        ScriptMutability::Mutable
    )?;
    
    Ok(result)
}

#[test]
fn test_vector_similarity_search() {
    let db = setup_test_db();
    
    // Insert sample embeddings
    insert_sample_embeddings(&db).expect("Failed to insert sample embeddings");
    
    // Create a query vector using the vec function in CozoScript
    // We'll use the same vector as in our sample data for perfect similarity
    let mut query_vec = Vec::with_capacity(384);
    for i in 0..384 {
        query_vec.push(DataValue::from(i as f32 / 384.0));
    }
    
    // Create parameters for the query
    let mut params = BTreeMap::new();
    params.insert("query_vec".to_string(), DataValue::from(query_vec));
    
    // Query to find similar code snippets using HNSW index
    let query = r#"
        ?[node_id, node_type, text_snippet, dist] := 
            ~code_embeddings:vector{
                node_id, node_type, text_snippet | 
                query: $query_vec, 
                k: 5, 
                ef: 50,
                bind_distance: dist
            }
        :order dist
    "#;
    
    let result = db.run_script(query, params, ScriptMutability::Immutable)
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
fn test_code_embeddings_hnsw_graph() {
    let db = setup_test_db();
    
    // Insert sample embeddings
    insert_sample_embeddings(&db).expect("Failed to insert sample embeddings");
    
    // Query to walk the HNSW graph at layer 0
    let query = r#"
        ?[fr_node_id, to_node_id, dist] := 
            *code_embeddings:vector{
                layer: 0, 
                fr_k: fr_node_id, 
                to_k: to_node_id, 
                dist
            }
        :limit 10
    "#;
    
    let _result = db.run_script(query, BTreeMap::new(), ScriptMutability::Immutable)
        .expect("Failed to walk code embeddings HNSW graph");
    
    #[cfg(feature = "debug")]
    test_helpers::print_debug("Code embeddings HNSW graph walking results", &_result);
}
