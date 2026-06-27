use super::*;

fn domain_records_for(
    build_domain_id: &str,
    cfg_domain_id: &str,
    invocation_id: &str,
) -> Vec<serde_json::Value> {
    let mut cfg_record = admitted_cfg_domain_record();
    cfg_record["cfg_domain_id"] = json!(cfg_domain_id);
    cfg_record["build_domain_id"] = json!(build_domain_id);

    let mut rustc_record = admitted_rustc_invocation_record();
    rustc_record["invocation_id"] = json!(invocation_id);
    rustc_record["build_domain_id"] = json!(build_domain_id);

    vec![
        build_domain_record_for(build_domain_id),
        cfg_record,
        rustc_record,
    ]
}

fn main_domain_records() -> Vec<serde_json::Value> {
    domain_records_for("bd:main", "cfg:main", "rustc:main")
}

#[test]
fn proof_graph_store_accepts_external_summary_artifacts() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(external_summary_record());
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let by_id = db
        .proof_graphrag_context("external-summary:dep:serde")
        .expect("external summary proof context");
    assert!(
        by_id.iter().any(|row| {
            row.kind == "external_summary"
                && row.fact_id == "external-summary:dep:serde"
                && row.build_domain_id.as_deref() == Some("bd:main")
                && row.summary_class.as_deref() == Some("opaque_blocked")
                && row.artifact_hash.as_deref() == Some("sha256:serde-artifact")
                && row.summary_version.as_deref() == Some("serde 1.0.0")
                && row.review_method.as_deref() == Some("manual-review")
                && row.scope_of_validity.as_deref() == Some("dependency serde under bd:main")
                && row.allowed_effects == vec!["external_summary_boundary".to_string()]
                && row.required_containment.as_deref() == Some("none")
                && row.invalidation_conditions.as_deref()
                    == Some("artifact hash or proof policy changes")
                && row.status.as_deref() == Some("blocked")
                && row.detail.as_deref() == Some("opaque_blocked")
        }),
        "GraphRAG proof lookup should expose stored external summary artifacts: {by_id:#?}"
    );

    let by_class = db
        .proof_graphrag_context("opaque_blocked")
        .expect("external summary class proof context");
    assert!(
        by_class.iter().any(
            |row| row.kind == "external_summary" && row.fact_id == "external-summary:dep:serde"
        ),
        "GraphRAG proof lookup should match JSON-only external summary class fields: {by_class:#?}"
    );

    let by_hash = db
        .proof_graphrag_context("sha256:serde-artifact")
        .expect("external summary artifact hash context");
    assert!(
        by_hash.iter().any(
            |row| row.kind == "external_summary" && row.fact_id == "external-summary:dep:serde"
        ),
        "GraphRAG proof lookup should match external summary artifact hashes: {by_hash:#?}"
    );
}

#[test]
fn proof_graphrag_context_links_external_summary_artifacts_and_resolution_refs() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(externally_summarized_resolution(Some(
        "external-summary:dep:serde",
    )));
    records.push(external_summary_record());
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("external-summary:dep:serde")
        .expect("external summary proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "external_summary" && row.fact_id == "external-summary:dep:serde"
        }),
        "summary-id lookup should include the stored external summary artifact: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some("call:external")
                && row.external_summary_id.as_deref() == Some("external-summary:dep:serde")
        }),
        "summary-id lookup should include call_resolution rows that reference the artifact: {rows:#?}"
    );
}

