use std::borrow::Cow;

use ploke_core::rag_types::{CallCalleeInfo, CallContextInfo, CallStatusKind, CallTargetKind};
use ploke_db::ProofGraphStore;
use ploke_tui::tools::{
    Tool,
    code_item_lookup::{CodeItemLookup, LookupParams},
};
use uuid::Uuid;

use crate::call_graph_tool_support::{
    AmbiguousDynamicToolCase, AmbiguousDynamicToolFixture, ContextTool, ContextToolObservation,
    DynamicToolCase, DynamicToolFixture, ExactItemQuery, IfuncToolCase, IfuncToolFixture,
    PathToolCase, PathToolFixture, ReceiverToolCase, ReceiverToolFixture,
    assert_admitted_external_summary_effect, assert_admitted_external_summary_proof,
    assert_admitted_macro_boundary_summary_proof, assert_ambiguous_candidate_proof,
    assert_ambiguous_dynamic_candidates_with_relation, assert_dynamic_context,
    assert_dynamic_proof, assert_ifunc_context, assert_ifunc_proof, assert_ifunc_unsafe_calls,
    assert_method_context, assert_method_proof, assert_path_blocker_proof, assert_path_context,
    assert_path_context_absent, assert_path_resolution_proof, assert_poll_producer,
    assert_resolved_method_context, assert_resolved_method_proof,
    assert_resolved_path_context_count, assert_resolved_path_context_target,
    assert_resolved_path_proof, assert_runtime_dispatch_blocker,
    assert_self_field_binding_evidence, observe_exact_item, request_parts_extract_target, ui_field,
};

// Endpoint-parity ledger: every selector runs once through Lookup and once
// through Edges. The seven matrices cover 2 dynamic targetless, 4 dynamic
// candidate, 2 Route::oneshot, 1 size_hint, 3 unsupported-receiver, 7 ifunc,
// and 2 admitted memchr-summary selectors. Summary mutations use a fresh
// fixture per endpoint and repeat that endpoint after admission.
#[tokio::test]
async fn code_item_tools_return_dynamic_targetless_real_corpus_rows() {
    for case in DynamicToolCase::AXUM {
        for tool in ContextTool::ALL {
            let fixture = DynamicToolFixture::new(case).await;
            assert_dynamic_case(&fixture, tool).await;
        }
    }
}

async fn assert_dynamic_case(fixture: &DynamicToolFixture, tool: ContextTool) {
    let observed = observe_exact_item(
        tool,
        &fixture.state,
        ExactItemQuery {
            name: fixture.case.method,
            file: &fixture.file_path,
            kind: "method",
            module: fixture.module_path_arg(),
            trait_name: None,
            type_name: Some(fixture.case.owner_type),
            label: fixture.case.label,
        },
    )
    .await;
    let calls = observed.array("call_context");
    let proofs = observed.array("proof_context");
    let runtime_needs = observed.array("runtime_dispatch_needs");
    let endpoint = endpoint_label(tool);

    let site = assert_dynamic_context(
        calls,
        fixture.owner,
        fixture.case.expected_path,
        fixture.case.expected_arg_count,
        fixture.case.label,
        endpoint,
    );
    assert_dynamic_proof(
        proofs,
        fixture.owner,
        site,
        fixture.case.build_domain(),
        fixture.case.label,
        endpoint,
    );
    if let Some(path) = fixture.case.expected_path {
        assert_self_field_binding_evidence(
            proofs,
            fixture.owner,
            site,
            fixture.case.build_domain(),
            path,
            "blocked",
            fixture.case.label,
            endpoint,
        );
    }
    if fixture.case.expects_runtime_dispatch_blocker() {
        assert_runtime_dispatch_blocker(proofs, site, fixture.case.label, endpoint);
        assert_runtime_dispatch_need(runtime_needs, site, fixture.case.label, endpoint);
    }

    assert_context_ui(tool, &observed, proofs, 1, Some(2), fixture.case.label);
}

