use std::borrow::Cow;

use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallPathEdgeInfo, CallSiteBucketInfo, CallSiteKind,
    CallStatusKind, CallTargetKind, ModuleBoundaryEdgeInfo, ProofContextInfo,
};
use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
};

use crate::call_graph_tool_support::{
    AsyncFutureToolFixture, AxumAwaitReceiverToolFixture, AxumErrorHandlingTraitsToolFixture,
    AxumExpandWithToolFixture, AxumRequestExtractPathToolFixture, AxumTaskSpawnEffectToolFixture,
    CallGraphToolFixture, ResultCallbackFixture, ReturnedClosureToolFixture,
    assert_await_result_unwrap_context, assert_await_result_unwrap_proof, assert_call_path_node,
    assert_forwarded_async_future_awaited_site, assert_forwarded_async_future_execution_flow,
    assert_forwarded_async_future_flow, assert_forwarded_returned_closure_binding_flow,
    assert_process_invariant_findings, assert_resolved_callable_param_proof,
    assert_resolved_dynamic_context, assert_resolved_method_target_context,
    assert_resolved_path_context, assert_result_callback_binding_payload, assert_target_proof,
    assert_task_spawn_effects, assert_task_spawn_policy_violation, assert_two_hop_call_path,
    ui_field,
};

#[tokio::test]
async fn code_item_lookup_returns_awaited_async_closure_future_tuple_field_context() {
    assert_awaited_async_closure_future_lookup(
        AsyncFutureToolFixture::tuple_field().await,
        "awaited async closure future tuple field",
        "async-future-tuple-lookup",
    )
    .await;
}

#[tokio::test]
async fn code_item_lookup_returns_awaited_async_closure_future_named_field_context() {
    assert_awaited_async_closure_future_lookup(
        AsyncFutureToolFixture::named_field().await,
        "awaited async closure future named field",
        "async-future-named-lookup",
    )
    .await;
}

#[tokio::test]
async fn code_item_lookup_returns_awaited_async_closure_future_named_field_alias_context() {
    assert_awaited_async_closure_future_lookup(
        AsyncFutureToolFixture::named_field_alias().await,
        "awaited async closure future named field alias",
        "async-future-named-alias-lookup",
    )
    .await;
}

#[tokio::test]
async fn code_item_lookup_returns_awaited_async_closure_future_indexed_array_context() {
    assert_awaited_async_closure_future_lookup(
        AsyncFutureToolFixture::indexed_array().await,
        "awaited async closure future indexed array",
        "async-future-indexed-array-lookup",
    )
    .await;
}

#[tokio::test]
async fn code_item_lookup_returns_awaited_returned_async_closure_context() {
    assert_awaited_returned_async_closure_lookup(
        AsyncFutureToolFixture::returned_async_closure().await,
        "awaited returned async closure",
        "returned-async-closure-lookup",
    )
    .await;
}

#[tokio::test]
async fn code_item_lookup_returns_stored_returned_async_closure_context() {
    assert_awaited_returned_async_closure_lookup(
        AsyncFutureToolFixture::stored_returned_async_closure().await,
        "stored returned async closure future",
        "stored-returned-async-closure-lookup",
    )
    .await;
}

#[tokio::test]
async fn code_item_lookup_returns_forwarded_returned_closure_context() {
    assert_forwarded_returned_closure_lookup(
        ReturnedClosureToolFixture::forwarded_returned_closure().await,
        "forwarded returned closure",
        "forwarded-returned-closure-lookup",
    )
    .await;
}

