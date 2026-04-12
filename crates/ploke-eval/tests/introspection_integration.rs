//! Integration tests for the introspection API
//!
//! These tests verify that the RunRecord introspection API works correctly against
//! real eval run data from the BurntSushi/ripgrep benchmark.
//!
//! Test Data Location:
//! - Record: ~/.ploke-eval/runs/BurntSushi__ripgrep-2209/record.json.gz
//! - DB: ~/.ploke-eval/runs/BurntSushi__ripgrep-2209/final-snapshot.db
//! - Source: ~/.ploke-eval/repos/BurntSushi/ripgrep
//!
//! Ground Truth Verification:
//! - GlobSet struct: confirmed in ~/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/src/lib.rs
//! - Crate count: 9 (grep, grep-cli, grep-pcre2, globset, grep-searcher, ignore, grep-printer, grep-regex, grep-matcher)

use ploke_eval::record::{RunRecord, read_compressed_record};
use std::path::PathBuf;

/// Path to the test run record
fn test_record_path() -> PathBuf {
    PathBuf::from("/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/record.json.gz")
}

/// Path to the test database
fn test_db_path() -> PathBuf {
    PathBuf::from("/home/brasides/.ploke-eval/runs/BurntSushi__ripgrep-2209/final-snapshot.db")
}

/// Helper to load the test record
fn load_test_record() -> RunRecord {
    let path = test_record_path();
    assert!(
        path.exists(),
        "Test record not found at {:?}. Run an eval to generate it.",
        path
    );
    read_compressed_record(&path).expect("Failed to read compressed record")
}

/// Helper to open the test database
async fn open_test_db() -> ploke_db::Database {
    let path = test_db_path();
    assert!(
        path.exists(),
        "Test database not found at {:?}. Run an eval to generate it.",
        path
    );
    ploke_db::Database::create_new_backup_default(&path)
        .await
        .expect("Failed to open DB from backup")
}

// ====================================================================================
// 1. SetupPhase Verification Tests
// ====================================================================================

#[test]
fn setup_phase_has_indexed_crates() {
    let record = load_test_record();

    // Verify SetupPhase exists
    let setup = record
        .phases
        .setup
        .as_ref()
        .expect("SetupPhase should be populated");

    // Verify we have 9 crates from ripgrep workspace
    assert_eq!(
        setup.indexed_crates.len(),
        9,
        "Expected 9 indexed crates for ripgrep workspace"
    );

    // Collect crate names
    let crate_names: Vec<&str> = setup
        .indexed_crates
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    // Verify expected crate names
    let expected_crates = [
        "grep",          // Core grep library
        "grep-cli",      // CLI utilities
        "grep-pcre2",    // PCRE2 regex support
        "globset",       // Glob pattern matching - contains GlobSet struct
        "grep-searcher", // File searching
        "ignore",        // Gitignore handling
        "grep-printer",  // Output formatting
        "grep-regex",    // Regex engine abstraction
        "grep-matcher",  // Matcher trait definitions
    ];

    for expected in &expected_crates {
        assert!(
            crate_names.contains(expected),
            "Expected crate '{}' not found in indexed crates: {:?}",
            expected,
            crate_names
        );
    }

    // Verify each crate has valid data
    for crate_summary in &setup.indexed_crates {
        assert!(
            !crate_summary.name.is_empty(),
            "Crate name should not be empty"
        );
        assert!(
            crate_summary.node_count > 0,
            "Crate {} should have nodes",
            crate_summary.name
        );
    }
}

#[test]
fn setup_phase_has_valid_db_timestamp() {
    let record = load_test_record();

    let setup = record
        .phases
        .setup
        .as_ref()
        .expect("SetupPhase should be populated");

    // DB timestamp should be positive (microseconds since epoch)
    assert!(
        setup.db_timestamp_micros > 0,
        "DB timestamp should be positive, got {}",
        setup.db_timestamp_micros
    );
}

// ====================================================================================
// 2. DbState::lookup() Tests
// ====================================================================================

