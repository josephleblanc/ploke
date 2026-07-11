use super::*;
use cozo::DataValue;
use ploke_db::{CallNodeKind, CallPathOptions};

#[test]
fn fixture_context_does_not_project_closure_or_async_body_calls_to_outer_owner()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let forbidden_path = path(&["local_target"]);
    let owners = [
        "closure_body_call_is_not_outer_call_site",
        "async_block_call_is_not_outer_call_site",
        "call_move_closure_literal_with_body_call",
        "call_async_closure_literal_with_body_call",
        "call_async_closure_binding_without_await_with_body_call",
        "call_awaited_async_closure_binding_with_body_call",
        "call_async_closure_future_binding_without_await_with_body_call",
        "call_awaited_async_closure_future_binding_with_body_call",
    ];

    for owner_name in owners {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert!(
            context.iter().all(|row| row.site.kind != CallSiteKind::Path
                || row.site.path.as_ref() != Some(&forbidden_path)),
            "{owner_name} leaked a closure/async body local_target() path row into the outer owner: {context:#?}"
        );
        assert!(
            context
                .iter()
                .flat_map(|row| row.targets.iter())
                .all(|target| target.target_id != local_target),
            "{owner_name} leaked a closure/async body edge to local_target into the outer owner: {context:#?}"
        );
    }

    Ok(())
}

#[derive(Clone, Copy)]
struct LocalItemCase {
    owner_name: &'static str,
    label: &'static str,
    source_line: u32,
}

const LOCAL_ITEM_CASES: &[LocalItemCase] = &[
    LocalItemCase {
        owner_name: "local_const_initializer_call_is_not_outer_call_site",
        label: "local_const",
        source_line: 1372,
    },
    LocalItemCase {
        owner_name: "local_static_initializer_call_is_not_outer_call_site",
        label: "local_static",
        source_line: 1435,
    },
    LocalItemCase {
        owner_name: "local_fn_body_call_is_not_outer_call_site",
        label: "local_fn:inner",
        source_line: 1441,
    },
    LocalItemCase {
        owner_name: "local_impl_method_body_call_is_not_outer_call_site",
        label: "local_impl_method:value",
        source_line: 1461,
    },
];

#[test]
fn fixture_context_does_not_project_local_item_initializer_calls_to_outer_owner()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "assoc_const_value")?;
    let forbidden_path = path(&["assoc_const_value"]);

    for case in LOCAL_ITEM_CASES {
        let owner = function_id_by_name(&db, case.owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert!(
            context.iter().all(|row| row.site.kind != CallSiteKind::Path
                || row.site.path.as_ref() != Some(&forbidden_path)),
            "{} assoc_const_value() path row leaked into the outer owner: {context:#?}",
            case.label
        );
        assert!(
            context
                .iter()
                .flat_map(|row| row.targets.iter())
                .all(|edge| edge.target_id != target),
            "{} edge to assoc_const_value leaked into the outer owner: {context:#?}",
            case.label
        );
    }

    Ok(())
}

#[test]
fn fixture_context_projects_local_item_initializer_calls_to_executable_owner() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "assoc_const_value")?;

    for case in LOCAL_ITEM_CASES {
        assert_local_item_initializer_owner(&db, case, target)?;
    }

    Ok(())
}

#[test]
fn fixture_context_projects_outer_local_fn_call_to_local_item_target() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // local_fn_body_call_is_not_outer_call_site defines block-local `fn inner()`
    // and the outer function calls it as `inner()`. The target should be the
    // executable local-item body, not a top-level FunctionNode.
    assert_outer_local_fn_call_to_local_item(&db, "local_fn_body_call_is_not_outer_call_site")
}

#[test]
fn fixture_context_projects_forward_outer_local_fn_call_to_local_item_target() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // local_fn_forward_call_resolves_local_item calls `inner()` before the
    // block-local `fn inner()` item is visited. Rust item declarations are
    // block-scoped, so the persisted call graph should still target the
    // executable local-item body.
    assert_outer_local_fn_call_to_local_item(&db, "local_fn_forward_call_resolves_local_item")
}

