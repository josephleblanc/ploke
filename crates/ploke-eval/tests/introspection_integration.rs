//! Canonical integration tests for the ploke-eval introspection API.
//!
//! These tests use a source-controlled, hermetic run-record fixture paired with
//! the registry-backed `fixture_nodes_canonical` database fixture. They exercise
//! the same replay/introspection APIs that historical real-run tests covered,
//! without depending on private `~/.ploke-eval` artifacts or external source
//! checkouts.

use ploke_eval::record::{RunRecord, read_compressed_record};
use ploke_test_utils::fixture_dbs::{FIXTURE_NODES_CANONICAL, fresh_backup_fixture_db};
use std::path::PathBuf;

const FIXTURE_MANIFEST_ID: &str = "ploke-eval-introspection-fixture";
const FIXTURE_CRATE_NAME: &str = "fixture_nodes";
const KNOWN_STRUCT: &str = "SimpleStruct";
const KNOWN_FUNCTION: &str = "new";
const KNOWN_TOOL: &str = "request_code_context";
const MISSING_NODE: &str = "ThisDoesNotExist12345";

fn fixture_record_path() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join("introspection-fixture")
        .join("record.json.gz")
}

fn fixture_db_path() -> PathBuf {
    FIXTURE_NODES_CANONICAL
        .checked_path()
        .expect("fixture_nodes_canonical path should validate")
        .into_path()
}

fn load_test_record() -> RunRecord {
    let path = fixture_record_path();
    assert!(
        path.is_file(),
        "introspection run-record fixture missing at {path:?}"
    );
    read_compressed_record(&path).expect("introspection fixture should deserialize")
}

async fn open_test_db() -> ploke_db::Database {
    let path = fixture_db_path();
    assert!(
        path.is_file(),
        "fixture_nodes_canonical database fixture missing at {path:?}"
    );
    fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("fixture_nodes_canonical should import as a fresh fixture database")
}

// ====================================================================================
// 1. SetupPhase Verification Tests
// ====================================================================================

#[test]
fn setup_phase_has_indexed_crates() {
    let record = load_test_record();

    let setup = record
        .phases
        .setup
        .as_ref()
        .expect("SetupPhase should be populated");

    assert_eq!(
        setup.indexed_crates.len(),
        1,
        "Expected one indexed crate for the hermetic fixture_nodes fixture"
    );

    let crate_names: Vec<&str> = setup
        .indexed_crates
        .iter()
        .map(|c| c.name.as_str())
        .collect();

    assert!(
        crate_names.contains(&FIXTURE_CRATE_NAME),
        "Expected crate '{FIXTURE_CRATE_NAME}' not found in indexed crates: {crate_names:?}"
    );

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

    let turn = record
        .phases
        .agent_turns
        .first()
        .expect("Should have at least one turn");

    let db_state = turn.db_state();

    let node_info = db_state
        .lookup(&db, KNOWN_STRUCT)
        .expect("lookup for fixture struct should execute")
        .expect("lookup should find the fixture struct");
    assert_eq!(
        node_info.name, KNOWN_STRUCT,
        "Found node should have fixture struct name"
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

    let node_info = db_state
        .lookup(&db, KNOWN_FUNCTION)
        .expect("lookup for fixture function should execute")
        .expect("lookup should find the fixture function or method");
    assert_eq!(
        node_info.name, KNOWN_FUNCTION,
        "Found node should have fixture function name"
    );
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

    let result = db_state
        .lookup(&db, MISSING_NODE)
        .expect("lookup for missing node should execute");
    assert!(
        result.is_none(),
        "lookup should not find a node named '{MISSING_NODE}'"
    );
}

// ====================================================================================
// 3. RunRecord::replay_query() Tests
// ====================================================================================

#[tokio::test]
async fn replay_query_returns_historical_data() {
    let record = load_test_record();
    let db = open_test_db().await;

    let query = "?[name] := *struct{name @ 'NOW'}";

    let query_result = record
        .replay_query(1, &db, query)
        .expect("replay_query for structs should execute");
    assert!(
        !query_result.rows.is_empty(),
        "Query should return structs from the fixture database"
    );
    assert!(
        query_result.headers.iter().any(|h| h == "name"),
        "Query result should have 'name' column"
    );
    let has_fixture_struct = query_result.rows.iter().any(|row| {
        row.iter()
            .any(|val| val.get_str().map(|s| s == KNOWN_STRUCT).unwrap_or(false))
    });
    assert!(
        has_fixture_struct,
        "Expected '{KNOWN_STRUCT}' in struct query results"
    );
}

#[tokio::test]
async fn replay_query_functions_at_turn() {
    let record = load_test_record();
    let db = open_test_db().await;

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

    let query = "?[name] := *struct{name @ 'NOW'}";

    let result = record.replay_query(99, &db, query);

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
// 4. Iterator Method Tests
// ====================================================================================

#[test]
fn conversations_returns_turns() {
    let record = load_test_record();

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

    let aggregated: Vec<_> = record.tool_calls();

    let expected_count: usize = record
        .phases
        .agent_turns
        .iter()
        .map(|t| t.tool_calls().len())
        .sum();

    assert_eq!(
        expected_count, 1,
        "fixture should contain exactly one representative tool call"
    );
    assert_eq!(
        aggregated.len(),
        expected_count,
        "tool_calls() should return all tool calls from all turns"
    );

    let call = aggregated
        .first()
        .expect("fixture should include a representative tool call");
    assert_eq!(
        call.request.tool, KNOWN_TOOL,
        "tool_calls() should expose the fixture tool call name"
    );
    assert_eq!(
        call.latency_ms, 7,
        "tool_calls() should preserve fixture tool latency"
    );
}

// ====================================================================================
// 5. Additional Integration Tests
// ====================================================================================

#[test]
fn run_record_has_valid_metadata() {
    let record = load_test_record();

    assert_eq!(
        record.schema_version, "run-record.v1",
        "Schema version should be 'run-record.v1'"
    );

    assert_eq!(
        record.manifest_id, FIXTURE_MANIFEST_ID,
        "Manifest ID should match the hermetic fixture"
    );

    assert_eq!(
        record.metadata.benchmark.instance_id, FIXTURE_MANIFEST_ID,
        "Benchmark instance ID should match the hermetic fixture"
    );
}

#[test]
fn time_travel_index_matches_turns() {
    let record = load_test_record();

    assert!(
        !record.db_time_travel_index.is_empty(),
        "Time travel index should not be empty"
    );

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

    let query = "?[count(id)] := *struct{id @ 'NOW'}";
    let result = db_state.query(&db, query);
    let query_result = result.expect("db_state.query should execute successfully");
    assert!(!query_result.rows.is_empty(), "Query should return results");
}
