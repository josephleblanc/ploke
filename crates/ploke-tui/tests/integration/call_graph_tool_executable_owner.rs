use std::borrow::Cow;

use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallPathInfo, CallReceiverInfo, CallResolutionKind,
    CallSiteKind, CallStatusKind, CallTargetKind,
};
use ploke_tui::tools::{
    Tool,
    code_item_call_path::{CodeItemCallPath, CodeItemCallPathEndpoint, CodeItemCallPathParams},
    code_item_lookup::{CodeItemLookup, LookupParams},
    get_code_edges::{CodeItemEdges, EdgesParams},
};

use crate::call_graph_tool_support::{
    AxumHandlerAsyncBlockToolFixture, LocalItemToolFixture, assert_method_proof,
    assert_path_blocker_proof, assert_runtime_dispatch_blocker, ui_field,
};

const AXUM_DOMAIN: &str = "bd:corpus-axum-call-graph";

#[tokio::test]
async fn code_item_lookup_accepts_local_item_body_owner() {
    let fixture = LocalItemToolFixture::axum_path_deserialize_local_impl_method().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("local_impl_method:deserialize"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("local_item"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("local-item-lookup"))
        .await
        .expect("code_item_lookup should accept local_item executable owners");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let call_context = payload
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("call_context array");
    let calls = call_context
        .iter()
        .map(|value| {
            serde_json::from_value::<CallContextInfo>(value.clone()).expect("call context row")
        })
        .collect::<Vec<_>>();
    // Ground truth: axum/src/extract/path/mod.rs:770 defines a local
    // `impl serde::Deserialize for Param`; line 774 calls
    // `<&str as serde::Deserialize>::deserialize(deserializer)?`.
    let row = local_item_serde_deserialize_call(&calls, &fixture);

    assert_eq!(row.status, CallStatusKind::External);
    assert!(row.resolution.is_none());
    assert!(row.targets.is_empty());
    assert!(local_item_unsupported_path_call(&calls, &fixture, &["Ok"]).is_some());
    assert!(local_item_unsupported_path_call(&calls, &fixture, &["Self"]).is_some());
    let method_row = local_item_to_owned_call(&calls, &fixture);
    assert_eq!(method_row.status, CallStatusKind::Unsupported);
    assert!(method_row.resolution.is_none());
    assert!(method_row.targets.is_empty());

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_outgoing"), "4");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= 2,
        "local_item lookup should surface proof rows"
    );
}

