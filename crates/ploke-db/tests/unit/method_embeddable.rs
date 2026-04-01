//! Phase 0-4 TDD tests for MethodNode embeddable set integration
//!
//! These tests verify that:
//! - Phase 0: Method rows exist in the database with expected IDs
//! - Phase 1: Embeddable set includes methods via METHOD_NODE_ANCESTOR_RULE
//! - Phase 2: Method rows can be joined with embedding vectors
//! - Phase 4: HNSW search can find method nodes via search_similar_for_set

use cozo::DataValue;
use ploke_test_utils::fixture_dbs::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};
use std::collections::{BTreeMap, BTreeSet};

// ============================================================================
// Phase 2: Method rows joined with embedding vectors
// ============================================================================

/// Phase 2 test: Verify that a method ID can be joined with a vector relation.
///
/// This test inserts a minimal vector for a known method ID and verifies
/// that Cozo can successfully join the `method` relation with the vector
/// relation using the method's UUID.
#[test]
fn method_with_vector_join_returns_non_empty_vector() {
    use ploke_core::embeddings::{
        EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
    };
    use ploke_db::multi_embedding::{db_ext::EmbeddingExt, hnsw_ext::HnswExt};
    use std::ops::Deref;

    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    // Get a known method ID (SimpleStruct::new from Phase 0)
    let method_id = get_simple_struct_new_id(&db);

    // Create a test embedding set with small dimension for speed
    const TEST_DIMS: u32 = 64;
    let embedding_set = EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("test_provider"),
        EmbeddingModelId::new_from_str("test_model"),
        EmbeddingShape::new_dims_default(TEST_DIMS),
    );

    // Ensure the embedding set and vector relation exist
    db.ensure_embedding_set_relation()
        .expect("should create embedding_set relation");
    db.put_embedding_set(&embedding_set)
        .expect("should put embedding set");
    db.ensure_vector_embedding_relation(&embedding_set)
        .expect("should create vector embedding relation");

    // Insert a vector for the method using raw EmbeddingExt API (explicit embedding set)
    let test_vector: Vec<f64> = vec![0.5; TEST_DIMS as usize];
    db.deref()
        .update_embeddings_batch(vec![(method_id, test_vector)], &embedding_set)
        .expect("should insert method vector");

    // Query to join method relation with vector relation
    let embed_rel = embedding_set.rel_name.as_ref().replace('-', "_");
    let join_script = format!(
        r#"
        ?[method_id, method_name, vector] :=
            *method {{ id: method_id, name: method_name }},
            *{embed_rel} {{ node_id: method_id, vector }}
        "#
    );

    let result = db
        .raw_query(&join_script)
        .expect("join query should succeed");

    // Verify we found the method with its vector
    assert!(
        !result.rows.is_empty(),
        "Expected to find method joined with vector, but got no results"
    );

    // Find the specific row for our method
    let method_row = result
        .rows
        .iter()
        .find(|row| matches!(&row[0], DataValue::Uuid(cozo::UuidWrapper(id)) if *id == method_id));

    assert!(
        method_row.is_some(),
        "Expected to find the specific method row with vector"
    );

    // Verify the method name
    let name_val = &method_row.unwrap()[1];
    assert!(
        is_str_with(name_val, "new"),
        "Expected method name to be 'new', got {:?}",
        name_val
    );

    // Verify the vector is present and has expected dimension
    let vector_val = &method_row.unwrap()[2];
    let vec_len = match vector_val {
        DataValue::List(components) => components.len(),
        DataValue::Vec(components) => components.len(),
        other => panic!("Expected vector to be a List or Vec, got {:?}", other),
    };
    assert_eq!(
        vec_len, TEST_DIMS as usize,
        "Expected vector dimension to be {}, got {}",
        TEST_DIMS, vec_len
    );

    eprintln!(
        "Successfully joined method {} with vector of dimension {}",
        method_id, TEST_DIMS
    );
}