#[test]
fn fixture_context_projects_macro_generated_local_fn_call_to_local_item_target()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner_name = "call_item_macro_generated_function";
    let outer = function_id_by_name(&db, owner_name)?;
    let context = db.call_context_for_owner(outer)?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // call_item_macro_generated_function invokes call_graph_item_macro!(), which
    // expands to a block-local `fn generated_by_item_macro()`, then calls it.
    // The call edge should target the generated local-item body while the macro
    // invocation row itself remains targetless and unsupported.
    assert_targetless_macro_row(
        &context,
        outer,
        TargetlessMacroCase::macro_call("call_graph_item_macro", owner_name),
    );

    assert_outer_local_fn_call_to_named_local_item(
        &db,
        owner_name,
        "local_fn:generated_by_item_macro",
        &["generated_by_item_macro"],
    )
}

fn assert_outer_local_fn_call_to_local_item(
    db: &Database,
    owner_name: &str,
) -> Result<(), DbError> {
    assert_outer_local_fn_call_to_named_local_item(db, owner_name, "local_fn:inner", &["inner"])
}

fn assert_outer_local_fn_call_to_named_local_item(
    db: &Database,
    owner_name: &str,
    label: &str,
    call_path: &[&str],
) -> Result<(), DbError> {
    let outer = function_id_by_name(db, owner_name)?;
    let local_fn = local_item_owner_for_parent_with_label(db, outer, label)?;

    let outer_context = db.call_context_for_owner(outer)?;
    let row = row_by_path(&outer_context, call_path);
    assert_eq!(row.site.owner_id, outer);
    assert_resolved_target(
        row,
        local_fn,
        CallRelationKind::LocalFunction,
        CallSiteKind::Path,
        CallTargetKind::LocalItem,
    );

    let callers = db.callers_for_target(local_fn)?;
    let caller = caller_by_owner_kind_path(&callers, outer, CallSiteKind::Path, call_path);
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.relation, CallRelationKind::LocalFunction);
    assert_eq!(caller.target.target_kind, CallTargetKind::LocalItem);

    let paths = db.call_paths_from_owner(
        outer,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == outer && path.end_id == local_fn && path.depth == 1)
        .unwrap_or_else(|| panic!("outer owner should have a one-hop path to {label}"));
    assert_eq!(path.edges[0].caller_id, outer);
    assert_eq!(path.edges[0].callee_id, local_fn);
    assert_eq!(path.edges[0].relation, CallRelationKind::LocalFunction);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::LocalItem);

    Ok(())
}

