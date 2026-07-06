use super::super::super::super::super::*;
use super::super::super::helpers::*;

#[tokio::test]
async fn call_paths_exact_preserves_direct_recursive_self_edge() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "recursive_fixture_call"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable exact call path collection"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `recursive_fixture_call(depth - 1)` is a resolved direct self-call.
    // RAG exact paths should expose the DB's one-edge path in every exact path
    // direction without expanding the cycle beyond that real edge.
    let options = ploke_db::CallPathOptions {
        max_depth: 4,
        max_paths: 16,
    };
    let outgoing = rag.exact_call_paths_from_owner(owner, options)?;
    assert_direct_recursive_path(&outgoing, owner, "RAG outgoing recursive path");

    let incoming = rag.exact_call_paths_to_target(owner, options)?;
    assert_eq!(
        incoming, outgoing,
        "RAG target-centered recursive path lookup should preserve the same one-edge path"
    );

    let direct = rag.exact_call_paths_between(owner, owner, options)?;
    assert_eq!(
        direct, outgoing,
        "RAG direct recursive reachability should expose the real self-edge"
    );

    let cycles = rag.exact_call_cycles_from_owner(owner, options)?;
    assert_eq!(
        cycles, outgoing,
        "RAG cycle query should preserve the DB path that starts and ends at the owner"
    );

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_real_fixture_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_try_result_instance_method"),
    )?;
    let try_target = unique_id_by_name(&db, "function", "try_local_assoc")?;
    let method_target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let owner_context = call_context
        .get(&owner)
        .expect("fixture owner should receive outgoing call context");
    assert_eq!(owner_context.len(), 3, "owner context: {owner_context:#?}");

    let ok_call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["Ok".to_string()],
                    }
        })
        .expect("Ok wrapper call should stay visible");
    assert_eq!(ok_call.status, CallStatusKind::Unsupported);
    assert!(ok_call.resolution.is_none());
    assert!(ok_call.targets.is_empty());

    let try_call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["try_local_assoc".to_string()],
                    }
        })
        .expect("try_local_assoc path call should be present");
    assert_eq!(try_call.status, CallStatusKind::Resolved);
    assert_eq!(try_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(try_call.targets.len(), 1);
    assert_eq!(try_call.targets[0].target_id, try_target);
    assert_eq!(try_call.targets[0].relation, CallTargetKind::Function);

    let method_call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "instance_value".to_string(),
                        receiver: Some(CallReceiverInfo::TryPathCallResult {
                            path: vec!["try_local_assoc".to_string()],
                        }),
                    }
        })
        .expect("try-result method call should be present");
    assert_eq!(method_call.status, CallStatusKind::Resolved);
    assert_eq!(method_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(method_call.targets.len(), 1);
    assert_eq!(method_call.targets[0].target_id, method_target);
    assert_eq!(method_call.targets[0].relation, CallTargetKind::Method);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_if_branch_receiver_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_if_expression_receiver_method"),
    )?;
    let method_target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let owner_context = call_context
        .get(&owner)
        .expect("if-branch receiver owner should receive outgoing call context");
    assert_eq!(owner_context.len(), 1, "owner context: {owner_context:#?}");

    let call = &owner_context[0];
    assert_eq!(
        call.callee,
        CallCalleeInfo::Method {
            name: "instance_value".to_string(),
            receiver: Some(CallReceiverInfo::IfBranchPaths {
                paths: vec![
                    vec!["LocalAssoc".to_string()],
                    vec!["LocalAssoc".to_string()],
                ],
            }),
        }
    );
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, method_target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Method);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_tuple_pattern_receiver_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_tuple_pattern_local_instance_method"),
    )?;
    let method_target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let owner_context = call_context
        .get(&owner)
        .expect("tuple-pattern receiver owner should receive outgoing call context");
    assert_eq!(owner_context.len(), 1, "owner context: {owner_context:#?}");

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1550-1553:
    // `let (value, _) = (LocalAssoc, 0); value.instance_value()` should reuse
    // the same initialized local receiver proof as a simple `let value =
    // LocalAssoc;` binding.
    let call = &owner_context[0];
    assert_eq!(
        call.callee,
        CallCalleeInfo::Method {
            name: "instance_value".to_string(),
            receiver: Some(CallReceiverInfo::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: vec!["LocalAssoc".to_string()],
            }),
        }
    );
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, method_target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Method);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_typed_tuple_pattern_receiver_rows() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_typed_tuple_pattern_local_instance_method"),
    )?;
    let pair_target = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "make_local_assoc_pair"),
    )?;
    let method_target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let owner_context = call_context
        .get(&owner)
        .expect("typed tuple-pattern receiver owner should receive outgoing call context");
    assert_eq!(owner_context.len(), 2, "owner context: {owner_context:#?}");

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `let (value, _): (LocalAssoc, i32) = make_local_assoc_pair();
    // value.instance_value()` should preserve the initializer path call and use
    // the explicit tuple annotation as the receiver's local type proof.
    let path_call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["make_local_assoc_pair".to_string()],
                    }
        })
        .expect("typed tuple-pattern initializer path call should be present");
    assert_eq!(path_call.status, CallStatusKind::Resolved);
    assert_eq!(path_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(path_call.targets.len(), 1);
    assert_eq!(path_call.targets[0].target_id, pair_target);
    assert_eq!(path_call.targets[0].relation, CallTargetKind::Function);

    let method_call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "instance_value".to_string(),
                        receiver: Some(CallReceiverInfo::TypedLocalBinding {
                            name: "value".to_string(),
                            type_path: vec!["LocalAssoc".to_string()],
                        }),
                    }
        })
        .expect("typed tuple-pattern receiver method call should be present");
    assert_eq!(method_call.status, CallStatusKind::Resolved);
    assert_eq!(method_call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(method_call.targets.len(), 1);
    assert_eq!(method_call.targets[0].target_id, method_target);
    assert_eq!(method_call.targets[0].relation, CallTargetKind::Method);

    Ok(())
}

