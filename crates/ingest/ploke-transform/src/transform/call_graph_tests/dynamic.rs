use super::*;

// CALL_GRAPH_GATE:db-projection - dynamic function edges must not be flattened to path functions.
#[cfg(feature = "call_graph")]
#[test]
fn test_call_graph_projection_for_dynamic_function_call() -> Result<(), Box<dyn std::error::Error>>
{
    let db = Db::new(MemStorage::default()).expect("Failed to create database");
    db.initialize().expect("Failed to initialize database");
    create_schema_all(&db)?;

    let successful_graphs = test_run_phases_and_collect("fixture_call_graph");
    let mut merged = ParsedCodeGraph::merge_new(successful_graphs).expect("Failed to merge graph");
    let tree = merged.build_tree_and_prune().unwrap_or_else(|e| {
        tracing::error!(target: "transform_function", "Error building tree: {}", e);
        panic!()
    });

    let call_report = resolve_call_relations_after_tree(&merged, &tree)?;
    let dynamic_relation_targets = |site_id| {
        call_report
            .relations
            .iter()
            .copied()
            .filter_map(|relation| match relation {
                CallRelation::DynamicFunction { source, target } if source == site_id => {
                    Some(target.into())
                }
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let (call_site_id, target_function_id) = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::DynamicFunction { source, target } => {
                let source_any = AnyCallSiteId::Dynamic(source);
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == source_any)?;
                match call {
                    CallNode::DynamicCall(dynamic_call)
                        if matches!(
                            &dynamic_call.callee,
                            DynamicCallCallee::Path { path }
                                if path.as_slice() == ["local_target"]
                        ) =>
                    {
                        Some((source, target))
                    }
                    _ => None,
                }
            }
            CallRelation::Function { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect("fixture_call_graph should resolve (local_target)() as DynamicFunction");
    let call_site_db_id = call_site_id.to_cozo_uuid();
    let target_db_id: DataValue = target_function_id.into();
    let (cast_site_id, cast_target_id) = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::DynamicFunction { source, target } => {
                let source_any = AnyCallSiteId::Dynamic(source);
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == source_any)?;
                match call {
                    CallNode::DynamicCall(dynamic_call)
                        if matches!(
                            &dynamic_call.callee,
                            DynamicCallCallee::FnPointerCastPath { path }
                                if path.as_slice() == ["local_target"]
                        ) =>
                    {
                        Some((source, target))
                    }
                    _ => None,
                }
            }
            CallRelation::Function { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect(
            "fixture_call_graph should resolve (local_target as fn() -> i32)() as DynamicFunction",
        );
    let cast_site_db = cast_site_id.to_cozo_uuid();
    let cast_target_db: DataValue = cast_target_id.into();
    let (binding_cast_site_id, binding_cast_target_id) = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::DynamicFunction { source, target } => {
                let source_any = AnyCallSiteId::Dynamic(source);
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == source_any)?;
                match call {
                    CallNode::DynamicCall(dynamic_call)
                        if matches!(
                            &dynamic_call.callee,
                            DynamicCallCallee::FnPointerCastInitializedLocalBinding {
                                path,
                                init_path,
                            } if path.as_slice() == ["f"]
                                && init_path.as_slice() == ["local_target"]
                        ) =>
                    {
                        Some((source, target))
                    }
                    _ => None,
                }
            }
            CallRelation::Function { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect("fixture_call_graph should resolve (f as fn() -> i32)() as DynamicFunction");
    let binding_cast_site_db = binding_cast_site_id.to_cozo_uuid();
    let binding_cast_target_db: DataValue = binding_cast_target_id.into();
    let (deref_site_id, deref_target_id) = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::DynamicFunction { source, target } => {
                let source_any = AnyCallSiteId::Dynamic(source);
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == source_any)?;
                match call {
                    CallNode::DynamicCall(dynamic_call)
                        if matches!(
                            &dynamic_call.callee,
                            DynamicCallCallee::DereferencedInitializedLocalBinding {
                                path,
                                init_path,
                            } if path.as_slice() == ["f"]
                                && init_path.as_slice() == ["local_target"]
                        ) =>
                    {
                        Some((source, target))
                    }
                    _ => None,
                }
            }
            CallRelation::Function { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect("fixture_call_graph should resolve (*f)() as DynamicFunction");
    let deref_site_db = deref_site_id.to_cozo_uuid();
    let deref_target_db: DataValue = deref_target_id.into();
    let (block_site_id, block_target_id) = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::DynamicFunction { source, target } => {
                let source_any = AnyCallSiteId::Dynamic(source);
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == source_any)?;
                match call {
                    CallNode::DynamicCall(dynamic_call)
                        if dynamic_call.span == (13065, 13085)
                            && matches!(
                                &dynamic_call.callee,
                                DynamicCallCallee::Path { path }
                                    if path.as_slice() == ["local_target"]
                            ) =>
                    {
                        Some((source, target))
                    }
                    _ => None,
                }
            }
            CallRelation::Function { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect("fixture_call_graph should resolve ({ local_target })() as DynamicFunction");
    let block_site_db = block_site_id.to_cozo_uuid();
    let block_target_db: DataValue = block_target_id.into();
    let (branch_site_id, branch_target_id) = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::DynamicFunction { source, target } => {
                let source_any = AnyCallSiteId::Dynamic(source);
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == source_any)?;
                match call {
                    CallNode::DynamicCall(dynamic_call)
                        if dynamic_call.span == (13188, 13238)
                            && matches!(
                                &dynamic_call.callee,
                                DynamicCallCallee::IfBranchPaths { paths }
                                    if paths.as_slice()
                                        == [vec!["local_target".to_string()], vec!["local_target".to_string()]]
                            ) =>
                    {
                        Some((source, target))
                    }
                    _ => None,
                }
            }
            CallRelation::Function { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect(
            "fixture_call_graph should resolve if same-branch dynamic call as DynamicFunction",
        );
    let branch_site_db = branch_site_id.to_cozo_uuid();
    let branch_target_db: DataValue = branch_target_id.into();
    let branch_ambiguous_site_id = merged
        .call_sites()
        .iter()
        .find_map(|call| match call {
            CallNode::DynamicCall(dynamic_call)
                if dynamic_call.span == (13306, 13356)
                    && matches!(
                        &dynamic_call.callee,
                        DynamicCallCallee::IfBranchPaths { paths }
                            if paths.as_slice()
                                == [vec!["local_target".to_string()], vec!["other_target".to_string()]]
                    ) =>
            {
                Some(dynamic_call.id)
            }
            _ => None,
        })
        .expect("fixture_call_graph should record ambiguous if-branch dynamic call site");
    let branch_ambiguous_targets = dynamic_relation_targets(branch_ambiguous_site_id);
    let branch_ambiguous_site_db = branch_ambiguous_site_id.to_cozo_uuid();
    let (match_site_id, match_target_id) = call_report
        .relations
        .iter()
        .copied()
        .find_map(|relation| match relation {
            CallRelation::DynamicFunction { source, target } => {
                let source_any = AnyCallSiteId::Dynamic(source);
                let call = merged
                    .call_sites()
                    .iter()
                    .find(|call| call.id() == source_any)?;
                match call {
                    CallNode::DynamicCall(dynamic_call)
                        if dynamic_call.span == (13422, 13505)
                            && matches!(
                                &dynamic_call.callee,
                                DynamicCallCallee::MatchArmPaths { paths }
                                    if paths.as_slice()
                                        == [vec!["local_target".to_string()], vec!["local_target".to_string()]]
                            ) =>
                    {
                        Some((source, target))
                    }
                    _ => None,
                }
            }
            CallRelation::Function { .. }
            | CallRelation::Method { .. }
            | CallRelation::AssociatedFunction { .. }
            | CallRelation::TupleStructConstructor { .. }
            | CallRelation::EnumVariantConstructor { .. } => None,
        })
        .expect(
            "fixture_call_graph should resolve match same-arm dynamic call as DynamicFunction",
        );
    let match_site_db = match_site_id.to_cozo_uuid();
    let match_target_db: DataValue = match_target_id.into();
    let match_ambiguous_site_id = merged
        .call_sites()
        .iter()
        .find_map(|call| match call {
            CallNode::DynamicCall(dynamic_call)
                if dynamic_call.span == (13576, 13659)
                    && matches!(
                        &dynamic_call.callee,
                        DynamicCallCallee::MatchArmPaths { paths }
                            if paths.as_slice()
                                == [vec!["local_target".to_string()], vec!["other_target".to_string()]]
                    ) =>
            {
                Some(dynamic_call.id)
            }
            _ => None,
        })
        .expect("fixture_call_graph should record ambiguous match-arm dynamic call site");
    let match_ambiguous_targets = dynamic_relation_targets(match_ambiguous_site_id);
    let match_ambiguous_site_db = match_ambiguous_site_id.to_cozo_uuid();

    transform_parsed_graph(&db, merged, &tree)?;

    let resolved_cases = [
        ("dynamic function", call_site_db_id, target_db_id, None),
        (
            "function-pointer cast",
            cast_site_db,
            cast_target_db,
            Some(&["local_target"][..]),
        ),
        (
            "initialized function-pointer cast",
            binding_cast_site_db,
            binding_cast_target_db,
            Some(&["f"][..]),
        ),
        (
            "dereferenced function-pointer",
            deref_site_db,
            deref_target_db,
            Some(&["f"][..]),
        ),
        (
            "block-path",
            block_site_db,
            block_target_db,
            Some(&["local_target"][..]),
        ),
        (
            "if-branch",
            branch_site_db,
            branch_target_db,
            Some(&["local_target"][..]),
        ),
        (
            "match-arm",
            match_site_db,
            match_target_db,
            Some(&["local_target"][..]),
        ),
    ];
    for (label, site_id, target_id, path) in resolved_cases {
        assert_dynamic_relation(&db, site_id.clone(), target_id, label)?;
        if let Some(path) = path {
            assert_dynamic_site_path(&db, site_id.clone(), Some(path))?;
        }
        assert_dynamic_status(&db, site_id, "Resolved", Some("LocalExact"))?;
    }

    let ambiguous_cases = [
        (
            "ambiguous if-branch",
            branch_ambiguous_site_db,
            branch_ambiguous_targets,
        ),
        (
            "ambiguous match-arm",
            match_ambiguous_site_db,
            match_ambiguous_targets,
        ),
    ];
    for (label, site_id, expected_targets) in ambiguous_cases {
        assert_dynamic_site_path(&db, site_id.clone(), None)?;
        assert_dynamic_status(&db, site_id.clone(), "Ambiguous", None)?;
        assert_dynamic_candidate_relations(&db, site_id, expected_targets, label)?;
    }

    Ok(())
}

fn assert_dynamic_relation(
    db: &Db<MemStorage>,
    site_id: DataValue,
    target_id: DataValue,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), site_id);
    params.insert("target_id".to_string(), target_id);
    let rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        rows.rows.len(),
        1,
        "expected one persisted {label} dynamic call_relation row"
    );
    assert_dynamic_relation_family(&rows.rows[0]);
    Ok(())
}

