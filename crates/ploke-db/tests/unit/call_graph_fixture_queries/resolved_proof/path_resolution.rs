use super::super::*;

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
