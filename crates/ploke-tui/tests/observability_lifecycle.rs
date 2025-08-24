use chrono::Utc;
use ploke_db::observability::{
    ObservabilityStore, ToolCallDone, ToolCallReq, ToolStatus, Validity,
};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[tokio::test]
async fn tool_call_requested_then_completed_persists_latency_and_outcome() {
    let db = ploke_db::Database::init_with_schema().expect("init db schema");

    let request_id = Uuid::new_v4();
    let call_id = "call-abc".to_string();
    let parent_id = Uuid::new_v4();

    // Prepare a simple JSON payload
    let args_json = serde_json::json!({"foo":"bar"}).to_string();

    let args_sha256 = {
        let mut hasher = Sha256::new();
        hasher.update(args_json.as_bytes());
        format!("sha256:{:x}", hasher.finalize())
    };

    // Record "requested"
    let req = ToolCallReq {
        request_id,
        call_id: call_id.clone(),
        parent_id,
        vendor: "openai".into(),
        tool_name: "dummy".into(),
        args_sha256,
        arguments_json: Some(args_json),
        started_at: Validity {
            at: Utc::now().timestamp_millis(),
            is_valid: true,
        },
        model: todo!(),
        provider_slug: todo!(),
    };
    db.record_tool_call_requested(req)
        .expect("requested upsert should succeed");

    // Small delay so ended_at >= started_at
    tokio::time::sleep(std::time::Duration::from_millis(5)).await;

    // Fetch to get the started_at from DB (matches observability flow)
    let (req_meta, maybe_done) = db
        .get_tool_call(request_id, &call_id)
        .expect("query")
        .expect("requested row exists");
    assert!(maybe_done.is_none(), "should be requested only");

    let ended_at_ms = Utc::now().timestamp_millis();
    let latency_ms = (ended_at_ms - req_meta.started_at.at).max(0);

    // Record "done" (completed)
    let done = ToolCallDone {
        request_id,
        call_id: call_id.clone(),
        ended_at: Validity {
            at: ended_at_ms,
            is_valid: true,
        },
        latency_ms,
        outcome_json: Some(serde_json::json!({"ok": true}).to_string()),
        error_kind: None,
        error_msg: None,
        status: ToolStatus::Completed,
    };
    db.record_tool_call_done(done)
        .expect("done upsert should succeed");

    // Verify terminal state
    let (_req2, done2) = db
        .get_tool_call(request_id, &call_id)
        .expect("query2")
        .expect("row exists");
    let d = done2.expect("should be done");
    assert!(d.latency_ms >= 0);
    assert!(matches!(d.status, ToolStatus::Completed));
}

#[tokio::test]
async fn tool_call_terminal_status_is_immutable() {
    let db = ploke_db::Database::init_with_schema().expect("init db schema");

    let request_id = Uuid::new_v4();
    let call_id = "call-immutable".to_string();
    let parent_id = Uuid::new_v4();

    // requested
    let req = ToolCallReq {
        request_id,
        call_id: call_id.clone(),
        parent_id,
        vendor: "openai".into(),
        tool_name: "dummy".into(),
        args_sha256: "sha256:deadbeef".into(),
        arguments_json: Some("{}".into()),
        started_at: Validity {
            at: Utc::now().timestamp_millis(),
            is_valid: true,
        },
        model: todo!(),
        provider_slug: todo!(),
    };
    db.record_tool_call_requested(req)
        .expect("requested upsert should succeed");

    // completed
    let done_ok = ToolCallDone {
        request_id,
        call_id: call_id.clone(),
        ended_at: Validity {
            at: Utc::now().timestamp_millis(),
            is_valid: true,
        },
        latency_ms: 0,
        outcome_json: Some(r#"{"ok":true}"#.into()),
        error_kind: None,
        error_msg: None,
        status: ToolStatus::Completed,
    };
    db.record_tool_call_done(done_ok)
        .expect("first terminal write should succeed");

    // attempt to change terminal status → error
    let done_fail = ToolCallDone {
        request_id,
        call_id: call_id.clone(),
        ended_at: Validity {
            at: Utc::now().timestamp_millis(),
            is_valid: true,
        },
        latency_ms: 0,
        outcome_json: None,
        error_kind: Some("forced".into()),
        error_msg: Some("should not be allowed".into()),
        status: ToolStatus::Failed,
    };
    let res = db.record_tool_call_done(done_fail);
    assert!(res.is_err(), "changing terminal status must be rejected");
}
