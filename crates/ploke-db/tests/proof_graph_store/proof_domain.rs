use super::*;

#[test]
fn proof_graph_store_rejects_missing_build_and_effect_schema_fields() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = vec![json!({
        "fact_kind": "build_domain",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "build_domain_id": "bd:main",
        "cargo_metadata_hash": "sha256:metadata",
        "cargo_lock_hash": "sha256:lock",
        "package_id": "ploke 0.1.0",
        "target_kind": "lib",
        "target_name": "ploke",
        "target_root": "src/lib.rs",
        "target_triple": "x86_64-unknown-linux-gnu",
        "host_triple": "x86_64-unknown-linux-gnu",
        "profile": "dev",
        "features_hash": "sha256:features",
        "active_cfg_hash": "sha256:cfg",
        "rustc_version": "rustc 1.96.0",
        "extractor_version": "proof-graph-test",
        "proof_policy_version": "proof-policy-test"
    })];
    records[0]
        .as_object_mut()
        .expect("build domain object")
        .remove("target_root");

    let build_error = db
        .upsert_proof_fact_values(&records)
        .expect_err("build_domain missing target_root should be rejected");
    assert!(build_error.to_string().contains("target_root"));

    let mut effect_records = proof_records();
    proof_record_by_kind_mut(&mut effect_records, "effect_seed")
        .as_object_mut()
        .expect("effect seed object")
        .remove("blocker_if_unresolved");
    let effect_error = db
        .upsert_proof_fact_values(&effect_records)
        .expect_err("effect_seed missing blocker_if_unresolved should reject the whole batch");
    assert!(effect_error.to_string().contains("blocker_if_unresolved"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_blockers_report_missing_build_domain_references() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    db.upsert_proof_fact_values(&[external_call_site_record()])
        .expect("import call-site proof fact without build-domain fact");

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert_eq!(
        blockers.len(),
        1,
        "call-site proof facts with absent build-domain evidence should fail closed: {blockers:#?}"
    );
    assert_eq!(blockers[0].blocker_id, "call:external");
    assert_eq!(blockers[0].reason, "canonical_identity_mismatch");
    assert_eq!(blockers[0].build_domain_id.as_deref(), Some("bd:main"));
    assert_eq!(blockers[0].call_site_id.as_deref(), Some("call:external"));

    let rows = db
        .proof_graphrag_context("call:external")
        .expect("call-site proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_site"
                && row.call_site_id.as_deref() == Some("call:external")
                && row.blocker_reason.as_deref() == Some("canonical_identity_mismatch")
        }),
        "proof context should expose the missing build-domain blocker: {rows:#?}"
    );
}

#[test]
fn proof_blockers_report_incomplete_build_domain_evidence() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    db.upsert_proof_fact_values(&[build_domain_record()])
        .expect("import build-domain proof fact without cfg/rustc evidence");

    let blockers = db.proof_blockers().expect("blocker inspection");
    let reasons = blockers
        .iter()
        .map(|row| row.reason.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        reasons,
        std::collections::BTreeSet::from([
            "cfg_domain_not_materialized",
            "rustc_invocation_evidence_missing",
        ]),
        "build-domain proof facts without cfg and rustc evidence should fail closed: {blockers:#?}"
    );
    assert!(
        blockers.iter().all(|row| {
            row.blocker_id == "bd:main"
                && row.status == "blocked"
                && row.build_domain_id.as_deref() == Some("bd:main")
                && row.call_site_id.is_none()
        }),
        "build-domain blockers should stay scoped to the build domain: {blockers:#?}"
    );

    let context = db
        .proof_symbol_lookup("bd:main")
        .expect("build-domain proof context");
    let context_reasons = context
        .iter()
        .filter(|row| row.kind == "build_domain")
        .filter_map(|row| row.blocker_reason.as_deref())
        .collect::<std::collections::BTreeSet<_>>();
    assert_eq!(
        context_reasons, reasons,
        "proof context should expose every derived build-domain blocker: {context:#?}"
    );
}
