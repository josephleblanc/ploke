use std::borrow::Cow;

use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallPathEdgeInfo, CallResolutionKind, CallSiteBucketInfo,
    CallSiteKind, CallStatusKind, CallTargetKind, ProofContextInfo,
};
use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
};

use crate::call_graph_tool_support::{
    AxumAwaitReceiverToolFixture, AxumBodyEmptyToolFixture, AxumBoxedIntoRouteToolFixture,
    AxumErrorHandlingTraitsToolFixture, AxumExpandWithToolFixture, AxumHandlerCallToolFixture,
    AxumJsonFromBytesToolFixture, AxumParseAttrsToolFixture, AxumRequestExtractPathToolFixture,
    AxumRunUiTestsToolFixture, AxumTaskSpawnEffectToolFixture, CallGraphToolFixture,
    CallableBlockerFixture, CallableBlockerShape, CallableParamResolvedFixture,
    ChronoAliasConstructorToolFixture, ChronoNaiveUtcToolFixture, FixtureBranchReceiverToolFixture,
    FixtureDynamicCallableToolFixture, FixtureSelfFieldReceiverToolFixture,
    assert_ambiguous_dynamic_candidates, assert_ambiguous_path_candidates,
    assert_await_result_unwrap_context, assert_await_result_unwrap_proof,
    assert_body_empty_dependency_root_proof, assert_body_empty_impact_summary,
    assert_body_empty_incoming_context, assert_boxed_into_route_incoming_context,
    assert_branch_receiver_context, assert_branch_receiver_proof, assert_call_path_node,
    assert_chrono_naive_utc_incoming_context, assert_dynamic_context, assert_dynamic_proof,
    assert_expected_path_incoming_context, assert_fixture_extern_c_abs_effects,
    assert_handler_call_incoming_context, assert_incoming_context,
    assert_initialized_local_receiver_context, assert_initialized_local_receiver_proof,
    assert_json_from_bytes_incoming_context, assert_no_external_summary_need_for_site,
    assert_parse_attrs_incoming_context, assert_path_blocker_proof, assert_path_context,
    assert_path_resolution_proof, assert_resolved_callable_param_proof,
    assert_resolved_path_context, assert_run_ui_tests_incoming_context,
    assert_runtime_dispatch_blocker, assert_self_field_receiver_context,
    assert_self_field_receiver_proof, assert_serde_json_summary_proof, assert_target_proof,
    assert_task_spawn_effects, assert_task_spawn_policy_violation, assert_two_hop_call_path,
    ui_field,
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
        parent_name: None,
        allowed_effects: Vec::new(),
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
async fn code_item_lookup_marks_unsafe_function_targets_in_call_impact() {
    let fixture = CallGraphToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("unsafe_target"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("unsafe-target-lookup"))
        .await
        .expect("unsafe_target lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Which call paths can reach `unsafe` blocks or FFI boundaries?"
    //
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:766 defines
    //   `pub unsafe fn unsafe_target()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:770 calls it from
    //   the safe wrapper `call_unsafe_function()`.
    let impact = payload
        .get("call_impact")
        .and_then(serde_json::Value::as_object)
        .expect("call_impact object");
    let target = impact
        .get("target")
        .and_then(serde_json::Value::as_object)
        .expect("call_impact target object");
    assert_eq!(
        target.get("is_unsafe").and_then(serde_json::Value::as_bool),
        Some(true),
        "code_item_lookup should serialize unsafe function item metadata: {impact:#?}"
    );

    let direct_callers = impact
        .get("direct_callers")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact direct_callers array");
    assert!(
        direct_callers.iter().any(|caller| {
            caller.get("name").and_then(serde_json::Value::as_str) == Some("call_unsafe_function")
                && caller.get("is_unsafe").and_then(serde_json::Value::as_bool) == Some(false)
        }),
        "code_item_lookup should preserve the safe direct caller without marking it unsafe: {direct_callers:#?}"
    );
}

#[tokio::test]
async fn code_item_lookup_surfaces_extern_c_effect_seed() {
    let fixture = CallGraphToolFixture::new().await;
    let expected = fixture.seed_extern_c_abs_effect();
    let params = LookupParams {
        item_name: Cow::Borrowed("call_extern_c_function"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("extern-c-effect-lookup"))
        .await
        .expect("call_extern_c_function lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let effects = payload
        .get("call_reach_effects")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach_effects array");

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Which call paths can reach `unsafe` blocks or FFI boundaries?"
    //
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:839-844 declares
    //   foreign function `abs(input)` inside an `unsafe extern "C"` block and
    //   calls `abs(value)` from `call_extern_c_function`.
    assert_fixture_extern_c_abs_effects(effects, &expected, "code_item_lookup call_reach_effects");
    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "reach_effects"), effects.len().to_string());
}

#[tokio::test]
async fn code_item_lookup_returns_recursive_cycle_paths() {
    let fixture = CallGraphToolFixture::new().await;
    let params = LookupParams {
        item_name: Cow::Borrowed("recursive_fixture_call"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("recursive-lookup-cycles"))
        .await
        .expect("recursive_fixture_call lookup");
    let payload: serde_json::Value =
        serde_json::from_str(&result.content).expect("deserialize ConciseContext");
    let owner = payload
        .get("id")
        .and_then(serde_json::Value::as_str)
        .expect("resolved item id");
    let cycles = payload
        .get("call_cycles_from_owner")
        .and_then(serde_json::Value::as_array)
        .expect("call_cycles_from_owner array");

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `recursive_fixture_call(depth - 1)` is a resolved direct self-call.
    // Exact lookup should expose the DB/RAG cycle helper so tool callers can
    // answer "does this item recursively reach itself?" without recomputing
    // traversal from generic outgoing and incoming path arrays.
    assert_eq!(cycles.len(), 1, "recursive cycle paths: {cycles:#?}");
    let cycle = &cycles[0];
    assert_eq!(
        cycle.get("start_id").and_then(serde_json::Value::as_str),
        Some(owner)
    );
    assert_eq!(
        cycle.get("end_id").and_then(serde_json::Value::as_str),
        Some(owner)
    );
    assert_eq!(
        cycle.get("depth").and_then(serde_json::Value::as_u64),
        Some(1)
    );
    assert_eq!(
        cycle
            .get("edges")
            .and_then(serde_json::Value::as_array)
            .map(Vec::len),
        Some(1)
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert_eq!(ui_field(ui, "call_cycles_from_owner"), "1");
}

#[tokio::test]
async fn code_item_lookup_returns_resolved_dynamic_callable_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:269
    //     `call_parenthesized_function_item_binding` binds
    //     `let f = local_target;` and calls `(f)()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1362
    //     `call_parenthesized_block_initialized_function_item_binding` binds
    //     `let f = { local_target };` and calls `(f)()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:657
    //     `call_match_guarded_function_item` calls
    //     `(match flag { true if flag => local_target, _ => local_target })()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1555 and 1598-1606
    //     private helper parameters receive constructed holder values from a
    //     single local caller, then call `(holder.callback)()`,
    //     `holder.callbacks[0]()` / `holder.0[0]()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1635-1642
    //     a private helper receives `[local_target]` from its only local caller,
    //     then calls `funcs[0]()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1682-1684
    //     a private helper aliases a function-pointer parameter with
    //     `let g = f`, then calls `(g)()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1696-1699
    //     a boxed `dyn Fn` binding initialized with `Box::new(local_target)`
    //     is dereferenced and called as `(*boxed_fn)()`.
    // Parser/DB/RAG already prove these as resolved dynamic-function edges.
    // This pins the same facts at the TUI tool boundary.
    for owner_name in [
        "call_parenthesized_function_item_binding",
        "call_parenthesized_block_initialized_function_item_binding",
        "call_match_guarded_function_item",
        "call_single_named_field_function_param",
        "call_single_indexed_function_pointer_param",
        "call_single_indexed_field_function_param",
        "call_single_indexed_tuple_field_function_param",
        "call_single_parenthesized_aliased_function_pointer_param",
        "call_dereferenced_boxed_dyn_fn_value_binding",
    ] {
        assert_resolved_dynamic_callable_lookup(owner_name).await;
    }
}

#[tokio::test]
async fn code_item_lookup_returns_forwarded_named_field_dynamic_callable_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF
    //     private `call_forwarded_named_field_leaf(holder)` resolves through a
    //     private wrapper whose complete caller set constructs a holder with
    //     `callback: local_target`.
    // This keeps the new field-forwarding proof covered at the tool boundary
    // without widening the already-expensive dynamic-callable batch.
    assert_resolved_dynamic_callable_lookup("call_forwarded_named_field_leaf").await;
}

#[tokio::test]
async fn code_item_lookup_returns_branch_receiver_method_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:
    //     `(if flag { LocalAssoc } else { LocalAssoc }).instance_value()`
    //     and
    //     `(match flag { true => LocalAssoc, false => LocalAssoc }).instance_value()`
    // Parser/DB/RAG prove both as exact local method edges using the branch-path
    // receiver carrier. This pins the same resolved context at the lookup tool.
    for owner_name in [
        "call_if_expression_receiver_method",
        "call_match_expression_receiver_method",
    ] {
        let fixture = FixtureBranchReceiverToolFixture::new_for_owner(owner_name).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(fixture.owner_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("branch-receiver-lookup"))
            .await
            .expect("branch receiver lookup");
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

        let call = assert_branch_receiver_context(call_context, &fixture, "code_item_lookup");
        assert_branch_receiver_proof(proof_context, &fixture, call.site_id, "code_item_lookup");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing branch receiver call context for {owner_name}"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_lookup_returns_branch_initialized_receiver_method_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1834-1845
    //     `let value = if flag { LocalAssoc } else { LocalAssoc };`
    //     and the equivalent `match` initializer both call
    //     `value.instance_value()`.
    // Parser/DB/RAG prove both as exact local method edges using the existing
    // initialized-local receiver carrier. This pins the same context at the
    // lookup tool boundary.
    for owner_name in [
        "call_if_initialized_local_instance_method",
        "call_match_initialized_local_instance_method",
    ] {
        let fixture = FixtureBranchReceiverToolFixture::new_for_owner(owner_name).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(fixture.owner_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("branch-init-receiver-lookup"))
            .await
            .expect("branch initialized receiver lookup");
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

        let call =
            assert_initialized_local_receiver_context(call_context, &fixture, "code_item_lookup");
        assert_initialized_local_receiver_proof(
            proof_context,
            &fixture,
            call.site_id,
            "code_item_lookup",
        );

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface outgoing initialized receiver call context for {owner_name}"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_lookup_returns_nested_self_field_method_context() {
    // Fixture source:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:
    //     NestedSelfFieldAssocOwner::call_nested_self_field_instance_method
    //     calls `self.inner.value.instance_value()`.
    // The resolver should walk both self-field segments through local struct
    // field types and expose the same proof rows at the lookup boundary.
    let fixture = FixtureSelfFieldReceiverToolFixture::nested_self_field().await;
    let params = LookupParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed(fixture.owner_type)),
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("nested-self-field-lookup"))
        .await
        .expect("nested self-field lookup");
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

    let call = assert_self_field_receiver_context(call_context, &fixture, "code_item_lookup");
    assert_self_field_receiver_proof(proof_context, &fixture, call.site_id, "code_item_lookup");

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_lookup should surface outgoing nested self-field call context"
    );
    assert_eq!(
        ui_field(ui, "proof_context"),
        proof_context.len().to_string()
    );
}