#[tokio::test]
async fn code_item_lookup_returns_forwarded_async_future_awaited_site() {
    let fixture = CallGraphToolFixture::new().await;
    for (item_name, label, ctx_name) in [
        (
            "call_forwarded_returned_async_future",
            "forwarded returned async future",
            "forwarded-async-future-lookup",
        ),
        (
            "call_stored_forwarded_returned_async_future_tuple_field",
            "stored forwarded returned async future",
            "stored-forwarded-async-future-lookup",
        ),
        (
            "call_aliased_stored_forwarded_returned_async_future_tuple_field",
            "aliased stored forwarded returned async future",
            "aliased-stored-forwarded-async-future-lookup",
        ),
    ] {
        let params = LookupParams {
            item_name: Cow::Borrowed(item_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx(ctx_name))
            .await
            .expect("forwarded async future lookup should succeed");
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let owner = payload
            .get("id")
            .and_then(serde_json::Value::as_str)
            .expect("payload id")
            .parse()
            .expect("payload id should be a UUID");
        let awaited_sites = payload
            .get("awaited_call_sites")
            .and_then(serde_json::Value::as_array)
            .expect("awaited_call_sites array");
        let returned_flows = payload
            .get("returned_call_binding_flows")
            .and_then(serde_json::Value::as_array)
            .expect("returned_call_binding_flows array");
        let future_flows = payload
            .get("returned_future_flows")
            .and_then(serde_json::Value::as_array)
            .expect("returned_future_flows array");
        let future_execution_flows = payload
            .get("returned_future_execution_flows")
            .and_then(serde_json::Value::as_array)
            .expect("returned_future_execution_flows array");

        // Source oracle:
        //   tests/fixture_crates/fixture_call_graph/src/lib.rs:2401-2407.
        //   Both callers await `make_forwarded_returned_async_future()`,
        //   directly or through `futures.0`, while returned-call binding flow
        //   remains fail-closed.
        assert_forwarded_async_future_awaited_site(awaited_sites, owner, label, "code_item_lookup");
        assert!(
            returned_flows.is_empty(),
            "lookup should not expose returned-call binding flow through the forwarded future boundary for {label}: {returned_flows:#?}"
        );
        assert_forwarded_async_future_flow(future_flows, owner, label, "code_item_lookup");
        assert_forwarded_async_future_execution_flow(
            future_execution_flows,
            owner,
            label,
            "code_item_lookup",
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert_eq!(ui_field(ui, "awaited_call_sites"), "1");
        assert_eq!(ui_field(ui, "returned_call_binding_flows"), "0");
        assert_eq!(ui_field(ui, "returned_future_flows"), "1");
        assert_eq!(ui_field(ui, "returned_future_execution_flows"), "1");
    }
}

async fn assert_awaited_async_closure_future_lookup(
    fixture: AsyncFutureToolFixture,
    label: &'static str,
    ctx_name: &'static str,
) {
    let params = LookupParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx(ctx_name))
        .await
        .unwrap_or_else(|err| panic!("{label} lookup should succeed: {err}"));
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

    // Fixture sources:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     `let futures = (closure(),); futures.0.await;`
    //     `let holder = AsyncFutureHolder { future: closure() };
    //      holder.future.await;`
    //     `let alias = holder.future; alias.await;`
    //     `let futures = [closure()]; futures[0].await;`
    //   prove the stored future is polled without adding returned-future or
    //   arbitrary future value-flow semantics.
    let callee = CallCalleeInfo::Path {
        path: vec!["closure".to_string()],
    };
    assert_resolved_path_context(
        call_context,
        fixture.owner,
        &callee,
        fixture.closure,
        CallTargetKind::Closure,
        label,
        "code_item_lookup",
    );
    assert_target_proof(proof_context, fixture.owner, fixture.closure, label);

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_lookup should surface the {label} closure call"
    );
}

async fn assert_awaited_returned_async_closure_lookup(
    fixture: AsyncFutureToolFixture,
    label: &'static str,
    ctx_name: &'static str,
) {
    let params = LookupParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx(ctx_name))
        .await
        .unwrap_or_else(|err| panic!("{label} lookup should succeed: {err}"));
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

    // Fixture sources:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:2359-2365
    //     direct `.await` and same-block `let future = ...; future.await`
    //     both poll the returned async closure future, so lookup should expose
    //     the DynamicClosure edge.
    assert_resolved_dynamic_context(
        call_context,
        fixture.owner,
        fixture.closure,
        CallTargetKind::DynamicClosure,
        label,
        "code_item_lookup",
    );
    assert_target_proof(proof_context, fixture.owner, fixture.closure, label);

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_lookup should surface the {label} dynamic closure call"
    );
}

