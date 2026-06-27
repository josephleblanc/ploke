use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
};

use crate::call_graph_tool_support::{
    CallGraphToolFixture, assert_incoming_context, assert_target_proof,
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
    assert_eq!(
        ui.fields
            .iter()
            .find(|field| field.name.as_ref() == "call_context")
            .map(|field| field.value.as_ref()),
        Some(call_count.as_str())
    );
    assert_eq!(
        ui.fields
            .iter()
            .find(|field| field.name.as_ref() == "proof_context")
            .map(|field| field.value.as_ref()),
        Some(proof_count.as_str())
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
    assert_eq!(
        ui.fields
            .iter()
            .find(|field| field.name.as_ref() == "call_context")
            .map(|field| field.value.as_ref()),
        Some(call_count.as_str())
    );
    assert_eq!(
        ui.fields
            .iter()
            .find(|field| field.name.as_ref() == "proof_context")
            .map(|field| field.value.as_ref()),
        Some(proof_count.as_str())
    );
}
