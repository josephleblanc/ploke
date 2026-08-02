use std::{borrow::Cow, path::Path, sync::Arc};

use ploke_core::rag_types::{
    CallCalleeInfo, CallContextInfo, CallSiteKind, CallStatusKind, CallTargetKind,
    CrateBoundaryEdgeInfo,
};
use ploke_test_utils::{CallCorpusFixture, CallPipelineCoverage, call_shape_cases};
use ploke_tui::tools::{
    Ctx, Tool, ToolUiPayload,
    code_item_lookup::{CodeItemLookup, LookupParams},
    get_code_edges::{CodeItemEdges, EdgesParams},
};

use crate::call_graph_tool_support::{
    AxumBodyEmptyToolFixture, AxumBodyNewToolFixture, AxumBoxedIntoRouteToolFixture,
    AxumCompositeRejectionToolFixture, AxumFromFnBasicToolFixture,
    AxumGeneratedRejectionToolFixture, AxumHandleErrorCallToolFixture, AxumHandlerCallToolFixture,
    AxumJsonFromBytesToolFixture, AxumParseAttrsToolFixture, AxumRunUiTestsToolFixture,
    AxumTapIoAcceptToolFixture, AxumTapIoConstructorToolFixture, CallGraphToolFixture,
    CallableBlockerFixture, CallableBlockerShape, CallableParamResolvedFixture,
    ChronoAliasConstructorToolFixture, ChronoNaiveUtcToolFixture, ChronoParseInternalToolFixture,
    ContextTool, DirectSelfFieldDispatchFixture, FixtureBranchReceiverToolFixture,
    FixtureDynamicCallableToolFixture, FixtureMethodCallableArgumentToolFixture,
    FixtureSelfFieldReceiverToolFixture, MemchrRunnerRunToolFixture, MemchrRunnerSetterToolFixture,
    SharedCallShapeToolFixture, assert_aliased_parameter_local_binding_payload,
    assert_ambiguous_candidate_proof, assert_ambiguous_dynamic_candidates,
    assert_ambiguous_dynamic_candidates_with_relation, assert_ambiguous_path_candidates,
    assert_body_empty_dependency_root_proof, assert_body_empty_impact_summary,
    assert_body_empty_incoming_context, assert_body_new_generated_incoming_context,
    assert_body_new_impact_summary, assert_body_new_incoming_context,
    assert_boxed_into_route_incoming_context, assert_branch_receiver_context,
    assert_branch_receiver_proof, assert_chrono_naive_utc_incoming_context,
    assert_chrono_typed_setter_candidate_payload, assert_dynamic_context, assert_dynamic_proof,
    assert_expected_path_incoming_context, assert_fixture_extern_c_abs_effects,
    assert_from_fn_basic_body_empty_crate_boundary, assert_generated_rejection_outgoing_context,
    assert_handle_error_returned_future_local_binding_payload,
    assert_handler_call_incoming_context, assert_incoming_context,
    assert_initialized_local_receiver_context, assert_initialized_local_receiver_proof,
    assert_initialized_path_local_binding_payload, assert_json_from_bytes_incoming_context,
    assert_local_function_binding_payload,
    assert_memchr_runner_run_assignment_argument_flow_payload,
    assert_memchr_runner_run_assignment_flow_payload,
    assert_memchr_runner_setter_local_binding_payload,
    assert_method_argument_parameter_local_binding_payload,
    assert_no_external_summary_need_for_site, assert_parse_attrs_incoming_context,
    assert_path_blocker_proof, assert_path_context, assert_path_resolution_proof,
    assert_resolved_callable_param_proof, assert_resolved_dynamic_context,
    assert_resolved_path_context, assert_run_ui_tests_incoming_context,
    assert_runtime_dispatch_blocker, assert_self_field_receiver_context,
    assert_self_field_receiver_proof, assert_serde_json_summary_proof,
    assert_serde_json_surface_measure_effect,
    assert_tap_io_accept_self_field_parameter_flow_payload,
    assert_tap_io_constructor_local_binding_payload, assert_target_proof,
    assert_typed_setter_local_binding_payload, fixture_graph_db, observe_function, observe_method,
    ui_field,
};

struct EndpointQuery<'a> {
    name: &'a str,
    file: &'a Path,
    kind: &'a str,
    module: String,
    trait_name: Option<&'a str>,
    type_name: Option<&'a str>,
    body: Option<&'a str>,
    label: &'a str,
}

impl<'a> EndpointQuery<'a> {
    fn item(name: &'a str, file: &'a Path, kind: &'a str, module: String) -> Self {
        Self {
            name,
            file,
            kind,
            module,
            trait_name: None,
            type_name: None,
            body: None,
            label: name,
        }
    }

    fn function(name: &'a str, file: &'a Path) -> Self {
        Self {
            name,
            file,
            kind: "function",
            module: "crate".to_string(),
            trait_name: None,
            type_name: None,
            body: None,
            label: name,
        }
    }

    fn method(name: &'a str, file: &'a Path, type_name: &'a str) -> Self {
        Self {
            name,
            file,
            kind: "method",
            module: "crate".to_string(),
            trait_name: None,
            type_name: Some(type_name),
            body: None,
            label: name,
        }
    }
}

struct EndpointObservation {
    tool: ContextTool,
    payload: serde_json::Value,
    ui: ToolUiPayload,
}

impl EndpointObservation {
    fn label(&self) -> &'static str {
        self.tool.label()
    }

    fn node(&self) -> &serde_json::Value {
        match self.tool {
            ContextTool::Lookup => &self.payload,
            ContextTool::Edges => self
                .payload
                .get("node_info")
                .expect("code_item_edges payload node_info"),
        }
    }

    fn field(&self, name: &str) -> &serde_json::Value {
        self.node()
            .get(name)
            .unwrap_or_else(|| panic!("{} payload missing {name}", self.label()))
    }

    fn array(&self, name: &str) -> &[serde_json::Value] {
        self.field(name)
            .as_array()
            .unwrap_or_else(|| panic!("{} payload {name} array", self.label()))
    }

    fn root_array(&self, name: &str) -> &[serde_json::Value] {
        self.payload
            .get(name)
            .and_then(serde_json::Value::as_array)
            .unwrap_or_else(|| panic!("{} root payload {name} array", self.label()))
    }

    fn edge_summary(&self, name: &str) -> usize {
        assert_eq!(self.tool, ContextTool::Edges);
        self.payload
            .get("call_graph_summary")
            .and_then(|summary| summary.get(name))
            .and_then(serde_json::Value::as_u64)
            .unwrap_or_else(|| panic!("missing call_graph_summary.{name}: {:#?}", self.payload))
            as usize
    }

    fn ui(&self) -> &ToolUiPayload {
        &self.ui
    }
}

async fn observe_endpoint(
    tool: ContextTool,
    query: EndpointQuery<'_>,
    ctx: Ctx,
) -> EndpointObservation {
    let file = query.file.display().to_string();
    let result = match tool {
        ContextTool::Lookup => {
            CodeItemLookup::execute(
                LookupParams {
                    item_name: Cow::Owned(query.name.to_string()),
                    file_path: Cow::Owned(file),
                    node_kind: Cow::Owned(query.kind.to_string()),
                    module_path: Cow::Owned(query.module),
                    owner_trait: query.trait_name.map(|name| Cow::Owned(name.to_string())),
                    owner_type: query.type_name.map(|name| Cow::Owned(name.to_string())),
                    parent_name: None,
                    body_contains: query.body.map(|body| Cow::Owned(body.to_string())),
                    allowed_effects: Vec::new(),
                },
                ctx,
            )
            .await
        }
        ContextTool::Edges => {
            CodeItemEdges::execute(
                EdgesParams {
                    item_name: Cow::Owned(query.name.to_string()),
                    file_path: Cow::Owned(file),
                    node_kind: Cow::Owned(query.kind.to_string()),
                    module_path: Cow::Owned(query.module),
                    owner_trait: query.trait_name.map(|name| Cow::Owned(name.to_string())),
                    owner_type: query.type_name.map(|name| Cow::Owned(name.to_string())),
                    parent_name: None,
                    body_contains: query.body.map(|body| Cow::Owned(body.to_string())),
                    allowed_effects: Vec::new(),
                },
                ctx,
            )
            .await
        }
    }
    .unwrap_or_else(|err| panic!("{} {}: {err}", tool.label(), query.label));
    let payload = serde_json::from_str(&result.content)
        .unwrap_or_else(|err| panic!("{} {} payload: {err}", tool.label(), query.label));
    let ui = result
        .ui_payload
        .unwrap_or_else(|| panic!("{} {} UI payload", tool.label(), query.label));
    EndpointObservation { tool, payload, ui }
}