async fn assert_forwarded_returned_closure_lookup(
    fixture: ReturnedClosureToolFixture,
    label: &'static str,
    ctx_name: &'static str,
) {
    let params = LookupParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx(ctx_name))
        .await
        .unwrap_or_else(|err| panic!("{label} lookup should succeed: {err}"));
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
    let returned_flows = payload
        .get("returned_call_binding_flows")
        .and_then(serde_json::Value::as_array)
        .expect("returned_call_binding_flows array");

    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1520
    //   `call_forwarded_returned_closure()` invokes a sync closure returned
    //   through `make_forwarded_returned_closure()` and produced by
    //   `make_target_closure()`.
    assert_resolved_dynamic_context(
        call_context,
        fixture.owner,
        fixture.closure,
        CallTargetKind::DynamicClosure,
        label,
        "code_item_lookup",
    );
    assert_target_proof(proof_context, fixture.owner, fixture.closure, label);
    assert_forwarded_returned_closure_binding_flow(
        returned_flows,
        fixture.owner,
        label,
        "code_item_lookup",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_lookup should surface the {label} dynamic closure call"
    );
    assert_eq!(ui_field(ui, "returned_call_binding_flows"), "1");
}

#[tokio::test]
async fn code_item_lookup_returns_result_method_callback_function_target() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     private `call_single_result_callback(f)` calls
    //     `Ok::<i32, ()>(1).and_then(f)`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     the only local caller passes `local_result_target`.
    let fixture = ResultCallbackFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("result-callback-lookup"))
        .await
        .expect("result callback lookup");
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
    let bindings = payload
        .get("local_bindings")
        .and_then(serde_json::Value::as_array)
        .expect("local_bindings array");
    let edges = payload
        .get("local_binding_edges")
        .and_then(serde_json::Value::as_array)
        .expect("local_binding_edges array");

    let site_id = assert_resolved_method_target_context(
        call_context,
        fixture.owner,
        &fixture.callee(),
        fixture.target,
        CallTargetKind::MethodCallbackFunction,
        Some(0),
        "result method callback",
        "code_item_lookup",
    );
    assert_resolved_callable_param_proof(
        proof_context,
        fixture.owner,
        fixture.target,
        site_id,
        fixture.build_domain,
        "code_item_lookup",
    );
    assert_result_callback_binding_payload(
        bindings,
        edges,
        fixture.owner,
        fixture.target,
        "call_single_result_callback",
        "code_item_lookup",
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_lookup should surface the resolved result callback row"
    );
    assert_eq!(
        ui_field(ui, "proof_context"),
        proof_context.len().to_string()
    );
    assert_eq!(ui_field(ui, "local_bindings"), bindings.len().to_string());
    assert_eq!(ui_field(ui, "local_binding_edges"), edges.len().to_string());
}

#[tokio::test]
async fn code_item_lookup_returns_real_corpus_reachable_effects() {
    let fixture = AxumTaskSpawnEffectToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("deserialize_error_status_codes"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
        allowed_effects: vec![Cow::Borrowed("ffi_boundary")],
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("axum-task-spawn-effect-lookup"))
        .await
        .expect("deserialize_error_status_codes lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let effects = payload
        .get("call_reach_effects")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach_effects array");
    let policy_violations = payload
        .get("call_effect_policy_violations")
        .and_then(serde_json::Value::as_array)
        .expect("call_effect_policy_violations array");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security/performance:
    //   "Can this entrypoint reach a sensitive sink?"
    //   "Is the reachable sink outside the caller's explicit effect policy?"
    //   "Which call chain reaches a task-spawn point?"
    //
    // Source-oracle chain:
    //   axum/src/form.rs:262
    //     `deserialize_error_status_codes` calls `TestClient::new(app)`.
    //   axum/src/test_helpers/test_client.rs:36
    //     `TestClient::new` calls `spawn_service(svc)`.
    //   axum/src/test_helpers/test_client.rs:23
    //     `spawn_service` calls `tokio::spawn(...)`.
    assert_task_spawn_effects(effects, &fixture, "code_item_lookup call_reach_effects");
    assert_task_spawn_policy_violation(
        policy_violations,
        &fixture,
        "code_item_lookup call_effect_policy_violations",
    );
    let owner = fixture.owner.to_string();
    assert_eq!(
        payload.get("id").and_then(serde_json::Value::as_str),
        Some(owner.as_str()),
        "code_item_lookup should resolve the upstream axum test owner: {payload:#?}"
    );
    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "reach_effects"), effects.len().to_string());
    assert_eq!(
        ui_field(ui, "effect_policy_violations"),
        policy_violations.len().to_string()
    );
}