#[tokio::test]
async fn lookup_finds_known_structs() {
    let record = load_test_record();
    let db = open_test_db().await;

    // Get the first turn to get a valid timestamp
    let turn = record
        .phases
        .agent_turns
        .first()
        .expect("Should have at least one turn");

    let db_state = turn.db_state();

    // Test: lookup("GlobSet") should find the struct
    // Ground truth: grep -r "pub struct GlobSet" in ripgrep source confirms it exists
    // at ~/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/src/lib.rs
    let node_info = db_state
        .lookup(&db, "GlobSet")
        .expect("lookup('GlobSet') should execute")
        .expect("lookup('GlobSet') should find the struct");
    assert_eq!(
        node_info.name, "GlobSet",
        "Found node should have name 'GlobSet'"
    );
    assert!(
        node_info.node_type.to_lowercase().contains("struct"),
        "Node type should indicate a struct, got: {}",
        node_info.node_type
    );
}

#[tokio::test]
async fn lookup_finds_known_functions() {
    let record = load_test_record();
    let db = open_test_db().await;

    let turn = record
        .phases
        .agent_turns
        .first()
        .expect("Should have at least one turn");

    let db_state = turn.db_state();

    // Test: lookup("new") should find constructor functions
    // Ground truth: Many structs in ripgrep have `new()` constructors
    let node_info = db_state
        .lookup(&db, "new")
        .expect("lookup('new') should execute")
        .expect("lookup('new') should find a function or method");
    assert_eq!(node_info.name, "new", "Found node should have name 'new'");
    assert!(
        node_info.node_type.to_lowercase().contains("function")
            || node_info.node_type.to_lowercase().contains("method"),
        "Node type should indicate a function or method, got: {}",
        node_info.node_type
    );
}

#[tokio::test]
async fn lookup_returns_none_for_nonexistent() {
    let record = load_test_record();
    let db = open_test_db().await;

    let turn = record
        .phases
        .agent_turns
        .first()
        .expect("Should have at least one turn");

    let db_state = turn.db_state();

    // Test: lookup("ThisDoesNotExist12345") should return Ok(None)
    // This tests the bad path - looking up something that definitely doesn't exist
    let result = db_state
        .lookup(&db, "ThisDoesNotExist12345")
        .expect("lookup for missing node should execute");
    assert!(
        result.is_none(),
        "lookup should not find a node named 'ThisDoesNotExist12345'"
    );
}

// ====================================================================================
// 3. RunRecord::replay_query() Tests
// ====================================================================================

#[tokio::test]
async fn replay_query_returns_historical_data() {
    let record = load_test_record();
    let db = open_test_db().await;

    // Test: Query structs at turn 1's timestamp
    // Query: ?[name] := *struct{name @ 'NOW'}
    let query = "?[name] := *struct{name @ 'NOW'}";

    let query_result = record
        .replay_query(1, &db, query)
        .expect("replay_query for structs should execute");
    assert!(
        !query_result.rows.is_empty(),
        "Query should return structs from the database"
    );
    assert!(
        query_result.headers.iter().any(|h| h == "name"),
        "Query result should have 'name' column"
    );
    let has_globset = query_result.rows.iter().any(|row| {
        row.iter()
            .any(|val| val.get_str().map(|s| s == "GlobSet").unwrap_or(false))
    });
    assert!(has_globset, "Expected 'GlobSet' in struct query results");
}

#[tokio::test]
async fn replay_query_functions_at_turn() {
    let record = load_test_record();
    let db = open_test_db().await;

    // Test: Query functions at turn 1's timestamp
    let query = "?[count(id)] := *function{id @ 'NOW'}";

    let query_result = record
        .replay_query(1, &db, query)
        .expect("replay_query for function count should execute");
    assert!(
        !query_result.rows.is_empty(),
        "Query should return function count"
    );
    let count = query_result.rows[0][0]
        .get_int()
        .expect("Function count query should return an integer");
    assert!(
        count > 0,
        "Function count should be positive, got {}",
        count
    );
}

