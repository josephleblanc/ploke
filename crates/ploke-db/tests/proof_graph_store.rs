use ploke_db::{Database, ProofGraphStore};
use serde_json::json;

const PROOF_FACT_SCHEMA_VERSION: &str = "ploke-proof-facts.v1";

fn proof_records() -> Vec<serde_json::Value> {
    vec![
        json!({
            "fact_kind": "call_site",
            "schema_version": PROOF_FACT_SCHEMA_VERSION,
            "call_site_id": "call:spawn",
            "build_domain_id": "bd:main",
            "caller_def_id": "def:launch",
            "source_span": {
                "file": "src/lib.rs",
                "start_byte": 10,
                "end_byte": 20,
                "line_start": 4,
                "line_end": 4
            },
            "evidence_use": "proof_only"
        }),
        json!({
            "fact_kind": "call_edge",
            "schema_version": PROOF_FACT_SCHEMA_VERSION,
            "call_edge_id": "edge:spawn",
            "call_site_id": "call:spawn",
            "caller_def_id": "def:launch",
            "callee_def_id": null,
            "resolution_state": "candidate_set",
            "evidence_use": "proof_only"
        }),
        json!({
            "fact_kind": "effect_seed",
            "schema_version": PROOF_FACT_SCHEMA_VERSION,
            "effect_seed_id": "effect:spawn",
            "call_site_id": "call:spawn",
            "effect_class": "operating_system_process_create",
            "confidence": "command-spawn",
            "blocker_if_unresolved": true,
            "evidence_use": "proof_only"
        }),
        json!({
            "fact_kind": "proof_blocker",
            "schema_version": PROOF_FACT_SCHEMA_VERSION,
            "blocker_id": "blocker:spawn:lifetime",
            "reason": "process_lifetime_evidence_missing",
            "status": "blocked",
            "build_domain_id": "bd:main",
            "call_site_id": "call:spawn",
            "detail": "process create lacks handoff/lifetime evidence"
        }),
        json!({
            "fact_kind": "call_site",
            "schema_version": PROOF_FACT_SCHEMA_VERSION,
            "call_site_id": "call:helper",
            "build_domain_id": "bd:main",
            "caller_def_id": "def:launch",
            "source_span": {
                "file": "src/lib.rs",
                "start_byte": 30,
                "end_byte": 40,
                "line_start": 5,
                "line_end": 5
            },
            "evidence_use": "navigation_only"
        }),
        json!({
            "fact_kind": "call_edge",
            "schema_version": PROOF_FACT_SCHEMA_VERSION,
            "call_edge_id": "edge:helper",
            "call_site_id": "call:helper",
            "caller_def_id": "def:launch",
            "callee_def_id": null,
            "resolution_state": "unresolved",
            "evidence_use": "navigation_only"
        }),
    ]
}

#[test]
fn proof_graph_store_retains_blockers_for_graphrag_and_checker_queries() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    db.upsert_proof_fact_values(&proof_records())
        .expect("import proof facts");

    let graph_hits = db
        .proof_graphrag_context("launch")
        .expect("graphrag proof context");
    assert!(
        graph_hits
            .iter()
            .any(|hit| hit.call_site_id.as_deref() == Some("call:spawn"))
    );
    assert!(
        graph_hits
            .iter()
            .any(|hit| hit.blocker_reason.as_deref() == Some("process_lifetime_evidence_missing"))
    );
    assert!(
        graph_hits
            .iter()
            .any(|hit| hit.evidence_use.as_deref() == Some("navigation_only"))
    );

    let checker_edges = db.proof_checker_edges().expect("checker edges");
    assert!(
        checker_edges
            .iter()
            .any(|edge| edge.call_site_id == "call:spawn"
                && edge.blocker_reason.as_deref() == Some("process_lifetime_evidence_missing"))
    );
    assert!(
        checker_edges.iter().any(
            |edge| edge.call_site_id == "call:helper" && edge.evidence_use == "navigation_only"
        )
    );

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert_eq!(blockers.len(), 1);
    assert_eq!(blockers[0].call_site_id.as_deref(), Some("call:spawn"));

    let provenance = db
        .proof_source_provenance("call:spawn")
        .expect("source provenance")
        .expect("provenance row");
    assert_eq!(provenance.source_file, "src/lib.rs");
    assert_eq!(provenance.line_start, Some(4));
}

#[test]
fn proof_symbol_lookup_keeps_linked_blockers_with_matching_symbol() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    db.upsert_proof_fact_values(&proof_records())
        .expect("import proof facts");

    let lookup = db.proof_symbol_lookup("def:launch").expect("symbol lookup");
    assert!(
        lookup
            .iter()
            .any(|hit| hit.call_site_id.as_deref() == Some("call:spawn"))
    );
    assert!(
        lookup
            .iter()
            .any(|hit| hit.blocker_reason.as_deref() == Some("process_lifetime_evidence_missing"))
    );
}

#[test]
fn proof_checker_edges_retains_multiple_blockers_for_one_call_site() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(json!({
        "fact_kind": "proof_blocker",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "blocker_id": "blocker:spawn:identity",
        "reason": "canonical_identity_mismatch",
        "status": "blocked",
        "build_domain_id": "bd:main",
        "call_site_id": "call:spawn",
        "detail": "call-site identity does not match canonical source"
    }));
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let checker_edges = db.proof_checker_edges().expect("checker edges");
    let spawn_reasons = checker_edges
        .iter()
        .filter(|edge| edge.call_site_id == "call:spawn")
        .filter_map(|edge| edge.blocker_reason.as_deref())
        .collect::<std::collections::BTreeSet<_>>();
    assert!(spawn_reasons.contains("process_lifetime_evidence_missing"));
    assert!(spawn_reasons.contains("canonical_identity_mismatch"));
}

#[test]
fn proof_graph_store_rejects_schema_version_mismatch_before_storage() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records[0]["schema_version"] = json!("stale-proof-facts.v0");

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("stale schema should be rejected");
    assert!(error.to_string().contains("schema_version"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graph_store_prevalidates_batch_before_partial_storage() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records[1]["schema_version"] = json!("stale-proof-facts.v0");

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("stale schema should reject the whole batch");
    assert!(error.to_string().contains("schema_version"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graph_store_rejects_missing_kind_required_fields_before_storage() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records[1]
        .as_object_mut()
        .expect("call edge object")
        .remove("caller_def_id");

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("call edge without caller_def_id should reject the whole batch");
    assert!(error.to_string().contains("caller_def_id"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graph_store_accepts_source_spans_without_optional_line_numbers() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let source_span = records[0]
        .get_mut("source_span")
        .and_then(serde_json::Value::as_object_mut)
        .expect("call-site source span");
    source_span.remove("line_start");
    source_span.remove("line_end");

    db.upsert_proof_fact_values(&records)
        .expect("line numbers are optional source-span metadata");

    let provenance = db
        .proof_source_provenance("call:spawn")
        .expect("source provenance")
        .expect("stored source provenance");
    assert_eq!(provenance.source_file, "src/lib.rs");
    assert_eq!(provenance.line_start, None);
}

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
    effect_records[2]
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
