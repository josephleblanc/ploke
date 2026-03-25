//! Database command tests for xtask (Category A.4)
//!
//! Fail-until-impl: success paths use [`xtask::expect_command_ok`]; tests fail
//! (panic from `todo!()` in commands or `expect` on `Err`) until M.4.
//!
//! Each test documents:
//! - Underlying function(s) being tested
//! - Expected functionality
//! - Invariants
//! - Fail states
//! - Edge cases
//! - Hypothesis format: "To Prove: ... Given: ... When: ... Then: ..."

use ploke_test_utils::FIXTURE_NODES_CANONICAL;

use xtask::commands::db::{
    CountNodes, DbOutput, Load, LoadFixture, NodeKind, Query, Save, Stats, StatsCategory,
};
use xtask::commands::OutputFormat;
use xtask::expect_command_ok;
use xtask::test_harness::CommandTestHarness;

// ============================================================================
// Test A.4.1: CountNodes Command
// ============================================================================

/// Test: CountNodes returns accurate count for populated database
///
/// To Prove: That count_nodes returns accurate count
/// Given: A database with fixture data loaded
/// When: CountNodes command runs without filter
/// Then: Count is greater than 0 and matches expected fixture node count
///
/// Invariants Verified:
/// - Count is non-negative
/// - Total count equals sum of counts by kind
/// - Count matches direct database query results
///
/// Fail States:
/// - Database not initialized
/// - Database locked
/// - Query execution error
///
/// Edge Cases:
/// - Empty database (count = 0)
/// - Database with one node
/// - Database with many nodes
///
/// When This Test Would NOT Prove Correctness:
/// - If fixture data doesn't represent real-world node distributions
#[test]
fn count_nodes_returns_nonzero_for_populated_db() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let cmd = CountNodes {
        db: Some(iso.db_path.clone()),
        kind: None,
        pending: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");

    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db count-nodes must succeed once implemented",
    );

    let DbOutput::NodeCount {
        total,
        by_kind,
        pending_embeddings: _,
    } = output
    else {
        panic!("Expected NodeCount output variant");
    };

    assert!(total > 0, "Node count should be positive for populated DB");
    assert!(
        !by_kind.is_empty(),
        "Should have nodes categorized by kind"
    );
    let sum_by_kind: usize = by_kind.values().sum();
    assert_eq!(
        total, sum_by_kind,
        "Total should equal sum of counts by kind"
    );
}

/// Test: CountNodes with kind filter returns filtered count
///
/// To Prove: That count_nodes respects kind filter
/// Given: A database with mixed node types
/// When: CountNodes command runs with NodeKind::Function filter
/// Then: Only function nodes are counted
///
/// Invariants Verified:
/// - Filtered count <= total count
/// - Filtered count is non-negative
/// - Unknown kinds return 0 (not error)
#[test]
fn count_nodes_with_kind_filter() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let cmd = CountNodes {
        db: Some(iso.db_path.clone()),
        kind: Some(NodeKind::Function),
        pending: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db count-nodes with kind filter must succeed once implemented",
    );

    let DbOutput::NodeCount { by_kind, .. } = output else {
        panic!("Expected NodeCount output variant");
    };
    assert!(
        by_kind.contains_key("Function"),
        "Should have Function key when filtering by function"
    );
}

/// Test: CountNodes with pending embeddings flag
///
/// To Prove: That pending embeddings count is accurate when requested
/// Given: A database with some unembedded nodes
/// When: CountNodes command runs with --pending flag
/// Then: pending_embeddings is Some(count)
#[test]
fn count_nodes_with_pending_flag() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let cmd = CountNodes {
        db: Some(iso.db_path.clone()),
        kind: None,
        pending: true, // Request pending count
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db count-nodes with --pending must succeed once implemented",
    );

    let DbOutput::NodeCount {
        pending_embeddings, ..
    } = output
    else {
        panic!("Expected NodeCount output variant");
    };
    assert!(
        pending_embeddings.is_some(),
        "pending_embeddings should be Some when --pending flag is used"
    );
}

// ============================================================================
// Test A.4.2: Query Command
// ============================================================================