#[tokio::test]
async fn code_item_lookup_omits_admitted_runtime_dispatch_summary_needs() {
    for case in DynamicToolCase::AXUM {
        let fixture = DynamicToolFixture::new(case).await;
        let params = LookupParams {
            item_name: Cow::Borrowed(case.method),
            file_path: Cow::Owned(fixture.file_path.display().to_string()),
            node_kind: Cow::Borrowed("method"),
            module_path: Cow::Owned(fixture.module_path_arg()),
            owner_trait: None,
            owner_type: Some(Cow::Borrowed(case.owner_type)),
            parent_name: None,
            body_contains: None,
            allowed_effects: Vec::new(),
        };

        let initial = CodeItemLookup::execute(
            params.clone(),
            fixture.ctx("dynamic-targetless-summary-before"),
        )
        .await
        .unwrap_or_else(|err| panic!("{} initial code_item_lookup: {err}", fixture.case.label));
        let payload: serde_json::Value =
            serde_json::from_str(&initial.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let runtime_needs = payload
            .get("runtime_dispatch_needs")
            .and_then(serde_json::Value::as_array)
            .expect("runtime_dispatch_needs array");
        let site_id = assert_dynamic_context(
            call_context,
            fixture.owner,
            fixture.case.expected_path,
            fixture.case.expected_arg_count,
            fixture.case.label,
            "lookup",
        );
        assert_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "lookup");

        let expected_path = fixture
            .case
            .expected_path
            .expect("dynamic callable field path");
        let field = expected_path
            .last()
            .copied()
            .expect("dynamic callable field");
        fixture
            .state
            .db
            .upsert_proof_fact_values(&[
                ploke_test_utils::axum_callable_field_runtime_dispatch_summary(
                    site_id,
                    field,
                    fixture.case.label,
                ),
            ])
            .unwrap_or_else(|err| {
                panic!(
                    "{} runtime dispatch summary insert: {err}",
                    fixture.case.label
                )
            });

        let result =
            CodeItemLookup::execute(params, fixture.ctx("dynamic-targetless-summary-after"))
                .await
                .unwrap_or_else(|err| {
                    panic!("{} summary code_item_lookup: {err}", fixture.case.label)
                });
        let payload: serde_json::Value =
            serde_json::from_str(&result.content).expect("deserialize ConciseContext");
        let call_context = payload
            .get("call_context")
            .and_then(serde_json::Value::as_array)
            .expect("call_context array");
        let runtime_needs = payload
            .get("runtime_dispatch_needs")
            .and_then(serde_json::Value::as_array)
            .expect("runtime_dispatch_needs array");
        assert_dynamic_context(
            call_context,
            fixture.owner,
            fixture.case.expected_path,
            fixture.case.expected_arg_count,
            fixture.case.label,
            "lookup",
        );
        assert_no_runtime_dispatch_need(runtime_needs, site_id, fixture.case.label, "lookup");

        let ui = result.ui_payload.as_ref().expect("ui payload");
        assert_eq!(ui_field(ui, "runtime_dispatch_needs"), "0");
    }
}

#[tokio::test]
async fn code_item_tools_return_real_corpus_dynamic_candidate_rows() {
    for case in AmbiguousDynamicToolCase::AXUM_LAYER
        .into_iter()
        .chain(AmbiguousDynamicToolCase::MEMCHR_SELF_FIELD)
    {
        for tool in ContextTool::ALL {
            let fixture = AmbiguousDynamicToolFixture::new(case.clone()).await;
            assert_dynamic_candidate(&fixture, tool).await;
        }
    }
}

async fn assert_dynamic_candidate(fixture: &AmbiguousDynamicToolFixture, tool: ContextTool) {
    let observed = observe_exact_item(
        tool,
        &fixture.state,
        ExactItemQuery {
            name: fixture.case.method,
            file: &fixture.file_path,
            kind: "method",
            module: fixture.module_path_arg(),
            trait_name: None,
            type_name: Some(fixture.case.owner_type),
            label: fixture.case.label,
        },
    )
    .await;
    let calls = observed.array("call_context");
    let proofs = observed.array("proof_context");
    let endpoint = endpoint_label(tool);

    let site = assert_ambiguous_dynamic_candidates_with_relation(
        calls,
        fixture.owner,
        Some(fixture.case.expected_path),
        fixture.case.expected_arg_count,
        &fixture.candidates,
        fixture.case.expected_relation.clone(),
        fixture.case.label,
        endpoint,
    );
    assert_ambiguous_candidate_proof(
        proofs,
        fixture.owner,
        site,
        fixture.case.build_domain(),
        &fixture.candidates,
        fixture.case.label,
        endpoint,
    );
    if fixture.case.expects_self_field_binding_evidence() {
        assert_self_field_binding_evidence(
            proofs,
            fixture.owner,
            site,
            fixture.case.build_domain(),
            fixture.case.expected_path,
            "ambiguous",
            fixture.case.label,
            endpoint,
        );
    }

    assert_context_ui(tool, &observed, proofs, 1, None, fixture.case.label);
}

#[tokio::test]
async fn code_item_tools_return_route_oneshot_targetless_real_corpus_rows() {
    for case in ReceiverToolCase::ROUTE_ONESHOT {
        for tool in ContextTool::ALL {
            let fixture = ReceiverToolFixture::new(case.clone()).await;
            assert_receiver_frontier(&fixture, ReceiverContract::Route, tool).await;
        }
    }
}

#[derive(Clone, Copy)]
enum ReceiverContract {
    Route,
    SizeHint,
}