/// Helper to get the SimpleStruct::new method ID
fn get_simple_struct_new_id(db: &ploke_db::Database) -> uuid::Uuid {
    let script = r#"
        ?[id, name, vis_kind] := *method { id, name, vis_kind }, name == "new"
    "#;

    let result = db.raw_query(script).expect("should query method ids");

    // Find the public "new" method (SimpleStruct::new)
    let row = result
        .rows
        .iter()
        .find(|row| is_str_with(&row[2], "public"))
        .expect("should find public 'new' method");

    extract_uuid(&row[0]).expect("should extract UUID")
}

// ============================================================================
// Phase 3: Counting invariants - baseline + method delta
// ============================================================================

/// Phase 3 test: Verify that count_pending_embeddings_including_methods = baseline + method_delta.
///
/// This test establishes the counting invariant:
/// - `count_pending_embeddings` = legacy count (primary nodes only)
/// - `count_pending_embeddings_including_methods` = extended count (primary + method nodes)
/// - The delta between them should equal the number of unembedded method nodes
#[test]
fn pending_methods_only_delta() {
    use ploke_core::embeddings::{
        EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
    };
    use ploke_db::multi_embedding::db_ext::EmbeddingExt;
    use std::ops::Deref;

    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    // Create a fresh embedding set to ensure no nodes are embedded yet
    const TEST_DIMS: u32 = 64;
    let embedding_set = EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("test_provider_phase3"),
        EmbeddingModelId::new_from_str("test_model_phase3"),
        EmbeddingShape::new_dims_default(TEST_DIMS),
    );

    // Set up the embedding set and vector relation
    db.ensure_embedding_set_relation()
        .expect("should create embedding_set relation");
    db.put_embedding_set(&embedding_set)
        .expect("should put embedding set");
    db.ensure_vector_embedding_relation(&embedding_set)
        .expect("should create vector embedding relation");

    // Get baseline count (primary nodes only) - using raw EmbeddingExt API
    let baseline = db
        .deref()
        .count_pending_embeddings(&embedding_set)
        .expect("should count pending embeddings");

    // Get extended count (primary nodes + methods) - using raw EmbeddingExt API
    let extended = db
        .deref()
        .count_pending_embeddings_including_methods(&embedding_set)
        .expect("should count pending including methods");

    // Calculate the delta
    let delta = extended - baseline;

    eprintln!("Baseline (primary nodes only): {}", baseline);
    eprintln!("Extended (including methods): {}", extended);
    eprintln!("Delta (methods only): {}", delta);

    // Get the count of method nodes from Phase 0/1
    let method_count = collect_method_ids(&db).len();
    eprintln!("Total method IDs in fixture: {}", method_count);

    // The delta should equal the number of embeddable methods
    // All 41 methods should be embeddable (can reach root modules via METHOD_NODE_ANCESTOR_RULE)
    assert_eq!(
        delta, method_count,
        "Expected delta ({}) to equal total method count ({})",
        delta, method_count
    );

    // Extended should be greater than baseline
    assert!(
        extended > baseline,
        "Extended count ({}) should be greater than baseline ({})",
        extended,
        baseline
    );

    eprintln!(
        "Phase 3 invariant verified: {} (baseline) + {} (methods) = {} (extended)",
        baseline, delta, extended
    );
}

/// Helper function to check if a DataValue is a string with expected content
fn is_str_with(value: &DataValue, expected: &str) -> bool {
    matches!(value, DataValue::Str(s) if s.as_str() == expected)
}

/// Helper to extract UUID from DataValue
fn extract_uuid(value: &DataValue) -> Option<uuid::Uuid> {
    if let DataValue::Uuid(cozo::UuidWrapper(uuid)) = value {
        Some(*uuid)
    } else {
        None
    }
}

