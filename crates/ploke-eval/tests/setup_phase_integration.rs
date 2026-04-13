//! Integration test for SetupPhase and introspection API
//!
//! This test verifies:
//! 1. SetupPhase is correctly populated after indexing
//! 2. IndexedCrateSummary has accurate node counts
//! 3. DbState::lookup() finds expected nodes (queries function relation)
//! 4. replay_query() returns correct historical state

use std::path::PathBuf;
use std::sync::Arc;

use ploke_db::Database;
use ploke_db::multi_embedding::db_ext::EmbeddingExt;
use ploke_eval::record::{CrateIndexStatus, IndexedCrateSummary, SetupPhase, TurnRecord};
use ploke_eval::runner::{IndexingStatusArtifact, RepoStateArtifact};
use ploke_test_utils::workspace_root;
use ploke_tui::parser::{IndexTargetResolveError, resolve_index_target, run_parse_resolved};
use uuid::Uuid;

/// Custom error type to handle both IndexTargetResolveError and other errors
#[derive(Debug)]
enum TestError {
    Resolve(IndexTargetResolveError),
    Other(Box<dyn std::error::Error>),
}

impl std::fmt::Display for TestError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TestError::Resolve(e) => write!(f, "{}", e),
            TestError::Other(e) => write!(f, "{}", e),
        }
    }
}

impl std::error::Error for TestError {}

impl From<IndexTargetResolveError> for TestError {
    fn from(e: IndexTargetResolveError) -> Self {
        TestError::Resolve(e)
    }
}

impl From<ploke_db::DbError> for TestError {
    fn from(e: ploke_db::DbError) -> Self {
        TestError::Other(Box::new(e))
    }
}

impl From<syn_parser::error::SynParserError> for TestError {
    fn from(e: syn_parser::error::SynParserError) -> Self {
        TestError::Other(Box::new(e))
    }
}

impl From<Arc<dyn std::error::Error + Send + Sync>> for TestError {
    fn from(e: Arc<dyn std::error::Error + Send + Sync>) -> Self {
        TestError::Other(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        )))
    }
}

impl From<std::io::Error> for TestError {
    fn from(e: std::io::Error) -> Self {
        TestError::Other(Box::new(e))
    }
}

impl From<ploke_error::Error> for TestError {
    fn from(e: ploke_error::Error) -> Self {
        TestError::Other(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            e.to_string(),
        )))
    }
}

/// Helper function to build SetupPhase from database after indexing
async fn build_setup_phase(
    db: &Database,
    repo_state: RepoStateArtifact,
    indexing_status: IndexingStatusArtifact,
    using_cached_db: bool,
) -> Result<SetupPhase, TestError> {
    // Get setup start/end times
    let setup_start_time = chrono::Utc::now();

    // 1. Get crate list from DB
    let crate_rows = db.list_crate_context_rows()?;

    // 2. For each crate, build IndexedCrateSummary
    let mut indexed_crates: Vec<IndexedCrateSummary> = Vec::new();
    for row in crate_rows {
        let node_count = db.count_nodes_for_namespace(row.namespace)?;

        // For embedded_count, we need to handle the case where embedding relation doesn't exist
        let embedded_count = db.count_embedded_for_namespace(row.namespace).unwrap_or(0);

        // Determine status based on whether we used cached DB
        let status = if using_cached_db {
            CrateIndexStatus::Skipped
        } else {
            CrateIndexStatus::Success
        };

        indexed_crates.push(IndexedCrateSummary {
            name: row.name,
            version: String::new(), // Version not stored in DB currently
            namespace: row.namespace,
            root_path: PathBuf::from(&row.root_path),
            file_count: 0, // Can be queried from DB if needed
            node_count,
            embedded_count,
            status,
            parse_error: None,
        });
    }

    // 3. Get DB timestamp
    let db_timestamp_micros = db.current_validity_micros()?;

    Ok(SetupPhase {
        started_at: setup_start_time.to_rfc3339(),
        ended_at: chrono::Utc::now().to_rfc3339(),
        repo_state,
        indexing_status,
        indexed_crates,
        parse_failures: vec![],
        db_timestamp_micros,
        tool_schema_version: None,
    })
}