async fn assert_receiver_frontier(
    fixture: &ReceiverToolFixture,
    contract: ReceiverContract,
    tool: ContextTool,
) {
    let observed = observe_exact_item(tool, &fixture.state, receiver_query(fixture)).await;
    let calls = observed.array("call_context");
    let proofs = observed.array("proof_context");
    let summary_needs = observed.array("external_summary_needs");
    let reach_effects = observed.array("call_reach_effects");
    let endpoint = endpoint_label(tool);
    let callee = fixture.case.callee();
    let site = assert_method_context(
        calls,
        fixture.owner,
        &callee,
        &fixture.case.status,
        fixture.case.generic_arg_count,
        fixture.case.label,
        endpoint,
    );

    if let Some(summary) = fixture.case.admitted_external_summary() {
        assert_admitted_external_summary_proof(
            proofs,
            fixture.owner,
            site,
            summary,
            fixture.case.label,
            endpoint,
        );
        assert_no_external_summary_need(summary_needs, site, fixture.case.label, endpoint);
        assert_admitted_external_summary_effect(
            reach_effects,
            fixture.owner,
            site,
            summary,
            fixture.case.label,
            endpoint,
        );
    } else {
        assert_method_proof(
            proofs,
            fixture.owner,
            site,
            &fixture.case.status,
            fixture.case.label,
            endpoint,
        );
        if matches!(
            (contract, tool),
            (ReceiverContract::Route, ContextTool::Lookup)
                | (ReceiverContract::SizeHint, ContextTool::Edges)
        ) {
            assert_external_summary_need(summary_needs, site, fixture.case.label, endpoint);
        }
    }

    assert_context_ui(tool, &observed, proofs, 1, Some(2), fixture.case.label);
    if matches!(contract, ReceiverContract::Route) || tool == ContextTool::Edges {
        assert_eq!(
            ui_field(observed.ui(), "external_summary_needs"),
            summary_needs.len().to_string(),
            "{endpoint} external summary need count for {}",
            fixture.case.label
        );
    }
}

fn receiver_query(fixture: &ReceiverToolFixture) -> ExactItemQuery<'_> {
    ExactItemQuery {
        name: fixture.case.item,
        file: &fixture.file_path,
        kind: fixture.case.node_kind(),
        module: fixture.module_path_arg(),
        trait_name: None,
        type_name: fixture.case.owner_type(),
        label: fixture.case.label,
    }
}

#[tokio::test]
async fn code_item_tools_return_size_hint_external_real_corpus_row() {
    for case in ReceiverToolCase::SIZE_HINT {
        for tool in ContextTool::ALL {
            let fixture = ReceiverToolFixture::new(case.clone()).await;
            assert_receiver_frontier(&fixture, ReceiverContract::SizeHint, tool).await;
        }
    }
}

#[tokio::test]
async fn code_item_tools_return_unsupported_receiver_targetless_real_corpus_rows() {
    for case in ReceiverToolCase::REQUEST_PARTS_TURBOFISH
        .into_iter()
        .chain(ReceiverToolCase::FUTURE_POLL)
    {
        for tool in ContextTool::ALL {
            let fixture = ReceiverToolFixture::new(case.clone()).await;
            assert_unsupported_receiver(&fixture, tool).await;
        }
    }
}

async fn assert_unsupported_receiver(fixture: &ReceiverToolFixture, tool: ContextTool) {
    let observed = observe_exact_item(tool, &fixture.state, receiver_query(fixture)).await;
    let calls = observed.array("call_context");
    let proofs = observed.array("proof_context");
    let runtime_needs = observed.array("runtime_dispatch_needs");
    let poll_flows = observed.array("future_poll_field_producer_flows");
    let endpoint = endpoint_label(tool);
    let callee = fixture.case.callee();

    if fixture.case.status == CallStatusKind::Resolved {
        let target = request_parts_extract_target(fixture.state.db.as_ref());
        let site = assert_resolved_method_context(
            calls,
            fixture.owner,
            &callee,
            target,
            fixture.case.generic_arg_count,
            fixture.case.label,
            endpoint,
        );
        assert_resolved_method_proof(
            proofs,
            fixture.owner,
            site,
            target,
            fixture.case.label,
            endpoint,
        );
    } else {
        let site = assert_method_context(
            calls,
            fixture.owner,
            &callee,
            &fixture.case.status,
            fixture.case.generic_arg_count,
            fixture.case.label,
            endpoint,
        );
        assert_method_proof(
            proofs,
            fixture.owner,
            site,
            &fixture.case.status,
            fixture.case.label,
            endpoint,
        );
        if fixture.case.expects_runtime_dispatch_blocker() {
            assert_runtime_dispatch_blocker(proofs, site, fixture.case.label, endpoint);
            assert_runtime_dispatch_need(runtime_needs, site, fixture.case.label, endpoint);
            assert_poll_producer(
                poll_flows,
                fixture.owner,
                fixture
                    .case
                    .poll_producer()
                    .expect("future-poll case should have producer proof shape"),
                observed.label(),
            );
            fixture
                .state
                .db
                .upsert_proof_fact_values(&[fixture.case.runtime_dispatch_summary(site)])
                .unwrap_or_else(|err| {
                    panic!(
                        "{} runtime dispatch summary insert: {err}",
                        fixture.case.label
                    )
                });

            let after = observe_exact_item(tool, &fixture.state, receiver_query(fixture)).await;
            let after_calls = after.array("call_context");
            let after_needs = after.array("runtime_dispatch_needs");
            assert_method_context(
                after_calls,
                fixture.owner,
                &callee,
                &fixture.case.status,
                fixture.case.generic_arg_count,
                fixture.case.label,
                endpoint,
            );
            assert_no_runtime_dispatch_need(after_needs, site, fixture.case.label, endpoint);
            assert_eq!(ui_field(after.ui(), "runtime_dispatch_needs"), "0");
        }
    }

    assert_context_ui(tool, &observed, proofs, 1, Some(2), fixture.case.label);
    assert_eq!(
        ui_field(observed.ui(), "runtime_dispatch_needs"),
        runtime_needs.len().to_string()
    );
    assert_eq!(
        ui_field(observed.ui(), "future_poll_field_producer_flows"),
        poll_flows.len().to_string()
    );
}