#[tokio::test]
async fn code_item_lookup_rejects_ambiguous_local_item_body_owner_without_parent() {
    let fixture =
        LocalItemToolFixture::axum_from_extractor_from_request_parts_local_impl_method().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("local_impl_method:from_request_parts"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("local_item"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
    };

    let err =
        match CodeItemLookup::execute(params, fixture.ctx("ambiguous-local-item-lookup")).await {
            Ok(result) => panic!(
                "unqualified local_impl_method:from_request_parts lookup should fail closed: {}",
                result.content
            ),
            Err(err) => err,
        };
    let message = err.to_string();
    assert!(
        message.contains("Multiple items matched"),
        "ambiguous local-item lookup should explain the duplicate exact coordinate: {message}"
    );
}

#[tokio::test]
async fn code_item_lookup_parent_qualifies_repeated_local_item_body_owner() {
    let fixture =
        LocalItemToolFixture::axum_from_extractor_from_request_parts_local_impl_method().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("local_impl_method:from_request_parts"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("local_item"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: Some(Cow::Borrowed("test_from_extractor")),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("parent-local-item-lookup"))
        .await
        .expect("parent_name should disambiguate repeated local impl method labels");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let owner_id = fixture.owner.to_string();
    assert_eq!(
        payload.get("id").and_then(serde_json::Value::as_str),
        Some(owner_id.as_str())
    );
    let call_context = payload
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("call_context array");
    let calls = call_context
        .iter()
        .map(|value| {
            serde_json::from_value::<CallContextInfo>(value.clone()).expect("call context row")
        })
        .collect::<Vec<_>>();

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/middleware/from_extractor.rs:311 defines
    //   `test_from_extractor`; lines 317-340 define a function-local impl
    //   method `from_request_parts`; line 328 calls `Secret::from_ref(state)`.
    // Expected traversal: parent-qualified exact lookup selects the nested
    // `local_impl_method:from_request_parts` owner and exposes the resolved
    // associated-function edge to `FromRef::from_ref`.
    let row = local_item_secret_from_ref_call(&calls, &fixture);
    assert_eq!(row.status, CallStatusKind::Resolved);
    assert_eq!(row.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1, "{row:#?}");
    assert_eq!(row.targets[0].relation, CallTargetKind::AssociatedFunction);

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_outgoing"), "5");
}

#[tokio::test]
async fn code_item_edges_accepts_local_item_body_owner() {
    let fixture = LocalItemToolFixture::axum_path_deserialize_local_impl_method().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("local_impl_method:deserialize"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("local_item"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("local-item-edges"))
        .await
        .expect("code_item_edges should accept local_item executable owners");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let node_info = payload.get("node_info").expect("node_info");
    let call_context = node_info
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let calls = call_context
        .iter()
        .map(|value| {
            serde_json::from_value::<CallContextInfo>(value.clone()).expect("call context row")
        })
        .collect::<Vec<_>>();
    // This owner is a local item embedded in a test body. It should be exact
    // addressable by tools and expose its call rows, even though its callee is
    // external and therefore has no local call path.
    let row = local_item_serde_deserialize_call(&calls, &fixture);
    assert_eq!(row.status, CallStatusKind::External);
    let method_row = local_item_to_owned_call(&calls, &fixture);
    assert_eq!(method_row.status, CallStatusKind::Unsupported);

    let edges = payload
        .get("edge_info")
        .and_then(serde_json::Value::as_array)
        .expect("edge_info array");
    assert!(
        edges.is_empty(),
        "call_body_owner local items do not currently own syntax_edge rows"
    );
    let paths = payload
        .get("call_paths_from_owner")
        .and_then(serde_json::Value::as_array)
        .expect("call_paths_from_owner array");
    assert!(paths.is_empty());

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "edges"), "0");
    assert_eq!(ui_field(ui, "call_context_outgoing"), "4");
    assert_eq!(ui_field(ui, "call_paths_from_owner"), "0");
}

#[tokio::test]
async fn code_item_edges_parent_qualifies_repeated_local_item_body_owner() {
    let fixture =
        LocalItemToolFixture::axum_from_extractor_from_request_parts_local_impl_method().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("local_impl_method:from_request_parts"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("local_item"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: Some(Cow::Borrowed("test_from_extractor")),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("parent-local-item-edges"))
        .await
        .expect("code_item_edges should accept parent-qualified local item owners");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");
    let calls = call_context
        .iter()
        .map(|value| {
            serde_json::from_value::<CallContextInfo>(value.clone()).expect("call context row")
        })
        .collect::<Vec<_>>();
    let row = local_item_secret_from_ref_call(&calls, &fixture);
    let target = row.targets[0].target_id;
    let paths = payload
        .get("call_paths_from_owner")
        .and_then(serde_json::Value::as_array)
        .expect("call_paths_from_owner array")
        .iter()
        .map(|value| serde_json::from_value::<CallPathInfo>(value.clone()).expect("call path row"))
        .collect::<Vec<_>>();
    assert!(
        paths.iter().any(|path| {
            path.start_id == fixture.owner
                && path.end_id == target
                && path.depth == 1
                && path.edges.len() == 1
                && path.edges[0].call_site_id == row.site_id
                && path.edges[0].relation == CallTargetKind::AssociatedFunction
        }),
        "parent-qualified local item should expose the one-hop Secret::from_ref traversal: {paths:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_paths_from_owner"), "1");
}

#[tokio::test]
async fn code_item_lookup_accepts_real_corpus_async_block_body_owner() {
    let fixture = AxumHandlerAsyncBlockToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("async_block"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("async_block"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: Some(Cow::Borrowed("call")),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("async-block-lookup"))
        .await
        .expect("code_item_lookup should accept async_block executable owners");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let owner_id = fixture.owner.to_string();
    assert_eq!(
        payload.get("id").and_then(serde_json::Value::as_str),
        Some(owner_id.as_str())
    );
    let call_context = payload
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("call_context array");
    let proof_context = payload
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("proof_context array");
    let calls = decode_call_context(call_context);

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/handler/mod.rs:216 defines the concrete Handler::call method.
    //   axum/src/handler/mod.rs:217 creates
    //   `Box::pin(async move { self().await.into_response() })`.
    // Expected traversal: the exact tool can address the nested async-block
    // owner, but its `self()` callable and awaited `into_response()` rows
    // stay unsupported and targetless until broader async poll/resume and
    // callable binding proof exists.
    assert_handler_async_block_targetless_rows(&calls, proof_context, &fixture, "code_item_lookup");

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 2,
        "async_block lookup should surface both targetless async-block rows"
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= 4,
        "async_block lookup should surface projected blocker proof rows"
    );
}

#[tokio::test]
async fn code_item_edges_accepts_real_corpus_async_block_body_owner() {
    let fixture = AxumHandlerAsyncBlockToolFixture::new().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("async_block"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("async_block"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: Some(Cow::Borrowed("call")),
    };

    let result = CodeItemEdges::execute(params, fixture.ctx("async-block-edges"))
        .await
        .expect("code_item_edges should accept async_block executable owners");
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
    let calls = decode_call_context(call_context);

    assert_handler_async_block_targetless_rows(&calls, proof_context, &fixture, "code_item_edges");

    let edges = payload
        .get("edge_info")
        .and_then(serde_json::Value::as_array)
        .expect("edge_info array");
    assert!(
        edges.is_empty(),
        "call_body_owner async blocks do not currently own syntax_edge rows"
    );
    let paths = payload
        .get("call_paths_from_owner")
        .and_then(serde_json::Value::as_array)
        .expect("call_paths_from_owner array")
        .iter()
        .map(|value| serde_json::from_value::<CallPathInfo>(value.clone()).expect("call path row"))
        .collect::<Vec<_>>();
    assert!(
        paths.is_empty(),
        "targetless async-block rows must not fabricate traversal paths: {paths:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "edges"), "0");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 2,
        "async_block edges should surface both targetless async-block rows"
    );
    assert!(
        ui_field(ui, "blocked_calls")
            .parse::<usize>()
            .expect("blocked call count")
            >= 2,
        "async_block edges should count both unsupported async-block rows"
    );
}

#[tokio::test]
async fn code_item_call_path_accepts_local_item_body_owner_endpoint() {
    let fixture = LocalItemToolFixture::axum_path_deserialize_local_impl_method().await;
    let endpoint = CodeItemCallPathEndpoint {
        item_name: Cow::Borrowed("local_impl_method:deserialize"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("local_item"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
    };
    let params = CodeItemCallPathParams {
        source: endpoint.clone(),
        target: endpoint,
        max_depth: Some(1),
        max_paths: Some(4),
    };

    let result = CodeItemCallPath::execute(params, fixture.ctx("local-item-call-path"))
        .await
        .expect("code_item_call_path should accept local_item executable owner endpoints");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize CodeItemCallPathResult");
    let owner_id = fixture.owner.to_string();

    assert_eq!(
        payload.get("source_id").and_then(serde_json::Value::as_str),
        Some(owner_id.as_str())
    );
    assert_eq!(
        payload.get("target_id").and_then(serde_json::Value::as_str),
        Some(owner_id.as_str())
    );
    assert_eq!(
        payload
            .get("reachable")
            .and_then(serde_json::Value::as_bool),
        Some(false),
        "an exact local_item endpoint must not fabricate a zero-length traversal path"
    );
    let paths = payload
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .expect("paths array");
    assert!(paths.is_empty());
    let proof_context = payload
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("proof_context array");
    assert!(
        proof_context.len() >= 2,
        "local_item endpoint should still carry projected proof rows"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "reachable"), "false");
    assert_eq!(ui_field(ui, "paths"), "0");
}

fn local_item_secret_from_ref_call<'a>(
    calls: &'a [CallContextInfo],
    fixture: &LocalItemToolFixture,
) -> &'a CallContextInfo {
    calls
        .iter()
        .find(|call| {
            call.owner_id == fixture.owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["Secret".to_string(), "from_ref".to_string()],
                    }
        })
        .unwrap_or_else(|| {
            panic!(
                "expected local_impl_method:from_request_parts to expose Secret::from_ref() call: {calls:#?}"
            )
        })
}

