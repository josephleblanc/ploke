use std::borrow::Cow;

use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallPathEdgeInfo, CallSiteBucketInfo, CallSiteKind,
    CallStatusKind, CallTargetKind,
};
use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
};

use crate::call_graph_tool_support::{
    AxumAwaitReceiverToolFixture, AxumBodyEmptyToolFixture, AxumBoxedIntoRouteToolFixture,
    AxumExpandWithToolFixture, AxumHandlerCallToolFixture, AxumJsonFromBytesToolFixture,
    AxumParseAttrsToolFixture, AxumRequestExtractPathToolFixture, AxumRunUiTestsToolFixture,
    CallGraphToolFixture, assert_await_result_unwrap_context, assert_await_result_unwrap_proof,
    assert_body_empty_incoming_context, assert_boxed_into_route_incoming_context,
    assert_call_path_node, assert_handler_call_incoming_context, assert_incoming_context,
    assert_json_from_bytes_incoming_context, assert_parse_attrs_incoming_context,
    assert_run_ui_tests_incoming_context, assert_target_proof, assert_two_hop_call_path, ui_field,
};

#[tokio::test]
async fn code_item_lookup_returns_call_and_proof_context_for_call_graph_item() {
    let fixture = CallGraphToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("call_crate_local_target"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
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
async fn code_item_lookup_returns_real_corpus_await_receiver_targetless_row() {
    let fixture = AxumAwaitReceiverToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = LookupParams {
        item_name: Cow::Borrowed("accept"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("ConnLimiter")),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("axum-await-lookup"))
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
    let reach = payload
        .get("call_reach")
        .and_then(serde_json::Value::as_object)
        .expect("call_reach object");
    let unsupported_frontier = reach
        .get("unsupported_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach unsupported_frontier_calls array");

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/serve/listener.rs:142 owns `ConnLimiter<T>::accept`.
    //   axum/src/serve/listener.rs:143 calls
    //   `self.sem.clone().acquire_owned().await.unwrap()`.
    // The exact lookup tool should expose the DB/RAG-pinned targetless
    // `AwaitResult.unwrap` row without inventing an outgoing target edge.
    let site_id =
        assert_await_result_unwrap_context(call_context, fixture.owner, "code_item_lookup");
    assert_await_result_unwrap_proof(proof_context, fixture.owner, site_id, "code_item_lookup");
    let unsupported_calls = unsupported_frontier
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed unsupported frontier rows");
    let unsupported = unsupported_calls
        .iter()
        .find(|call| call.site_id == site_id)
        .unwrap_or_else(|| {
            panic!(
                "code_item_lookup should expose AwaitResult unwrap in unsupported frontier rows: {unsupported_calls:#?}"
            )
        });
    assert_eq!(unsupported.owner_id, fixture.owner);
    assert_eq!(unsupported.status, CallStatusKind::Unsupported);
    assert!(
        unsupported.targets.is_empty(),
        "unsupported frontier call should remain targetless: {unsupported:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_lookup should surface outgoing targetless call-context count"
    );
    assert_eq!(
        ui_field(ui, "reach_unsupported_frontier_calls"),
        unsupported_calls.len().to_string()
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= 2,
        "code_item_lookup should surface targetless AwaitResult proof rows"
    );
}

#[tokio::test]
async fn code_item_lookup_returns_real_corpus_two_hop_call_paths() {
    let fixture = AxumRequestExtractPathToolFixture::new().await;
    let start_params = LookupParams {
        item_name: Cow::Borrowed("extract"),
        file_path: Cow::Owned(fixture.start_file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(fixture.start_module_path_arg()),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("Request")),
    };

    let start_result = CodeItemLookup::execute(
        start_params,
        fixture.ctx("axum-request-extract-lookup-paths"),
    )
    .await
    .expect("RequestExt::extract lookup");
    let start_payload: serde_json::Value =
        serde_json::from_str(&start_result.content).expect("deserialize start ConciseContext");
    let outgoing_paths = start_payload
        .get("call_paths_from_owner")
        .and_then(serde_json::Value::as_array)
        .expect("call_paths_from_owner array");
    let reach = start_payload
        .get("call_reach")
        .and_then(serde_json::Value::as_object)
        .expect("call_reach object");
    let reach_paths = reach
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach paths array");
    let reach_callees = reach
        .get("callees")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach callees array");
    let reach_direct_callees = reach
        .get("direct_callees")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach direct_callees array");
    let reach_direct_call_sites = reach
        .get("direct_call_sites")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach direct_call_sites array");
    let reach_boundary_call_sites = reach
        .get("boundary_call_sites")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach boundary_call_sites array");
    let reach_boundary_edges = reach
        .get("boundary_edges")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach boundary_edges array");
    let reach_public_callees = reach
        .get("public_callees")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach public_callees array");
    let reach_source_files = reach
        .get("source_files")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach source_files array");

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    // Source-oracle chain:
    //   axum-core/src/ext_traits/request.rs:268
    //     `RequestExt::extract` calls `self.extract_with_state(&())`.
    //   axum-core/src/ext_traits/request.rs:279
    //     `RequestExt::extract_with_state` calls `E::from_request(self, state)`.
    //   axum-core/src/extract/mod.rs:85
    //     defines the `FromRequest::from_request` trait method binding.
    // Exact lookup should expose the same call-path fields as request context
    // and code-item edges, so an exact-coordinate lookup can answer navigation
    // questions without requiring a second edge-tool call.
    assert_two_hop_call_path(
        outgoing_paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "code_item_lookup outgoing paths",
    );
    assert_two_hop_call_path(
        reach_paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "code_item_lookup reach paths",
    );
    assert_impact_node(
        reach_callees,
        fixture.intermediate,
        "extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "code_item_lookup reach callees",
    );
    assert_impact_node(
        reach_callees,
        fixture.target,
        "from_request",
        "axum-core/src/extract/mod.rs",
        "code_item_lookup reach callees",
    );
    assert_impact_node(
        reach_direct_callees,
        fixture.intermediate,
        "extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "code_item_lookup reach direct callees",
    );
    let reach_direct_sites = reach_direct_call_sites
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed reach direct callsite rows");
    assert_eq!(
        reach_direct_sites.len(),
        1,
        "code_item_lookup reach should surface the exact resolved direct callsite row: {reach_direct_sites:#?}"
    );
    assert!(
        reach_direct_sites.iter().any(|call| {
            call.owner_id == fixture.start
                && call.kind == CallSiteKind::Method
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Method { name, .. } if name == "extract_with_state"
                )
                && call.arg_count == Some(1)
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == fixture.intermediate)
        }),
        "code_item_lookup reach should include the extract_with_state callsite row: {reach_direct_sites:#?}"
    );
    assert!(
        reach_boundary_call_sites.is_empty(),
        "code_item_lookup reach should not mark the same-module extract -> extract_with_state call as a module-boundary row: {reach_boundary_call_sites:#?}"
    );
    let boundary_edges = reach_boundary_edges
        .iter()
        .map(|edge| serde_json::from_value::<CallPathEdgeInfo>(edge.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed reach boundary edges");
    assert_eq!(
        boundary_edges.len(),
        1,
        "code_item_lookup reach should expose the transitive cross-module FromRequest edge: {boundary_edges:#?}"
    );
    let boundary_edge = &boundary_edges[0];
    assert_eq!(boundary_edge.caller_id, fixture.intermediate);
    assert_eq!(boundary_edge.callee_id, fixture.target);
    assert_eq!(boundary_edge.source_kind, CallSiteKind::Path);
    assert_eq!(boundary_edge.relation, CallTargetKind::AssociatedFunction);
    assert_impact_node(
        reach_public_callees,
        fixture.target,
        "from_request",
        "axum-core/src/extract/mod.rs",
        "code_item_lookup reach public callees",
    );
    assert_source_file(
        reach_source_files,
        "axum-core/src/ext_traits/request.rs",
        "code_item_lookup reach source files",
    );
    assert_source_file(
        reach_source_files,
        "axum-core/src/extract/mod.rs",
        "code_item_lookup reach source files",
    );
    let target_id = fixture.target.to_string();
    let outgoing_path = outgoing_paths
        .iter()
        .find(|path| {
            path.get("end_id").and_then(serde_json::Value::as_str) == Some(target_id.as_str())
                && path.get("depth").and_then(serde_json::Value::as_u64) == Some(2)
        })
        .unwrap_or_else(|| panic!("missing outgoing two-hop path: {outgoing_paths:#?}"));
    let outgoing_nodes = outgoing_path
        .get("nodes")
        .and_then(serde_json::Value::as_array)
        .expect("outgoing path nodes");
    assert_call_path_node(
        outgoing_nodes,
        fixture.start,
        "::extract",
        "axum-core/src/ext_traits/request.rs",
        "code_item_lookup outgoing paths",
    );
    assert_call_path_node(
        outgoing_nodes,
        fixture.intermediate,
        "::extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "code_item_lookup outgoing paths",
    );
    assert_call_path_node(
        outgoing_nodes,
        fixture.target,
        "::from_request",
        "axum-core/src/extract/mod.rs",
        "code_item_lookup outgoing paths",
    );
    let start_ui = start_result.ui_payload.as_ref().expect("start UI payload");
    assert!(
        ui_field(start_ui, "call_paths_from_owner")
            .parse::<usize>()
            .expect("outgoing path count")
            >= 1,
        "code_item_lookup should surface outgoing call-path carrier counts"
    );
    assert!(
        ui_field(start_ui, "reach_callees")
            .parse::<usize>()
            .expect("reach callee count")
            >= 2,
        "code_item_lookup should surface eventual reach callee counts"
    );
    assert!(
        ui_field(start_ui, "reach_direct_callees")
            .parse::<usize>()
            .expect("direct reach callee count")
            >= 1,
        "code_item_lookup should surface direct reach callee counts"
    );
    assert_eq!(
        ui_field(start_ui, "reach_direct_call_sites"),
        reach_direct_sites.len().to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_boundary_call_sites"),
        reach_boundary_call_sites.len().to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_boundary_edges"),
        boundary_edges.len().to_string()
    );
    assert!(
        ui_field(start_ui, "reach_public_callees")
            .parse::<usize>()
            .expect("public reach callee count")
            >= 1,
        "code_item_lookup should surface public reach callee counts"
    );
    assert_eq!(
        ui_field(start_ui, "reach_source_files"),
        reach_source_files.len().to_string()
    );

    let boundary_params = LookupParams {
        item_name: Cow::Borrowed("extract_with_state"),
        file_path: Cow::Owned(fixture.start_file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(fixture.start_module_path_arg()),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("Request")),
    };
    let boundary_result = CodeItemLookup::execute(
        boundary_params,
        fixture.ctx("axum-request-extract-boundary-lookup"),
    )
    .await
    .expect("RequestExt::extract_with_state lookup");
    let boundary_payload: serde_json::Value = serde_json::from_str(&boundary_result.content)
        .expect("deserialize boundary ConciseContext");
    let boundary_reach = boundary_payload
        .get("call_reach")
        .and_then(serde_json::Value::as_object)
        .expect("boundary call_reach object");
    let boundary_calls = boundary_reach
        .get("boundary_call_sites")
        .and_then(serde_json::Value::as_array)
        .expect("boundary call_reach boundary_call_sites array")
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed boundary callsite rows");
    assert_eq!(
        boundary_calls.len(),
        1,
        "code_item_lookup should surface the exact cross-module E::from_request callsite row: {boundary_calls:#?}"
    );
    assert!(
        boundary_calls.iter().any(|call| {
            call.owner_id == fixture.intermediate
                && call.kind == CallSiteKind::Path
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path } if path == &vec![
                        "E".to_string(),
                        "from_request".to_string()
                    ]
                )
                && call.arg_count == Some(2)
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == fixture.target)
        }),
        "code_item_lookup should include the cross-module FromRequest boundary row: {boundary_calls:#?}"
    );
    let boundary_ui = boundary_result
        .ui_payload
        .as_ref()
        .expect("boundary UI payload");
    assert_eq!(
        ui_field(boundary_ui, "reach_boundary_call_sites"),
        boundary_calls.len().to_string()
    );

    let target_params = LookupParams {
        item_name: Cow::Borrowed("from_request"),
        file_path: Cow::Owned(fixture.target_file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(fixture.target_module_path_arg()),
        owner_trait: Some(Cow::Borrowed("FromRequest")),
        owner_type: None,
    };
    let target_result =
        CodeItemLookup::execute(target_params, fixture.ctx("axum-from-request-lookup-paths"))
            .await
            .expect("FromRequest::from_request lookup");
    let target_payload: serde_json::Value =
        serde_json::from_str(&target_result.content).expect("deserialize target ConciseContext");
    let incoming_paths = target_payload
        .get("call_paths_to_target")
        .and_then(serde_json::Value::as_array)
        .expect("call_paths_to_target array");
    let impact = target_payload
        .get("call_impact")
        .and_then(serde_json::Value::as_object)
        .expect("call_impact object");
    let impact_paths = impact
        .get("paths")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact paths array");
    let impact_callers = impact
        .get("callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact callers array");
    let impact_direct_callers = impact
        .get("direct_callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact direct_callers array");
    let impact_direct_call_sites = impact
        .get("direct_call_sites")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact direct_call_sites array");
    let impact_buckets = impact
        .get("callsite_buckets")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact callsite_buckets array");
    let impact_public_callers = impact
        .get("public_callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact public_callers array");
    let impact_source_files = impact
        .get("source_files")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact source_files array");
    assert_two_hop_call_path(
        incoming_paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "code_item_lookup incoming paths",
    );
    assert_two_hop_call_path(
        impact_paths,
        fixture.start,
        fixture.intermediate,
        fixture.target,
        "code_item_lookup impact paths",
    );
    assert_impact_node(
        impact_callers,
        fixture.start,
        "extract",
        "axum-core/src/ext_traits/request.rs",
        "code_item_lookup impact callers",
    );
    assert_impact_node(
        impact_callers,
        fixture.intermediate,
        "extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "code_item_lookup impact callers",
    );
    assert_impact_node(
        impact_direct_callers,
        fixture.intermediate,
        "extract_with_state",
        "axum-core/src/ext_traits/request.rs",
        "code_item_lookup impact direct callers",
    );
    let direct_call_sites = impact_direct_call_sites
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed impact direct callsite rows");
    assert_eq!(
        direct_call_sites.len(),
        2,
        "code_item_lookup impact should surface both direct target-centered callsite rows: {direct_call_sites:#?}"
    );
    assert!(
        direct_call_sites.iter().any(|call| {
            call.owner_id == fixture.intermediate
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path }
                        if path.iter().map(String::as_str).eq(["E", "from_request"])
                )
                && call.arg_count == Some(2)
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == fixture.target)
        }),
        "code_item_lookup impact should include the E::from_request callsite row: {direct_call_sites:#?}"
    );
    let callsite_buckets = impact_buckets
        .iter()
        .map(|bucket| serde_json::from_value::<CallSiteBucketInfo>(bucket.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed impact callsite bucket rows");
    assert!(
        callsite_buckets.iter().any(|bucket| {
            bucket.kind == CallSiteKind::Path
                && bucket.relation == CallTargetKind::AssociatedFunction
                && bucket.count == 2
        }),
        "code_item_lookup impact should summarize the direct path/associated-function callsites: {callsite_buckets:#?}"
    );
    assert!(
        impact_public_callers.is_empty(),
        "direct stored-public impact bucket should remain empty for inherited method callers: {impact_public_callers:#?}"
    );
    assert_source_file(
        impact_source_files,
        "axum-core/src/ext_traits/request.rs",
        "code_item_lookup impact source files",
    );
    assert_source_file(
        impact_source_files,
        "axum-core/src/extract/mod.rs",
        "code_item_lookup impact source files",
    );
    let target_ui = target_result
        .ui_payload
        .as_ref()
        .expect("target UI payload");
    assert!(
        ui_field(target_ui, "call_paths_to_target")
            .parse::<usize>()
            .expect("incoming path count")
            >= 1,
        "code_item_lookup should surface incoming call-path carrier counts"
    );
    assert!(
        ui_field(target_ui, "impact_callers")
            .parse::<usize>()
            .expect("impact caller count")
            >= 2,
        "code_item_lookup should surface impact caller counts"
    );
    assert!(
        ui_field(target_ui, "impact_direct_callers")
            .parse::<usize>()
            .expect("direct impact caller count")
            >= 2,
        "code_item_lookup should surface all direct impact caller counts"
    );
    assert_eq!(
        ui_field(target_ui, "impact_direct_call_sites"),
        direct_call_sites.len().to_string()
    );
    assert_eq!(
        ui_field(target_ui, "impact_callsite_buckets"),
        callsite_buckets.len().to_string()
    );
    assert_eq!(ui_field(target_ui, "impact_public_callers"), "0");
    assert_eq!(
        ui_field(target_ui, "impact_source_files"),
        impact_source_files.len().to_string()
    );
}

#[tokio::test]
async fn code_item_lookup_surfaces_fail_closed_proc_macro_impact_gap() {
    let fixture = AxumExpandWithToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("expand_with"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("axum-expand-with-impact-lookup"))
        .await
        .expect("expand_with lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Dead code detection:
    //   "Is this function reachable from any binary, test, macro entrypoint,
    //   or exported API?"
    //
    // Source oracle:
    //   axum-macros/src/lib.rs:377,426,665,715 call `expand_with(...)` from
    //   public proc-macro entrypoints.
    // Current contract: proc-macro item bodies are not visited for structural
    // call-site extraction yet, so exact lookup must report the impact summary
    // as fail-closed instead of fabricating public callers.
    let impact = payload
        .get("call_impact")
        .and_then(serde_json::Value::as_object)
        .expect("call_impact object");
    let target_id = fixture.target.to_string();
    assert_eq!(
        impact
            .get("target")
            .and_then(|target| target.get("id"))
            .and_then(serde_json::Value::as_str),
        Some(target_id.as_str())
    );
    for field in [
        "paths",
        "callers",
        "direct_callers",
        "direct_call_sites",
        "public_callers",
    ] {
        let rows = impact
            .get(field)
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("call_impact {field} array: {impact:#?}"));
        assert!(
            rows.is_empty(),
            "expand_with impact {field} should remain empty until proc-macro bodies are modeled: {rows:#?}"
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "impact_callers"), "0");
    assert_eq!(ui_field(ui, "impact_direct_callers"), "0");
    assert_eq!(ui_field(ui, "impact_direct_call_sites"), "0");
    assert_eq!(ui_field(ui, "impact_public_callers"), "0");
}

#[tokio::test]
async fn code_item_lookup_returns_incoming_callers_for_call_graph_target() {
    let fixture = CallGraphToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("local_target"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
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
        owner_trait: None,
        owner_type: None,
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
    let impact = payload
        .get("call_impact")
        .and_then(serde_json::Value::as_object)
        .expect("call_impact object");
    let impact_callers = impact
        .get("callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact callers array");
    let impact_test_callers = impact
        .get("test_callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact test_callers array");
    let impact_non_test_callers = impact
        .get("non_test_callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact non_test_callers array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/body.rs:110 and :116 call `Self::empty()`.
    //   axum-core/src/response/into_response.rs response conversion rows call
    //   `Body::empty()`.
    // Expected tool traversal: exact lookup of the callee method exposes all
    // four current incoming caller-site edges and their projected proof rows.
    assert_body_empty_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_lookup",
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_lookup",
        );
    }
    assert_eq!(
        impact_test_callers.len() + impact_non_test_callers.len(),
        impact_callers.len(),
        "code_item_lookup impact test/non-test buckets should partition eventual callers: {impact:#?}"
    );
    assert!(
        !impact_non_test_callers.is_empty(),
        "Body::empty resolved impact subset should expose non-test callers: {impact:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "4");
    assert_eq!(
        ui_field(ui, "impact_test_callers"),
        impact_test_callers.len().to_string()
    );
    assert_eq!(
        ui_field(ui, "impact_non_test_callers"),
        impact_non_test_callers.len().to_string()
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_lookup should surface real-corpus Body::empty proof rows"
    );
}

#[tokio::test]
async fn code_item_lookup_returns_real_corpus_parse_attrs_callers() {
    let fixture = AxumParseAttrsToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = LookupParams {
        item_name: Cow::Borrowed("parse_attrs"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(module_path.clone()),
        owner_trait: None,
        owner_type: None,
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("axum-parse-attrs-lookup"))
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
    //   axum-macros/src/attr_parsing.rs:59 defines `parse_attrs`.
    //   axum-macros/src/typed_path.rs:23 calls
    //   `crate::attr_parsing::parse_attrs(...)`.
    //   from_ref.rs:30 and from_request/mod.rs:{112,196,598,727,892,908}
    //   call imported `parse_attrs(...)`.
    // Expected tool traversal: exact lookup of the callee function exposes all
    // eight incoming caller-site edges and their projected proof rows.
    assert_parse_attrs_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_lookup",
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_lookup",
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "8");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_lookup should surface real-corpus parse_attrs proof rows"
    );

    let turbofish_params = LookupParams {
        item_name: Cow::Borrowed("parse_parenthesized_attribute"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
    };
    let turbofish_result = CodeItemLookup::execute(
        turbofish_params,
        fixture.ctx("axum-type-name-turbofish-lookup"),
    )
    .await
    .expect("parse_parenthesized_attribute lookup");
    let turbofish_payload: serde_json::Value =
        serde_json::from_str(&turbofish_result.content).expect("deserialize turbofish context");
    let turbofish_context = turbofish_payload
        .get("call_context")
        .and_then(serde_json::Value::as_array)
        .expect("turbofish call_context array")
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed turbofish call_context rows");
    let type_name = turbofish_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path }
                        if path.iter().map(String::as_str).eq(["std", "any", "type_name"])
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "code_item_lookup should expose std::any::type_name::<K> row: {turbofish_context:#?}"
            )
        });

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // API understanding:
    //   "What argument shapes do existing callers pass?"
    //
    // Source oracle:
    //   axum-macros/src/attr_parsing.rs:22 calls
    //   `std::any::type_name::<K>()`.
    //
    // Expected tool payload: exact lookup of the owner function exposes the
    // external targetless call-site row with one turbofish generic argument.
    assert_eq!(type_name.status, CallStatusKind::External);
    assert_eq!(type_name.generic_arg_count, Some(1));
    assert!(
        type_name.targets.is_empty(),
        "external type_name::<K> call should remain targetless: {type_name:#?}"
    );
}

