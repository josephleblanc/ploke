use std::collections::BTreeMap;

use cozo::{DataValue, JsonData, ScriptMutability};
use ploke_db::{Database, ProofGraphStore, ProofInvariantFinding, ProofInvariantStatus};
use serde_json::{Value, json};

const PROOF_FACT_SCHEMA_VERSION: &str = "ploke-proof-facts.v1";
const DETACHED_INVARIANT: &str = "detached_process_successor_handoff";
const CROWN_INVARIANT: &str = "crown_ruling_lineage_uniqueness";

fn call_site() -> Value {
    call_site_named("call:handoff-spawn", "bd:checker", "def:handoff", 4)
}

fn call_site_named(
    call_site_id: &str,
    build_domain_id: &str,
    caller_def_id: &str,
    line: u32,
) -> Value {
    json!({
        "fact_kind": "call_site",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_site_id": call_site_id,
        "build_domain_id": build_domain_id,
        "caller_def_id": caller_def_id,
        "source_span": {
            "file": "src/handoff.rs",
            "start_byte": line * 10,
            "end_byte": line * 10 + 20,
            "line_start": line,
            "line_end": line
        },
        "evidence_use": "proof_only"
    })
}

fn call_edge() -> Value {
    call_edge_named(
        "edge:handoff-spawn",
        "call:handoff-spawn",
        "def:handoff",
        "def:spawn-successor",
    )
}

fn call_edge_named(
    call_edge_id: &str,
    call_site_id: &str,
    caller_def_id: &str,
    callee_def_id: &str,
) -> Value {
    json!({
        "fact_kind": "call_edge",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_edge_id": call_edge_id,
        "call_site_id": call_site_id,
        "caller_def_id": caller_def_id,
        "callee_def_id": callee_def_id,
        "resolution_state": "resolved",
        "evidence_use": "proof_only"
    })
}

fn unresolved_call_edge_for(call_site_id: &str, evidence_use: &str) -> Value {
    json!({
        "fact_kind": "call_edge",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_edge_id": format!("edge:{call_site_id}:unresolved"),
        "call_site_id": call_site_id,
        "caller_def_id": "def:handoff",
        "resolution_state": "unresolved",
        "evidence_use": evidence_use
    })
}

fn call_resolution_for(call_site_id: &str, resolution_state: &str, reason: Option<&str>) -> Value {
    let mut value = json!({
        "fact_kind": "call_resolution",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_site_id": call_site_id,
        "resolution_state": resolution_state
    });
    if let Some(reason) = reason {
        value["blocking_reason"] = json!(reason);
    }
    if resolution_state == "externally_summarized" {
        value["external_summary_id"] = json!("external-summary:test");
    }
    value
}

fn process_effect() -> Value {
    process_effect_named("effect:handoff-spawn", "call:handoff-spawn")
}

fn process_effect_named(effect_seed_id: &str, call_site_id: &str) -> Value {
    json!({
        "fact_kind": "effect_seed",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "effect_seed_id": effect_seed_id,
        "call_site_id": call_site_id,
        "effect_class": "operating_system_process_create",
        "confidence": "command-spawn",
        "blocker_if_unresolved": true,
        "evidence_use": "proof_only"
    })
}

fn expansion_boundary(
    boundary_id: &str,
    build_domain_id: &str,
    boundary_kind: &str,
    expansion_state: &str,
    blocking_reason: Option<&str>,
    line: u32,
) -> Value {
    let mut value = json!({
        "fact_kind": "expansion_boundary",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "boundary_id": boundary_id,
        "build_domain_id": build_domain_id,
        "boundary_kind": boundary_kind,
        "source_span": {
            "file": "build.rs",
            "start_byte": line * 10,
            "end_byte": line * 10 + 5,
            "line_start": line,
            "line_end": line
        },
        "expansion_state": expansion_state
    });
    if let Some(blocking_reason) = blocking_reason {
        value["blocking_reason"] = json!(blocking_reason);
    }
    if expansion_state == "externally_summarized" {
        value["external_summary_id"] = json!("external-summary:test");
    }
    value
}

