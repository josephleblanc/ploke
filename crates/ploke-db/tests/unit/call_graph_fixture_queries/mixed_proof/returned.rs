use super::*;

#[test]
fn fixture_projection_stores_real_returned_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_function")?;
    let maker = function_id_by_name(&db, "make_fn")?;
    let returned = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned function proof context rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["make_fn"]);
    let path_site = path_row.site.id;
    let path_span = path_row.site.span;
    assert_resolved_target(
        path_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_rows = context
        .iter()
        .filter(|row| row.site.kind == CallSiteKind::Dynamic)
        .collect::<Vec<_>>();
    assert_eq!(
        dynamic_rows.len(),
        1,
        "expected one outer returned-function dynamic row: {context:#?}"
    );
    let dynamic_row = dynamic_rows[0];
    let dynamic_site = dynamic_row.site.id;
    let dynamic_span = dynamic_row.site.span;
    assert_eq!(dynamic_row.site.path.as_ref(), Some(&path(&["make_fn"])));
    assert_resolved_target(
        dynamic_row,
        returned,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 6);

    assert_owner_proof_edges(
        &db,
        "returned-function resolved calls",
        &[
            OwnerProofEdge {
                owner,
                site: path_site,
                span: path_span,
                target: maker,
            },
            OwnerProofEdge {
                owner,
                site: dynamic_site,
                span: dynamic_span,
                target: returned,
            },
        ],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_returned_parameter_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(
        &db,
        "call_returned_forwarded_function_pointer_param_with_local_target",
    )?;
    let helper = function_id_by_name(&db, "return_forwarded_function_pointer")?;
    let returned = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned parameter function proof context rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["return_forwarded_function_pointer"]);
    let dynamic_row = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["return_forwarded_function_pointer"],
    );
    assert_resolved_target(
        path_row,
        helper,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    assert_resolved_target(
        dynamic_row,
        returned,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 6);

    assert_owner_proof_edges(
        &db,
        "returned parameter function resolved calls",
        &[
            OwnerProofEdge {
                owner,
                site: path_row.site.id,
                span: path_row.site.span,
                target: helper,
            },
            OwnerProofEdge {
                owner,
                site: dynamic_row.site.id,
                span: dynamic_row.site.span,
                target: returned,
            },
        ],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_returned_closure_dynamic_edge() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1431:
        // The outer dynamic call invokes the closure returned directly by
        // `make_closure` at lines 1426-1428.
        ReturnedClosureProofCase {
            owner: "call_returned_closure",
            maker: "make_closure",
            path: &["make_closure"],
        },
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1453:
        // The outer dynamic call invokes the closure binding returned by
        // `make_bound_closure` at lines 1447-1449.
        ReturnedClosureProofCase {
            owner: "call_returned_bound_closure",
            maker: "make_bound_closure",
            path: &["make_bound_closure"],
        },
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1500:
        // The outer dynamic call invokes the returned local alias of the
        // closure binding in `make_alias_bound_closure` at lines 1493-1496.
        ReturnedClosureProofCase {
            owner: "call_returned_alias_bound_closure",
            maker: "make_alias_bound_closure",
            path: &["make_alias_bound_closure"],
        },
        // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1520:
        // The outer dynamic call invokes the sync closure returned by
        // `make_target_closure` through the forwarding producer
        // `make_forwarded_returned_closure`.
        ReturnedClosureProofCase {
            owner: "call_forwarded_returned_closure",
            maker: "make_forwarded_returned_closure",
            path: &["make_forwarded_returned_closure"],
        },
    ];

    let mut expected = Vec::new();
    for case in cases {
        expected.extend(project_returned_closure_proof(&db, case)?);
    }

    assert_owner_proof_edges(
        &db,
        "returned-closure resolved calls",
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_resolves_forwarded_returned_closure_value_flow() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1511-1520:
    // `make_forwarded_returned_closure()` returns the closure produced by
    // `make_target_closure()`, while `call_forwarded_returned_closure()`
    // immediately invokes the producer result. This is the bounded sync
    // returned-callable forwarding case: the caller reaches the returned
    // closure owner and can then traverse the closure body to `local_target`.
    let owner = function_id_by_name(&db, "call_forwarded_returned_closure")?;
    let producer = function_id_by_name(&db, "make_forwarded_returned_closure")?;
    let maker = function_id_by_name(&db, "make_target_closure")?;
    let local_target = function_id_by_name(&db, "local_target")?;

    let owner_context = db.call_context_for_owner(owner)?;
    assert_eq!(
        owner_context.len(),
        2,
        "forwarded returned closure caller should expose the producer path and outer dynamic call: {owner_context:#?}"
    );
    let producer_row = row_by_path(&owner_context, &["make_forwarded_returned_closure"]);
    assert_resolved_target(
        producer_row,
        producer,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic = row_by_kind_path(
        &owner_context,
        CallSiteKind::Dynamic,
        &["make_forwarded_returned_closure"],
    );
    assert_resolved_target(
        dynamic,
        dynamic.targets[0].target_id,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    let closure = dynamic.targets[0].target_id;
    assert!(
        !relations_for_site(&db, dynamic.site.id)?.rows.is_empty(),
        "forwarded returned closure flow should persist a dynamic closure edge"
    );

    let producer_context = db.call_context_for_owner(producer)?;
    assert_eq!(
        producer_context.len(),
        1,
        "forwarded returned closure producer should only expose the maker path: {producer_context:#?}"
    );
    let maker_row = row_by_path(&producer_context, &["make_target_closure"]);
    assert_resolved_target(
        maker_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let producer_paths = db.call_paths_between(
        owner,
        producer,
        ploke_db::CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert_eq!(
        producer_paths.len(),
        1,
        "caller should traverse exactly one resolved edge to the closure producer: {producer_paths:#?}"
    );
    assert_eq!(producer_paths[0].depth, 1);
    assert_eq!(producer_paths[0].edges[0].caller_id, owner);
    assert_eq!(producer_paths[0].edges[0].callee_id, producer);

    let closure_paths = db.call_paths_between(
        owner,
        closure,
        ploke_db::CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert_eq!(
        closure_paths.len(),
        1,
        "caller should traverse exactly one resolved edge to the returned closure owner: {closure_paths:#?}"
    );
    assert_eq!(closure_paths[0].depth, 1);
    assert_eq!(closure_paths[0].edges[0].caller_id, owner);
    assert_eq!(closure_paths[0].edges[0].callee_id, closure);
    assert_eq!(
        closure_paths[0].edges[0].relation,
        CallRelationKind::DynamicClosure
    );

    let local_target_paths = db.call_paths_between(
        owner,
        local_target,
        ploke_db::CallPathOptions {
            max_depth: 4,
            max_paths: 16,
        },
    )?;
    assert!(
        local_target_paths.iter().any(|path| {
            path.depth == 2
                && path.edges[0].caller_id == owner
                && path.edges[0].callee_id == closure
                && path.edges[0].relation == CallRelationKind::DynamicClosure
                && path.edges[1].caller_id == closure
                && path.edges[1].callee_id == local_target
                && path.edges[1].relation == CallRelationKind::Function
        }),
        "forwarded returned closure flow should traverse caller -> closure -> local_target: {local_target_paths:#?}"
    );

    Ok(())
}

#[test]
fn fixture_projection_stores_awaited_returned_async_closure_edge_only_when_polled()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let maker = function_id_by_name(&db, "make_returned_async_closure")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2355-2365:
    // `make_returned_async_closure()()` is targetless until the returned async
    // closure future is immediately polled by `.await`, either directly or
    // through a same-block local future binding.
    let no_await_owner = function_id_by_name(&db, "call_returned_async_closure_without_await")?;
    let no_await_context = db.call_context_for_owner(no_await_owner)?;
    assert_eq!(
        no_await_context.len(),
        2,
        "un-awaited returned async closure proof context rows: {no_await_context:#?}"
    );
    let no_await_path = row_by_path(&no_await_context, &["make_returned_async_closure"]);
    assert_resolved_target(
        no_await_path,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    let no_await_dynamic = row_by_kind_path(
        &no_await_context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(no_await_dynamic.status.status, CallStatusKind::Unsupported);
    assert_eq!(no_await_dynamic.status.resolution, None);
    assert!(
        no_await_dynamic.targets.is_empty(),
        "un-awaited returned async closure proof row must stay targetless: {no_await_dynamic:#?}"
    );
    let no_await_count =
        db.project_call_proof_facts_for_owner(no_await_owner, "bd:fixture-call-graph")?;
    assert_eq!(no_await_count, 5);

    let mut expected = vec![OwnerProofEdge {
        owner: no_await_owner,
        site: no_await_path.site.id,
        span: no_await_path.site.span,
        target: maker,
    }];
    for (owner, label) in [
        (
            "call_awaited_returned_async_closure",
            "awaited returned async closure",
        ),
        (
            "call_stored_returned_async_closure",
            "stored returned async closure future",
        ),
    ] {
        expected.extend(project_polled_returned_async_proof(
            &db, owner, label, maker,
        )?);
    }

    assert_owner_proof_edges(
        &db,
        "returned async closure resolved calls",
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_keeps_forwarded_returned_async_future_fail_closed() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2368-2373:
    // `make_forwarded_returned_async_future()` returns the future produced by
    // `make_returned_async_closure()()`, while
    // `call_forwarded_returned_async_future()` awaits only the producer
    // function result. Current call-graph proof does not carry future value
    // flow across that function boundary, so only the producer call traverses.
    let owner = function_id_by_name(&db, "call_forwarded_returned_async_future")?;
    let producer = function_id_by_name(&db, "make_forwarded_returned_async_future")?;
    let returned_maker = function_id_by_name(&db, "make_returned_async_closure")?;
    let local_target = function_id_by_name(&db, "local_target")?;

    let owner_context = db.call_context_for_owner(owner)?;
    assert_eq!(
        owner_context.len(),
        1,
        "forwarded returned async future caller should only expose the awaited producer call: {owner_context:#?}"
    );
    let producer_row = row_by_path(&owner_context, &["make_forwarded_returned_async_future"]);
    assert_resolved_target(
        producer_row,
        producer,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let producer_context = db.call_context_for_owner(producer)?;
    assert_eq!(
        producer_context.len(),
        2,
        "producer should expose the returned async closure maker path plus targetless outer call: {producer_context:#?}"
    );
    let returned_maker_row = row_by_path(&producer_context, &["make_returned_async_closure"]);
    assert_resolved_target(
        returned_maker_row,
        returned_maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    let dynamic = row_by_kind_path(
        &producer_context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(dynamic.status.status, CallStatusKind::Unsupported);
    assert_eq!(dynamic.status.resolution, None);
    assert!(
        dynamic.targets.is_empty(),
        "non-local returned async future flow must not fabricate a closure edge: {dynamic:#?}"
    );
    assert!(
        relations_for_site(&db, dynamic.site.id)?.rows.is_empty(),
        "non-local returned async future flow must not persist a call edge"
    );

    let producer_paths = db.call_paths_between(
        owner,
        producer,
        ploke_db::CallPathOptions {
            max_depth: 1,
            max_paths: 8,
        },
    )?;
    assert_eq!(
        producer_paths.len(),
        1,
        "caller should traverse exactly one resolved edge to the future producer: {producer_paths:#?}"
    );
    assert_eq!(producer_paths[0].depth, 1);
    assert_eq!(producer_paths[0].edges[0].caller_id, owner);
    assert_eq!(producer_paths[0].edges[0].callee_id, producer);

    let local_target_paths = db.call_paths_between(
        owner,
        local_target,
        ploke_db::CallPathOptions {
            max_depth: 4,
            max_paths: 16,
        },
    )?;
    assert!(
        local_target_paths.is_empty(),
        "non-local returned async future flow should remain fail-closed until a typed future-flow carrier exists: {local_target_paths:#?}"
    );

    Ok(())
}

fn project_polled_returned_async_proof(
    db: &ploke_db::Database,
    owner_name: &str,
    label: &str,
    maker: Uuid,
) -> Result<Vec<OwnerProofEdge>, DbError> {
    let owner = function_id_by_name(db, owner_name)?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "{label} proof context rows: {context:#?}");

    let path_row = row_by_path(&context, &["make_returned_async_closure"]);
    assert_resolved_target(
        path_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_row = row_by_kind_path(
        &context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(dynamic_row.targets.len(), 1);
    let closure = dynamic_row.targets[0].target_id;
    assert_resolved_target(
        dynamic_row,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 6, "{label} proof fact count");

    Ok(vec![
        OwnerProofEdge {
            owner,
            site: path_row.site.id,
            span: path_row.site.span,
            target: maker,
        },
        OwnerProofEdge {
            owner,
            site: dynamic_row.site.id,
            span: dynamic_row.site.span,
            target: closure,
        },
    ])
}

struct ReturnedClosureProofCase {
    owner: &'static str,
    maker: &'static str,
    path: &'static [&'static str],
}

fn project_returned_closure_proof(
    db: &ploke_db::Database,
    case: ReturnedClosureProofCase,
) -> Result<Vec<OwnerProofEdge>, DbError> {
    let owner = function_id_by_name(db, case.owner)?;
    let maker = function_id_by_name(db, case.maker)?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "{} returned closure proof context rows: {context:#?}",
        case.owner
    );

    let path_row = row_by_path(&context, case.path);
    let path_site = path_row.site.id;
    let path_span = path_row.site.span;
    assert_resolved_target(
        path_row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic_row = row_by_kind_path(&context, CallSiteKind::Dynamic, case.path);
    let dynamic_site = dynamic_row.site.id;
    let dynamic_span = dynamic_row.site.span;
    assert_eq!(
        dynamic_row.targets.len(),
        1,
        "{} returned closure dynamic row: {dynamic_row:#?}",
        case.owner
    );
    let closure = dynamic_row.targets[0].target_id;
    assert_resolved_target(
        dynamic_row,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 6);

    Ok(vec![
        OwnerProofEdge {
            owner,
            site: path_site,
            span: path_span,
            target: maker,
        },
        OwnerProofEdge {
            owner,
            site: dynamic_site,
            span: dynamic_span,
            target: closure,
        },
    ])
}