#[tokio::test]
async fn code_item_lookup_returns_real_corpus_json_from_bytes_callers() {
    let fixture = AxumJsonFromBytesToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = LookupParams {
        item_name: Cow::Borrowed("from_bytes"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("axum-json-from-bytes-lookup"))
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
    let reach = payload
        .get("call_reach")
        .and_then(serde_json::Value::as_object)
        .expect("call_reach object");
    let frontier = reach
        .get("frontier_calls")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach frontier_calls array");
    let external_frontier = reach
        .get("external_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach external_frontier_calls array");
    let reach_source_files = reach
        .get("source_files")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach source_files array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:112 and :128 call `Self::from_bytes(&bytes)`.
    //   axum/src/json.rs:184 calls
    //     `serde_json::Deserializer::from_slice(bytes)`.
    // Expected tool traversal: exact lookup of the callee method exposes both
    // trait-impl `Self::from_bytes` caller-site edges and proof rows. Its
    // owner reach summary also surfaces the serde_json dependency-root call as
    // an external frontier row, not a fabricated local edge.
    assert_json_from_bytes_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_lookup",
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_lookup",
        );
    }
    let frontier_calls = frontier
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed frontier call rows");
    let serde_frontier = frontier_calls
        .iter()
        .find(|call| {
            call.owner_id == fixture.target
                && call.kind == CallSiteKind::Path
                && call.status == CallStatusKind::External
                && call.targets.is_empty()
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path }
                        if path
                            .iter()
                            .map(String::as_str)
                            .eq(["serde_json", "Deserializer", "from_slice"])
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "code_item_lookup should surface serde_json external frontier row: {frontier_calls:#?}"
            )
    });
    assert_eq!(serde_frontier.owner_id, fixture.target);
    let external_frontier_calls = external_frontier
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed external frontier call rows");
    let external_serde_frontier = external_frontier_calls
        .iter()
        .find(|call| {
            call.owner_id == fixture.target
                && call.kind == CallSiteKind::Path
                && call.status == CallStatusKind::External
                && call.targets.is_empty()
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path }
                        if path
                            .iter()
                            .map(String::as_str)
                            .eq(["serde_json", "Deserializer", "from_slice"])
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "code_item_lookup should surface serde_json in external frontier rows: {external_frontier_calls:#?}"
            )
        });
    assert_eq!(external_serde_frontier.owner_id, fixture.target);
    assert_source_file(
        reach_source_files,
        "axum/src/json.rs",
        "code_item_lookup Json::from_bytes reach source files",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "2");
    assert!(
        ui_field(ui, "reach_frontier_calls")
            .parse::<usize>()
            .expect("reach frontier count")
            >= 1,
        "code_item_lookup should surface reach frontier call counts"
    );
    assert!(
        ui_field(ui, "reach_external_frontier_calls")
            .parse::<usize>()
            .expect("external frontier count")
            >= 1,
        "code_item_lookup should surface external reach frontier call counts"
    );
    assert_eq!(
        ui_field(ui, "reach_source_files"),
        reach_source_files.len().to_string()
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_lookup should surface real-corpus Json::from_bytes proof rows"
    );
}

