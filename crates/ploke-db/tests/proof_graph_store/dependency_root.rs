use super::*;

fn dependency_record(
    call_site_id: &str,
    caller_def_id: &str,
    resolved_def_id: &str,
) -> serde_json::Value {
    json!({
        "fact_kind": "dependency_root",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "dependency_root_id": format!("dependency-root:test:{call_site_id}"),
        "build_domain_id": "bd:main",
        "call_site_id": call_site_id,
        "caller_def_id": caller_def_id,
        "resolved_def_id": resolved_def_id,
        "dependency_name": "axum_core",
        "target_kind": "workspace_trait_method",
        "target_name": "axum_core::extract::FromRef::from_ref",
        "target_root": "axum-core/src/extract/from_ref.rs",
        "import_path": ["axum_core", "extract", "FromRef"],
        "resolved_path": ["axum_core", "extract", "FromRef", "from_ref"],
        "artifact_hash": "sha256:dependency-root-test",
        "version": "dependency-root-test-v1",
        "review_method": "source-oracle-review",
        "scope_of_validity": "parsed workspace dependency root import resolves to the target trait method",
        "status": "admitted",
        "evidence_use": "proof_only"
    })
}

#[test]
fn proof_graph_store_accepts_dependency_root_artifacts() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(dependency_record(
        "call:from-ref",
        "def:caller",
        "def:from-ref",
    ));
    db.upsert_proof_fact_values(&records)
        .expect("import dependency-root proof facts");

    let rows = db
        .proof_symbol_lookup("def:from-ref")
        .expect("target proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "dependency_root"
                && row.fact_id == "dependency-root:test:call:from-ref"
                && row.build_domain_id.as_deref() == Some("bd:main")
                && row.call_site_id.as_deref() == Some("call:from-ref")
                && row.caller_def_id.as_deref() == Some("def:caller")
                && row.resolved_def_id.as_deref() == Some("def:from-ref")
                && row.target_kind.as_deref() == Some("workspace_trait_method")
                && row.target_name.as_deref()
                    == Some("axum_core::extract::FromRef::from_ref")
                && row.target_root.as_deref() == Some("axum-core/src/extract/from_ref.rs")
                && row.artifact_hash.as_deref() == Some("sha256:dependency-root-test")
                && row.summary_version.as_deref() == Some("dependency-root-test-v1")
                && row.review_method.as_deref() == Some("source-oracle-review")
                && row.scope_of_validity.as_deref()
                    == Some(
                        "parsed workspace dependency root import resolves to the target trait method"
                    )
                && row.status.as_deref() == Some("admitted")
                && row.evidence_use.as_deref() == Some("proof_only")
        }),
        "target-centered proof lookup should expose dependency-root proof rows: {rows:#?}"
    );
}

#[test]
fn proof_graph_store_rejects_dependency_root_without_resolved_def() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut root = dependency_record("call:from-ref", "def:caller", "def:from-ref");
    root.as_object_mut()
        .expect("dependency root object")
        .remove("resolved_def_id");
    records.push(root);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("dependency_root without resolved_def_id should reject the whole batch");
    assert!(error.to_string().contains("resolved_def_id"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graph_store_rejects_dependency_root_without_import_path() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut root = dependency_record("call:from-ref", "def:caller", "def:from-ref");
    root.as_object_mut()
        .expect("dependency root object")
        .remove("import_path");
    records.push(root);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("dependency_root without import_path should reject the whole batch");
    assert!(error.to_string().contains("import_path"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}
