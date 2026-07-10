use super::*;

#[test]
fn test_call_graph_projection_for_closure_body_owner() -> Result<(), Box<dyn std::error::Error>> {
    assert_executable_body_projection(ExecutableProjectionCase {
        owner_name: "closure_body_call_is_not_outer_call_site",
        kind: ExecutableBodyKind::Closure,
        owner_kind: "Closure",
        label: "closure",
        callee_path: &["local_target"],
        target_name: "local_target",
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
        callee_path: &["local_target"],
        target_name: "local_target",
    })
}

#[test]
fn test_call_graph_projection_for_async_closure_body_owner()
-> Result<(), Box<dyn std::error::Error>> {
    assert_executable_body_projection(ExecutableProjectionCase {
        owner_name: "call_async_closure_literal_with_body_call",
        kind: ExecutableBodyKind::Closure,
        owner_kind: "Closure",
        label: "async_closure",
        callee_path: &["local_target"],
        target_name: "local_target",
    })
}

#[test]
fn test_call_graph_projection_for_local_const_initializer_owner()
-> Result<(), Box<dyn std::error::Error>> {
    assert_executable_body_projection(ExecutableProjectionCase {
        owner_name: "local_const_initializer_call_is_not_outer_call_site",
        kind: ExecutableBodyKind::LocalItem,
        owner_kind: "LocalItem",
        label: "local_const",
        callee_path: &["assoc_const_value"],
        target_name: "assoc_const_value",
    })
}

#[test]
fn test_call_graph_projection_for_local_function_item_target()
-> Result<(), Box<dyn std::error::Error>> {
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
        .find(|function| function.name == "local_fn_body_call_is_not_outer_call_site")
        .map(|function| function.id)
        .expect("fixture_call_graph should define local_fn_body_call_is_not_outer_call_site");
    let body = merged
        .executable_bodies()
        .iter()
        .find(|body| {
            body.parent == CallBodyOwnerId::Function(outer)
                && body.kind == ExecutableBodyKind::LocalItem
                && body.label.as_deref() == Some("local_fn:inner")
        })
        .expect("outer function should own local_fn:inner body")
        .clone();

    let call_report = resolve_call_relations_after_tree(&merged, &tree)?;
    let call_site_id = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::LocalFunction { source, target } if target == body.id => {
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == AnyCallSiteId::Path(source))?;
                let CallNode::PathCall(path_call) = call else {
                    return None;
                };
                (path_call.owner == CallBodyOwnerId::Function(outer)
                    && path_call.path.iter().map(String::as_str).eq(["inner"]))
                .then_some(source)
            }
            _ => None,
        })
        .expect("outer inner() call should resolve to local_fn:inner body");

    transform_parsed_graph(&db, merged, &tree)?;

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), call_site_id.to_cozo_uuid());
    params.insert("target_id".to_string(), body.id.to_cozo_uuid());
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
        "expected one persisted outer inner() LocalFunction call_relation row"
    );
    assert_eq!(&relation_rows.rows[0][2], &DataValue::from("LocalFunction"));
    assert_eq!(&relation_rows.rows[0][3], &DataValue::from("Path"));
    assert_eq!(&relation_rows.rows[0][4], &DataValue::from("LocalItem"));

    Ok(())
}

#[test]
fn test_call_graph_projection_for_async_closure_callee_evidence()
-> Result<(), Box<dyn std::error::Error>> {
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    create_schema_all(&db)?;

    let successful_graphs = test_run_phases_and_collect("fixture_call_graph");
    let mut merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
    let tree = merged.build_tree_and_prune().unwrap_or_else(|e| {
        tracing::error!(target: "transform_function", "Error building tree: {}", e);
        panic!()
    });

    let cases = [
        (
            "call_async_closure_binding_without_await_with_body_call",
            "AsyncClosureBinding",
        ),
        (
            "call_awaited_async_closure_binding_with_body_call",
            "AwaitedAsyncClosureBinding",
        ),
    ];
    let mut expected = Vec::new();

    for (owner_name, kind) in cases {
        let owner = merged
            .functions()
            .iter()
            .find(|function| function.name == owner_name)
            .map(|function| function.id)
            .unwrap_or_else(|| panic!("fixture_call_graph should define {owner_name}"));
        let (site, closure) = merged
            .call_sites()
            .iter()
            .find_map(|call| {
                let CallNode::PathCall(call) = call else {
                    return None;
                };
                if call.owner != CallBodyOwnerId::Function(owner) || call.path != ["closure"] {
                    return None;
                }
                match &call.callee {
                    PathCallCallee::AsyncClosureBinding { path, closure_id } => {
                        assert_eq!(kind, "AsyncClosureBinding");
                        assert_eq!(path, &vec!["closure".to_string()]);
                        Some((call.id, *closure_id))
                    }
                    PathCallCallee::AwaitedAsyncClosureBinding { path, closure_id } => {
                        assert_eq!(kind, "AwaitedAsyncClosureBinding");
                        assert_eq!(path, &vec!["closure".to_string()]);
                        Some((call.id, *closure_id))
                    }
                    _ => None,
                }
            })
            .unwrap_or_else(|| panic!("{owner_name} should expose closure() callee evidence"));
        expected.push((site.to_cozo_uuid(), kind, closure.to_cozo_uuid()));
    }

    transform_parsed_graph(&db, merged, &tree)?;

    for (site, kind, closure) in expected {
        let mut params = BTreeMap::new();
        params.insert("site_id".to_string(), site.clone());
        let rows = db.run_script(
            r#"?[source_id, source_kind, callee_kind, callee_path, closure_id] :=
                source_id = $site_id,
                *call_callee_evidence {
                    source_id,
                    source_kind,
                    callee_kind,
                    callee_path,
                    closure_id @ 'NOW'
                }"#,
            params,
            ScriptMutability::Immutable,
        )?;
        assert_eq!(
            rows.rows.len(),
            1,
            "expected one persisted {kind} call_callee_evidence row"
        );
        assert_eq!(&rows.rows[0][1], &DataValue::from("Path"));
        assert_eq!(&rows.rows[0][2], &DataValue::from(kind));
        assert_eq!(
            &rows.rows[0][3],
            &DataValue::List(vec![DataValue::from("closure")])
        );
        assert_eq!(&rows.rows[0][4], &closure);
    }

    Ok(())
}

struct ExecutableProjectionCase {
    owner_name: &'static str,
    kind: ExecutableBodyKind,
    owner_kind: &'static str,
    label: &'static str,
    callee_path: &'static [&'static str],
    target_name: &'static str,
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
    let expected_target = merged
        .functions()
        .iter()
        .find(|function| function.name == case.target_name)
        .map(|function| function.id)
        .unwrap_or_else(|| panic!("fixture_call_graph should define {}", case.target_name));

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
                    && path_call.path == case.callee_path)
                    .then_some((source, target))
            }
            CallRelation::DynamicFunction { .. }
            | CallRelation::DynamicClosure { .. }
            | CallRelation::Closure { .. }
            | CallRelation::LocalFunction { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .unwrap_or_else(|| {
            panic!(
                "{:?}-owned {:?}() should resolve to the local function target",
                case.kind, case.callee_path
            )
        });
    assert_eq!(
        target_id, expected_target,
        "{:?}-owned {:?}() resolved to the wrong target",
        case.kind, case.callee_path
    );

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
        &DataValue::List(
            case.callee_path
                .iter()
                .copied()
                .map(DataValue::from)
                .collect()
        )
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
