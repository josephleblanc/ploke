use ploke_db::{Database, ProofGraphContextRow, ProofGraphStore};
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

fn expansion_boundary_record() -> serde_json::Value {
    json!({
        "fact_kind": "expansion_boundary",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "boundary_id": "boundary:macro-rules",
        "build_domain_id": "bd:main",
        "boundary_kind": "macro_rules_invocation",
        "source_span": {
            "file": "src/lib.rs",
            "start_byte": 70,
            "end_byte": 90,
            "line_start": 8,
            "line_end": 8
        },
        "expansion_state": "unresolved",
        "blocking_reason": "macro_expansion_not_available"
    })
}

fn externally_summarized_resolution(summary_id: Option<&str>) -> serde_json::Value {
    let mut value = json!({
        "fact_kind": "call_resolution",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_site_id": "call:external",
        "resolution_state": "externally_summarized",
        "blocking_reason": "external_dependency_summary_missing"
    });
    if let Some(summary_id) = summary_id {
        value["external_summary_id"] = json!(summary_id);
    }
    value
}

fn assert_process_context(label: &str, hits: &[ProofGraphContextRow]) {
    assert!(
        hits.iter()
            .any(|hit| hit.call_site_id.as_deref() == Some("call:spawn")),
        "{label} should include the process-spawn call site: {hits:#?}"
    );
    assert!(
        hits.iter().any(|hit| {
            hit.blocker_reason.as_deref() == Some("process_lifetime_evidence_missing")
        }),
        "{label} should include the linked process-lifetime blocker: {hits:#?}"
    );
    assert!(
        hits.iter()
            .any(|hit| hit.evidence_use.as_deref() == Some("navigation_only")),
        "{label} should include linked navigation-only helper rows: {hits:#?}"
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
fn proof_graph_store_retains_blockers_for_graphrag_and_checker_queries() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    db.upsert_proof_fact_values(&proof_records())
        .expect("import proof facts");

    let graph_hits = db
        .proof_graphrag_context("launch")
        .expect("graphrag proof context");
    assert_process_context("graphrag context", &graph_hits);

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
    assert_process_context("symbol lookup", &lookup);
}

#[test]
fn proof_graphrag_context_matches_json_only_expansion_boundary_fields() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(expansion_boundary_record());
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("macro_rules_invocation")
        .expect("boundary-kind proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "expansion_boundary"
                && row.fact_id == "boundary:macro-rules"
                && row.status.as_deref() == Some("unresolved")
                && row.blocker_reason.as_deref() == Some("macro_expansion_not_available")
                && row.detail.as_deref() == Some("macro_rules_invocation")
        }),
        "GraphRAG proof lookup should match JSON-only boundary_kind fields: {rows:#?}"
    );
}

#[test]
fn proof_domain_context_keeps_linked_blockers_for_build_domain() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    db.upsert_proof_fact_values(&proof_records())
        .expect("import proof facts");

    let rows = db
        .proof_domain_context("bd:main")
        .expect("build-domain proof context");
    assert_eq!(
        rows.len(),
        proof_records().len(),
        "build-domain context should include direct facts and linked rows without repeating build_domain_id on every fact: {rows:#?}"
    );
    assert_process_context("build-domain context", &rows);
    assert!(
        rows.iter().any(|hit| {
            hit.kind == "call_site"
                && hit.call_site_id.as_deref() == Some("call:spawn")
                && hit.build_domain_id.as_deref() == Some("bd:main")
                && hit.start_byte == Some(10)
                && hit.end_byte == Some(20)
                && hit.line_end == Some(4)
        }),
        "build-domain context should expose matched call-site domain and span: {rows:#?}"
    );
    assert!(
        rows.iter().any(|hit| {
            hit.kind == "call_edge"
                && hit.call_site_id.as_deref() == Some("call:spawn")
                && hit.call_edge_id.as_deref() == Some("edge:spawn")
                && hit.resolution_state.as_deref() == Some("candidate_set")
        }),
        "build-domain context should expose linked call-edge identity and resolution state: {rows:#?}"
    );
    assert!(
        rows.iter().any(|hit| {
            hit.kind == "effect_seed"
                && hit.call_site_id.as_deref() == Some("call:spawn")
                && hit.effect_class.as_deref() == Some("operating_system_process_create")
        }),
        "build-domain context should expose linked effect seed classification: {rows:#?}"
    );
    assert!(
        rows.iter().any(|hit| {
            hit.kind == "proof_blocker"
                && hit.call_site_id.as_deref() == Some("call:spawn")
                && hit.status.as_deref() == Some("blocked")
        }),
        "build-domain context should expose linked blocker status: {rows:#?}"
    );
    assert!(
        db.proof_domain_context("bd:missing")
            .expect("missing build-domain context")
            .is_empty(),
        "unknown build domain should not match proof rows"
    );

    let error = db
        .proof_domain_context("")
        .expect_err("empty build-domain lookup should fail closed");
    assert!(
        error
            .to_string()
            .contains("requires non-empty build_domain_id"),
        "unexpected empty-domain error: {error}"
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

#[test]
fn proof_graph_store_rejects_invalid_enum_like_fields_before_storage() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records[0]["evidence_use"] = json!("proof-and-navigation");

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("invalid evidence_use spelling should be rejected before storage");
    assert!(error.to_string().contains("evidence_use"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );

    let mut effect_records = proof_records();
    effect_records[2]["effect_class"] = json!("shell_command");
    let effect_error = db
        .upsert_proof_fact_values(&effect_records)
        .expect_err("non-schema effect_class alias should be rejected before storage");
    assert!(effect_error.to_string().contains("effect_class"));
}

#[test]
fn proof_graph_store_accepts_all_stable_effect_class_values() {
    for effect_class in [
        "operating_system_process_create",
        "operating_system_process_replace",
        "operating_system_process_configure",
        "operating_system_process_wait",
        "operating_system_process_kill",
        "operating_system_process_reap",
        "async_task_spawn",
        "async_task_join",
        "async_task_abort",
        "authority_mint",
        "authority_retire",
        "authority_lock",
        "authority_unlock",
        "history_open_block",
        "history_seal_block",
        "history_append_block",
        "surface_measure",
        "surface_digest_compare",
        "durable_evidence_write",
        "durable_evidence_read",
        "external_summary_boundary",
    ] {
        let db = Database::new_init().expect("create db");
        db.ensure_proof_graph_schema().expect("proof graph schema");
        let mut records = proof_records();
        records[2]["effect_seed_id"] = json!(format!("effect:{effect_class}"));
        records[2]["effect_class"] = json!(effect_class);

        db.upsert_proof_fact_values(&records)
            .unwrap_or_else(|error| {
                panic!("effect_class {effect_class} should be accepted: {error}")
            });
    }
}