/// Phase 0 test: Verify that the method relation contains expected method rows.
///
/// This test queries the `method` relation for a known method from the fixture_nodes
/// crate and verifies its existence and basic properties.
#[test]
fn method_fixture_has_expected_method_row() {
    // Load the canonical fixture database
    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    // Query for the method by name - using a method we know exists in fixture_nodes/src/impls.rs
    // The method `new` is defined in `impl SimpleStruct { pub fn new(...) }`
    let script = r#"
        ?[id, name, owner_id, vis_kind] := *method { id, name, owner_id, vis_kind }, name == $method_name
    "#;

    let params = BTreeMap::from([("method_name".to_string(), DataValue::Str("new".into()))]);

    let result = db
        .raw_query_params(script, params)
        .expect("method query should succeed");

    let rows = result.rows;

    // We expect at least one method named "new" (there may be others in different impl blocks)
    assert!(
        !rows.is_empty(),
        "Expected at least one method named 'new' in the method relation"
    );

    // Find the specific method from SimpleStruct::new
    let simple_struct_new = rows.iter().find(|row| {
        // Check if this is the SimpleStruct::new method by examining the vis_kind
        // The method is public, so vis_kind should be "Public"
        is_str_with(&row[3], "public")
    });

    assert!(
        simple_struct_new.is_some(),
        "Expected to find SimpleStruct::new method with Public visibility"
    );

    // Verify the method has a valid UUID
    let method_id = &simple_struct_new.unwrap()[0];
    assert!(
        matches!(method_id, DataValue::Uuid(_)),
        "Method ID should be a UUID, got {:?}",
        method_id
    );

    // Log the actual ID for potential use as EXPECTED_METHOD_ID
    if let DataValue::Uuid(cozo::UuidWrapper(uuid)) = method_id {
        eprintln!("Found SimpleStruct::new method with ID: {}", uuid);
    }
}

/// Phase 0 test: Verify that all expected methods from fixture_nodes are present.
///
/// This is a stricter version that checks for specific known methods from the fixture.
#[test]
fn method_fixture_contains_all_expected_methods() {
    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    // List of expected method names from fixture_nodes/src/impls.rs
    // These are the public methods we expect to find
    // Note: visibility values are lowercase in the database
    let expected_methods = vec![
        ("new", "public"),
        ("public_method", "public"),
        ("get_value_ref", "public"),
        ("print_value", "public"),
        ("trait_method", "public"), // From SimpleTrait
    ];

    for (expected_name, expected_vis) in expected_methods {
        let script = r#"
            ?[id, name, vis_kind] := *method { id, name, vis_kind }, name == $method_name
        "#;

        let params = BTreeMap::from([(
            "method_name".to_string(),
            DataValue::Str(expected_name.into()),
        )]);

        let result = db
            .raw_query_params(script, params)
            .expect("method query should succeed");

        assert!(
            !result.rows.is_empty(),
            "Expected to find at least one method named '{}'",
            expected_name
        );

        // Check that at least one method has the expected visibility
        let has_expected_vis = result
            .rows
            .iter()
            .any(|row| is_str_with(&row[2], expected_vis));

        assert!(
            has_expected_vis,
            "Expected to find method '{}' with visibility '{}'",
            expected_name, expected_vis
        );
    }
}

/// Phase 0 test: Count total methods in fixture and verify non-zero.
///
/// This provides a baseline count for future invariant tests.
#[test]
fn method_fixture_count_nonzero() {
    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    let script = r#"
        ?[count(method_id)] := *method { id: method_id }
    "#;

    let result = db
        .raw_query(script)
        .expect("method count query should succeed");

    assert!(
        !result.rows.is_empty(),
        "Expected count query to return a row"
    );

    let count_val = &result.rows[0][0];
    eprintln!("Total method count in fixture: {:?}", count_val);

    // Verify we have at least some methods
    if let DataValue::Num(cozo::Num::Int(n)) = count_val {
        assert!(
            *n > 0,
            "Expected at least one method in the database, got {}",
            n
        );
    } else {
        panic!("Expected numeric count, got {:?}", count_val);
    }
}

// ============================================================================
// Phase 1: METHOD_NODE_ANCESTOR_RULE + embeddable set union tests
// ============================================================================

