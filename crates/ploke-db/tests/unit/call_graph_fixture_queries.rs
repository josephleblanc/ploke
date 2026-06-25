use ploke_db::{
    CallContextOptions, CallContextRelation, CallContextSeed, CallReceiver, CallRelationKind,
    CallResolutionKind, CallSiteKind, CallStatusKind, CallTargetKind, Database, DbError,
    ProofGraphStore, to_uuid,
};
use uuid::Uuid;

use super::call_graph_fixture_common::*;

mod blocker_proof;
mod constructor_proof;
mod context_expansion;
mod dynamic_proof;
mod invariants;
mod proof_lookup;
mod target_proof;

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
fn fixture_context_reads_projected_associated_function_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_local_assoc_make")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["LocalAssoc", "make"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(
        row.targets[0].relation,
        CallRelationKind::AssociatedFunction
    );
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_self_and_qualified_associated_function_calls()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let cases = [
        (
            method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?,
            path(&["Self", "make"]),
        ),
        (
            function_id_by_name(&db, "call_qualified_local_assoc_make")?,
            path(&["LocalAssoc", "make"]),
        ),
    ];

    for (owner, expected_path) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_imported_associated_function_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "ImportedAssoc", "make")?;
    let cases = [
        (
            "call_imported_type_assoc_make",
            path(&["ImportedAssocAlias", "make"]),
        ),
        (
            "call_glob_imported_type_assoc_make",
            path(&["ImportedAssoc", "make"]),
        ),
        (
            "call_reexported_type_assoc_make",
            path(&["ReexportedAssoc", "make"]),
        ),
    ];

    for (owner_name, expected_path) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(
            row.targets[0].relation,
            CallRelationKind::AssociatedFunction
        );
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
        assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_type_alias_associated_function_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let make_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let assoc_cases = [
        (
            "call_type_alias_assoc_make",
            path(&["LocalAssocTypeAlias", "make"]),
        ),
        (
            "call_type_alias_chain_assoc_make",
            path(&["LocalAssocAliasChain", "make"]),
        ),
        (
            "call_imported_type_alias_assoc_make",
            path(&["ImportedLocalAssocAlias", "make"]),
        ),
    ];

    for (owner_name, expected_path) in assoc_cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_resolved_target(
            row,
            make_target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
        );
    }

    let owner = function_id_by_name(&db, "call_method_as_associated_function")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "method-as-associated context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(
        row.site.path.as_ref(),
        Some(&path(&["LocalAssoc", "instance_value"]))
    );
    assert_eq!(row.site.arg_count, Some(1));
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_imported_trait_associated_function_calls() -> Result<(), DbError>
{
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_trait_name(&db, "LocalAssocFunctionTrait", "trait_make")?;
    let owner = function_id_by_name(&db, "call_trait_associated_function")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "local trait associated function context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(
        row.site.path.as_ref(),
        Some(&path(&["LocalAssocFunctionTrait", "trait_make"]))
    );
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );

    let target = method_id_by_trait_name(&db, "ImportedAssocFunctionTrait", "imported_trait_make")?;
    let cases = [
        (
            path(&["crate", "trait_assoc_function_scope", "with_direct_import"]),
            "call_direct_imported_trait_associated_function",
            path(&["ImportedAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "trait_assoc_function_scope", "with_alias_import"]),
            "call_alias_imported_trait_associated_function",
            path(&["VisibleAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "trait_assoc_function_scope", "with_glob_import"]),
            "call_glob_imported_trait_associated_function",
            path(&["ImportedAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "trait_assoc_reexport_scope"]),
            "call_reexported_trait_associated_function",
            path(&["ReexportedAssocFunctionTrait", "imported_trait_make"]),
        ),
        (
            path(&["crate", "grouped_trait_assoc_function_scope"]),
            "call_grouped_imported_trait_associated_function",
            path(&["GroupedAssocFunctionTrait", "imported_trait_make"]),
        ),
    ];

    for (module_path, owner_name, expected_path) in cases {
        let refs = module_path.iter().map(String::as_str).collect::<Vec<_>>();
        let owner = function_id_by_name_in_module(&db, &refs, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(
            row.targets[0].relation,
            CallRelationKind::AssociatedFunction
        );
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Path);
        assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_function_item_binding_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let imported_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "imported_target")?;
    let cases = [
        (
            "call_local_function_item_binding",
            path(&["f"]),
            local_target,
        ),
        (
            "call_aliased_function_item_binding",
            path(&["g"]),
            local_target,
        ),
        (
            "call_typed_function_pointer_binding",
            path(&["f"]),
            local_target,
        ),
        (
            "call_typed_function_pointer_alias_binding",
            path(&["g"]),
            local_target,
        ),
        (
            "call_imported_function_item_binding",
            path(&["f"]),
            imported_target,
        ),
    ];

    for (owner_name, expected_path, target) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
        assert_eq!(row.site.path.as_ref(), Some(&expected_path));
        assert_eq!(row.site.arg_count, Some(0));
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );
    }

    let owner = function_id_by_name(&db, "call_shadowed_local_target_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "shadowed binding context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["local_target"])));
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "shadowed closure binding must not fabricate local function edges: {row:#?}"
    );

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
fn fixture_context_reads_projected_dynamic_function_call() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_parenthesized_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");

    let row = &context[0];
    assert_eq!(row.site.kind, CallSiteKind::Dynamic);
    assert_eq!(row.site.path.as_ref(), Some(&path(&["local_target"])));
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, None);
    assert_eq!(row.status.status, CallStatusKind::Resolved);
    assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
    assert_eq!(row.targets.len(), 1);
    assert_eq!(row.targets[0].target_id, target);
    assert_eq!(row.targets[0].relation, CallRelationKind::DynamicFunction);
    assert_eq!(row.targets[0].source_kind, CallSiteKind::Dynamic);
    assert_eq!(row.targets[0].target_kind, CallTargetKind::Function);

    Ok(())
}