#[tokio::test]
async fn code_item_lookup_returns_function_pointer_param_blocker() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:
    //     public `call_function_pointer_param(f)` calls `f()`;
    //     private `call_multi_conflicting_function_pointer_param(f)` also
    //     calls `f()`, but its local callers pass different functions;
    //     private `call_forwarded_conflicting_function_pointer_leaf(f)`
    //     receives `f` through a private wrapper whose callers pass
    //     different functions;
    //     public `call_generic_fn_once_value_binding(generic_f)` calls
    //     `generic_f()`;
    //     private `call_multi_conflicting_generic_fn_once_param(generic_f)`
    //     also calls `generic_f()`, but its local callers pass different
    //     functions;
    //     private `call_multi_conflicting_named_field_function_param(holder)`
    //     calls `(holder.callback)()`, but its local callers pass different
    //     functions in that field;
    //     public `call_field_function_param(holder)` calls
    //     `(holder.callback)()`;
    //     public `call_indexed_function_pointer(funcs)` calls `funcs[0]()`;
    //     public `call_indexed_field_function_param(holder)` calls
    //     `holder.callbacks[0]()`;
    //     public `call_indexed_tuple_field_function_param(holder)` calls
    //     `holder.0[0]()`.
    //
    // Public opaque parameters stay blocked and targetless. Private complete
    // local caller sets with conflicting callable arguments expose candidate
    // targets, but still do not fabricate a resolved call edge.
    for fixture in [
        CallableBlockerFixture::function_pointer_param().await,
        CallableBlockerFixture::multi_conflicting_function_pointer_param().await,
        CallableBlockerFixture::forwarded_conflicting_function_pointer_leaf().await,
        CallableBlockerFixture::generic_fn_once_value_binding().await,
        CallableBlockerFixture::multi_conflicting_generic_fn_once_param().await,
        CallableBlockerFixture::multi_conflicting_named_field_function_param().await,
        CallableBlockerFixture::field_function_param().await,
        CallableBlockerFixture::indexed_function_pointer().await,
        CallableBlockerFixture::indexed_field_function_param().await,
        CallableBlockerFixture::indexed_tuple_field_function_param().await,
    ] {
        assert_callable_blocker_lookup(fixture).await;
    }
}

