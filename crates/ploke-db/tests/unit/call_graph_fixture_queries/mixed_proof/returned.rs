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
fn fixture_projection_stores_awaited_returned_async_closure_edge_only_when_polled()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let maker = function_id_by_name(&db, "make_returned_async_closure")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:2355-2360:
    // `make_returned_async_closure()()` is targetless until the returned async
    // closure future is immediately polled by `.await`.
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

    let awaited_owner = function_id_by_name(&db, "call_awaited_returned_async_closure")?;
    let awaited_context = db.call_context_for_owner(awaited_owner)?;
    assert_eq!(
        awaited_context.len(),
        2,
        "awaited returned async closure proof context rows: {awaited_context:#?}"
    );
    let awaited_path = row_by_path(&awaited_context, &["make_returned_async_closure"]);
    assert_resolved_target(
        awaited_path,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    let awaited_dynamic = row_by_kind_path(
        &awaited_context,
        CallSiteKind::Dynamic,
        &["make_returned_async_closure"],
    );
    assert_eq!(awaited_dynamic.targets.len(), 1);
    let closure = awaited_dynamic.targets[0].target_id;
    assert_resolved_target(
        awaited_dynamic,
        closure,
        CallRelationKind::DynamicClosure,
        CallSiteKind::Dynamic,
        CallTargetKind::Closure,
    );
    let awaited_count =
        db.project_call_proof_facts_for_owner(awaited_owner, "bd:fixture-call-graph")?;
    assert_eq!(awaited_count, 6);

    assert_owner_proof_edges(
        &db,
        "returned async closure resolved calls",
        &[
            OwnerProofEdge {
                owner: no_await_owner,
                site: no_await_path.site.id,
                span: no_await_path.site.span,
                target: maker,
            },
            OwnerProofEdge {
                owner: awaited_owner,
                site: awaited_path.site.id,
                span: awaited_path.site.span,
                target: maker,
            },
            OwnerProofEdge {
                owner: awaited_owner,
                site: awaited_dynamic.site.id,
                span: awaited_dynamic.site.span,
                target: closure,
            },
        ],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
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
