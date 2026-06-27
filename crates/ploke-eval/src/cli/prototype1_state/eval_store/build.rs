use std::{collections::BTreeMap, fs, path::Path};

use cozo::DataValue;
use ploke_records::ids::CampaignId;

use super::{
    cozo_store::{EvalDb, mutate_owner_db},
    error::EvalStoreError,
    evidence::{hash_parts, sha256_bytes},
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

define_eval_schema!(BinaryRefSchema {
    "eval_binary_ref",
    binary_ref_id: "String" =>
    campaign_id: "String",
    artifact_id: "String?",
    built_by: "String?",
    source_ref: "String",
    content_sha256: "String?",
    protocol_digest: "String?",
    recorded_at: "String?",
});

define_eval_schema!(BuildEventSchema {
    "eval_build_event",
    build_id: "String" =>
    campaign_id: "String",
    node_id: "String",
    runtime_id: "String?",
    artifact_id: "String?",
    phase: "String",
    outcome: "String",
    binary_ref: "String?",
    log_ref: "String?",
    recorded_at: "String",
});

pub(crate) const BUILD_EVENT_REL: &str = BuildEventSchema::RELATION;
pub(crate) const BINARY_REF_REL: &str = BinaryRefSchema::RELATION;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BinaryRefEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) artifact_id: Option<String>,
    pub(crate) built_by: Option<String>,
    pub(crate) source_ref: String,
    pub(crate) content_sha256: Option<String>,
    pub(crate) protocol_digest: Option<String>,
    pub(crate) recorded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildEventEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) node_id: String,
    pub(crate) runtime_id: Option<String>,
    pub(crate) artifact_id: Option<String>,
    pub(crate) phase: String,
    pub(crate) outcome: String,
    pub(crate) binary_ref: Option<String>,
    pub(crate) log_ref: Option<String>,
    pub(crate) recorded_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildProvenanceEvidence {
    pub(crate) binary_ref: BinaryRefEvidence,
    pub(crate) build_event: BuildEventEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct BuildProvenanceReceipt {
    pub(crate) binary_ref_id: String,
    pub(crate) build_id: String,
}

struct EvalBinaryRefRow {
    binary_ref_id: String,
    campaign_id: String,
    artifact_id: Option<String>,
    built_by: Option<String>,
    source_ref: String,
    content_sha256: Option<String>,
    protocol_digest: Option<String>,
    recorded_at: Option<String>,
}

struct EvalBuildEventRow {
    build_id: String,
    campaign_id: String,
    node_id: String,
    runtime_id: Option<String>,
    artifact_id: Option<String>,
    phase: String,
    outcome: String,
    binary_ref: Option<String>,
    log_ref: Option<String>,
    recorded_at: String,
}

pub(super) fn ensure_build_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    BinaryRefSchema::SCHEMA.ensure_installed(db, "schema.eval_binary_ref")?;
    BuildEventSchema::SCHEMA.ensure_installed(db, "schema.eval_build_event")?;
    Ok(())
}

pub(crate) fn write_build_provenance_to_owner_db(
    db_path: &Path,
    evidence: BuildProvenanceEvidence,
) -> Result<BuildProvenanceReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        ensure_build_schema(db)?;
        let binary = binary_ref_row(evidence.binary_ref)?;
        let mut build = build_event_row(evidence.build_event)?;
        if build.binary_ref.is_none() {
            build.binary_ref = Some(binary.binary_ref_id.clone());
        }
        put_binary_ref_row(db, &binary)?;
        put_build_event_row(db, &build)?;
        Ok(BuildProvenanceReceipt {
            binary_ref_id: binary.binary_ref_id,
            build_id: build.build_id,
        })
    })
}

pub(crate) fn file_sha256(path: &Path) -> Result<String, EvalStoreError> {
    let bytes = fs::read(path).map_err(|source| EvalStoreError::Io {
        phase: "binary_ref.read",
        path: path.to_path_buf(),
        source,
    })?;
    Ok(sha256_bytes(&bytes))
}

fn binary_ref_row(evidence: BinaryRefEvidence) -> Result<EvalBinaryRefRow, EvalStoreError> {
    require_non_empty("binary_ref.source_ref", &evidence.source_ref)?;
    if let Some(artifact_id) = &evidence.artifact_id {
        require_non_empty("binary_ref.artifact_id", artifact_id)?;
    }
    if let Some(content_sha256) = &evidence.content_sha256 {
        require_non_empty("binary_ref.content_sha256", content_sha256)?;
    }
    let binary_ref_id = binary_ref_id(
        &evidence.campaign_id,
        evidence.artifact_id.as_deref(),
        evidence.built_by.as_deref(),
        &evidence.source_ref,
        evidence.content_sha256.as_deref(),
        evidence.protocol_digest.as_deref(),
    );
    Ok(EvalBinaryRefRow {
        binary_ref_id,
        campaign_id: evidence.campaign_id.to_string(),
        artifact_id: evidence.artifact_id,
        built_by: evidence.built_by,
        source_ref: evidence.source_ref,
        content_sha256: evidence.content_sha256,
        protocol_digest: evidence.protocol_digest,
        recorded_at: evidence.recorded_at,
    })
}

