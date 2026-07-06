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
