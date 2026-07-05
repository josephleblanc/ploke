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
