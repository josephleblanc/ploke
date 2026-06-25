use ploke_db::{
    CallContextOptions, CallContextRelation, CallContextSeed, CallReceiver, CallRelationKind,
    CallResolutionKind, CallSiteKind, CallStatusKind, CallTargetKind, Database, DbError,
    ProofGraphStore, to_uuid,
};
use uuid::Uuid;

use super::call_graph_fixture_common::*;

mod associated_context;
mod blocker_proof;
mod constructor_proof;
mod context_expansion;
mod dynamic_context;
mod dynamic_proof;
mod invariants;
mod mixed_proof;
mod owner_context;
mod proof_lookup;
mod resolved_proof;
mod target_proof;
mod trait_method_context;

#[test]
fn fixture_context_reads_projected_path_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(
        row.site.path.as_ref(),
        Some(&path(&["crate", "local_target"]))
    );
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(row.targets[0].relation, CallRelationKind::Function);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_path_resolution_forms() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let nested_target =
        function_id_by_name_in_module(&db, &["crate", "local_mod"], "nested_target")?;
    let imported_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "imported_target")?;
    let globbed_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "globbed_target")?;
    let cases: [(&[&str], &str, Vec<String>, Uuid); 11] = [
        (
            &["crate"],
            "call_unqualified_local_target",
            path(&["local_target"]),
            local_target,
        ),
        (
            &["crate", "local_mod"],
            "call_self_nested_target",
            path(&["self", "nested_target"]),
            nested_target,
        ),
        (
            &["crate"],
            "call_crate_module_nested_target",
            path(&["crate", "local_mod", "nested_target"]),
            nested_target,
        ),
        (
            &["crate"],
            "call_self_module_nested_target",
            path(&["self", "local_mod", "nested_target"]),
            nested_target,
        ),
        (
            &["crate", "super_path_scope"],
            "call_super_local_target",
            path(&["super", "local_target"]),
            local_target,
        ),
        (
            &["crate"],
            "call_imported_alias_target",
            path(&["imported_alias"]),
            imported_target,
        ),
        (
            &["crate"],
            "call_glob_imported_target",
            path(&["globbed_target"]),
            globbed_target,
        ),
        (
            &["crate"],
            "call_reexported_target",
            path(&["reexported_target"]),
            imported_target,
        ),
        (
            &["crate"],
            "call_imported_module_target",
            path(&["targets_alias", "globbed_target"]),
            globbed_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_alias_target",
            path(&["grouped_alias"]),
            imported_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_globbed_target",
            path(&["grouped_globbed_alias"]),
            globbed_target,
        ),
    ];

    for (module_path, owner_name, expected_path, target) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

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
            "call_initialized_local_instance_method",
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
fn fixture_context_reads_projected_tuple_struct_constructor_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_new_type_constructor")?;
    let target = struct_id_by_name(&db, "NewType")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["NewType"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(
        row.targets[0].relation,
        CallRelationKind::TupleStructConstructor
    );
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Struct);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_ambiguous_dynamic_candidates() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let expected = dynamic_candidates(&db)?;

    for owner_name in AMBIGUOUS_DYNAMIC_OWNERS {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        assert_dynamic_candidates(&context[0], owner, &expected, owner_name);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_generic_unsafe_extern_and_chained_calls() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_generic_identity_turbofish")?;
    let target = function_id_by_name(&db, "generic_identity")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "generic function context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["generic_identity"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(1));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner = function_id_by_name(&db, "call_method_turbofish")?;
    let target = method_id_by_impl_self_type_name(&db, "GenericMethodTarget", "generic_instance")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "generic method context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("generic_instance"));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(1));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["GenericMethodTarget"]),
        })
    );
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_unsafe_function")?;
    let target = function_id_by_name(&db, "unsafe_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "unsafe function context rows: {context:#?}"
    );
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["unsafe_target"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner = function_id_by_name(&db, "call_extern_c_function")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "extern C context rows: {context:#?}");
    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["abs"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "extern C call must not fabricate local target edges: {row:#?}"
    );

    let owner = function_id_by_name(&db, "call_chained_returned_function")?;
    let target = function_id_by_name(&db, "make_unary_fn")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "chained call context rows: {context:#?}");

    let row = row_by_path(&context, &["make_unary_fn"]);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
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
        "expected one outer dynamic call row: {context:#?}"
    );
    let row = dynamic_rows[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "returned-function dynamic call must remain unsupported without fake targets: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_raw_identifier_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_raw_identifier_function")?;
    let target = function_id_by_exact_name(&db, "r#match")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "raw function context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["r#match"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner = function_id_by_name(&db, "call_raw_identifier_method")?;
    let target = method_id_by_impl_self_type_exact_name(&db, "RawMethodTarget", "r#type")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "raw method context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("r#type"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["RawMethodTarget"]),
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