#[tokio::test]
async fn code_item_lookup_uses_stored_effect_policy_when_allowlist_omitted() {
    let fixture = AxumTaskSpawnEffectToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("deserialize_error_status_codes"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
    };

    let result =
        CodeItemLookup::execute(params, fixture.ctx("axum-task-spawn-stored-policy-lookup"))
            .await
            .expect("deserialize_error_status_codes stored-policy lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let policy_violations = payload
        .get("call_effect_policy_violations")
        .and_then(serde_json::Value::as_array)
        .expect("call_effect_policy_violations array");

    // Source oracle:
    //   axum/src/form.rs:262 -> TestClient::new(app)
    //   axum/src/test_helpers/test_client.rs:36 -> spawn_service(svc)
    //   axum/src/test_helpers/test_client.rs:23 -> tokio::spawn(...)
    //
    // The fixture admits a stored owner `effect_policy` for
    // `deserialize_error_status_codes` that allows only `ffi_boundary`.
    assert_task_spawn_policy_violation(
        policy_violations,
        &fixture,
        "code_item_lookup stored-policy call_effect_policy_violations",
    );
}

#[tokio::test]
async fn code_item_lookup_returns_fixture_process_invariant_findings() {
    let fixture = CallGraphToolFixture::new().await;
    let expected = fixture.seed_extern_c_process_invariant();
    let params = LookupParams {
        item_name: Cow::Borrowed("call_extern_c_function"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("extern-c-process-invariant-lookup"))
        .await
        .expect("call_extern_c_function lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let findings = payload
        .get("call_proof_invariant_findings")
        .and_then(serde_json::Value::as_array)
        .expect("call_proof_invariant_findings array");

    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:839-844 declares
    //   foreign function `abs(input)` inside an `unsafe extern "C"` block and
    //   calls `abs(value)` from `call_extern_c_function`.
    //
    // The fixture marks the external `abs(value)` frontier as an
    // `operating_system_process_create` effect. The exact lookup payload should
    // expose the blocked detached-process invariant without inventing a local
    // target for `abs`.
    assert_process_invariant_findings(
        findings,
        &expected,
        "code_item_lookup call_proof_invariant_findings",
    );
    let owner = expected.owner.to_string();
    assert_eq!(
        payload.get("id").and_then(serde_json::Value::as_str),
        Some(owner.as_str()),
        "code_item_lookup should resolve the fixture extern C owner: {payload:#?}"
    );
    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(
        ui_field(ui, "proof_invariant_findings"),
        findings.len().to_string()
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
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
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
    let external_frontier = reach
        .get("external_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach external_frontier_calls array");

    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Source chain:
    //   axum/src/serve/listener.rs:142 owns `ConnLimiter<T>::accept`.
    //   axum/src/serve/listener.rs:143 calls
    //   `self.sem.clone().acquire_owned().await.unwrap()`.
    // The exact lookup tool should expose the DB/RAG-pinned targetless
    // `AwaitMethodCallResult(acquire_owned).unwrap` row without inventing an
    // outgoing target edge.
    let site_id =
        assert_await_result_unwrap_context(call_context, fixture.owner, "code_item_lookup");
    assert_await_result_unwrap_proof(proof_context, fixture.owner, site_id, "code_item_lookup");
    let external_calls = external_frontier
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed external frontier rows");
    let external = external_calls
        .iter()
        .find(|call| call.site_id == site_id)
        .unwrap_or_else(|| {
            panic!(
                "code_item_lookup should expose AwaitMethodCallResult unwrap in external frontier rows: {external_calls:#?}"
            )
        });
    assert_eq!(external.owner_id, fixture.owner);
    assert_eq!(external.status, CallStatusKind::External);
    assert!(
        external.targets.is_empty(),
        "external frontier call should remain targetless: {external:#?}"
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
        ui_field(ui, "reach_external_frontier_calls"),
        external_calls.len().to_string()
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= 2,
        "code_item_lookup should surface targetless AwaitMethodCallResult proof rows"
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
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
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
    let reach_source_crates = reach
        .get("source_crates")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach source_crates array");
    let reach_source_modules = reach
        .get("source_modules")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach source_modules array");
    let module_boundary_edges = start_payload
        .get("module_boundary_edges")
        .and_then(serde_json::Value::as_array)
        .expect("module_boundary_edges array");

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
    let reach_owner = reach
        .get("owner")
        .and_then(serde_json::Value::as_object)
        .expect("call_reach owner object");
    assert_eq!(
        module_path_field(reach_owner, "code_item_lookup reach owner"),
        vec!["crate", "ext_traits", "request"]
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
    let module_edges = module_boundary_edges
        .iter()
        .map(|edge| serde_json::from_value::<ModuleBoundaryEdgeInfo>(edge.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed module boundary rows");
    assert_eq!(
        module_edges.len(),
        1,
        "code_item_lookup should expose the enriched module-boundary row for architecture-review questions: {module_edges:#?}"
    );
    let module_edge = &module_edges[0];
    assert_eq!(module_edge.edge.caller_id, fixture.intermediate);
    assert_eq!(module_edge.edge.callee_id, fixture.target);
    assert_eq!(module_edge.edge.source_kind, CallSiteKind::Path);
    assert_eq!(
        module_edge.edge.relation,
        CallTargetKind::AssociatedFunction
    );
    assert_eq!(module_edge.caller.id, fixture.intermediate);
    assert_eq!(module_edge.caller.name, "extract_with_state");
    assert_eq!(
        module_edge.caller.module_path,
        vec![
            "crate".to_string(),
            "ext_traits".to_string(),
            "request".to_string()
        ]
    );
    assert_eq!(module_edge.callee.id, fixture.target);
    assert_eq!(module_edge.callee.name, "from_request");
    assert_eq!(
        module_edge.callee.module_path,
        vec!["crate".to_string(), "extract".to_string()]
    );
    assert_eq!(module_edge.site.owner_id, fixture.intermediate);
    assert_eq!(module_edge.site.kind, CallSiteKind::Path);
    assert_eq!(module_edge.site.status, CallStatusKind::Resolved);
    assert_eq!(module_edge.site.arg_count, Some(2));
    assert!(
        matches!(
            &module_edge.site.callee,
            CallCalleeInfo::Path { path } if path == &vec!["E".to_string(), "from_request".to_string()]
        ),
        "code_item_lookup should preserve the E::from_request boundary callsite: {module_edge:#?}"
    );
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
    assert_source_crate(
        reach_source_crates,
        "axum-core",
        "code_item_lookup reach source crates",
    );
    assert_source_module(
        reach_source_modules,
        &["crate", "ext_traits", "request"],
        "code_item_lookup reach source modules",
    );
    assert_source_module(
        reach_source_modules,
        &["crate", "extract"],
        "code_item_lookup reach source modules",
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
    assert_eq!(
        ui_field(start_ui, "module_boundary_edges"),
        module_edges.len().to_string()
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
    assert_eq!(
        ui_field(start_ui, "reach_source_crates"),
        reach_source_crates.len().to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_source_cfgs"),
        reach
            .get("source_cfgs")
            .and_then(serde_json::Value::as_array)
            .expect("call_reach source_cfgs array")
            .len()
            .to_string()
    );
    assert_eq!(
        ui_field(start_ui, "reach_source_modules"),
        reach_source_modules.len().to_string()
    );

    let boundary_params = LookupParams {
        item_name: Cow::Borrowed("extract_with_state"),
        file_path: Cow::Owned(fixture.start_file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(fixture.start_module_path_arg()),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("Request")),
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
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
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
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
    let impact_target = impact
        .get("target")
        .and_then(serde_json::Value::as_object)
        .expect("call_impact target object");
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
    let impact_source_modules = impact
        .get("source_modules")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact source_modules array");
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
    let start_id = fixture.start.to_string();
    let start_caller = impact_callers
        .iter()
        .find(|caller| {
            caller.get("id").and_then(serde_json::Value::as_str) == Some(start_id.as_str())
        })
        .and_then(serde_json::Value::as_object)
        .unwrap_or_else(|| panic!("impact callers should include RequestExt::extract"));
    assert_eq!(
        module_path_field(start_caller, "code_item_lookup impact caller"),
        vec!["crate", "ext_traits", "request"]
    );
    assert_eq!(
        module_path_field(impact_target, "code_item_lookup impact target"),
        vec!["crate", "extract"]
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
        34,
        "code_item_lookup impact should surface all direct target-centered callsite rows, including generated handler and tuple extractor rows: {direct_call_sites:#?}"
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
                && bucket.count == 34
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
    assert_source_module(
        impact_source_modules,
        &["crate", "ext_traits", "request"],
        "code_item_lookup impact source modules",
    );
    assert_source_module(
        impact_source_modules,
        &["crate", "extract"],
        "code_item_lookup impact source modules",
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
    assert_eq!(
        ui_field(target_ui, "impact_source_modules"),
        impact_source_modules.len().to_string()
    );
}

#[tokio::test]
async fn code_item_lookup_surfaces_proc_macro_impact_callers() {
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
    // Current contract: proc-macro item bodies are modeled as Macro body owners,
    // so exact lookup must surface the four one-hop public macro callers.
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
    let expected_callers = [
        "derive_from_request",
        "derive_from_request_parts",
        "derive_typed_path",
        "derive_from_ref",
    ];
    for field in ["paths", "callers", "direct_callers", "direct_call_sites"] {
        let rows = impact
            .get(field)
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("call_impact {field} array: {impact:#?}"));
        assert_eq!(
            rows.len(),
            expected_callers.len(),
            "expand_with impact {field} should expose one row per proc-macro caller: {rows:#?}"
        );
    }
    let callers = impact
        .get("callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact callers array");
    let public_callers = impact
        .get("public_callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact public_callers array");
    assert_eq!(
        public_callers.len(),
        expected_callers.len(),
        "expand_with impact public_callers should expose public proc-macro entrypoints: {public_callers:#?}"
    );
    for name in expected_callers {
        assert!(
            callers.iter().any(|caller| {
                caller.get("name").and_then(serde_json::Value::as_str) == Some(name)
            }),
            "expand_with impact callers should include {name}: {callers:#?}"
        );
        assert!(
            public_callers.iter().any(|caller| {
                caller.get("name").and_then(serde_json::Value::as_str) == Some(name)
            }),
            "expand_with impact public_callers should include {name}: {public_callers:#?}"
        );
    }
    let direct_call_sites = impact
        .get("direct_call_sites")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact direct_call_sites array");
    let direct_call_sites = direct_call_sites
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .expect("typed expand_with direct callsite rows");
    for call in direct_call_sites {
        assert!(
            matches!(&call.callee, CallCalleeInfo::Path { path } if path == &vec!["expand_with".to_string()]),
            "expand_with direct callsite should preserve the path callee: {call:#?}"
        );
        assert_eq!(
            call.arg_count,
            Some(2),
            "expand_with direct callsite should preserve arity: {call:#?}"
        );
        assert!(
            call.targets
                .iter()
                .any(|target| target.target_id.to_string() == target_id),
            "expand_with direct callsite should target the looked-up function: {call:#?}"
        );
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "impact_callers"), "4");
    assert_eq!(ui_field(ui, "impact_direct_callers"), "4");
    assert_eq!(ui_field(ui, "impact_direct_call_sites"), "4");
    assert_eq!(ui_field(ui, "impact_public_callers"), "4");
    assert_eq!(
        ui_field(ui, "impact_source_cfgs"),
        impact
            .get("source_cfgs")
            .and_then(serde_json::Value::as_array)
            .expect("call_impact source_cfgs array")
            .len()
            .to_string()
    );
}

#[tokio::test]
async fn code_item_lookup_reports_private_target_without_incoming_callers() {
    let fixture = AxumErrorHandlingTraitsToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("traits"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Owned(fixture.module_path_arg()),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        body_contains: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("axum-traits-zero-impact-lookup"))
        .await
        .expect("error_handling::traits lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Dead code detection:
    //   "Which private helpers have no incoming callers?"
    //   "Is this function reachable from any binary, test, macro entrypoint,
    //   or exported API?"
    //
    // Source oracle:
    //   axum/src/error_handling/mod.rs:257 defines `#[test] fn traits()`.
    //   No checked-in axum source row calls `traits(...)`; generated test
    //   harness entrypoints are outside the persisted source call graph.
    let incoming_paths = payload
        .get("call_paths_to_target")
        .and_then(serde_json::Value::as_array)
        .expect("call_paths_to_target array");
    assert!(
        incoming_paths.is_empty(),
        "code_item_lookup should expose zero incoming call paths: {incoming_paths:#?}"
    );

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
    assert_eq!(
        impact
            .get("target")
            .and_then(|target| target.get("is_public"))
            .and_then(serde_json::Value::as_bool),
        Some(false)
    );
    for field in [
        "paths",
        "callers",
        "direct_callers",
        "direct_call_sites",
        "callsite_buckets",
        "public_callers",
        "test_callers",
        "non_test_callers",
    ] {
        let rows = impact
            .get(field)
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("call_impact {field} array: {impact:#?}"));
        assert!(
            rows.is_empty(),
            "code_item_lookup zero-caller impact {field} should be empty: {rows:#?}"
        );
    }
    let source_files = impact
        .get("source_files")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact source_files array");
    assert_source_file(
        source_files,
        "axum/src/error_handling/mod.rs",
        "code_item_lookup zero-caller impact source files",
    );

    let proof_context = payload
        .get("proof_context")
        .and_then(serde_json::Value::as_array)
        .expect("proof_context array");
    let proof_rows = proof_context
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        proof_rows.iter().any(|proof| {
            proof.kind == "entrypoint_summary"
                && proof.definition_id.as_deref() == Some(target_id.as_str())
                && proof.target_kind.as_deref() == Some("test")
                && proof.target_name.as_deref() == Some("generated-test-harness")
                && proof.summary_class.as_deref() == Some("analyzed_source")
                && proof.status.as_deref() == Some("admitted")
        }),
        "code_item_lookup should expose the generated test-harness entrypoint proof summary without source callers: {proof_context:#?}"
    );
    let build_domains = payload
        .get("call_build_domains")
        .and_then(serde_json::Value::as_array)
        .expect("call_build_domains array");
    assert_eq!(
        build_domains.len(),
        1,
        "code_item_lookup should expose one generated-test build domain: {build_domains:#?}"
    );
    let domain = build_domains[0]
        .as_object()
        .expect("call_build_domains object");
    assert_eq!(
        domain
            .get("build_domain_id")
            .and_then(serde_json::Value::as_str),
        Some("bd:corpus-axum-call-graph")
    );
    assert_eq!(
        domain
            .get("target_kind")
            .and_then(serde_json::Value::as_str),
        Some("library")
    );
    assert_eq!(
        domain
            .get("target_name")
            .and_then(serde_json::Value::as_str),
        Some("axum")
    );
    assert_eq!(
        domain
            .get("target_root")
            .and_then(serde_json::Value::as_str),
        Some("axum/src/lib.rs")
    );
    let entrypoints = payload
        .get("call_test_entrypoints")
        .and_then(serde_json::Value::as_array)
        .expect("call_test_entrypoints array");
    assert_eq!(
        entrypoints.len(),
        1,
        "code_item_lookup should expose one generated-test entrypoint summary: {entrypoints:#?}"
    );
    let entrypoint = entrypoints[0]
        .as_object()
        .expect("call_test_entrypoints object");
    assert_eq!(
        entrypoint
            .get("entrypoint_summary_id")
            .and_then(serde_json::Value::as_str),
        Some("entrypoint-summary:axum-error-handling-traits-test")
    );
    assert_eq!(
        entrypoint
            .get("definition_id")
            .and_then(serde_json::Value::as_str),
        Some(target_id.as_str())
    );
    assert_eq!(
        entrypoint
            .get("target_kind")
            .and_then(serde_json::Value::as_str),
        Some("test")
    );
    assert_eq!(
        entrypoint
            .get("target_name")
            .and_then(serde_json::Value::as_str),
        Some("generated-test-harness")
    );
    assert_eq!(
        entrypoint
            .get("summary_class")
            .and_then(serde_json::Value::as_str),
        Some("analyzed_source")
    );
    assert_eq!(
        entrypoint
            .get("required_containment")
            .and_then(serde_json::Value::as_str),
        Some("rust-test-harness")
    );
    let allowed_effects = entrypoint
        .get("allowed_effects")
        .and_then(serde_json::Value::as_array)
        .expect("call_test_entrypoints.allowed_effects array");
    assert_eq!(
        allowed_effects
            .iter()
            .filter_map(serde_json::Value::as_str)
            .collect::<Vec<_>>(),
        vec!["ffi_boundary"]
    );
    let test_selection = payload
        .get("call_test_selection")
        .and_then(serde_json::Value::as_object)
        .expect("call_test_selection object");
    assert_eq!(
        test_selection
            .get("target")
            .and_then(|target| target.get("id"))
            .and_then(serde_json::Value::as_str),
        Some(target_id.as_str()),
        "code_item_lookup call_test_selection should target error_handling::traits: {test_selection:#?}"
    );
    for field in ["source_test_callers", "source_test_paths"] {
        let rows = test_selection
            .get(field)
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("call_test_selection {field} array: {test_selection:#?}"));
        assert!(
            rows.is_empty(),
            "code_item_lookup generated harness selection should not fabricate source {field}: {rows:#?}"
        );
    }
    assert_eq!(
        test_selection
            .get("generated_entrypoints")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1),
        "code_item_lookup call_test_selection should include the generated harness entrypoint: {test_selection:#?}"
    );
    assert_eq!(
        test_selection
            .get("build_domains")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1),
        "code_item_lookup call_test_selection should include the linked build domain: {test_selection:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(
        ui_field(ui, "proof_context"),
        proof_context.len().to_string()
    );
    assert_eq!(
        ui_field(ui, "call_build_domains"),
        build_domains.len().to_string()
    );
    assert_eq!(
        ui_field(ui, "call_test_entrypoints"),
        entrypoints.len().to_string()
    );
    assert_eq!(ui_field(ui, "call_test_selection_source_tests"), "0");
    assert_eq!(ui_field(ui, "call_test_selection_source_paths"), "0");
    assert_eq!(
        ui_field(ui, "call_test_selection_generated_entrypoints"),
        "1"
    );
    assert_eq!(ui_field(ui, "call_test_selection_build_domains"), "1");
    assert_eq!(ui_field(ui, "call_context_incoming"), "0");
    assert_eq!(ui_field(ui, "call_paths_to_target"), "0");
    assert_eq!(ui_field(ui, "impact_callers"), "0");
    assert_eq!(ui_field(ui, "impact_direct_callers"), "0");
    assert_eq!(ui_field(ui, "impact_direct_call_sites"), "0");
    assert_eq!(ui_field(ui, "impact_public_callers"), "0");
    assert_eq!(ui_field(ui, "impact_test_callers"), "0");
    assert_eq!(ui_field(ui, "impact_non_test_callers"), "0");
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

fn assert_source_crate(crates: &[serde_json::Value], expected: &str, label: &str) {
    assert!(
        crates.iter().any(|name| name.as_str() == Some(expected)),
        "{label} should include source crate {expected:?}: {crates:#?}"
    );
}

fn assert_source_module(modules: &[serde_json::Value], expected: &[&str], label: &str) {
    assert!(
        modules.iter().any(|module| {
            module
                .as_array()
                .is_some_and(|segments| module_segments_eq(segments, expected))
        }),
        "{label} should include source module {expected:?}: {modules:#?}"
    );
}

fn module_segments_eq(segments: &[serde_json::Value], expected: &[&str]) -> bool {
    segments.len() == expected.len()
        && segments
            .iter()
            .zip(expected)
            .all(|(segment, expected)| segment.as_str() == Some(*expected))
}

fn module_path_field<'a>(
    node: &'a serde_json::Map<String, serde_json::Value>,
    label: &str,
) -> Vec<&'a str> {
    node.get("module_path")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{label} should include module_path array: {node:#?}"))
        .iter()
        .map(|segment| {
            segment
                .as_str()
                .unwrap_or_else(|| panic!("{label} module_path should contain strings: {node:#?}"))
        })
        .collect()
}
