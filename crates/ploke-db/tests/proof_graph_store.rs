use ploke_db::{Database, ProofGraphContextRow, ProofGraphStore};
use serde_json::json;

const PROOF_FACT_SCHEMA_VERSION: &str = "ploke-proof-facts.v1";

fn proof_records() -> Vec<serde_json::Value> {
    vec![
        build_domain_record(),
        admitted_cfg_domain_record(),
        admitted_rustc_invocation_record(),
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

fn proof_record_by_kind_mut<'a>(
    records: &'a mut [serde_json::Value],
    kind: &str,
) -> &'a mut serde_json::Value {
    records
        .iter_mut()
        .find(|record| record.get("fact_kind").and_then(serde_json::Value::as_str) == Some(kind))
        .unwrap_or_else(|| panic!("missing proof record kind {kind}"))
}

fn build_domain_record() -> serde_json::Value {
    json!({
        "fact_kind": "build_domain",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "build_domain_id": "bd:main",
        "cargo_metadata_hash": "sha256:metadata",
        "cargo_lock_hash": "sha256:lock",
        "package_id": "ploke 0.1.0",
        "target_kind": "library",
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
    })
}

fn build_domain_record_for(build_domain_id: &str) -> serde_json::Value {
    let mut value = build_domain_record();
    value["build_domain_id"] = json!(build_domain_id);
    value
}

fn cfg_domain_record() -> serde_json::Value {
    json!({
        "fact_kind": "cfg_domain",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "cfg_domain_id": "cfg:main",
        "build_domain_id": "bd:main",
        "active_cfg_hash": "sha256:cfg",
        "status": "blocked",
        "blocking_reason": "cfg_domain_not_materialized"
    })
}

fn admitted_cfg_domain_record() -> serde_json::Value {
    let mut value = cfg_domain_record();
    value["status"] = json!("admitted");
    value
        .as_object_mut()
        .expect("cfg domain object")
        .remove("blocking_reason");
    value
}

fn rustc_invocation_record() -> serde_json::Value {
    json!({
        "fact_kind": "rustc_invocation",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "invocation_id": "rustc:main",
        "build_domain_id": "bd:main",
        "rustc_program": "rustc",
        "rustc_version": "rustc 1.96.0",
        "working_directory": "/workspace/ploke",
        "argument_vector_hash": "sha256:argv",
        "environment_hash": "sha256:env",
        "status": "blocked",
        "blocking_reason": "rustc_invocation_evidence_missing"
    })
}