fn external_summary(status: &str, summary_class: &str) -> Value {
    json!({
        "fact_kind": "external_summary",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "external_summary_id": format!("external-summary:{status}:{summary_class}"),
        "build_domain_id": "bd:checker",
        "summary_class": summary_class,
        "artifact_hash": format!("sha256:{status}-{summary_class}"),
        "version": "external 1.0.0",
        "review_method": "manual-review",
        "scope_of_validity": "proof invariant fixture",
        "allowed_effects": ["external_summary_boundary"],
        "required_containment": "none",
        "invalidation_conditions": "artifact hash or proof policy changes",
        "status": status,
        "evidence_use": "proof_only"
    })
}

fn cfg_domain(build_domain_id: &str, status: &str, blocking_reason: Option<&str>) -> Value {
    let mut value = json!({
        "fact_kind": "cfg_domain",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "cfg_domain_id": format!("cfg:{build_domain_id}"),
        "build_domain_id": build_domain_id,
        "active_cfg_hash": "sha256:cfg",
        "status": status
    });
    if let Some(blocking_reason) = blocking_reason {
        value["blocking_reason"] = json!(blocking_reason);
    }
    value
}

fn authority(id: &str, term: &str, status: &str, line: u32) -> Value {
    authority_with_scope(
        id,
        term,
        status,
        "bd:checker",
        Some("call:handoff-spawn"),
        "proof_only",
        line,
    )
}

fn authority_with_scope(
    id: &str,
    term: &str,
    status: &str,
    build_domain_id: &str,
    call_site_id: Option<&str>,
    evidence_use: &str,
    line: u32,
) -> Value {
    let mut value = json!({
        "fact_kind": "authority",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "authority_fact_id": id,
        "build_domain_id": build_domain_id,
        "source_span": {
            "file": "src/handoff.rs",
            "start_byte": line * 10,
            "end_byte": line * 10 + 5,
            "line_start": line,
            "line_end": line
        },
        "authority_term": term,
        "status": status,
        "evidence_use": evidence_use
    });
    if let Some(call_site_id) = call_site_id {
        value["call_site_id"] = json!(call_site_id);
    }
    value
}

fn domain_authority(id: &str, term: &str, status: &str, build_domain_id: &str, line: u32) -> Value {
    authority_with_scope(id, term, status, build_domain_id, None, "proof_only", line)
}

fn blocker(reason: &str) -> Value {
    blocker_for(reason, "bd:checker", Some("call:handoff-spawn"))
}

fn blocker_for(reason: &str, build_domain_id: &str, call_site_id: Option<&str>) -> Value {
    let mut value = json!({
        "fact_kind": "proof_blocker",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "blocker_id": format!("blocker:{reason}"),
        "reason": reason,
        "status": "blocked",
        "build_domain_id": build_domain_id,
        "detail": "fixture blocker"
    });
    if let Some(call_site_id) = call_site_id {
        value["call_site_id"] = json!(call_site_id);
    }
    value
}

fn unscoped_blocker(reason: &str) -> Value {
    json!({
        "fact_kind": "proof_blocker",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "blocker_id": format!("blocker:{reason}"),
        "reason": reason,
        "status": "blocked",
        "detail": "fixture blocker"
    })
}

fn legal_handoff_records() -> Vec<Value> {
    vec![
        call_site(),
        call_edge(),
        process_effect(),
        authority("authority:successor", "successor", "admitted", 7),
        authority("authority:parent", "parent_lineage", "admitted", 8),
        authority(
            "authority:predecessor",
            "predecessor_retired",
            "admitted",
            9,
        ),
        authority("authority:crown", "crown_ruling", "admitted", 10),
    ]
}

fn proof_and_navigation_handoff_records() -> Vec<Value> {
    let mut records = legal_handoff_records();
    for record in &mut records {
        if record.get("evidence_use").is_some() {
            record["evidence_use"] = json!("proof_and_navigation");
        }
    }
    records
}

fn navigation_only_unresolved_process_records() -> Vec<Value> {
    let mut site = call_site_named("call:navigation-spawn", "bd:checker", "def:handoff", 70);
    site["evidence_use"] = json!("navigation_only");
    let mut effect = process_effect_named("effect:navigation-spawn", "call:navigation-spawn");
    effect["evidence_use"] = json!("navigation_only");
    vec![
        site,
        unresolved_call_edge_for("call:navigation-spawn", "navigation_only"),
        effect,
    ]
}

