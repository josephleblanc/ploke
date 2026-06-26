use super::*;

fn proof_record(kind: &str) -> serde_json::Value {
    proof_records()
        .into_iter()
        .find(|record| record.get("fact_kind").and_then(serde_json::Value::as_str) == Some(kind))
        .unwrap_or_else(|| panic!("missing proof record kind {kind}"))
}

fn authority_record() -> serde_json::Value {
    serde_json::json!({
        "fact_kind": "authority",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "authority_fact_id": "authority:evidence",
        "build_domain_id": "bd:evidence",
        "authority_term": "successor",
        "status": "admitted",
        "source_span": {
            "file": "src/lib.rs",
            "start_byte": 40,
            "end_byte": 50
        },
        "evidence_use": "proof_only"
    })
}

fn fact_samples_with_evidence_use() -> Vec<(&'static str, serde_json::Value)> {
    vec![
        ("build_domain", build_domain_record()),
        ("cfg_domain", cfg_domain_record()),
        ("rustc_invocation", rustc_invocation_record()),
        ("expansion_boundary", expansion_boundary_record()),
        ("expanded_item", expanded_item_record()),
        ("call_site", proof_record("call_site")),
        ("call_edge", proof_record("call_edge")),
        (
            "call_resolution",
            externally_summarized_resolution(Some("external-summary:dep:serde")),
        ),
        ("external_summary", external_summary_record()),
        ("effect_seed", proof_record("effect_seed")),
        ("authority", authority_record()),
        ("proof_blocker", proof_record("proof_blocker")),
    ]
}

#[test]
fn proof_graph_store_rejects_missing_evidence_use_for_all_fact_kinds() {
    for (kind, mut fact) in fact_samples_with_evidence_use() {
        fact.as_object_mut()
            .expect("proof fact object")
            .remove("evidence_use")
            .unwrap_or_else(|| panic!("sample {kind} must start with evidence_use"));

        let db = Database::new_init().expect("create db");
        db.ensure_proof_graph_schema().expect("proof graph schema");
        let error = match db.upsert_proof_fact_values(&[fact]) {
            Ok(()) => panic!("{kind} without evidence_use should reject"),
            Err(error) => error,
        };
        assert!(
            error.to_string().contains("evidence_use"),
            "{kind} without evidence_use should report evidence_use, got {error}"
        );
        assert!(
            db.proof_graphrag_context("")
                .expect("query graph")
                .is_empty(),
            "{kind} rejection should not store partial proof rows"
        );
    }
}
