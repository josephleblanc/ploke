use ploke_db::CallPathOptions;

use super::*;

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
    assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(&["abs"], 1, CallStatusKind::External, "extern C abs"),
    );

    let owner = function_id_by_name(&db, "call_imported_external_type_alias_constructor")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "imported external type alias constructor context rows: {context:#?}"
    );
    let row = assert_targetless_row(
        &context,
        owner,
        TargetlessRowCase::path(
            &["ImportedExternalVec", "new"],
            0,
            CallStatusKind::External,
            "imported external type alias constructor",
        ),
    );
    assert!(
        relations_for_site(&db, row.site.id)?.rows.is_empty(),
        "imported external type alias constructor must not fabricate local call_relation targets"
    );

    let owner = function_id_by_name(&db, "call_chained_returned_function")?;
    let maker = function_id_by_name(&db, "make_unary_fn")?;
    let returned = function_id_by_name(&db, "unary_target")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(context.len(), 2, "chained call context rows: {context:#?}");

    let row = row_by_path(&context, &["make_unary_fn"]);
    assert_eq!(row.site.arg_count, Some(0));
    assert_eq!(row.site.generic_arg_count, Some(0));
    assert_resolved_target(
        row,
        maker,
        CallRelationKind::Function,
        CallSiteKind::Path,
        CallTargetKind::Function,
    );

    let dynamic = row_by_kind_path(&context, CallSiteKind::Dynamic, &["make_unary_fn"]);
    assert_eq!(dynamic.site.arg_count, Some(1));
    assert_resolved_target(
        dynamic,
        returned,
        CallRelationKind::DynamicFunction,
        CallSiteKind::Dynamic,
        CallTargetKind::Function,
    );

    let owner = function_id_by_name(&db, "call_qualified_dyn_any_downcast_mut")?;
    let context = db.call_context_for_owner(owner)?;
    assert_eq!(
        context.len(),
        1,
        "qualified dyn Any path context rows: {context:#?}"
    );
    let row = row_by_path(&context, &["std", "any", "Any", "downcast_mut"]);
    assert_eq!(row.site.owner_id, owner);
    assert_eq!(row.site.kind, CallSiteKind::Path);
    assert_eq!(row.site.arg_count, Some(1));
    assert_eq!(row.site.generic_arg_count, Some(1));
    assert_eq!(row.status.status, CallStatusKind::External);
    assert_eq!(row.status.resolution, None);
    assert!(
        row.targets.is_empty(),
        "qualified dyn Any downcast_mut must stay targetless: {row:#?}"
    );
    assert!(
        relations_for_site(&db, row.site.id)?.rows.is_empty(),
        "qualified dyn Any downcast_mut must stay an external frontier without local targets"
    );

    Ok(())
}

#[test]
fn fixture_reach_surfaces_extern_c_call_as_external_frontier() -> Result<(), DbError> {
    let db = setup_call_graph_fixture_db("fixture_call_graph")?;
    let owner = function_id_by_name(&db, "call_extern_c_function")?;

    // Usage questions:
    //   docs/active/agents/2026-06-30_call-graph-usage-questions.md
    //
    // Security analysis:
    //   "Which call paths can reach unsafe blocks or FFI boundaries?"
    //   "Which external dependency calls are made from this user-facing entrypoint?"
    //
    // Source oracle:
    //   tests/fixture_crates/fixture_call_graph/src/lib.rs:844 declares
    //   `abs(value)` inside an `unsafe extern "C"` block and calls it from
    //   `call_extern_c_function`. The call is visible as an external frontier,
    //   but no local traversal edge is fabricated for the foreign function.
    let report = db.call_reach_for_owner(
        owner,
        CallPathOptions {
            max_depth: 2,
            max_paths: 16,
        },
    )?;

    assert_eq!(report.owner.id, owner);
    assert_eq!(report.owner.name, "call_extern_c_function");
    assert!(
        report.paths.is_empty() && report.callees.is_empty(),
        "extern C calls should not fabricate local reach edges: {report:#?}"
    );
    let frontier = assert_targetless_row(
        &report.frontier_calls,
        owner,
        TargetlessRowCase::path(&["abs"], 1, CallStatusKind::External, "extern C abs"),
    );
    let external_frontier = assert_targetless_row(
        &report.external_frontier_calls,
        owner,
        TargetlessRowCase::path(&["abs"], 1, CallStatusKind::External, "extern C abs"),
    );
    assert_eq!(
        external_frontier.site.id, frontier.site.id,
        "external frontier subset should preserve the same extern C callsite"
    );
    assert!(
        report.unsupported_frontier_calls.is_empty()
            && report.unresolved_frontier_calls.is_empty()
            && report.ambiguous_frontier_calls.is_empty(),
        "extern C reach should classify the FFI boundary as external only: {report:#?}"
    );
    assert!(
        report
            .source_files
            .iter()
            .any(|file| file.ends_with("fixture_call_graph/src/lib.rs")),
        "extern C reach should point back to the fixture source file: {report:#?}"
    );

    Ok(())
}