fn assert_dynamic_candidate_relations(
    db: &Db<MemStorage>,
    site_id: DataValue,
    mut expected_targets: Vec<DataValue>,
    label: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), site_id);
    let rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    let mut actual_targets = rows
        .rows
        .iter()
        .map(|row| row[1].clone())
        .collect::<Vec<_>>();
    actual_targets.sort();
    expected_targets.sort();
    assert_eq!(
        actual_targets, expected_targets,
        "{label} dynamic call should persist proven candidates"
    );
    assert_eq!(
        rows.rows.len(),
        2,
        "expected two persisted {label} dynamic candidate rows"
    );
    for row in &rows.rows {
        assert_dynamic_relation_family(row);
    }
    Ok(())
}

fn assert_dynamic_relation_family(row: &[DataValue]) {
    assert_eq!(&row[2], &DataValue::from("DynamicFunction"));
    assert_eq!(&row[3], &DataValue::from("Dynamic"));
    assert_eq!(&row[4], &DataValue::from("Function"));
}

fn assert_dynamic_site_path(
    db: &Db<MemStorage>,
    site_id: DataValue,
    expected_path: Option<&[&str]>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), site_id);
    let rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(rows.rows.len(), 1);
    assert_eq!(&rows.rows[0][1], &DataValue::from("Dynamic"));
    let expected_path = expected_path
        .map(|path| {
            DataValue::List(
                path.iter()
                    .map(|segment| DataValue::from(*segment))
                    .collect(),
            )
        })
        .unwrap_or(DataValue::Null);
    assert_eq!(&rows.rows[0][2], &expected_path);
    Ok(())
}

fn assert_dynamic_status(
    db: &Db<MemStorage>,
    site_id: DataValue,
    expected_status: &str,
    expected_resolution: Option<&str>,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), site_id);
    let rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(rows.rows.len(), 1);
    assert_eq!(&rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(&rows.rows[0][2], &DataValue::from(expected_status));
    let expected_resolution = expected_resolution
        .map(DataValue::from)
        .unwrap_or(DataValue::Null);
    assert_eq!(&rows.rows[0][3], &expected_resolution);
    Ok(())
}