fn unscoped_legal_handoff_records_for(
    label: &str,
    build_domain_id: &str,
    line_base: u32,
) -> Vec<Value> {
    let call_site_id = format!("call:{label}-spawn");
    let call_edge_id = format!("edge:{label}-spawn");
    let effect_seed_id = format!("effect:{label}-spawn");
    let caller_def_id = format!("def:{label}-handoff");
    let callee_def_id = format!("def:{label}-successor");
    vec![
        call_site_named(&call_site_id, build_domain_id, &caller_def_id, line_base),
        call_edge_named(&call_edge_id, &call_site_id, &caller_def_id, &callee_def_id),
        process_effect_named(&effect_seed_id, &call_site_id),
        domain_authority(
            &format!("authority:{label}:successor"),
            "successor",
            "admitted",
            build_domain_id,
            line_base + 1,
        ),
        domain_authority(
            &format!("authority:{label}:parent"),
            "parent_lineage",
            "admitted",
            build_domain_id,
            line_base + 2,
        ),
        domain_authority(
            &format!("authority:{label}:predecessor"),
            "predecessor_retired",
            "admitted",
            build_domain_id,
            line_base + 3,
        ),
        domain_authority(
            &format!("authority:{label}:crown"),
            "crown_ruling",
            "admitted",
            build_domain_id,
            line_base + 4,
        ),
    ]
}

fn db_with(records: Vec<Value>) -> Database {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    db.upsert_proof_fact_values(&records)
        .expect("import proof facts");
    db
}

fn db_with_raw_unscoped_process_effect() -> Database {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let mut params = BTreeMap::new();
    params.insert(
        "fact_id".to_string(),
        data_string(Some("effect:missing-site")),
    );
    params.insert("kind".to_string(), data_string(Some("effect_seed")));
    params.insert(
        "schema_version".to_string(),
        data_string(Some(PROOF_FACT_SCHEMA_VERSION)),
    );
    params.insert("json".to_string(), DataValue::Json(JsonData(json!({}))));
    params.insert("evidence_use".to_string(), data_string(Some("proof_only")));
    params.insert("build_domain_id".to_string(), DataValue::Null);
    params.insert("call_site_id".to_string(), DataValue::Null);
    params.insert("call_edge_id".to_string(), DataValue::Null);
    params.insert("caller_def_id".to_string(), DataValue::Null);
    params.insert("callee_def_id".to_string(), DataValue::Null);
    params.insert("resolution_state".to_string(), DataValue::Null);
    params.insert("source_file".to_string(), DataValue::Null);
    params.insert("start_byte".to_string(), DataValue::Null);
    params.insert("end_byte".to_string(), DataValue::Null);
    params.insert("line_start".to_string(), DataValue::Null);
    params.insert("line_end".to_string(), DataValue::Null);
    params.insert(
        "effect_class".to_string(),
        data_string(Some("operating_system_process_create")),
    );
    params.insert("blocker_reason".to_string(), DataValue::Null);
    params.insert("status".to_string(), DataValue::Null);
    params.insert("detail".to_string(), data_string(Some("command-spawn")));

    let script = r#"
{
    ?[fact_id, kind, schema_version, json, evidence_use, build_domain_id, call_site_id, call_edge_id, caller_def_id, callee_def_id, resolution_state, source_file, start_byte, end_byte, line_start, line_end, effect_class, blocker_reason, status, detail] :=
        fact_id = $fact_id,
        kind = $kind,
        schema_version = $schema_version,
        json = $json,
        evidence_use = $evidence_use,
        build_domain_id = $build_domain_id,
        call_site_id = $call_site_id,
        call_edge_id = $call_edge_id,
        caller_def_id = $caller_def_id,
        callee_def_id = $callee_def_id,
        resolution_state = $resolution_state,
        source_file = $source_file,
        start_byte = $start_byte,
        end_byte = $end_byte,
        line_start = $line_start,
        line_end = $line_end,
        effect_class = $effect_class,
        blocker_reason = $blocker_reason,
        status = $status,
        detail = $detail
    :put proof_fact { fact_id => kind, schema_version, json, evidence_use, build_domain_id, call_site_id, call_edge_id, caller_def_id, callee_def_id, resolution_state, source_file, start_byte, end_byte, line_start, line_end, effect_class, blocker_reason, status, detail }
}
"#;
    db.run_script(script, params, ScriptMutability::Mutable)
        .expect("insert raw unscoped process effect");
    db
}

