use super::lookup::*;
use super::*;

const LOCAL_TARGET: &[&str] = &["local_target"];
const F_BINDING: &[&str] = &["f"];

pub(super) struct DynamicProjectionCases {
    pub(super) resolved: Vec<ResolvedProjectionCase>,
    pub(super) ambiguous: Vec<AmbiguousProjectionCase>,
}

pub(super) struct ResolvedProjectionCase {
    pub(super) label: &'static str,
    pub(super) site_id: DataValue,
    pub(super) target_id: DataValue,
    pub(super) path: Option<&'static [&'static str]>,
}

pub(super) struct AmbiguousProjectionCase {
    pub(super) label: &'static str,
    pub(super) site_id: DataValue,
    pub(super) expected_targets: Vec<DataValue>,
}

pub(super) fn dynamic_projection_cases(
    call_report: &CallResolutionReport,
    graph: &ParsedCodeGraph,
) -> DynamicProjectionCases {
    let (call_site_id, target_function_id) = find_dynamic_relation(
        call_report,
        graph,
        |dynamic_call| {
            matches!(
                &dynamic_call.callee,
                DynamicCallCallee::Path { path } if path.as_slice() == ["local_target"]
            )
        },
        "fixture_call_graph should resolve (local_target)() as DynamicFunction",
    );
    let (cast_site_id, cast_target_id) = find_dynamic_relation(
        call_report,
        graph,
        |dynamic_call| {
            matches!(
                &dynamic_call.callee,
                DynamicCallCallee::FnPointerCastPath { path }
                    if path.as_slice() == ["local_target"]
            )
        },
        "fixture_call_graph should resolve (local_target as fn() -> i32)() as DynamicFunction",
    );
    let (binding_cast_site_id, binding_cast_target_id) = find_dynamic_relation(
        call_report,
        graph,
        |dynamic_call| {
            matches!(
                &dynamic_call.callee,
                DynamicCallCallee::FnPointerCastInitializedLocalBinding {
                    path,
                    init_path,
                } if path.as_slice() == ["f"] && init_path.as_slice() == ["local_target"]
            )
        },
        "fixture_call_graph should resolve (f as fn() -> i32)() as DynamicFunction",
    );
    let (deref_site_id, deref_target_id) = find_dynamic_relation(
        call_report,
        graph,
        |dynamic_call| {
            matches!(
                &dynamic_call.callee,
                DynamicCallCallee::DereferencedInitializedLocalBinding {
                    path,
                    init_path,
                } if path.as_slice() == ["f"] && init_path.as_slice() == ["local_target"]
            )
        },
        "fixture_call_graph should resolve (*f)() as DynamicFunction",
    );
    let (block_site_id, block_target_id) = find_dynamic_relation(
        call_report,
        graph,
        |dynamic_call| {
            dynamic_call.span == (13065, 13085)
                && matches!(
                    &dynamic_call.callee,
                    DynamicCallCallee::Path { path } if path.as_slice() == ["local_target"]
                )
        },
        "fixture_call_graph should resolve ({ local_target })() as DynamicFunction",
    );
    let (branch_site_id, branch_target_id) = find_dynamic_relation(
        call_report,
        graph,
        |dynamic_call| {
            dynamic_call.span == (13188, 13238)
                && matches!(
                    &dynamic_call.callee,
                    DynamicCallCallee::IfBranchPaths { paths }
                        if paths.as_slice()
                            == [
                                vec!["local_target".to_string()],
                                vec!["local_target".to_string()]
                            ]
                )
        },
        "fixture_call_graph should resolve if same-branch dynamic call as DynamicFunction",
    );
    let branch_site = find_dynamic_site(
        graph,
        |dynamic_call| {
            dynamic_call.span == (13306, 13356)
                && matches!(
                    &dynamic_call.callee,
                    DynamicCallCallee::IfBranchPaths { paths }
                        if paths.as_slice()
                            == [
                                vec!["local_target".to_string()],
                                vec!["other_target".to_string()]
                            ]
                )
        },
        "fixture_call_graph should record ambiguous if-branch dynamic call site",
    );
    let (match_site_id, match_target_id) = find_dynamic_relation(
        call_report,
        graph,
        |dynamic_call| {
            dynamic_call.span == (13422, 13505)
                && matches!(
                    &dynamic_call.callee,
                    DynamicCallCallee::MatchArmPaths { paths }
                        if paths.as_slice()
                            == [
                                vec!["local_target".to_string()],
                                vec!["local_target".to_string()]
                            ]
                )
        },
        "fixture_call_graph should resolve match same-arm dynamic call as DynamicFunction",
    );
    let match_site = find_dynamic_site(
        graph,
        |dynamic_call| {
            dynamic_call.span == (13576, 13659)
                && matches!(
                    &dynamic_call.callee,
                    DynamicCallCallee::MatchArmPaths { paths }
                        if paths.as_slice()
                            == [
                                vec!["local_target".to_string()],
                                vec!["other_target".to_string()]
                            ]
                )
        },
        "fixture_call_graph should record ambiguous match-arm dynamic call site",
    );

    DynamicProjectionCases {
        resolved: vec![
            ResolvedProjectionCase {
                label: "dynamic function",
                site_id: call_site_id.to_cozo_uuid(),
                target_id: target_function_id.into(),
                path: None,
            },
            ResolvedProjectionCase {
                label: "function-pointer cast",
                site_id: cast_site_id.to_cozo_uuid(),
                target_id: cast_target_id.into(),
                path: Some(LOCAL_TARGET),
            },
            ResolvedProjectionCase {
                label: "initialized function-pointer cast",
                site_id: binding_cast_site_id.to_cozo_uuid(),
                target_id: binding_cast_target_id.into(),
                path: Some(F_BINDING),
            },
            ResolvedProjectionCase {
                label: "dereferenced function-pointer",
                site_id: deref_site_id.to_cozo_uuid(),
                target_id: deref_target_id.into(),
                path: Some(F_BINDING),
            },
            ResolvedProjectionCase {
                label: "block-path",
                site_id: block_site_id.to_cozo_uuid(),
                target_id: block_target_id.into(),
                path: Some(LOCAL_TARGET),
            },
            ResolvedProjectionCase {
                label: "if-branch",
                site_id: branch_site_id.to_cozo_uuid(),
                target_id: branch_target_id.into(),
                path: Some(LOCAL_TARGET),
            },
            ResolvedProjectionCase {
                label: "match-arm",
                site_id: match_site_id.to_cozo_uuid(),
                target_id: match_target_id.into(),
                path: Some(LOCAL_TARGET),
            },
        ],
        ambiguous: vec![
            AmbiguousProjectionCase {
                label: "ambiguous if-branch",
                site_id: branch_site.to_cozo_uuid(),
                expected_targets: dynamic_relation_targets(call_report, branch_site),
            },
            AmbiguousProjectionCase {
                label: "ambiguous match-arm",
                site_id: match_site.to_cozo_uuid(),
                expected_targets: dynamic_relation_targets(call_report, match_site),
            },
        ],
    }
}

fn dynamic_relation_targets(
    call_report: &CallResolutionReport,
    site_id: DynamicCallSiteId,
) -> Vec<DataValue> {
    call_report
        .relations
        .iter()
        .copied()
        .filter_map(|relation| match relation {
            CallRelation::DynamicFunction { source, target } if source == site_id => {
                Some(target.into())
            }
            _ => None,
        })
        .collect()
}
