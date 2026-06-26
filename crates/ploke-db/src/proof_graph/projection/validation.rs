use super::*;

pub(super) fn validate_required_fields(value: &Value, kind: &str) -> Result<(), DbError> {
    match kind {
        "build_domain" => require_fields(
            value,
            &[
                "build_domain_id",
                "cargo_metadata_hash",
                "cargo_lock_hash",
                "package_id",
                "target_kind",
                "target_name",
                "target_root",
                "target_triple",
                "host_triple",
                "profile",
                "features_hash",
                "active_cfg_hash",
                "rustc_version",
                "extractor_version",
                "proof_policy_version",
            ],
        ),
        "cfg_domain" => require_fields(
            value,
            &[
                "cfg_domain_id",
                "build_domain_id",
                "active_cfg_hash",
                "status",
            ],
        ),
        "rustc_invocation" => require_fields(
            value,
            &[
                "invocation_id",
                "build_domain_id",
                "rustc_program",
                "rustc_version",
                "working_directory",
                "argument_vector_hash",
                "environment_hash",
                "status",
            ],
        ),
        "expansion_boundary" => {
            require_fields(
                value,
                &[
                    "boundary_id",
                    "build_domain_id",
                    "boundary_kind",
                    "expansion_state",
                ],
            )?;
            require_source_span(value)
        }
        "expanded_item" => {
            require_fields(
                value,
                &[
                    "expanded_item_id",
                    "boundary_id",
                    "build_domain_id",
                    "definition_id",
                ],
            )?;
            require_source_span(value)
        }
        "call_site" => {
            require_fields(value, &["call_site_id", "build_domain_id", "caller_def_id"])?;
            require_source_span(value)
        }
        "call_edge" => require_fields(
            value,
            &[
                "call_edge_id",
                "call_site_id",
                "caller_def_id",
                "resolution_state",
            ],
        ),
        "call_resolution" => require_fields(value, &["call_site_id", "resolution_state"]),
        "effect_seed" => {
            require_fields(
                value,
                &[
                    "effect_seed_id",
                    "call_site_id",
                    "effect_class",
                    "confidence",
                ],
            )?;
            require_json_bool(value, "blocker_if_unresolved")
        }
        "authority" => {
            require_fields(
                value,
                &[
                    "authority_fact_id",
                    "build_domain_id",
                    "authority_term",
                    "status",
                ],
            )?;
            require_source_span(value)
        }
        "proof_blocker" => require_fields(value, &["blocker_id", "reason", "status", "detail"]),
        other => Err(DbError::QueryConstruction(format!(
            "unknown proof fact kind {other}"
        ))),
    }
}

pub(super) fn validate_enum_fields(value: &Value, kind: &str) -> Result<(), DbError> {
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

fn require_fields(value: &Value, fields: &[&str]) -> Result<(), DbError> {
    for field in fields {
        required_json_string(value, field)?;
    }
    Ok(())
}

fn require_source_span(value: &Value) -> Result<(), DbError> {
    let span = value.get("source_span").ok_or_else(|| {
        DbError::QueryConstruction("proof fact JSON missing object field source_span".to_string())
    })?;
    required_json_string(span, "file")?;
    for field in ["start_byte", "end_byte"] {
        if span.get(field).and_then(Value::as_u64).is_none() {
            return Err(DbError::QueryConstruction(format!(
                "proof fact source_span missing integer field {field}"
            )));
        }
    }
    for field in ["line_start", "line_end"] {
        if span.get(field).is_some() && span.get(field).and_then(Value::as_u64).is_none() {
            return Err(DbError::QueryConstruction(format!(
                "proof fact source_span field {field} is not an integer"
            )));
        }
    }
    Ok(())
}

fn require_json_bool(value: &Value, field: &str) -> Result<(), DbError> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .map(|_| ())
        .ok_or_else(|| {
            DbError::QueryConstruction(format!("proof fact JSON missing bool field {field}"))
        })
}