fn data_string(value: Option<&str>) -> DataValue {
    value
        .map(|value| DataValue::Str(value.into()))
        .unwrap_or(DataValue::Null)
}

fn finding_for<'a>(
    findings: &'a [ProofInvariantFinding],
    invariant: &str,
    call_site_id: &str,
) -> &'a ProofInvariantFinding {
    let mut matching = findings.iter().filter(|finding| {
        finding.invariant == invariant && finding.call_site_id.as_deref() == Some(call_site_id)
    });
    let finding = matching.next().expect("expected scoped finding");
    assert!(
        matching.next().is_none(),
        "duplicate scoped finding for {invariant} at {call_site_id}: {findings:?}"
    );
    finding
}

fn assert_no_status(
    findings: &[ProofInvariantFinding],
    invariant: &str,
    status: ProofInvariantStatus,
) {
    assert!(
        findings
            .iter()
            .filter(|finding| finding.invariant == invariant)
            .all(|finding| finding.status != status),
        "unexpected {status:?} finding for {invariant}: {findings:?}"
    );
}

#[test]
fn proof_invariant_checker_passes_legal_successor_handoff_fixture() {
    let db = db_with(legal_handoff_records());

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");

    assert_eq!(
        finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn").status,
        ProofInvariantStatus::Pass
    );
    assert_eq!(
        finding_for(&findings, CROWN_INVARIANT, "call:handoff-spawn").status,
        ProofInvariantStatus::Pass
    );
    assert_no_status(&findings, DETACHED_INVARIANT, ProofInvariantStatus::Fail);
    assert_no_status(&findings, DETACHED_INVARIANT, ProofInvariantStatus::Blocked);
    assert_no_status(&findings, CROWN_INVARIANT, ProofInvariantStatus::Fail);
    assert_no_status(&findings, CROWN_INVARIANT, ProofInvariantStatus::Blocked);
}

#[test]
fn proof_invariant_checker_accepts_proof_and_navigation_evidence_for_handoff() {
    let db = db_with(proof_and_navigation_handoff_records());

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");

    assert_eq!(
        finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn").status,
        ProofInvariantStatus::Pass
    );
    assert_eq!(
        finding_for(&findings, CROWN_INVARIANT, "call:handoff-spawn").status,
        ProofInvariantStatus::Pass
    );
}

#[test]
fn proof_invariant_checker_treats_process_replace_as_detached_process_obligation() {
    let mut replace_effect = process_effect();
    replace_effect["effect_class"] = json!("operating_system_process_replace");
    let db = db_with(vec![call_site(), call_edge(), replace_effect]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Fail);
    assert!(
        finding
            .reason
            .contains("detached process create lacks admitted successor handoff")
    );
}