fn admitted_rustc_invocation_record() -> serde_json::Value {
    let mut value = rustc_invocation_record();
    value["status"] = json!("admitted");
    value
        .as_object_mut()
        .expect("rustc invocation object")
        .remove("blocking_reason");
    value
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

fn expanded_item_record() -> serde_json::Value {
    json!({
        "fact_kind": "expanded_item",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "expanded_item_id": "expanded:item:macro",
        "boundary_id": "boundary:macro-rules",
        "build_domain_id": "bd:main",
        "definition_id": "def:expanded-macro-item",
        "source_span": {
            "file": "src/lib.rs",
            "start_byte": 90,
            "end_byte": 120,
            "line_start": 9,
            "line_end": 10
        },
        "evidence_use": "proof_only"
    })
}

fn externally_summarized_boundary(summary_id: Option<&str>) -> serde_json::Value {
    let mut value = expansion_boundary_record();
    value["boundary_id"] = json!("boundary:external-summary");
    value["boundary_kind"] = json!("external_summary");
    value["expansion_state"] = json!("externally_summarized");
    if let Some(summary_id) = summary_id {
        value["external_summary_id"] = json!(summary_id);
    }
    value
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

fn external_call_site_record() -> serde_json::Value {
    json!({
        "fact_kind": "call_site",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_site_id": "call:external",
        "build_domain_id": "bd:main",
        "caller_def_id": "def:launch",
        "source_span": {
            "file": "src/lib.rs",
            "start_byte": 100,
            "end_byte": 110
        },
        "evidence_use": "proof_only"
    })
}

fn external_summary_record() -> serde_json::Value {
    json!({
        "fact_kind": "external_summary",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "external_summary_id": "external-summary:dep:serde",
        "build_domain_id": "bd:main",
        "summary_class": "opaque_blocked",
        "artifact_hash": "sha256:serde-artifact",
        "version": "serde 1.0.0",
        "review_method": "manual-review",
        "scope_of_validity": "dependency serde under bd:main",
        "allowed_effects": ["external_summary_boundary"],
        "required_containment": "none",
        "invalidation_conditions": "artifact hash or proof policy changes",
        "status": "blocked",
        "evidence_use": "proof_only"
    })
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
fn proof_graph_store_accepts_external_summary_artifacts() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(external_summary_record());
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let by_id = db
        .proof_graphrag_context("external-summary:dep:serde")
        .expect("external summary proof context");
    assert!(
        by_id.iter().any(|row| {
            row.kind == "external_summary"
                && row.fact_id == "external-summary:dep:serde"
                && row.build_domain_id.as_deref() == Some("bd:main")
                && row.summary_class.as_deref() == Some("opaque_blocked")
                && row.artifact_hash.as_deref() == Some("sha256:serde-artifact")
                && row.summary_version.as_deref() == Some("serde 1.0.0")
                && row.review_method.as_deref() == Some("manual-review")
                && row.scope_of_validity.as_deref() == Some("dependency serde under bd:main")
                && row.allowed_effects == vec!["external_summary_boundary".to_string()]
                && row.required_containment.as_deref() == Some("none")
                && row.invalidation_conditions.as_deref()
                    == Some("artifact hash or proof policy changes")
                && row.status.as_deref() == Some("blocked")
                && row.detail.as_deref() == Some("opaque_blocked")
        }),
        "GraphRAG proof lookup should expose stored external summary artifacts: {by_id:#?}"
    );

    let by_class = db
        .proof_graphrag_context("opaque_blocked")
        .expect("external summary class proof context");
    assert!(
        by_class.iter().any(
            |row| row.kind == "external_summary" && row.fact_id == "external-summary:dep:serde"
        ),
        "GraphRAG proof lookup should match JSON-only external summary class fields: {by_class:#?}"
    );

    let by_hash = db
        .proof_graphrag_context("sha256:serde-artifact")
        .expect("external summary artifact hash context");
    assert!(
        by_hash.iter().any(
            |row| row.kind == "external_summary" && row.fact_id == "external-summary:dep:serde"
        ),
        "GraphRAG proof lookup should match external summary artifact hashes: {by_hash:#?}"
    );
}

#[test]
fn proof_graphrag_context_links_external_summary_artifacts_and_resolution_refs() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(externally_summarized_resolution(Some(
        "external-summary:dep:serde",
    )));
    records.push(external_summary_record());
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("external-summary:dep:serde")
        .expect("external summary proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "external_summary" && row.fact_id == "external-summary:dep:serde"
        }),
        "summary-id lookup should include the stored external summary artifact: {rows:#?}"
    );
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some("call:external")
                && row.external_summary_id.as_deref() == Some("external-summary:dep:serde")
        }),
        "summary-id lookup should include call_resolution rows that reference the artifact: {rows:#?}"
    );
}

#[test]
fn proof_graph_store_rejects_external_summary_without_artifact_identity() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut summary = external_summary_record();
    summary
        .as_object_mut()
        .expect("external summary object")
        .remove("artifact_hash");
    records.push(summary);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("external_summary without artifact_hash should reject the whole batch");
    assert!(error.to_string().contains("artifact_hash"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graphrag_context_exposes_build_domain_metadata() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    db.upsert_proof_fact_values(&proof_records())
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("proof-policy-test")
        .expect("build-domain proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "build_domain"
                && row.fact_id == "bd:main"
                && row.build_domain_id.as_deref() == Some("bd:main")
                && row.target_kind.as_deref() == Some("library")
                && row.target_name.as_deref() == Some("ploke")
                && row.target_root.as_deref() == Some("src/lib.rs")
                && row.profile.as_deref() == Some("dev")
                && row.rustc_version.as_deref() == Some("rustc 1.96.0")
                && row.proof_policy_version.as_deref() == Some("proof-policy-test")
        }),
        "GraphRAG proof lookup should expose build-domain metadata: {rows:#?}"
    );
}