#[tokio::test]
async fn call_context_collection_reads_incoming_rows_for_target_seed() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let target = unique_id_by_name(&db, "function", "try_local_assoc")?;
    let caller = one_uuid(
        &db,
        &function_in_module_query(&["crate"], "call_try_result_instance_method"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(target, 1.0)])?;
    let target_context = call_context
        .get(&target)
        .expect("target seed should receive incoming caller context");
    let call = target_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Path
                && call.callee
                    == CallCalleeInfo::Path {
                        path: vec!["try_local_assoc".to_string()],
                    }
                && call
                    .targets
                    .iter()
                    .any(|candidate| candidate.target_id == target)
        })
        .expect(
            "target seed should retain incoming call context from call_try_result_instance_method",
        );
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Function);
    assert_eq!(
        call.owner_id, caller,
        "target-seed incoming call context should expose the caller owner id"
    );

    let caller_context = db.call_context_for_owner(caller)?;
    assert!(
        caller_context.iter().any(|row| row.site.id == call.site_id),
        "incoming target context should point at the real caller site"
    );

    Ok(())
}

fn assert_direct_recursive_path(
    paths: &[ploke_core::rag_types::CallPathInfo],
    owner: Uuid,
    label: &str,
) {
    assert_eq!(
        paths.len(),
        1,
        "{label} should contain exactly one path and stop cycle expansion: {paths:#?}"
    );
    let path = &paths[0];
    assert_eq!(path.start_id, owner);
    assert_eq!(path.end_id, owner);
    assert_eq!(path.depth, 1);
    assert_eq!(path.edges.len(), 1);
    assert_eq!(path.edges[0].caller_id, owner);
    assert_eq!(path.edges[0].callee_id, owner);
    assert_eq!(path.edges[0].relation, CallTargetKind::Function);
    assert!(
        path.nodes.iter().any(|node| node.id == owner
            && node
                .canon_path
                .as_ref()
                .ends_with("::recursive_fixture_call")
            && node
                .file_path
                .as_ref()
                .ends_with("fixture_call_graph/src/lib.rs")),
        "{label} should attach recursive_fixture_call node metadata: {path:#?}"
    );
}

#[tokio::test]
async fn call_context_collection_reads_real_self_field_method_owner() -> Result<(), Error> {
    init_tracing_once();
    let db = Arc::new(Database::new(setup_db_full_multi_embedding(
        "fixture_call_graph",
    )?));
    let owner = one_uuid(
        &db,
        &method_by_impl_self_query("SelfFieldAssocOwner", "call_self_field_instance_method"),
    )?;
    let target = one_uuid(
        &db,
        &method_by_impl_self_query("LocalAssoc", "instance_value"),
    )?;
    let rag = init_test_rag_mock(Arc::clone(&db));
    assert!(
        !rag.call_context_degraded(),
        "fresh fixture call_graph schema should enable call context collection"
    );

    let call_context = rag.collect_call_context(&[(owner, 1.0)])?;
    let owner_context = call_context
        .get(&owner)
        .expect("self-field method owner should receive outgoing call context");
    assert_eq!(
        owner_context.len(),
        1,
        "self-field method owner context: {owner_context:#?}"
    );

    let call = owner_context
        .iter()
        .find(|call| {
            call.kind == CallSiteKind::Method
                && call.callee
                    == CallCalleeInfo::Method {
                        name: "instance_value".to_string(),
                        receiver: Some(CallReceiverInfo::SelfField {
                            path: vec!["value".to_string()],
                        }),
                    }
        })
        .expect("self-field method call should be present");
    assert_eq!(call.status, CallStatusKind::Resolved);
    assert_eq!(call.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(call.targets.len(), 1);
    assert_eq!(call.targets[0].target_id, target);
    assert_eq!(call.targets[0].relation, CallTargetKind::Method);

    Ok(())
}