#[tokio::test]
async fn code_item_lookup_returns_forwarded_named_field_param_blocker() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    //     private `call_forwarded_conflicting_named_field_leaf(holder)` receives
    //     `holder` through a private wrapper whose callers pass different
    //     callback functions.
    assert_callable_blocker_lookup(
        CallableBlockerFixture::forwarded_conflicting_named_field_leaf().await,
    )
    .await;
}

async fn assert_callable_blocker_lookup(fixture: CallableBlockerFixture) {
    let params = LookupParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("fn-pointer-param-lookup"))
        .await
        .expect("function pointer param lookup");
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

    let label = format!("{} callable parameter call", fixture.owner_name);
    let callee = CallCalleeInfo::Path {
        path: fixture.path.clone(),
    };
    let site_id = match fixture.shape {
        CallableBlockerShape::Path => assert_path_context(
            call_context,
            fixture.owner,
            &callee,
            &CallStatusKind::Unsupported,
            label.as_str(),
            "code_item_lookup",
        ),
        CallableBlockerShape::Dynamic => assert_dynamic_context(
            call_context,
            fixture.owner,
            None,
            None,
            label.as_str(),
            "code_item_lookup",
        ),
        CallableBlockerShape::AmbiguousPath => assert_ambiguous_path_candidates(
            call_context,
            fixture.owner,
            &callee,
            &fixture.candidates,
            label.as_str(),
            "code_item_lookup",
        ),
        CallableBlockerShape::AmbiguousDynamic => assert_ambiguous_dynamic_candidates(
            call_context,
            fixture.owner,
            &fixture.candidates,
            label.as_str(),
            "code_item_lookup",
        ),
    };
    match fixture.shape {
        CallableBlockerShape::Path => assert_path_resolution_proof(
            proof_context,
            fixture.owner,
            site_id,
            fixture.build_domain,
            "blocked",
            "type_resolution_missing",
            label.as_str(),
            "code_item_lookup",
        ),
        CallableBlockerShape::Dynamic => assert_dynamic_proof(
            proof_context,
            fixture.owner,
            site_id,
            fixture.build_domain,
            label.as_str(),
            "code_item_lookup",
        ),
        CallableBlockerShape::AmbiguousPath | CallableBlockerShape::AmbiguousDynamic => {
            assert_path_resolution_proof(
                proof_context,
                fixture.owner,
                site_id,
                fixture.build_domain,
                "ambiguous",
                "type_resolution_missing",
                label.as_str(),
                "code_item_lookup",
            );
        }
    }

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_lookup should surface the callable parameter call"
    );
    assert_eq!(
        ui_field(ui, "proof_context"),
        proof_context.len().to_string()
    );
}