fn local_item_serde_deserialize_call<'a>(
    calls: &'a [CallContextInfo],
    fixture: &LocalItemToolFixture,
) -> &'a CallContextInfo {
    calls
        .iter()
        .find(|call| {
            call.owner_id == fixture.owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec![
                            "serde".to_string(),
                            "Deserialize".to_string(),
                            "deserialize".to_string(),
                        ],
                    }
        })
        .unwrap_or_else(|| {
            panic!(
                "expected local_impl_method:deserialize to expose serde::Deserialize::deserialize() call: {calls:#?}"
            )
        })
}

fn local_item_to_owned_call<'a>(
    calls: &'a [CallContextInfo],
    fixture: &LocalItemToolFixture,
) -> &'a CallContextInfo {
    calls
        .iter()
        .find(|call| {
            call.owner_id == fixture.owner
                && call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "to_owned".to_string(),
                        receiver: Some(CallReceiverInfo::Unsupported),
                    }
        })
        .unwrap_or_else(|| {
            panic!(
                "expected local_impl_method:deserialize to expose targetless s.to_owned() call: {calls:#?}"
            )
        })
}

fn local_item_unsupported_path_call<'a>(
    calls: &'a [CallContextInfo],
    fixture: &LocalItemToolFixture,
    path: &[&str],
) -> Option<&'a CallContextInfo> {
    let path = path
        .iter()
        .map(|part| (*part).to_string())
        .collect::<Vec<_>>();
    calls.iter().find(|call| {
        call.owner_id == fixture.owner
            && call.kind == CallSiteKind::Path
            && call.status == CallStatusKind::Unsupported
            && call.callee == CallCalleeInfo::Path { path: path.clone() }
    })
}

