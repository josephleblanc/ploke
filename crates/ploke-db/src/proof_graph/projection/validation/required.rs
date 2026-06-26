use super::*;

pub(in crate::proof_graph::projection) fn validate_required_fields(
    value: &Value,
    kind: &str,
) -> Result<(), DbError> {
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
            require_source_span(value)?;
            require_external_summary_id_for_state(value, "expansion_state")
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
        "call_resolution" => {
            require_fields(value, &["call_site_id", "resolution_state"])?;
            require_external_summary_id_for_state(value, "resolution_state")
        }
        "external_summary" => {
            require_fields(
                value,
                &[
                    "external_summary_id",
                    "build_domain_id",
                    "summary_class",
                    "artifact_hash",
                    "version",
                    "review_method",
                    "scope_of_validity",
                    "required_containment",
                    "invalidation_conditions",
                    "status",
                ],
            )?;
            require_json_string_array(value, "allowed_effects")
        }
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

fn require_fields(value: &Value, fields: &[&str]) -> Result<(), DbError> {
    for field in fields {
        required_json_string(value, field)?;
    }
    Ok(())
}

fn require_external_summary_id_for_state(value: &Value, state_field: &str) -> Result<(), DbError> {
    if value.get(state_field).and_then(Value::as_str) == Some("externally_summarized") {
        required_json_string(value, "external_summary_id")?;
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

fn require_json_string_array(value: &Value, field: &str) -> Result<(), DbError> {
    let Some(items) = value.get(field).and_then(Value::as_array) else {
        return Err(DbError::QueryConstruction(format!(
            "proof fact JSON missing string array field {field}"
        )));
    };
    if items.is_empty() || items.iter().any(|item| item.as_str().is_none()) {
        return Err(DbError::QueryConstruction(format!(
            "proof fact JSON field {field} must be a non-empty string array"
        )));
    }
    Ok(())
}