#[tokio::test]
async fn code_item_lookup_returns_real_corpus_boxed_into_route_constructor_callers() {
    let fixture = AxumBoxedIntoRouteToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = LookupParams {
        item_name: Cow::Borrowed("BoxedIntoRoute"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("struct"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("axum-boxed-into-route-lookup"))
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
    //   axum/src/boxed.rs:12 defines `BoxedIntoRoute<S, E>(...)`.
    //   axum/src/boxed.rs:38 calls `BoxedIntoRoute(Box::new(...))`.
    // Expected tool traversal: exact lookup of the tuple-struct target exposes
    // the one incoming constructor edge and its projected proof row.
    assert_boxed_into_route_incoming_context(
        call_context,
        &fixture.caller,
        fixture.target,
        "code_item_lookup",
    );
    assert_target_proof(
        proof_context,
        fixture.caller.owner,
        fixture.target,
        "code_item_lookup",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "1");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= 1,
        "code_item_lookup should surface real-corpus BoxedIntoRoute proof rows"
    );
}

#[tokio::test]
async fn code_item_lookup_returns_real_corpus_run_ui_tests_callers() {
    let fixture = AxumRunUiTestsToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = LookupParams {
        item_name: Cow::Borrowed("run_ui_tests"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("axum-run-ui-tests-lookup"))
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
    //   axum-macros/src/lib.rs:797 defines `run_ui_tests`.
    //   debug_handler.rs:885,890; typed_path.rs:443; from_ref.rs:104;
    //   from_request/mod.rs:1050 call `crate::run_ui_tests(...)`.
    // Expected tool traversal: exact lookup of the callee function exposes all
    // five current incoming caller-site edges and their projected proof rows.
    assert_run_ui_tests_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_lookup",
    );
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_lookup",
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "5");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_lookup should surface real-corpus run_ui_tests proof rows"
    );
}