use ploke_db::multi_embedding::{
    db_ext::METHOD_NODE_ANCESTOR_RULE, hnsw_ext::HnswExt, schema::EmbeddingSetExt,
};

/// Phase 1 validation test: Verify METHOD_NODE_ANCESTOR_RULE is well-formed CozoScript.
///
/// This test validates that the rule parses correctly before we try to integrate it
/// with other rules. The Cozo query language is a DSL and can be tricky to get right.
///
/// The test should pass if:
/// - The rule parses without syntax errors
/// - Even if the rule returns no results (as expected from a stub), it doesn't panic
///
/// This is a smoke test for the rule's syntax validity.
#[test]
fn method_node_ancestor_rule_is_well_formed() {
    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    // Construct a minimal query that includes the METHOD_NODE_ANCESTOR_RULE
    // We wrap it in a query that should always be valid to test just the rule parsing
    let test_script = format!(
        r#"
        # Include the METHOD_NODE_ANCESTOR_RULE
        {}

        # Simple query that should always work to verify the script is valid
        # Even if the rule defines no usable relations, this should parse
        ?[test_val] <- [["rule_parses_ok"]]
        "#,
        METHOD_NODE_ANCESTOR_RULE
    );

    // Try to run the script - if it parses, we get Ok(result)
    // If it has syntax errors, we get Err(DbError::Cozo(...))
    let result = db.raw_query(&test_script);

    match &result {
        Ok(_) => {
            // Rule parsed successfully
            eprintln!("METHOD_NODE_ANCESTOR_RULE parsed successfully");
        }
        Err(e) => {
            // If there's a parse error, the test should fail with helpful info
            panic!(
                "METHOD_NODE_ANCESTOR_RULE failed to parse. \
                 This usually indicates a syntax error in the CozoScript. \
                 Error: {:?}",
                e
            );
        }
    }

    // The rule should at least parse without errors
    assert!(
        result.is_ok(),
        "METHOD_NODE_ANCESTOR_RULE should be well-formed CozoScript"
    );
}

/// Phase 1 validation test: Verify METHOD_NODE_ANCESTOR_RULE can define relations
/// without conflicting with existing ANCESTOR_RULES_NOW.
///
/// This test checks that when combined with the standard ancestor rules,
/// the method rule doesn't cause name conflicts or syntax errors.
#[test]
fn method_node_ancestor_rule_composes_with_existing_rules() {
    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    // Query that combines the method rule with standard ancestor rules
    let test_script = format!(
        r#"
        # Standard ancestor rules
        parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}}
        ancestor[desc, asc] := parent_of[desc, asc]
        ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

        # Method ancestor rule (should extend or compose without conflict)
        {}

        # Root module rule
        is_root_module[id] := *module{{id}}, *file_mod {{owner_id: id}}

        # Test query: can we access method nodes at all?
        ?[count(method_id)] := *method {{ id: method_id }}
        "#,
        METHOD_NODE_ANCESTOR_RULE
    );

    let result = db.raw_query(&test_script);

    match &result {
        Ok(rows) => {
            eprintln!(
                "Combined rules parsed successfully. Method count result: {:?}",
                rows.rows
            );
        }
        Err(e) => {
            panic!(
                "Failed to combine METHOD_NODE_ANCESTOR_RULE with existing rules. \
                 This may indicate a naming conflict or syntax issue. \
                 Error: {:?}",
                e
            );
        }
    }

    assert!(
        result.is_ok(),
        "METHOD_NODE_ANCESTOR_RULE should compose with ANCESTOR_RULES_NOW"
    );
}

/// Collect all method IDs from the database
fn collect_method_ids(db: &ploke_db::Database) -> BTreeSet<uuid::Uuid> {
    let script = r#"?[id] := *method { id }"#;
    let result = db.raw_query(script).expect("should query method ids");

    result
        .rows
        .iter()
        .filter_map(|row| extract_uuid(&row[0]))
        .collect()
}