fn decode_call_context(values: &[serde_json::Value]) -> Vec<CallContextInfo> {
    values
        .iter()
        .map(|value| {
            serde_json::from_value::<CallContextInfo>(value.clone()).expect("call context row")
        })
        .collect()
}

fn async_block_self_call<'a>(
    calls: &'a [CallContextInfo],
    fixture: &AxumHandlerAsyncBlockToolFixture,
) -> &'a CallContextInfo {
    calls
        .iter()
        .find(|call| {
            call.owner_id == fixture.owner
                && call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["self".to_string()],
                    }
        })
        .unwrap_or_else(|| {
            panic!("expected Handler::call async block to expose self() call: {calls:#?}")
        })
}

fn async_block_into_response_call<'a>(
    calls: &'a [CallContextInfo],
    fixture: &AxumHandlerAsyncBlockToolFixture,
) -> &'a CallContextInfo {
    calls
        .iter()
        .find(|call| {
            call.owner_id == fixture.owner
                && call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "into_response".to_string(),
                        receiver: Some(CallReceiverInfo::AwaitPathCallResult {
                            path: vec!["self".to_string()],
                        }),
                    }
        })
        .unwrap_or_else(|| {
            panic!(
                "expected Handler::call async block to expose awaited into_response() call: {calls:#?}"
            )
        })
}

fn assert_handler_async_block_targetless_rows(
    calls: &[CallContextInfo],
    proof_context: &[serde_json::Value],
    fixture: &AxumHandlerAsyncBlockToolFixture,
    tool: &str,
) {
    let self_row = async_block_self_call(calls, fixture);
    assert_unsupported_targetless(self_row, "Handler::call async-block self()");
    assert_path_blocker_proof(
        proof_context,
        fixture.owner,
        self_row.site_id,
        AXUM_DOMAIN,
        "type_resolution_missing",
        "Handler::call async-block self()",
        tool,
    );
    assert_runtime_dispatch_blocker(
        proof_context,
        self_row.site_id,
        "Handler::call async-block self() async poll/resume",
        tool,
    );

    let into_response = async_block_into_response_call(calls, fixture);
    assert_unsupported_targetless(into_response, "Handler::call async-block into_response()");
    assert_method_proof(
        proof_context,
        fixture.owner,
        into_response.site_id,
        &CallStatusKind::Unsupported,
        "Handler::call async-block into_response()",
        tool,
    );
    assert_runtime_dispatch_blocker(
        proof_context,
        into_response.site_id,
        "Handler::call async-block into_response() async poll/resume",
        tool,
    );
}

fn assert_unsupported_targetless(call: &CallContextInfo, label: &str) {
    assert_eq!(
        call.status,
        CallStatusKind::Unsupported,
        "{label}: {call:#?}"
    );
    assert_eq!(call.resolution, None, "{label}: {call:#?}");
    assert!(
        call.targets.is_empty(),
        "{label} should remain targetless: {call:#?}"
    );
}
