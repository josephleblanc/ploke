use super::super::super::super::super::*;
use super::super::super::helpers::*;
use super::expected::{assert_ambiguous_call_candidates, path_call};

struct ResolvedCase {
    label: &'static str,
    owner: Uuid,
    count: Option<usize>,
    source: Uuid,
    kind: CallSiteKind,
    callee: CallCalleeInfo,
    target: Uuid,
    relation: CallTargetKind,
}

impl ResolvedCase {
    fn path(
        label: &'static str,
        owner: Uuid,
        count: Option<usize>,
        path: &[&str],
        target: Uuid,
    ) -> Self {
        Self::path_from(label, owner, count, owner, path, target)
    }

    fn path_from(
        label: &'static str,
        owner: Uuid,
        count: Option<usize>,
        source: Uuid,
        path: &[&str],
        target: Uuid,
    ) -> Self {
        Self {
            label,
            owner,
            count,
            source,
            kind: CallSiteKind::Path,
            callee: path_call(path),
            target,
            relation: CallTargetKind::Function,
        }
    }

    fn dynamic(label: &'static str, owner: Uuid, count: Option<usize>, target: Uuid) -> Self {
        Self {
            label,
            owner,
            count,
            source: owner,
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            target,
            relation: CallTargetKind::DynamicFunction,
        }
    }
}

macro_rules! resolved_cases {
    ($($kind:ident($($arg:expr),+ $(,)?);)+) => {
        [$(ResolvedCase::$kind($($arg),+)),+]
    };
}

struct TargetlessCase {
    label: &'static str,
    owner: Uuid,
    count: Option<usize>,
    kind: CallSiteKind,
    callee: CallCalleeInfo,
    status: CallStatusKind,
}

impl TargetlessCase {
    fn path(
        label: &'static str,
        owner: Uuid,
        count: Option<usize>,
        path: &[&str],
        status: CallStatusKind,
    ) -> Self {
        Self {
            label,
            owner,
            count,
            kind: CallSiteKind::Path,
            callee: path_call(path),
            status,
        }
    }

    fn dynamic(label: &'static str, owner: Uuid, status: CallStatusKind) -> Self {
        Self {
            label,
            owner,
            count: None,
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            status,
        }
    }
}

struct AmbiguousCase {
    label: &'static str,
    owner: Uuid,
    kind: CallSiteKind,
    callee: CallCalleeInfo,
    relation: CallTargetKind,
}

impl AmbiguousCase {
    fn path(label: &'static str, owner: Uuid, path: &[&str]) -> Self {
        Self {
            label,
            owner,
            kind: CallSiteKind::Path,
            callee: path_call(path),
            relation: CallTargetKind::Function,
        }
    }

    fn dynamic(label: &'static str, owner: Uuid) -> Self {
        Self {
            label,
            owner,
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        }
    }
}

struct ClosureCase {
    label: &'static str,
    owner: Uuid,
    maker: Uuid,
}

fn fixture_owner_id(db: &Database, name: &str) -> Result<Uuid, Error> {
    Ok(one_uuid(db, &function_in_module_query(&["crate"], name))?)
}

fn assert_count(context: &[CallContextInfo], count: Option<usize>, label: &str) {
    if let Some(count) = count {
        assert_eq!(context.len(), count, "{label} context: {context:#?}");
    }
}