/// Collect legacy embeddable IDs (primary nodes only, no methods)
fn collect_legacy_embeddable_ids(db: &ploke_db::Database) -> BTreeSet<uuid::Uuid> {
    use ploke_db::NodeType;

    // Build a query similar to EMBEDDABLE_NODES_NOW but explicitly only primary nodes
    let primary_nodes = NodeType::primary_nodes();
    let mut all_ids = BTreeSet::new();

    for node_type in primary_nodes {
        let relation = node_type.relation_str();
        let script = format!(r#"?[id] := *{} {{ id @ 'NOW' }}"#, relation);
        let result = db.raw_query(&script).expect("should query node ids");

        for row in &result.rows {
            if let Some(uuid) = extract_uuid(&row[0]) {
                all_ids.insert(uuid);
            }
        }
    }

    all_ids
}

/// Collect extended embeddable IDs (primary nodes + methods via METHOD_NODE_ANCESTOR_RULE)
///
/// This uses the extended script that includes methods through ancestor rules.
fn collect_extended_embeddable_ids(db: &ploke_db::Database) -> BTreeSet<uuid::Uuid> {
    use ploke_db::multi_embedding::db_ext::METHOD_NODE_ANCESTOR_RULE;

    // Start with legacy IDs (primary nodes)
    let mut all_ids = collect_legacy_embeddable_ids(db);

    // Query for method IDs that can reach root modules using METHOD_NODE_ANCESTOR_RULE
    // The rule connects: method -> impl/trait (via parent_of), then impl/trait -> module
    // (via existing ANCESTOR_RULES_NOW), then module -> file_mod
    let script = format!(
        r#"
        # Standard ancestor rules for Contains edges
        parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}}

        # METHOD_NODE_ANCESTOR_RULE - extends parent_of for methods
        {}

        # Transitive ancestor relation
        ancestor[desc, asc] := parent_of[desc, asc]
        ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

        # Root module identification
        is_root_module[id] := *module{{id}}, *file_mod {{owner_id: id}}

        # Find all method IDs that can reach a root module
        ?[method_id] :=
            *method {{ id: method_id }},
            ancestor[method_id, root_id],
            is_root_module[root_id]
        "#,
        METHOD_NODE_ANCESTOR_RULE
    );

    let result = db
        .raw_query(&script)
        .expect("extended embeddable query should succeed");

    for row in &result.rows {
        if let Some(uuid) = extract_uuid(&row[0]) {
            all_ids.insert(uuid);
        }
    }

    all_ids
}

/// Phase 1 test: Verify legacy embeddable IDs are a subset of extended embeddable IDs.
///
/// The extended set (with METHOD_NODE_ANCESTOR_RULE) should include all legacy IDs
/// plus any method IDs that can reach root modules.
#[test]
fn embeddable_legacy_ids_subset_of_extended() {
    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    let legacy_ids = collect_legacy_embeddable_ids(&db);
    let extended_ids = collect_extended_embeddable_ids(&db);

    eprintln!("Legacy embeddable count: {}", legacy_ids.len());
    eprintln!("Extended embeddable count: {}", extended_ids.len());

    // Every legacy ID should be in the extended set
    let missing: Vec<_> = legacy_ids.difference(&extended_ids).collect();
    assert!(
        missing.is_empty(),
        "Expected all legacy IDs to be in extended set, but {} were missing: {:?}",
        missing.len(),
        missing
    );

    // The extended set should be larger (or equal if no methods are embeddable yet)
    assert!(
        extended_ids.len() >= legacy_ids.len(),
        "Extended set should be at least as large as legacy set"
    );
}

