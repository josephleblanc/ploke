use serde_json::Value;

use super::PROOF_FACT_SCHEMA_VERSION;
use crate::DbError;

#[derive(Debug, Clone)]
pub(super) struct ProofFactProjection {
    pub(super) fact_id: String,
    pub(super) kind: String,
    pub(super) schema_version: String,
    pub(super) json: Value,
    pub(super) evidence_use: Option<String>,
    pub(super) build_domain_id: Option<String>,
    pub(super) call_site_id: Option<String>,
    pub(super) call_edge_id: Option<String>,
    pub(super) caller_def_id: Option<String>,
    pub(super) callee_def_id: Option<String>,
    pub(super) resolution_state: Option<String>,
    pub(super) source_file: Option<String>,
    pub(super) start_byte: Option<u32>,
    pub(super) end_byte: Option<u32>,
    pub(super) line_start: Option<u32>,
    pub(super) line_end: Option<u32>,
    pub(super) effect_class: Option<String>,
    pub(super) blocker_reason: Option<String>,
    pub(super) status: Option<String>,
    pub(super) detail: Option<String>,
}

impl ProofFactProjection {
    pub(super) fn from_value(value: &Value) -> Result<Self, DbError> {
        let kind = required_json_string(value, "fact_kind")?.to_string();
        let schema_version = required_json_string(value, "schema_version")?.to_string();
        if schema_version != PROOF_FACT_SCHEMA_VERSION {
            return Err(DbError::QueryConstruction(format!(
                "proof fact schema_version {schema_version} does not match {PROOF_FACT_SCHEMA_VERSION}"
            )));
        }
        let fact_id = fact_id(value, &kind)?;
        validate_required_fields(value, &kind)?;
        validate_enum_fields(value, &kind)?;
        let mut projection = Self::base(fact_id, kind.clone(), schema_version, value.clone());
        projection.evidence_use =
            json_string(value, "evidence_use").or(Some("proof_only".to_string()));
        projection.build_domain_id = json_string(value, "build_domain_id");
        projection.call_site_id = json_string(value, "call_site_id");
        projection.call_edge_id = json_string(value, "call_edge_id");
        projection.caller_def_id = json_string(value, "caller_def_id");
        projection.callee_def_id = json_string(value, "callee_def_id");
        projection.resolution_state = json_string(value, "resolution_state");
        projection.effect_class =
            json_string(value, "effect_class").or_else(|| json_string(value, "authority_term"));
        projection.blocker_reason =
            json_string(value, "reason").or_else(|| json_string(value, "blocking_reason"));
        projection.status =
            json_string(value, "status").or_else(|| json_string(value, "expansion_state"));
        projection.detail = json_string(value, "detail")
            .or_else(|| json_string(value, "confidence"))
            .or_else(|| json_string(value, "target_name"));
        if let Some(span) = value.get("source_span") {
            projection.source_file = json_string(span, "file");
            projection.start_byte = json_u32(span, "start_byte")?;
            projection.end_byte = json_u32(span, "end_byte")?;
            projection.line_start = json_u32(span, "line_start")?;
            projection.line_end = json_u32(span, "line_end")?;
        }
        Ok(projection)
    }

    fn base(fact_id: String, kind: String, schema_version: String, json: Value) -> Self {
        Self {
            fact_id,
            kind,
            schema_version,
            json,
            evidence_use: None,
            build_domain_id: None,
            call_site_id: None,
            call_edge_id: None,
            caller_def_id: None,
            callee_def_id: None,
            resolution_state: None,
            source_file: None,
            start_byte: None,
            end_byte: None,
            line_start: None,
            line_end: None,
            effect_class: None,
            blocker_reason: None,
            status: None,
            detail: None,
        }
    }
}

fn fact_id(value: &Value, kind: &str) -> Result<String, DbError> {
    let field = match kind {
        "build_domain" => "build_domain_id",
        "cfg_domain" => "cfg_domain_id",
        "rustc_invocation" => "invocation_id",
        "expansion_boundary" => "boundary_id",
        "expanded_item" => "expanded_item_id",
        "call_site" => "call_site_id",
        "call_edge" => "call_edge_id",
        "call_resolution" => "call_site_id",
        "effect_seed" => "effect_seed_id",
        "authority" => "authority_fact_id",
        "proof_blocker" => "blocker_id",
        other => {
            return Err(DbError::QueryConstruction(format!(
                "unknown proof fact kind {other}"
            )));
        }
    };
    let id = required_json_string(value, field)?;
    if kind == "call_resolution" {
        Ok(format!("resolution:{id}"))
    } else {
        Ok(id.to_string())
    }
}

fn validate_required_fields(value: &Value, kind: &str) -> Result<(), DbError> {
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

fn validate_enum_fields(value: &Value, kind: &str) -> Result<(), DbError> {
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

fn json_string(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(ToOwned::to_owned)
}

fn required_json_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, DbError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        DbError::QueryConstruction(format!("proof fact JSON missing string field {field}"))
    })
}

fn json_u32(value: &Value, field: &str) -> Result<Option<u32>, DbError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .map(u32::try_from)
        .transpose()
        .map_err(|_| DbError::QueryConstruction(format!("proof field {field} is out of u32 range")))
}
