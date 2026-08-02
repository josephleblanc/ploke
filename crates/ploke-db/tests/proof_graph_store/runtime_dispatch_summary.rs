use super::*;

fn runtime_dispatch_summary_record() -> serde_json::Value {
    json!({
        "fact_kind": "runtime_dispatch_summary",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "dispatch_summary_id": "runtime-dispatch-summary:call:spawn",
        "build_domain_id": "bd:main",
        "call_site_id": "call:spawn",
        "summary_class": "allowed_only_under_containment",
        "artifact_hash": "sha256:runtime-dispatch-summary",
        "version": "runtime-dispatch-summary-v1",
        "review_method": "manual-review",
        "scope_of_validity": "call:spawn runtime dispatch frontier under bd:main",
        "required_containment": "summary does not create local traversal edges",
        "invalidation_conditions": "callsite, dispatch model, or proof policy changes",
        "status": "admitted",
        "evidence_use": "proof_only"
    })
}

#[test]
fn proof_graph_store_accepts_runtime_dispatch_summary_artifacts() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(runtime_dispatch_summary_record());
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("runtime-dispatch-summary:call:spawn")
        .expect("runtime dispatch summary proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "runtime_dispatch_summary"
                && row.fact_id == "runtime-dispatch-summary:call:spawn"
                && row.build_domain_id.as_deref() == Some("bd:main")
                && row.call_site_id.as_deref() == Some("call:spawn")
                && row.summary_class.as_deref() == Some("allowed_only_under_containment")
                && row.artifact_hash.as_deref() == Some("sha256:runtime-dispatch-summary")
                && row.summary_version.as_deref() == Some("runtime-dispatch-summary-v1")
                && row.review_method.as_deref() == Some("manual-review")
                && row.scope_of_validity.as_deref()
                    == Some("call:spawn runtime dispatch frontier under bd:main")
                && row.required_containment.as_deref()
                    == Some("summary does not create local traversal edges")
                && row.invalidation_conditions.as_deref()
                    == Some("callsite, dispatch model, or proof policy changes")
                && row.status.as_deref() == Some("admitted")
                && row.detail.as_deref() == Some("allowed_only_under_containment")
        }),
        "GraphRAG proof lookup should expose stored runtime dispatch summaries: {rows:#?}"
    );
}

#[test]
fn proof_graph_store_rejects_runtime_dispatch_summary_without_call_site() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut summary = runtime_dispatch_summary_record();
    summary
        .as_object_mut()
        .expect("runtime dispatch summary object")
        .remove("call_site_id");
    records.push(summary);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("runtime_dispatch_summary without call_site_id should reject the batch");
    assert!(error.to_string().contains("call_site_id"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graph_store_rejects_admitted_opaque_runtime_dispatch_summary() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut summary = runtime_dispatch_summary_record();
    summary["summary_class"] = json!("opaque_blocked");
    records.push(summary);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("admitted opaque runtime_dispatch_summary should reject the batch");
    assert!(
        error
            .to_string()
            .contains("runtime_dispatch_summary with summary_class opaque_blocked")
    );
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}