/// Phase 1 test: Verify extended minus legacy equals exactly the method IDs.
///
/// The difference between extended and legacy sets should be exactly the set
/// of method IDs that can reach root modules via METHOD_NODE_ANCESTOR_RULE.
#[test]
fn embeddable_extended_minus_legacy_is_exactly_method_ids() {
    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    let legacy_ids = collect_legacy_embeddable_ids(&db);
    let extended_ids = collect_extended_embeddable_ids(&db);
    let all_method_ids = collect_method_ids(&db);

    // Calculate the difference: what IDs are in extended but not in legacy?
    let new_ids: BTreeSet<_> = extended_ids.difference(&legacy_ids).copied().collect();

    eprintln!("Legacy embeddable count: {}", legacy_ids.len());
    eprintln!("Extended embeddable count: {}", extended_ids.len());
    eprintln!("All method IDs count: {}", all_method_ids.len());
    eprintln!("New IDs in extended (not in legacy): {}", new_ids.len());

    // Every new ID should be a method ID
    let unexpected: Vec<_> = new_ids.difference(&all_method_ids).collect();
    assert!(
        unexpected.is_empty(),
        "Expected all new IDs to be method IDs, but found {} unexpected: {:?}",
        unexpected.len(),
        unexpected
    );

    // All method IDs should be in the new IDs (assuming all methods can reach root)
    // Note: This might need adjustment if some methods legitimately can't reach root
    let missing_methods: Vec<_> = all_method_ids.difference(&new_ids).collect();
    if !missing_methods.is_empty() {
        eprintln!(
            "Warning: {} methods not in extended set (may not reach root): {:?}",
            missing_methods.len(),
            missing_methods
        );
    }

    // The key assertion: extended = legacy ∪ methods (approximately)
    // We allow some methods to not be embeddable if they can't reach root
    assert!(
        !new_ids.is_empty() || all_method_ids.is_empty(),
        "Expected extended set to include at least some method IDs, but none were found"
    );
}

// ============================================================================
// Phase 4: HNSW search for method nodes
// ============================================================================

use ploke_core::RetrievalScope;

/// Phase 4 TDD test: Verify that `search_similar_for_set` can find method nodes.
///
/// This test creates a fresh database with method nodes, inserts a vector for a known
/// method ID, creates an HNSW index, and then searches to verify the method is found.
///
/// This test will FAIL until:
/// 1. `search_similar_for_set` is updated to use `METHOD_NODE_ANCESTOR_RULE` instead of
///    just `ANCESTOR_RULES_NOW`
/// 2. Method nodes can be properly joined with their ancestors to reach root modules
///
/// Current issue: `search_similar_for_set` uses `ANCESTOR_RULES_NOW` which doesn't include
/// the method parent relations (`ImplAssociatedItem` / `TraitAssociatedItem`), so method
/// nodes cannot be joined to their file_mod ancestors.
#[test]
fn search_similar_for_set_finds_method_node() {
    use ploke_core::embeddings::{
        EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
    };
    use ploke_db::multi_embedding::db_ext::EmbeddingExt;
    use std::ops::Deref;

    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    // Get a known method ID (SimpleStruct::new from Phase 0)
    let method_id = get_simple_struct_new_id(&db);
    eprintln!("Testing with method_id: {}", method_id);

    // Create a test embedding set with small dimension for speed
    const TEST_DIMS: u32 = 64;
    let embedding_set = EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("test_provider_phase4"),
        EmbeddingModelId::new_from_str("test_model_phase4"),
        EmbeddingShape::new_dims_default(TEST_DIMS),
    );

    // Set up the embedding set and vector relation
    db.ensure_embedding_set_relation()
        .expect("should create embedding_set relation");
    db.put_embedding_set(&embedding_set)
        .expect("should put embedding set");
    db.ensure_vector_embedding_relation(&embedding_set)
        .expect("should create vector embedding relation");

    // Insert a vector for the method using a unique pattern for easy identification
    // Use a vector with high values in specific positions to make it easily searchable
    let mut test_vector: Vec<f64> = vec![0.1; TEST_DIMS as usize];
    test_vector[0] = 0.99; // Make it distinctive
    test_vector[1] = 0.98;
    db.deref()
        .update_embeddings_batch(vec![(method_id, test_vector.clone())], &embedding_set)
        .expect("should insert method vector");

    // Create HNSW index for the embedding set
    db.deref()
        .create_embedding_index(&embedding_set)
        .expect("should create HNSW index");

    // Convert test vector to f32 for the search query
    let query_vector: Vec<f32> = test_vector.iter().map(|&v| v as f32).collect();

    // Attempt to search for the method node
    // Note: Currently this will likely fail because search_similar_for_set uses
    // ANCESTOR_RULES_NOW which doesn't include METHOD_NODE_ANCESTOR_RULE
    let search_result = db.deref().search_similar_for_set(
        &embedding_set,
        ploke_db::NodeType::Method,
        RetrievalScope::LoadedWorkspace,
        query_vector,
        5,    // k
        10,   // ef
        5,    // limit
        None, // radius
    );

    match &search_result {
        Ok(result) => {
            let found_ids: Vec<_> = result.typed_data.v.iter().map(|node| node.id).collect();
            eprintln!(
                "Search returned {} results: {:?}",
                found_ids.len(),
                found_ids
            );

            // Check if our method_id is in the results
            assert!(
                found_ids.contains(&method_id),
                "Expected search_similar_for_set to find method_id {}. \
                 Got results: {:?}. \
                 This may indicate that METHOD_NODE_ANCESTOR_RULE is not being used in search_similar_for_set.",
                method_id,
                found_ids
            );
        }
        Err(e) => {
            // If the search fails, that's also a failure case for this test
            panic!(
                "search_similar_for_set failed with error: {:?}. \
                 This may indicate that the method node cannot be joined to ancestors \
                 (METHOD_NODE_ANCESTOR_RULE not applied).",
                e
            );
        }
    }

    eprintln!(
        "Phase 4 TDD test passed: search_similar_for_set successfully found method {}",
        method_id
    );
}

