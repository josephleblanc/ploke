use std::{collections::BTreeMap, path::Path};

use cozo::DataValue;
use ploke_records::ids::CampaignId;

use super::{
    cozo_store::{EvalDb, mutate_owner_db},
    error::EvalStoreError,
    evidence::{hash_parts, sha256_bytes},
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

define_eval_schema!(OperationSchema {
    "eval_operation",
    operation_id: "String" =>
    campaign_id: "String",
    generator_id: "String",
    target_kind: "String",
    target_ref: "String",
    procedure_id: "String?",
    output_artifact_id: "String?",
    output_patch_id: "String?",
    recorded_at: "String?",
});

define_eval_schema!(PatchSchema {
    "eval_patch",
    patch_id: "String" =>
    campaign_id: "String",
    base_artifact_id: "String?",
    creator_id: "String?",
    tool_call_id: "String?",
    target_relpath: "String?",
    patch_ref: "String?",
    content_sha256: "String?",
    status: "String?",
});

define_eval_schema!(ApplyEventSchema {
    "eval_apply_event",
    apply_id: "String" =>
    campaign_id: "String",
    patch_id: "String",
    runtime_id: "String?",
    artifact_id: "String?",
    outcome: "String",
    output_artifact_id: "String?",
    recorded_at: "String",
});

pub(crate) const OPERATION_REL: &str = OperationSchema::RELATION;
pub(crate) const PATCH_REL: &str = PatchSchema::RELATION;
pub(crate) const APPLY_EVENT_REL: &str = ApplyEventSchema::RELATION;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OperationEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) generator_id: String,
    pub(crate) target_kind: String,
    pub(crate) target_ref: String,
    pub(crate) procedure_id: Option<String>,
    pub(crate) output_artifact_id: Option<String>,
    pub(crate) output_patch_id: Option<String>,
    pub(crate) recorded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PatchEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) patch_id: String,
    pub(crate) base_artifact_id: Option<String>,
    pub(crate) creator_id: Option<String>,
    pub(crate) tool_call_id: Option<String>,
    pub(crate) target_relpath: Option<String>,
    pub(crate) patch_ref: Option<String>,
    pub(crate) content_sha256: Option<String>,
    pub(crate) status: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApplyEventEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) apply_id: String,
    pub(crate) patch_id: String,
    pub(crate) runtime_id: Option<String>,
    pub(crate) artifact_id: Option<String>,
    pub(crate) outcome: String,
    pub(crate) output_artifact_id: Option<String>,
    pub(crate) recorded_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OperationProvenanceEvidence {
    pub(crate) operation: OperationEvidence,
    pub(crate) patch: PatchEvidence,
    pub(crate) apply_event: ApplyEventEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OperationProvenanceReceipt {
    pub(crate) operation_id: String,
    pub(crate) patch_id: String,
    pub(crate) apply_id: String,
}

struct EvalOperationRow {
    operation_id: String,
    campaign_id: String,
    generator_id: String,
    target_kind: String,
    target_ref: String,
    procedure_id: Option<String>,
    output_artifact_id: Option<String>,
    output_patch_id: Option<String>,
    recorded_at: Option<String>,
}

struct EvalPatchRow {
    patch_id: String,
    campaign_id: String,
    base_artifact_id: Option<String>,
    creator_id: Option<String>,
    tool_call_id: Option<String>,
    target_relpath: Option<String>,
    patch_ref: Option<String>,
    content_sha256: Option<String>,
    status: Option<String>,
}

struct EvalApplyEventRow {
    apply_id: String,
    campaign_id: String,
    patch_id: String,
    runtime_id: Option<String>,
    artifact_id: Option<String>,
    outcome: String,
    output_artifact_id: Option<String>,
    recorded_at: String,
}

pub(super) fn ensure_operation_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    OperationSchema::SCHEMA.ensure_installed(db, "schema.eval_operation")?;
    PatchSchema::SCHEMA.ensure_installed(db, "schema.eval_patch")?;
    ApplyEventSchema::SCHEMA.ensure_installed(db, "schema.eval_apply_event")?;
    Ok(())
}

pub(crate) fn write_operation_provenance_to_owner_db(
    db_path: &Path,
    evidence: OperationProvenanceEvidence,
) -> Result<OperationProvenanceReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        ensure_operation_schema(db)?;
        let operation = operation_row(evidence.operation)?;
        let patch = patch_row(evidence.patch)?;
        let apply_event = apply_event_row(evidence.apply_event)?;
        put_operation_row(db, &operation)?;
        put_patch_row(db, &patch)?;
        put_apply_event_row(db, &apply_event)?;
        Ok(OperationProvenanceReceipt {
            operation_id: operation.operation_id,
            patch_id: patch.patch_id,
            apply_id: apply_event.apply_id,
        })
    })
}

pub(crate) fn content_sha256(content: &str) -> String {
    sha256_bytes(content.as_bytes())
}

fn operation_row(evidence: OperationEvidence) -> Result<EvalOperationRow, EvalStoreError> {
    require_non_empty("operation.generator_id", &evidence.generator_id)?;
    require_non_empty("operation.target_kind", &evidence.target_kind)?;
    require_non_empty("operation.target_ref", &evidence.target_ref)?;
    if let Some(output_artifact_id) = &evidence.output_artifact_id {
        require_non_empty("operation.output_artifact_id", output_artifact_id)?;
    }
    if let Some(output_patch_id) = &evidence.output_patch_id {
        require_non_empty("operation.output_patch_id", output_patch_id)?;
    }
    let operation_id = operation_id(&evidence);
    Ok(EvalOperationRow {
        operation_id,
        campaign_id: evidence.campaign_id.to_string(),
        generator_id: evidence.generator_id,
        target_kind: evidence.target_kind,
        target_ref: evidence.target_ref,
        procedure_id: evidence.procedure_id,
        output_artifact_id: evidence.output_artifact_id,
        output_patch_id: evidence.output_patch_id,
        recorded_at: evidence.recorded_at,
    })
}

