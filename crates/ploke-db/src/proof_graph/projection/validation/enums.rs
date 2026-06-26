use super::*;

pub(in crate::proof_graph::projection) fn validate_enum_fields(
    value: &Value,
    kind: &str,
) -> Result<(), DbError> {
    validate_optional_enum(
        value,
        "evidence_use",
        &["proof_only", "navigation_only", "proof_and_navigation"],
    )?;
    validate_optional_enum(value, "status", &["admitted", "rejected", "blocked"])?;
    validate_optional_enum(
        value,
        "resolution_state",
        &[
            "resolved",
            "candidate_set",
            "ambiguous",
            "unresolved",
            "externally_summarized",
            "blocked",
        ],
    )?;
    validate_optional_enum(
        value,
        "expansion_state",
        &["expanded", "unresolved", "externally_summarized", "blocked"],
    )?;
    validate_optional_enum(
        value,
        "effect_class",
        &[
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
        ],
    )?;
    validate_optional_enum(
        value,
        "authority_term",
        &[
            "parent_lineage",
            "crown_ruling",
            "authority_token_constructor",
            "successor",
            "predecessor_retired",
            "immutable_surface_digest_admission",
        ],
    )?;
    validate_optional_enum(value, "reason", PROOF_BLOCKER_REASONS)?;
    validate_optional_enum(value, "blocking_reason", PROOF_BLOCKER_REASONS)?;
    validate_optional_enum(
        value,
        "boundary_kind",
        &[
            "macro_rules_invocation",
            "proc_macro_derive",
            "proc_macro_attribute",
            "proc_macro_function",
            "build_script",
            "include",
            "external_summary",
        ],
    )?;
    if kind == "build_domain" {
        validate_optional_enum(
            value,
            "target_kind",
            &[
                "library",
                "binary",
                "test",
                "example",
                "benchmark",
                "build_script",
                "proc_macro",
            ],
        )?;
    }
    Ok(())
}

const PROOF_BLOCKER_REASONS: &[&str] = &[
    "macro_expansion_not_available",
    "proc_macro_summary_missing",
    "build_script_summary_missing",
    "external_dependency_summary_missing",
    "cfg_domain_not_materialized",
    "type_resolution_missing",
    "dynamic_dispatch_unbounded",
    "external_command_summary_missing",
    "build_script_execution_blocked",
    "proc_macro_execution_blocked",
    "canonical_identity_mismatch",
    "schema_version_mismatch",
    "authority_evidence_missing",
    "process_lifetime_evidence_missing",
    "rustc_invocation_evidence_missing",
];

fn validate_optional_enum(value: &Value, field: &str, allowed: &[&str]) -> Result<(), DbError> {
    let Some(raw) = value.get(field) else {
        return Ok(());
    };
    let Some(text) = raw.as_str() else {
        return Err(DbError::QueryConstruction(format!(
            "proof fact JSON field {field} is not a string"
        )));
    };
    if allowed.contains(&text) {
        Ok(())
    } else {
        Err(DbError::QueryConstruction(format!(
            "proof fact JSON field {field} has invalid value {text}"
        )))
    }
}