#[test]
fn fixture_context_reads_projected_parenthesized_binding_dynamic_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases = [
        ("call_parenthesized_function_item_binding", &["f"][..]),
        (
            "call_parenthesized_aliased_function_item_binding",
            &["g"][..],
        ),
        (
            "call_parenthesized_typed_function_pointer_alias_binding",
            &["g"][..],
        ),
    ];

    for (owner_name, expected_path) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, expected_path);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.site.receiver, None);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_returned_function_nested_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_function")?;
    let target = function_id_by_name(&db, "make_fn")?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned function context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["make_fn"]);
    assert_eq!(row.site.owner_id, owner);
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
        "expected one outer returned-function dynamic call row: {context:#?}"
    );
    let row = dynamic_rows[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
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
fn fixture_context_reads_projected_resolved_dynamic_function_shapes() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "local_target")?;
    let cases = [
        (
            "call_function_pointer_cast_path",
            path(&["local_target"]),
            1,
        ),
        ("call_function_pointer_cast_binding", path(&["f"]), 1),
        (
            "call_dereferenced_function_pointer_binding",
            path(&["f"]),
            1,
        ),
        ("call_block_function_item", path(&["local_target"]), 1),
        ("call_if_same_function_item", path(&["local_target"]), 1),
        ("call_match_same_function_item", path(&["local_target"]), 1),
        (
            "call_named_field_function_binding",
            path(&["holder", "callback"]),
            1,
        ),
        (
            "call_aliased_named_field_function_binding",
            path(&["alias", "callback"]),
            1,
        ),
        (
            "call_indexed_named_field_function_binding",
            path(&["holder", "callbacks", "0"]),
            1,
        ),
        (
            "call_indexed_named_field_array_alias_binding",
            path(&["holder", "callbacks", "0"]),
            1,
        ),
        (
            "call_aliased_indexed_named_field_function_binding",
            path(&["alias", "callbacks", "0"]),
            1,
        ),
        (
            "call_indexed_tuple_field_function_binding",
            path(&["holder", "0", "0"]),
            2,
        ),
        (
            "call_indexed_tuple_field_array_alias_binding",
            path(&["holder", "0", "0"]),
            2,
        ),
        (
            "call_aliased_indexed_tuple_field_function_binding",
            path(&["alias", "0", "0"]),
            2,
        ),
        (
            "call_indexed_initialized_function_array",
            path(&["funcs", "0"]),
            1,
        ),
        (
            "call_typed_indexed_initialized_function_array",
            path(&["funcs", "0"]),
            1,
        ),
        (
            "call_aliased_indexed_initialized_function_array",
            path(&["alias", "0"]),
            1,
        ),
    ];

    for (owner_name, expected_path, expected_rows) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let refs = expected_path.iter().map(String::as_str).collect::<Vec<_>>();
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            expected_rows,
            "{owner_name} context rows: {context:#?}"
        );

        let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &refs);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.site.receiver, None);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::DynamicFunction,
            CallSiteKind::Dynamic,
            CallTargetKind::Function,
        );
    }

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
fn fixture_context_reads_projected_targetless_dynamic_failures() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        (
            "call_match_guarded_function_item",
            None,
            CallStatusKind::Unsupported,
        ),
        ("call_if_closure_branch", None, CallStatusKind::Unsupported),
        ("call_match_closure_arm", None, CallStatusKind::Unsupported),
        (
            "call_parenthesized_function_pointer_param",
            Some(path(&["f"])),
            CallStatusKind::Unsupported,
        ),
        (
            "call_if_function_pointer_param_branch",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_match_function_pointer_param_arm",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_if_nested_branch_expression",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_match_nested_arm_expression",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_closure_binding_cast",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_dereferenced_closure_binding",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_field_function_param",
            Some(path(&["holder", "callback"])),
            CallStatusKind::Unsupported,
        ),
        (
            "call_indexed_function_pointer",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_indexed_field_function_param",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_indexed_tuple_field_function_param",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_move_closure_literal_with_body_call",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_async_closure_literal_with_body_call",
            None,
            CallStatusKind::Unsupported,
        ),
        (
            "call_parenthesized_generic_fn_once_value_binding",
            Some(path(&["generic_f"])),
            CallStatusKind::Unsupported,
        ),
    ];

    for (owner_name, expected_path, expected_status) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Dynamic);
        assert_eq!(row.site.path, expected_path);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, None);
        assert_eq!(row.status.status, expected_status);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "{owner_name} dynamic failure must not fabricate targets: {row:#?}"
        );
    }

    let owner = function_id_by_name(&db, "call_parenthesized_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "boxed dyn Fn context rows: {context:#?}");

    let row = row_by_path(&context, &["Box", "new"]);
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "Box::new must not fabricate local target edges: {row:#?}"
    );

    let row = row_by_kind_path(&context, CallSiteKind::Dynamic, &["boxed_fn"]);
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "boxed dyn Fn dynamic call must remain unsupported without fake targets: {row:#?}"
    );

    Ok(())
}

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
fn fixture_context_reads_projected_callable_value_path_failures_and_vec_external()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let path_failures = [
        ("call_function_pointer_param", &["f"][..]),
        ("call_generic_fn_once_value_binding", &["generic_f"][..]),
    ];

    for (owner_name, expected_path) in path_failures {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = row_by_path(&context, expected_path);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.arg_count, Some(0));
        assert_eq!(row.site.generic_arg_count, Some(0));
        assert_eq!(row.status.status, CallStatusKind::Unsupported);
        assert_eq!(row.status.resolution, None);
        assert!(
            row.targets.is_empty(),
            "callable value path row must not fabricate local targets: {row:#?}"
        );
    }

    let owner = function_id_by_name(&db, "call_boxed_dyn_fn_value_binding")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "boxed dyn Fn path context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["Box", "new"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "Box::new setup call must not fabricate local targets: {row:#?}"
    );

    let row = row_by_path(&context, &["boxed_fn"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::Unsupported);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "boxed dyn Fn path call must not fabricate local targets: {row:#?}"
    );

    let owner = function_id_by_name(&db, "call_prelude_vec_new")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "Vec::new context rows: {context:#?}");
    let row = row_by_path(&context, &["Vec", "new"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "Vec::new external row must not fabricate local targets: {row:#?}"
    );

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
fn fixture_context_reads_projected_trait_dispatch_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let cases = [
        (
            "call_param_trait_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_typed_local_trait_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_initialized_local_trait_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_parenthesized_initialized_local_trait_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_concrete_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_aliased_concrete_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_reference_alias_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
        (
            "call_reference_chain_trait_object_binding_method",
            CallReceiver::InitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["TraitDispatchTarget"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("trait_value"));
        assert_eq!(row.site.receiver, Some(receiver));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(row.targets[0].relation, CallRelationKind::Method);
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
        assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_constrained_generic_self_trait_method() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_constrained_generic_self_trait_method")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ConstrainedGenericSelfTrait",
        "GenericWrapper",
        "constrained_generic_self_value",
    )?;

    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "constrained generic self trait method context rows: {context:#?}"
    );

    let row = &context[0];
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Method);
    assert_eq!(
        row.site.method.as_deref(),
        Some("constrained_generic_self_value")
    );
    assert_eq!(
        row.site.receiver,
        Some(CallReceiver::LocalBinding {
            name: "value".to_string(),
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
fn fixture_context_reads_projected_generic_bound_and_trait_object_methods() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_trait_name(&db, "GenericBoundTrait", "bound_value")?;
    let cases = [
        (
            "call_inline_generic_bound_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_where_generic_bound_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_impl_trait_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_trait_object_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_local_trait_object_binding_method",
            CallReceiver::TypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["GenericBoundTrait"]),
            },
        ),
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("bound_value"));
        assert_eq!(row.site.receiver, Some(receiver));
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
fn fixture_context_reads_projected_imported_trait_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ScopedTrait",
        "ScopedTraitTarget",
        "scoped_value",
    )?;
    let cases = [
        (
            &["crate", "trait_scope", "with_direct_import"][..],
            "call_direct_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_alias_import"][..],
            "call_alias_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_glob_import"][..],
            "call_glob_imported_trait_method",
        ),
        (
            &["crate", "trait_reexport_scope"][..],
            "call_reexported_trait_method",
        ),
        (
            &["crate", "grouped_trait_import_scope"][..],
            "call_grouped_imported_trait_method",
        ),
    ];

    for (module_path, owner_name) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("scoped_value"));
        assert_eq!(
            row.site.receiver,
            Some(CallReceiver::LocalBinding {
                name: "value".to_string(),
            })
        );
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
fn fixture_context_reads_projected_blanket_trait_method_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let cases = [
        (
            "call_blanket_trait_method",
            "BlanketDispatchTrait",
            "blanket_value",
        ),
        (
            "call_inline_bound_blanket_trait_method",
            "InlineBoundBlanketTrait",
            "inline_bound_value",
        ),
        (
            "call_where_bound_blanket_trait_method",
            "WhereBoundBlanketTrait",
            "where_bound_value",
        ),
        (
            "call_transitive_bound_blanket_trait_method",
            "TransitiveBoundBlanketTrait",
            "transitive_bound_value",
        ),
    ];

    for (owner_name, trait_name, method_name) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let target = method_id_by_impl_trait_name(&db, trait_name, method_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some(method_name));
        assert_eq!(
            row.site.receiver,
            Some(CallReceiver::LocalBinding {
                name: "value".to_string(),
            })
        );
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
fn fixture_context_reads_projected_borrowed_and_dereferenced_method_receivers()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let cases = [
        (
            "call_borrowed_typed_local_instance_method",
            CallReceiver::BorrowedTypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_local_instance_method",
            CallReceiver::DereferencedInitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_param_instance_method",
            CallReceiver::DereferencedLocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_borrowed_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
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
    ];

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let row = &context[0];
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Method);
        assert_eq!(row.site.method.as_deref(), Some("instance_value"));
        assert_eq!(row.site.receiver, Some(receiver));
        assert_eq!(row.status.status, CallStatusKind::Resolved);
        assert_eq!(row.status.resolution, Some(CallResolutionKind::LocalExact));
        assert_eq!(row.targets.len(), 1);
        assert_eq!(row.targets[0].target_id, target);
        assert_eq!(row.targets[0].relation, CallRelationKind::Method);
        assert_eq!(row.targets[0].source_kind, CallSiteKind::Method);
        assert_eq!(row.targets[0].target_kind, CallTargetKind::Method);
    }

    Ok(())
}