fn patch_row(evidence: PatchEvidence) -> Result<EvalPatchRow, EvalStoreError> {
    require_non_empty("patch.patch_id", &evidence.patch_id)?;
    if let Some(base_artifact_id) = &evidence.base_artifact_id {
        require_non_empty("patch.base_artifact_id", base_artifact_id)?;
    }
    if let Some(target_relpath) = &evidence.target_relpath {
        require_non_empty("patch.target_relpath", target_relpath)?;
    }
    if let Some(content_sha256) = &evidence.content_sha256 {
        require_non_empty("patch.content_sha256", content_sha256)?;
    }
    Ok(EvalPatchRow {
        patch_id: evidence.patch_id,
        campaign_id: evidence.campaign_id.to_string(),
        base_artifact_id: evidence.base_artifact_id,
        creator_id: evidence.creator_id,
        tool_call_id: evidence.tool_call_id,
        target_relpath: evidence.target_relpath,
        patch_ref: evidence.patch_ref,
        content_sha256: evidence.content_sha256,
        status: evidence.status,
    })
}

fn apply_event_row(evidence: ApplyEventEvidence) -> Result<EvalApplyEventRow, EvalStoreError> {
    require_non_empty("apply_event.apply_id", &evidence.apply_id)?;
    require_non_empty("apply_event.patch_id", &evidence.patch_id)?;
    require_non_empty("apply_event.outcome", &evidence.outcome)?;
    require_non_empty("apply_event.recorded_at", &evidence.recorded_at)?;
    if let Some(output_artifact_id) = &evidence.output_artifact_id {
        require_non_empty("apply_event.output_artifact_id", output_artifact_id)?;
    }
    Ok(EvalApplyEventRow {
        apply_id: evidence.apply_id,
        campaign_id: evidence.campaign_id.to_string(),
        patch_id: evidence.patch_id,
        runtime_id: evidence.runtime_id,
        artifact_id: evidence.artifact_id,
        outcome: evidence.outcome,
        output_artifact_id: evidence.output_artifact_id,
        recorded_at: evidence.recorded_at,
    })
}

fn put_operation_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalOperationRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &OperationSchema::SCHEMA,
        operation_params(row),
        "put.eval_operation",
    )?;
    Ok(())
}

fn put_patch_row<D: EvalDb + ?Sized>(db: &D, row: &EvalPatchRow) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &PatchSchema::SCHEMA,
        patch_params(row),
        "put.eval_patch",
    )?;
    Ok(())
}

fn put_apply_event_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalApplyEventRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &ApplyEventSchema::SCHEMA,
        apply_event_params(row),
        "put.eval_apply_event",
    )?;
    Ok(())
}

fn operation_params(row: &EvalOperationRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("operation_id".to_string(), row.operation_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("generator_id".to_string(), row.generator_id.clone().into());
    params.insert("target_kind".to_string(), row.target_kind.clone().into());
    params.insert("target_ref".to_string(), row.target_ref.clone().into());
    params.insert("procedure_id".to_string(), option_string(&row.procedure_id));
    params.insert(
        "output_artifact_id".to_string(),
        option_string(&row.output_artifact_id),
    );
    params.insert(
        "output_patch_id".to_string(),
        option_string(&row.output_patch_id),
    );
    params.insert("recorded_at".to_string(), option_string(&row.recorded_at));
    params
}

fn patch_params(row: &EvalPatchRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("patch_id".to_string(), row.patch_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert(
        "base_artifact_id".to_string(),
        option_string(&row.base_artifact_id),
    );
    params.insert("creator_id".to_string(), option_string(&row.creator_id));
    params.insert("tool_call_id".to_string(), option_string(&row.tool_call_id));
    params.insert(
        "target_relpath".to_string(),
        option_string(&row.target_relpath),
    );
    params.insert("patch_ref".to_string(), option_string(&row.patch_ref));
    params.insert(
        "content_sha256".to_string(),
        option_string(&row.content_sha256),
    );
    params.insert("status".to_string(), option_string(&row.status));
    params
}

fn apply_event_params(row: &EvalApplyEventRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("apply_id".to_string(), row.apply_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("patch_id".to_string(), row.patch_id.clone().into());
    params.insert("runtime_id".to_string(), option_string(&row.runtime_id));
    params.insert("artifact_id".to_string(), option_string(&row.artifact_id));
    params.insert("outcome".to_string(), row.outcome.clone().into());
    params.insert(
        "output_artifact_id".to_string(),
        option_string(&row.output_artifact_id),
    );
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

fn operation_id(evidence: &OperationEvidence) -> String {
    hash_parts(&[
        "p1.eval.operation.v1",
        evidence.campaign_id.as_str(),
        &evidence.generator_id,
        &evidence.target_kind,
        &evidence.target_ref,
        evidence.procedure_id.as_deref().unwrap_or(""),
        evidence.output_artifact_id.as_deref().unwrap_or(""),
        evidence.output_patch_id.as_deref().unwrap_or(""),
    ])
}
