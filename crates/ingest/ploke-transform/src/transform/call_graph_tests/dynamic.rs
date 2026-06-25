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

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), call_site_db_id.clone());
    params.insert("target_id".to_string(), target_db_id);
    let relation_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        relation_rows.rows.len(),
        1,
        "expected one persisted dynamic function call_relation row"
    );
    assert_eq!(
        &relation_rows.rows[0][2],
        &DataValue::from("DynamicFunction")
    );
    assert_eq!(&relation_rows.rows[0][3], &DataValue::from("Dynamic"));
    assert_eq!(&relation_rows.rows[0][4], &DataValue::from("Function"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), call_site_db_id);
    let status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(status_rows.rows.len(), 1);
    assert_eq!(&status_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(&status_rows.rows[0][2], &DataValue::from("Resolved"));
    assert_eq!(&status_rows.rows[0][3], &DataValue::from("LocalExact"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), cast_site_db.clone());
    params.insert("target_id".to_string(), cast_target_db);
    let cast_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        cast_rows.rows.len(),
        1,
        "expected one persisted function-pointer cast dynamic call_relation row"
    );
    assert_eq!(&cast_rows.rows[0][2], &DataValue::from("DynamicFunction"));
    assert_eq!(&cast_rows.rows[0][3], &DataValue::from("Dynamic"));
    assert_eq!(&cast_rows.rows[0][4], &DataValue::from("Function"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), cast_site_db.clone());
    let site_rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(site_rows.rows.len(), 1);
    assert_eq!(&site_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(
        &site_rows.rows[0][2],
        &DataValue::List(vec![DataValue::from("local_target")])
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), cast_site_db);
    let cast_status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(cast_status_rows.rows.len(), 1);
    assert_eq!(&cast_status_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(&cast_status_rows.rows[0][2], &DataValue::from("Resolved"));
    assert_eq!(&cast_status_rows.rows[0][3], &DataValue::from("LocalExact"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), binding_cast_site_db.clone());
    params.insert("target_id".to_string(), binding_cast_target_db);
    let binding_cast_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        binding_cast_rows.rows.len(),
        1,
        "expected one persisted initialized function-pointer cast dynamic call_relation row"
    );
    assert_eq!(
        &binding_cast_rows.rows[0][2],
        &DataValue::from("DynamicFunction")
    );
    assert_eq!(&binding_cast_rows.rows[0][3], &DataValue::from("Dynamic"));
    assert_eq!(&binding_cast_rows.rows[0][4], &DataValue::from("Function"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), binding_cast_site_db.clone());
    let binding_site_rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(binding_site_rows.rows.len(), 1);
    assert_eq!(&binding_site_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(
        &binding_site_rows.rows[0][2],
        &DataValue::List(vec![DataValue::from("f")])
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), binding_cast_site_db);
    let binding_cast_status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(binding_cast_status_rows.rows.len(), 1);
    assert_eq!(
        &binding_cast_status_rows.rows[0][1],
        &DataValue::from("Dynamic")
    );
    assert_eq!(
        &binding_cast_status_rows.rows[0][2],
        &DataValue::from("Resolved")
    );
    assert_eq!(
        &binding_cast_status_rows.rows[0][3],
        &DataValue::from("LocalExact")
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), deref_site_db.clone());
    params.insert("target_id".to_string(), deref_target_db);
    let deref_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        deref_rows.rows.len(),
        1,
        "expected one persisted dereferenced function-pointer dynamic call_relation row"
    );
    assert_eq!(&deref_rows.rows[0][2], &DataValue::from("DynamicFunction"));
    assert_eq!(&deref_rows.rows[0][3], &DataValue::from("Dynamic"));
    assert_eq!(&deref_rows.rows[0][4], &DataValue::from("Function"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), deref_site_db.clone());
    let deref_site_rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(deref_site_rows.rows.len(), 1);
    assert_eq!(&deref_site_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(
        &deref_site_rows.rows[0][2],
        &DataValue::List(vec![DataValue::from("f")])
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), deref_site_db);
    let deref_status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(deref_status_rows.rows.len(), 1);
    assert_eq!(&deref_status_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(&deref_status_rows.rows[0][2], &DataValue::from("Resolved"));
    assert_eq!(
        &deref_status_rows.rows[0][3],
        &DataValue::from("LocalExact")
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), block_site_db.clone());
    params.insert("target_id".to_string(), block_target_db);
    let block_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        block_rows.rows.len(),
        1,
        "expected one persisted block-path dynamic call_relation row"
    );
    assert_eq!(&block_rows.rows[0][2], &DataValue::from("DynamicFunction"));
    assert_eq!(&block_rows.rows[0][3], &DataValue::from("Dynamic"));
    assert_eq!(&block_rows.rows[0][4], &DataValue::from("Function"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), block_site_db.clone());
    let block_site_rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(block_site_rows.rows.len(), 1);
    assert_eq!(&block_site_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(
        &block_site_rows.rows[0][2],
        &DataValue::List(vec![DataValue::from("local_target")])
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), block_site_db);
    let block_status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(block_status_rows.rows.len(), 1);
    assert_eq!(&block_status_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(&block_status_rows.rows[0][2], &DataValue::from("Resolved"));
    assert_eq!(
        &block_status_rows.rows[0][3],
        &DataValue::from("LocalExact")
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), branch_site_db.clone());
    params.insert("target_id".to_string(), branch_target_db);
    let branch_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        branch_rows.rows.len(),
        1,
        "expected one persisted if-branch dynamic call_relation row"
    );
    assert_eq!(&branch_rows.rows[0][2], &DataValue::from("DynamicFunction"));
    assert_eq!(&branch_rows.rows[0][3], &DataValue::from("Dynamic"));
    assert_eq!(&branch_rows.rows[0][4], &DataValue::from("Function"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), branch_site_db.clone());
    let branch_site_rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(branch_site_rows.rows.len(), 1);
    assert_eq!(&branch_site_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(
        &branch_site_rows.rows[0][2],
        &DataValue::List(vec![DataValue::from("local_target")])
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), branch_site_db);
    let branch_status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(branch_status_rows.rows.len(), 1);
    assert_eq!(&branch_status_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(&branch_status_rows.rows[0][2], &DataValue::from("Resolved"));
    assert_eq!(
        &branch_status_rows.rows[0][3],
        &DataValue::from("LocalExact")
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), branch_ambiguous_site_db.clone());
    let ambiguous_site_rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(ambiguous_site_rows.rows.len(), 1);
    assert_eq!(&ambiguous_site_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(&ambiguous_site_rows.rows[0][2], &DataValue::Null);

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), branch_ambiguous_site_db.clone());
    let ambiguous_status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(ambiguous_status_rows.rows.len(), 1);
    assert_eq!(
        &ambiguous_status_rows.rows[0][1],
        &DataValue::from("Dynamic")
    );
    assert_eq!(
        &ambiguous_status_rows.rows[0][2],
        &DataValue::from("Ambiguous")
    );
    assert_eq!(&ambiguous_status_rows.rows[0][3], &DataValue::Null);

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), branch_ambiguous_site_db);
    let ambiguous_relation_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    let mut actual_branch_targets = ambiguous_relation_rows
        .rows
        .iter()
        .map(|row| row[1].clone())
        .collect::<Vec<_>>();
    let mut expected_branch_targets = branch_ambiguous_targets;
    actual_branch_targets.sort();
    expected_branch_targets.sort();
    assert_eq!(
        actual_branch_targets, expected_branch_targets,
        "ambiguous if-branch dynamic call should persist proven candidates"
    );
    assert_eq!(ambiguous_relation_rows.rows.len(), 2);
    for row in &ambiguous_relation_rows.rows {
        assert_eq!(&row[2], &DataValue::from("DynamicFunction"));
        assert_eq!(&row[3], &DataValue::from("Dynamic"));
        assert_eq!(&row[4], &DataValue::from("Function"));
    }

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), match_site_db.clone());
    params.insert("target_id".to_string(), match_target_db);
    let match_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            target_id = $target_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(
        match_rows.rows.len(),
        1,
        "expected one persisted match-arm dynamic call_relation row"
    );
    assert_eq!(&match_rows.rows[0][2], &DataValue::from("DynamicFunction"));
    assert_eq!(&match_rows.rows[0][3], &DataValue::from("Dynamic"));
    assert_eq!(&match_rows.rows[0][4], &DataValue::from("Function"));

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), match_site_db.clone());
    let match_site_rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(match_site_rows.rows.len(), 1);
    assert_eq!(&match_site_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(
        &match_site_rows.rows[0][2],
        &DataValue::List(vec![DataValue::from("local_target")])
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), match_site_db);
    let match_status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(match_status_rows.rows.len(), 1);
    assert_eq!(&match_status_rows.rows[0][1], &DataValue::from("Dynamic"));
    assert_eq!(&match_status_rows.rows[0][2], &DataValue::from("Resolved"));
    assert_eq!(
        &match_status_rows.rows[0][3],
        &DataValue::from("LocalExact")
    );

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), match_ambiguous_site_db.clone());
    let match_ambiguous_site_rows = db.run_script(
        r#"?[id, call_kind, path] :=
            id = $call_site_id,
            *call_site{id, call_kind, path @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(match_ambiguous_site_rows.rows.len(), 1);
    assert_eq!(
        &match_ambiguous_site_rows.rows[0][1],
        &DataValue::from("Dynamic")
    );
    assert_eq!(&match_ambiguous_site_rows.rows[0][2], &DataValue::Null);

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), match_ambiguous_site_db.clone());
    let match_ambiguous_status_rows = db.run_script(
        r#"?[source_id, source_kind, status_kind, resolution_kind] :=
            source_id = $call_site_id,
            *call_resolution_status{source_id, source_kind, status_kind, resolution_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    assert_eq!(match_ambiguous_status_rows.rows.len(), 1);
    assert_eq!(
        &match_ambiguous_status_rows.rows[0][1],
        &DataValue::from("Dynamic")
    );
    assert_eq!(
        &match_ambiguous_status_rows.rows[0][2],
        &DataValue::from("Ambiguous")
    );
    assert_eq!(&match_ambiguous_status_rows.rows[0][3], &DataValue::Null);

    let mut params = BTreeMap::new();
    params.insert("call_site_id".to_string(), match_ambiguous_site_db);
    let match_ambiguous_relation_rows = db.run_script(
        r#"?[source_id, target_id, relation_kind, source_kind, target_kind] :=
            source_id = $call_site_id,
            *call_relation{source_id, target_id, relation_kind, source_kind, target_kind @ 'NOW'}"#,
        params,
        ScriptMutability::Immutable,
    )?;
    let mut actual_match_targets = match_ambiguous_relation_rows
        .rows
        .iter()
        .map(|row| row[1].clone())
        .collect::<Vec<_>>();
    let mut expected_match_targets = match_ambiguous_targets;
    actual_match_targets.sort();
    expected_match_targets.sort();
    assert_eq!(
        actual_match_targets, expected_match_targets,
        "ambiguous match-arm dynamic call should persist proven candidates"
    );
    assert_eq!(match_ambiguous_relation_rows.rows.len(), 2);
    for row in &match_ambiguous_relation_rows.rows {
        assert_eq!(&row[2], &DataValue::from("DynamicFunction"));
        assert_eq!(&row[3], &DataValue::from("Dynamic"));
        assert_eq!(&row[4], &DataValue::from("Function"));
    }

    Ok(())
}