#[tokio::test]
async fn code_item_lookup_returns_non_awaited_async_closure_poll_resume_blockers() {
    let fixture = CallGraphToolFixture::new().await;

    for (owner_name, label) in [
        (
            "call_async_closure_binding_without_await_with_body_call",
            "non-awaited async closure binding",
        ),
        (
            "call_async_closure_future_binding_without_await_with_body_call",
            "unawaited async closure future binding",
        ),
    ] {
        let expected = fixture.async_closure_blocker(owner_name);
        let params = LookupParams {
            item_name: Cow::Borrowed(owner_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("async-closure-blocker-lookup"))
            .await
            .expect("async closure poll/resume blocker lookup");
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

        // Fixture source:
        //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1701-1714
        //   calls an async closure without awaiting the returned future. The
        //   outer call remains targetless; the derived proof blocker explains
        //   the missing async poll/resume proof input.
        let callee = CallCalleeInfo::Path {
            path: expected.path.clone(),
        };
        let site_id = assert_path_context(
            call_context,
            expected.owner,
            &callee,
            &CallStatusKind::Unsupported,
            label,
            "code_item_lookup",
        );
        assert_eq!(site_id, expected.site);
        assert_path_blocker_proof(
            proof_context,
            expected.owner,
            site_id,
            "bd:fixture-call-graph",
            "type_resolution_missing",
            label,
            "code_item_lookup",
        );
        assert_runtime_dispatch_blocker(proof_context, site_id, label, "code_item_lookup");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface the targetless async closure call"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_lookup_returns_multi_caller_function_pointer_param_target() {
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1740-1742
    //     private `call_multi_function_pointer_param(f)` calls `f()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1744-1749
    //     both local callers pass `local_target`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1942-1951
    //     private `call_forwarded_function_pointer_leaf(f)` resolves through a
    //     private wrapper whose complete caller set passes `local_target`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1764-1768
    //     private `call_multi_generic_fn_once_param(generic_f)` calls
    //     `generic_f()`.
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:1771-1776
    //     both local generic callers pass `local_target`.
    for fixture in [
        CallableParamResolvedFixture::multi_function_pointer_param().await,
        CallableParamResolvedFixture::forwarded_function_pointer_leaf().await,
        CallableParamResolvedFixture::multi_generic_fn_once_param().await,
    ] {
        let params = LookupParams {
            item_name: Cow::Borrowed(fixture.owner_name),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("function"),
            module_path: Cow::Borrowed("crate"),
            owner_trait: None,
            owner_type: None,
            parent_name: None,
            allowed_effects: Vec::new(),
        };

        let result = CodeItemLookup::execute(params, fixture.ctx("multi-callable-param-lookup"))
            .await
            .unwrap_or_else(|err| panic!("{} lookup: {err}", fixture.owner_name));
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

        // Exact tool lookup should preserve the complete-private-caller proof:
        // multiple local callers are accepted only because every visible
        // argument proves the same target.
        let callee = CallCalleeInfo::Path {
            path: fixture.path.clone(),
        };
        let site_id = assert_resolved_path_context(
            call_context,
            fixture.owner,
            &callee,
            fixture.target,
            CallTargetKind::Function,
            "multi-caller callable parameter",
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

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert!(
            ui_field(ui, "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "code_item_lookup should surface the resolved multi-caller callable parameter row"
        );
        assert_eq!(
            ui_field(ui, "proof_context"),
            proof_context.len().to_string()
        );
    }
}

async fn assert_resolved_dynamic_callable_lookup(owner_name: &'static str) {
    let fixture = FixtureDynamicCallableToolFixture::new_for_owner(owner_name).await;
    let params = LookupParams {
        item_name: Cow::Borrowed(fixture.owner_name),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("function"),
        module_path: Cow::Borrowed("crate"),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("dynamic-callable-lookup"))
        .await
        .expect("dynamic callable lookup");
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

    let calls = call_context
        .iter()
        .filter_map(|call| serde_json::from_value::<CallContextInfo>(call.clone()).ok())
        .filter(|call| {
            call.owner_id == fixture.owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
        })
        .collect::<Vec<_>>();
    assert_eq!(
        calls.len(),
        1,
        "code_item_lookup should expose exactly one resolved dynamic callable row for {owner_name}: {call_context:#?}"
    );
    let call = calls[0].clone();
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1, "{call:#?}");
    assert_eq!(call.targets[0].target_id, fixture.target);
    assert_eq!(call.targets[0].relation, CallTargetKind::DynamicFunction);

    let owner = fixture.owner.to_string();
    let site = call.site_id.to_string();
    let target = fixture.target.to_string();
    let proof_rows = proof_context
        .iter()
        .filter_map(|proof| serde_json::from_value::<ProofContextInfo>(proof.clone()).ok())
        .collect::<Vec<_>>();
    assert!(
        proof_rows.iter().any(|proof| {
            proof.kind == "call_site"
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.build_domain_id.as_deref() == Some("bd:fixture-call-graph")
        }),
        "code_item_lookup should return the dynamic call_site proof row for {owner_name}: {proof_context:#?}"
    );
    assert!(
        proof_rows.iter().any(|proof| {
            proof.kind == "call_edge"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.caller_def_id.as_deref() == Some(owner.as_str())
                && proof.callee_def_id.as_deref() == Some(target.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
        }),
        "code_item_lookup should return the resolved dynamic call_edge proof row for {owner_name}: {proof_context:#?}"
    );
    assert!(
        proof_rows.iter().any(|proof| {
            proof.kind == "call_resolution"
                && proof.call_site_id.as_deref() == Some(site.as_str())
                && proof.resolution_state.as_deref() == Some("resolved")
                && proof.resolved_def_id.as_deref() == Some(target.as_str())
        }),
        "code_item_lookup should return the resolved dynamic call_resolution proof row for {owner_name}: {proof_context:#?}"
    );

    let ui = result.ui_payload.as_ref().expect("ui payload");
    assert!(
        ui_field(ui, "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "code_item_lookup should surface outgoing dynamic callable call context for {owner_name}"
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= 3,
        "code_item_lookup should surface resolved dynamic callable proof rows for {owner_name}"
    );
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
    // `AwaitMethodCallResult(acquire_owned).unwrap` row without inventing an
    // outgoing target edge.
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
                "code_item_lookup should expose AwaitMethodCallResult unwrap in unsupported frontier rows: {unsupported_calls:#?}"
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
    assert_eq!(ui_field(ui, "call_context_incoming"), "0");
    assert_eq!(ui_field(ui, "call_paths_to_target"), "0");
    assert_eq!(ui_field(ui, "impact_callers"), "0");
    assert_eq!(ui_field(ui, "impact_direct_callers"), "0");
    assert_eq!(ui_field(ui, "impact_direct_call_sites"), "0");
    assert_eq!(ui_field(ui, "impact_public_callers"), "0");
    assert_eq!(ui_field(ui, "impact_test_callers"), "0");
    assert_eq!(ui_field(ui, "impact_non_test_callers"), "0");
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
        parent_name: None,
        allowed_effects: Vec::new(),
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
        parent_name: None,
        allowed_effects: Vec::new(),
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
    let impact_source_crates = impact
        .get("source_crates")
        .and_then(serde_json::Value::as_array)
        .expect("call_impact source_crates array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum-core/src/body.rs:52 defines `Body::empty`.
    //   axum-core/src/body.rs:110 and :116 call `Self::empty()`.
    //   axum-core/src/response/into_response.rs response conversion rows call
    //   `Body::empty()`.
    //   Four axum direct parsed-workspace import rows and eleven local
    //   re-exported, inherited, closure, and local-item workspace rows also call
    //   `Body::empty()`.
    // Expected tool traversal: exact lookup of the callee method exposes all
    // twenty-three current incoming caller-site edges and their projected proof rows.
    assert_body_empty_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_lookup",
    );
    assert_body_empty_impact_summary(impact, "code_item_lookup");
    for caller in &fixture.callers {
        assert_target_proof(
            proof_context,
            caller.owner,
            fixture.target,
            "code_item_lookup",
        );
    }
    assert_body_empty_dependency_root_proof(
        proof_context,
        &fixture.dependency_root_sites,
        fixture.target,
        "code_item_lookup",
    );
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
    assert_eq!(ui_field(ui, "call_context_incoming"), "23");
    assert_eq!(
        ui_field(ui, "impact_test_callers"),
        impact_test_callers.len().to_string()
    );
    assert_eq!(
        ui_field(ui, "impact_non_test_callers"),
        impact_non_test_callers.len().to_string()
    );
    assert_eq!(
        ui_field(ui, "impact_source_crates"),
        impact_source_crates.len().to_string()
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
        parent_name: None,
        allowed_effects: Vec::new(),
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
    assert_eq!(
        ui_field(ui, "call_context_incoming"),
        fixture.callers.len().to_string().as_str()
    );
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
        parent_name: None,
        allowed_effects: Vec::new(),
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
    let serde_site = fixture.admit_serde_summary();
    let module_path = fixture.module_path_arg();
    let params = LookupParams {
        item_name: Cow::Borrowed("from_bytes"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
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
    let summary_needs = payload
        .get("external_summary_needs")
        .and_then(serde_json::Value::as_array)
        .expect("external_summary_needs array");
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
    let reach_source_cfgs = reach
        .get("source_cfgs")
        .and_then(serde_json::Value::as_array)
        .expect("call_reach source_cfgs array");

    // Real-corpus oracle matrix:
    //   docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md
    //   axum/src/json.rs:164 defines `Json::from_bytes`.
    //   axum/src/json.rs:112 and :128 call `Self::from_bytes(&bytes)`.
    //   axum/src/json.rs:184 calls
    //     `serde_json::Deserializer::from_slice(bytes)`.
    //   axum/src/lib.rs:488-489 gates the file module with
    //     `#[cfg(feature = "json")] mod json;`.
    // Expected tool traversal: exact lookup of the callee method exposes both
    // trait-impl `Self::from_bytes` caller-site edges and proof rows. Its
    // owner reach summary also surfaces the serde_json dependency-root call as
    // an external frontier row, not a fabricated local edge. The fixture
    // admits an audited summary before tool execution, so the payload should
    // expose the externally-summarized proof and no longer queue this site as
    // an active missing-summary need, while preserving the inherited feature
    // gate.
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
    assert_eq!(serde_frontier.site_id, serde_site);
    assert_serde_json_summary_proof(
        proof_context,
        fixture.target,
        serde_site,
        "Json::from_bytes serde_json::Deserializer::from_slice",
        "code_item_lookup",
    );
    assert_no_external_summary_need_for_site(
        summary_needs,
        serde_site,
        "Json::from_bytes serde_json::Deserializer::from_slice",
        "code_item_lookup",
    );
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
    assert!(
        reach_source_cfgs
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|cfg| cfg == r#"feature = "json""#),
        "code_item_lookup Json::from_bytes reach should preserve the json feature cfg: {reach_source_cfgs:#?}"
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
    assert_eq!(
        ui_field(ui, "reach_source_cfgs"),
        reach_source_cfgs.len().to_string()
    );
    assert_eq!(
        ui_field(ui, "external_summary_needs"),
        summary_needs.len().to_string()
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
        parent_name: None,
        allowed_effects: Vec::new(),
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
    //   axum/src/boxed.rs:{23,38,51} call `Self(...)`,
    //   `BoxedIntoRoute(...)`, and `Self(...)`.
    // Expected tool traversal: exact lookup of the tuple-struct target exposes
    // the incoming constructor edges and their projected proof rows.
    assert_boxed_into_route_incoming_context(
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
    assert_eq!(
        ui_field(ui, "call_context_incoming"),
        fixture.callers.len().to_string().as_str()
    );
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_lookup should surface real-corpus BoxedIntoRoute proof rows"
    );
}

#[tokio::test]
async fn code_item_lookup_returns_real_corpus_chrono_alias_constructor_callers() {
    let fixture = ChronoAliasConstructorToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = LookupParams {
        item_name: Cow::Borrowed("Single"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("variant"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: None,
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("chrono-alias-constructor-lookup"))
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
    //   chrono/src/offset/mod.rs:77 aliases
    //   `MappedLocalTime<T> = LocalResult<T>`.
    //   chrono/src/offset/mod.rs:81-83 defines `LocalResult::Single(T)`.
    //   chrono/src/offset/mod.rs:{143,156,468,502,535},
    //   offset/{fixed.rs:135,138,utc.rs:122,125,local/unix.rs:159}, and
    //   datetime/tests.rs:{75,79} call `MappedLocalTime::Single(...)`.
    // Expected tool traversal: exact lookup of the underlying enum-variant
    // target exposes all 12 incoming alias constructor edges and proof rows.
    assert_expected_path_incoming_context(
        call_context,
        &fixture.callers,
        fixture.target,
        "code_item_lookup",
        "MappedLocalTime::Single",
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
    assert_eq!(ui_field(ui, "call_context_incoming"), "12");
    assert!(
        ui_field(ui, "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= fixture.callers.len(),
        "code_item_lookup should surface real-corpus chrono alias constructor proof rows"
    );
}

#[tokio::test]
async fn code_item_lookup_returns_real_corpus_chrono_option_ok_or_try_receiver_callers() {
    let fixture = ChronoNaiveUtcToolFixture::new().await;
    let module_path = fixture.module_path_arg();
    let params = LookupParams {
        item_name: Cow::Borrowed("naive_utc"),
        file_path: Cow::Owned(fixture.file_path.display().to_string()),
        node_kind: Cow::Borrowed("method"),
        module_path: Cow::Owned(module_path),
        owner_trait: None,
        owner_type: Some(Cow::Borrowed("DateTime")),
        parent_name: None,
        allowed_effects: Vec::new(),
    };

    let result = CodeItemLookup::execute(params, fixture.ctx("chrono-naive-utc-lookup"))
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
    //   chrono/src/datetime/mod.rs:563 defines `DateTime<Tz>::naive_utc`.
    //   chrono/src/datetime/mod.rs:768,803 define associated constructors
    //   returning `Option<Self>`.
    //   chrono/src/format/parsed.rs:836,953 call
    //   `DateTime::from_timestamp*(...).ok_or(OUT_OF_RANGE)?.naive_utc()`.
    // Expected tool traversal: exact lookup of `DateTime::naive_utc` exposes
    // both incoming try-receiver method edges and their projected proof rows.
    assert_chrono_naive_utc_incoming_context(
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
        "code_item_lookup should surface real-corpus chrono naive_utc proof rows"
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
        parent_name: None,
        allowed_effects: Vec::new(),
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
            parent_name: None,
            allowed_effects: Vec::new(),
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
            parent_name: None,
            allowed_effects: Vec::new(),
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