/// Indexes a fixture and returns the database with populated data
async fn index_fixture(fixture_path: PathBuf) -> Result<(Database, i64), TestError> {
    // Initialize database
    let db = Database::init_with_schema()?;

    // Setup multi-embedding schema (needed for embedded count queries)
    let _ = db.setup_multi_embedding();

    // Resolve and run indexing
    let pwd = std::env::current_dir()?;
    let resolved = resolve_index_target(Some(fixture_path), &pwd)?;

    // Run parsing (blocking operation in spawn_blocking)
    let db_arc = Arc::new(db);
    let db_clone = Arc::clone(&db_arc);
    let resolved_clone = resolved.clone();

    tokio::task::spawn_blocking(move || run_parse_resolved(db_clone, &resolved_clone))
        .await
        .map_err(|e| {
            TestError::Other(Box::new(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Task join error: {:?}", e),
            )))
        })?
        .map_err(|e| TestError::Other(Box::new(e)))?;

    // Get timestamp after indexing
    let timestamp_micros = db_arc.current_validity_micros()?;

    // Unwrap Arc to get the Database back
    let db = Arc::try_unwrap(db_arc).map_err(|_| {
        TestError::Other(Box::new(std::io::Error::new(
            std::io::ErrorKind::Other,
            "Failed to unwrap Arc",
        )))
    })?;

    Ok((db, timestamp_micros))
}

#[tokio::test]
async fn setup_phase_populates_correctly_after_indexing() {
    // 1. Setup fixture path
    let fixture_path = workspace_root().join("fixture_test_crate");

    // 2. Run indexing
    let (db, _timestamp) = index_fixture(fixture_path.clone())
        .await
        .expect("Failed to index fixture");

    // 3. Build SetupPhase
    let repo_state = RepoStateArtifact {
        repo_root: fixture_path.clone(),
        requested_base_sha: None,
        checked_out_head_sha: None,
        git_status_porcelain: String::new(),
    };

    let indexing_status = IndexingStatusArtifact {
        status: "completed".to_string(),
        detail: "Successfully indexed test crate".to_string(),
    };

    let setup = build_setup_phase(&db, repo_state, indexing_status, false)
        .await
        .expect("Failed to build setup phase");

    // 4. Verify indexed_crates is not empty
    assert!(
        !setup.indexed_crates.is_empty(),
        "indexed_crates should not be empty after indexing"
    );

    // 5. Verify each crate has valid data
    for crate_summary in &setup.indexed_crates {
        // name is not empty
        assert!(
            !crate_summary.name.is_empty(),
            "Crate name should not be empty"
        );

        // namespace is a valid UUID (non-nil)
        assert!(
            crate_summary.namespace != Uuid::nil(),
            "Crate namespace should be a valid UUID, got nil"
        );

        // node_count > 0 (fixture has nodes)
        assert!(
            crate_summary.node_count > 0,
            "Crate {} should have nodes, found {}",
            crate_summary.name,
            crate_summary.node_count
        );

        // status is Success for fresh indexing
        assert!(
            matches!(crate_summary.status, CrateIndexStatus::Success),
            "Crate {} should have Success status for fresh indexing",
            crate_summary.name
        );

        // root_path exists
        assert!(
            crate_summary.root_path.exists(),
            "Crate root path should exist: {:?}",
            crate_summary.root_path
        );
    }

    // 6. Verify total node count is reasonable for the fixture
    let total_nodes: usize = setup.indexed_crates.iter().map(|c| c.node_count).sum();
    assert!(
        total_nodes > 50,
        "Expected at least 50 total nodes in fixture_test_crate, found {}",
        total_nodes
    );

    // 7. Verify db_timestamp_micros is valid (positive)
    assert!(
        setup.db_timestamp_micros > 0,
        "DB timestamp should be positive"
    );
}

