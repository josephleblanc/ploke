use super::*;

#[derive(Clone, Copy)]
struct NodeContextCase {
    label: &'static str,
    node: Uuid,
    min_outgoing: usize,
    min_incoming: usize,
}

impl NodeContextCase {
    fn new(label: &'static str, node: Uuid, min_outgoing: usize, min_incoming: usize) -> Self {
        Self {
            label,
            node,
            min_outgoing,
            min_incoming,
        }
    }
}

#[test]
fn fixture_call_context_for_node_combines_owner_and_target_context() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let path_owner = function_id_by_name(&db, "call_crate_local_target")?;
    let dynamic_owner = function_id_by_name(&db, "call_parenthesized_local_target")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    for case in [
        NodeContextCase::new("path-call owner", path_owner, 1, 0),
        NodeContextCase::new("function target", local_target, 0, 2),
        NodeContextCase::new("method target", method_target, 0, 4),
    ] {
        assert_node_context_matches_helpers(&db, case)?;
    }

    let owner = db.call_context_for_node(path_owner)?;
    let row = row_by_path(&owner.outgoing, &["crate", "local_target"]);
    assert_resolved_target(
        row,
        local_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let target = db.call_context_for_node(local_target)?;
    let path_row = row_by_kind_path(
        &target.incoming,
        CallSiteKind::Path,
        &["crate", "local_target"],
    );
    assert_eq!(path_row.site.owner_id, path_owner);
    assert_resolved_target(
        path_row,
        local_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic = row_by_owner_kind_path(
        &target.incoming,
        dynamic_owner,
        CallSiteKind::Dynamic,
        &["local_target"],
    );
    assert_eq!(dynamic.site.owner_id, dynamic_owner);
    assert_resolved_target(
        dynamic,
        local_target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let method = db.call_context_for_node(method_target)?;
    let typed_owner = function_id_by_name(&db, "call_typed_local_instance_method")?;
    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let typed =
        row_by_owner_method_receiver(&method.incoming, typed_owner, "instance_value", &receiver);
    assert_resolved_target(
        typed,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}

fn assert_node_context_matches_helpers(
    db: &Database,
    case: NodeContextCase,
) -> Result<CallNodeContext, DbError> {
    let context = db.call_context_for_node(case.node)?;
    assert_eq!(context.node_id, case.node, "{} node id", case.label);

    let outgoing = db.call_context_for_owner(case.node)?;
    let incoming = db.call_context_for_target(case.node)?;
    assert_eq!(context.outgoing, outgoing, "{} outgoing rows", case.label);
    assert_eq!(context.incoming, incoming, "{} incoming rows", case.label);

    assert!(
        context.outgoing.len() >= case.min_outgoing,
        "{} should have at least {} outgoing rows: {context:#?}",
        case.label,
        case.min_outgoing
    );
    assert!(
        context.incoming.len() >= case.min_incoming,
        "{} should have at least {} incoming rows: {context:#?}",
        case.label,
        case.min_incoming
    );

    Ok(context)
}