#[test]
fn fixture_context_reads_projected_method_body_owner_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;

    let owner = method_id_by_trait_name(&db, "TraitDefaultCall", "default_calls_local")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait default path context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["local_target"]);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let owner = method_id_by_impl_trait_and_self_type_names(
        &db,
        "TraitImplBodyCallTrait",
        "TraitDispatchTarget",
        "impl_calls_required",
    )?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "TraitImplBodyCallTrait",
        "TraitDispatchTarget",
        "required_impl_call",
    )?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait impl self-method context rows: {context:#?}"
    );

    let row = row_by_method_receiver(&context, "required_impl_call", &CallReceiver::SelfValue);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = method_id_by_trait_name(&db, "DefaultRequiredCall", "default_calls_required")?;
    let target = method_id_by_trait_name(&db, "DefaultRequiredCall", "required")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait default self-method context rows: {context:#?}"
    );

    let row = row_by_method_receiver(&context, "required", &CallReceiver::SelfValue);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );

    let owner = method_id_by_trait_name(&db, "TraitDefaultAssocCall", "default_calls_assoc")?;
    let target = method_id_by_trait_name(&db, "TraitDefaultAssocCall", "required_assoc")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "trait default associated-function context rows: {context:#?}"
    );

    let row = row_by_path(&context, &["Self", "required_assoc"]);
    assert_eq!(row.site.owner_id, owner);
    assert_resolved_target(
        row,
        target,
        CallRelationKind::AssociatedFunction,
        CallSiteKind::Path,
        CallTargetKind::Method,
    );

    Ok(())
}