#[test]
fn proof_graphrag_context_exposes_cfg_and_rustc_metadata() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(cfg_domain_record());
    records.push(rustc_invocation_record());
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let cfg_rows = db
        .proof_graphrag_context("sha256:cfg")
        .expect("cfg-domain proof context");
    assert!(
        cfg_rows.iter().any(|row| {
            row.kind == "cfg_domain"
                && row.fact_id == "cfg:main"
                && row.cfg_domain_id.as_deref() == Some("cfg:main")
                && row.build_domain_id.as_deref() == Some("bd:main")
                && row.active_cfg_hash.as_deref() == Some("sha256:cfg")
                && row.status.as_deref() == Some("blocked")
                && row.blocker_reason.as_deref() == Some("cfg_domain_not_materialized")
        }),
        "GraphRAG proof lookup should expose cfg-domain metadata: {cfg_rows:#?}"
    );

    let rustc_rows = db
        .proof_graphrag_context("sha256:argv")
        .expect("rustc-invocation proof context");
    assert!(
        rustc_rows.iter().any(|row| {
            row.kind == "rustc_invocation"
                && row.fact_id == "rustc:main"
                && row.invocation_id.as_deref() == Some("rustc:main")
                && row.build_domain_id.as_deref() == Some("bd:main")
                && row.rustc_program.as_deref() == Some("rustc")
                && row.working_directory.as_deref() == Some("/workspace/ploke")
                && row.argument_vector_hash.as_deref() == Some("sha256:argv")
                && row.environment_hash.as_deref() == Some("sha256:env")
                && row.rustc_version.as_deref() == Some("rustc 1.96.0")
                && row.status.as_deref() == Some("blocked")
                && row.blocker_reason.as_deref() == Some("rustc_invocation_evidence_missing")
        }),
        "GraphRAG proof lookup should expose rustc-invocation metadata: {rustc_rows:#?}"
    );
}

#[test]
fn proof_graph_store_rejects_external_summary_boundary_without_summary_id() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(externally_summarized_boundary(None));

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("externally_summarized expansion_boundary should require external_summary_id");
    assert!(error.to_string().contains("external_summary_id"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
}

#[test]
fn proof_graphrag_context_exposes_external_summary_boundary_ids() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(externally_summarized_boundary(Some(
        "external-summary:macro:serde",
    )));
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("external-summary:macro:serde")
        .expect("boundary summary proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "expansion_boundary"
                && row.fact_id == "boundary:external-summary"
                && row.status.as_deref() == Some("externally_summarized")
                && row.detail.as_deref() == Some("external_summary")
                && row.external_summary_id.as_deref() == Some("external-summary:macro:serde")
        }),
        "GraphRAG proof lookup should expose external_summary_id for externally_summarized expansion boundaries: {rows:#?}"
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
fn proof_blockers_include_derived_summary_and_resolution_gaps() {
    struct Case<'a> {
        blocker_id: &'a str,
        reason: &'a str,
        status: &'a str,
        build_domain_id: Option<&'a str>,
        call_site_id: Option<&'a str>,
        detail: &'a str,
    }

    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut boundary = externally_summarized_boundary(Some("external-summary:dep:serde"));
    boundary
        .as_object_mut()
        .expect("boundary object")
        .remove("blocking_reason");
    let records = vec![
        build_domain_record(),
        externally_summarized_resolution(Some("external-summary:dep:serde")),
        boundary,
        external_summary_record(),
    ];
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let blockers = db.proof_blockers().expect("blocker inspection");
    let cases = [
        Case {
            blocker_id: "boundary:external-summary",
            reason: "external_dependency_summary_missing",
            status: "externally_summarized",
            build_domain_id: Some("bd:main"),
            call_site_id: None,
            detail: "external_summary",
        },
        Case {
            blocker_id: "external-summary:dep:serde",
            reason: "opaque_blocked",
            status: "blocked",
            build_domain_id: Some("bd:main"),
            call_site_id: None,
            detail: "opaque_blocked",
        },
        Case {
            blocker_id: "resolution:call:external",
            reason: "external_dependency_summary_missing",
            status: "externally_summarized",
            build_domain_id: None,
            call_site_id: Some("call:external"),
            detail: "external_dependency_summary_missing",
        },
    ];

    assert_eq!(
        blockers.len(),
        cases.len(),
        "derived proof blockers: {blockers:#?}"
    );
    for case in cases {
        let row = blockers
            .iter()
            .find(|row| row.blocker_id == case.blocker_id)
            .unwrap_or_else(|| panic!("missing blocker {} in {blockers:#?}", case.blocker_id));
        assert_eq!(row.reason, case.reason, "{}", case.blocker_id);
        assert_eq!(row.status, case.status, "{}", case.blocker_id);
        assert_eq!(
            row.build_domain_id.as_deref(),
            case.build_domain_id,
            "{}",
            case.blocker_id
        );
        assert_eq!(
            row.call_site_id.as_deref(),
            case.call_site_id,
            "{}",
            case.blocker_id
        );
        assert_eq!(row.detail, case.detail, "{}", case.blocker_id);
    }
}