// Endpoint-parity ledger: every semantic selector below executes once through
// Lookup and once through Edges. Fixtures are recreated per endpoint, which
// preserves isolation for seeded extern-C effects as well as read-only cases.
// The exact fixture selectors are `call_crate_local_target`, `unsafe_target`,
// `make_ready_local_assoc`, `call_extern_c_function`, and
// `recursive_fixture_call`, all in fixture_call_graph/src/lib.rs. Their source
// contracts are the async target/caller at 581-586, unsafe target/caller at
// 766-770, extern-C `abs` declaration/call at 839-844, and the direct recursive
// self-call in `recursive_fixture_call`.
#[tokio::test]
async fn code_item_tools_return_call_and_proof_context() {
    for tool in ContextTool::ALL {
        let fixture = CallGraphToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::function("call_crate_local_target", &fixture.file_path),
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        let owner = fixture.owner.to_string();
        assert!(
            calls.iter().any(|call| {
                call.get("owner_id").and_then(serde_json::Value::as_str) == Some(owner.as_str())
                    && call.get("kind").and_then(serde_json::Value::as_str) == Some("path")
                    && call
                        .get("targets")
                        .and_then(serde_json::Value::as_array)
                        .is_some_and(|targets| !targets.is_empty())
            }),
            "{} should return node-scoped call context: {calls:#?}",
            observed.label()
        );
        assert!(
            proofs.iter().any(|proof| {
                proof.get("kind").and_then(serde_json::Value::as_str) == Some("call_edge")
                    && proof
                        .get("caller_def_id")
                        .and_then(serde_json::Value::as_str)
                        == Some(owner.as_str())
            }),
            "{} should return node-scoped proof context: {proofs:#?}",
            observed.label()
        );
        assert_eq!(
            ui_field(observed.ui(), "call_context"),
            calls.len().to_string()
        );
        assert_eq!(
            ui_field(observed.ui(), "proof_context"),
            proofs.len().to_string()
        );
        assert!(
            ui_field(observed.ui(), "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "{} should surface outgoing call context",
            observed.label()
        );
    }
}

#[derive(Clone, Copy)]
struct ImpactCase {
    item: &'static str,
    field: &'static str,
    caller: &'static str,
    caller_value: bool,
}

const IMPACT_CASES: [ImpactCase; 2] = [
    ImpactCase {
        item: "unsafe_target",
        field: "is_unsafe",
        caller: "call_unsafe_function",
        caller_value: false,
    },
    ImpactCase {
        item: "make_ready_local_assoc",
        field: "is_async",
        caller: "call_await_result_instance_method",
        caller_value: true,
    },
];

#[tokio::test]
async fn code_item_tools_mark_target_call_impact() {
    for case in IMPACT_CASES {
        for tool in ContextTool::ALL {
            let fixture = CallGraphToolFixture::new().await;
            let observed = observe_endpoint(
                tool,
                EndpointQuery::function(case.item, &fixture.file_path),
                fixture.ctx(tool.label()),
            )
            .await;
            let impact = observed
                .field("call_impact")
                .as_object()
                .expect("call_impact object");
            let target = impact
                .get("target")
                .and_then(serde_json::Value::as_object)
                .expect("call_impact target object");
            assert_eq!(
                target.get(case.field).and_then(serde_json::Value::as_bool),
                Some(true),
                "{} should serialize {} metadata: {impact:#?}",
                observed.label(),
                case.field
            );
            let callers = impact
                .get("direct_callers")
                .and_then(serde_json::Value::as_array)
                .expect("call_impact direct_callers array");
            assert!(
                callers.iter().any(|caller| {
                    caller.get("name").and_then(serde_json::Value::as_str) == Some(case.caller)
                        && caller.get(case.field).and_then(serde_json::Value::as_bool)
                            == Some(case.caller_value)
                }),
                "{} should preserve direct caller metadata: {callers:#?}",
                observed.label()
            );
            if tool == ContextTool::Edges {
                assert_eq!(ui_field(observed.ui(), "impact_direct_callers"), "1");
            }
        }
    }
}

#[tokio::test]
async fn code_item_tools_surface_extern_c_effects() {
    for tool in ContextTool::ALL {
        let fixture = CallGraphToolFixture::new().await;
        let expected = fixture.seed_extern_c_abs_effect();
        let observed = observe_endpoint(
            tool,
            EndpointQuery::function("call_extern_c_function", &fixture.file_path),
            fixture.ctx(tool.label()),
        )
        .await;
        let effects = observed.array("call_reach_effects");
        assert_fixture_extern_c_abs_effects(effects, &expected, observed.label());
        assert_eq!(
            ui_field(observed.ui(), "reach_effects"),
            effects.len().to_string()
        );

        if tool == ContextTool::Edges {
            let reach = observed
                .field("call_reach")
                .as_object()
                .expect("call_reach object");
            let paths = reach
                .get("paths")
                .and_then(serde_json::Value::as_array)
                .expect("call_reach paths array");
            let callees = reach
                .get("callees")
                .and_then(serde_json::Value::as_array)
                .expect("call_reach callees array");
            assert!(
                paths.is_empty() && callees.is_empty(),
                "extern C calls should not fabricate local reach: {reach:#?}"
            );
            let frontier = reach
                .get("external_frontier_calls")
                .and_then(serde_json::Value::as_array)
                .expect("external_frontier_calls array")
                .iter()
                .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
                .collect::<Result<Vec<_>, _>>()
                .expect("typed external frontier rows");
            let abs_call = frontier
                .iter()
                .find(|call| {
                    call.kind == CallSiteKind::Path
                        && call.status == CallStatusKind::External
                        && call.targets.is_empty()
                        && call.callee
                            == CallCalleeInfo::Path {
                                path: vec!["abs".to_string()],
                            }
                })
                .unwrap_or_else(|| panic!("extern C abs frontier row: {frontier:#?}"));
            assert_eq!(abs_call.arg_count, Some(1));
            let files = reach
                .get("source_files")
                .and_then(serde_json::Value::as_array)
                .expect("call_reach source_files array");
            assert_source_file(files, "fixture_call_graph/src/lib.rs", observed.label());
            assert_eq!(
                ui_field(observed.ui(), "reach_external_frontier_calls"),
                "1"
            );
        }
    }
}

fn assert_source_file(files: &[serde_json::Value], suffix: &str, label: &str) {
    assert!(
        files
            .iter()
            .filter_map(serde_json::Value::as_str)
            .any(|path| path.ends_with(suffix)),
        "{label} should include source file ending with {suffix:?}: {files:#?}"
    );
}

#[tokio::test]
async fn code_item_tools_return_recursive_cycle_paths() {
    for tool in ContextTool::ALL {
        let fixture = CallGraphToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::function("recursive_fixture_call", &fixture.file_path),
            fixture.ctx(tool.label()),
        )
        .await;
        let owner = observed.field("id").as_str().expect("resolved item id");
        let cycles = observed.root_array("call_cycles_from_owner");
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
        assert_eq!(ui_field(observed.ui(), "call_cycles_from_owner"), "1");
    }
}

// Source and selector ledger for the payload matrices below:
// - fixture_call_graph/src/lib.rs:185-187, 1447-1453, 2410-2429 cover the
//   initialized, local-item, aliased-field, and method-argument bindings.
// - axum/src/serve/listener.rs:116-123 and 236 cover TapIo construction and
//   `accept`; axum/src/error_handling/mod.rs:140-147 covers HandleError::call.
// - memchr/src/tests/substring/mod.rs:94,110,133-154 covers Runner assignment
//   and argument flow; fixture_call_graph/src/lib.rs:1701-1714 and 1834-1845
//   covers non-awaited async closures and branch-initialized receivers.
// - `call_if_expression_receiver_method` / `call_match_expression_receiver_method`
//   cover direct branch receivers; NestedSelfFieldAssocOwner's
//   `call_nested_self_field_instance_method` covers `self.inner.value`.
// Contracts remain exact payload/proof rows, resolved receiver targets, flow
// carriers, endpoint UI counts, and Edges-only blocker summaries/frontiers.
// Exact body selectors are kept on the EndpointQuery values below.
#[derive(Clone, Copy)]
enum BindingCase {
    Initialized,
    LocalFunction,
    AliasedField,
}

