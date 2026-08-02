use std::borrow::Cow;

use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallReceiverInfo, CallSiteKind, CallStatusKind, CallTargetKind,
};
use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
    get_code_edges::{CodeItemEdges, EdgesParams},
};

use crate::call_graph_tool_support::{AxumExpandWithToolFixture, AxumRouteMethodToolFixture};

#[tokio::test]
async fn code_item_lookup_exposes_axum_method_callback_candidates() {
    let fixture = AxumExpandWithToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("expand_with"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(
        params,
        fixture.ctx("axum-expand-with-method-callback-lookup"),
    )
    .await
    .expect("expand_with lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let call_context = payload
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("call_context array");

    assert_axum_expand_with_method_callback(call_context, "code_item_lookup");
}

#[tokio::test]
async fn code_item_edges_exposes_axum_method_callback_candidates() {
    let fixture = AxumExpandWithToolFixture::new().await;
    let params = EdgesParams {
        item_name: Cow::Borrowed("expand_with"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemEdges::execute(
        params,
        fixture.ctx("axum-expand-with-method-callback-edges"),
    )
    .await
    .expect("expand_with edges");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
    let call_context = payload
        .get("node_info")
        .and_then(|node| node.get("call_context"))
        .and_then(serde_json::Value::as_array)
        .expect("node_info.call_context array");

    assert_axum_expand_with_method_callback(call_context, "code_item_edges");
}

fn assert_axum_expand_with_method_callback(call_context: &[serde_json::Value], tool: &str) {
    let rows = call_context
        .iter()
        .map(|row| serde_json::from_value::<CallContextInfo>(row.clone()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("{tool} method-callback context should deserialize: {err}"));
    let and_then = rows
        .iter()
        .filter(|row| {
            matches!(
                &row.callee,
                CallCalleeInfo::Method {
                    name,
                    receiver: Some(CallReceiverInfo::PathCallResult { path })
                } if name == "and_then" && path == &vec!["syn".to_string(), "parse".to_string()]
            )
        })
        .collect::<Vec<_>>();

    // Source oracle:
    //   axum-macros/src/lib.rs:724 calls
    //   `expand(syn::parse(input).and_then(f))` inside `expand_with`.
    //   axum-macros/src/lib.rs:715 passes the function item `from_ref::expand`.
    //   axum-macros/src/lib.rs:377,426,665 pass closure callbacks.
    // Current contract: the call remains ambiguous and targetless for traversal,
    // but TUI tools must expose all finite method-callback candidates.
    assert_eq!(
        and_then.len(),
        1,
        "{tool} should expose exactly one axum expand_with and_then(f) row: {rows:#?}"
    );
    let row = and_then[0];
    assert_eq!(row.kind, CallSiteKind::Method);
    assert_eq!(row.arg_count, Some(1));
    assert_eq!(row.status, CallStatusKind::Ambiguous);
    assert_eq!(row.resolution, None);
    assert_eq!(
        row.targets.len(),
        4,
        "{tool} and_then(f) should expose four finite callback candidates: {row:#?}"
    );
    assert_eq!(
        row.targets
            .iter()
            .filter(|target| target.relation == CallTargetKind::MethodCallbackFunction)
            .count(),
        1,
        "{tool} and_then(f) should include from_ref::expand as the function candidate: {row:#?}"
    );
    assert_eq!(
        row.targets
            .iter()
            .filter(|target| target.relation == CallTargetKind::MethodCallbackClosure)
            .count(),
        3,
        "{tool} and_then(f) should include the three closure callback candidates: {row:#?}"
    );
}

#[tokio::test]
async fn code_item_lookup_exposes_axum_generated_route_method_edges() {
    let fixture = AxumRouteMethodToolFixture::new().await;
    for case in &fixture.cases {
        let params = LookupParams {
            item_name: Cow::Borrowed(case.name),
            file_path: Cow::Owned(case.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(case.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed("MethodRouter")),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result =
            CodeItemLookup::execute(params, fixture.ctx("axum-generated-route-method-lookup"))
                .await
                .unwrap_or_else(|err| panic!("{} lookup: {err}", case.name));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");

        assert_axum_generated_route_method(call_context, case, "code_item_lookup");
    }
}

#[tokio::test]
async fn code_item_edges_exposes_axum_generated_route_method_edges() {
    let fixture = AxumRouteMethodToolFixture::new().await;
    for case in &fixture.cases {
        let params = EdgesParams {
            item_name: Cow::Borrowed(case.name),
            file_path: Cow::Owned(case.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(case.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed("MethodRouter")),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result =
            CodeItemEdges::execute(params, fixture.ctx("axum-generated-route-method-edges"))
                .await
                .unwrap_or_else(|err| panic!("{} edges: {err}", case.name));
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize NodeEdgeInfo");
        let call_context = payload
            .get("node_info")
            .and_then(|node| node.get("call_context"))
            .and_then(serde_json::Value::as_array)
            .expect("node_info.call_context array");

        assert_axum_generated_route_method(call_context, case, "code_item_edges");
    }
}

fn assert_axum_generated_route_method(
    call_context: &[serde_json::Value],
    case: &crate::call_graph_tool_support::RouteMethodCase,
    tool: &str,
) {
    let rows = call_context
        .iter()
        .map(|row| serde_json::from_value::<CallContextInfo>(row.clone()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("{tool} generated route method context: {err}"));
    let call = rows
        .iter()
        .find(|row| row.owner_id == case.call.owner && row.site_id == case.call.site)
        .unwrap_or_else(|| {
            panic!(
                "{tool} should expose generated {} self-call row: {rows:#?}",
                case.name
            )
        });

    // Source oracle:
    //   axum/src/routing/method_routing.rs:263-326 templates
    //   `chained_handler_fn!`; invocation :648 generates `post`, whose body
    //   calls `self.on(MethodFilter::POST, handler)`.
    //   axum/src/routing/method_routing.rs:176-259 templates
    //   `chained_service_fn!`; invocation :998 generates `post_service`, whose
    //   body calls `self.on_service(MethodFilter::POST, svc)`.
    assert_eq!(call.kind, CallSiteKind::Method);
    assert_eq!(call.arg_count, Some(2));
    assert_eq!(call.callee, case.call.callee);
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(
        call.resolution,
        Some(ploke_core::rag_types::CallResolutionKind::LocalExact)
    );
    assert_eq!(
        call.targets.len(),
        1,
        "{tool} generated {} should expose one local target: {call:#?}",
        case.name
    );
    assert_eq!(call.targets[0].target_id, case.call.target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Method);
}
