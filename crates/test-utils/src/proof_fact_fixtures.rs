use uuid::Uuid;

pub fn axum_entrypoint_record(domain_id: &str, definition_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "fact_kind": "entrypoint_summary",
        "schema_version": "ploke-proof-facts.v1",
        "entrypoint_summary_id": "entrypoint-summary:axum-error-handling-traits-test",
        "build_domain_id": domain_id,
        "definition_id": definition_id.to_string(),
        "target_kind": "test",
        "target_name": "generated-test-harness",
        "summary_class": "analyzed_source",
        "artifact_hash": "sha256:axum-error-handling-traits-test-harness",
        "version": "axum-call-graph-test-entrypoint-v1",
        "review_method": "source-oracle-review",
        "scope_of_validity": "axum error_handling::traits generated #[test] harness in corpus_axum_call_graph",
        "required_containment": "rust-test-harness",
        "invalidation_conditions": "source oracle, fixture hash, or proof policy changes",
        "status": "admitted",
        "evidence_use": "proof_only"
    })
}