fn select_call<'a>(
    context: &'a [CallContextInfo],
    source: Uuid,
    kind: &CallSiteKind,
    callee: &CallCalleeInfo,
    target: Option<Uuid>,
    label: &str,
) -> &'a CallContextInfo {
    let matches = context
        .iter()
        .filter(|call| {
            call.owner_id == source
                && &call.kind == kind
                && &call.callee == callee
                && target.is_none_or(|target| {
                    call.targets
                        .iter()
                        .any(|candidate| candidate.target_id == target)
                })
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{label} should match exactly one call: {context:#?}"
    );
    matches[0]
}

fn assert_resolved(context: &[CallContextInfo], case: &ResolvedCase) {
    assert_count(context, case.count, case.label);
    let call = select_call(
        context,
        case.source,
        &case.kind,
        &case.callee,
        Some(case.target),
        case.label,
    );
    assert_eq!(call.status, CallStatusKind::Resolved, "{}", case.label);
    assert_eq!(
        call.resolution,
        Some(CallResolutionKind::LocalExact),
        "{}",
        case.label
    );
    assert_eq!(call.targets.len(), 1, "{}: {call:#?}", case.label);
    assert_eq!(call.targets[0].target_id, case.target, "{}", case.label);
    assert_eq!(call.targets[0].relation, case.relation, "{}", case.label);
}

fn assert_targetless(context: &[CallContextInfo], case: &TargetlessCase) {
    assert_count(context, case.count, case.label);
    let call = select_call(
        context,
        case.owner,
        &case.kind,
        &case.callee,
        None,
        case.label,
    );
    assert_eq!(call.status, case.status, "{}", case.label);
    assert!(call.resolution.is_none(), "{}: {call:#?}", case.label);
    assert!(
        call.targets.is_empty(),
        "{} must not fabricate targets: {call:#?}",
        case.label
    );
}

fn assert_ambiguous(context: &[CallContextInfo], case: &AmbiguousCase, first: Uuid, second: Uuid) {
    let call = select_call(
        context,
        case.owner,
        &case.kind,
        &case.callee,
        None,
        case.label,
    );
    assert_ambiguous_call_candidates(call, first, second, case.relation.clone(), case.label);
}

fn assert_closure(context: &[CallContextInfo], case: &ClosureCase) {
    let matches = context
        .iter()
        .filter(|call| {
            call.owner_id == case.owner
                && call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target| target.relation == CallTargetKind::DynamicClosure)
        })
        .collect::<Vec<_>>();
    assert_eq!(
        matches.len(),
        1,
        "{} should match exactly one dynamic closure call: {context:#?}",
        case.label
    );
    let call = matches[0];
    assert_eq!(call.status, CallStatusKind::Resolved, "{}", case.label);
    assert_eq!(
        call.resolution,
        Some(CallResolutionKind::LocalExact),
        "{}",
        case.label
    );
    assert_eq!(call.targets.len(), 1, "{}: {call:#?}", case.label);
    assert_ne!(
        call.targets[0].target_id, case.maker,
        "{} target should be the closure owner, not the maker function",
        case.label
    );
    assert_eq!(
        call.targets[0].relation,
        CallTargetKind::DynamicClosure,
        "{}",
        case.label
    );
}

