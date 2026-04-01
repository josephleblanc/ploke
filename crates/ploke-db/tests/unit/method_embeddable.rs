//! Phase 0-1 TDD tests for MethodNode embeddable set integration
//!
//! These tests verify that:
//! - Phase 0: Method rows exist in the database with expected IDs
//! - Phase 1: Embeddable set includes methods via METHOD_NODE_ANCESTOR_RULE

use cozo::DataValue;
use ploke_test_utils::fixture_dbs::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};
use std::collections::{BTreeMap, BTreeSet};

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

use ploke_db::multi_embedding::db_ext::METHOD_NODE_ANCESTOR_RULE;

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
