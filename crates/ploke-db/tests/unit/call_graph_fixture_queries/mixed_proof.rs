use super::*;

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
