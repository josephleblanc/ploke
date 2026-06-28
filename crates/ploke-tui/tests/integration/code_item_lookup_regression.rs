use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
};

use crate::call_graph_tool_support::{
    AxumBodyEmptyToolFixture, CallGraphToolFixture, assert_body_empty_incoming_context,
    assert_incoming_context, assert_target_proof, ui_field,
};

#[tokio::test]
async fn code_item_lookup_returns_call_and_proof_context_for_call_graph_item() {
    let fixture = CallGraphToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("call_crate_local_target"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("call-graph-lookup"))
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let call_context = payload
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("call_context array");
    let proof_context = payload
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("proof_context array");
    let owner = fixture.owner.to_string();

    assert!(
        call_context.iter().any(|call| {
            call.get("owner_id").and_then(serde_json::Value::as_str) == Some(owner.as_str())
                && call.get("kind").and_then(serde_json::Value::as_str) == Some("path")
                && call
                    .get("targets")
                    .and_then(serde_json::Value::as_array)
                    .is_some_and(|targets| !targets.is_empty())
        }),
        "code_item_lookup should return node-scoped call context for call_crate_local_target: {call_context:#?}"
    );
    assert!(
        proof_context.iter().any(|proof| {
            proof.get("kind").and_then(serde_json::Value::as_str) == Some("call_edge")
                && proof
                    .get("caller_def_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(owner.as_str())
        }),
        "code_item_lookup should return node-scoped proof context for call_crate_local_target: {proof_context:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    let call_count = call_context.len().to_string();
    let proof_count = proof_context.len().to_string();
    assert_eq!(ui_field(ui, "call_context"), call_count.as_str());
    assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_lookup should surface outgoing call-context count for owner lookups"
    );
}

#[tokio::test]
async fn code_item_lookup_returns_incoming_callers_for_call_graph_target() {
    let fixture = CallGraphToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("local_target"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("call-graph-target-lookup"))
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let call_context = payload
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("call_context array");
    let proof_context = payload
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("proof_context array");
    assert_incoming_context(
        call_context,
        fixture.owner,
        fixture.target,
        "code_item_lookup",
    );
    assert_target_proof(
        proof_context,
        fixture.owner,
        fixture.target,
        "code_item_lookup",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    let call_count = call_context.len().to_string();
    let proof_count = proof_context.len().to_string();
    assert_eq!(ui_field(ui, "call_context"), call_count.as_str());
    assert_eq!(ui_field(ui, "proof_context"), proof_count.as_str());
    assert!(
        ui_field(ui, "call_context_incoming")
            .parse::<usize>()
            .expect("incoming count")
            >= 1,
        "code_item_lookup should surface incoming caller count for target lookups"
    );
}

#[tokio::test]
async fn code_item_lookup_returns_real_corpus_body_empty_callers() {
    let fixture = AxumBodyEmptyToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = LookupParams {
        item_name: Cow::Borrowed("empty"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("axum-body-empty-lookup"))
        .await
        .expect("tool execution");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let call_context = payload
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("call_context array");
    let proof_context = payload
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("proof_context array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/response/into_response.rs:128 calls `Body::empty()`.
    //   axum-core/src/response/into_response.rs:163 calls `Body::empty()`.
    // Expected tool traversal: exact lookup of the callee method exposes both
    // incoming caller-site edges and their projected proof rows.
    assert_body_empty_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_lookup",
    );
    for caller in &fixture.callers {
        assert_target_proof(proof_context, *caller, fixture.target, "code_item_lookup");
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "2");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= 2,
        "code_item_lookup should surface real-corpus Body::empty proof rows"
    );
}