#[test]
fn fixture_context_reads_projected_const_and_static_initializer_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let target = function_id_by_name_in_module(&db, &["crate", "const_static"], "five")?;

    let cases = [
        (
            const_id_by_name(&db, "FN_CALL_CONST")?,
            "const initializer context rows",
        ),
        (
            static_id_by_name(&db, "STATIC_FN_CALL")?,
            "static initializer context rows",
        ),
    ];

    for (owner, label) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{label}: {context:#?}");

        let row = row_by_path(&context, &["five"]);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
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
fn fixture_context_reads_projected_associated_const_initializer_calls() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "assoc_const_value")?;
    let cases = [
        const_id_by_name(&db, "IMPL_ASSOC_VALUE")?,
        const_id_by_name(&db, "TRAIT_ASSOC_VALUE")?,
    ];

    for owner in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "associated const initializer context rows: {context:#?}"
        );

        let row = row_by_path(&context, &["assoc_const_value"]);
        assert_eq!(row.site.owner_id, owner);
        assert_eq!(row.site.kind, CallSiteKind::Path);
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

#[test]
fn fixture_projection_stores_real_resolved_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_crate_local_target")?;
    let target = function_id_by_name(&db, "local_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 1, "context rows: {context:#?}");
    let site = context[0].site.id;
    let span = context[0].site.span;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 3);

    assert_owner_proof_edges(
        &db,
        "resolved call",
        &[OwnerProofEdge {
            owner,
            site,
            span,
            target,
        }],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_path_resolution_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_target = function_id_by_name(&db, "local_target")?;
    let nested_target =
        function_id_by_name_in_module(&db, &["crate", "local_mod"], "nested_target")?;
    let imported_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "imported_target")?;
    let globbed_target =
        function_id_by_name_in_module(&db, &["crate", "import_targets"], "globbed_target")?;
    let cases: [(&[&str], &str, &[&str], Uuid); 11] = [
        (
            &["crate"],
            "call_unqualified_local_target",
            &["local_target"],
            local_target,
        ),
        (
            &["crate", "local_mod"],
            "call_self_nested_target",
            &["self", "nested_target"],
            nested_target,
        ),
        (
            &["crate"],
            "call_crate_module_nested_target",
            &["crate", "local_mod", "nested_target"],
            nested_target,
        ),
        (
            &["crate"],
            "call_self_module_nested_target",
            &["self", "local_mod", "nested_target"],
            nested_target,
        ),
        (
            &["crate", "super_path_scope"],
            "call_super_local_target",
            &["super", "local_target"],
            local_target,
        ),
        (
            &["crate"],
            "call_imported_alias_target",
            &["imported_alias"],
            imported_target,
        ),
        (
            &["crate"],
            "call_glob_imported_target",
            &["globbed_target"],
            globbed_target,
        ),
        (
            &["crate"],
            "call_reexported_target",
            &["reexported_target"],
            imported_target,
        ),
        (
            &["crate"],
            "call_imported_module_target",
            &["targets_alias", "globbed_target"],
            globbed_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_alias_target",
            &["grouped_alias"],
            imported_target,
        ),
        (
            &["crate", "grouped_function_import_scope"],
            "call_grouped_imported_globbed_target",
            &["grouped_globbed_alias"],
            globbed_target,
        ),
    ];
    let mut expected_edges = Vec::new();

    for (module_path, owner_name, expected_path, target) in cases {
        let owner = function_id_by_name_in_module(&db, module_path, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_path(&context, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "path-resolution",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_associated_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let local_make = method_id_by_impl_self_type_name(&db, "LocalAssoc", "make")?;
    let imported_make = method_id_by_impl_self_type_name(&db, "ImportedAssoc", "make")?;
    let instance_value = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let local_trait_make = method_id_by_trait_name(&db, "LocalAssocFunctionTrait", "trait_make")?;
    let imported_trait_make =
        method_id_by_trait_name(&db, "ImportedAssocFunctionTrait", "imported_trait_make")?;

    let mut cases = Vec::new();
    cases.push((
        "call_local_assoc_make",
        function_id_by_name(&db, "call_local_assoc_make")?,
        &["LocalAssoc", "make"][..],
        local_make,
    ));
    cases.push((
        "call_self_make",
        method_id_by_impl_self_type_name(&db, "LocalAssoc", "call_self_make")?,
        &["Self", "make"],
        local_make,
    ));
    cases.push((
        "call_qualified_local_assoc_make",
        function_id_by_name(&db, "call_qualified_local_assoc_make")?,
        &["LocalAssoc", "make"],
        local_make,
    ));
    cases.push((
        "call_imported_type_assoc_make",
        function_id_by_name(&db, "call_imported_type_assoc_make")?,
        &["ImportedAssocAlias", "make"],
        imported_make,
    ));
    cases.push((
        "call_glob_imported_type_assoc_make",
        function_id_by_name(&db, "call_glob_imported_type_assoc_make")?,
        &["ImportedAssoc", "make"],
        imported_make,
    ));
    cases.push((
        "call_reexported_type_assoc_make",
        function_id_by_name(&db, "call_reexported_type_assoc_make")?,
        &["ReexportedAssoc", "make"],
        imported_make,
    ));
    cases.push((
        "call_type_alias_assoc_make",
        function_id_by_name(&db, "call_type_alias_assoc_make")?,
        &["LocalAssocTypeAlias", "make"],
        local_make,
    ));
    cases.push((
        "call_type_alias_chain_assoc_make",
        function_id_by_name(&db, "call_type_alias_chain_assoc_make")?,
        &["LocalAssocAliasChain", "make"],
        local_make,
    ));
    cases.push((
        "call_imported_type_alias_assoc_make",
        function_id_by_name(&db, "call_imported_type_alias_assoc_make")?,
        &["ImportedLocalAssocAlias", "make"],
        local_make,
    ));
    cases.push((
        "call_method_as_associated_function",
        function_id_by_name(&db, "call_method_as_associated_function")?,
        &["LocalAssoc", "instance_value"],
        instance_value,
    ));
    cases.push((
        "call_trait_associated_function",
        function_id_by_name(&db, "call_trait_associated_function")?,
        &["LocalAssocFunctionTrait", "trait_make"],
        local_trait_make,
    ));

    for (module_path, owner_name, expected_path) in [
        (
            &["crate", "trait_assoc_function_scope", "with_direct_import"][..],
            "call_direct_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"][..],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_alias_import"],
            "call_alias_imported_trait_associated_function",
            &["VisibleAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "trait_assoc_function_scope", "with_glob_import"],
            "call_glob_imported_trait_associated_function",
            &["ImportedAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "trait_assoc_reexport_scope"],
            "call_reexported_trait_associated_function",
            &["ReexportedAssocFunctionTrait", "imported_trait_make"],
        ),
        (
            &["crate", "grouped_trait_assoc_function_scope"],
            "call_grouped_imported_trait_associated_function",
            &["GroupedAssocFunctionTrait", "imported_trait_make"],
        ),
    ] {
        cases.push((
            owner_name,
            function_id_by_name_in_module(&db, module_path, owner_name)?,
            expected_path,
            imported_trait_make,
        ));
    }

    let mut expected_edges = Vec::new();

    for (owner_name, owner, expected_path, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_path(&context, expected_path);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::AssociatedFunction,
            CallSiteKind::Path,
            CallTargetKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "associated-function",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_local_receiver_method_call_proof_facts() -> Result<(), DbError> {
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
        (
            "call_borrowed_typed_local_instance_method",
            CallReceiver::BorrowedTypedLocalBinding {
                name: "value".to_string(),
                type_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_local_instance_method",
            CallReceiver::DereferencedInitializedLocalBinding {
                name: "value".to_string(),
                init_path: path(&["LocalAssoc"]),
            },
        ),
        (
            "call_dereferenced_param_instance_method",
            CallReceiver::DereferencedLocalBinding {
                name: "value".to_string(),
            },
        ),
        (
            "call_borrowed_param_instance_method",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
        ),
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
    ];
    let mut expected_edges = Vec::new();

    for (owner_name, receiver) in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_method_receiver(&context, "instance_value", &receiver);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "local receiver method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_trait_family_method_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let generic_target = method_id_by_trait_name(&db, "GenericBoundTrait", "bound_value")?;
    let imported_target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ScopedTrait",
        "ScopedTraitTarget",
        "scoped_value",
    )?;
    let constrained_target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "ConstrainedGenericSelfTrait",
        "GenericWrapper",
        "constrained_generic_self_value",
    )?;
    let mut cases = Vec::new();

    for owner_name in [
        "call_inline_generic_bound_method",
        "call_where_generic_bound_method",
        "call_impl_trait_method",
        "call_trait_object_method",
    ] {
        cases.push((
            owner_name,
            function_id_by_name(&db, owner_name)?,
            "bound_value",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            generic_target,
        ));
    }

    cases.push((
        "call_local_trait_object_binding_method",
        function_id_by_name(&db, "call_local_trait_object_binding_method")?,
        "bound_value",
        CallReceiver::TypedLocalBinding {
            name: "value".to_string(),
            type_path: path(&["GenericBoundTrait"]),
        },
        generic_target,
    ));

    for (module_path, owner_name) in [
        (
            &["crate", "trait_scope", "with_direct_import"][..],
            "call_direct_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_alias_import"],
            "call_alias_imported_trait_method",
        ),
        (
            &["crate", "trait_scope", "with_glob_import"],
            "call_glob_imported_trait_method",
        ),
        (
            &["crate", "trait_reexport_scope"],
            "call_reexported_trait_method",
        ),
        (
            &["crate", "grouped_trait_import_scope"],
            "call_grouped_imported_trait_method",
        ),
    ] {
        cases.push((
            owner_name,
            function_id_by_name_in_module(&db, module_path, owner_name)?,
            "scoped_value",
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            imported_target,
        ));
    }

    cases.push((
        "call_constrained_generic_self_trait_method",
        function_id_by_name(&db, "call_constrained_generic_self_trait_method")?,
        "constrained_generic_self_value",
        CallReceiver::LocalBinding {
            name: "value".to_string(),
        },
        constrained_target,
    ));

    for (owner_name, trait_name, method_name) in [
        (
            "call_blanket_trait_method",
            "BlanketDispatchTrait",
            "blanket_value",
        ),
        (
            "call_inline_bound_blanket_trait_method",
            "InlineBoundBlanketTrait",
            "inline_bound_value",
        ),
        (
            "call_where_bound_blanket_trait_method",
            "WhereBoundBlanketTrait",
            "where_bound_value",
        ),
        (
            "call_transitive_bound_blanket_trait_method",
            "TransitiveBoundBlanketTrait",
            "transitive_bound_value",
        ),
    ] {
        cases.push((
            owner_name,
            function_id_by_name(&db, owner_name)?,
            method_name,
            CallReceiver::LocalBinding {
                name: "value".to_string(),
            },
            method_id_by_impl_trait_name(&db, trait_name, method_name)?,
        ));
    }

    let mut expected_edges = Vec::new();

    for (owner_name, owner, method_name, receiver, target) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");
        let row = row_by_method_receiver(&context, method_name, &receiver);
        let site = row.site.id;
        let span = row.site.span;
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3, "{owner_name} proof fact count");
        expected_edges.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "trait family method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_result_and_field_receiver_method_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let clone_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "clone_assoc")?;
    let make_target = function_id_by_name(&db, "make_local_assoc")?;
    let ready_target = function_id_by_name(&db, "make_ready_local_assoc")?;
    let tuple_target = struct_id_by_name(&db, "TupleFieldMethodReceiver")?;
    let mut expected_edges = Vec::new();

    let owner = function_id_by_name(&db, "call_path_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "path-result context rows: {context:#?}");
    let row = row_by_path(&context, &["make_local_assoc"]);
    assert_resolved_target(
        row,
        make_target,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: make_target,
    });

    let receiver = CallReceiver::PathCallResult {
        path: path(&["make_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_method_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "method-result context rows: {context:#?}");
    let receiver = CallReceiver::TypedLocalBinding {
        name: "value".to_string(),
        type_path: path(&["LocalAssoc"]),
    };
    let row = row_by_method_receiver(&context, "clone_assoc", &receiver);
    assert_resolved_target(
        row,
        clone_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: clone_target,
    });

    let receiver = CallReceiver::MethodCallResult {
        method_name: "clone_assoc".to_string(),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_await_result_instance_method")?;
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
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: ready_target,
    });

    let receiver = CallReceiver::AwaitPathCallResult {
        path: path(&["make_ready_local_assoc"]),
    };
    let row = row_by_method_receiver(&context, "instance_value", &receiver);
    assert_resolved_target(
        row,
        method_target,
        CallRelationKind::Method,
        CallSiteKind::Method,
        CallTargetKind::Method,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    let owner = function_id_by_name(&db, "call_tuple_field_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "field-method context rows: {context:#?}");
    let row = row_by_path(&context, &["TupleFieldMethodReceiver"]);
    assert_resolved_target(
        row,
        tuple_target,
        CallRelationKind::TupleStructConstructor,
        CallSiteKind::Path,
        CallTargetKind::Struct,
    );
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: tuple_target,
    });

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
    expected_edges.push(OwnerProofEdge {
        owner,
        site: row.site.id,
        span: row.site.span,
        target: method_target,
    });
    assert_eq!(
        db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?,
        6
    );

    assert_owner_proof_edges(
        &db,
        "result/field receiver method",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_returned_function_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_returned_function")?;
    let target = function_id_by_name(&db, "make_fn")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        2,
        "returned function proof context rows: {context:#?}"
    );

    let path_row = row_by_path(&context, &["make_fn"]);
    let path_site = path_row.site.id;
    let path_span = path_row.site.span;
    assert_resolved_target(
        path_row,
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
        "expected one outer returned-function dynamic row: {context:#?}"
    );
    let dynamic_row = dynamic_rows[0];
    let dynamic_site = dynamic_row.site.id;
    let dynamic_span = dynamic_row.site.span;
    assert_eq!(dynamic_row.status.status, CallStatusKind::Unsupported);
    assert_eq!(dynamic_row.status.resolution, None);
    assert!(
        dynamic_row.targets.is_empty(),
        "returned-function dynamic proof setup must be targetless: {dynamic_row:#?}"
    );

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 5);

    assert_owner_proof_edges(
        &db,
        "returned-function resolved path",
        &[OwnerProofEdge {
            owner,
            site: path_site,
            span: path_span,
            target,
        }],
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    assert_blocker_proofs(
        &db,
        "returned-function dynamic blocker",
        &[BlockerProofSite {
            site: dynamic_site,
            span: dynamic_span,
            blocker_reason: "dynamic_dispatch_unbounded",
        }],
        "fixture_call_graph/src/lib.rs",
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_const_and_static_initializer_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_nodes")?;
    let target = function_id_by_name_in_module(&db, &["crate", "const_static"], "five")?;
    let cases = [
        (
            const_id_by_name(&db, "FN_CALL_CONST")?,
            "const initializer proof context rows",
        ),
        (
            static_id_by_name(&db, "STATIC_FN_CALL")?,
            "static initializer proof context rows",
        ),
    ];
    let mut expected = Vec::new();

    for (owner, label) in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{label}: {context:#?}");

        let row = row_by_path(&context, &["five"]);
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.owner_id, owner);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-nodes")?;
        assert_eq!(count, 3);
        expected.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "const/static initializer",
        &expected,
        "fixture_nodes/src/const_static.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_associated_const_initializer_call_proof_facts()
-> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = function_id_by_name(&db, "assoc_const_value")?;
    let cases = [
        const_id_by_name(&db, "IMPL_ASSOC_VALUE")?,
        const_id_by_name(&db, "TRAIT_ASSOC_VALUE")?,
    ];
    let mut expected = Vec::new();

    for owner in cases {
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(
            context.len(),
            1,
            "associated const initializer proof context rows: {context:#?}"
        );

        let row = row_by_path(&context, &["assoc_const_value"]);
        let site = row.site.id;
        let span = row.site.span;
        assert_eq!(row.site.owner_id, owner);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Function,
            CallSiteKind::Path,
            CallTargetKind::Function,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        expected.push(OwnerProofEdge {
            owner,
            site,
            span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "associated const initializer",
        &expected,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}

#[test]
fn fixture_projection_stores_real_multi_row_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let try_target = function_id_by_name(&db, "try_local_assoc")?;
    let method_target = method_id_by_impl_self_type_name(&db, "LocalAssoc", "instance_value")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "context rows: {context:#?}");
    let ok_site = row_by_path(&context, &["Ok"]).site.id;
    let try_site = row_by_path(&context, &["try_local_assoc"]).site.id;
    let receiver = CallReceiver::TryPathCallResult {
        path: path(&["try_local_assoc"]),
    };
    let method_site = row_by_method_receiver(&context, "instance_value", &receiver)
        .site
        .id;

    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(count, 8);

    let mut edges = db.proof_checker_edges()?;
    edges.sort_by(|left, right| left.call_site_id.cmp(&right.call_site_id));
    assert_eq!(edges.len(), 2, "proof checker edges: {edges:#?}");
    assert!(
        edges
            .iter()
            .all(|edge| edge.caller_def_id == owner.to_string())
    );
    assert!(edges.iter().all(|edge| edge.resolution_state == "resolved"));
    assert!(edges.iter().all(|edge| edge.blocker_reason.is_none()));
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == try_site.to_string()
                && edge.callee_def_id.as_deref() == Some(try_target.to_string().as_str())
        }),
        "try path proof edges: {edges:#?}"
    );
    assert!(
        edges.iter().any(|edge| {
            edge.call_site_id == method_site.to_string()
                && edge.callee_def_id.as_deref() == Some(method_target.to_string().as_str())
        }),
        "method proof edges: {edges:#?}"
    );

    let blocked = db.proof_graphrag_context("type_resolution_missing")?;
    assert!(
        blocked.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some(ok_site.to_string().as_str())
                && row.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "blocked proof rows: {blocked:#?}"
    );

    for site in [ok_site, try_site, method_site] {
        let provenance = db
            .proof_source_provenance(&site.to_string())?
            .expect("projected fixture call-site source provenance");
        assert!(
            provenance
                .source_file
                .ends_with("fixture_call_graph/src/lib.rs"),
            "source provenance: {provenance:#?}"
        );
    }

    Ok(())
}

#[test]
fn fixture_projection_links_mixed_owner_proof_rows_to_call_context() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_try_result_instance_method")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 3, "context rows: {context:#?}");

    let expected_fact_count = context
        .iter()
        .map(|row| 2 + row.targets.len())
        .sum::<usize>();
    let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
    assert_eq!(
        count, expected_fact_count,
        "proof projection should emit one call_site and one call_resolution per site plus resolved edges"
    );

    let proof_rows = db.proof_graphrag_context("")?;
    let resolved_context = context
        .iter()
        .filter(|row| row.status.status == CallStatusKind::Resolved)
        .count();
    let checker_edges = db.proof_checker_edges()?;
    assert_eq!(
        checker_edges.len(),
        resolved_context,
        "proof checker edges should match resolved call-context rows: {checker_edges:#?}"
    );

    for row in &context {
        let site = row.site.id.to_string();
        let site_rows = proof_rows
            .iter()
            .filter(|fact| fact.call_site_id.as_deref() == Some(site.as_str()))
            .collect::<Vec<_>>();
        assert_eq!(
            proof_kind_count(&site_rows, "call_site"),
            1,
            "projected proof rows should include one call_site fact for {site}: {site_rows:#?}"
        );
        assert_eq!(
            proof_kind_count(&site_rows, "call_resolution"),
            1,
            "projected proof rows should include one call_resolution fact for {site}: {site_rows:#?}"
        );

        let edge_rows = site_rows
            .iter()
            .filter(|fact| fact.kind == "call_edge")
            .collect::<Vec<_>>();
        match row.status.status {
            CallStatusKind::Resolved => {
                assert_eq!(
                    edge_rows.len(),
                    1,
                    "resolved call site {site} should project one call_edge fact: {site_rows:#?}"
                );
                let owner_id = owner.to_string();
                let target_id = row.targets[0].target_id.to_string();
                assert_eq!(
                    edge_rows[0].caller_def_id.as_deref(),
                    Some(owner_id.as_str())
                );
                assert_eq!(
                    edge_rows[0].callee_def_id.as_deref(),
                    Some(target_id.as_str())
                );
                assert_eq!(edge_rows[0].blocker_reason, None);
            }
            CallStatusKind::Unresolved
            | CallStatusKind::Ambiguous
            | CallStatusKind::External
            | CallStatusKind::Unsupported => {
                assert!(
                    edge_rows.is_empty(),
                    "non-resolved call site {site} must not project call_edge facts: {site_rows:#?}"
                );
                let resolution = proof_fact_for_kind(&site_rows, "call_resolution");
                assert!(
                    resolution.blocker_reason.is_some(),
                    "non-resolved call site {site} should project a blocker reason: {site_rows:#?}"
                );
            }
        }
    }

    Ok(())
}

#[test]
fn fixture_projection_stores_real_trait_dispatch_call_proof_facts() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let target = method_id_by_impl_trait_and_self_type_names(
        &db,
        "LocalDispatchTrait",
        "TraitDispatchTarget",
        "trait_value",
    )?;
    let cases = [
        "call_initialized_local_trait_method",
        "call_concrete_trait_object_binding_method",
        "call_reference_chain_trait_object_binding_method",
    ];
    let mut expected_edges = Vec::new();

    for owner_name in cases {
        let owner = function_id_by_name(&db, owner_name)?;
        let context = db.call_context_for_owner(owner)?;
        assert_eq!(context.len(), 1, "{owner_name} context rows: {context:#?}");

        let receiver = CallReceiver::InitializedLocalBinding {
            name: "value".to_string(),
            init_path: path(&["TraitDispatchTarget"]),
        };
        let row = row_by_method_receiver(&context, "trait_value", &receiver);
        assert_resolved_target(
            row,
            target,
            CallRelationKind::Method,
            CallSiteKind::Method,
            CallTargetKind::Method,
        );

        let count = db.project_call_proof_facts_for_owner(owner, "bd:fixture-call-graph")?;
        assert_eq!(count, 3);
        expected_edges.push(OwnerProofEdge {
            owner,
            site: row.site.id,
            span: row.site.span,
            target,
        });
    }

    assert_owner_proof_edges(
        &db,
        "trait dispatch",
        &expected_edges,
        "fixture_call_graph/src/lib.rs",
        "type_resolution_missing",
        ProofEdgeCount::Exact,
    )?;

    Ok(())
}