#[tokio::test]
async fn introspection_api_queries_historical_state() {
    // 1. Setup and index fixture
    let fixture_path = workspace_root().join("fixture_test_crate");
    let (db, timestamp_micros) = index_fixture(fixture_path)
        .await
        .expect("Failed to index fixture");

    // 2. Create a TurnRecord with the known timestamp
    let turn = TurnRecord {
        turn_number: 1,
        started_at: chrono::Utc::now().to_rfc3339(),
        ended_at: chrono::Utc::now().to_rfc3339(),
        db_timestamp_micros: timestamp_micros,
        issue_prompt: "Test introspection".to_string(),
        llm_request: None,
        llm_response: None,
        tool_calls: vec![],
        outcome: ploke_eval::record::TurnOutcome::Content,
        agent_turn_artifact: None,
    };

    // 3. Use db_state() to get the timestamp
    let db_state = turn.db_state();
    assert_eq!(
        db_state.timestamp_micros(),
        timestamp_micros,
        "DbState should have the correct timestamp"
    );

    // 4. Look up a function that should exist in the fixture (main function)
    let node_info = db_state
        .lookup(&db, "main")
        .expect("db_state.lookup('main') should execute")
        .expect("Should find 'main' function in the database");
    assert_eq!(node_info.name, "main", "Found node should have name 'main'");
    assert_eq!(
        node_info.node_type, "function",
        "Node should have type 'function'"
    );
    assert!(node_info.id != Uuid::nil(), "Node should have a valid UUID");

    // 5. Test lookup for non-existent function
    let not_found = db_state
        .lookup(&db, "this_function_does_not_exist")
        .expect("db_state.lookup() should not error for non-existent node");
    assert!(
        not_found.is_none(),
        "Should return None for non-existent node"
    );

    // 6. Use db_state().query() to execute arbitrary query on function relation
    let query_result = db_state.query(&db, "?[name] := *function{name @ 'NOW'} :limit 10");
    assert!(
        query_result.is_ok(),
        "query() should execute successfully: {:?}",
        query_result.err()
    );

    let query_result = query_result.unwrap();
    assert!(
        !query_result.rows.is_empty(),
        "Query should return some functions"
    );

    // 7. Verify query results have expected structure
    assert!(
        query_result.headers.iter().any(|h| h == "name"),
        "Query result should have 'name' column"
    );
}

#[tokio::test]
async fn replay_query_works_with_run_record() {
    use ploke_eval::record::{
        AgentMetadata, BenchmarkMetadata, RunMetadata, RunRecord, RuntimeMetadata,
    };
    use ploke_eval::runner::RunArm;
    use ploke_eval::spec::EvalBudget;

    // 1. Setup and index fixture
    let fixture_path = workspace_root().join("fixture_test_crate");
    let (db, timestamp_micros) = index_fixture(fixture_path.clone())
        .await
        .expect("Failed to index fixture");

    // 2. Create a minimal RunRecord with time travel marker
    let mut record = RunRecord {
        schema_version: "run-record.v1".to_string(),
        manifest_id: "test-instance".to_string(),
        metadata: RunMetadata {
            run_arm: RunArm::shell_only_control(),
            benchmark: BenchmarkMetadata {
                instance_id: "test-instance".to_string(),
                repo_root: fixture_path,
                base_sha: None,
                issue: None,
            },
            agent: AgentMetadata::default(),
            runtime: RuntimeMetadata::default(),
            budget: EvalBudget::default(),
        },
        phases: ploke_eval::record::RunPhases::default(),
        db_time_travel_index: vec![ploke_eval::record::TimeTravelMarker {
            turn: 1,
            timestamp_micros,
            event: "turn_complete".to_string(),
        }],
        timing: None,
        conversation: vec![],
    };

    // Add a turn record
    record.phases.agent_turns.push(TurnRecord {
        turn_number: 1,
        started_at: chrono::Utc::now().to_rfc3339(),
        ended_at: chrono::Utc::now().to_rfc3339(),
        db_timestamp_micros: timestamp_micros,
        issue_prompt: "Test replay query".to_string(),
        llm_request: None,
        llm_response: None,
        tool_calls: vec![],
        outcome: ploke_eval::record::TurnOutcome::Content,
        agent_turn_artifact: None,
    });

    // 3. Use replay_query to query at turn 1 (query function relation)
    let result = record.replay_query(1, &db, "?[count(id)] := *function{id @ 'NOW'}");
    assert!(
        result.is_ok(),
        "replay_query should succeed: {:?}",
        result.err()
    );

    let query_result = result.unwrap();
    assert!(
        !query_result.rows.is_empty(),
        "replay_query should return results"
    );

    // 4. Verify the count is reasonable (should be > 0 for indexed fixture)
    let count_value = &query_result.rows[0][0];
    if let Some(count) = count_value.get_int() {
        assert!(
            count > 0,
            "Function count should be positive, got {}",
            count
        );
    }

    // 5. Test replay_query for non-existent turn
    let not_found = record.replay_query(99, &db, "?[name] := *function{name @ 'NOW'}");
    assert!(
        not_found.is_err(),
        "replay_query should error for non-existent turn"
    );

    let err = not_found.unwrap_err();
    assert!(
        err.to_string().contains("not found") || err.to_string().contains("Timestamp"),
        "Error should indicate turn not found: {}",
        err
    );
}

