use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
    get_code_edges::{CodeItemEdges, EdgesParams},
};

use crate::call_graph_tool_support::{
    AxumRemainingTarget, AxumRemainingToolFixture, assert_dependency_root_proof,
    assert_expected_remaining_incoming_context, assert_target_proof, ui_field,
};

#[tokio::test]
async fn code_item_lookup_returns_remaining_real_corpus_supported_callers() {
    for case in AxumRemainingTarget::TOOL_REACHABLE_CASES {
        assert_lookup_case(case, "axum-remaining-lookup").await;
    }
}

#[tokio::test]
async fn code_item_lookup_returns_request_parts_ext_local_receiver_caller() {
    assert_lookup_case(
        AxumRemainingTarget::RequestPartsExtExtract,
        "axum-request-parts-ext-lookup",
    )
    .await;
}

#[tokio::test]
async fn code_item_lookup_returns_trait_bound_remaining_real_corpus_callers() {
    for case in [
        AxumRemainingTarget::FromRequestParts,
        AxumRemainingTarget::FromRef,
    ] {
        assert_lookup_case(case, "axum-trait-bound-lookup").await;
    }
}

#[tokio::test]
async fn code_item_lookup_returns_closure_owned_expand_field_caller() {
    assert_lookup_case(AxumRemainingTarget::ExpandField, "axum-expand-field-lookup").await;
}

#[tokio::test]
async fn code_item_lookup_returns_test_client_new_high_fanout_callers() {
    assert_lookup_case(
        AxumRemainingTarget::TestClientNew,
        "axum-test-client-new-lookup",
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_remaining_real_corpus_supported_callers() {
    for case in AxumRemainingTarget::TOOL_REACHABLE_CASES {
        assert_edges_case(case, "axum-remaining-edges").await;
    }
}

#[tokio::test]
async fn code_item_edges_returns_request_parts_ext_local_receiver_caller() {
    assert_edges_case(
        AxumRemainingTarget::RequestPartsExtExtract,
        "axum-request-parts-ext-edges",
    )
    .await;
}

#[tokio::test]
async fn code_item_edges_returns_trait_bound_remaining_real_corpus_callers() {
    for case in [
        AxumRemainingTarget::FromRequestParts,
        AxumRemainingTarget::FromRef,
    ] {
        assert_edges_case(case, "axum-trait-bound-edges").await;
    }
}

#[tokio::test]
async fn code_item_edges_returns_closure_owned_expand_field_caller() {
    assert_edges_case(AxumRemainingTarget::ExpandField, "axum-expand-field-edges").await;
}

#[tokio::test]
async fn code_item_edges_returns_test_client_new_high_fanout_callers() {
    assert_edges_case(
        AxumRemainingTarget::TestClientNew,
        "axum-test-client-new-edges",
    )
    .await;
}

async fn assert_lookup_case(case: AxumRemainingTarget, call_id: &'static str) {
    let fixture = AxumRemainingToolFixture::new(case).await;
    let module_path = fixture.module_path_arg();
    let params = LookupParams {
        item_name: Cow::Borrowed(fixture.item_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed(fixture.node_kind),
        module_path: Cow::Owned(module_path),
        owner_trait: fixture.owner_trait.map(Cow::Borrowed),
        owner_type: fixture.owner_type.map(Cow::Borrowed),
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx(call_id))
        .await
        .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.label));
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

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // This batch covers tool-reachable supported rows from the DB/RAG matrix.
    // Each exact lookup should expose the same incoming call-site identities
    // and callee shape as the target-centered DB traversal.
    assert_expected_remaining_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_lookup",
        fixture.label,
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_lookup",
        );
    }
    assert_dependency_root_proof(
        proof_context,
        &fixture.dependency_root_sites,
        fixture.target,
        "code_item_lookup",
        fixture.label,
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(
        ui_field(ui, "call_context_incoming"),
        fixture.callers.len().to_string().as_str()
    );
}

async fn assert_edges_case(case: AxumRemainingTarget, call_id: &'static str) {
    let fixture = AxumRemainingToolFixture::new(case).await;
    let module_path = fixture.module_path_arg();
    let params = EdgesParams {
        item_name: Cow::Borrowed(fixture.item_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed(fixture.node_kind),
        module_path: Cow::Owned(module_path),
        owner_trait: fixture.owner_trait.map(Cow::Borrowed),
        owner_type: fixture.owner_type.map(Cow::Borrowed),
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx(call_id))
        .await
        .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.label));
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let node_info = payload.get("node_info").expect("node_info");
    let call_context = node_info
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let proof_context = node_info
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.proof_context array");

    // Same real-corpus oracle batch as the lookup path above, exercised
    // through `code_item_edges` so tool callers can traverse from the exact
    // target item to its incoming callsites in the edge-oriented payload.
    assert_expected_remaining_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_edges",
        fixture.label,
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_edges",
        );
    }
    assert_dependency_root_proof(
        proof_context,
        &fixture.dependency_root_sites,
        fixture.target,
        "code_item_edges",
        fixture.label,
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(
        ui_field(ui, "call_context_incoming"),
        fixture.callers.len().to_string().as_str()
    );
}