#[tokio::test]
async fn replay_query_returns_error_for_nonexistent_turn() {
    let record = load_test_record();
    let db = open_test_db().await;

    // Test: Query at turn 99 (which doesn't exist)
    let query = "?[name] := *struct{name @ 'NOW'}";

    let result = record.replay_query(99, &db, query);

    // Should return an error since turn 99 doesn't exist
    assert!(
        result.is_err(),
        "replay_query should error for non-existent turn"
    );

    let err = result.unwrap_err();
    let err_msg = err.to_string();
    assert!(
        err_msg.contains("not found") || err_msg.contains("Timestamp"),
        "Error should indicate turn not found: {}",
        err_msg
    );
}

// ====================================================================================
// 4. Iterator Method Tests (placeholder - will implement later)
// ====================================================================================

#[test]
fn conversations_returns_turns() {
    let record = load_test_record();

    // Test: run.conversations() should return an iterator over turns
    let mut count = 0;
    for turn in record.conversations() {
        count += 1;
        assert!(turn.turn_number > 0, "Turn number should be positive");
        assert!(
            turn.db_timestamp_micros > 0,
            "Turn should have valid timestamp"
        );
    }

    assert!(count > 0, "Run should have at least one turn");
    assert_eq!(
        count,
        record.turn_count(),
        "conversations() should return all turns"
    );
}

#[test]
fn tool_calls_returns_all_calls() {
    let record = load_test_record();

    // Test: run.tool_calls() should aggregate from all turns
    let aggregated: Vec<_> = record.tool_calls();

    // Manually calculate expected count using the tool_calls() method
    // (not the raw field, since tool_calls() extracts from artifact events when needed)
    let expected_count: usize = record
        .phases
        .agent_turns
        .iter()
        .map(|t| t.tool_calls().len())
        .sum();

    assert_eq!(
        aggregated.len(),
        expected_count,
        "tool_calls() should return all tool calls from all turns"
    );

    println!("Total tool calls in run: {}", aggregated.len());

    // Verify all returned items are tool execution records
    for call in &aggregated {
        assert!(
            !call.request.tool.is_empty(),
            "Tool name should not be empty"
        );
    }
}

// ====================================================================================
// 5. Additional Integration Tests
// ====================================================================================

#[test]
fn run_record_has_valid_metadata() {
    let record = load_test_record();

    // Verify schema version
    assert_eq!(
        record.schema_version, "run-record.v1",
        "Schema version should be 'run-record.v1'"
    );

    // Verify manifest ID matches expected
    assert_eq!(
        record.manifest_id, "BurntSushi__ripgrep-2209",
        "Manifest ID should match the run directory"
    );

    // Verify benchmark metadata
    assert_eq!(
        record.metadata.benchmark.instance_id, "BurntSushi__ripgrep-2209",
        "Benchmark instance ID should match"
    );
}

#[test]
fn time_travel_index_matches_turns() {
    let record = load_test_record();

    // Verify db_time_travel_index has entries for turns
    assert!(
        !record.db_time_travel_index.is_empty(),
        "Time travel index should not be empty"
    );

    // Verify each turn has a corresponding timestamp
    for turn in &record.phases.agent_turns {
        let timestamp = record.timestamp_for_turn(turn.turn_number);
        assert!(
            timestamp.is_some(),
            "Turn {} should have a timestamp in the index",
            turn.turn_number
        );
        assert_eq!(
            timestamp.unwrap(),
            turn.db_timestamp_micros,
            "Timestamp should match turn's db_timestamp_micros"
        );
    }
}

#[tokio::test]
async fn db_state_query_executes_at_timestamp() {
    let record = load_test_record();
    let db = open_test_db().await;

    let turn = record
        .phases
        .agent_turns
        .first()
        .expect("Should have at least one turn");

    let db_state = turn.db_state();

    // Test: Execute a query at the turn's timestamp
    let query = "?[count(id)] := *struct{id @ 'NOW'}";
    let result = db_state.query(&db, query);
    let query_result = result.expect("db_state.query should execute successfully");
    assert!(!query_result.rows.is_empty(), "Query should return results");
}
