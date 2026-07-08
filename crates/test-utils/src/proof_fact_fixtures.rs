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

pub fn axum_dependency_record(
    domain_id: &str,
    call_site_id: Uuid,
    caller_def_id: Uuid,
    resolved_def_id: Uuid,
) -> serde_json::Value {
    serde_json::json!({
        "fact_kind": "dependency_root",
        "schema_version": "ploke-proof-facts.v1",
        "dependency_root_id": format!("dependency-root:axum-core-from-ref:{call_site_id}"),
        "build_domain_id": domain_id,
        "call_site_id": call_site_id.to_string(),
        "caller_def_id": caller_def_id.to_string(),
        "resolved_def_id": resolved_def_id.to_string(),
        "dependency_name": "axum_core",
        "target_kind": "workspace_trait_method",
        "target_name": "axum_core::extract::FromRef::from_ref",
        "target_root": "axum-core/src/extract/from_ref.rs",
        "import_path": ["axum_core", "extract", "FromRef"],
        "resolved_path": ["axum_core", "extract", "FromRef", "from_ref"],
        "artifact_hash": "sha256:axum-core-from-ref-dependency-root",
        "version": "axum-call-graph-dependency-root-v1",
        "review_method": "source-oracle-review",
        "scope_of_validity": "axum dependency-root FromRef::from_ref source oracle in corpus_axum_call_graph",
        "status": "admitted",
        "evidence_use": "proof_only"
    })
}

pub fn axum_parts_blocker(call_site_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "fact_kind": "proof_blocker",
        "schema_version": "ploke-proof-facts.v1",
        "blocker_id": format!("blocker:axum-request-parts-into-parts:{call_site_id}"),
        "reason": "external_dependency_summary_missing",
        "status": "blocked",
        "call_site_id": call_site_id.to_string(),
        "detail": "axum-core/src/ext_traits/request_parts.rs:164 requires a summary for http::Request::into_parts returning http::request::Parts before the turbofish receiver can resolve",
        "evidence_use": "proof_only"
    })
}