#[test]
fn proof_graph_store_rejects_external_summary_without_artifact_identity() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut summary = external_summary_record();
    summary
        .as_object_mut()
        .expect("external summary object")
        .remove("artifact_hash");
    records.push(summary);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("external_summary without artifact_hash should reject the whole batch");
    assert!(error.to_string().contains("artifact_hash"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graph_store_rejects_external_summary_without_evidence_use() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut summary = external_summary_record();
    summary
        .as_object_mut()
        .expect("external summary object")
        .remove("evidence_use");
    records.push(summary);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("external_summary without evidence_use should reject the whole batch");
    assert!(error.to_string().contains("evidence_use"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graph_store_rejects_externally_summarized_rows_without_evidence_use() {
    for (label, mut row) in [
        (
            "expansion_boundary",
            externally_summarized_boundary(Some("external-summary:dep:serde")),
        ),
        (
            "call_resolution",
            externally_summarized_resolution(Some("external-summary:dep:serde")),
        ),
    ] {
        let db = Database::new_init().expect("create db");
        db.ensure_proof_graph_schema().expect("proof graph schema");
        row.as_object_mut()
            .expect("externally summarized row object")
            .remove("evidence_use");
        let mut records = proof_records();
        records.push(row);

        let error = db.upsert_proof_fact_values(&records).expect_err(&format!(
            "{label} without evidence_use should reject the whole batch"
        ));
        assert!(error.to_string().contains("evidence_use"));
        assert!(
            db.proof_graphrag_context("")
                .expect("query graph")
                .is_empty()
        );
    }
}

#[test]
fn proof_graph_store_rejects_external_summary_boundary_without_summary_id() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(externally_summarized_boundary(None));

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("externally_summarized expansion_boundary should require external_summary_id");
    assert!(error.to_string().contains("external_summary_id"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graphrag_context_exposes_external_summary_boundary_ids() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(externally_summarized_boundary(Some(
        "external-summary:macro:serde",
    )));
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("external-summary:macro:serde")
        .expect("boundary summary proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "expansion_boundary"
                && row.fact_id == "boundary:external-summary"
                && row.status.as_deref() == Some("externally_summarized")
                && row.detail.as_deref() == Some("external_summary")
                && row.external_summary_id.as_deref() == Some("external-summary:macro:serde")
        }),
        "GraphRAG proof lookup should expose external_summary_id for externally_summarized expansion boundaries: {rows:#?}"
    );
}

#[test]
fn proof_graph_store_rejects_external_summary_resolution_without_summary_id() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(externally_summarized_resolution(None));

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("externally_summarized call_resolution should require external_summary_id");
    assert!(error.to_string().contains("external_summary_id"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graphrag_context_exposes_external_summary_ids() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(externally_summarized_resolution(Some(
        "external-summary:dep:serde",
    )));
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("external-summary:dep:serde")
        .expect("external summary proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some("call:external")
                && row.resolution_state.as_deref() == Some("externally_summarized")
                && row.external_summary_id.as_deref() == Some("external-summary:dep:serde")
        }),
        "GraphRAG proof lookup should expose external_summary_id for externally_summarized call resolutions: {rows:#?}"
    );
}

#[test]
fn proof_blockers_include_derived_summary_and_resolution_gaps() {
    struct Case<'a> {
        blocker_id: &'a str,
        reason: &'a str,
        status: &'a str,
        build_domain_id: Option<&'a str>,
        call_site_id: Option<&'a str>,
        detail: &'a str,
    }

    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut boundary = externally_summarized_boundary(Some("external-summary:dep:serde"));
    boundary
        .as_object_mut()
        .expect("boundary object")
        .remove("blocking_reason");
    let mut records = main_domain_records();
    records.extend([
        externally_summarized_resolution(Some("external-summary:dep:serde")),
        boundary,
        external_summary_record(),
    ]);
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let blockers = db.proof_blockers().expect("blocker inspection");
    let cases = [
        Case {
            blocker_id: "boundary:external-summary",
            reason: "external_dependency_summary_missing",
            status: "externally_summarized",
            build_domain_id: Some("bd:main"),
            call_site_id: None,
            detail: "external_summary",
        },
        Case {
            blocker_id: "external-summary:dep:serde",
            reason: "opaque_blocked",
            status: "blocked",
            build_domain_id: Some("bd:main"),
            call_site_id: None,
            detail: "opaque_blocked",
        },
        Case {
            blocker_id: "resolution:call:external",
            reason: "external_dependency_summary_missing",
            status: "externally_summarized",
            build_domain_id: None,
            call_site_id: Some("call:external"),
            detail: "external_dependency_summary_missing",
        },
    ];

    assert_eq!(
        blockers.len(),
        cases.len(),
        "derived proof blockers: {blockers:#?}"
    );
    for case in cases {
        let row = blockers
            .iter()
            .find(|row| row.blocker_id == case.blocker_id)
            .unwrap_or_else(|| panic!("missing blocker {} in {blockers:#?}", case.blocker_id));
        assert_eq!(row.reason, case.reason, "{}", case.blocker_id);
        assert_eq!(row.status, case.status, "{}", case.blocker_id);
        assert_eq!(
            row.build_domain_id.as_deref(),
            case.build_domain_id,
            "{}",
            case.blocker_id
        );
        assert_eq!(
            row.call_site_id.as_deref(),
            case.call_site_id,
            "{}",
            case.blocker_id
        );
        assert_eq!(row.detail, case.detail, "{}", case.blocker_id);
    }
}

#[test]
fn proof_blockers_discharge_linked_admitted_external_summary_artifacts() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut resolution = externally_summarized_resolution(Some("external-summary:dep:serde"));
    resolution
        .as_object_mut()
        .expect("resolution object")
        .remove("blocking_reason");
    let mut boundary = externally_summarized_boundary(Some("external-summary:dep:serde"));
    boundary
        .as_object_mut()
        .expect("boundary object")
        .remove("blocking_reason");
    let mut summary = external_summary_record();
    summary["summary_class"] = json!("audited_no_process_effects");
    summary["status"] = json!("admitted");
    let mut records = main_domain_records();
    records.extend([external_call_site_record(), resolution, boundary, summary]);
    db.upsert_proof_fact_values(&records)
        .expect("import admitted external summary proof facts");

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert!(
        blockers.is_empty(),
        "admitted linked external summary should discharge derived missing-summary blockers: {blockers:#?}"
    );

    let rows = db
        .proof_graphrag_context("external-summary:dep:serde")
        .expect("external summary proof context");
    assert!(
        rows.iter()
            .filter(|row| { row.kind == "call_resolution" || row.kind == "expansion_boundary" })
            .all(|row| row.blocker_reason.is_none()),
        "admitted linked external summary should clear derived context blockers: {rows:#?}"
    );
}

