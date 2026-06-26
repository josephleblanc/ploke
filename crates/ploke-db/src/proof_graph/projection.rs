use serde_json::Value;

use super::PROOF_FACT_SCHEMA_VERSION;
use crate::DbError;

mod extract;
mod validation;

use extract::{fact_id, json_string, json_u32, required_json_string};
use validation::{validate_enum_fields, validate_required_fields};

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
        projection.evidence_use = json_string(value, "evidence_use");
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
            .or_else(|| json_string(value, "summary_class"))
            .or_else(|| json_string(value, "confidence"))
            .or_else(|| json_string(value, "target_name"))
            .or_else(|| json_string(value, "boundary_kind"));
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