#[test]
fn fixture_context_reads_projected_prelude_drop_shadowing() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_prelude_drop_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "prelude drop context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["drop"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "prelude drop must not fabricate local target edges: {row:#?}"
    );

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "prelude_shadow_scope"],
        "call_local_drop_shadow",
    )?;
    let target = function_id_by_name_in_module(&db, &["crate", "prelude_shadow_scope"], "drop")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "local drop shadow context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["drop"])));
    assert_eq!(row.site.arg_count, Some(1));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_external_and_shadowed_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = function_id_by_name(&db, "call_literal_str_to_string")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "literal to_string context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("to_string"));
    assert_eq!(row.site.receiver, Some(CallReceiver::Literal));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "literal to_string must not fabricate local target edges: {row:#?}"
    );

    let owner = function_id_by_name(&db, "call_typed_vec_len_external")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "typed Vec context rows: {context:#?}");

    let new_row = row_by_path(&context, &["Vec", "new"]);
    assert_eq!(new_row.status.status, CallStatusKind::External);
    assert_eq!(new_row.status.resolution, None);
    assert!(
        new_row.targets.is_empty(),
        "Vec::new must not fabricate local target edges: {new_row:#?}"
    );

    let row = row_by_method_receiver(
        &context,
        "len",
        &CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["Vec"]),
        },
    );
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "unshadowed Vec::len must not fabricate local target edges: {row:#?}"
    );

    let owner = function_id_by_name_in_module(
        &db,
        &["crate", "local_prelude_shadow"],
        "call_shadowed_typed_vec_len",
    )?;
    let target = method_id_by_impl_self_type_name(&db, "Vec", "len")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "shadowed Vec::len context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("len"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["Vec"]),
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

#[test]
fn fixture_context_reads_projected_external_path_status_without_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_prelude_string_new")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["String", "new"])));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(row.targets.is_empty(), "external row targets: {row:#?}");

    Ok(())
}

#[test]
fn fixture_context_reads_projected_ambiguous_method_status_without_targets() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_ambiguous_trait_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("overlap"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::LocalBinding {
            name: "value".to_string(),
        })
    );
    assert_eq!(row.status.status, CallStatusKind::Ambiguous);
    assert_eq!(row.status.resolution, None);
    assert!(row.targets.is_empty(), "ambiguous row targets: {row:#?}");

    Ok(())
}

#[test]
fn fixture_context_reads_projected_inherent_method_precedence() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_inherent_over_trait_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "inherent precedence rows: {context:#?}");

    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["InherentPrecedenceTarget"]),
    };
    let row = row_by_method_receiver(&context, "priority", &receiver);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].relation, CallRelationKind::Method);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
    assert!(
        method_owner_is_inherent_impl(&db, row.targets[0].target_id, "InherentPrecedenceTarget")?,
        "inherent method call must project a target owned by the inherent impl: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_unsupported_method_status_without_targets() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_unimported_trait_method")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(row.site.method.as_deref(), Some("scoped_value"));
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::LocalBinding {
            name: "value".to_string(),
        })
    );
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(row.targets.is_empty(), "unsupported row targets: {row:#?}");

    Ok(())
}