fn build_event_row(evidence: BuildEventEvidence) -> Result<EvalBuildEventRow, EvalStoreError> {
    require_non_empty("build_event.node_id", &evidence.node_id)?;
    require_non_empty("build_event.phase", &evidence.phase)?;
    require_non_empty("build_event.outcome", &evidence.outcome)?;
    require_non_empty("build_event.recorded_at", &evidence.recorded_at)?;
    if let Some(artifact_id) = &evidence.artifact_id {
        require_non_empty("build_event.artifact_id", artifact_id)?;
    }
    let build_id = build_id(&evidence);
    Ok(EvalBuildEventRow {
        build_id,
        campaign_id: evidence.campaign_id.to_string(),
        node_id: evidence.node_id,
        runtime_id: evidence.runtime_id,
        artifact_id: evidence.artifact_id,
        phase: evidence.phase,
        outcome: evidence.outcome,
        binary_ref: evidence.binary_ref,
        log_ref: evidence.log_ref,
        recorded_at: evidence.recorded_at,
    })
}

fn put_binary_ref_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalBinaryRefRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &BinaryRefSchema::SCHEMA,
        binary_ref_params(row),
        "put.eval_binary_ref",
    )?;
    Ok(())
}

fn put_build_event_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalBuildEventRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &BuildEventSchema::SCHEMA,
        build_event_params(row),
        "put.eval_build_event",
    )?;
    Ok(())
}

fn binary_ref_params(row: &EvalBinaryRefRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "binary_ref_id".to_string(),
        row.binary_ref_id.clone().into(),
    );
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("artifact_id".to_string(), option_string(&row.artifact_id));
    params.insert("built_by".to_string(), option_string(&row.built_by));
    params.insert("source_ref".to_string(), row.source_ref.clone().into());
    params.insert(
        "content_sha256".to_string(),
        option_string(&row.content_sha256),
    );
    params.insert(
        "protocol_digest".to_string(),
        option_string(&row.protocol_digest),
    );
    params.insert("recorded_at".to_string(), option_string(&row.recorded_at));
    params
}

fn build_event_params(row: &EvalBuildEventRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("build_id".to_string(), row.build_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("runtime_id".to_string(), option_string(&row.runtime_id));
    params.insert("artifact_id".to_string(), option_string(&row.artifact_id));
    params.insert("phase".to_string(), row.phase.clone().into());
    params.insert("outcome".to_string(), row.outcome.clone().into());
    params.insert("binary_ref".to_string(), option_string(&row.binary_ref));
    params.insert("log_ref".to_string(), option_string(&row.log_ref));
    params.insert("recorded_at".to_string(), row.recorded_at.clone().into());
    params
}

fn option_string(value: &Option<String>) -> DataValue {
    value
        .clone()
        .map(DataValue::from)
        .unwrap_or(DataValue::Null)
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), EvalStoreError> {
    if value.is_empty() {
        return Err(EvalStoreError::Validation {
            field,
            detail: "must not be empty".to_string(),
        });
    }
    Ok(())
}

fn binary_ref_id(
    campaign_id: &CampaignId,
    artifact_id: Option<&str>,
    built_by: Option<&str>,
    source_ref: &str,
    content_sha256: Option<&str>,
    protocol_digest: Option<&str>,
) -> String {
    hash_parts(&[
        "p1.eval.binary_ref.v1",
        campaign_id.as_str(),
        artifact_id.unwrap_or(""),
        built_by.unwrap_or(""),
        source_ref,
        content_sha256.unwrap_or(""),
        protocol_digest.unwrap_or(""),
    ])
}

fn build_id(evidence: &BuildEventEvidence) -> String {
    hash_parts(&[
        "p1.eval.build_event.v1",
        evidence.campaign_id.as_str(),
        &evidence.node_id,
        evidence.runtime_id.as_deref().unwrap_or(""),
        evidence.artifact_id.as_deref().unwrap_or(""),
        &evidence.phase,
        &evidence.outcome,
        evidence.binary_ref.as_deref().unwrap_or(""),
        evidence.log_ref.as_deref().unwrap_or(""),
        &evidence.recorded_at,
    ])
}