#[test]
fn proof_blockers_discharge_linked_admitted_external_summary_artifacts() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut resolution = externally_summarized_resolution(Some("external-summary:dep:serde"));
    resolution
        .as_object_mut()
        .expect("resolution object")
        .remove("blocking_reason");
    let mut boundary = externally_summarized_boundary(Some("external-summary:dep:serde"));
    boundary
        .as_object_mut()
        .expect("boundary object")
        .remove("blocking_reason");
    let mut summary = external_summary_record();
    summary["summary_class"] = json!("audited_no_process_effects");
    summary["status"] = json!("admitted");
    db.upsert_proof_fact_values(&[
        build_domain_record(),
        external_call_site_record(),
        resolution,
        boundary,
        summary,
    ])
    .expect("import admitted external summary proof facts");

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert!(
        blockers.is_empty(),
        "admitted linked external summary should discharge derived missing-summary blockers: {blockers:#?}"
    );

    let rows = db
        .proof_graphrag_context("external-summary:dep:serde")
        .expect("external summary proof context");
    assert!(
        rows.iter()
            .filter(|row| { row.kind == "call_resolution" || row.kind == "expansion_boundary" })
            .all(|row| row.blocker_reason.is_none()),
        "admitted linked external summary should clear derived context blockers: {rows:#?}"
    );
}

#[test]
fn proof_blockers_require_linked_call_site_domain_for_external_summary_discharge() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut resolution = externally_summarized_resolution(Some("external-summary:dep:serde"));
    resolution
        .as_object_mut()
        .expect("resolution object")
        .remove("blocking_reason");
    let mut summary = external_summary_record();
    summary["summary_class"] = json!("audited_no_process_effects");
    summary["status"] = json!("admitted");
    db.upsert_proof_fact_values(&[build_domain_record(), resolution, summary])
        .expect("import unscoped external summary proof facts");

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert_eq!(
        blockers.len(),
        1,
        "call-site scoped resolution without a linked call_site domain should fail closed: {blockers:#?}"
    );
    assert_eq!(blockers[0].blocker_id, "resolution:call:external");
    assert_eq!(blockers[0].reason, "external_dependency_summary_missing");

    let rows = db
        .proof_graphrag_context("external-summary:dep:serde")
        .expect("external summary proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_resolution"
                && row.call_site_id.as_deref() == Some("call:external")
                && row.blocker_reason.as_deref() == Some("external_dependency_summary_missing")
        }),
        "context lookup should retain the missing-summary blocker when call_site scope is absent: {rows:#?}"
    );
}

#[test]
fn proof_blockers_require_allowed_effect_for_external_summary_discharge() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut resolution = externally_summarized_resolution(Some("external-summary:dep:serde"));
    resolution
        .as_object_mut()
        .expect("resolution object")
        .remove("blocking_reason");
    let mut boundary = externally_summarized_boundary(Some("external-summary:dep:serde"));
    boundary
        .as_object_mut()
        .expect("boundary object")
        .remove("blocking_reason");
    let mut summary = external_summary_record();
    summary["summary_class"] = json!("audited_no_process_effects");
    summary["status"] = json!("admitted");
    summary["allowed_effects"] = json!(["durable_evidence_read"]);
    db.upsert_proof_fact_values(&[build_domain_record(), resolution, boundary, summary])
        .expect("import admitted external summary proof facts");

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert_eq!(
        blockers.len(),
        2,
        "admitted external summary without external-summary authority should fail closed: {blockers:#?}"
    );
    assert!(
        blockers
            .iter()
            .all(|row| row.reason == "external_dependency_summary_missing"),
        "missing allowed external-summary effect should preserve missing-summary blockers: {blockers:#?}"
    );
}