impl BindingCase {
    fn owner(self) -> &'static str {
        match self {
            Self::Initialized => "call_local_function_item_binding",
            Self::LocalFunction => "local_fn_body_call_is_not_outer_call_site",
            Self::AliasedField => "call_single_aliased_named_field_function_param",
        }
    }
}

#[tokio::test]
async fn code_item_tools_return_dynamic_local_binding_payloads() {
    for case in [
        BindingCase::Initialized,
        BindingCase::LocalFunction,
        BindingCase::AliasedField,
    ] {
        for tool in ContextTool::ALL {
            let fixture = FixtureDynamicCallableToolFixture::new_for_owner(case.owner()).await;
            let observed = observe_endpoint(
                tool,
                EndpointQuery::function(fixture.owner_name, &fixture.file_path),
                fixture.ctx(tool.label()),
            )
            .await;
            let bindings = observed.array("local_bindings");
            let edges = observed.array("local_binding_edges");
            match case {
                BindingCase::Initialized => assert_initialized_path_local_binding_payload(
                    bindings,
                    edges,
                    fixture.owner,
                    fixture.target,
                    fixture.owner_name,
                    observed.label(),
                ),
                BindingCase::LocalFunction => assert_local_function_binding_payload(
                    bindings,
                    edges,
                    fixture.owner,
                    fixture.owner_name,
                    observed.label(),
                ),
                BindingCase::AliasedField => assert_aliased_parameter_local_binding_payload(
                    bindings,
                    edges,
                    fixture.owner,
                    fixture.owner_name,
                    observed.label(),
                ),
            }
            assert_binding_ui(&observed, bindings, edges);
        }
    }
}

#[tokio::test]
async fn code_item_tools_return_method_argument_binding_payload() {
    for tool in ContextTool::ALL {
        let fixture = FixtureMethodCallableArgumentToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::method(fixture.owner_name, &fixture.file_path, fixture.owner_type),
            fixture.ctx(tool.label()),
        )
        .await;
        let bindings = observed.array("local_bindings");
        let edges = observed.array("local_binding_edges");
        assert_method_argument_parameter_local_binding_payload(
            bindings,
            edges,
            fixture.owner,
            fixture.target,
            fixture.parameter,
            fixture.method_call_site,
            "LocalAssoc::call_function_pointer_param",
            observed.label(),
        );
        assert_binding_ui(&observed, bindings, edges);
    }
}

fn assert_binding_ui(
    observed: &EndpointObservation,
    bindings: &[serde_json::Value],
    edges: &[serde_json::Value],
) {
    assert_eq!(
        ui_field(observed.ui(), "local_bindings"),
        bindings.len().to_string()
    );
    assert_eq!(
        ui_field(observed.ui(), "local_binding_edges"),
        edges.len().to_string()
    );
}

#[tokio::test]
async fn code_item_tools_return_tap_io_constructor_payload() {
    for tool in ContextTool::ALL {
        let fixture = AxumTapIoConstructorToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery {
                name: "tap_io",
                file: &fixture.file_path,
                kind: "method",
                module: fixture.module_path_arg(),
                trait_name: Some("ListenerExt"),
                type_name: None,
                body: None,
                label: "TapIo constructor frontier",
            },
            fixture.ctx(tool.label()),
        )
        .await;
        let bindings = observed.array("local_bindings");
        let edges = observed.array("local_binding_edges");
        assert_tap_io_constructor_local_binding_payload(
            bindings,
            edges,
            fixture.owner,
            observed.label(),
        );
        assert_binding_ui(&observed, bindings, edges);
    }
}

#[tokio::test]
async fn code_item_tools_return_memchr_setter_payload() {
    for tool in ContextTool::ALL {
        let fixture = MemchrRunnerSetterToolFixture::new_fwd().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery {
                name: "fwd",
                file: &fixture.file_path,
                kind: "method",
                module: fixture.module_path_arg(),
                trait_name: None,
                type_name: Some("Runner"),
                body: Some("self.fwd = Some(Box::new(search));"),
                label: "memchr Runner::fwd setter",
            },
            fixture.ctx(tool.label()),
        )
        .await;
        let bindings = observed.array("local_bindings");
        let edges = observed.array("local_binding_edges");
        assert_memchr_runner_setter_local_binding_payload(
            bindings,
            edges,
            fixture.owner,
            fixture.field_name,
            observed.label(),
        );
        assert_binding_ui(&observed, bindings, edges);
    }
}