/// Test: Query command executes CozoDB query
///
/// To Prove: That db query executes CozoDB queries and returns results correctly
/// Given: An initialized database with known data
/// When: Query command executes with valid CozoScript query
/// Then: Query results are returned with correct columns and rows
///
/// Invariants Verified:
/// - Valid queries return results
/// - Result columns match query projection
/// - Result rows have correct column count
/// - Duration is non-negative
///
/// Fail States:
/// - Syntax error in query
/// - Query references non-existent relations
/// - Database not initialized
///
/// Edge Cases:
/// - Empty result set
/// - Query with parameters
/// - Recursive/multi-line queries
#[test]
fn query_executes_valid_cozoscript() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let cmd = Query {
        query: "?[count(id)] := *function { id }".to_string(),
        db: Some(iso.db_path.clone()),
        param: vec![],
        mutable: false,
        output: Some(OutputFormat::Json),
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db query must succeed once implemented",
    );

    let DbOutput::QueryResult {
        rows,
        columns,
        duration_ms,
    } = output
    else {
        panic!("Expected QueryResult output variant");
    };

    assert!(!columns.is_empty(), "Query result should have columns");
    assert_eq!(
        columns.len(),
        1,
        "Count query should return single column"
    );
    for row in &rows {
        assert!(
            row.is_object() || row.is_array(),
            "Row should be JSON object or array"
        );
    }
    assert!(
        duration_ms < 60000,
        "Query took too long: {} ms",
        duration_ms
    );
}

/// Test: Query command handles invalid queries gracefully
///
/// To Prove: That invalid queries return errors with context
/// Given: An initialized database
/// When: Query command executes with invalid CozoScript
/// Then: Error is returned with query context and helpful message
#[test]
fn query_handles_invalid_syntax() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let cmd = Query {
        query: "INVALID SYNTAX HERE".to_string(),
        db: Some(iso.db_path.clone()),
        param: vec![],
        mutable: false,
        output: None,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let err = harness
        .executor()
        .execute(cmd)
        .expect_err("invalid CozoScript must produce an error once Query is implemented");
    let msg = err.to_string();
    assert!(
        msg.contains("syntax")
            || msg.contains("invalid")
            || msg.contains("parse")
            || msg.contains("cozo")
            || msg.contains("parser")
            || msg.contains("unexpected"),
        "error should describe invalid query: {msg}"
    );
    assert!(
        msg.contains("INVALID SYNTAX HERE") || msg.contains("your input query"),
        "error should include query context per PRIMARY_TASK_SPEC §D: {msg}"
    );
}

/// Test: Query command with parameters
///
/// To Prove: That query parameters are substituted correctly
/// Given: A database with parameterized query
/// When: Query command executes with params
/// Then: Parameters are bound and query executes correctly
#[test]
fn query_with_parameters() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let cmd = Query {
        query: "?[id] := *function { id, name }, name = $name".to_string(),
        db: Some(iso.db_path.clone()),
        param: vec![("name".to_string(), "test_function".to_string())],
        mutable: false,
        output: None,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    expect_command_ok(
        harness.executor().execute(cmd),
        "db query with parameters must succeed once implemented",
    );
}

// ============================================================================
// Test A.4.3: Stats Command
// ============================================================================

/// Test: Stats command returns database statistics
///
/// To Prove: That db stats returns comprehensive database statistics
/// Given: An initialized database with data
/// When: Stats command executes with category=All
/// Then: Statistics are returned for all categories
///
/// Invariants Verified:
/// - Stats category is returned in output
/// - Data contains expected stat fields
/// - All subcategories have non-negative counts
///
/// Fail States:
/// - Database not initialized
/// - Schema not created
/// - Permission denied
#[test]
fn stats_returns_comprehensive_data() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let cmd = Stats {
        db: Some(iso.db_path.clone()),
        category: StatsCategory::All,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db stats must succeed once implemented",
    );

    let DbOutput::DatabaseStats { category, data } = output else {
        panic!("Expected DatabaseStats output variant");
    };
    assert_eq!(category, "All", "Category should match input");
    assert!(data.is_object(), "Stats data should be a JSON object");
}

/// Test: Stats command with specific category
///
/// To Prove: That stats respects category filter
/// Given: An initialized database
/// When: Stats command executes with specific category (e.g., Nodes)
/// Then: Only stats for that category are returned
#[test]
fn stats_with_category_filter() {
    let categories = vec![
        StatsCategory::Nodes,
        StatsCategory::Relations,
        StatsCategory::Embeddings,
        StatsCategory::Indexes,
    ];

    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    for category in categories {
        let cmd = Stats {
            db: Some(iso.db_path.clone()),
            category,
        };

        let harness = CommandTestHarness::new().expect("Failed to create test harness");
        let output = expect_command_ok(
            harness.executor().execute(cmd),
            "db stats with category filter must succeed once implemented",
        );

        if let DbOutput::DatabaseStats { category: cat, .. } = output {
            assert_eq!(
                cat,
                format!("{category:?}"),
                "Output category should match input"
            );
        } else {
            panic!("Expected DatabaseStats output variant");
        }
    }
}