fn assert_context_ui(
    tool: ContextTool,
    observed: &ContextToolObservation,
    proofs: &[serde_json::Value],
    min_out: usize,
    lookup_min: Option<usize>,
    label: &str,
) {
    let endpoint = endpoint_label(tool);
    assert!(
        ui_field(observed.ui(), "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= min_out,
        "{endpoint} should surface at least {min_out} outgoing rows for {label}"
    );
    match (tool, lookup_min) {
        (ContextTool::Lookup, Some(min)) => assert!(
            ui_field(observed.ui(), "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= min,
            "lookup should surface at least {min} proof rows for {label}"
        ),
        _ => assert_eq!(
            ui_field(observed.ui(), "proof_context"),
            proofs.len().to_string(),
            "{endpoint} proof count for {label}"
        ),
    }
}

#[derive(Clone, Copy)]
enum PathContract {
    NestedOwnerAbsence,
    ShadowedSetup,
    ExternalFrontier,
    CallableObject,
    MacroBoundary,
}

// Source selectors remain on the PathToolCase constants in targetless.rs and
// mirror docs/active/agents/call-graph/2026-06-28_real-corpus-call-site-oracle-matrices.md.
// These read-only contracts reuse one fixture across both endpoints.
// Semantic execution ledger (each entry runs Lookup and Edges):
//   Secret::from_ref nested owner absence: 1 x 2
//   shadowed get setup boundary: 1 x 2
//   Request::builder and std::mem::replace external frontiers: 2 x 2
//   Runner fwd/rev callable trait objects: 2 x 2
//   IntoServiceFuture::new, routing::post, and routing::get_service: 3 x 2
#[tokio::test]
async fn code_item_tools_preserve_nested_owner_path_absence() {
    assert_path_cases(
        PathToolCase::FROM_REF_DEP_ROOT,
        PathContract::NestedOwnerAbsence,
    )
    .await;
}

#[tokio::test]
async fn code_item_tools_preserve_shadowed_get_setup_boundary() {
    assert_path_cases(PathToolCase::SHADOWED_GET, PathContract::ShadowedSetup).await;
}

#[tokio::test]
async fn code_item_tools_return_external_path_frontiers() {
    assert_path_cases(
        PathToolCase::REQUEST_BUILDER_ALIAS,
        PathContract::ExternalFrontier,
    )
    .await;
    assert_path_cases(
        PathToolCase::STD_MEM_REPLACE,
        PathContract::ExternalFrontier,
    )
    .await;
}

#[tokio::test]
async fn code_item_tools_return_callable_trait_object_paths() {
    assert_path_cases(
        PathToolCase::MEMCHR_CALLABLE_TRAIT_OBJECT,
        PathContract::CallableObject,
    )
    .await;
}

#[tokio::test]
async fn code_item_tools_return_generated_macro_boundary_paths() {
    assert_path_cases(
        PathToolCase::INTO_SERVICE_FUTURE_NEW,
        PathContract::MacroBoundary,
    )
    .await;
    assert_path_cases(PathToolCase::ROUTING_POST, PathContract::MacroBoundary).await;
    assert_path_cases(
        PathToolCase::ROUTING_GET_SERVICE,
        PathContract::MacroBoundary,
    )
    .await;
}

async fn assert_path_cases<const N: usize>(cases: [PathToolCase; N], contract: PathContract) {
    for case in cases {
        let fixture = PathToolFixture::new(case).await;
        for tool in ContextTool::ALL {
            assert_path_case(&fixture, contract, tool).await;
        }
    }
}

async fn assert_path_case(fixture: &PathToolFixture, contract: PathContract, tool: ContextTool) {
    let observed = observe_exact_item(
        tool,
        &fixture.state,
        ExactItemQuery {
            name: fixture.case.item,
            file: &fixture.file_path,
            kind: fixture.case.node_kind(),
            module: fixture.module_path_arg(),
            trait_name: fixture.case.owner_trait(),
            type_name: fixture.case.owner_type(),
            label: fixture.case.label,
        },
    )
    .await;
    let calls = observed.array("call_context");
    let proofs = observed.array("proof_context");
    let endpoint = endpoint_label(tool);

    match contract {
        PathContract::NestedOwnerAbsence => {
            assert_path_context_absent(
                calls,
                fixture.owner,
                &fixture.case.callee(),
                fixture.case.label,
                endpoint,
            );
            let _ui = observed.ui();
        }
        PathContract::ShadowedSetup => {
            assert_resolved_path_context_count(
                calls,
                fixture.owner,
                &fixture.case.callee(),
                CallTargetKind::Function,
                2,
                fixture.case.label,
                endpoint,
            );
            assert_macro_blocker_count(
                proofs,
                fixture.owner,
                "macro_expansion_not_available",
                11,
                fixture.case.label,
                endpoint,
            );
            assert!(
                ui_field(observed.ui(), "call_context_outgoing")
                    .parse::<usize>()
                    .expect("outgoing count")
                    >= 2,
                "{endpoint} should surface both shadowed get setup rows"
            );
            assert_eq!(
                ui_field(observed.ui(), "proof_context"),
                proofs.len().to_string(),
                "{endpoint} shadowed get proof count"
            );
        }
        PathContract::ExternalFrontier => {
            assert_external_path_case(fixture, &observed, calls, proofs, tool, endpoint);
        }
        PathContract::CallableObject => {
            let site = assert_path_context(
                calls,
                fixture.owner,
                &fixture.case.callee(),
                &fixture.case.status,
                fixture.case.label,
                endpoint,
            );
            assert_path_blocker_proof(
                proofs,
                fixture.owner,
                site,
                fixture.case.build_domain(),
                "type_resolution_missing",
                fixture.case.label,
                endpoint,
            );
            assert_runtime_dispatch_blocker(proofs, site, fixture.case.label, endpoint);
            assert_path_ui(tool, &observed, proofs, 1, fixture.case.label);
        }
        PathContract::MacroBoundary => {
            assert_macro_path_case(fixture, &observed, calls, proofs, tool, endpoint);
        }
    }
}

fn assert_external_path_case(
    fixture: &PathToolFixture,
    observed: &ContextToolObservation,
    calls: &[serde_json::Value],
    proofs: &[serde_json::Value],
    tool: ContextTool,
    endpoint: &str,
) {
    let needs = observed.array("external_summary_needs");
    let effects = observed.array("call_reach_effects");
    let site = assert_path_context(
        calls,
        fixture.owner,
        &fixture.case.callee(),
        &fixture.case.status,
        fixture.case.label,
        endpoint,
    );
    if let Some(summary) = fixture.case.admitted_external_summary() {
        assert_admitted_external_summary_proof(
            proofs,
            fixture.owner,
            site,
            summary,
            fixture.case.label,
            endpoint,
        );
        assert_no_external_summary_need(needs, site, fixture.case.label, endpoint);
        assert_admitted_external_summary_effect(
            effects,
            fixture.owner,
            site,
            summary,
            fixture.case.label,
            endpoint,
        );
    } else {
        assert_path_blocker_proof(
            proofs,
            fixture.owner,
            site,
            "bd:corpus-axum-call-graph",
            "external_dependency_summary_missing",
            fixture.case.label,
            endpoint,
        );
    }

    assert_path_ui(tool, observed, proofs, 1, fixture.case.label);
    assert_eq!(
        ui_field(observed.ui(), "external_summary_needs"),
        needs.len().to_string(),
        "{endpoint} external summary need count for {}",
        fixture.case.label
    );
    assert_eq!(
        ui_field(observed.ui(), "reach_effects"),
        effects.len().to_string(),
        "{endpoint} reach effect count for {}",
        fixture.case.label
    );
}

fn assert_macro_path_case(
    fixture: &PathToolFixture,
    observed: &ContextToolObservation,
    calls: &[serde_json::Value],
    proofs: &[serde_json::Value],
    tool: ContextTool,
    endpoint: &str,
) {
    let boundary = fixture
        .case
        .admitted_macro_boundary_summary()
        .expect("generated-boundary fixture");
    let reach = observed
        .field("call_reach")
        .as_object()
        .expect("call_reach object");
    if fixture.case.status == CallStatusKind::Resolved {
        let (site, target) = assert_resolved_path_context_target(
            calls,
            fixture.owner,
            &fixture.case.callee(),
            fixture.case.expected_resolved_relation(),
            fixture.case.label,
            endpoint,
        );
        assert_resolved_path_proof(
            proofs,
            fixture.owner,
            site,
            target,
            fixture.case.label,
            endpoint,
        );
        assert_admitted_macro_boundary_summary_proof(
            proofs,
            fixture.owner,
            site,
            boundary,
            fixture.case.label,
            endpoint,
        );
        let direct = assert_resolved_direct_call_site_reach(
            reach,
            fixture.owner,
            &fixture.case,
            target,
            fixture.case.expected_resolved_relation(),
            endpoint,
        );
        assert_eq!(
            ui_field(observed.ui(), "reach_direct_call_sites"),
            direct.to_string(),
            "{endpoint} direct call-site reach count for {}",
            fixture.case.label
        );
    } else {
        let site = assert_path_context(
            calls,
            fixture.owner,
            &fixture.case.callee(),
            &fixture.case.status,
            fixture.case.label,
            endpoint,
        );
        assert_path_resolution_proof(
            proofs,
            fixture.owner,
            site,
            "bd:corpus-axum-call-graph",
            boundary.expected_state,
            boundary
                .expected_blocker
                .expect("blocked macro boundary should carry a blocker"),
            fixture.case.label,
            endpoint,
        );
        assert_admitted_macro_boundary_summary_proof(
            proofs,
            fixture.owner,
            site,
            boundary,
            fixture.case.label,
            endpoint,
        );
        let (field, count, ambiguous) =
            assert_status_frontier_reach(reach, fixture.owner, &fixture.case, endpoint);
        assert_eq!(
            ui_field(observed.ui(), field.as_str()),
            count.to_string(),
            "{endpoint} frontier count for {}",
            fixture.case.label
        );
        assert_eq!(
            ui_field(observed.ui(), "reach_ambiguous_frontier_calls"),
            ambiguous.to_string(),
            "{endpoint} ambiguous frontier count for {}",
            fixture.case.label
        );
    }

    assert_path_ui(tool, observed, proofs, 1, fixture.case.label);
}

fn assert_path_ui(
    tool: ContextTool,
    observed: &ContextToolObservation,
    proofs: &[serde_json::Value],
    min_out: usize,
    label: &str,
) {
    let endpoint = endpoint_label(tool);
    assert!(
        ui_field(observed.ui(), "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= min_out,
        "{endpoint} should surface at least {min_out} outgoing rows for {label}"
    );
    match tool {
        ContextTool::Lookup => assert!(
            ui_field(observed.ui(), "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 2,
            "lookup should surface at least two proof rows for {label}"
        ),
        ContextTool::Edges => assert_eq!(
            ui_field(observed.ui(), "proof_context"),
            proofs.len().to_string(),
            "edges proof count for {label}"
        ),
    }
}

const fn endpoint_label(tool: ContextTool) -> &'static str {
    match tool {
        ContextTool::Lookup => "lookup",
        ContextTool::Edges => "edges",
    }
}

#[tokio::test]
async fn code_item_tools_return_memchr_ifunc_generated_transmute_frontiers() {
    for case in IfuncToolCase::MEMCHR {
        for tool in ContextTool::ALL {
            let fixture = IfuncToolFixture::new(case).await;
            assert_ifunc_case(&fixture, tool).await;
        }
    }
}

async fn assert_ifunc_case(fixture: &IfuncToolFixture, tool: ContextTool) {
    let observed = observe_exact_item(
        tool,
        &fixture.state,
        ExactItemQuery {
            name: fixture.case.item,
            file: &fixture.file_path,
            kind: "function",
            module: fixture.module_path_arg(),
            trait_name: None,
            type_name: None,
            label: fixture.case.label,
        },
    )
    .await;
    let calls = observed.array("call_context");
    let proofs = observed.array("proof_context");
    let unsafe_calls = observed.array("unsafe_block_calls");
    let endpoint = endpoint_label(tool);

    let sites = assert_ifunc_context(
        calls,
        fixture.owner,
        fixture.case.expected_arg_count,
        fixture.case.label,
        endpoint,
    );
    assert_ifunc_proof(
        proofs,
        fixture.owner,
        sites,
        fixture.case.build_domain(),
        fixture.case.label,
        endpoint,
    );
    assert_ifunc_unsafe_calls(
        unsafe_calls,
        fixture.owner,
        sites,
        fixture.case.expected_arg_count,
        fixture.case.label,
        endpoint,
    );

    assert_context_ui(tool, &observed, proofs, 2, Some(4), fixture.case.label);
    assert_eq!(ui_field(observed.ui(), "unsafe_block_calls"), "2");
}

#[tokio::test]
async fn code_item_tools_omit_admitted_memchr_runtime_dispatch_summary_needs() {
    for case in PathToolCase::MEMCHR_CALLABLE_TRAIT_OBJECT {
        for tool in ContextTool::ALL {
            let fixture = PathToolFixture::new(case.clone()).await;
            assert_memchr_summary(&fixture, tool).await;
        }
    }
}

async fn assert_memchr_summary(fixture: &PathToolFixture, tool: ContextTool) {
    let initial = observe_exact_item(tool, &fixture.state, path_query(fixture)).await;
    let calls = initial.array("call_context");
    let runtime_needs = initial.array("runtime_dispatch_needs");
    let endpoint = endpoint_label(tool);
    let site = assert_path_context(
        calls,
        fixture.owner,
        &fixture.case.callee(),
        &fixture.case.status,
        fixture.case.label,
        endpoint,
    );
    assert_runtime_dispatch_need(runtime_needs, site, fixture.case.label, endpoint);

    fixture
        .state
        .db
        .upsert_proof_fact_values(&[
            ploke_test_utils::memchr_callable_trait_object_runtime_dispatch_summary(site),
        ])
        .unwrap_or_else(|err| {
            panic!(
                "{} runtime dispatch summary insert: {err}",
                fixture.case.label
            )
        });

    let after = observe_exact_item(tool, &fixture.state, path_query(fixture)).await;
    let after_calls = after.array("call_context");
    let after_needs = after.array("runtime_dispatch_needs");
    assert_path_context(
        after_calls,
        fixture.owner,
        &fixture.case.callee(),
        &fixture.case.status,
        fixture.case.label,
        endpoint,
    );
    assert_no_runtime_dispatch_need(after_needs, site, fixture.case.label, endpoint);
    assert_eq!(ui_field(after.ui(), "runtime_dispatch_needs"), "0");
}

fn path_query(fixture: &PathToolFixture) -> ExactItemQuery<'_> {
    ExactItemQuery {
        name: fixture.case.item,
        file: &fixture.file_path,
        kind: fixture.case.node_kind(),
        module: fixture.module_path_arg(),
        trait_name: fixture.case.owner_trait(),
        type_name: fixture.case.owner_type(),
        label: fixture.case.label,
    }
}

fn assert_external_summary_need(
    needs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site = site_id.to_string();
    let need = needs
        .iter()
        .find(|need| {
            need.get("call_site")
                .and_then(|call| call.get("site_id"))
                .and_then(serde_json::Value::as_str)
                == Some(site.as_str())
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose an external summary need for {label}: {needs:#?}")
        });
    let reasons = need
        .get("blocker_reasons")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} external summary need reasons missing: {need:#?}"));
    assert!(
        reasons
            .iter()
            .any(|reason| reason.as_str() == Some("external_dependency_summary_missing")),
        "{tool} external summary need should preserve missing-summary blocker for {label}: {need:#?}"
    );
    let paths = need
        .get("paths_to_owner")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} external summary need paths missing: {need:#?}"));
    assert!(
        paths.is_empty(),
        "{tool} direct frontier need should not include intermediate owner paths for {label}: {need:#?}"
    );
}

fn assert_no_external_summary_need(
    needs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site = site_id.to_string();
    assert!(
        needs.iter().all(|need| {
            need.get("call_site")
                .and_then(|call| call.get("site_id"))
                .and_then(serde_json::Value::as_str)
                != Some(site.as_str())
        }),
        "{tool} should not expose a remaining external summary need for admitted {label}: {needs:#?}"
    );
}

fn assert_runtime_dispatch_need(
    needs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site = site_id.to_string();
    let need = needs
        .iter()
        .find(|need| {
            need.get("call_site")
                .and_then(|call| call.get("site_id"))
                .and_then(serde_json::Value::as_str)
                == Some(site.as_str())
        })
        .unwrap_or_else(|| {
            panic!("{tool} should expose a runtime-dispatch need for {label}: {needs:#?}")
        });
    let blockers = need
        .get("blocker_reasons")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} runtime_dispatch_needs.blocker_reasons array"));
    assert!(
        blockers
            .iter()
            .any(|reason| reason.as_str() == Some("dynamic_dispatch_unbounded")),
        "{tool} runtime-dispatch need should preserve dynamic_dispatch_unbounded for {label}: {need:#?}"
    );
    let paths = need
        .get("paths_to_owner")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} runtime_dispatch_needs.paths_to_owner array"));
    assert!(
        paths.is_empty(),
        "{tool} direct runtime-dispatch need should not include intermediate owner paths for {label}: {need:#?}"
    );
}

fn assert_no_runtime_dispatch_need(
    needs: &[serde_json::Value],
    site_id: Uuid,
    label: &str,
    tool: &str,
) {
    let site = site_id.to_string();
    assert!(
        needs.iter().all(|need| {
            need.get("call_site")
                .and_then(|call| call.get("site_id"))
                .and_then(serde_json::Value::as_str)
                != Some(site.as_str())
        }),
        "{tool} should not expose a remaining runtime-dispatch need for admitted {label}: {needs:#?}"
    );
}

fn assert_status_frontier_reach(
    reach: &serde_json::Map<String, serde_json::Value>,
    owner: uuid::Uuid,
    case: &PathToolCase,
    tool: &str,
) -> (String, usize, usize) {
    let bucket = match &case.status {
        CallStatusKind::Unsupported => "unsupported_frontier_calls",
        CallStatusKind::Unresolved => "unresolved_frontier_calls",
        CallStatusKind::Ambiguous => "ambiguous_frontier_calls",
        other => panic!(
            "{tool} generated-boundary reach helper only accepts frontier statuses, got {other:?}"
        ),
    };
    let frontier = reach
        .get(bucket)
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} call_reach {bucket} array"));
    let calls = frontier
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("{tool} {bucket} rows should deserialize: {err}"));
    let call = calls
        .iter()
        .find(|call| {
            call.owner_id == owner
                && call.status == case.status
                && call.targets.is_empty()
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path }
                        if path.iter().map(String::as_str).eq(case.path.iter().copied())
                )
        })
        .unwrap_or_else(|| {
            panic!(
                "{tool} should expose {} in {bucket}: {calls:#?}",
                case.label,
            )
        });
    assert_eq!(call.owner_id, owner);

    let ambiguous = reach
        .get("ambiguous_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} call_reach ambiguous_frontier_calls array"));
    assert!(
        ambiguous.is_empty(),
        "{tool} should not report ambiguous frontier rows for {}: {ambiguous:#?}",
        case.label
    );
    (format!("reach_{bucket}"), calls.len(), ambiguous.len())
}

