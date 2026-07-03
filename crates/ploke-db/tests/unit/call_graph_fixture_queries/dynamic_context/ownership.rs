use super::*;
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

#[test]
fn fixture_context_does_not_project_local_const_initializer_calls_to_outer_owner()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "local_const_initializer_call_is_not_outer_call_site")?;
    let target = function_id_by_name(&db, "assoc_const_value")?;
    let forbidden_path = path(&["assoc_const_value"]);

    let context = db.call_context_for_owner(owner)?;
    assert!(
        context.iter().all(|row| row.site.kind != CallSiteKind::Path
            || row.site.path.as_ref() != Some(&forbidden_path)),
        "local const initializer assoc_const_value() path row leaked into the outer owner: {context:#?}"
    );
    assert!(
        context
            .iter()
            .flat_map(|row| row.targets.iter())
            .all(|edge| edge.target_id != target),
        "local const initializer edge to assoc_const_value leaked into the outer owner: {context:#?}"
    );

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

    Ok(())
}

fn closure_owner_for_parent(db: &Database, parent: Uuid) -> Result<Uuid, DbError> {
    let rows = db.raw_query(&format!(
        r#"?[id, owner_kind, parent_kind, label] :=
            parent = to_uuid("{parent}"),
            *call_body_owner {{
                id,
                owner_kind,
                parent_id: parent,
                parent_kind,
                label @ 'NOW'
            }}"#
    ))?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected exactly one closure call_body_owner row for parent {parent}: {:#?}",
        rows.rows
    );
    assert_eq!(
        data_str(&rows.rows[0][1], "call_body_owner.owner_kind"),
        "Closure"
    );
    assert_eq!(
        data_str(&rows.rows[0][2], "call_body_owner.parent_kind"),
        "Function"
    );
    assert_eq!(
        data_str(&rows.rows[0][3], "call_body_owner.label"),
        "closure"
    );
    to_uuid(&rows.rows[0][0])
}