#[test]
fn proof_blockers_require_linked_call_site_domain_for_external_summary_discharge() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut resolution = externally_summarized_resolution(Some("external-summary:dep:serde"));
    resolution
        .as_object_mut()
        .expect("resolution object")
        .remove("blocking_reason");
    let mut summary = external_summary_record();
    summary["summary_class"] = json!("audited_no_process_effects");
    summary["status"] = json!("admitted");
    let mut records = main_domain_records();
    records.extend([resolution, summary]);
    db.upsert_proof_fact_values(&records)
        .expect("import unscoped external summary proof facts");

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert_eq!(
        blockers.len(),
        1,
        "call-site scoped resolution without a linked call_site domain should fail closed: {blockers:#?}"
    );
    assert_eq!(blockers[0].blocker_id, "resolution:call:external");
    assert_eq!(blockers[0].reason, "external_dependency_summary_missing");

    let rows = db
        .proof_graphrag_context("external-summary:dep:serde")
        .expect("external summary proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some("call:external")
                && row.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
        }),
        "context lookup should retain the missing-summary blocker when call_site scope is absent: {rows:#?}"
    );
}

#[test]
fn proof_blockers_require_allowed_effect_for_external_summary_discharge() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut resolution = externally_summarized_resolution(Some("external-summary:dep:serde"));
    resolution
        .as_object_mut()
        .expect("resolution object")
        .remove("blocking_reason");
    let mut boundary = externally_summarized_boundary(Some("external-summary:dep:serde"));
    boundary
        .as_object_mut()
        .expect("boundary object")
        .remove("blocking_reason");
    let mut summary = external_summary_record();
    summary["summary_class"] = json!("audited_no_process_effects");
    summary["status"] = json!("admitted");
    summary["allowed_effects"] = json!(["durable_evidence_read"]);
    let mut records = main_domain_records();
    records.extend([resolution, boundary, summary]);
    db.upsert_proof_fact_values(&records)
        .expect("import admitted external summary proof facts");

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert_eq!(
        blockers.len(),
        2,
        "admitted external summary without external-summary authority should fail closed: {blockers:#?}"
    );
    assert!(
        blockers
            .iter()
            .all(|row| row.reason == "external_dependency_summary_missing"),
        "missing allowed external-summary effect should preserve missing-summary blockers: {blockers:#?}"
    );
}

#[test]
fn proof_blockers_require_call_site_domain_match_for_external_summary_discharge() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut resolution = externally_summarized_resolution(Some("external-summary:dep:serde"));
    resolution
        .as_object_mut()
        .expect("resolution object")
        .remove("blocking_reason");
    let mut summary = external_summary_record();
    summary["summary_class"] = json!("audited_no_process_effects");
    summary["status"] = json!("admitted");
    summary["build_domain_id"] = json!("bd:other");
    let mut records = main_domain_records();
    records.extend(domain_records_for("bd:other", "cfg:other", "rustc:other"));
    records.extend([external_call_site_record(), resolution, summary]);
    db.upsert_proof_fact_values(&records)
        .expect("import mismatched external summary proof facts");

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert_eq!(
        blockers.len(),
        1,
        "external summary in a different build domain should not discharge call-site scoped resolution blockers: {blockers:#?}"
    );
    assert_eq!(blockers[0].blocker_id, "resolution:call:external");
    assert_eq!(blockers[0].reason, "external_dependency_summary_missing");
}

