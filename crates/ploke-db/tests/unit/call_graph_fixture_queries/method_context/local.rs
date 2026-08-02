use ploke_db::CallPathOptions;

use super::*;

const METHOD_TUPLE_RETURN_PATTERN_LOCAL_INIT_CALL_SPAN: (usize, usize) = (41242, 41260);
const MATCH_ARM_INITIALIZED_RECEIVER_GUARD_CALL_SPAN: (u32, u32) = (41598, 41620);
const MATCH_ARM_INITIALIZED_RECEIVER_BODY_CALL_SPAN: (u32, u32) = (41628, 41650);
const MATCH_STRUCT_PATTERN_INITIALIZED_RECEIVER_CALL_SPAN: (u32, u32) = (42053, 42075);

#[test]
fn fixture_context_reads_projected_typed_local_method_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_typed_local_instance_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("instance_value"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["LocalAssoc"]),
        })
    );
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].relation, CallRelationKind::Method);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);

    Ok(())
}

#[test]
fn fixture_context_resolves_result_method_callback_function() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_single_result_callback")?;
    let caller = function_id_by_name(&db, "call_single_result_callback_with_local_target")?;
    let target = function_id_by_name(&db, "local_result_target")?;

    // tests/fixture_crates/fixture_call_graph/src/lib.rs EOF:
    // `call_single_result_callback(f)` invokes
    // `Ok::<i32, ()>(1).and_then(f)`. The helper is private and its only local
    // caller passes `local_result_target`, so the method callback edge is exact.
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "result callback context rows: {context:#?}"
    );

    let receiver = CallReceiver::PathCallResult {
        path: path(&["Ok"]),
    };
    let row = row_by_method_receiver(&context, "and_then", &receiver);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::MethodCallbackFunction,
        CallSiteKind::Method,
        CallTargetKind::Function,
    );

    let callers = db.callers_for_target(target)?;
    let caller_row = caller_by_owner_method_receiver(&callers, owner, "and_then", &receiver);
    assert_eq!(caller_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        caller_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(
        caller_row.target.relation,
        CallRelationKind::MethodCallbackFunction
    );
    assert_eq!(caller_row.target.target_kind, CallTargetKind::Function);

    let caller_context = db.call_context_for_owner(caller)?;
    let wrapper_call = row_by_path(&caller_context, &["call_single_result_callback"]);
    assert_resolved_target(
        wrapper_call,
        owner,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let paths = db.call_paths_between(
        caller,
        target,
        CallPathOptions {
            max_depth: 2,
            max_paths: 8,
        },
    )?;
    let path = paths
        .iter()
        .find(|path| path.start_id == caller && path.end_id == target && path.depth == 2)
        .unwrap_or_else(|| {
            panic!("result callback caller should traverse caller -> helper -> local_result_target: {paths:#?}")
        });
    assert_eq!(path.edges[0].caller_id, caller);
    assert_eq!(path.edges[0].callee_id, owner);
    assert_eq!(path.edges[0].relation, CallRelationKind::Function);
    assert_eq!(path.edges[1].caller_id, owner);
    assert_eq!(path.edges[1].callee_id, target);
    assert_eq!(
        path.edges[1].relation,
        CallRelationKind::MethodCallbackFunction
    );
    assert_eq!(path.edges[1].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_local_and_alias_instance_method_receivers() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let cases = [
        (
            "call_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_param_alias_instance_method",
            CallReceiver::AliasedLocalBinding {
                name: "alias".to_string(),
                source_path: path(&["value"]),
            },
        ),
        (
            "call_borrowed_param_alias_instance_method",
            CallReceiver::AliasedLocalBinding {
                name: "alias".to_string(),
                source_path: path(&["value"]),
            },
        ),
        (
            "call_initialized_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_initialized_local_alias_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_tuple_pattern_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_parenthesized_typed_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_type_alias_chain_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssocAliasChain"]),
            },
        ),
        (
            "call_imported_type_alias_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["ImportedLocalAssocAlias"]),
            },
        ),
        (
            "call_if_expression_receiver_method",
            CallReceiver::IfBranchPaths {
                paths: vec![path(&["LocalAssoc"]), path(&["LocalAssoc"])],
            },
        ),
        (
            "call_match_expression_receiver_method",
            CallReceiver::IfBranchPaths {
                paths: vec![path(&["LocalAssoc"]), path(&["LocalAssoc"])],
            },
        ),
        (
            "call_if_initialized_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_match_initialized_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_method_receiver(&context, "instance_value", &receiver);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_typed_tuple_pattern_receiver() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_typed_tuple_pattern_local_instance_method")?;
    let pair_target = function_id_by_name(&db, "make_local_assoc_pair")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "typed tuple-pattern owner should expose the tuple initializer and receiver rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["make_local_assoc_pair"]);
    assert_eq!(path_row.site.owner_id, owner);
    assert_eq!(path_row.site.arg_count, Some(0));
    assert_resolved_target(
        path_row,
        pair_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let method_row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_eq!(method_row.site.owner_id, owner);
    assert_eq!(method_row.site.arg_count, Some(0));
    assert_resolved_target(
        method_row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_tuple_return_pattern_receiver() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_tuple_return_pattern_local_instance_method")?;
    let pair_target = function_id_by_name(&db, "make_local_assoc_pair")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "tuple-return owner should expose the tuple initializer and receiver rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["make_local_assoc_pair"]);
    assert_eq!(path_row.site.owner_id, owner);
    assert_eq!(path_row.site.arg_count, Some(0));
    assert_resolved_target(
        path_row,
        pair_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `let (value, _) = make_local_assoc_pair(); value.instance_value()`
    // should use the local function's tuple return type as receiver proof.
    let receiver = CallReceiver::TupleReturnBinding {
        name: "value".to_string(),
        path: path(&["make_local_assoc_pair"]),
        index: 0,
    };
    let method_row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_eq!(method_row.site.owner_id, owner);
    assert_eq!(method_row.site.arg_count, Some(0));
    assert_resolved_target(
        method_row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_method_tuple_return_pattern_receiver() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(
        &db,
        "call_method_tuple_return_pattern_local_instance_method",
    )?;
    let pair_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "tuple_pair")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "method tuple-return owner should expose the initializer and receiver rows: {context:#?}"
    );

    let init_receiver = CallReceiver::InitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["LocalAssoc"]),
    };
    let init_row = row_by_method_receiver(&context, "tuple_pair", &init_receiver);
    assert_eq!(init_row.site.owner_id, owner);
    assert_eq!(init_row.site.arg_count, Some(0));
    assert_resolved_target(
        init_row,
        pair_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `let (next, _) = value.tuple_pair(); next.instance_value()`
    // should use the initializer method's tuple return type as receiver proof.
    let receiver = CallReceiver::TupleMethodReturn {
        name: "next".to_string(),
        method_name: "tuple_pair".to_string(),
        method_span: METHOD_TUPLE_RETURN_PATTERN_LOCAL_INIT_CALL_SPAN,
        index: 0,
    };
    let method_row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_eq!(method_row.site.owner_id, owner);
    assert_eq!(method_row.site.arg_count, Some(0));
    assert_resolved_target(
        method_row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_match_arm_initialized_receiver() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_match_arm_initialized_receiver_method")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let receiver = CallReceiver::InitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["LocalAssoc"]),
    };

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "match-arm initialized receiver owner should expose guard and body rows: {context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:
    // `value if flag && value.instance_value() > 0 => value.instance_value()`
    // should use the arm pattern binding as receiver proof in both guard and body.
    let mut spans = context
        .iter()
        .filter(|row| {
            row.site.kind == CallSiteKind::Method
                && row.site.method.as_deref() == Some("instance_value")
                && row.site.receiver.as_ref() == Some(&receiver)
        })
        .map(|row| {
            assert_eq!(row.site.owner_id, owner);
            assert_eq!(row.site.arg_count, Some(0));
            assert_resolved_target(
                row,
                target,
                CallRelationKind::Method,
                CallSiteKind::Method,
                CallTargetKind::Method,
            );
            row.site.span
        })
        .collect::<Vec<_>>();
    spans.sort();
    assert_eq!(
        spans,
        vec![
            MATCH_ARM_INITIALIZED_RECEIVER_GUARD_CALL_SPAN,
            MATCH_ARM_INITIALIZED_RECEIVER_BODY_CALL_SPAN
        ],
        "match-arm receiver spans"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_match_struct_pattern_initialized_receiver() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_match_struct_pattern_initialized_receiver_method")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let receiver = CallReceiver::InitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["LocalAssoc"]),
    };

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "match struct-pattern receiver owner should expose one row: {context:#?}"
    );

    // tests/fixture_crates/fixture_call_graph/src/lib.rs:1828-1831:
    // `ParamFieldMethodReceiver { value } => value.instance_value()` should
    // reuse the source-visible struct initializer field as receiver proof.
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(
        row.site.span, MATCH_STRUCT_PATTERN_INITIALIZED_RECEIVER_CALL_SPAN,
        "match struct-pattern receiver span"
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_reference_instance_method_receivers() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let cases = [
        (
            "call_referenced_local_instance_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_typed_reference_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_typed_double_reference_local_instance_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_method_receiver(&context, "instance_value", &receiver);
        assert_eq!(row.site.owner_id, owner);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_explicit_drop_method_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_explicit_drop_method")?;
    let target = method_id_by_impl_self_type_name(&db, "ExplicitDropTarget", "drop")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "explicit drop context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("drop"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::InitializedLocalBinding {
            name: "value".to_string(),
            init_path: path(&["ExplicitDropTarget"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}