#[tokio::test]
async fn call_context_collection_reads_real_fixture_callable_path_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let make_fn = unique_id_by_name(&db, "function", "make_fn")?;
    let make_closure = unique_id_by_name(&db, "function", "make_closure")?;
    let make_bound = unique_id_by_name(&db, "function", "make_bound_closure")?;
    let make_alias = unique_id_by_name(&db, "function", "make_alias_bound_closure")?;
    let target_maker = unique_id_by_name(&db, "function", "make_target_closure")?;
    let closure_producer = unique_id_by_name(&db, "function", "make_forwarded_returned_closure")?;
    let make_returned_async = unique_id_by_name(&db, "function", "make_returned_async_closure")?;
    let future_producer =
        unique_id_by_name(&db, "function", "make_forwarded_returned_async_future")?;
    let returned_param_helper =
        unique_id_by_name(&db, "function", "return_forwarded_function_pointer")?;
    let returned_conflicting_helper = unique_id_by_name(
        &db,
        "function",
        "return_conflicting_forwarded_function_pointer",
    )?;
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let other_target = unique_id_by_name(&db, "function", "other_target")?;
    let returned_owner = fixture_owner_id(&db, "call_returned_function")?;
    let returned_param_owner = fixture_owner_id(
        &db,
        "call_returned_forwarded_function_pointer_param_with_local_target",
    )?;
    let returned_conflicting_local_owner = fixture_owner_id(
        &db,
        "call_returned_conflicting_forwarded_function_pointer_param_with_local_target",
    )?;
    let returned_conflicting_other_owner = fixture_owner_id(
        &db,
        "call_returned_conflicting_forwarded_function_pointer_param_with_other_target",
    )?;
    let returned_closure_owner = fixture_owner_id(&db, "call_returned_closure")?;
    let bound_owner = fixture_owner_id(&db, "call_returned_bound_closure")?;
    let alias_owner = fixture_owner_id(&db, "call_returned_alias_bound_closure")?;
    let closure_owner = fixture_owner_id(&db, "call_forwarded_returned_closure")?;
    let returned_async_no_await_owner =
        fixture_owner_id(&db, "call_returned_async_closure_without_await")?;
    let returned_async_awaited_owner =
        fixture_owner_id(&db, "call_awaited_returned_async_closure")?;
    let returned_async_stored_owner = fixture_owner_id(&db, "call_stored_returned_async_closure")?;
    let future_owner = fixture_owner_id(&db, "call_forwarded_returned_async_future")?;
    let stored_future_owner = fixture_owner_id(
        &db,
        "call_stored_forwarded_returned_async_future_tuple_field",
    )?;
    let aliased_future_owner = fixture_owner_id(
        &db,
        "call_aliased_stored_forwarded_returned_async_future_tuple_field",
    )?;
    let branch_owner = fixture_owner_id(&db, "call_if_initialized_function_item_binding")?;
    let block_owner = fixture_owner_id(&db, "call_block_initialized_function_item_binding")?;
    let fn_param_owner = fixture_owner_id(&db, "call_function_pointer_param")?;
    let single_param_owner = fixture_owner_id(&db, "call_single_function_pointer_param")?;
    let multi_param_owner = fixture_owner_id(&db, "call_multi_function_pointer_param")?;
    let forwarded_param_owner = fixture_owner_id(&db, "call_forwarded_function_pointer_leaf")?;
    let two_hop_forwarded_param_owner =
        fixture_owner_id(&db, "call_two_hop_forwarded_function_pointer_leaf")?;
    let forwarded_referenced_owner =
        fixture_owner_id(&db, "call_forwarded_referenced_dyn_fn_leaf")?;
    let two_hop_forwarded_referenced_owner =
        fixture_owner_id(&db, "call_two_hop_forwarded_referenced_dyn_fn_leaf")?;
    let multi_conflicting_param_owner =
        fixture_owner_id(&db, "call_multi_conflicting_function_pointer_param")?;
    let forwarded_conflicting_owner =
        fixture_owner_id(&db, "call_forwarded_conflicting_function_pointer_leaf")?;
    let two_hop_forwarded_conflicting_owner = fixture_owner_id(
        &db,
        "call_two_hop_forwarded_conflicting_function_pointer_leaf",
    )?;
    let multi_conflicting_generic_owner =
        fixture_owner_id(&db, "call_multi_conflicting_generic_fn_once_param")?;
    let multi_conflicting_field_owner =
        fixture_owner_id(&db, "call_multi_conflicting_named_field_function_param")?;
    let forwarded_conflicting_field_owner =
        fixture_owner_id(&db, "call_forwarded_conflicting_named_field_leaf")?;
    let single_parenthesized_param_owner =
        fixture_owner_id(&db, "call_single_parenthesized_function_pointer_param")?;
    let generic_owner = fixture_owner_id(&db, "call_generic_fn_once_value_binding")?;
    let boxed_owner = fixture_owner_id(&db, "call_boxed_dyn_fn_value_binding")?;
    let vec_owner = fixture_owner_id(&db, "call_prelude_vec_new")?;
    let mut rag = init_test_rag_mock(Arc::clone(&db));
    rag.cfg.call_context.max_owner_hits = 64;
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable callable path call context"
    );

    let call_context = rag.collect_call_context(&[
        (returned_owner, 1.0),
        (returned_param_owner, 1.0),
        (returned_conflicting_local_owner, 1.0),
        (returned_conflicting_other_owner, 1.0),
        (returned_closure_owner, 1.0),
        (bound_owner, 1.0),
        (alias_owner, 1.0),
        (closure_owner, 1.0),
        (closure_producer, 1.0),
        (returned_async_no_await_owner, 1.0),
        (returned_async_awaited_owner, 1.0),
        (returned_async_stored_owner, 1.0),
        (future_owner, 1.0),
        (future_producer, 1.0),
        (branch_owner, 1.0),
        (block_owner, 1.0),
        (fn_param_owner, 1.0),
        (single_param_owner, 1.0),
        (multi_param_owner, 1.0),
        (forwarded_param_owner, 1.0),
        (two_hop_forwarded_param_owner, 1.0),
        (forwarded_referenced_owner, 1.0),
        (two_hop_forwarded_referenced_owner, 1.0),
        (multi_conflicting_param_owner, 1.0),
        (forwarded_conflicting_owner, 1.0),
        (two_hop_forwarded_conflicting_owner, 1.0),
        (multi_conflicting_generic_owner, 1.0),
        (multi_conflicting_field_owner, 1.0),
        (forwarded_conflicting_field_owner, 1.0),
        (single_parenthesized_param_owner, 1.0),
        (generic_owner, 1.0),
        (boxed_owner, 1.0),
        (vec_owner, 1.0),
    ])?;

    let resolved_cases = resolved_cases! {
        path("inner make_fn path call", returned_owner, Some(2), &["make_fn"], make_fn);
        dynamic("outer returned-function dynamic call", returned_owner, None, local_target);
        path("inner returned-parameter helper path call", returned_param_owner, Some(2),
            &["return_forwarded_function_pointer"], returned_param_helper);
        dynamic("outer returned-parameter dynamic call", returned_param_owner, None, local_target);
        path("returned conflicting local caller", returned_conflicting_local_owner, Some(2),
            &["return_conflicting_forwarded_function_pointer"], returned_conflicting_helper);
        path("returned conflicting other caller", returned_conflicting_other_owner, Some(2),
            &["return_conflicting_forwarded_function_pointer"], returned_conflicting_helper);
        path("make_closure", returned_closure_owner, Some(2), &["make_closure"], make_closure);
        path("make_bound_closure", bound_owner, Some(2), &["make_bound_closure"], make_bound);
        path("make_alias_bound_closure", alias_owner, Some(2), &["make_alias_bound_closure"], make_alias);
        path("make_forwarded_returned_closure", closure_owner, Some(2),
            &["make_forwarded_returned_closure"], closure_producer);
        path_from("incoming returned closure caller", closure_producer, Some(2), closure_owner,
            &["make_forwarded_returned_closure"], closure_producer);
        path_from("closure maker path call", closure_producer, None, closure_producer,
            &["make_target_closure"], target_maker);
        path("un-awaited returned async closure", returned_async_no_await_owner, Some(2),
            &["make_returned_async_closure"], make_returned_async);
        path("awaited returned async closure", returned_async_awaited_owner, Some(2),
            &["make_returned_async_closure"], make_returned_async);
        path("stored returned async closure future", returned_async_stored_owner, Some(2),
            &["make_returned_async_closure"], make_returned_async);
        path("forwarded returned async future caller", future_owner, Some(1),
            &["make_forwarded_returned_async_future"], future_producer);
        path_from("incoming forwarded future caller", future_producer, Some(5), future_owner,
            &["make_forwarded_returned_async_future"], future_producer);
        path_from("incoming stored forwarded future caller", future_producer, Some(5), stored_future_owner,
            &["make_forwarded_returned_async_future"], future_producer);
        path_from("incoming aliased forwarded future caller", future_producer, Some(5), aliased_future_owner,
            &["make_forwarded_returned_async_future"], future_producer);
        path_from("returned async closure maker path call", future_producer, None, future_producer,
            &["make_returned_async_closure"], make_returned_async);
        path("branch-initialized function item", branch_owner, Some(1), &["f"], local_target);
        path("block-initialized function item", block_owner, Some(1), &["f"], local_target);
        path("single-caller function-pointer param", single_param_owner, None, &["f"], local_target);
        path("same-target multi-caller function-pointer param", multi_param_owner, None,
            &["f"], local_target);
        path("forwarded function-pointer leaf", forwarded_param_owner, None, &["f"], local_target);
        path("two-hop forwarded function-pointer leaf", two_hop_forwarded_param_owner, None,
            &["f"], local_target);
        path("forwarded referenced dyn Fn leaf", forwarded_referenced_owner, None,
            &["f"], local_target);
        path("two-hop forwarded referenced dyn Fn leaf", two_hop_forwarded_referenced_owner, None,
            &["f"], local_target);
        dynamic("single-caller parenthesized function-pointer param",
            single_parenthesized_param_owner, None, local_target);
        path("boxed dyn Fn path call", boxed_owner, Some(2), &["boxed_fn"], local_target);
    };
    for case in &resolved_cases {
        let context = call_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} should receive call context", case.label));
        assert_resolved(context, case);
    }

    let targetless_cases = [
        TargetlessCase::dynamic(
            "un-awaited returned async closure",
            returned_async_no_await_owner,
            CallStatusKind::Unsupported,
        ),
        TargetlessCase::dynamic(
            "non-local returned future flow",
            future_producer,
            CallStatusKind::Unsupported,
        ),
        TargetlessCase::path(
            "opaque fn pointer path call",
            fn_param_owner,
            Some(1),
            &["f"],
            CallStatusKind::Unsupported,
        ),
        TargetlessCase::path(
            "generic FnOnce path call",
            generic_owner,
            Some(1),
            &["generic_f"],
            CallStatusKind::Unsupported,
        ),
        TargetlessCase::path(
            "Box::new setup call",
            boxed_owner,
            None,
            &["Box", "new"],
            CallStatusKind::External,
        ),
        TargetlessCase::path(
            "Vec::new external call",
            vec_owner,
            Some(1),
            &["Vec", "new"],
            CallStatusKind::External,
        ),
    ];
    for case in &targetless_cases {
        let context = call_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} should receive call context", case.label));
        assert_targetless(context, case);
    }

    let ambiguous_cases = [
        AmbiguousCase::dynamic(
            "returned conflicting local caller",
            returned_conflicting_local_owner,
        ),
        AmbiguousCase::dynamic(
            "returned conflicting other caller",
            returned_conflicting_other_owner,
        ),
        AmbiguousCase::path(
            "conflicting multi-caller function-pointer",
            multi_conflicting_param_owner,
            &["f"],
        ),
        AmbiguousCase::path(
            "forwarded conflicting function-pointer",
            forwarded_conflicting_owner,
            &["f"],
        ),
        AmbiguousCase::path(
            "two-hop forwarded conflicting function-pointer",
            two_hop_forwarded_conflicting_owner,
            &["f"],
        ),
        AmbiguousCase::path(
            "conflicting multi-caller generic FnOnce",
            multi_conflicting_generic_owner,
            &["generic_f"],
        ),
        AmbiguousCase::dynamic(
            "conflicting multi-caller named-field",
            multi_conflicting_field_owner,
        ),
        AmbiguousCase::dynamic(
            "forwarded conflicting named-field",
            forwarded_conflicting_field_owner,
        ),
    ];
    for case in &ambiguous_cases {
        let context = call_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} should receive call context", case.label));
        assert_ambiguous(context, case, local_target, other_target);
    }

    let closure_cases = [
        ClosureCase {
            label: "make_closure",
            owner: returned_closure_owner,
            maker: make_closure,
        },
        ClosureCase {
            label: "make_bound_closure",
            owner: bound_owner,
            maker: make_bound,
        },
        ClosureCase {
            label: "make_alias_bound_closure",
            owner: alias_owner,
            maker: make_alias,
        },
        ClosureCase {
            label: "make_forwarded_returned_closure",
            owner: closure_owner,
            maker: closure_producer,
        },
        ClosureCase {
            label: "awaited returned async closure",
            owner: returned_async_awaited_owner,
            maker: make_returned_async,
        },
        ClosureCase {
            label: "stored returned async closure future",
            owner: returned_async_stored_owner,
            maker: make_returned_async,
        },
    ];
    for case in &closure_cases {
        let context = call_context
            .get(&case.owner)
            .unwrap_or_else(|| panic!("{} should receive call context", case.label));
        assert_closure(context, case);
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_generic_fn_once_parameter()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    struct Case {
        owner: &'static str,
        source: &'static str,
        kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
    }

    let cases = [
        Case {
            owner: "call_single_generic_fn_once_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1519-1524 `generic_f()`",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["generic_f".to_string()],
            },
            relation: CallTargetKind::Function,
        },
        Case {
            owner: "call_multi_generic_fn_once_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1764-1769 `generic_f()`",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["generic_f".to_string()],
            },
            relation: CallTargetKind::Function,
        },
        Case {
            owner: "call_single_parenthesized_generic_fn_once_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1729-1734 `(generic_f)()`",
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
    ];
    let owners = cases
        .iter()
        .map(|case| {
            one_uuid(&db, &function_in_module_query(&["crate"], case.owner)).map(|id| (id, 1.0))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let call_context = rag.collect_call_context(&owners)?;

    for (case, (owner, _score)) in cases.iter().zip(owners.iter().copied()) {
        let context = call_context.get(&owner).unwrap_or_else(|| {
            panic!(
                "{} single-caller generic FnOnce owner should receive outgoing call context",
                case.owner
            )
        });
        let call = context
            .iter()
            .find(|call| {
                call.kind == case.kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target| target.target_id == local_target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should include resolved generic FnOnce parameter call -> local_target from {}: {context:#?}",
                    case.owner, case.source
                )
            });

        // These private helpers each have one local caller that supplies
        // `local_target`, so this is exact value-flow proof, not broad
        // callable-trait dispatch.
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, local_target);
        assert_eq!(call.targets[0].relation, case.relation);
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_callable_trait_object_parameters()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    struct Case {
        owner: &'static str,
        source: &'static str,
        kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
        proof_note: &'static str,
    }

    let cases = [
        Case {
            owner: "call_single_referenced_dyn_fn_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:2148-2153 `f()` with `&dyn Fn` parameter",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["f".to_string()],
            },
            relation: CallTargetKind::Function,
            proof_note: "caller supplies `&local_target`",
        },
        Case {
            owner: "call_single_parenthesized_referenced_dyn_fn_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:2156-2161 `(f)()` with `&dyn Fn` parameter",
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
            proof_note: "caller supplies `&local_target`",
        },
        Case {
            owner: "call_single_boxed_dyn_fn_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs EOF `f()` with `Box<dyn Fn>` parameter",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["f".to_string()],
            },
            relation: CallTargetKind::Function,
            proof_note: "caller supplies `Box::new(local_target)`",
        },
        Case {
            owner: "call_single_parenthesized_boxed_dyn_fn_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs EOF `(f)()` with `Box<dyn Fn>` parameter",
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
            proof_note: "caller supplies `Box::new(local_target)`",
        },
        Case {
            owner: "call_forwarded_boxed_dyn_fn_leaf",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs EOF `f()` with forwarded `Box<dyn Fn>` parameter",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["f".to_string()],
            },
            relation: CallTargetKind::Function,
            proof_note: "private forwarding chain ends in `Box::new(local_target)`",
        },
        Case {
            owner: "call_two_hop_forwarded_boxed_dyn_fn_leaf",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs EOF `f()` with two-hop forwarded `Box<dyn Fn>` parameter",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["f".to_string()],
            },
            relation: CallTargetKind::Function,
            proof_note: "two-hop private forwarding chain ends in `Box::new(local_target)`",
        },
    ];
    let owners = cases
        .iter()
        .map(|case| {
            one_uuid(&db, &function_in_module_query(&["crate"], case.owner)).map(|id| (id, 1.0))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let call_context = rag.collect_call_context(&owners)?;

    for (case, (owner, _score)) in cases.iter().zip(owners.iter().copied()) {
        let context = call_context.get(&owner).unwrap_or_else(|| {
            panic!(
                "{} callable trait-object owner should receive outgoing call context",
                case.owner
            )
        });
        let call = context
            .iter()
            .find(|call| {
                call.kind == case.kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target| target.target_id == local_target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should include resolved callable trait-object parameter call -> local_target from {} ({note}): {context:#?}",
                    case.owner,
                    case.source,
                    note = case.proof_note
                )
            });

        // These private helpers each have one local caller that supplies
        // the exact callable target, so this is complete private-caller proof,
        // not broad callable trait-object dispatch.
        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, local_target);
        assert_eq!(call.targets[0].relation, case.relation);
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_result_method_callback()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_result_callback"),
    )?;
    let local_target = unique_id_by_name(&db, "function", "local_result_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .expect("single-caller result callback owner should receive outgoing call context");
    let call = context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "and_then".to_string(),
                        receiver: Some(CallReceiverInfo::PathCallResult {
                            path: vec!["Ok".to_string()],
                        }),
                    }
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == local_target)
        })
        .unwrap_or_else(|| {
            panic!(
                "single-caller result callback context should include resolved and_then(f) -> local_result_target: {context:#?}"
            )
        });

    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // `call_single_result_callback(f)` is private and every local caller passes
    // `local_result_target`, so this is complete private-caller proof for the
    // callback argument of `Ok::<i32, ()>(1).and_then(f)`.
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, local_target);
    assert_eq!(
        call.targets[0].relation,
        CallTargetKind::MethodCallbackFunction
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_branch_parameter_calls()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    let cases = [
        (
            "call_single_if_function_pointer_param_branch",
            "tests/fixture_crates/fixture_call_graph/src/lib.rs:1655 `(if flag { f } else { f })()`",
        ),
        (
            "call_single_match_function_pointer_param_arm",
            "tests/fixture_crates/fixture_call_graph/src/lib.rs:1663-1666 match arms return `f`",
        ),
    ];
    let owners = cases
        .iter()
        .map(|(owner, _source)| {
            one_uuid(&db, &function_in_module_query(&["crate"], owner)).map(|id| (id, 1.0))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let call_context = rag.collect_call_context(&owners)?;

    for ((owner_name, source), (owner, _score)) in cases.iter().zip(owners.iter().copied()) {
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{owner_name} should receive outgoing call context"));
        let call = context
            .iter()
            .find(|call| {
                call.kind == CallSiteKind::Dynamic
                    && call.callee == CallCalleeInfo::Dynamic
                    && call
                        .targets
                        .iter()
                        .any(|target| target.target_id == local_target)
            })
            .unwrap_or_else(|| {
                panic!("{owner_name} should include resolved branch parameter call -> local_target from {source}: {context:#?}")
            });

        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, local_target);
        assert_eq!(call.targets[0].relation, CallTargetKind::DynamicFunction);
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_aliased_callable_parameters()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    struct Case {
        owner: &'static str,
        source: &'static str,
        kind: CallSiteKind,
        callee: CallCalleeInfo,
        relation: CallTargetKind,
    }

    let cases = [
        Case {
            owner: "call_single_aliased_function_pointer_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1673-1675 `let g = f; g()`",
            kind: CallSiteKind::Path,
            callee: CallCalleeInfo::Path {
                path: vec!["g".to_string()],
            },
            relation: CallTargetKind::Function,
        },
        Case {
            owner: "call_single_parenthesized_aliased_function_pointer_param",
            source: "tests/fixture_crates/fixture_call_graph/src/lib.rs:1682-1684 `let g = f; (g)()`",
            kind: CallSiteKind::Dynamic,
            callee: CallCalleeInfo::Dynamic,
            relation: CallTargetKind::DynamicFunction,
        },
    ];
    let owners = cases
        .iter()
        .map(|case| {
            one_uuid(&db, &function_in_module_query(&["crate"], case.owner)).map(|id| (id, 1.0))
        })
        .collect::<Result<Vec<_>, _>>()?;

    let call_context = rag.collect_call_context(&owners)?;

    for (case, (owner, _score)) in cases.iter().zip(owners.iter().copied()) {
        let context = call_context
            .get(&owner)
            .unwrap_or_else(|| panic!("{} should receive outgoing call context", case.owner));
        let call = context
            .iter()
            .find(|call| {
                call.kind == case.kind
                    && call.callee == case.callee
                    && call
                        .targets
                        .iter()
                        .any(|target| target.target_id == local_target)
            })
            .unwrap_or_else(|| {
                panic!(
                    "{} should include resolved aliased parameter call -> local_target from {}: {context:#?}",
                    case.owner, case.source
                )
            });

        assert_eq!(call.status, CallStatusKind::Resolved);
        assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(call.targets.len(), 1);
        assert_eq!(call.targets[0].target_id, local_target);
        assert_eq!(call.targets[0].relation, case.relation);
    }

    Ok(())
}

#[tokio::test]
async fn call_context_collection_resolves_private_single_caller_function_pointer_cast_parameter()
-> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_single_function_pointer_param_cast"),
    )?;
    let local_target = unique_id_by_name(&db, "function", "local_target")?;
    let rag = init_test_rag_mock(Arc::clone(&db));

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let context = call_context
        .get(&owner)
        .expect("single-caller function-pointer cast owner should receive outgoing call context");
    let call = context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Dynamic
                && call.callee == CallCalleeInfo::Dynamic
                && call
                    .targets
                    .iter()
                    .any(|target| target.target_id == local_target)
        })
        .unwrap_or_else(|| {
            panic!(
                "single-caller function-pointer cast context should include resolved (f as fn() -> i32)() -> local_target: {context:#?}"
            )
        });

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1564-1569:
    // the helper is private and every local caller supplies `local_target`,
    // so this cast form reuses the existing exact parameter proof boundary.
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, local_target);
    assert_eq!(call.targets[0].relation, CallTargetKind::DynamicFunction);

    Ok(())
}