#[tokio::test]
async fn code_item_tools_return_memchr_assignment_flows() {
    for tool in ContextTool::ALL {
        let fixture = MemchrRunnerRunToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery {
                name: "run",
                file: &fixture.file_path,
                kind: "method",
                module: fixture.module_path_arg(),
                trait_name: None,
                type_name: Some("Runner"),
                body: Some("fwd(t.haystack.as_bytes(), t.needle.as_bytes())"),
                label: "memchr Runner::run assignment flow",
            },
            fixture.ctx(tool.label()),
        )
        .await;
        let flows = observed.array("self_field_assignment_flows");
        let args = observed.array("self_field_assignment_argument_flows");
        assert_memchr_runner_run_assignment_flow_payload(flows, fixture.owner, observed.label());
        assert_memchr_runner_run_assignment_argument_flow_payload(
            args,
            fixture.owner,
            observed.label(),
        );
        assert_eq!(ui_field(observed.ui(), "self_field_assignment_flows"), "3");
        assert_eq!(
            ui_field(observed.ui(), "self_field_assignment_argument_flows"),
            args.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_tools_return_tap_io_accept_flow() {
    for tool in ContextTool::ALL {
        let fixture = AxumTapIoAcceptToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery {
                name: "accept",
                file: &fixture.file_path,
                kind: "method",
                module: fixture.module_path_arg(),
                trait_name: Some("Listener"),
                type_name: Some("TapIo"),
                body: Some("(self.tap_fn)(&mut io)"),
                label: "TapIo::accept parameter flow",
            },
            fixture.ctx(tool.label()),
        )
        .await;
        let flows = observed.array("self_field_parameter_flows");
        assert_tap_io_accept_self_field_parameter_flow_payload(
            flows,
            fixture.owner,
            observed.label(),
        );
        assert_eq!(ui_field(observed.ui(), "self_field_parameter_flows"), "1");
    }
}

#[tokio::test]
async fn code_item_tools_return_handle_error_producer_payload() {
    for tool in ContextTool::ALL {
        let fixture = AxumHandleErrorCallToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery {
                name: "call",
                file: &fixture.file_path,
                kind: "method",
                module: fixture.module_path_arg(),
                trait_name: Some("Service<Request>"),
                type_name: Some("HandleError"),
                body: Some(AxumHandleErrorCallToolFixture::BODY_MARKER),
                label: "body-filtered HandleError::call producer",
            },
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let bindings = observed.array("local_bindings");
        let edges = observed.array("local_binding_edges");
        assert_handle_error_returned_future_local_binding_payload(
            calls,
            bindings,
            edges,
            fixture.owner,
            observed.label(),
        );
        assert_binding_ui(&observed, bindings, edges);
    }
}

#[derive(Clone, Copy)]
enum BranchCase {
    Expression,
    Initialized,
}

#[tokio::test]
async fn code_item_tools_return_branch_receiver_context() {
    assert_branch_cases(
        BranchCase::Expression,
        [
            "call_if_expression_receiver_method",
            "call_match_expression_receiver_method",
        ],
    )
    .await;
    assert_branch_cases(
        BranchCase::Initialized,
        [
            "call_if_initialized_local_instance_method",
            "call_match_initialized_local_instance_method",
        ],
    )
    .await;
}

async fn assert_branch_cases(case: BranchCase, owners: [&'static str; 2]) {
    for owner in owners {
        for tool in ContextTool::ALL {
            let fixture = FixtureBranchReceiverToolFixture::new_for_owner(owner).await;
            let observed = observe_endpoint(
                tool,
                EndpointQuery::function(fixture.owner_name, &fixture.file_path),
                fixture.ctx(tool.label()),
            )
            .await;
            let calls = observed.array("call_context");
            let proofs = observed.array("proof_context");
            match case {
                BranchCase::Expression => {
                    let call = assert_branch_receiver_context(calls, &fixture, observed.label());
                    assert_branch_receiver_proof(proofs, &fixture, call.site_id, observed.label());
                }
                BranchCase::Initialized => {
                    let call = assert_initialized_local_receiver_context(
                        calls,
                        &fixture,
                        observed.label(),
                    );
                    assert_initialized_local_receiver_proof(
                        proofs,
                        &fixture,
                        call.site_id,
                        observed.label(),
                    );
                }
            }
            assert_call_proof_ui(&observed, proofs);
        }
    }
}

#[tokio::test]
async fn code_item_tools_return_nested_self_field_context() {
    for tool in ContextTool::ALL {
        let fixture = FixtureSelfFieldReceiverToolFixture::nested_self_field().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::method(fixture.owner_name, &fixture.file_path, fixture.owner_type),
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        let call = assert_self_field_receiver_context(calls, &fixture, observed.label());
        assert_self_field_receiver_proof(proofs, &fixture, call.site_id, observed.label());
        assert_call_proof_ui(&observed, proofs);
    }
}

#[tokio::test]
async fn code_item_tools_return_non_awaited_async_closure_blockers() {
    for tool in ContextTool::ALL {
        let fixture = CallGraphToolFixture::new().await;
        for (owner, label) in [
            (
                "call_async_closure_binding_without_await_with_body_call",
                "non-awaited async closure binding",
            ),
            (
                "call_async_closure_future_binding_without_await_with_body_call",
                "unawaited async closure future binding",
            ),
        ] {
            let expected = fixture.async_closure_blocker(owner);
            let observed = observe_endpoint(
                tool,
                EndpointQuery::function(owner, &fixture.file_path),
                fixture.ctx(tool.label()),
            )
            .await;
            let calls = observed.array("call_context");
            let proofs = observed.array("proof_context");
            let callee = CallCalleeInfo::Path {
                path: expected.path.clone(),
            };
            let site = assert_path_context(
                calls,
                expected.owner,
                &callee,
                &CallStatusKind::Unsupported,
                label,
                observed.label(),
            );
            assert_eq!(site, expected.site);
            assert_path_blocker_proof(
                proofs,
                expected.owner,
                site,
                "bd:fixture-call-graph",
                "type_resolution_missing",
                label,
                observed.label(),
            );
            assert_runtime_dispatch_blocker(proofs, site, label, observed.label());
            if tool == ContextTool::Edges {
                assert!(
                    observed.edge_summary("blocked") >= 1,
                    "code_item_edges should count the targetless async closure row"
                );
            }
            assert_call_proof_ui(&observed, proofs);
        }
    }
}

fn assert_call_proof_ui(observed: &EndpointObservation, proofs: &[serde_json::Value]) {
    assert!(
        ui_field(observed.ui(), "call_context_outgoing")
            .parse::<usize>()
            .expect("outgoing count")
            >= 1,
        "{} should surface outgoing call context",
        observed.label()
    );
    assert_eq!(
        ui_field(observed.ui(), "proof_context"),
        proofs.len().to_string()
    );
}

// Remaining real-corpus parity ledger. Selectors and source contracts:
// - fixture_call_graph `local_target`; axum middleware/from_fn.rs:394,411
//   `basic` -> axum-core Body::empty at body.rs:52.
// - axum-core body.rs:46,52,110-138 covers Body::new/empty callers.
// - axum-core macros.rs:30-115,154-180 plus extract/rejection.rs:42-100
//   covers MissingExtension and QueryRejection generated delegates.
// Each case recreates its fixture for Lookup and Edges independently.
#[tokio::test]
async fn code_item_tools_return_incoming_callers() {
    for tool in ContextTool::ALL {
        let fixture = CallGraphToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::function("local_target", &fixture.file_path),
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        assert_incoming_context(calls, fixture.owner, fixture.target, observed.label());
        assert_target_proof(proofs, fixture.owner, fixture.target, observed.label());
        assert_eq!(
            ui_field(observed.ui(), "call_context"),
            calls.len().to_string()
        );
        assert_eq!(
            ui_field(observed.ui(), "proof_context"),
            proofs.len().to_string()
        );
        assert!(
            ui_field(observed.ui(), "call_context_incoming")
                .parse::<usize>()
                .expect("incoming count")
                >= 1,
            "{} should surface incoming caller count",
            observed.label()
        );
    }
}

#[tokio::test]
async fn code_item_tools_return_crate_boundary_edges() {
    for tool in ContextTool::ALL {
        let fixture = AxumFromFnBasicToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::item(
                "basic",
                &fixture.file_path,
                "function",
                fixture.module_path_arg(),
            ),
            fixture.ctx(tool.label()),
        )
        .await;
        let edges = observed
            .array("crate_boundary_edges")
            .iter()
            .map(|edge| serde_json::from_value::<CrateBoundaryEdgeInfo>(edge.clone()))
            .collect::<Result<Vec<_>, _>>()
            .expect("typed crate-boundary rows");
        assert_from_fn_basic_body_empty_crate_boundary(
            &edges,
            fixture.owner,
            fixture.body_empty_target,
            observed.label(),
        );
        assert_eq!(
            ui_field(observed.ui(), "crate_boundary_edges"),
            edges.len().to_string()
        );
    }
}

#[tokio::test]
async fn code_item_tools_return_body_empty_callers() {
    for tool in ContextTool::ALL {
        let fixture = AxumBodyEmptyToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::item(
                "empty",
                &fixture.file_path,
                "method",
                fixture.module_path_arg(),
            ),
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        let impact = observed
            .field("call_impact")
            .as_object()
            .expect("call_impact object");
        let source_crates = impact
            .get("source_crates")
            .and_then(serde_json::Value::as_array)
            .expect("call_impact source_crates array");

        assert_body_empty_incoming_context(
            calls,
            &fixture.callers,
            fixture.target,
            observed.label(),
        );
        assert_body_empty_impact_summary(impact, observed.label());
        for caller in &fixture.callers {
            assert_target_proof(proofs, caller.owner, fixture.target, observed.label());
        }
        assert_body_empty_dependency_root_proof(
            proofs,
            &fixture.dependency_root_sites,
            fixture.target,
            observed.label(),
        );

        if tool == ContextTool::Lookup {
            let callers = impact
                .get("callers")
                .and_then(serde_json::Value::as_array)
                .expect("call_impact callers array");
            let tests = impact
                .get("test_callers")
                .and_then(serde_json::Value::as_array)
                .expect("call_impact test_callers array");
            let non_tests = impact
                .get("non_test_callers")
                .and_then(serde_json::Value::as_array)
                .expect("call_impact non_test_callers array");
            assert_eq!(
                tests.len() + non_tests.len(),
                callers.len(),
                "lookup test/non-test impact buckets should partition callers: {impact:#?}"
            );
            assert!(
                !non_tests.is_empty(),
                "Body::empty should expose non-test callers: {impact:#?}"
            );
            assert_eq!(
                ui_field(observed.ui(), "impact_test_callers"),
                tests.len().to_string()
            );
            assert_eq!(
                ui_field(observed.ui(), "impact_non_test_callers"),
                non_tests.len().to_string()
            );
        }

        assert_eq!(ui_field(observed.ui(), "call_context_incoming"), "23");
        assert_eq!(
            ui_field(observed.ui(), "impact_source_crates"),
            source_crates.len().to_string()
        );
        assert!(
            ui_field(observed.ui(), "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= fixture.callers.len(),
            "{} should surface Body::empty proof rows",
            observed.label()
        );
    }
}

#[tokio::test]
async fn code_item_tools_return_body_new_generated_callers() {
    for tool in ContextTool::ALL {
        let fixture = AxumBodyNewToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::item(
                "new",
                &fixture.file_path,
                "method",
                fixture.module_path_arg(),
            ),
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        let impact = observed
            .field("call_impact")
            .as_object()
            .expect("call_impact object");
        assert_body_new_incoming_context(calls, &fixture.callers, fixture.target, observed.label());
        assert_body_new_generated_incoming_context(
            calls,
            &fixture.generated_callers,
            fixture.target,
            observed.label(),
        );
        assert_body_new_impact_summary(
            impact,
            &fixture.generated_callers,
            fixture.target,
            observed.label(),
        );
        for caller in &fixture.generated_callers {
            assert_target_proof(proofs, caller.owner, fixture.target, observed.label());
        }
        if tool == ContextTool::Edges {
            assert_eq!(
                ui_field(observed.ui(), "call_context_incoming"),
                fixture.callers.len().to_string()
            );
            assert!(
                ui_field(observed.ui(), "proof_context")
                    .parse::<usize>()
                    .expect("proof count")
                    >= fixture.generated_callers.len(),
                "code_item_edges should surface generated Body::new proof rows"
            );
        }
    }
}

#[tokio::test]
async fn code_item_tools_return_generated_rejection_calls() {
    for tool in ContextTool::ALL {
        let fixture = AxumGeneratedRejectionToolFixture::new().await;
        let mut query = EndpointQuery::item(
            "into_response",
            &fixture.file_path,
            "method",
            fixture.module_path_arg(),
        );
        query.trait_name = Some("IntoResponse");
        query.type_name = Some("MissingExtension");
        let observed = observe_endpoint(tool, query, fixture.ctx(tool.label())).await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        let owner = fixture.owner.to_string();
        assert_eq!(
            observed.field("id").as_str(),
            Some(owner.as_str()),
            "{} should resolve MissingExtension::into_response",
            observed.label()
        );
        assert_generated_rejection_outgoing_context(calls, &fixture.calls, observed.label());
        for call in &fixture.calls {
            assert_target_proof(proofs, call.owner, call.target, observed.label());
        }
        assert!(
            ui_field(observed.ui(), "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= fixture.calls.len(),
            "{} should surface generated rejection calls",
            observed.label()
        );
        assert!(
            ui_field(observed.ui(), "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= fixture.calls.len(),
            "{} should surface generated rejection proofs",
            observed.label()
        );
    }

    for tool in ContextTool::ALL {
        let fixture = AxumCompositeRejectionToolFixture::new().await;
        let mut query = EndpointQuery::item(
            "into_response",
            &fixture.file_path,
            "method",
            fixture.module_path_arg(),
        );
        query.trait_name = Some("IntoResponse");
        query.type_name = Some("QueryRejection");
        let observed = observe_endpoint(tool, query, fixture.ctx(tool.label())).await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        let owner = fixture.owner.to_string();
        assert_eq!(
            observed.field("id").as_str(),
            Some(owner.as_str()),
            "{} should resolve QueryRejection::into_response",
            observed.label()
        );
        assert_generated_rejection_outgoing_context(
            calls,
            std::slice::from_ref(&fixture.call),
            observed.label(),
        );
        assert_target_proof(
            proofs,
            fixture.call.owner,
            fixture.call.target,
            observed.label(),
        );
        assert!(
            ui_field(observed.ui(), "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "{} should surface composite rejection call",
            observed.label()
        );
        assert!(
            ui_field(observed.ui(), "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= 1,
            "{} should surface composite rejection proof",
            observed.label()
        );
    }
}

// axum-macros attr_parsing.rs:22,59 and its eleven callers pin parse_attrs plus
// the Lookup-only `std::any::type_name::<K>` turbofish row. axum json.rs:
// 112,128,164,184 and lib.rs:488-489 pin Json::from_bytes, its admitted serde
// summary, external frontier, surface-measure effect, and inherited cfg.
#[tokio::test]
async fn code_item_tools_return_parse_attrs_callers() {
    for tool in ContextTool::ALL {
        let fixture = AxumParseAttrsToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::item(
                "parse_attrs",
                &fixture.file_path,
                "function",
                fixture.module_path_arg(),
            ),
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        assert_parse_attrs_incoming_context(
            calls,
            &fixture.callers,
            fixture.target,
            observed.label(),
        );
        for caller in &fixture.callers {
            assert_target_proof(proofs, caller.owner, fixture.target, observed.label());
        }
        assert_eq!(
            ui_field(observed.ui(), "call_context_incoming"),
            fixture.callers.len().to_string()
        );
        assert!(
            ui_field(observed.ui(), "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= fixture.callers.len(),
            "{} should surface parse_attrs proof rows",
            observed.label()
        );

        if tool == ContextTool::Lookup {
            let turbofish = observe_endpoint(
                tool,
                EndpointQuery::item(
                    "parse_parenthesized_attribute",
                    &fixture.file_path,
                    "function",
                    fixture.module_path_arg(),
                ),
                fixture.ctx("axum-type-name-turbofish-lookup"),
            )
            .await;
            let rows = turbofish
                .array("call_context")
                .iter()
                .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
                .collect::<Result<Vec<_>, _>>()
                .expect("typed turbofish call_context rows");
            let type_name = rows
                .iter()
                .find(|call| {
                    call.kind == CallSiteKind::Path
                        && matches!(
                            &call.callee,
                            CallCalleeInfo::Path { path }
                                if path.iter().map(String::as_str).eq(["std", "any", "type_name"])
                        )
                })
                .unwrap_or_else(|| panic!("std::any::type_name::<K> row: {rows:#?}"));
            assert_eq!(type_name.status, CallStatusKind::External);
            assert_eq!(type_name.generic_arg_count, Some(1));
            assert!(
                type_name.targets.is_empty(),
                "external type_name::<K> should remain targetless: {type_name:#?}"
            );
        }
    }
}

#[tokio::test]
async fn code_item_tools_return_json_from_bytes_callers() {
    for tool in ContextTool::ALL {
        let fixture = AxumJsonFromBytesToolFixture::new().await;
        let serde_site = fixture.admit_serde_summary();
        let observed = observe_endpoint(
            tool,
            EndpointQuery::item(
                "from_bytes",
                &fixture.file_path,
                "method",
                fixture.module_path_arg(),
            ),
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        let needs = observed.array("external_summary_needs");
        let effects = observed.array("call_reach_effects");
        let reach = observed
            .field("call_reach")
            .as_object()
            .expect("call_reach object");
        let external = reach
            .get("external_frontier_calls")
            .and_then(serde_json::Value::as_array)
            .expect("external_frontier_calls array");
        let cfgs = reach
            .get("source_cfgs")
            .and_then(serde_json::Value::as_array)
            .expect("source_cfgs array");

        assert_json_from_bytes_incoming_context(
            calls,
            &fixture.callers,
            fixture.target,
            observed.label(),
        );
        for caller in &fixture.callers {
            assert_target_proof(proofs, caller.owner, fixture.target, observed.label());
        }
        let external_calls = external
            .iter()
            .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
            .collect::<Result<Vec<_>, _>>()
            .expect("typed external frontier rows");
        let serde_frontier = find_serde_frontier(&external_calls, fixture.target, observed.label());
        assert_eq!(serde_frontier.site_id, serde_site);
        assert_serde_json_summary_proof(
            proofs,
            fixture.target,
            serde_site,
            "Json::from_bytes serde_json::Deserializer::from_slice",
            observed.label(),
        );
        assert_serde_json_surface_measure_effect(
            effects,
            fixture.target,
            serde_site,
            "Json::from_bytes serde_json::Deserializer::from_slice",
            observed.label(),
        );
        assert_no_external_summary_need_for_site(
            needs,
            serde_site,
            "Json::from_bytes serde_json::Deserializer::from_slice",
            observed.label(),
        );
        assert!(
            cfgs.iter()
                .filter_map(serde_json::Value::as_str)
                .any(|cfg| cfg == r#"feature = "json""#),
            "{} should preserve the json cfg: {cfgs:#?}",
            observed.label()
        );

        assert_eq!(ui_field(observed.ui(), "call_context_incoming"), "2");
        assert_eq!(
            ui_field(observed.ui(), "reach_source_cfgs"),
            cfgs.len().to_string()
        );
        assert_eq!(
            ui_field(observed.ui(), "external_summary_needs"),
            needs.len().to_string()
        );
        assert_eq!(
            ui_field(observed.ui(), "reach_effects"),
            effects.len().to_string()
        );
        assert!(
            ui_field(observed.ui(), "proof_context")
                .parse::<usize>()
                .expect("proof count")
                >= fixture.callers.len(),
            "{} should surface Json::from_bytes proof rows",
            observed.label()
        );

        if tool == ContextTool::Lookup {
            let frontier = reach
                .get("frontier_calls")
                .and_then(serde_json::Value::as_array)
                .expect("frontier_calls array");
            let frontier_calls = frontier
                .iter()
                .map(|call| serde_json::from_value::<CallContextInfo>(call.clone()))
                .collect::<Result<Vec<_>, _>>()
                .expect("typed frontier rows");
            let serde_call = find_serde_frontier(&frontier_calls, fixture.target, observed.label());
            assert_eq!(serde_call.owner_id, fixture.target);
            assert_eq!(serde_call.site_id, serde_site);
            assert_eq!(serde_frontier.owner_id, fixture.target);
            let files = reach
                .get("source_files")
                .and_then(serde_json::Value::as_array)
                .expect("source_files array");
            assert_source_file(files, "axum/src/json.rs", observed.label());
            assert!(
                ui_field(observed.ui(), "reach_frontier_calls")
                    .parse::<usize>()
                    .expect("frontier count")
                    >= 1,
                "lookup should surface reach frontier calls"
            );
            assert!(
                ui_field(observed.ui(), "reach_external_frontier_calls")
                    .parse::<usize>()
                    .expect("external frontier count")
                    >= 1,
                "lookup should surface external frontier calls"
            );
            assert_eq!(
                ui_field(observed.ui(), "reach_source_files"),
                files.len().to_string()
            );
        }
    }
}

// axum boxed.rs:12,23,38,51 pins the tuple-struct constructor; chrono
// offset/mod.rs:77,81-83 plus its 12 callers pins the MappedLocalTime alias;
// chrono datetime/mod.rs:563 pins all seven resolved receiver shapes for
// DateTime::naive_utc.
#[tokio::test]
async fn code_item_tools_return_constructor_and_try_receiver_callers() {
    for tool in ContextTool::ALL {
        let fixture = AxumBoxedIntoRouteToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::item(
                "BoxedIntoRoute",
                &fixture.file_path,
                "struct",
                fixture.module_path_arg(),
            ),
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        assert_boxed_into_route_incoming_context(
            calls,
            &fixture.callers,
            fixture.target,
            observed.label(),
        );
        for caller in &fixture.callers {
            assert_target_proof(proofs, caller.owner, fixture.target, observed.label());
        }
        assert_incoming_ui(
            &observed,
            &fixture.callers.len().to_string(),
            fixture.callers.len(),
        );
    }

    for tool in ContextTool::ALL {
        let fixture = ChronoAliasConstructorToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::item(
                "Single",
                &fixture.file_path,
                "variant",
                fixture.module_path_arg(),
            ),
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        assert_expected_path_incoming_context(
            calls,
            &fixture.callers,
            fixture.target,
            observed.label(),
            "MappedLocalTime::Single",
        );
        for caller in &fixture.callers {
            assert_target_proof(proofs, caller.owner, fixture.target, observed.label());
        }
        assert_incoming_ui(&observed, "12", fixture.callers.len());
    }

    for tool in ContextTool::ALL {
        let fixture = ChronoNaiveUtcToolFixture::new().await;
        let mut query = EndpointQuery::item(
            "naive_utc",
            &fixture.file_path,
            "method",
            fixture.module_path_arg(),
        );
        query.type_name = Some("DateTime");
        let observed = observe_endpoint(tool, query, fixture.ctx(tool.label())).await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        assert_chrono_naive_utc_incoming_context(
            calls,
            &fixture.callers,
            fixture.target,
            observed.label(),
        );
        for caller in &fixture.callers {
            assert_target_proof(proofs, caller.owner, fixture.target, observed.label());
        }
        assert_incoming_ui(&observed, "7", fixture.callers.len());
    }
}

// chrono format/parse.rs:378-421 pins the typed `set: Setter` local binding
// and candidate proof frontier. axum-macros lib.rs:797 plus debug_handler.rs:
// 885,890, typed_path.rs:443, from_ref.rs:104, and from_request/mod.rs:1050
// pin all five run_ui_tests callers.
#[tokio::test]
async fn code_item_tools_return_typed_setter_and_run_ui_context() {
    for tool in ContextTool::ALL {
        let fixture = ChronoParseInternalToolFixture::new().await;
        let mut query = EndpointQuery::item(
            "parse_internal",
            &fixture.file_path,
            "function",
            fixture.module_path_arg(),
        );
        query.body = Some("set(parsed, v)?");
        let observed = observe_endpoint(tool, query, fixture.ctx(tool.label())).await;
        let bindings = observed.array("local_bindings");
        let edges = observed.array("local_binding_edges");
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        assert_typed_setter_local_binding_payload(
            bindings,
            edges,
            fixture.owner,
            "chrono parse_internal",
            observed.label(),
        );
        assert_chrono_typed_setter_candidate_payload(
            calls,
            proofs,
            fixture.owner,
            "chrono parse_internal",
            observed.label(),
        );
        assert_eq!(
            ui_field(observed.ui(), "local_bindings"),
            bindings.len().to_string()
        );
        assert_eq!(
            ui_field(observed.ui(), "local_binding_edges"),
            edges.len().to_string()
        );
        assert!(
            ui_field(observed.ui(), "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "{} should surface outgoing chrono setter candidate context",
            observed.label()
        );
        assert_eq!(
            ui_field(observed.ui(), "proof_context"),
            proofs.len().to_string()
        );
    }

    for tool in ContextTool::ALL {
        let fixture = AxumRunUiTestsToolFixture::new().await;
        let observed = observe_endpoint(
            tool,
            EndpointQuery::item(
                "run_ui_tests",
                &fixture.file_path,
                "function",
                fixture.module_path_arg(),
            ),
            fixture.ctx(tool.label()),
        )
        .await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        assert_run_ui_tests_incoming_context(
            calls,
            &fixture.callers,
            fixture.target,
            observed.label(),
        );
        for caller in &fixture.callers {
            assert_target_proof(proofs, caller.owner, fixture.target, observed.label());
        }
        assert_incoming_ui(&observed, "5", fixture.callers.len());
    }
}

// axum handler/mod.rs:153 and handler/service.rs:171 pin the trait-qualified
// Handler::call target and its one caller. Lookup additionally keeps the strict
// failure contract for the unqualified selector; Edges remains qualified-only.
#[tokio::test]
async fn code_item_tools_disambiguate_handler_call_by_owner_trait() {
    for tool in ContextTool::ALL {
        let fixture = AxumHandlerCallToolFixture::new().await;
        let module = fixture.module_path_arg();
        if tool == ContextTool::Lookup {
            let ambiguous = CodeItemLookup::execute(
                LookupParams {
                    item_name: Cow::Borrowed("call"),
                    file_path: Cow::Owned(fixture.file_path.display().to_string()),
                    node_kind: Cow::Borrowed("method"),
                    module_path: Cow::Owned(module.clone()),
                    owner_trait: None,
                    owner_type: None,
                    parent_name: None,
                    body_contains: None,
                    allowed_effects: Vec::new(),
                },
                fixture.ctx("axum-handler-call-ambiguous-lookup"),
            )
            .await
            .expect_err("unqualified Handler::call lookup should remain ambiguous");
            let message = ambiguous.to_string();
            assert!(
                message.contains("Multiple items matched `call`"),
                "unqualified call lookup should preserve the strict ambiguity error: {message}"
            );
        }

        let mut query = EndpointQuery::item("call", &fixture.file_path, "method", module);
        query.trait_name = Some("Handler");
        let observed = observe_endpoint(tool, query, fixture.ctx(tool.label())).await;
        let calls = observed.array("call_context");
        let proofs = observed.array("proof_context");
        assert_handler_call_incoming_context(
            calls,
            &fixture.caller,
            fixture.target,
            observed.label(),
        );
        assert_target_proof(
            proofs,
            fixture.caller.owner,
            fixture.target,
            observed.label(),
        );
        assert_incoming_ui(&observed, "1", 1);
    }
}

fn assert_incoming_ui(observed: &EndpointObservation, expected: &str, proof_min: usize) {
    assert_eq!(ui_field(observed.ui(), "call_context_incoming"), expected);
    assert!(
        ui_field(observed.ui(), "proof_context")
            .parse::<usize>()
            .expect("proof count")
            >= proof_min,
        "{} should surface the expected incoming proof rows",
        observed.label()
    );
}

fn find_serde_frontier<'a>(
    calls: &'a [CallContextInfo],
    owner: uuid::Uuid,
    label: &str,
) -> &'a CallContextInfo {
    calls
        .iter()
        .find(|call| {
            call.owner_id == owner
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
        .unwrap_or_else(|| panic!("{label} serde_json external frontier row: {calls:#?}"))
}

const DYNAMIC_CASES: &[&str] = &[
    // Fixture source: tests/fixture_crates/fixture_call_graph/src/lib.rs.
    // These cover parenthesized and guarded bindings, field/index projections,
    // callable parameters, boxed/referenced callables, and forwarded values.
    "call_parenthesized_function_item_binding",
    "call_parenthesized_block_initialized_function_item_binding",
    "call_match_guarded_function_item",
    "call_aliased_indexed_named_field_function_binding",
    "call_single_named_field_function_param",
    "call_single_indexed_function_pointer_param",
    "call_single_indexed_field_function_param",
    "call_single_indexed_tuple_field_function_param",
    "call_single_parenthesized_aliased_function_pointer_param",
    "call_dereferenced_boxed_dyn_fn_value_binding",
    "call_parenthesized_referenced_dyn_fn_value_binding",
    "call_single_parenthesized_referenced_dyn_fn_param",
    "call_single_parenthesized_boxed_dyn_fn_param",
    "call_parenthesized_mut_referenced_dyn_fnmut_value_binding",
    "call_forwarded_named_field_leaf",
    "call_two_hop_forwarded_named_field_leaf",
    "call_returned_forwarded_function_pointer_param_with_local_target",
];

// owner, callee path, exact projected proof count, diagnostic label
type CallableCase = (&'static str, &'static [&'static str], usize, &'static str);

const CALLABLE_CASES: &[CallableCase] = &[
    (
        "call_single_boxed_dyn_fn_param",
        &["f"],
        6,
        "private boxed dyn Fn parameter",
    ),
    (
        "call_multi_function_pointer_param",
        &["f"],
        9,
        "multi-caller callable parameter",
    ),
    (
        "call_forwarded_function_pointer_leaf",
        &["f"],
        6,
        "multi-caller callable parameter",
    ),
    (
        "call_two_hop_forwarded_function_pointer_leaf",
        &["f"],
        6,
        "multi-caller callable parameter",
    ),
    (
        "call_multi_generic_fn_once_param",
        &["generic_f"],
        9,
        "multi-caller callable parameter",
    ),
    (
        "call_forwarded_referenced_dyn_fn_leaf",
        &["f"],
        6,
        "forwarded referenced dyn Fn parameter",
    ),
    (
        "call_two_hop_forwarded_referenced_dyn_fn_leaf",
        &["f"],
        6,
        "forwarded referenced dyn Fn parameter",
    ),
    (
        "call_forwarded_boxed_dyn_fn_leaf",
        &["f"],
        6,
        "forwarded boxed dyn Fn parameter",
    ),
    (
        "call_two_hop_forwarded_boxed_dyn_fn_leaf",
        &["f"],
        6,
        "forwarded boxed dyn Fn parameter",
    ),
];

#[derive(Clone, Copy)]
struct BlockerCase {
    owner: &'static str,
    path: &'static [&'static str],
    shape: CallableBlockerShape,
    proofs: usize,
}

const BLOCKER_CASES: &[BlockerCase] = &[
    BlockerCase {
        owner: "call_function_pointer_param",
        path: &["f"],
        shape: CallableBlockerShape::Path,
        proofs: 2,
    },
    BlockerCase {
        owner: "call_multi_conflicting_function_pointer_param",
        path: &["f"],
        shape: CallableBlockerShape::AmbiguousPath,
        proofs: 8,
    },
    BlockerCase {
        owner: "call_forwarded_conflicting_function_pointer_leaf",
        path: &["f"],
        shape: CallableBlockerShape::AmbiguousPath,
        proofs: 5,
    },
    BlockerCase {
        owner: "call_generic_fn_once_value_binding",
        path: &["generic_f"],
        shape: CallableBlockerShape::Path,
        proofs: 2,
    },
    BlockerCase {
        owner: "call_multi_conflicting_generic_fn_once_param",
        path: &["generic_f"],
        shape: CallableBlockerShape::AmbiguousPath,
        proofs: 8,
    },
    BlockerCase {
        owner: "call_multi_conflicting_named_field_function_param",
        path: &["holder", "callback"],
        shape: CallableBlockerShape::AmbiguousDynamic,
        proofs: 8,
    },
    BlockerCase {
        owner: "call_field_function_param",
        path: &["holder", "callback"],
        shape: CallableBlockerShape::Dynamic,
        proofs: 2,
    },
    BlockerCase {
        owner: "call_indexed_function_pointer",
        path: &["funcs", "0"],
        shape: CallableBlockerShape::Dynamic,
        proofs: 2,
    },
    BlockerCase {
        owner: "call_indexed_field_function_param",
        path: &["holder", "callbacks", "0"],
        shape: CallableBlockerShape::Dynamic,
        proofs: 2,
    },
    BlockerCase {
        owner: "call_indexed_tuple_field_function_param",
        path: &["holder", "0", "0"],
        shape: CallableBlockerShape::Dynamic,
        proofs: 2,
    },
    BlockerCase {
        owner: "call_forwarded_conflicting_named_field_leaf",
        path: &["holder", "callback"],
        shape: CallableBlockerShape::AmbiguousDynamic,
        proofs: 5,
    },
    BlockerCase {
        owner: "call_returned_conflicting_forwarded_function_pointer_param_with_local_target",
        path: &["return_conflicting_forwarded_function_pointer"],
        shape: CallableBlockerShape::AmbiguousDynamic,
        proofs: 6,
    },
    BlockerCase {
        owner: "call_returned_conflicting_forwarded_function_pointer_param_with_other_target",
        path: &["return_conflicting_forwarded_function_pointer"],
        shape: CallableBlockerShape::AmbiguousDynamic,
        proofs: 6,
    },
];

#[tokio::test]
async fn code_item_tools_cover_resolved_dynamic_callable_cases() {
    for owner_name in DYNAMIC_CASES {
        let db = fixture_graph_db();
        let fixture = FixtureDynamicCallableToolFixture::with_db(Arc::clone(&db), owner_name).await;
        for tool in ContextTool::ALL {
            let observed =
                observe_function(tool, &fixture.state, &fixture.file_path, fixture.owner_name)
                    .await;
            let label = format!("{} case {}", observed.label(), fixture.owner_name);
            let site = assert_resolved_dynamic_context(
                observed.array("call_context"),
                fixture.owner,
                fixture.target,
                CallTargetKind::DynamicFunction,
                fixture.owner_name,
                &label,
            );
            assert_resolved_callable_param_proof(
                observed.array("proof_context"),
                fixture.owner,
                fixture.target,
                site,
                "bd:fixture-call-graph",
                &label,
            );

            assert!(
                ui_field(observed.ui(), "call_context_outgoing")
                    .parse::<usize>()
                    .expect("outgoing count")
                    >= 1,
                "{label} should surface outgoing dynamic call context"
            );
            assert!(
                ui_field(observed.ui(), "proof_context")
                    .parse::<usize>()
                    .expect("proof count")
                    >= 3,
                "{label} should surface the resolved proof row set"
            );
        }
    }
}

#[tokio::test]
async fn code_item_tools_cover_resolved_callable_parameter_cases() {
    for &(owner, path, proofs, label) in CALLABLE_CASES {
        let db = fixture_graph_db();
        let fixture =
            CallableParamResolvedFixture::with_db(Arc::clone(&db), owner, path, proofs).await;
        assert_callable_case(fixture, label).await;
    }
}

async fn assert_callable_case(fixture: CallableParamResolvedFixture, case_name: &str) {
    for tool in ContextTool::ALL {
        let observed =
            observe_function(tool, &fixture.state, &fixture.file_path, fixture.owner_name).await;
        let label = format!(
            "{} case {} ({case_name})",
            observed.label(),
            fixture.owner_name
        );
        let callee = CallCalleeInfo::Path {
            path: fixture.path.clone(),
        };
        let site = assert_resolved_path_context(
            observed.array("call_context"),
            fixture.owner,
            &callee,
            fixture.target,
            CallTargetKind::Function,
            case_name,
            &label,
        );
        let proofs = observed.array("proof_context");
        assert_resolved_callable_param_proof(
            proofs,
            fixture.owner,
            fixture.target,
            site,
            fixture.build_domain,
            &label,
        );

        assert!(
            ui_field(observed.ui(), "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "{label} should surface the resolved callable row"
        );
        assert_eq!(
            ui_field(observed.ui(), "proof_context"),
            proofs.len().to_string(),
            "{label} UI proof count"
        );
    }
}

#[tokio::test]
async fn code_item_tools_cover_callable_blocker_cases() {
    // Fixture source: tests/fixture_crates/fixture_call_graph/src/lib.rs.
    // Public opaque callable parameters remain targetless, while complete
    // conflicting caller sets expose candidates without fabricating edges.
    for case in BLOCKER_CASES {
        let db = fixture_graph_db();
        let fixture = CallableBlockerFixture::with_db(
            Arc::clone(&db),
            case.owner,
            case.path,
            case.shape,
            case.proofs,
        )
        .await;
        assert_callable_blocker_case(&fixture).await;
    }

    // DirectSelfFieldDispatcher::invoke is the method-shaped member of the
    // same candidate-only callable family.
    let db = fixture_graph_db();
    let fixture = DirectSelfFieldDispatchFixture::with_db(Arc::clone(&db)).await;
    assert_direct_self_field_case(&fixture).await;
}

async fn assert_callable_blocker_case(fixture: &CallableBlockerFixture) {
    for tool in ContextTool::ALL {
        let observed =
            observe_function(tool, &fixture.state, &fixture.file_path, fixture.owner_name).await;
        let label = format!(
            "{} case {} callable parameter call",
            tool.label(),
            fixture.owner_name
        );
        let callee = CallCalleeInfo::Path {
            path: fixture.path.clone(),
        };
        let site = match fixture.shape {
            CallableBlockerShape::Path => assert_path_context(
                observed.array("call_context"),
                fixture.owner,
                &callee,
                &CallStatusKind::Unsupported,
                &label,
                tool.label(),
            ),
            CallableBlockerShape::Dynamic => assert_dynamic_context(
                observed.array("call_context"),
                fixture.owner,
                None,
                None,
                &label,
                tool.label(),
            ),
            CallableBlockerShape::AmbiguousPath => assert_ambiguous_path_candidates(
                observed.array("call_context"),
                fixture.owner,
                &callee,
                &fixture.candidates,
                &label,
                tool.label(),
            ),
            CallableBlockerShape::AmbiguousDynamic => assert_ambiguous_dynamic_candidates(
                observed.array("call_context"),
                fixture.owner,
                &fixture.candidates,
                &label,
                tool.label(),
            ),
        };
        match fixture.shape {
            CallableBlockerShape::Path => assert_path_resolution_proof(
                observed.array("proof_context"),
                fixture.owner,
                site,
                fixture.build_domain,
                "blocked",
                "type_resolution_missing",
                &label,
                tool.label(),
            ),
            CallableBlockerShape::Dynamic => assert_dynamic_proof(
                observed.array("proof_context"),
                fixture.owner,
                site,
                fixture.build_domain,
                &label,
                tool.label(),
            ),
            CallableBlockerShape::AmbiguousPath | CallableBlockerShape::AmbiguousDynamic => {
                assert_path_resolution_proof(
                    observed.array("proof_context"),
                    fixture.owner,
                    site,
                    fixture.build_domain,
                    "ambiguous",
                    "type_resolution_missing",
                    &label,
                    tool.label(),
                );
            }
        }

        if tool == ContextTool::Edges {
            let blocked = observed.edge_summary("blocked");
            if matches!(
                fixture.shape,
                CallableBlockerShape::AmbiguousPath | CallableBlockerShape::AmbiguousDynamic
            ) {
                assert_eq!(
                    blocked, 0,
                    "{label} summary must not count candidate rows as targetless blockers"
                );
            } else {
                assert!(
                    blocked >= 1,
                    "{label} summary should count the targetless callable row"
                );
            }
        }

        let proofs = observed.array("proof_context");
        assert!(
            ui_field(observed.ui(), "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "{label} should surface outgoing call context"
        );
        assert_eq!(
            ui_field(observed.ui(), "proof_context"),
            proofs.len().to_string(),
            "{label} UI proof count"
        );
    }
}

async fn assert_direct_self_field_case(fixture: &DirectSelfFieldDispatchFixture) {
    for tool in ContextTool::ALL {
        let observed = observe_method(
            tool,
            &fixture.state,
            &fixture.file_path,
            fixture.owner_name,
            fixture.owner_type,
        )
        .await;
        let label = format!(
            "{} case DirectSelfFieldDispatcher::invoke direct self-field call",
            tool.label()
        );
        let path = fixture.path.iter().map(String::as_str).collect::<Vec<_>>();
        let site = assert_ambiguous_dynamic_candidates_with_relation(
            observed.array("call_context"),
            fixture.owner,
            Some(path.as_slice()),
            None,
            &fixture.candidates,
            CallTargetKind::DynamicFunction,
            &label,
            tool.label(),
        );
        assert_ambiguous_candidate_proof(
            observed.array("proof_context"),
            fixture.owner,
            site,
            fixture.build_domain,
            &fixture.candidates,
            &label,
            tool.label(),
        );

        if tool == ContextTool::Edges {
            assert_eq!(
                observed.edge_summary("blocked"),
                0,
                "{label} summary must not count candidate rows as targetless blockers"
            );
        }

        let proofs = observed.array("proof_context");
        assert!(
            ui_field(observed.ui(), "call_context_outgoing")
                .parse::<usize>()
                .expect("outgoing count")
                >= 1,
            "{label} should surface outgoing call context"
        );
        assert_eq!(
            ui_field(observed.ui(), "proof_context"),
            proofs.len().to_string(),
            "{label} UI proof count"
        );
    }
}

#[tokio::test]
#[ignore = "full code_item_lookup/code_item_edges parity over real-corpus matrix is usage-summary heavy; run manually when auditing TUI parity"]
async fn code_item_tools_cover_shared_real_corpus_call_shape_matrix() {
    // Matrix:
    //   docs/active/agents/call-graph/
    //   2026-06-28_real-corpus-call-site-oracle-matrices.md
    //
    // Shared case source locations include:
    //   axum-macros/src/typed_path.rs:23
    //   chrono/src/offset/mod.rs:143
    //   axum/src/handler/service.rs:174
    //   axum/src/serve/listener.rs:236
    //
    // Expected traversal: exact lookup of the target item exposes the resolved
    // incoming call edge; exact lookup of the owner item exposes the targetless
    // outgoing frontier row and proof blocker. The edge-oriented tool must
    // preserve the same call/proof payload.
    for corpus in [
        CallCorpusFixture::Axum,
        CallCorpusFixture::Chrono,
        CallCorpusFixture::Memchr,
        CallCorpusFixture::GenericArray,
    ] {
        let db = SharedCallShapeToolFixture::db_for_fixture(corpus);
        for case in call_shape_cases().iter().filter(|case| {
            case.fixture == corpus && case.coverage.contains(&CallPipelineCoverage::TuiTool)
        }) {
            if !case_filter_matches(case.name) {
                continue;
            }
            eprintln!("shared TUI call-shape case: {} setup", case.name);
            let fixture = SharedCallShapeToolFixture::new_with_db(case, Arc::clone(&db)).await;
            eprintln!("shared TUI call-shape case: {} lookup", case.name);
            assert_lookup_payload(&fixture).await;
            eprintln!("shared TUI call-shape case: {} edges", case.name);
            assert_edges_payload(&fixture).await;
            eprintln!("shared TUI call-shape case: {} done", case.name);
        }
    }
}

fn case_filter_matches(case_name: &str) -> bool {
    std::env::var("PLOKE_SHARED_CALL_SHAPE_CASE")
        .map(|filter| case_name.contains(&filter))
        .unwrap_or(true)
}

async fn assert_lookup_payload(fixture: &SharedCallShapeToolFixture) {
    let result = CodeItemLookup::execute(
        fixture.lookup_params(),
        fixture.ctx("shared-call-shape-lookup"),
    )
    .await
    .unwrap_or_else(|err| panic!("{} code_item_lookup: {err}", fixture.case.name));
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

    fixture.assert_call_context(call_context, "code_item_lookup");
    fixture.assert_proof_context(proof_context, "code_item_lookup");
    fixture.assert_ui_counts(
        result.ui_payload.as_ref().expect("ui payload"),
        proof_context.len(),
    );
}

async fn assert_edges_payload(fixture: &SharedCallShapeToolFixture) {
    let result = CodeItemEdges::execute(
        fixture.edges_params(),
        fixture.ctx("shared-call-shape-edges"),
    )
    .await
    .unwrap_or_else(|err| panic!("{} code_item_edges: {err}", fixture.case.name));
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

    fixture.assert_call_context(call_context, "code_item_edges");
    fixture.assert_proof_context(proof_context, "code_item_edges");
    fixture.assert_ui_counts(
        result.ui_payload.as_ref().expect("ui payload"),
        proof_context.len(),
    );
}