#[test]
fn proof_invariant_checker_blocks_active_blocker_even_without_process_scope() {
    let db = db_with(vec![unscoped_blocker("process_lifetime_evidence_missing")]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");

    assert!(findings.iter().any(|finding| {
        finding.invariant == DETACHED_INVARIANT
            && finding.call_site_id.is_none()
            && finding.status == ProofInvariantStatus::Blocked
            && finding.reason.contains("process_lifetime_evidence_missing")
    }));
    assert_no_status(&findings, DETACHED_INVARIANT, ProofInvariantStatus::Pass);
}

#[test]
fn proof_invariant_checker_ignores_scoped_blocker_without_process_scope() {
    let db = db_with(vec![
        call_site_named("call:non-process", "bd:checker", "def:non-process", 12),
        blocker_for(
            "process_lifetime_evidence_missing",
            "bd:checker",
            Some("call:non-process"),
        ),
    ]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");

    assert!(findings.iter().any(|finding| {
        finding.invariant == DETACHED_INVARIANT
            && finding.call_site_id.is_none()
            && finding.status == ProofInvariantStatus::Pass
            && finding
                .reason
                .contains("no proof-only detached process effects recorded")
    }));
    assert_no_status(&findings, DETACHED_INVARIANT, ProofInvariantStatus::Blocked);
}

#[test]
fn proof_invariant_checker_blocks_scoped_blocker_with_missing_call_site_without_process_scope() {
    let db = db_with(vec![blocker_for(
        "process_lifetime_evidence_missing",
        "bd:checker",
        Some("call:missing"),
    )]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");

    assert!(findings.iter().any(|finding| {
        finding.invariant == DETACHED_INVARIANT
            && finding.call_site_id.is_none()
            && finding.status == ProofInvariantStatus::Blocked
            && finding.reason.contains("process_lifetime_evidence_missing")
    }));
    assert_no_status(&findings, DETACHED_INVARIANT, ProofInvariantStatus::Pass);
}

#[test]
fn proof_invariant_checker_fails_illegal_detached_spawn_without_successor() {
    let db = db_with(vec![call_site(), call_edge(), process_effect()]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Fail);
    assert!(
        finding
            .reason
            .contains("detached process create lacks admitted successor handoff")
    );
}

#[test]
fn proof_invariant_checker_fails_two_crown_ruling_parents_in_one_lineage() {
    let mut records = legal_handoff_records();
    records.push(authority(
        "authority:crown:second",
        "crown_ruling",
        "admitted",
        11,
    ));
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, CROWN_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Fail);
    assert!(finding.reason.contains("multiple Crown<Ruling>"));
}

#[test]
fn proof_invariant_checker_blocks_incomplete_process_or_authority_evidence() {
    let db = db_with(vec![
        call_site(),
        call_edge(),
        process_effect(),
        authority("authority:successor:blocked", "successor", "blocked", 7),
        blocker("process_lifetime_evidence_missing"),
    ]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let detached = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");
    let crown = finding_for(&findings, CROWN_INVARIANT, "call:handoff-spawn");

    assert_eq!(detached.status, ProofInvariantStatus::Blocked);
    assert!(
        detached
            .reason
            .contains("process_lifetime_evidence_missing")
    );
    assert_eq!(crown.status, ProofInvariantStatus::Blocked);
    assert!(crown.reason.contains("authority evidence is blocked"));
}

#[test]
fn proof_invariant_checker_blocks_macro_process_spawn_when_expansion_evidence_missing() {
    let mut records = legal_handoff_records();
    records.push(expansion_boundary(
        "boundary:macro-spawn",
        "bd:checker",
        "macro_rules_invocation",
        "unresolved",
        Some("macro_expansion_not_available"),
        60,
    ));
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(finding.reason.contains("macro_expansion_not_available"));
}

#[test]
fn proof_invariant_checker_blocks_cfg_domain_evidence_gap_explicitly() {
    let mut records = legal_handoff_records();
    records.push(cfg_domain(
        "bd:checker",
        "blocked",
        Some("cfg_domain_not_materialized"),
    ));
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(finding.reason.contains("cfg_domain_not_materialized"));
}

#[test]
fn proof_invariant_checker_blocks_external_dependency_resolution_gap_explicitly() {
    let mut records = legal_handoff_records();
    records.push(call_resolution_for(
        "call:handoff-spawn",
        "externally_summarized",
        Some("external_dependency_summary_missing"),
    ));
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(
        finding
            .reason
            .contains("external_dependency_summary_missing")
    );
}

#[test]
fn proof_invariant_checker_blocks_external_summary_expansion_boundaries() {
    let cases = [
        ("external_summary", "external_dependency_summary_missing"),
        ("proc_macro_function", "proc_macro_summary_missing"),
        ("build_script", "build_script_summary_missing"),
    ];

    for (boundary_kind, reason) in cases {
        let mut records = legal_handoff_records();
        records.push(expansion_boundary(
            "boundary:external-summary",
            "bd:checker",
            boundary_kind,
            "externally_summarized",
            None,
            63,
        ));
        let db = db_with(records);

        let findings = db
            .proof_invariant_findings()
            .expect("proof invariant findings");
        let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

        assert_eq!(
            finding.status,
            ProofInvariantStatus::Blocked,
            "{boundary_kind} should block detached proof"
        );
        assert!(
            finding.reason.contains(reason),
            "{boundary_kind} should report {reason}: {}",
            finding.reason
        );
    }
}

#[test]
fn proof_invariant_checker_blocks_blocked_external_summary_artifacts() {
    let cases = [
        ("blocked", "opaque_blocked"),
        ("rejected", "audited_no_process_effects"),
    ];

    for (status, summary_class) in cases {
        let mut records = legal_handoff_records();
        records.push(external_summary(status, summary_class));
        let db = db_with(records);

        let findings = db
            .proof_invariant_findings()
            .expect("proof invariant findings");
        let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

        assert_eq!(
            finding.status,
            ProofInvariantStatus::Blocked,
            "{status} external summary should block detached proof"
        );
        assert!(
            finding.reason.contains(summary_class),
            "{status} external summary should report {summary_class}: {}",
            finding.reason
        );
    }
}

#[test]
fn proof_invariant_checker_discharges_linked_admitted_external_summary_gaps() {
    let mut records = legal_handoff_records();
    records.push(call_resolution_for(
        "call:handoff-spawn",
        "externally_summarized",
        None,
    ));
    records.push(expansion_boundary(
        "boundary:external-summary",
        "bd:checker",
        "external_summary",
        "externally_summarized",
        None,
        63,
    ));
    let mut summary = external_summary("admitted", "audited_no_process_effects");
    summary["external_summary_id"] = json!("external-summary:test");
    records.push(summary);
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Pass);
    assert!(
        !finding
            .reason
            .contains("external_dependency_summary_missing"),
        "admitted linked external summary should discharge missing-summary blockers: {finding:#?}"
    );
}

#[test]
fn proof_invariant_checker_requires_allowed_effect_for_external_summary_discharge() {
    let mut records = legal_handoff_records();
    records.push(call_resolution_for(
        "call:handoff-spawn",
        "externally_summarized",
        None,
    ));
    records.push(expansion_boundary(
        "boundary:external-summary",
        "bd:checker",
        "external_summary",
        "externally_summarized",
        None,
        63,
    ));
    let mut summary = external_summary("admitted", "audited_no_process_effects");
    summary["external_summary_id"] = json!("external-summary:test");
    summary["allowed_effects"] = json!(["durable_evidence_read"]);
    records.push(summary);
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(
        finding
            .reason
            .contains("external_dependency_summary_missing"),
        "summary without external-summary authority should fail closed: {finding:#?}"
    );
}

#[test]
fn proof_invariant_checker_requires_call_site_domain_match_for_external_summary_discharge() {
    let mut records = legal_handoff_records();
    records.push(call_resolution_for(
        "call:handoff-spawn",
        "externally_summarized",
        None,
    ));
    let mut summary = external_summary("admitted", "audited_no_process_effects");
    summary["external_summary_id"] = json!("external-summary:test");
    summary["build_domain_id"] = json!("bd:other");
    records.push(summary);
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(
        finding
            .reason
            .contains("external_dependency_summary_missing"),
        "summary from another build domain should fail closed: {finding:#?}"
    );
}

#[test]
fn proof_invariant_checker_blocks_authority_evidence_gap_without_demoting_to_fail() {
    let db = db_with(vec![
        call_site(),
        call_edge(),
        process_effect(),
        authority("authority:successor:blocked", "successor", "blocked", 7),
    ]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(finding.reason.contains("authority evidence is blocked"));
}

#[test]
fn proof_invariant_checker_ignores_navigation_only_authority_for_detached_handoff() {
    let db = db_with(vec![
        call_site(),
        call_edge(),
        process_effect(),
        authority_with_scope(
            "authority:successor:navigation",
            "successor",
            "admitted",
            "bd:checker",
            Some("call:handoff-spawn"),
            "navigation_only",
            7,
        ),
        authority("authority:parent", "parent_lineage", "admitted", 8),
        authority(
            "authority:predecessor",
            "predecessor_retired",
            "admitted",
            9,
        ),
        authority("authority:crown", "crown_ruling", "admitted", 10),
    ]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Fail);
    assert!(
        finding
            .reason
            .contains("detached process create lacks admitted successor handoff")
    );
}

#[test]
fn proof_invariant_checker_rejects_unknown_evidence_use_for_authority() {
    let db = Database::new_init().expect("create db");
    db.ensure_proof_graph_schema().expect("proof graph schema");
    let error = db
        .upsert_proof_fact_values(&[
            call_site(),
            call_edge(),
            process_effect(),
            authority_with_scope(
                "authority:successor:audit",
                "successor",
                "admitted",
                "bd:checker",
                Some("call:handoff-spawn"),
                "audit_only",
                7,
            ),
        ])
        .expect_err("invalid evidence_use should reject before checking");

    assert!(error.to_string().contains("evidence_use"));
}

#[test]
fn proof_invariant_checker_requires_authority_build_domain_match_for_same_call_site() {
    let db = db_with(vec![
        call_site_named("call:shared-spawn", "bd:primary", "def:shared", 20),
        call_edge_named(
            "edge:shared-spawn",
            "call:shared-spawn",
            "def:shared",
            "def:shared-successor",
        ),
        process_effect_named("effect:shared-spawn", "call:shared-spawn"),
        authority_with_scope(
            "authority:shared:successor:other-domain",
            "successor",
            "admitted",
            "bd:other",
            Some("call:shared-spawn"),
            "proof_only",
            21,
        ),
        authority_with_scope(
            "authority:shared:parent:other-domain",
            "parent_lineage",
            "admitted",
            "bd:other",
            Some("call:shared-spawn"),
            "proof_only",
            22,
        ),
        authority_with_scope(
            "authority:shared:predecessor:other-domain",
            "predecessor_retired",
            "admitted",
            "bd:other",
            Some("call:shared-spawn"),
            "proof_only",
            23,
        ),
        authority_with_scope(
            "authority:shared:crown:other-domain",
            "crown_ruling",
            "admitted",
            "bd:other",
            Some("call:shared-spawn"),
            "proof_only",
            24,
        ),
    ]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:shared-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Fail);
    assert!(
        finding
            .reason
            .contains("detached process create lacks admitted successor handoff")
    );
}

#[test]
fn proof_invariant_checker_blocks_mismatched_call_site_identity_fail_closed() {
    let mut mismatched_edge = call_edge();
    mismatched_edge["caller_def_id"] = json!("def:other-caller");
    let db = db_with(vec![
        call_site(),
        mismatched_edge,
        process_effect(),
        authority("authority:successor", "successor", "admitted", 7),
        authority("authority:parent", "parent_lineage", "admitted", 8),
        authority(
            "authority:predecessor",
            "predecessor_retired",
            "admitted",
            9,
        ),
        authority("authority:crown", "crown_ruling", "admitted", 10),
    ]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(finding.reason.contains("canonical_identity_mismatch"));
}

#[test]
fn proof_invariant_checker_blocks_unresolved_proof_critical_call() {
    let db = db_with(vec![
        call_site(),
        unresolved_call_edge_for("call:handoff-spawn", "proof_only"),
        process_effect(),
        authority("authority:successor", "successor", "admitted", 7),
        authority("authority:parent", "parent_lineage", "admitted", 8),
        authority(
            "authority:predecessor",
            "predecessor_retired",
            "admitted",
            9,
        ),
        authority("authority:crown", "crown_ruling", "admitted", 10),
    ]);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(finding.reason.contains("type_resolution_missing"));
}

#[test]
fn proof_invariant_checker_does_not_claim_success_for_navigation_only_unresolved_process_call() {
    let db = db_with(navigation_only_unresolved_process_records());

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");

    assert!(findings.iter().any(|finding| {
        finding.invariant == DETACHED_INVARIANT
            && finding.call_site_id.is_none()
            && finding.status == ProofInvariantStatus::Blocked
            && finding
                .reason
                .contains("navigation-only unresolved process call")
    }));
    assert_no_status(&findings, DETACHED_INVARIANT, ProofInvariantStatus::Pass);
}

#[test]
fn proof_invariant_checker_blocks_process_effect_without_call_site_fail_closed() {
    let db = db_with_raw_unscoped_process_effect();

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");

    assert!(findings.iter().any(|finding| {
        finding.invariant == DETACHED_INVARIANT
            && finding.call_site_id.is_none()
            && finding.status == ProofInvariantStatus::Blocked
            && finding.reason.contains("lacks call_site_id")
    }));
    assert_no_status(&findings, DETACHED_INVARIANT, ProofInvariantStatus::Pass);
}

#[test]
fn proof_invariant_checker_scopes_blockers_to_matching_call_site() {
    let mut records = legal_handoff_records();
    records.push(blocker_for(
        "process_lifetime_evidence_missing",
        "bd:checker",
        Some("call:unrelated-spawn"),
    ));
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");

    assert_eq!(
        finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn").status,
        ProofInvariantStatus::Pass
    );
    assert_no_status(&findings, DETACHED_INVARIANT, ProofInvariantStatus::Blocked);
}

#[test]
fn proof_invariant_checker_blocks_unscoped_proof_blocker_fail_closed() {
    let mut records = legal_handoff_records();
    records.push(unscoped_blocker("process_lifetime_evidence_missing"));
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(finding.reason.contains("process_lifetime_evidence_missing"));
}

#[test]
fn proof_invariant_checker_blocks_rejected_proof_blocker_fail_closed() {
    let mut records = legal_handoff_records();
    let mut rejected_blocker = unscoped_blocker("process_lifetime_evidence_missing");
    rejected_blocker["status"] = json!("rejected");
    records.push(rejected_blocker);
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Blocked);
    assert!(finding.reason.contains("process_lifetime_evidence_missing"));
}

#[test]
fn proof_invariant_checker_checks_each_detached_process_site_independently() {
    let mut records = legal_handoff_records();
    records.push(call_site_named(
        "call:uncovered-spawn",
        "bd:checker",
        "def:uncovered",
        20,
    ));
    records.push(call_edge_named(
        "edge:uncovered-spawn",
        "call:uncovered-spawn",
        "def:uncovered",
        "def:spawn-uncovered",
    ));
    records.push(process_effect_named(
        "effect:uncovered-spawn",
        "call:uncovered-spawn",
    ));
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let covered = finding_for(&findings, DETACHED_INVARIANT, "call:handoff-spawn");
    let uncovered = finding_for(&findings, DETACHED_INVARIANT, "call:uncovered-spawn");

    assert_eq!(covered.status, ProofInvariantStatus::Pass);
    assert_eq!(uncovered.status, ProofInvariantStatus::Fail);
    assert!(
        uncovered
            .reason
            .contains("detached process create lacks admitted successor handoff")
    );
}

#[test]
fn proof_invariant_checker_does_not_use_domain_authority_for_detached_site() {
    let db = db_with(unscoped_legal_handoff_records_for(
        "domain-only",
        "bd:domain-only",
        30,
    ));

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");
    let finding = finding_for(&findings, DETACHED_INVARIANT, "call:domain-only-spawn");

    assert_eq!(finding.status, ProofInvariantStatus::Fail);
    assert!(
        finding
            .reason
            .contains("detached process create lacks admitted successor handoff")
    );
}

#[test]
fn proof_invariant_checker_allows_independent_crown_rulings_per_build_domain() {
    let mut records = unscoped_legal_handoff_records_for("primary", "bd:primary", 40);
    records.extend(unscoped_legal_handoff_records_for(
        "secondary",
        "bd:secondary",
        50,
    ));
    let db = db_with(records);

    let findings = db
        .proof_invariant_findings()
        .expect("proof invariant findings");

    assert_eq!(
        finding_for(&findings, CROWN_INVARIANT, "call:primary-spawn").status,
        ProofInvariantStatus::Pass
    );
    assert_eq!(
        finding_for(&findings, CROWN_INVARIANT, "call:secondary-spawn").status,
        ProofInvariantStatus::Pass
    );
    assert_no_status(&findings, CROWN_INVARIANT, ProofInvariantStatus::Fail);
}
