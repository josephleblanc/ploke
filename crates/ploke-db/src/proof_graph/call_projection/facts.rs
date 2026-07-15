use serde_json::Value;

use crate::call_graph::{CallContextRow, CallSiteKind, CallStatusKind, CallTargetRow};

use super::PROOF_FACT_SCHEMA_VERSION;

pub(super) fn call_site_fact(
    row: &CallContextRow,
    build_domain_id: &str,
    source_file: &str,
) -> Value {
    serde_json::json!({
        "fact_kind": "call_site",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_site_id": row.site.id.to_string(),
        "build_domain_id": build_domain_id,
        "caller_def_id": row.site.owner_id.to_string(),
        "unsafe_block": row.site.unsafe_block,
        "source_span": {
            "file": source_file,
            "start_byte": row.site.span.0,
            "end_byte": row.site.span.1
        },
        "evidence_use": "proof_and_navigation"
    })
}

pub(super) fn call_edge_facts(row: &CallContextRow) -> Vec<Value> {
    if row.status.status != CallStatusKind::Resolved {
        return Vec::new();
    }

    row.targets
        .iter()
        .map(|target| call_edge_fact(row, target))
        .collect()
}

pub(super) fn call_resolution_fact(row: &CallContextRow) -> Value {
    let mut value = serde_json::json!({
        "fact_kind": "call_resolution",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_site_id": row.site.id.to_string(),
        "resolution_state": resolution_state(row.status.status),
        "evidence_use": "proof_and_navigation"
    });

    if let Some(target) = resolved_target(row) {
        value["resolved_def_id"] = serde_json::json!(target.target_id.to_string());
    }
    let candidates = candidate_targets(row);
    if !candidates.is_empty() {
        value["candidate_def_ids"] = serde_json::json!(candidates);
    }
    if let Some(reason) = blocking_reason(row) {
        value["blocking_reason"] = serde_json::json!(reason);
    }

    value
}

pub(super) fn binding_evidence_fact(
    row: &CallContextRow,
    evidence_kind: &str,
    evidence_path: &[String],
    build_domain_id: &str,
    source_file: &str,
) -> Value {
    let callee_path = evidence_path.join("::");
    serde_json::json!({
        "fact_kind": "binding_evidence",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "binding_evidence_id": format!("binding-evidence:returned-callable:{}", row.site.id),
        "build_domain_id": build_domain_id,
        "call_site_id": row.site.id.to_string(),
        "caller_def_id": row.site.owner_id.to_string(),
        "binding_evidence_kind": "returned_callable",
        "callee_kind": evidence_kind,
        "callee_path": evidence_path,
        "resolution_state": resolution_state(row.status.status),
        "detail": match row.status.status {
            CallStatusKind::Resolved => format!(
                "{} invokes callable returned by {callee_path}; returned-callable binding evidence is projected for proof context",
                row.site.owner_id
            ),
            _ => format!(
                "{} invokes callable returned by {callee_path}; binding evidence records the fail-closed boundary until returned-callable value flow is proven",
                row.site.owner_id
            ),
        },
        "source_span": {
            "file": source_file,
            "start_byte": row.site.span.0,
            "end_byte": row.site.span.1
        },
        "evidence_use": "proof_only"
    })
}

pub(super) fn self_field_binding_evidence_fact(
    row: &CallContextRow,
    field_path: &[String],
    build_domain_id: &str,
    source_file: &str,
) -> Value {
    let path = field_path.join(".");
    serde_json::json!({
        "fact_kind": "binding_evidence",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "binding_evidence_id": format!("binding-evidence:self-field-callable:{}", row.site.id),
        "build_domain_id": build_domain_id,
        "call_site_id": row.site.id.to_string(),
        "caller_def_id": row.site.owner_id.to_string(),
        "binding_evidence_kind": "self_field_callable",
        "callee_kind": "SelfField",
        "callee_path": field_path,
        "resolution_state": resolution_state(row.status.status),
        "detail": match row.status.status {
            CallStatusKind::Ambiguous => format!(
                "{} calls callable self-field {path}; binding evidence records the finite candidate set without admitting a traversal edge until field value flow is proven",
                row.site.owner_id
            ),
            _ => format!(
                "{} calls callable self-field {path}; binding evidence records the fail-closed boundary until field value flow is proven",
                row.site.owner_id
            ),
        },
        "source_span": {
            "file": source_file,
            "start_byte": row.site.span.0,
            "end_byte": row.site.span.1
        },
        "evidence_use": "proof_only"
    })
}

fn call_edge_fact(row: &CallContextRow, target: &CallTargetRow) -> Value {
    serde_json::json!({
        "fact_kind": "call_edge",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_edge_id": format!("edge:{}:{}", row.site.id, target.target_id),
        "call_site_id": row.site.id.to_string(),
        "caller_def_id": row.site.owner_id.to_string(),
        "callee_def_id": target.target_id.to_string(),
        "resolution_state": resolution_state(row.status.status),
        "evidence_use": "proof_and_navigation"
    })
}

fn resolved_target(row: &CallContextRow) -> Option<&CallTargetRow> {
    (row.status.status == CallStatusKind::Resolved).then(|| &row.targets[0])
}

fn candidate_targets(row: &CallContextRow) -> Vec<String> {
    match row.status.status {
        CallStatusKind::Ambiguous => row
            .targets
            .iter()
            .map(|target| target.target_id.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn resolution_state(status: CallStatusKind) -> &'static str {
    match status {
        CallStatusKind::Resolved => "resolved",
        CallStatusKind::Unresolved => "unresolved",
        CallStatusKind::Ambiguous => "ambiguous",
        CallStatusKind::External => "blocked",
        CallStatusKind::Unsupported => "blocked",
    }
}

fn blocking_reason(row: &CallContextRow) -> Option<&'static str> {
    match row.status.status {
        CallStatusKind::Resolved => None,
        CallStatusKind::External => Some("external_dependency_summary_missing"),
        CallStatusKind::Unsupported => Some(match row.site.kind {
            CallSiteKind::Dynamic => "dynamic_dispatch_unbounded",
            CallSiteKind::Macro => "macro_expansion_not_available",
            CallSiteKind::Path | CallSiteKind::Method => "type_resolution_missing",
        }),
        CallStatusKind::Unresolved | CallStatusKind::Ambiguous => Some("type_resolution_missing"),
    }
}
