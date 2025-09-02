use ploke_core::ArcStr;
use ploke_db::observability::{ObservabilityStore, ToolCallReq, Validity};
use uuid::Uuid;

#[test]
fn debug_record_tool_call_requested() {
    let db = ploke_db::Database::init_with_schema().expect("init db schema");
    let req = ToolCallReq {
        request_id: Uuid::new_v4(),
        call_id: ArcStr::from( "call-xyz" ),
        parent_id: Uuid::new_v4(),
        model: "gpt-x".to_string(),
        provider_slug: Some("openai".to_string()),
        tool_name: "dummy".to_string(),
        args_sha256: "sha256:deadbeef".to_string(),
        arguments_json: Some("{}".to_string()),
        started_at: Validity { at: 0, is_valid: true },
    };
    let res = db.record_tool_call_requested(req);
    assert!(res.is_ok(), "expected Ok, got {:?}", res);
}
