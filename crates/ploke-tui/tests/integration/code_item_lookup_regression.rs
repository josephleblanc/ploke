use std::borrow::Cow;

use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
};

use crate::call_graph_tool_support::{
    AxumAwaitReceiverToolFixture, AxumBodyEmptyToolFixture, AxumBoxedIntoRouteToolFixture,
    AxumHandlerCallToolFixture, AxumJsonFromBytesToolFixture, AxumParseAttrsToolFixture,
    AxumRunUiTestsToolFixture, CallGraphToolFixture, assert_await_result_unwrap_context,
    assert_body_empty_incoming_context, assert_boxed_into_route_incoming_context,
    assert_handler_call_incoming_context, assert_incoming_context,
    assert_json_from_bytes_incoming_context, assert_parse_attrs_incoming_context,
    assert_run_ui_tests_incoming_context, assert_target_proof, ui_field,
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
    assert_await_result_unwrap_context(call_context, fixture.owner, "code_item_lookup");

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_lookup should surface outgoing targetless call-context count"
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

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "4");
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
        module_path: Cow::Owned(module_path),
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

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:112 and :128 call `Self::from_bytes(&bytes)`.
    // Expected tool traversal: exact lookup of the callee method exposes both
    // trait-impl `Self::from_bytes` caller-site edges and proof rows.
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

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_context_incoming"), "2");
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