#[test]
fn proof_blockers_keep_macro_and_build_summary_gaps_without_specific_semantics() {
    for (boundary_kind, expected_reason) in [
        ("proc_macro_function", "proc_macro_summary_missing"),
        ("build_script", "build_script_summary_missing"),
    ] {
        let db = Database::new_init().expect("create db");
        db.ensure_proof_graph_schema().expect("proof graph schema");
        let mut boundary = externally_summarized_boundary(Some("external-summary:dep:serde"));
        boundary["boundary_kind"] = json!(boundary_kind);
        boundary
            .as_object_mut()
            .expect("boundary object")
            .remove("blocking_reason");
        let mut summary = external_summary_record();
        summary["summary_class"] = json!("audited_no_process_effects");
        summary["status"] = json!("admitted");
        let mut records = main_domain_records();
        records.extend([boundary, summary]);
        db.upsert_proof_fact_values(&records)
            .expect("import admitted external summary proof facts");

        let blockers = db.proof_blockers().expect("blocker inspection");
        assert_eq!(
            blockers.len(),
            1,
            "{boundary_kind} should not be discharged by generic external-summary semantics: {blockers:#?}"
        );
        assert_eq!(blockers[0].blocker_id, "boundary:external-summary");
        assert_eq!(blockers[0].reason, expected_reason);
    }
}

#[test]
fn proof_blockers_discharge_macro_and_build_summaries_with_dedicated_authority() {
    for (boundary_kind, effect) in [
        ("proc_macro_function", "proc_macro_summary_boundary"),
        ("build_script", "build_script_summary_boundary"),
    ] {
        let db = Database::new_init().expect("create db");
        db.ensure_proof_graph_schema().expect("proof graph schema");
        let mut boundary = externally_summarized_boundary(Some("external-summary:dep:serde"));
        boundary["boundary_kind"] = json!(boundary_kind);
        boundary
            .as_object_mut()
            .expect("boundary object")
            .remove("blocking_reason");
        let mut summary = external_summary_record();
        summary["summary_class"] = json!("audited_no_process_effects");
        summary["status"] = json!("admitted");
        summary["allowed_effects"] = json!([effect]);
        let mut records = main_domain_records();
        records.extend([boundary, summary]);
        db.upsert_proof_fact_values(&records)
            .expect("import dedicated macro/build summary proof facts");

        let blockers = db.proof_blockers().expect("blocker inspection");
        assert!(
            blockers.is_empty(),
            "{boundary_kind} should discharge when the admitted summary carries dedicated authority: {blockers:#?}"
        );

        let rows = db
            .proof_graphrag_context("external-summary:dep:serde")
            .expect("summary context");
        assert!(
            rows.iter()
                .filter(|row| row.kind == "expansion_boundary")
                .all(|row| row.blocker_reason.is_none()),
            "dedicated macro/build summary authority should clear derived context blockers: {rows:#?}"
        );
    }
}

#[test]
fn proof_blockers_require_matching_macro_or_build_summary_authority() {
    for (boundary_kind, effect, expected_reason) in [
        (
            "proc_macro_function",
            "build_script_summary_boundary",
            "proc_macro_summary_missing",
        ),
        (
            "build_script",
            "proc_macro_summary_boundary",
            "build_script_summary_missing",
        ),
    ] {
        let db = Database::new_init().expect("create db");
        db.ensure_proof_graph_schema().expect("proof graph schema");
        let mut boundary = externally_summarized_boundary(Some("external-summary:dep:serde"));
        boundary["boundary_kind"] = json!(boundary_kind);
        boundary
            .as_object_mut()
            .expect("boundary object")
            .remove("blocking_reason");
        let mut summary = external_summary_record();
        summary["summary_class"] = json!("audited_no_process_effects");
        summary["status"] = json!("admitted");
        summary["allowed_effects"] = json!([effect]);
        let mut records = main_domain_records();
        records.extend([boundary, summary]);
        db.upsert_proof_fact_values(&records)
            .expect("import mismatched macro/build summary proof facts");

        let blockers = db.proof_blockers().expect("blocker inspection");
        assert_eq!(
            blockers.len(),
            1,
            "{boundary_kind} should fail closed when summary authority is {effect}: {blockers:#?}"
        );
        assert_eq!(blockers[0].blocker_id, "boundary:external-summary");
        assert_eq!(blockers[0].reason, expected_reason);
    }
}