fn assert_resolved_direct_call_site_reach(
    reach: &serde_json::Map<String, serde_json::Value>,
    owner: uuid::Uuid,
    case: &PathToolCase,
    target: uuid::Uuid,
    relation: CallTargetKind,
    tool: &str,
) -> usize {
    let direct = reach
        .get("direct_call_sites")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} call_reach direct_call_sites array"));
    let calls = direct
        .iter()
        .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
        .collect::<Result<Vec<_>, _>>()
        .unwrap_or_else(|err| panic!("{tool} direct_call_sites rows should deserialize: {err}"));
    assert!(
        calls.iter().any(|call| {
            call.owner_id == owner
                && call.status == CallStatusKind::Resolved
                && matches!(
                    &call.callee,
                    CallCalleeInfo::Path { path }
                        if path.iter().map(String::as_str).eq(case.path.iter().copied())
                )
                && call.targets.len() == 1
                && call.targets[0].target_id == target
                && call.targets[0].relation == relation
        }),
        "{tool} should expose {} as a resolved direct callsite: {calls:#?}",
        case.label
    );
    let unresolved = reach
        .get("unresolved_frontier_calls")
        .and_then(serde_json::Value::as_array)
        .unwrap_or_else(|| panic!("{tool} call_reach unresolved_frontier_calls array"));
    assert!(
        unresolved.iter().all(|call| {
            serde_json::from_value::<CallContextInfo>(call.clone()).map_or(true, |call| {
                !(call.owner_id == owner
                    && matches!(
                        &call.callee,
                        CallCalleeInfo::Path { path }
                            if path.iter().map(String::as_str).eq(case.path.iter().copied())
                    ))
            })
        }),
        "{tool} should not keep resolved {} in unresolved frontier: {unresolved:#?}",
        case.label
    );
    calls.len()
}

