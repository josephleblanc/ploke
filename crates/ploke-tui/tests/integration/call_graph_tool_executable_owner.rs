use std::borrow::Cow;

use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallReceiverInfo, CallSiteKind, CallStatusKind,
};
use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
    get_code_edges::{CodeItemEdges, EdgesParams},
};

use crate::call_graph_tool_support::{LocalItemToolFixture, ui_field};

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
async fn code_item_edges_accepts_local_item_body_owner() {
    let fixture = LocalItemToolFixture::axum_path_deserialize_local_impl_method().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("local_impl_method:deserialize"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("local_item"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
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
