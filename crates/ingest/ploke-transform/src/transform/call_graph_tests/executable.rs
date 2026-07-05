use super::*;

#[test]
fn test_call_graph_projection_for_closure_body_owner() -> Result<(), Box<dyn std::error::Error>> {
    assert_executable_body_projection(ExecutableProjectionCase {
        owner_name: "closure_body_call_is_not_outer_call_site",
        kind: ExecutableBodyKind::Closure,
        owner_kind: "Closure",
        label: "closure",
    })
}

#[test]
fn test_call_graph_projection_for_async_block_body_owner() -> Result<(), Box<dyn std::error::Error>>
{
    assert_executable_body_projection(ExecutableProjectionCase {
        owner_name: "async_block_call_is_not_outer_call_site",
        kind: ExecutableBodyKind::AsyncBlock,
        owner_kind: "AsyncBlock",
        label: "async_block",
    })
}

struct ExecutableProjectionCase {
    owner_name: &'static str,
    kind: ExecutableBodyKind,
    owner_kind: &'static str,
    label: &'static str,
}

fn assert_executable_body_projection(
    case: ExecutableProjectionCase,
) -> Result<(), Box<dyn std::error::Error>> {
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    create_schema_all(&db)?;

    let successful_graphs = test_run_phases_and_collect("fixture_call_graph");
    let mut merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
    let tree = merged.build_tree_and_prune().unwrap_or_else(|e| {
        tracing::error!(target: "transform_function", "Error building tree: {}", e);
        panic!()
    });

    let outer = merged
        .functions()
        .iter()
        .find(|function| function.name == case.owner_name)
        .map(|function| function.id)
        .unwrap_or_else(|| panic!("fixture_call_graph should define {}", case.owner_name));
    let body = merged
        .executable_bodies()
        .iter()
        .find(|body| body.parent == CallBodyOwnerId::Function(outer) && body.kind == case.kind)
        .unwrap_or_else(|| {
            panic!(
                "outer function {} should own one {:?} body",
                case.owner_name, case.kind
            )
        })
        .clone();

    let call_report = resolve_call_relations_after_tree(&merged, &tree)?;
    let (call_site_id, target_id) = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::Function { source, target } => {
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == AnyCallSiteId::Path(source))?;
                let CallNode::PathCall(path_call) = call else {
                    return None;
                };
                (path_call.owner == CallBodyOwnerId::Executable(body.id)
                    && path_call.path == ["local_target"])
                .then_some((source, target))
            }
            CallRelation::DynamicFunction { .. }
            | CallRelation::DynamicClosure { .. }
            | CallRelation::Closure { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "{:?}-owned local_target() should resolve to the local function target",
                case.kind
            )
        });

    let body_id = body.id.to_cozo_uuid();
    let parent_id: DataValue = outer.into();
    let call_site_db_id = call_site_id.to_cozo_uuid();
    let target_db_id: DataValue = target_id.into();

    transform_parsed_graph(&db, merged, &tree)?;

    let mut params = BTreeMap::new();
    params.insert("body_id".to_string(), body_id.clone());
    params.insert("parent_id".to_string(), parent_id);
    let owner_rows = db.run_script(
        r#"?[id, owner_kind, parent_id, parent_kind, label] :=
            id = $body_id,
            parent_id = $parent_id,
            *call_body_owner { id, owner_kind, parent_id, parent_kind, label @ 'NOW' }"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        owner_rows.rows.len(),
        1,
        "expected one persisted {:?} call_body_owner row",
        case.kind
    );
    assert_eq!(&owner_rows.rows[0][1], &DataValue::from(case.owner_kind));
    assert_eq!(&owner_rows.rows[0][3], &DataValue::from("Function"));
    assert_eq!(&owner_rows.rows[0][4], &DataValue::from(case.label));

    let mut params = BTreeMap::new();
    params.insert("body_id".to_string(), body_id.clone());
    params.insert("call_site_id".to_string(), call_site_db_id.clone());
    let site_rows = db.run_script(
        r#"?[id, owner_id, call_kind, path] :=
            id = $call_site_id,
            owner_id = $body_id,
            *call_site { id, owner_id, call_kind, path @ 'NOW' }"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        site_rows.rows.len(),
        1,
        "expected one {:?}-owned local_target() call_site row",
        case.kind
    );
    assert_eq!(&site_rows.rows[0][2], &DataValue::from("Path"));
    assert_eq!(
        &site_rows.rows[0][3],
        &DataValue::List(vec![DataValue::from("local_target")])
    );

    let mut params = BTreeMap::new();
    params.insert("body_id".to_string(), body_id);
    params.insert("call_site_id".to_string(), call_site_db_id.clone());
    let edge_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $body_id,
            target_id = $call_site_id,
            *call_site_edge { source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW' }"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        edge_rows.rows.len(),
        1,
        "expected one {:?} BodyContainsCall edge",
        case.kind
    );
    assert_eq!(&edge_rows.rows[0][2], &DataValue::from("BodyContainsCall"));
    assert_eq!(&edge_rows.rows[0][3], &DataValue::from(case.owner_kind));
    assert_eq!(&edge_rows.rows[0][4], &DataValue::from("Path"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), call_site_db_id.clone());
    params.insert("target_id".to_string(), target_db_id);
    let relation_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation { source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW' }"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        relation_rows.rows.len(),
        1,
        "expected one {:?}-owned local_target() call_relation row",
        case.kind
    );
    assert_eq!(&relation_rows.rows[0][2], &DataValue::from("Function"));
    assert_eq!(&relation_rows.rows[0][3], &DataValue::from("Path"));
    assert_eq!(&relation_rows.rows[0][4], &DataValue::from("Function"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), call_site_db_id);
    let status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status { source_id, source_kind, status_kind, resolution_kind @ 'NOW' }"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(status_rows.rows.len(), 1);
    assert_eq!(&status_rows.rows[0][1], &DataValue::from("Path"));
    assert_eq!(&status_rows.rows[0][2], &DataValue::from("Resolved"));
    assert_eq!(&status_rows.rows[0][3], &DataValue::from("LocalExact"));

    Ok(())
}