fn assert_macro_blocker_count(
    proofs: &[serde_json::Value],
    owner: Uuid,
    reason: &str,
    expected: usize,
    label: &str,
    tool: &str,
) {
    let owner = owner.to_string();
    let rows = proofs
        .iter()
        .filter_map(|proof| {
            serde_json::from_value::<ploke_core::rag_types::ProofContextInfo>(proof.clone()).ok()
        })
        .collect::<Vec<_>>();
    let owner_site_ids = rows
        .iter()
        .filter(|proof| {
            proof.kind == "call_site" && proof.caller_def_id.as_deref() == Some(owner.as_str())
        })
        .filter_map(|proof| proof.call_site_id.as_deref())
        .collect::<Vec<_>>();
    let matching = rows
        .iter()
        .filter(|proof| {
            proof.kind == "call_resolution"
                && proof.resolution_state.as_deref() == Some("blocked")
                && proof.blocker_reason.as_deref() == Some(reason)
                && proof
                    .call_site_id
                    .as_deref()
                    .is_some_and(|site| owner_site_ids.contains(&site))
        })
        .collect::<Vec<_>>();
    assert!(
        matching.len() >= expected,
        "{tool} should expose at least {expected} {reason} macro blocker rows for {label}: {rows:#?}"
    );
    for proof in matching {
        let site = proof
            .call_site_id
            .as_deref()
            .unwrap_or_else(|| panic!("{tool} {reason} proof row should carry call_site_id"));
        assert!(
            rows.iter().any(|row| {
                row.kind == "call_site"
                    && row.caller_def_id.as_deref() == Some(owner.as_str())
                    && row.call_site_id.as_deref() == Some(site)
            }),
            "{tool} should expose the call_site row for {reason} blocker {site} in {label}: {rows:#?}"
        );
        assert!(
            rows.iter().all(|row| {
                row.kind != "call_edge" || row.call_site_id.as_deref() != Some(site)
            }),
            "{tool} must not fabricate call_edge proof rows for {reason} blocker {site} in {label}: {rows:#?}"
        );
    }
}
