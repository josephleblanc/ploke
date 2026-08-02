use super::*;

fn entrypoint_record(definition_id: &str) -> serde_json::Value {
    json!({
        "fact_kind": "entrypoint_summary",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "entrypoint_summary_id": "entrypoint-summary:test-harness",
        "build_domain_id": "bd:main",
        "definition_id": definition_id,
        "target_kind": "test",
        "target_name": "generated-test-harness",
        "summary_class": "analyzed_source",
        "artifact_hash": "sha256:test-harness-artifact",
        "version": "test-harness-v1",
        "review_method": "source-oracle-review",
        "scope_of_validity": "generated #[test] harness reaches the target definition",
        "required_containment": "rust-test-harness",
        "invalidation_conditions": "source oracle, fixture hash, or proof policy changes",
        "status": "admitted",
        "evidence_use": "proof_only"
    })
}

#[test]
fn proof_graph_store_accepts_entrypoint_summary_artifacts() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(entrypoint_record("def:test-target"));
    db.upsert_proof_fact_values(&records)
        .expect("import entrypoint summary proof facts");

    let rows = db
        .proof_symbol_lookup("def:test-target")
        .expect("target proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "entrypoint_summary"
                && row.fact_id == "entrypoint-summary:test-harness"
                && row.build_domain_id.as_deref() == Some("bd:main")
                && row.definition_id.as_deref() == Some("def:test-target")
                && row.target_kind.as_deref() == Some("test")
                && row.target_name.as_deref() == Some("generated-test-harness")
                && row.summary_class.as_deref() == Some("analyzed_source")
                && row.artifact_hash.as_deref() == Some("sha256:test-harness-artifact")
                && row.summary_version.as_deref() == Some("test-harness-v1")
                && row.review_method.as_deref() == Some("source-oracle-review")
                && row.scope_of_validity.as_deref()
                    == Some("generated #[test] harness reaches the target definition")
                && row.required_containment.as_deref() == Some("rust-test-harness")
                && row.status.as_deref() == Some("admitted")
                && row.evidence_use.as_deref() == Some("proof_only")
        }),
        "target-centered proof lookup should expose generated entrypoint summaries: {rows:#?}"
    );
}

#[test]
fn proof_graph_store_rejects_entrypoint_summary_without_definition() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut summary = entrypoint_record("def:test-target");
    summary
        .as_object_mut()
        .expect("entrypoint summary object")
        .remove("definition_id");
    records.push(summary);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("entrypoint_summary without definition_id should reject the whole batch");
    assert!(error.to_string().contains("definition_id"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graph_store_rejects_entrypoint_summary_invalid_target_kind() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut summary = entrypoint_record("def:test-target");
    summary["target_kind"] = json!("integration-test");
    records.push(summary);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("entrypoint_summary with non-schema target_kind should reject the whole batch");
    assert!(error.to_string().contains("target_kind"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}