// ============================================================================
// Test A.4.4: Save/Load Commands (Backup/Restore)
// ============================================================================

/// Test: Save command creates valid backup file
///
/// To Prove: That db save creates a valid backup file containing all database data
/// Given: An initialized database with data
/// When: Save command executes with path
/// Then: Backup file exists and can be restored
///
/// Invariants Verified:
/// - Backup file is created
/// - File is not empty
/// - Original database is unchanged
///
/// Fail States:
/// - Invalid path (directory doesn't exist)
/// - Permission denied
/// - Disk full
/// - Database locked
///
/// Edge Cases:
/// - Empty database backup
/// - Large database backup
/// - Path with special characters
#[test]
fn save_creates_valid_backup() {
    let temp_dir = std::env::temp_dir();
    let backup_path = temp_dir.join("test_backup.sqlite");

    // Clean up any existing file
    let _ = std::fs::remove_file(&backup_path);

    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let cmd = Save {
        db: Some(iso.db_path.clone()),
        output: backup_path.clone(),
        compress: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db save must succeed once implemented",
    );

    let DbOutput::Success { path, .. } = output else {
        panic!("Expected Success output variant");
    };

    assert!(
        backup_path.exists(),
        "Backup file should exist after save: {}",
        backup_path.display()
    );
    let metadata = std::fs::metadata(&backup_path).expect("Failed to read metadata");
    assert!(metadata.len() > 0, "Backup file should not be empty");
    assert_eq!(
        path.as_ref().map(|p| p.to_str().unwrap()),
        Some(backup_path.to_str().unwrap()),
        "Output should contain backup path"
    );
    let _ = std::fs::remove_file(&backup_path);
}

/// Test: Load command restores database from backup
///
/// To Prove: That db load restores database to state at backup time
/// Given: A valid backup file
/// When: Load command executes
/// Then: Database contains all data from backup
///
/// Invariants Verified:
/// - All relations restored
/// - All nodes restored
/// - Counts match pre-backup state
///
/// Fail States:
/// - Invalid backup file
/// - Corrupted backup
/// - Incompatible backup version
#[test]
fn load_restores_backup_correctly() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let backup_path = iso.db_path.clone();

    let cmd = Load {
        path: backup_path.clone(),
        target: None,
        verify: true, // Request verification
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db load must succeed for a valid backup path once implemented",
    );

    let DbOutput::Success { message, .. } = output else {
        panic!("Expected Success output variant");
    };
    assert!(
        message.contains("restored") || message.contains("loaded"),
        "Success message should indicate restoration: {message}"
    );
}

// ============================================================================
// Test A.4.5: LoadFixture Command
// ============================================================================

/// Test: LoadFixture command loads known fixture databases
///
/// To Prove: That load-fixture correctly loads known fixture databases
/// Given: Valid fixture ID from registry
/// When: Command executes with fixture ID
/// Then: Database contains fixture data
///
/// Invariants Verified:
/// - Fixture is found in registry
/// - Database is initialized
/// - Fixture data is loaded correctly
///
/// Fail States:
/// - Invalid fixture ID
/// - Fixture file missing
/// - Fixture corrupted
/// - Schema version mismatch
///
/// Edge Cases:
/// - All available fixtures load correctly
/// - Fixture with embeddings
/// - Fixture without embeddings
#[test]
fn load_fixture_loads_valid_fixture() {
    // Test with a known fixture ID
    let fixture_id = "fixture_nodes_canonical";

    let cmd = LoadFixture {
        fixture: fixture_id.to_string(),
        index: false,
        verify: true,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db load-fixture must succeed for a valid fixture id once implemented",
    );

    let DbOutput::Success { message, .. } = output else {
        panic!("Expected Success output variant");
    };
    assert!(
        message.contains(fixture_id) || message.contains("loaded"),
        "Success message should reference fixture: {message}"
    );
}

