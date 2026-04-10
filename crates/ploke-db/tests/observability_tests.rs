#![allow(unused_variables)]

use ploke_core::ArcStr;
use ploke_db::{Database, ObservabilityStore, ToolCallDone, ToolCallReq, ToolStatus, Validity};
use ploke_test_utils::fixture_dbs::{FIXTURE_NODES_CANONICAL, shared_backup_fixture_db};

#[test]
fn current_validity_micros_returns_monotonic_timestamp() {
    // Initialize a fresh database
    let db = Database::init_with_schema().expect("init db");

    // Get initial timestamp
    let ts1 = db.current_validity_micros().expect("get first timestamp");

    // Timestamp should be positive (microseconds since epoch)
    assert!(
        ts1 > 0,
        "expected positive timestamp, got {}",
        ts1
    );

    // Get second timestamp - should be >= first (monotonic)
    let ts2 = db.current_validity_micros().expect("get second timestamp");
    assert!(
        ts2 >= ts1,
        "expected monotonic timestamps, got ts1={}, ts2={}",
        ts1,
        ts2
    );

    // Verify timestamps are reasonable (within last hour, not in future)
    // Cozo validity timestamps are in microseconds since epoch
    let now_micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64;
    
    let one_hour_micros = 60 * 60 * 1_000_000i64;
    assert!(
        ts1 > now_micros - one_hour_micros,
        "timestamp should be within last hour: ts1={}, now={}",
        ts1,
        now_micros
    );
    assert!(
        ts1 <= now_micros,
        "timestamp should not be in future: ts1={}, now={}",
        ts1,
        now_micros
    );
}

/// Test current_validity_micros against a fixture database with real data.
/// 
/// This validates that the timestamp helper works correctly on a database
/// that contains actual fixture data (fixture_nodes), not just an empty schema.
/// 
/// Uses shared_backup_fixture_db for fast, read-only access to cached fixture.
#[test]
fn current_validity_micros_works_with_fixture_database() {
    // Load the canonical fixture database (cached, read-only)
    let db = shared_backup_fixture_db(&FIXTURE_NODES_CANONICAL)
        .expect("should load fixture_nodes_canonical backup");

    // Get timestamp from fixture database
    let ts1 = db.current_validity_micros().expect("get timestamp from fixture db");

    // Timestamp should be positive
    assert!(ts1 > 0, "expected positive timestamp from fixture db, got {}", ts1);

    // Get second timestamp - should be >= first (monotonic)
    let ts2 = db.current_validity_micros().expect("get second timestamp from fixture db");
    assert!(
        ts2 >= ts1,
        "expected monotonic timestamps on fixture db, got ts1={}, ts2={}",
        ts1,
        ts2
    );

    // Verify timestamps are reasonable (not ancient, not future)
    let now_micros = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64;
    
    // Allow 24 hours slack since fixture might be older
    let day_micros = 24 * 60 * 60 * 1_000_000i64;
    assert!(
        ts1 > now_micros - day_micros,
        "fixture db timestamp should be recent: ts1={}, now={}",
        ts1,
        now_micros
    );
    assert!(
        ts1 <= now_micros,
        "fixture db timestamp should not be in future: ts1={}, now={}",
        ts1,
        now_micros
    );
}

#[test]
#[ignore = "temporary ignore: test uses todo! placeholders; focusing on feature work"]
fn tool_call_requested_idempotent() {
    let db = Database::init_with_schema().expect("init db");
    let request_id = uuid::Uuid::new_v4();
    let parent_id = uuid::Uuid::new_v4();

    let req = ToolCallReq {
        request_id,
        call_id: ArcStr::from("call-1"),
        parent_id,

        tool_name: ArcStr::from("test_tool"),
        args_sha256: "abc123".to_string(),
        arguments_json: Some(r#"{"arg":1}"#.to_string()),
        started_at: Validity {
            at: 0,
            is_valid: true,
        },
        model: todo!(),
        provider_slug: todo!(),
    };

    // First insert
    db.record_tool_call_requested(req.clone())
        .expect("first requested");

    // Second insert should be no-op
    db.record_tool_call_requested(req.clone())
        .expect("second requested no-op");

    // Snapshot at NOW should return a single latest row (no duplication in snapshot)
    let rows = db
        .list_tool_calls_by_parent(parent_id, 10)
        .expect("list by parent");
    assert_eq!(rows.len(), 1, "expected a single snapshot row");
    let (stored_req, stored_done) = &rows[0];
    assert_eq!(stored_req.request_id, request_id);
    assert!(stored_done.is_none(), "should not have terminal status yet");
}

#[test]
#[ignore = "temporary ignore: test uses todo! placeholders; focusing on feature work"]
fn tool_call_done_idempotent_and_transition_rules() {
    let db = Database::init_with_schema().expect("init db");
    let request_id = uuid::Uuid::new_v4();
    let parent_id = uuid::Uuid::new_v4();
    let call_id = ArcStr::from("call-2");

    let req = ToolCallReq {
        request_id,
        call_id: call_id.clone(),
        parent_id,

        tool_name: ArcStr::from("test_tool"),
        args_sha256: "abc123".to_string(),
        arguments_json: Some(r#"{"arg":1}"#.to_string()),
        started_at: Validity {
            at: 0,
            is_valid: true,
        },
        model: todo!(),
        provider_slug: todo!(),
    };
    db.record_tool_call_requested(req).expect("requested");

    let done = ToolCallDone {
        request_id,
        call_id: call_id.clone(),
        ended_at: Validity {
            at: 1234,
            is_valid: true,
        },
        latency_ms: 25,
        outcome_json: Some(r#"{"ok":true}"#.to_string()),
        error_kind: None,
        error_msg: None,
        status: ToolStatus::Completed,
    };

    // First completion
    db.record_tool_call_done(done.clone())
        .expect("first completion ok");

    // Second completion with identical payload should be no-op
    db.record_tool_call_done(done.clone())
        .expect("second completion no-op");

    // Verify stored terminal state
    let got = db
        .get_tool_call(request_id, &call_id)
        .expect("get tool call")
        .expect("must exist");
    assert!(got.1.is_some(), "terminal state should be present");
    assert_eq!(got.1.unwrap().status, ToolStatus::Completed);

    // Attempt invalid transition: Completed -> Failed
    let invalid_done = ToolCallDone {
        status: ToolStatus::Failed,
        error_kind: Some("SomeError".to_string()),
        error_msg: Some("boom".to_string()),
        outcome_json: None,
        ..done
    };
    let err = db.record_tool_call_done(invalid_done).unwrap_err();
    // Error kind string equality
    match err {
        ploke_db::DbError::InvalidLifecycle(_) => {}
        other => panic!("expected InvalidLifecycle error, got: {:?}", other),
    }
}