#[tokio::test]
async fn code_item_lookup_disambiguates_real_corpus_handler_call_by_owner_trait() {
    let fixture = AxumHandlerCallToolFixture::new().await;
    let module_path = fixture.module_path_arg();

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/handler/mod.rs:153 declares trait method `Handler::call`.
    //   axum/src/handler/service.rs:171 calls
    //   `Handler::call(handler, req, self.state.clone())`.
    // Expected exact-tool behavior: unqualified `call` remains ambiguous in
    // this file/module, while owner_trait="Handler" selects the trait method
    // and surfaces the one incoming caller edge plus projected proof row.
    let ambiguous = CodeItemLookup::execute(
        LookupParams {
            item_name: Cow::Borrowed("call"),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(module_path.clone()),
            owner_trait: None,
            owner_type: None,
        },
        fixture.ctx("axum-handler-call-ambiguous-lookup"),
    )
    .await
    .expect_err("unqualified Handler::call lookup should remain ambiguous");
    let ambiguous_message = ambiguous.to_string();
    assert!(
        ambiguous_message.contains("Multiple items matched `call`"),
        "unqualified call lookup should preserve the strict ambiguity error: {ambiguous_message}"
    );

    let result = CodeItemLookup::execute(
        LookupParams {
            item_name: Cow::Borrowed("call"),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(module_path),
            owner_trait: Some(Cow::Borrowed("Handler")),
            owner_type: None,
        },
        fixture.ctx("axum-handler-call-lookup"),
    )
    .await
    .expect("owner-qualified Handler::call lookup");
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

    assert_handler_call_incoming_context(
        call_context,
        &fixture.caller,
        fixture.target,
        "code_item_lookup",
    );
    assert_target_proof(
        proof_context,
        fixture.caller.owner,
        fixture.target,
        "code_item_lookup",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "1");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof context count")
            >= 1,
        "code_item_lookup should surface real-corpus Handler::call proof rows"
    );
}

fn assert_impact_node(
    nodes: &[serde_json::Value],
    id: uuid::Uuid,
    name: &str,
    file_suffix: &str,
    label: &str,
) {
    let id = id.to_string();
    assert!(
        nodes.iter().any(|node| {
            node.get("id").and_then(serde_json::Value::as_str) == Some(id.as_str())
                && node.get("name").and_then(serde_json::Value::as_str) == Some(name)
                && node
                    .get("file_path")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|path| path.ends_with(file_suffix))
        }),
        "{label} should include impact node {id} named {name:?} in {file_suffix:?}: {nodes:#?}"
    );
}

fn assert_source_file(files: &[serde_json::Value], suffix: &str, label: &str) {
    assert!(
        files
            .iter()
            .any(|file| file.as_str().is_some_and(|path| path.ends_with(suffix))),
        "{label} should include source file ending with {suffix:?}: {files:#?}"
    );
}