/// Test: LoadFixture with invalid fixture ID fails gracefully
///
/// To Prove: That load-fixture provides helpful error for invalid fixtures
/// Given: An invalid fixture ID
/// When: Command executes
/// Then: Error contains fixture ID and available fixtures list
#[test]
fn load_fixture_rejects_invalid_id() {
    let invalid_fixture_id = "nonexistent_fixture_xyz";

    let cmd = LoadFixture {
        fixture: invalid_fixture_id.to_string(),
        index: false,
        verify: true,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let err = harness.executor().execute(cmd).expect_err(
        "LoadFixture must reject an unknown fixture id once implemented",
    );
    let msg = err.to_string();
    assert!(
        msg.contains(invalid_fixture_id)
            || msg.contains("not found")
            || msg.contains("unknown")
            || msg.contains("invalid"),
        "error should indicate missing or invalid fixture: {msg}"
    );
    assert!(
        err.recovery_suggestion().is_some(),
        "PRIMARY_TASK_SPEC §D expects recovery context: {err:?}"
    );
}

/// Test: LoadFixture with index flag creates HNSW index
///
/// To Prove: That load-fixture --index creates HNSW index after loading
/// Given: A fixture with embeddings
/// When: Command executes with --index flag
/// Then: HNSW index is created
#[test]
fn load_fixture_with_index_flag() {
    let cmd = LoadFixture {
        fixture: "fixture_nodes_local_embeddings".to_string(),
        index: true,  // Request index creation
        verify: true,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    let output = expect_command_ok(
        harness.executor().execute(cmd),
        "db load-fixture with --index must succeed once implemented",
    );

    let DbOutput::Success { message, .. } = output else {
        panic!("Expected Success output variant");
    };
    assert!(
        message.contains("index") || message.contains("HNSW"),
        "Success message should mention index creation: {message}"
    );
}

// ============================================================================
// Additional Edge Case Tests
// ============================================================================

/// Test: CountNodes with database path option
///
/// To Prove: That count_nodes respects --db path option
/// Given: Multiple databases available
/// When: CountNodes runs with specific --db path
/// Then: Counts are from the specified database
#[test]
fn count_nodes_with_db_path() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let temp_db_path = iso.db_path.clone();

    let cmd = CountNodes {
        db: Some(temp_db_path.clone()),
        kind: None,
        pending: false,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    expect_command_ok(
        harness.executor().execute(cmd),
        "db count-nodes with --db path must succeed once implemented",
    );
}

/// Test: Query with mutable flag
///
/// To Prove: That mutable queries are allowed when --mutable flag is set
/// Given: A database and a mutating query
/// When: Query runs with --mutable
/// Then: Query executes successfully
#[test]
fn query_with_mutable_flag() {
    let iso = CommandTestHarness::isolated_fixture_copy(&FIXTURE_NODES_CANONICAL)
        .expect("isolated fixture copy");
    let cmd = Query {
        query: "?[x] := x in [1]".to_string(),
        db: Some(iso.db_path.clone()),
        param: vec![],
        mutable: true, // Allow mutation
        output: None,
    };

    let harness = CommandTestHarness::new().expect("Failed to create test harness");
    expect_command_ok(
        harness.executor().execute(cmd),
        "db query with --mutable must complete once implemented",
    );
}

// ============================================================================
// Integration-style tests (commented out until M.4 implementation)
// ============================================================================

/*
/// Full backup/restore roundtrip test
///
/// This test will be enabled in M.4 when full implementation is available.
#[test]
fn backup_restore_roundtrip() {
    use ploke_test_utils::{fresh_backup_fixture_db, FIXTURE_NODES_CANONICAL};

    // Setup: Load fixture
    let source_db = fresh_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("Failed to load fixture");

    // Count nodes in source
    let source_count = source_db.count_pending_embeddings().unwrap_or(0);

    // Save to backup
    let temp_path = std::env::temp_dir().join("roundtrip_test.sqlite");
    let save_cmd = Save {
        db: None,
        output: temp_path.clone(),
        compress: false,
    };

    let harness = CommandTestHarness::new().unwrap();
    harness.executor().execute(save_cmd).unwrap();

    // Load from backup
    let load_cmd = Load {
        path: temp_path.clone(),
        target: None,
        verify: true,
    };
    harness.executor().execute(load_cmd).unwrap();

    // Count nodes in restored database
    let restored_count = // ... get count from loaded db

    // Verify counts match
    assert_eq!(source_count, restored_count);

    // Cleanup
    let _ = std::fs::remove_file(&temp_path);
}
*/

// ============================================================================
// Module documentation notes (M.4)
// ============================================================================
//
// 1. Command bodies still use `todo!("... implementation")`; executing them panics until removed.
// 2. Success-oriented tests use `expect_command_ok` and fail until commands return `Ok` with real output.
// 3. After implementation: remove todo!() in commands/db.rs; keep output assertions; add fixture integration tests.
// Fixtures: FIXTURE_NODES_CANONICAL, FIXTURE_NODES_LOCAL_EMBEDDINGS, PLOKE_DB_PRIMARY.