#[test]
fn proof_blockers_require_call_site_domain_match_for_external_summary_discharge() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut resolution = externally_summarized_resolution(Some("external-summary:dep:serde"));
    resolution
        .as_object_mut()
        .expect("resolution object")
        .remove("blocking_reason");
    let mut summary = external_summary_record();
    summary["summary_class"] = json!("audited_no_process_effects");
    summary["status"] = json!("admitted");
    summary["build_domain_id"] = json!("bd:other");
    db.upsert_proof_fact_values(&[
        build_domain_record(),
        build_domain_record_for("bd:other"),
        external_call_site_record(),
        resolution,
        summary,
    ])
    .expect("import mismatched external summary proof facts");

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert_eq!(
        blockers.len(),
        1,
        "external summary in a different build domain should not discharge call-site scoped resolution blockers: {blockers:#?}"
    );
    assert_eq!(blockers[0].blocker_id, "resolution:call:external");
    assert_eq!(blockers[0].reason, "external_dependency_summary_missing");
}

#[test]
fn proof_blockers_keep_macro_and_build_summary_gaps_without_specific_semantics() {
    for (boundary_kind, expected_reason) in [
        ("proc_macro_function", "proc_macro_summary_missing"),
        ("build_script", "build_script_summary_missing"),
    ] {
        let db = Database::new_init().expect("create db");
        db.ensure_proof_graph_schema().expect("proof graph schema");
        let mut boundary = externally_summarized_boundary(Some("external-summary:dep:serde"));
        boundary["boundary_kind"] = json!(boundary_kind);
        boundary
            .as_object_mut()
            .expect("boundary object")
            .remove("blocking_reason");
        let mut summary = external_summary_record();
        summary["summary_class"] = json!("audited_no_process_effects");
        summary["status"] = json!("admitted");
        db.upsert_proof_fact_values(&[build_domain_record(), boundary, summary])
            .expect("import admitted external summary proof facts");

        let blockers = db.proof_blockers().expect("blocker inspection");
        assert_eq!(
            blockers.len(),
            1,
            "{boundary_kind} should not be discharged by generic external-summary semantics: {blockers:#?}"
        );
        assert_eq!(blockers[0].blocker_id, "boundary:external-summary");
        assert_eq!(blockers[0].reason, expected_reason);
    }
}

#[test]
fn proof_context_rows_include_derived_blocker_reasons() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    db.upsert_proof_fact_values(&proof_records())
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("def:launch")
        .expect("graphrag proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "call_edge"
                && row.call_site_id.as_deref() == Some("call:spawn")
                && row.blocker_reason.as_deref() == Some("type_resolution_missing")
        }),
        "derived unresolved call-edge blocker should be visible in proof context rows: {rows:#?}"
    );
}

#[test]
fn proof_graphrag_context_exposes_authority_terms() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    db.upsert_proof_fact_values(&[json!({
        "fact_kind": "authority",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "authority_fact_id": "authority:handoff:successor",
        "build_domain_id": "bd:main",
        "authority_term": "successor",
        "status": "admitted",
        "evidence_use": "proof_only",
        "source_span": {
            "file": "src/lib.rs",
            "start_byte": 50,
            "end_byte": 60
        }
    })])
    .expect("import authority proof fact");

    let rows = db
        .proof_graphrag_context("authority:handoff")
        .expect("authority proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "authority"
                && row.authority_term.as_deref() == Some("successor")
                && row.status.as_deref() == Some("admitted")
                && row.effect_class.as_deref() == Some("successor")
        }),
        "proof context should expose authority_term without losing existing checker field: {rows:#?}"
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
        checker_edges
            .iter()
            .any(|edge| edge.call_site_id == "call:spawn"
                && edge.blocker_reason.as_deref() == Some("type_resolution_missing")),
        "checker edges should expose derived unresolved call-edge blockers: {checker_edges:#?}"
    );
    assert!(
        checker_edges.iter().any(
            |edge| edge.call_site_id == "call:helper" && edge.evidence_use == "navigation_only"
        )
    );

    let blockers = db.proof_blockers().expect("blocker inspection");
    assert_eq!(
        blockers.len(),
        2,
        "explicit and derived blockers should be inspectable: {blockers:#?}"
    );
    assert!(
        blockers.iter().any(|row| {
            row.call_site_id.as_deref() == Some("call:spawn")
                && row.reason == "process_lifetime_evidence_missing"
        }),
        "explicit blocker should remain inspectable: {blockers:#?}"
    );
    assert!(
        blockers.iter().any(|row| {
            row.call_site_id.as_deref() == Some("call:spawn")
                && row.reason == "type_resolution_missing"
        }),
        "derived unresolved call-edge blocker should be inspectable: {blockers:#?}"
    );

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
                && row.boundary_id.as_deref() == Some("boundary:macro-rules")
                && row.boundary_kind.as_deref() == Some("macro_rules_invocation")
                && row.status.as_deref() == Some("unresolved")
                && row.blocker_reason.as_deref() == Some("macro_expansion_not_available")
                && row.detail.as_deref() == Some("macro_rules_invocation")
        }),
        "GraphRAG proof lookup should match JSON-only boundary_kind fields: {rows:#?}"
    );
}