#[tokio::test]
async fn setup_phase_with_cached_db_path() {
    // Test that SetupPhase correctly reports Skipped status for cached DB
    let fixture_path = workspace_root().join("fixture_test_crate");

    // First indexing (fresh)
    let (db, _) = index_fixture(fixture_path.clone())
        .await
        .expect("Failed to index fixture first time");

    // Build SetupPhase as if using cached DB
    let repo_state = RepoStateArtifact {
        repo_root: fixture_path,
        requested_base_sha: None,
        checked_out_head_sha: None,
        git_status_porcelain: String::new(),
    };

    let indexing_status = IndexingStatusArtifact {
        status: "cached".to_string(),
        detail: "Using cached database".to_string(),
    };

    let setup = build_setup_phase(&db, repo_state, indexing_status, true)
        .await
        .expect("Failed to build setup phase for cached DB");

    // Verify all crates have Skipped status
    for crate_summary in &setup.indexed_crates {
        assert!(
            matches!(crate_summary.status, CrateIndexStatus::Skipped),
            "Crate {} should have Skipped status for cached DB",
            crate_summary.name
        );
    }
}

#[tokio::test]
async fn node_counts_are_accurate_for_fixture() {
    // Test that node counts match actual fixture data
    let fixture_path = workspace_root().join("fixture_test_crate");
    let (db, _) = index_fixture(fixture_path)
        .await
        .expect("Failed to index fixture");

    // Get crate contexts
    let crate_rows = db
        .list_crate_context_rows()
        .expect("Should list crate contexts");

    assert!(
        !crate_rows.is_empty(),
        "Should have at least one crate context"
    );

    // For each crate, verify node count is consistent
    for row in crate_rows {
        let node_count = db
            .count_nodes_for_namespace(row.namespace)
            .expect("Should count nodes");

        // The fixture_test_crate has multiple source files, so should have nodes
        assert!(node_count > 0, "Crate {} should have nodes", row.name);

        // Verify we can query for those nodes using the same logic as count_nodes_for_namespace
        let query = format!(
            r#"root[id] := *file_mod{{ owner_id: id, namespace @ 'NOW' }}, namespace = to_uuid("{}")
desc[id] := root[id]
parent_of[child, parent] := *syntax_edge{{ source_id: parent, target_id: child, relation_kind: "Contains" @ 'NOW' }}
desc[id] := parent_of[id, parent], desc[parent]
?[count(id)] := desc[id]"#,
            row.namespace
        );

        let result = db.raw_query(&query).expect("Should execute count query");

        if let Some(row_data) = result.rows.first() {
            if let Some(queried_count) = row_data.first().and_then(|v| v.get_int()) {
                assert_eq!(
                    queried_count as usize, node_count,
                    "Node count should match between count_nodes_for_namespace and direct query"
                );
            }
        }
    }
}

#[tokio::test]
async fn setup_phase_lists_multiple_crates() {
    // Test that SetupPhase correctly lists all crates in a workspace fixture
    let fixture_path = workspace_root().join("tests/fixture_workspace/ws_fixture_01");

    // Skip if fixture doesn't exist
    if !fixture_path.exists() {
        eprintln!(
            "Skipping test: fixture_workspace not found at {:?}",
            fixture_path
        );
        return;
    }

    let (db, _) = index_fixture(fixture_path.clone())
        .await
        .expect("Failed to index workspace fixture");

    let repo_state = RepoStateArtifact {
        repo_root: fixture_path,
        requested_base_sha: None,
        checked_out_head_sha: None,
        git_status_porcelain: String::new(),
    };

    let indexing_status = IndexingStatusArtifact {
        status: "completed".to_string(),
        detail: "Successfully indexed workspace".to_string(),
    };

    let setup = build_setup_phase(&db, repo_state, indexing_status, false)
        .await
        .expect("Failed to build setup phase");

    // Should have at least 2 crates (member_root and member_nested)
    assert!(
        setup.indexed_crates.len() >= 2,
        "Workspace fixture should have at least 2 crates, found {}",
        setup.indexed_crates.len()
    );

    // Each crate should have nodes
    for crate_summary in &setup.indexed_crates {
        assert!(
            crate_summary.node_count > 0,
            "Crate {} should have nodes",
            crate_summary.name
        );
    }
}