#[test]
fn fixture_context_preserves_path_result_owner_order_and_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_path_result_instance_method")?;
    let make_target = function_id_by_name(&db, "make_local_assoc")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "context rows: {context:#?}");

    let path_row = &context[0];
    assert_eq!(path_row.site.kind, CallSiteKind::Path);
    assert_eq!(
        path_row.site.path.as_ref(),
        Some(&path(&["make_local_assoc"]))
    );
    assert_eq!(path_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        path_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(path_row.targets.len(), 1);
    assert_eq!(path_row.targets[0].target_id, make_target);
    assert_eq!(path_row.targets[0].relation, CallRelationKind::Function);

    let method_row = &context[1];
    assert_eq!(method_row.site.kind, CallSiteKind::Method);
    assert_eq!(method_row.site.method.as_deref(), Some("instance_value"));
    assert_eq!(
        method_row.site.receiver,
        Some(CallReceiver::PathCallResult {
            path: path(&["make_local_assoc"]),
        })
    );
    assert_eq!(method_row.status.status, CallStatusKind::Resolved);
    assert_eq!(
        method_row.status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    assert_eq!(method_row.targets.len(), 1);
    assert_eq!(method_row.targets[0].relation, CallRelationKind::Method);

    Ok(())
}

#[test]
fn fixture_low_level_helpers_read_projected_sites_targets_and_statuses() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let sites = db.call_sites_for_owner(owner)?;
    assert_eq!(sites.len(), 3, "call sites: {sites:#?}");
    assert!(
        sites.windows(2).all(|pair| pair[0].span <= pair[1].span),
        "call_sites_for_owner must preserve source-span order: {sites:#?}"
    );

    let ok_path = path(&["Ok"]);
    let ok_site = sites
        .iter()
        .find(|site| site.kind == CallSiteKind::Path && site.path.as_ref() == Some(&ok_path))
        .expect("Ok path call site");
    let ok_status = db
        .call_resolution_for_site(ok_site.id)?
        .expect("Ok status row");
    assert_eq!(ok_status.site_id, ok_site.id);
    assert_eq!(ok_status.site_kind, CallSiteKind::Path);
    assert_eq!(ok_status.status, CallStatusKind::Unsupported);
    assert_eq!(ok_status.resolution, None);
    assert!(
        db.call_targets_for_site(ok_site.id)?.is_empty(),
        "unsupported Ok path should not have call targets"
    );

    let try_path = path(&["try_local_assoc"]);
    let try_site = sites
        .iter()
        .find(|site| site.kind == CallSiteKind::Path && site.path.as_ref() == Some(&try_path))
        .expect("try_local_assoc path call site");
    let try_status = db
        .call_resolution_for_site(try_site.id)?
        .expect("try_local_assoc status row");
    assert_eq!(try_status.site_id, try_site.id);
    assert_eq!(try_status.site_kind, CallSiteKind::Path);
    assert_eq!(try_status.status, CallStatusKind::Resolved);
    assert_eq!(try_status.resolution, Some(CallResolutionKind::LocalExact));
    let try_targets = db.call_targets_for_site(try_site.id)?;
    assert_eq!(try_targets.len(), 1, "try targets: {try_targets:#?}");
    assert_eq!(try_targets[0].site_id, try_site.id);
    assert_eq!(try_targets[0].target_id, try_target);
    assert_eq!(try_targets[0].relation, CallRelationKind::Function);
    assert_eq!(try_targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(try_targets[0].target_kind, CallTargetKind::Function);

    let receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let method_site = sites
        .iter()
        .find(|site| {
            site.kind == CallSiteKind::Method
                && site.method.as_deref() == Some("instance_value")
                && site.receiver.as_ref() == Some(&receiver)
        })
        .expect("try receiver method call site");
    let method_status = db
        .call_resolution_for_site(method_site.id)?
        .expect("method status row");
    assert_eq!(method_status.site_id, method_site.id);
    assert_eq!(method_status.site_kind, CallSiteKind::Method);
    assert_eq!(method_status.status, CallStatusKind::Resolved);
    assert_eq!(
        method_status.resolution,
        Some(CallResolutionKind::LocalExact)
    );
    let method_targets = db.call_targets_for_site(method_site.id)?;
    assert_eq!(
        method_targets.len(),
        1,
        "method targets: {method_targets:#?}"
    );
    assert_eq!(method_targets[0].site_id, method_site.id);
    assert_eq!(method_targets[0].target_id, method_target);
    assert_eq!(method_targets[0].relation, CallRelationKind::Method);
    assert_eq!(method_targets[0].source_kind, CallSiteKind::Method);
    assert_eq!(method_targets[0].target_kind, CallTargetKind::Method);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_result_receiver_method_chains() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let owner = function_id_by_name(&db, "call_method_result_instance_method")?;
    let clone_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "clone_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "method-result context rows: {context:#?}");

    let clone_receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let row = row_by_method_receiver(&context, "clone_assoc", &clone_receiver);
    assert_resolved_target(
        row,
        clone_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let result_receiver = CallReceiver::MethodCallResult {
        method_name: "clone_assoc".to_string(),
    };
    let row = row_by_method_receiver(&context, "instance_value", &result_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_await_result_instance_method")?;
    let ready_target = function_id_by_name(&db, "make_ready_local_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "await-result context rows: {context:#?}");

    let row = row_by_path(&context, &["make_ready_local_assoc"]);
    assert_resolved_target(
        row,
        ready_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let await_receiver = CallReceiver::AwaitPathCallResult {
        path: path(&["make_ready_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &await_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "try-result context rows: {context:#?}");

    let row = row_by_path(&context, &["Ok"]);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(row.targets.is_empty(), "Ok row targets: {row:#?}");

    let row = row_by_path(&context, &["try_local_assoc"]);
    assert_resolved_target(
        row,
        try_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let try_receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &try_receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_field_receiver_and_dynamic_field_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;

    let owner = function_id_by_name(&db, "call_tuple_field_instance_method")?;
    let struct_target = struct_id_by_name(&db, "TupleFieldMethodReceiver")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-method context rows: {context:#?}");

    let row = row_by_path(&context, &["TupleFieldMethodReceiver"]);
    assert_resolved_target(
        row,
        struct_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );

    let receiver = CallReceiver::FieldInitializedLocalBinding {
        name: "value".to_string(),
        init_path: path(&["TupleFieldMethodReceiver"]),
        field_path: path(&["0"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = function_id_by_name(&db, "call_tuple_field_function")?;
    let struct_target = struct_id_by_name(&db, "TupleFieldFunction")?;
    let function_target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-dynamic context rows: {context:#?}");

    let row = row_by_path(&context, &["TupleFieldFunction"]);
    assert_resolved_target(
        row,
        struct_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );

    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["value", "0"]);
    assert_eq!(row.site.receiver, None);
    assert_resolved_target(
        row,
        function_target,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_function_pointer_param_cast_path_without_target() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_function_pointer_param_cast")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "function-pointer param cast context rows: {context:#?}"
    );

    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["f"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.receiver, None);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "opaque function-pointer param cast must not fabricate targets: {row:#?}"
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_macro_statuses_without_targets() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        ("call_crate_scoped_macro", "crate::crate_scoped_macro"),
        ("call_vec_macro", "vec"),
        ("call_imported_macro_alias", "imported_macro_alias"),
        ("call_item_macro_inside_body", "call_graph_item_macro"),
    ];

    for (owner_name, macro_name) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Macro);
        assert_eq!(row.site.macro_name.as_deref(), Some(macro_name));
        assert_eq!(row.site.path, None);
        assert_eq!(row.site.receiver, None);
        assert_eq!(row.site.arg_count, None);
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(row.targets.is_empty(), "macro row targets: {row:#?}");
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_enum_variant_constructor_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let owner = function_id_by_name_in_module(&db, &["crate", "imports"], "use_imported_items")?;

    let context = db.call_context_for_owner(owner)?;
    let row = row_by_path(&context, &["EnumWithData", "Variant1"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(
        row.targets[0].relation,
        CallRelationKind::EnumVariantConstructor
    );
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Variant);

    Ok(())
}