#[test]
fn proof_graphrag_context_exposes_expanded_item_metadata() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    records.push(expansion_boundary_record());
    records.push(expanded_item_record());
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");

    let rows = db
        .proof_graphrag_context("def:expanded-macro-item")
        .expect("expanded-item proof context");
    assert!(
        rows.iter().any(|row| {
            row.kind == "expanded_item"
                && row.fact_id == "expanded:item:macro"
                && row.expanded_item_id.as_deref() == Some("expanded:item:macro")
                && row.boundary_id.as_deref() == Some("boundary:macro-rules")
                && row.definition_id.as_deref() == Some("def:expanded-macro-item")
        }),
        "GraphRAG proof lookup should expose expanded-item linkage metadata: {rows:#?}"
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
                && hit.effect_seed_id.as_deref() == Some("effect:spawn")
                && hit.call_site_id.as_deref() == Some("call:spawn")
                && hit.effect_class.as_deref() == Some("operating_system_process_create")
                && hit.confidence.as_deref() == Some("command-spawn")
                && hit.blocker_if_unresolved == Some(true)
        }),
        "build-domain context should expose linked effect seed metadata: {rows:#?}"
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
    proof_record_by_kind_mut(&mut records, "call_site")["schema_version"] =
        json!("stale-proof-facts.v0");

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
    proof_record_by_kind_mut(&mut records, "call_edge")["schema_version"] =
        json!("stale-proof-facts.v0");

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
    proof_record_by_kind_mut(&mut records, "call_edge")
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
    let source_span = proof_record_by_kind_mut(&mut records, "call_site")
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
}

#[test]
fn proof_graph_store_rejects_invalid_enum_like_fields_before_storage() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    proof_record_by_kind_mut(&mut records, "call_site")["evidence_use"] =
        json!("proof-and-navigation");

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
    proof_record_by_kind_mut(&mut effect_records, "effect_seed")["effect_class"] =
        json!("shell_command");
    let effect_error = db
        .upsert_proof_fact_values(&effect_records)
        .expect_err("non-schema effect_class alias should be rejected before storage");
    assert!(effect_error.to_string().contains("effect_class"));

    let mut summary_records = proof_records();
    let mut summary = external_summary_record();
    summary["summary_class"] = json!("trusted_common_dependency");
    summary_records.push(summary);
    let summary_error = db
        .upsert_proof_fact_values(&summary_records)
        .expect_err("non-schema summary_class alias should be rejected before storage");
    assert!(summary_error.to_string().contains("summary_class"));

    let mut allowed_effect_records = proof_records();
    let mut summary = external_summary_record();
    summary["allowed_effects"] = json!(["shell_command"]);
    allowed_effect_records.push(summary);
    let allowed_effect_error = db
        .upsert_proof_fact_values(&allowed_effect_records)
        .expect_err("non-schema allowed_effects value should be rejected before storage");
    assert!(allowed_effect_error.to_string().contains("allowed_effects"));
}

#[test]
fn proof_graph_store_rejects_admitted_opaque_external_summary() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut records = proof_records();
    let mut summary = external_summary_record();
    summary["status"] = json!("admitted");
    summary["summary_class"] = json!("opaque_blocked");
    records.push(summary);

    let error = db
        .upsert_proof_fact_values(&records)
        .expect_err("opaque_blocked external summaries cannot be admitted");
    assert!(error.to_string().contains("opaque_blocked"));
    assert!(
        db.proof_graphrag_context("")
            .expect("query graph")
            .is_empty()
    );
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
        let effect_seed = proof_record_by_kind_mut(&mut records, "effect_seed");
        effect_seed["effect_seed_id"] = json!(format!("effect:{effect_class}"));
        effect_seed["effect_class"] = json!(effect_class);

        db.upsert_proof_fact_values(&records)
            .unwrap_or_else(|error| {
                panic!("effect_class {effect_class} should be accepted: {error}")
            });
    }
}
