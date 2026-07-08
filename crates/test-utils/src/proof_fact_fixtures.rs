use uuid::Uuid;

const AXUM_CALL_GRAPH_DOMAIN_ID: &str = "bd:corpus-axum-call-graph";
pub const AXUM_OPAQUE_FUTURE_SUMMARY_ID: &str = "external-summary:axum-opaque-future-macro";

pub fn axum_opaque_future_boundary_id(call_site_id: Uuid) -> String {
    format!("boundary:{call_site_id}:opaque_future")
}

pub fn axum_opaque_future_macro_summary_records(call_site_id: Uuid) -> Vec<serde_json::Value> {
    let site = call_site_id.to_string();
    let boundary_id = axum_opaque_future_boundary_id(call_site_id);
    let mut records = axum_call_graph_domain_records(AXUM_CALL_GRAPH_DOMAIN_ID);
    records.extend([
        serde_json::json!({
            "fact_kind": "expansion_boundary",
            "schema_version": "ploke-proof-facts.v1",
            "boundary_id": boundary_id,
            "build_domain_id": AXUM_CALL_GRAPH_DOMAIN_ID,
            "call_site_id": site,
            "boundary_kind": "macro_rules_invocation",
            "expansion_state": "externally_summarized",
            "external_summary_id": AXUM_OPAQUE_FUTURE_SUMMARY_ID,
            "source_span": {
                "file": "axum/src/handler/future.rs",
                "start_byte": 285,
                "end_byte": 506
            },
            "evidence_use": "proof_only"
        }),
        serde_json::json!({
            "fact_kind": "external_summary",
            "schema_version": "ploke-proof-facts.v1",
            "external_summary_id": AXUM_OPAQUE_FUTURE_SUMMARY_ID,
            "build_domain_id": AXUM_CALL_GRAPH_DOMAIN_ID,
            "summary_class": "audited_no_process_effects",
            "artifact_hash": "sha256:axum-opaque-future-summary",
            "version": "axum-opaque-future-summary-v1",
            "review_method": "source-oracle-review",
            "scope_of_validity": "axum opaque_future macro boundary for IntoServiceFuture",
            "allowed_effects": ["external_summary_boundary"],
            "required_containment": "none",
            "invalidation_conditions": "source oracle, fixture hash, or proof policy changes",
            "status": "admitted",
            "evidence_use": "proof_only"
        }),
    ]);
    records
}

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

fn axum_call_graph_domain_records(domain_id: &str) -> Vec<serde_json::Value> {
    vec![
        serde_json::json!({
            "fact_kind": "build_domain",
            "schema_version": "ploke-proof-facts.v1",
            "build_domain_id": domain_id,
            "cargo_metadata_hash": "sha256:axum-metadata",
            "cargo_lock_hash": "sha256:axum-lock",
            "package_id": "github:tokio-rs/axum",
            "target_kind": "library",
            "target_name": "axum",
            "target_root": "axum/src/lib.rs",
            "target_triple": "x86_64-unknown-linux-gnu",
            "host_triple": "x86_64-unknown-linux-gnu",
            "profile": "dev",
            "features_hash": "sha256:axum-features",
            "active_cfg_hash": "sha256:axum-cfg",
            "rustc_version": "rustc fixture",
            "extractor_version": "ploke-test",
            "proof_policy_version": "proof-policy-test",
            "evidence_use": "proof_only"
        }),
        serde_json::json!({
            "fact_kind": "cfg_domain",
            "schema_version": "ploke-proof-facts.v1",
            "cfg_domain_id": "cfg:corpus-axum-call-graph",
            "build_domain_id": domain_id,
            "active_cfg_hash": "sha256:axum-cfg",
            "status": "admitted",
            "evidence_use": "proof_only"
        }),
        serde_json::json!({
            "fact_kind": "rustc_invocation",
            "schema_version": "ploke-proof-facts.v1",
            "invocation_id": "rustc:corpus-axum-call-graph",
            "build_domain_id": domain_id,
            "rustc_program": "rustc",
            "rustc_version": "rustc fixture",
            "working_directory": "/workspace/axum",
            "argument_vector_hash": "sha256:axum-rustc-argv",
            "environment_hash": "sha256:axum-rustc-env",
            "status": "admitted",
            "evidence_use": "proof_only"
        }),
    ]
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

pub fn axum_dyn_future_poll_blocker(call_site_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "fact_kind": "proof_blocker",
        "schema_version": "ploke-proof-facts.v1",
        "blocker_id": format!("blocker:axum-dyn-future-poll-runtime-dispatch:{call_site_id}"),
        "reason": "dynamic_dispatch_unbounded",
        "status": "blocked",
        "call_site_id": call_site_id.to_string(),
        "detail": "axum/src/error_handling/mod.rs:251 dyn Future::poll concrete runtime future unresolved",
        "evidence_use": "proof_only"
    })
}

pub fn axum_handler_async_block_poll_resume_blocker(
    call_site_id: Uuid,
    callee: &str,
) -> serde_json::Value {
    serde_json::json!({
        "fact_kind": "proof_blocker",
        "schema_version": "ploke-proof-facts.v1",
        "blocker_id": format!("blocker:axum-handler-async-block-poll-resume:{callee}:{call_site_id}"),
        "reason": "dynamic_dispatch_unbounded",
        "status": "blocked",
        "call_site_id": call_site_id.to_string(),
        "detail": format!("axum/src/handler/mod.rs:217 async-block `{callee}` remains targetless until callable binding and async poll/resume proof are modeled"),
        "evidence_use": "proof_only"
    })
}

pub fn fixture_extern_c_abs_effect_record(call_site_id: Uuid) -> serde_json::Value {
    serde_json::json!({
        "fact_kind": "effect_seed",
        "schema_version": "ploke-proof-facts.v1",
        "effect_seed_id": "effect:fixture-extern-c-abs",
        "call_site_id": call_site_id.to_string(),
        "effect_class": "ffi_boundary",
        "confidence": "fixture-source-oracle",
        "blocker_if_unresolved": true,
        "evidence_use": "proof_only"
    })
}