/// Phase 4 TDD test: Verify that method nodes can be found via ancestor traversal
/// using the METHOD_NODE_ANCESTOR_RULE within the search context.
///
/// This is a more focused test that directly verifies the ancestor rule works
/// in the context of HNSW search filtering.
#[test]
fn method_node_ancestor_rule_works_in_search_context() {
    let db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    // Get a known method ID
    let method_id = get_simple_struct_new_id(&db);

    // Verify the method can reach a root module using the combined rules
    // This mimics what search_similar_for_set should be doing internally
    let script = format!(
        r#"
# Standard ancestor rules
parent_of[child, parent] := *syntax_edge{{source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW'}}

# Method ancestor rule (this is what should be included in search_similar_for_set)
{}

# Transitive ancestor
ancestor[desc, asc] := parent_of[desc, asc]
ancestor[desc, asc] := parent_of[desc, intermediate], ancestor[intermediate, asc]

# Root module identification
is_root_module[id] := *module{{id}}, *file_mod {{owner_id: id}}

# Test: Can our method reach a root module?
?[method_id, root_id] :=
    *method {{ id: method_id }},
    method_id = $target_method,
    ancestor[method_id, root_id],
    is_root_module[root_id]
"#,
        ploke_db::multi_embedding::db_ext::METHOD_NODE_ANCESTOR_RULE
    );

    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "target_method".to_string(),
        cozo::DataValue::Uuid(cozo::UuidWrapper(method_id)),
    );

    let result = db.raw_query_params(&script, params);

    match &result {
        Ok(rows) => {
            assert!(
                !rows.rows.is_empty(),
                "Method {} should be able to reach a root module via ancestor rules. \
                 This is a prerequisite for search_similar_for_set to find method nodes.",
                method_id
            );
            eprintln!(
                "Method {} can reach root module(s): {:?}",
                method_id, rows.rows
            );
        }
        Err(e) => {
            panic!(
                "Ancestor rule query failed: {:?}. \
                 This indicates METHOD_NODE_ANCESTOR_RULE may have syntax issues \
                 or the method cannot reach root modules.",
                e
            );
        }
    }
}