fn assert_local_item_initializer_owner(
    db: &Database,
    case: &LocalItemCase,
    target: uuid::Uuid,
) -> Result<(), DbError> {
    let outer = function_id_by_name(db, case.owner_name)?;
    let local_item = local_item_owner_for_parent_with_label(db, outer, case.label)?;

    let outer_context = db.call_context_for_owner(outer)?;
    assert!(
        outer_context.iter().all(|row| row.site.owner_id == outer),
        "outer function context should only contain rows owned by the outer function: {outer_context:#?}"
    );
    let assoc_const_value_path = path(&["assoc_const_value"]);
    assert!(
        outer_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&assoc_const_value_path)),
        "outer function should not absorb the {} initializer assoc_const_value() row: {outer_context:#?}",
        case.label
    );
    assert!(
        outer_context
            .iter()
            .flat_map(|row| row.targets.iter())
            .all(|edge| edge.target_id != target),
        "outer function should not expose a fabricated edge to assoc_const_value: {outer_context:#?}"
    );

    let info = db.call_node_info(local_item)?.unwrap_or_else(|| {
        panic!(
            "{} call_body_owner should expose call-node metadata",
            case.label
        )
    });
    assert_eq!(info.kind, CallNodeKind::LocalItem);
    assert_eq!(info.name, case.label);
    assert!(
        info.file_path.ends_with("fixture_call_graph/src/lib.rs"),
        "{} owner at fixture_call_graph/src/lib.rs:{} should inherit the parent source file: {info:#?}",
        case.label,
        case.source_line
    );

    // The source line for each case is listed in LOCAL_ITEM_CASES above.
    // The function-local item initializer calls `assoc_const_value()`. That
    // initializer call belongs to the local-item owner, not the enclosing
    // function owner.
    let local_item_context = db.call_context_for_owner(local_item)?;
    let row = row_by_path(&local_item_context, &["assoc_const_value"]);
    assert_eq!(row.site.owner_id, local_item);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let callers = db.callers_for_target(target)?;
    let caller = caller_by_owner_kind_path(
        &callers,
        local_item,
        CallSiteKind::Path,
        &["assoc_const_value"],
    );
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.relation, CallRelationKind::Function);

    let paths = db.call_paths_from_owner(
        local_item,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == local_item && path.end_id == target && path.depth == 1)
        .unwrap_or_else(|| {
            panic!(
                "{} owner should have a one-hop path to assoc_const_value",
                case.label
            )
        });
    assert_eq!(path.edges[0].caller_id, local_item);
    assert_eq!(path.edges[0].callee_id, target);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_context_projects_closure_body_call_to_executable_owner() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let outer = function_id_by_name(&db, "closure_body_call_is_not_outer_call_site")?;
    let closure = closure_owner_for_parent(&db, outer)?;

    let outer_context = db.call_context_for_owner(outer)?;
    assert!(
        outer_context.iter().all(|row| row.site.owner_id == outer),
        "outer function context should only contain rows owned by the outer function: {outer_context:#?}"
    );
    let local_target_path = path(&["local_target"]);
    assert!(
        outer_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&local_target_path)),
        "outer function should not absorb the closure-body local_target() row: {outer_context:#?}"
    );
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:155-157:
    // `closure()` is a path-style call to the local closure binding. It should
    // target the closure owner so graph traversal can continue into the
    // closure body.
    let closure_call = row_by_path(&outer_context, &["closure"]);
    assert_resolved_target(
        closure_call,
        closure,
        CallRelationKind::Closure,
        CallSiteKind::Path,
        CallTargetKind::Closure,
    );

    let info = db
        .call_node_info(closure)?
        .expect("closure call_body_owner should expose call-node metadata");
    assert_eq!(info.kind, CallNodeKind::Closure);
    assert_eq!(info.name, "closure");
    assert!(
        info.file_path.ends_with("fixture_call_graph/src/lib.rs"),
        "closure owner should inherit the parent source file: {info:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:155-157:
    // `closure_body_call_is_not_outer_call_site` binds `|| local_target()` and
    // invokes the closure. The call graph models the body call under the
    // closure owner, not under the outer function owner.
    let closure_context = db.call_context_for_owner(closure)?;
    let row = row_by_path(&closure_context, &["local_target"]);
    assert_eq!(row.site.owner_id, closure);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let callers = db.callers_for_target(target)?;
    let caller =
        caller_by_owner_kind_path(&callers, closure, CallSiteKind::Path, &["local_target"]);
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.relation, CallRelationKind::Function);

    let paths = db.call_paths_from_owner(
        closure,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert!(
        paths
            .iter()
            .any(|path| path.start_id == closure && path.end_id == target && path.depth == 1),
        "closure owner should have a one-hop resolved path to local_target: {paths:#?}"
    );
    let paths = db.call_paths_from_owner(
        outer,
        CallPathOptions {
            max_depth: 2,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == outer && path.end_id == target && path.depth == 2)
        .expect("outer function should have a two-hop path to local_target through the closure");
    assert_eq!(path.edges[0].caller_id, outer);
    assert_eq!(path.edges[0].callee_id, closure);
    assert_eq!(path.edges[0].relation, CallRelationKind::Closure);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Closure);
    assert_eq!(path.edges[1].caller_id, closure);
    assert_eq!(path.edges[1].callee_id, target);
    assert_eq!(path.edges[1].relation, CallRelationKind::Function);
    assert_eq!(path.edges[1].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_context_projects_move_closure_literal_call_to_executable_owner() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let outer = function_id_by_name(&db, "call_move_closure_literal_with_body_call")?;
    let closure = closure_owner_for_parent(&db, outer)?;

    let outer_context = db.call_context_for_owner(outer)?;
    assert!(
        outer_context.iter().all(|row| row.site.owner_id == outer),
        "outer function context should only contain rows owned by the outer function: {outer_context:#?}"
    );
    let local_target_path = path(&["local_target"]);
    assert!(
        outer_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&local_target_path)),
        "outer function should not absorb the move-closure local_target() row: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:710:
    // `(move || local_target())()` directly invokes a non-async closure
    // literal. The outer dynamic call should target the closure owner, and the
    // closure owner should then own the body call to `local_target()`.
    let dynamic_row = outer_context
        .iter()
        .find(|row| row.site.kind == CallSiteKind::Dynamic && row.site.path.is_none())
        .expect("move closure literal should project a pathless dynamic call row");
    assert_resolved_target(
        dynamic_row,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );

    let closure_context = db.call_context_for_owner(closure)?;
    let row = row_by_path(&closure_context, &["local_target"]);
    assert_eq!(row.site.owner_id, closure);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_from_owner(
        outer,
        CallPathOptions {
            max_depth: 2,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == outer && path.end_id == target && path.depth == 2)
        .expect("outer function should have a two-hop path through the move closure literal");
    assert_eq!(path.edges[0].caller_id, outer);
    assert_eq!(path.edges[0].callee_id, closure);
    assert_eq!(path.edges[0].relation, CallRelationKind::DynamicClosure);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Closure);
    assert_eq!(path.edges[1].caller_id, closure);
    assert_eq!(path.edges[1].callee_id, target);
    assert_eq!(path.edges[1].relation, CallRelationKind::Function);
    assert_eq!(path.edges[1].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_context_projects_async_block_call_to_executable_owner() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let outer = function_id_by_name(&db, "async_block_call_is_not_outer_call_site")?;
    let async_body = async_block_owner_for_parent(&db, outer)?;

    let outer_context = db.call_context_for_owner(outer)?;
    assert!(
        outer_context.iter().all(|row| row.site.owner_id == outer),
        "outer function context should only contain rows owned by the outer function: {outer_context:#?}"
    );
    let local_target_path = path(&["local_target"]);
    assert!(
        outer_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&local_target_path)),
        "outer function should not absorb the async-block local_target() row: {outer_context:#?}"
    );
    assert!(
        outer_context
            .iter()
            .flat_map(|row| row.targets.iter())
            .all(|edge| edge.target_id != target),
        "outer function should not expose a fabricated edge to local_target: {outer_context:#?}"
    );

    let info = db
        .call_node_info(async_body)?
        .expect("async block call_body_owner should expose call-node metadata");
    assert_eq!(info.kind, CallNodeKind::AsyncBlock);
    assert_eq!(info.name, "async_block");
    assert!(
        info.file_path.ends_with("fixture_call_graph/src/lib.rs"),
        "async block owner should inherit the parent source file: {info:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:160-164:
    // `async_block_call_is_not_outer_call_site` creates an async block whose
    // body calls `local_target()`. The body call belongs to the async block
    // owner, not the enclosing function.
    let async_context = db.call_context_for_owner(async_body)?;
    let row = row_by_path(&async_context, &["local_target"]);
    assert_eq!(row.site.owner_id, async_body);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let callers = db.callers_for_target(target)?;
    let caller =
        caller_by_owner_kind_path(&callers, async_body, CallSiteKind::Path, &["local_target"]);
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.relation, CallRelationKind::Function);

    let paths = db.call_paths_from_owner(
        async_body,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == async_body && path.end_id == target && path.depth == 1)
        .expect("async block owner should have a one-hop path to local_target");
    assert_eq!(path.edges[0].caller_id, async_body);
    assert_eq!(path.edges[0].callee_id, target);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_context_projects_async_closure_body_call_to_executable_owner() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let outer = function_id_by_name(&db, "call_async_closure_literal_with_body_call")?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let outer_context = db.call_context_for_owner(outer)?;
    assert!(
        outer_context.iter().all(|row| row.site.owner_id == outer),
        "outer function context should only contain rows owned by the outer function: {outer_context:#?}"
    );
    let local_target_path = path(&["local_target"]);
    assert!(
        outer_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&local_target_path)),
        "outer function should not absorb the async-closure local_target() row: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:848-850:
    // `(async || local_target())()` creates an async closure future. The body
    // call is owned by the async-closure executable owner, but the outer
    // dynamic call stays targetless because invoking the async closure does
    // not itself poll the returned future.
    let dynamic_row = outer_context
        .iter()
        .find(|row| row.site.kind == CallSiteKind::Dynamic && row.site.path.is_none())
        .expect("async closure literal should project a pathless dynamic call row");
    assert_eq!(dynamic_row.status.status, CallStatusKind::Unsupported);
    assert!(
        dynamic_row.targets.is_empty(),
        "async closure invocation must not fabricate a direct body edge: {dynamic_row:#?}"
    );

    let info = db
        .call_node_info(async_closure)?
        .expect("async closure call_body_owner should expose call-node metadata");
    assert_eq!(info.kind, CallNodeKind::Closure);
    assert_eq!(info.name, "async_closure");
    assert!(
        info.file_path.ends_with("fixture_call_graph/src/lib.rs"),
        "async closure owner should inherit the parent source file: {info:#?}"
    );

    let async_closure_context = db.call_context_for_owner(async_closure)?;
    let row = row_by_path(&async_closure_context, &["local_target"]);
    assert_eq!(row.site.owner_id, async_closure);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let callers = db.callers_for_target(target)?;
    let caller = caller_by_owner_kind_path(
        &callers,
        async_closure,
        CallSiteKind::Path,
        &["local_target"],
    );
    assert_eq!(caller.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(caller.target.relation, CallRelationKind::Function);

    let paths = db.call_paths_from_owner(
        async_closure,
        CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == async_closure && path.end_id == target && path.depth == 1)
        .expect("async closure owner should have a one-hop path to local_target");
    assert_eq!(path.edges[0].caller_id, async_closure);
    assert_eq!(path.edges[0].callee_id, target);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Function);

    let outer_paths = db.call_paths_from_owner(
        outer,
        CallPathOptions {
            max_depth: 2,
            max_paths: 8,
        },
    )?;
    assert!(
        outer_paths.iter().all(|path| path.end_id != target),
        "async closure invocation should not imply a two-hop path to local_target until the returned future is polled: {outer_paths:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_resolves_awaited_async_closure_literal_call_to_executable_owner()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let outer = function_id_by_name(&db, "call_awaited_async_closure_literal_with_body_call")?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let outer_context = db.call_context_for_owner(outer)?;
    assert!(
        outer_context.iter().all(|row| row.site.owner_id == outer),
        "outer function context should only contain rows owned by the outer function: {outer_context:#?}"
    );
    let local_target_path = path(&["local_target"]);
    assert!(
        outer_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&local_target_path)),
        "outer function should not absorb the awaited async-closure local_target() row: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1530-1532:
    // `(async || local_target())().await` immediately polls the async closure
    // future. The outer dynamic call can target the async-closure owner, while
    // the body call remains owned by that executable owner.
    let dynamic_row = outer_context
        .iter()
        .find(|row| row.site.kind == CallSiteKind::Dynamic && row.site.path.is_none())
        .expect("awaited async closure literal should project a pathless dynamic call row");
    assert_resolved_target(
        dynamic_row,
        async_closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );

    let async_closure_context = db.call_context_for_owner(async_closure)?;
    let row = row_by_path(&async_closure_context, &["local_target"]);
    assert_eq!(row.site.owner_id, async_closure);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_from_owner(
        outer,
        CallPathOptions {
            max_depth: 2,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == outer && path.end_id == target && path.depth == 2)
        .expect("awaited async closure should expose a two-hop path to local_target");
    assert_eq!(path.edges[0].caller_id, outer);
    assert_eq!(path.edges[0].callee_id, async_closure);
    assert_eq!(path.edges[0].relation, CallRelationKind::DynamicClosure);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Closure);
    assert_eq!(path.edges[1].caller_id, async_closure);
    assert_eq!(path.edges[1].callee_id, target);
    assert_eq!(path.edges[1].relation, CallRelationKind::Function);
    assert_eq!(path.edges[1].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_context_keeps_non_awaited_async_closure_binding_targetless() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let outer = function_id_by_name(
        &db,
        "call_async_closure_binding_without_await_with_body_call",
    )?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let outer_context = db.call_context_for_owner(outer)?;
    assert!(
        outer_context.iter().all(|row| row.site.owner_id == outer),
        "outer function context should only contain rows owned by the outer function: {outer_context:#?}"
    );
    let local_target_path = path(&["local_target"]);
    assert!(
        outer_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&local_target_path)),
        "outer function should not absorb the async-closure binding local_target() row: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1701-1704:
    // `closure()` invokes a local async-closure binding but does not poll the
    // returned future. The call site should preserve the binding proof while
    // remaining targetless.
    let closure_call = row_by_path(&outer_context, &["closure"]);
    assert_eq!(closure_call.status.status, CallStatusKind::Unsupported);
    assert!(
        closure_call.targets.is_empty(),
        "non-awaited async closure binding must not fabricate a closure edge: {closure_call:#?}"
    );

    let async_closure_context = db.call_context_for_owner(async_closure)?;
    let row = row_by_path(&async_closure_context, &["local_target"]);
    assert_eq!(row.site.owner_id, async_closure);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let outer_paths = db.call_paths_from_owner(
        outer,
        CallPathOptions {
            max_depth: 2,
            max_paths: 8,
        },
    )?;
    assert!(
        outer_paths.iter().all(|path| path.end_id != target),
        "non-awaited async closure binding should not expose a path to local_target: {outer_paths:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_resolves_awaited_async_closure_binding_to_executable_owner()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let outer = function_id_by_name(&db, "call_awaited_async_closure_binding_with_body_call")?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let outer_context = db.call_context_for_owner(outer)?;
    assert!(
        outer_context.iter().all(|row| row.site.owner_id == outer),
        "outer function context should only contain rows owned by the outer function: {outer_context:#?}"
    );
    let local_target_path = path(&["local_target"]);
    assert!(
        outer_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&local_target_path)),
        "outer function should not absorb the awaited async-closure binding local_target() row: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1706-1709:
    // `closure().await` immediately polls the async-closure binding. The path
    // call can target the closure owner, and traversal can continue through the
    // closure-body `local_target()` call.
    let closure_call = row_by_path(&outer_context, &["closure"]);
    assert_resolved_target(
        closure_call,
        async_closure,
        CallRelationKind::Closure,
        CallSiteKind::Path,
        CallTargetKind::Closure,
    );

    let async_closure_context = db.call_context_for_owner(async_closure)?;
    let row = row_by_path(&async_closure_context, &["local_target"]);
    assert_eq!(row.site.owner_id, async_closure);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_from_owner(
        outer,
        CallPathOptions {
            max_depth: 2,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == outer && path.end_id == target && path.depth == 2)
        .expect("awaited async closure binding should expose a two-hop path to local_target");
    assert_eq!(path.edges[0].caller_id, outer);
    assert_eq!(path.edges[0].callee_id, async_closure);
    assert_eq!(path.edges[0].relation, CallRelationKind::Closure);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Closure);
    assert_eq!(path.edges[1].caller_id, async_closure);
    assert_eq!(path.edges[1].callee_id, target);
    assert_eq!(path.edges[1].relation, CallRelationKind::Function);
    assert_eq!(path.edges[1].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_context_keeps_non_awaited_async_closure_future_binding_targetless() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let outer = function_id_by_name(
        &db,
        "call_async_closure_future_binding_without_await_with_body_call",
    )?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let outer_context = db.call_context_for_owner(outer)?;
    let local_target_path = path(&["local_target"]);
    assert!(
        outer_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&local_target_path)),
        "outer function should not absorb the async-closure future body call: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1711-1714:
    // `_future = closure()` stores the async-closure future but never awaits it.
    // The original path call remains targetless.
    let closure_call = row_by_path(&outer_context, &["closure"]);
    assert_eq!(closure_call.status.status, CallStatusKind::Unsupported);
    assert!(
        closure_call.targets.is_empty(),
        "unawaited async closure future binding must not fabricate a closure edge: {closure_call:#?}"
    );

    let async_closure_context = db.call_context_for_owner(async_closure)?;
    let row = row_by_path(&async_closure_context, &["local_target"]);
    assert_eq!(row.site.owner_id, async_closure);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let outer_paths = db.call_paths_from_owner(
        outer,
        CallPathOptions {
            max_depth: 2,
            max_paths: 8,
        },
    )?;
    assert!(
        outer_paths.iter().all(|path| path.end_id != target),
        "unawaited async closure future binding should not expose a path to local_target: {outer_paths:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_resolves_awaited_async_closure_future_binding_to_executable_owner()
-> Result<(), DbError> {
    assert_awaited_async_closure_future_path(
        "call_awaited_async_closure_future_binding_with_body_call",
        "awaited async-closure future binding",
        "awaited async closure future binding",
    )
}

#[test]
fn fixture_context_resolves_awaited_async_closure_future_alias_to_executable_owner()
-> Result<(), DbError> {
    assert_awaited_async_closure_future_path(
        "call_awaited_async_closure_future_alias_with_body_call",
        "awaited async-closure future alias",
        "awaited async closure future alias",
    )
}

#[test]
fn fixture_context_resolves_awaited_async_closure_future_block_alias_to_executable_owner()
-> Result<(), DbError> {
    assert_awaited_async_closure_future_path(
        "call_awaited_async_closure_future_block_alias_with_body_call",
        "awaited async-closure future block alias",
        "awaited async closure future block alias",
    )
}

#[test]
fn fixture_context_resolves_awaited_async_closure_future_alias_chain_to_executable_owner()
-> Result<(), DbError> {
    assert_awaited_async_closure_future_path(
        "call_awaited_async_closure_future_alias_chain_with_body_call",
        "awaited async-closure future alias chain",
        "awaited async closure future alias chain",
    )
}

fn assert_awaited_async_closure_future_path(
    owner_name: &str,
    body_label: &str,
    path_label: &str,
) -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let outer = function_id_by_name(&db, owner_name)?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;

    let outer_context = db.call_context_for_owner(outer)?;
    let local_target_path = path(&["local_target"]);
    assert!(
        outer_context
            .iter()
            .all(|row| row.site.path.as_ref() != Some(&local_target_path)),
        "outer function should not absorb the {body_label} body call: {outer_context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1716-1720:
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1722-1727:
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1821-1825:
    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1913-1919:
    // `future = closure(); future.await;`, `future = closure(); alias =
    // future; alias.await;`, and `future = closure(); alias = { future };
    // alias.await;`, and `future = closure(); alias = future; second = alias;
    // second.await;` prove the original async-closure binding call is polled
    // through bounded same-block evidence.
    let closure_call = row_by_path(&outer_context, &["closure"]);
    assert_resolved_target(
        closure_call,
        async_closure,
        CallRelationKind::Closure,
        CallSiteKind::Path,
        CallTargetKind::Closure,
    );

    let async_closure_context = db.call_context_for_owner(async_closure)?;
    let row = row_by_path(&async_closure_context, &["local_target"]);
    assert_eq!(row.site.owner_id, async_closure);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_from_owner(
        outer,
        CallPathOptions {
            max_depth: 2,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == outer && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| panic!("{path_label} should expose a two-hop path to local_target"));
    assert_eq!(path.edges[0].caller_id, outer);
    assert_eq!(path.edges[0].callee_id, async_closure);
    assert_eq!(path.edges[0].relation, CallRelationKind::Closure);
    assert_eq!(path.edges[0].target_kind, CallTargetKind::Closure);
    assert_eq!(path.edges[1].caller_id, async_closure);
    assert_eq!(path.edges[1].callee_id, target);
    assert_eq!(path.edges[1].relation, CallRelationKind::Function);
    assert_eq!(path.edges[1].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_snippet_metadata_materializes_closure_body_owner() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let outer = function_id_by_name(&db, "closure_body_call_is_not_outer_call_site")?;
    let closure = closure_owner_for_parent(&db, outer)?;
    let span = body_owner_span(&db, closure)?;

    let nodes = db
        .get_snippet_context_nodes_ordered(vec![closure])
        .map_err(|err| DbError::Cozo(err.to_string()))?;
    assert_eq!(
        nodes.len(),
        1,
        "closure call_body_owner should be snippet-materializable for RAG expansion"
    );
    let (node, paths) = &nodes[0];
    assert_eq!(node.id, closure);
    assert_eq!(node.name, "closure");
    assert_eq!((node.start_byte, node.end_byte), span);
    assert!(
        node.file_path.ends_with("fixture_call_graph/src/lib.rs"),
        "closure owner should materialize using the parent source file: {node:#?}"
    );
    assert!(
        paths.file.ends_with("fixture_call_graph/src/lib.rs"),
        "closure owner path metadata should inherit the parent source file: {paths:#?}"
    );
    assert_eq!(
        paths.canon, "crate::closure",
        "closure owner canon path should use the parent module path plus the executable label"
    );

    Ok(())
}

#[test]
fn fixture_snippet_metadata_materializes_async_block_owner() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let outer = function_id_by_name(&db, "async_block_call_is_not_outer_call_site")?;
    let async_body = async_block_owner_for_parent(&db, outer)?;
    let span = body_owner_span(&db, async_body)?;

    let nodes = db
        .get_snippet_context_nodes_ordered(vec![async_body])
        .map_err(|err| DbError::Cozo(err.to_string()))?;
    assert_eq!(
        nodes.len(),
        1,
        "async block call_body_owner should be snippet-materializable for RAG expansion"
    );
    let (node, paths) = &nodes[0];
    assert_eq!(node.id, async_body);
    assert_eq!(node.name, "async_block");
    assert_eq!((node.start_byte, node.end_byte), span);
    assert!(
        node.file_path.ends_with("fixture_call_graph/src/lib.rs"),
        "async block owner should materialize using the parent source file: {node:#?}"
    );
    assert!(
        paths.file.ends_with("fixture_call_graph/src/lib.rs"),
        "async block owner path metadata should inherit the parent source file: {paths:#?}"
    );
    assert_eq!(
        paths.canon, "crate::async_block",
        "async block owner canon path should use the parent module path plus the executable label"
    );

    Ok(())
}

#[test]
fn fixture_snippet_metadata_materializes_async_closure_owner() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let outer = function_id_by_name(&db, "call_async_closure_literal_with_body_call")?;
    let async_closure = async_closure_owner_for_parent(&db, outer)?;
    let span = body_owner_span(&db, async_closure)?;

    let nodes = db
        .get_snippet_context_nodes_ordered(vec![async_closure])
        .map_err(|err| DbError::Cozo(err.to_string()))?;
    assert_eq!(
        nodes.len(),
        1,
        "async closure call_body_owner should be snippet-materializable for RAG expansion"
    );
    let (node, paths) = &nodes[0];
    assert_eq!(node.id, async_closure);
    assert_eq!(node.name, "async_closure");
    assert_eq!((node.start_byte, node.end_byte), span);
    assert!(
        node.file_path.ends_with("fixture_call_graph/src/lib.rs"),
        "async closure owner should materialize using the parent source file: {node:#?}"
    );
    assert!(
        paths.file.ends_with("fixture_call_graph/src/lib.rs"),
        "async closure owner path metadata should inherit the parent source file: {paths:#?}"
    );
    assert_eq!(
        paths.canon, "crate::async_closure",
        "async closure owner canon path should use the parent module path plus the executable label"
    );

    Ok(())
}

#[test]
fn fixture_snippet_metadata_materializes_local_item_owners() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    for case in LOCAL_ITEM_CASES {
        let outer = function_id_by_name(&db, case.owner_name)?;
        let local_item = local_item_owner_for_parent_with_label(&db, outer, case.label)?;
        let span = body_owner_span(&db, local_item)?;

        let nodes = db
            .get_snippet_context_nodes_ordered(vec![local_item])
            .map_err(|err| DbError::Cozo(err.to_string()))?;
        assert_eq!(
            nodes.len(),
            1,
            "{} call_body_owner should be snippet-materializable for RAG expansion",
            case.label
        );
        let (node, paths) = &nodes[0];
        assert_eq!(node.id, local_item);
        assert_eq!(node.name, case.label);
        assert_eq!((node.start_byte, node.end_byte), span);
        assert!(
            node.file_path.ends_with("fixture_call_graph/src/lib.rs"),
            "{} owner should materialize using the parent source file: {node:#?}",
            case.label
        );
        assert!(
            paths.file.ends_with("fixture_call_graph/src/lib.rs"),
            "{} owner path metadata should inherit the parent source file: {paths:#?}",
            case.label
        );
        assert_eq!(
            paths.canon,
            format!("crate::{}", case.label),
            "{} owner canon path should use the parent module path plus the executable label",
            case.label
        );
    }

    Ok(())
}

fn body_owner_span(db: &Database, owner: Uuid) -> Result<(usize, usize), DbError> {
    let rows = db.raw_query(&format!(
        r#"?[span] :=
            *call_body_owner {{ id: to_uuid("{owner}"), span @ 'NOW' }}"#
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one span row for closure owner {owner}: {:#?}",
        rows.rows
    );
    span_pair(&rows.rows[0][0], "call_body_owner.span")
}

fn span_pair(value: &DataValue, label: &str) -> Result<(usize, usize), DbError> {
    let DataValue::List(items) = value else {
        return Err(DbError::QueryExecution(format!(
            "{label} should be a two-item span list, got {value:?}"
        )));
    };
    let [start, end] = items.as_slice() else {
        return Err(DbError::QueryExecution(format!(
            "{label} should have exactly two entries, got {value:?}"
        )));
    };
    let start = start.get_int().ok_or_else(|| {
        DbError::QueryExecution(format!("{label} start should be an integer, got {start:?}"))
    })? as usize;
    let end = end.get_int().ok_or_else(|| {
        DbError::QueryExecution(format!("{label} end should be an integer, got {end:?}"))
    })? as usize;
    Ok((start, end))
}
