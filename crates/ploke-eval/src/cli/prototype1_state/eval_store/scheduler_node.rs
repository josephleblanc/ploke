use std::{collections::BTreeMap, fs, path::Path};

use chrono::Utc;
use cozo::DataValue;
use ploke_records::ids::CampaignId;
use serde::Serialize;

use crate::{
    cli::prototype1_state::eval_store::cozo_store::owner_eval_db_file_for_record_path,
    intervention::{Prototype1NodeRecord, Prototype1NodeStatus},
    loop_graph::{ArtifactId, OperationTarget, PatchId},
};

use super::{
    cozo_store::{DbEvalStore, EvalDb, mutate_owner_db},
    error::EvalStoreError,
    evidence::{hash_parts, sha256_bytes},
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

define_eval_schema!(SchedulerNodeSchema {
    "eval_scheduler_node",
    campaign_id: "String",
    node_id: "String" =>
    projection_schema_version: "String",
    node_schema_version: "String",
    parent_node_id: "String?",
    generation: "Int",
    instance_id: "String",
    source_state_id: "String",
    operation_target_kind: "String?",
    base_artifact_id: "String?",
    patch_id: "String?",
    derived_artifact_id: "String?",
    parent_branch_id: "String?",
    branch_id: "String",
    candidate_id: "String",
    target_relpath: "String",
    node_path: "String",
    node_dir: "String",
    workspace_root: "String",
    binary_path: "String",
    runner_request_path: "String",
    runner_result_path: "String",
    status: "String",
    created_at: "String",
    updated_at: "String",
    content_sha256: "String",
    ingested_at: "String",
});

define_eval_schema!(SchedulerNodeStatusSchema {
    "eval_scheduler_node_status_event",
    status_event_id: "String" =>
    campaign_id: "String",
    node_id: "String",
    projection_schema_version: "String",
    node_schema_version: "String",
    generation: "Int",
    branch_id: "String",
    candidate_id: "String",
    target_relpath: "String",
    status: "String",
    node_path: "String",
    content_sha256: "String",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(SchedulerNodeTargetSchema {
    "eval_scheduler_node_target_part",
    campaign_id: "String",
    node_id: "String",
    content_sha256: "String",
    target_part: "String",
    target_index: "Int" =>
    projection_schema_version: "String",
    target_kind: "String",
    artifact_id: "String?",
    patch_id: "String?",
    base_artifact_id: "String?",
});

pub(crate) const SCHEDULER_NODE_SCHEMA_VERSION: &str = "prototype1-scheduler-node.v1";
pub(crate) const SCHEDULER_NODE_REL: &str = SchedulerNodeSchema::RELATION;
pub(crate) const SCHEDULER_NODE_STATUS_REL: &str = SchedulerNodeStatusSchema::RELATION;
pub(crate) const SCHEDULER_NODE_TARGET_REL: &str = SchedulerNodeTargetSchema::RELATION;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SchedulerNodeEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) record_path: std::path::PathBuf,
    pub(crate) record: Prototype1NodeRecord,
}

struct SchedulerNodeRows {
    node: EvalSchedulerNodeRow,
    status: EvalSchedulerNodeStatusRow,
    targets: Vec<EvalSchedulerNodeTargetRow>,
}

struct EvalSchedulerNodeRow {
    campaign_id: String,
    node_id: String,
    projection_schema_version: String,
    node_schema_version: String,
    parent_node_id: Option<String>,
    generation: i64,
    instance_id: String,
    source_state_id: String,
    operation_target_kind: Option<String>,
    base_artifact_id: Option<String>,
    patch_id: Option<String>,
    derived_artifact_id: Option<String>,
    parent_branch_id: Option<String>,
    branch_id: String,
    candidate_id: String,
    target_relpath: String,
    node_path: String,
    node_dir: String,
    workspace_root: String,
    binary_path: String,
    runner_request_path: String,
    runner_result_path: String,
    status: String,
    created_at: String,
    updated_at: String,
    content_sha256: String,
    ingested_at: String,
}

struct EvalSchedulerNodeStatusRow {
    status_event_id: String,
    campaign_id: String,
    node_id: String,
    projection_schema_version: String,
    node_schema_version: String,
    generation: i64,
    branch_id: String,
    candidate_id: String,
    target_relpath: String,
    status: String,
    node_path: String,
    content_sha256: String,
    recorded_at: String,
    ingested_at: String,
}

struct EvalSchedulerNodeTargetRow {
    campaign_id: String,
    node_id: String,
    content_sha256: String,
    target_part: String,
    target_index: i64,
    projection_schema_version: String,
    target_kind: String,
    artifact_id: Option<String>,
    patch_id: Option<String>,
    base_artifact_id: Option<String>,
}

pub(super) fn ensure_scheduler_node_schema<D: EvalDb + ?Sized>(
    db: &D,
) -> Result<(), EvalStoreError> {
    SchedulerNodeSchema::SCHEMA.ensure_installed(db, "schema.eval_scheduler_node")?;
    SchedulerNodeStatusSchema::SCHEMA
        .ensure_installed(db, "schema.eval_scheduler_node_status_event")?;
    SchedulerNodeTargetSchema::SCHEMA
        .ensure_installed(db, "schema.eval_scheduler_node_target_part")?;
    Ok(())
}

pub(crate) fn write_scheduler_node_if_owner_db_exists(
    record_path: &Path,
    record: &Prototype1NodeRecord,
) -> Result<(), EvalStoreError> {
    let Some(campaign_id) = campaign_id_for_record_path(record_path)? else {
        return Ok(());
    };
    let db_path = owner_eval_db_file_for_record_path(record_path)?;
    if !db_path.is_file() {
        return Ok(());
    }
    write_scheduler_node_to_owner_db(
        &db_path,
        SchedulerNodeEvidence {
            campaign_id,
            record_path: record_path.to_path_buf(),
            record: record.clone(),
        },
    )
}

pub(crate) fn write_scheduler_node_to_owner_db(
    db_path: &Path,
    evidence: SchedulerNodeEvidence,
) -> Result<(), EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        DbEvalStore::new(db).install_schema()?;
        let rows = scheduler_node_rows(evidence)?;
        put_scheduler_node_row(db, &rows.node)?;
        put_scheduler_node_status_row(db, &rows.status)?;
        for target in &rows.targets {
            put_scheduler_node_target_row(db, target)?;
        }
        Ok(())
    })
}

fn scheduler_node_rows(
    evidence: SchedulerNodeEvidence,
) -> Result<SchedulerNodeRows, EvalStoreError> {
    let record = evidence.record;
    require_non_empty("scheduler_node.node_id", &record.node_id)?;
    require_non_empty("scheduler_node.node_schema_version", &record.schema_version)?;
    require_non_empty("scheduler_node.instance_id", &record.instance_id)?;
    require_non_empty("scheduler_node.source_state_id", &record.source_state_id)?;
    require_non_empty("scheduler_node.branch_id", &record.branch_id)?;
    require_non_empty("scheduler_node.candidate_id", &record.candidate_id)?;
    let content = fs::read(&evidence.record_path).map_err(|source| EvalStoreError::Io {
        phase: "scheduler_node.read_record",
        path: evidence.record_path.clone(),
        source,
    })?;
    let content_sha256 = sha256_bytes(&content);
    let ingested_at = Utc::now().to_rfc3339();
    let campaign_id = evidence.campaign_id.to_string();
    let node_path = evidence.record_path.display().to_string();
    let status = status_name(record.status)?;
    let target_relpath = record.target_relpath.display().to_string();
    let operation_target_kind = record.operation_target.as_ref().map(operation_target_kind);
    let generation = i64::from(record.generation);
    let node = EvalSchedulerNodeRow {
        campaign_id: campaign_id.clone(),
        node_id: record.node_id.clone(),
        projection_schema_version: SCHEDULER_NODE_SCHEMA_VERSION.to_string(),
        node_schema_version: record.schema_version.clone(),
        parent_node_id: record.parent_node_id.clone(),
        generation,
        instance_id: record.instance_id.clone(),
        source_state_id: record.source_state_id.clone(),
        operation_target_kind,
        base_artifact_id: record.base_artifact_id.as_ref().map(id_string),
        patch_id: record.patch_id.as_ref().map(patch_string),
        derived_artifact_id: record.derived_artifact_id.as_ref().map(id_string),
        parent_branch_id: record.parent_branch_id.clone(),
        branch_id: record.branch_id.clone(),
        candidate_id: record.candidate_id.clone(),
        target_relpath: target_relpath.clone(),
        node_path: node_path.clone(),
        node_dir: record.node_dir.display().to_string(),
        workspace_root: record.workspace_root.display().to_string(),
        binary_path: record.binary_path.display().to_string(),
        runner_request_path: record.runner_request_path.display().to_string(),
        runner_result_path: record.runner_result_path.display().to_string(),
        status: status.clone(),
        created_at: record.created_at.clone(),
        updated_at: record.updated_at.clone(),
        content_sha256: content_sha256.clone(),
        ingested_at: ingested_at.clone(),
    };
    let status_event_id = status_event_id(
        &campaign_id,
        &record.node_id,
        &status,
        &record.updated_at,
        &content_sha256,
    );
    let status_row = EvalSchedulerNodeStatusRow {
        status_event_id,
        campaign_id: campaign_id.clone(),
        node_id: record.node_id.clone(),
        projection_schema_version: SCHEDULER_NODE_SCHEMA_VERSION.to_string(),
        node_schema_version: record.schema_version.clone(),
        generation,
        branch_id: record.branch_id.clone(),
        candidate_id: record.candidate_id.clone(),
        target_relpath,
        status,
        node_path,
        content_sha256: content_sha256.clone(),
        recorded_at: record.updated_at.clone(),
        ingested_at: ingested_at.clone(),
    };
    let targets = target_rows(&campaign_id, &content_sha256, &record)?;
    Ok(SchedulerNodeRows {
        node,
        status: status_row,
        targets,
    })
}

fn target_rows(
    campaign_id: &str,
    content_sha256: &str,
    record: &Prototype1NodeRecord,
) -> Result<Vec<EvalSchedulerNodeTargetRow>, EvalStoreError> {
    let Some(target) = record.operation_target.as_ref() else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    match target {
        OperationTarget::Artifact { artifact_id } => rows.push(target_row(
            campaign_id,
            &record.node_id,
            content_sha256,
            "artifact",
            0,
            "artifact",
            Some(artifact_id.as_str()),
            None,
            None,
        )?),
        OperationTarget::PatchSet {
            base_artifact_id,
            patch_ids,
        } => {
            rows.push(target_row(
                campaign_id,
                &record.node_id,
                content_sha256,
                "base",
                0,
                "patch_set",
                None,
                None,
                Some(base_artifact_id.as_str()),
            )?);
            for (index, patch_id) in patch_ids.iter().enumerate() {
                rows.push(target_row(
                    campaign_id,
                    &record.node_id,
                    content_sha256,
                    "patch",
                    index,
                    "patch_set",
                    None,
                    Some(patch_id.as_str()),
                    Some(base_artifact_id.as_str()),
                )?);
            }
        }
        OperationTarget::ArtifactSet {
            base_artifact_id,
            artifact_ids,
        } => {
            if let Some(base_artifact_id) = base_artifact_id {
                rows.push(target_row(
                    campaign_id,
                    &record.node_id,
                    content_sha256,
                    "base",
                    0,
                    "artifact_set",
                    None,
                    None,
                    Some(base_artifact_id.as_str()),
                )?);
            }
            for (index, artifact_id) in artifact_ids.iter().enumerate() {
                rows.push(target_row(
                    campaign_id,
                    &record.node_id,
                    content_sha256,
                    "artifact",
                    index,
                    "artifact_set",
                    Some(artifact_id.as_str()),
                    None,
                    base_artifact_id.as_ref().map(ArtifactId::as_str),
                )?);
            }
        }
    }
    Ok(rows)
}

#[allow(clippy::too_many_arguments)]
fn target_row(
    campaign_id: &str,
    node_id: &str,
    content_sha256: &str,
    target_part: &str,
    index: usize,
    target_kind: &str,
    artifact_id: Option<&str>,
    patch_id: Option<&str>,
    base_artifact_id: Option<&str>,
) -> Result<EvalSchedulerNodeTargetRow, EvalStoreError> {
    Ok(EvalSchedulerNodeTargetRow {
        campaign_id: campaign_id.to_string(),
        node_id: node_id.to_string(),
        content_sha256: content_sha256.to_string(),
        target_part: target_part.to_string(),
        target_index: usize_to_i64(index, "scheduler_node_target.target_index")?,
        projection_schema_version: SCHEDULER_NODE_SCHEMA_VERSION.to_string(),
        target_kind: target_kind.to_string(),
        artifact_id: artifact_id.map(ToOwned::to_owned),
        patch_id: patch_id.map(ToOwned::to_owned),
        base_artifact_id: base_artifact_id.map(ToOwned::to_owned),
    })
}

fn put_scheduler_node_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalSchedulerNodeRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &SchedulerNodeSchema::SCHEMA,
        scheduler_node_params(row),
        "put.eval_scheduler_node",
    )
}

fn put_scheduler_node_status_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalSchedulerNodeStatusRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &SchedulerNodeStatusSchema::SCHEMA,
        scheduler_node_status_params(row),
        "put.eval_scheduler_node_status_event",
    )
}

fn put_scheduler_node_target_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalSchedulerNodeTargetRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &SchedulerNodeTargetSchema::SCHEMA,
        scheduler_node_target_params(row),
        "put.eval_scheduler_node_target_part",
    )
}

fn scheduler_node_params(row: &EvalSchedulerNodeRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert(
        "projection_schema_version".to_string(),
        row.projection_schema_version.clone().into(),
    );
    params.insert(
        "node_schema_version".to_string(),
        row.node_schema_version.clone().into(),
    );
    params.insert(
        "parent_node_id".to_string(),
        option_string(&row.parent_node_id),
    );
    params.insert("generation".to_string(), row.generation.into());
    params.insert("instance_id".to_string(), row.instance_id.clone().into());
    params.insert(
        "source_state_id".to_string(),
        row.source_state_id.clone().into(),
    );
    params.insert(
        "operation_target_kind".to_string(),
        option_string(&row.operation_target_kind),
    );
    params.insert(
        "base_artifact_id".to_string(),
        option_string(&row.base_artifact_id),
    );
    params.insert("patch_id".to_string(), option_string(&row.patch_id));
    params.insert(
        "derived_artifact_id".to_string(),
        option_string(&row.derived_artifact_id),
    );
    params.insert(
        "parent_branch_id".to_string(),
        option_string(&row.parent_branch_id),
    );
    params.insert("branch_id".to_string(), row.branch_id.clone().into());
    params.insert("candidate_id".to_string(), row.candidate_id.clone().into());
    params.insert(
        "target_relpath".to_string(),
        row.target_relpath.clone().into(),
    );
    params.insert("node_path".to_string(), row.node_path.clone().into());
    params.insert("node_dir".to_string(), row.node_dir.clone().into());
    params.insert(
        "workspace_root".to_string(),
        row.workspace_root.clone().into(),
    );
    params.insert("binary_path".to_string(), row.binary_path.clone().into());
    params.insert(
        "runner_request_path".to_string(),
        row.runner_request_path.clone().into(),
    );
    params.insert(
        "runner_result_path".to_string(),
        row.runner_result_path.clone().into(),
    );
    params.insert("status".to_string(), row.status.clone().into());
    params.insert("created_at".to_string(), row.created_at.clone().into());
    params.insert("updated_at".to_string(), row.updated_at.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

fn scheduler_node_status_params(row: &EvalSchedulerNodeStatusRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "status_event_id".to_string(),
        row.status_event_id.clone().into(),
    );
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert(
        "projection_schema_version".to_string(),
        row.projection_schema_version.clone().into(),
    );
    params.insert(
        "node_schema_version".to_string(),
        row.node_schema_version.clone().into(),
    );
    params.insert("generation".to_string(), row.generation.into());
    params.insert("branch_id".to_string(), row.branch_id.clone().into());
    params.insert("candidate_id".to_string(), row.candidate_id.clone().into());
    params.insert(
        "target_relpath".to_string(),
        row.target_relpath.clone().into(),
    );
    params.insert("status".to_string(), row.status.clone().into());
    params.insert("node_path".to_string(), row.node_path.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("recorded_at".to_string(), row.recorded_at.clone().into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

fn scheduler_node_target_params(row: &EvalSchedulerNodeTargetRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("target_part".to_string(), row.target_part.clone().into());
    params.insert("target_index".to_string(), row.target_index.into());
    params.insert(
        "projection_schema_version".to_string(),
        row.projection_schema_version.clone().into(),
    );
    params.insert("target_kind".to_string(), row.target_kind.clone().into());
    params.insert("artifact_id".to_string(), option_string(&row.artifact_id));
    params.insert("patch_id".to_string(), option_string(&row.patch_id));
    params.insert(
        "base_artifact_id".to_string(),
        option_string(&row.base_artifact_id),
    );
    params
}

fn campaign_id_for_record_path(record_path: &Path) -> Result<Option<CampaignId>, EvalStoreError> {
    let Some(prototype_root) = record_path
        .ancestors()
        .find(|ancestor| ancestor.file_name().and_then(|name| name.to_str()) == Some("prototype1"))
    else {
        return Ok(None);
    };
    let campaign_dir = prototype_root
        .parent()
        .ok_or_else(|| EvalStoreError::Validation {
            field: "scheduler_node.campaign_id",
            detail: format!(
                "record '{}' is under prototype1 but has no campaign directory",
                record_path.display()
            ),
        })?;
    let campaign_id = campaign_dir
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| EvalStoreError::Validation {
            field: "scheduler_node.campaign_id",
            detail: format!(
                "record '{}' has a non-utf8 campaign directory",
                record_path.display()
            ),
        })?;
    require_non_empty("scheduler_node.campaign_id", campaign_id)?;
    Ok(Some(CampaignId::from(campaign_id.to_string())))
}

fn operation_target_kind(target: &OperationTarget) -> String {
    match target {
        OperationTarget::Artifact { .. } => "artifact".to_string(),
        OperationTarget::PatchSet { .. } => "patch_set".to_string(),
        OperationTarget::ArtifactSet { .. } => "artifact_set".to_string(),
    }
}

fn status_event_id(
    campaign_id: &str,
    node_id: &str,
    status: &str,
    updated_at: &str,
    content_sha256: &str,
) -> String {
    hash_parts(&[
        "p1.eval.scheduler_node.status.v1",
        campaign_id,
        node_id,
        status,
        updated_at,
        content_sha256,
    ])
}

fn status_name(status: Prototype1NodeStatus) -> Result<String, EvalStoreError> {
    serde_name(&status, "scheduler_node.status")
}

fn serde_name<T: Serialize>(value: &T, field: &'static str) -> Result<String, EvalStoreError> {
    match serde_json::to_value(value).map_err(|source| EvalStoreError::Validation {
        field,
        detail: source.to_string(),
    })? {
        serde_json::Value::String(value) => Ok(value),
        other => Err(EvalStoreError::Validation {
            field,
            detail: format!("expected serde string, got {other}"),
        }),
    }
}

fn id_string(id: &ArtifactId) -> String {
    id.as_str().to_string()
}

fn patch_string(id: &PatchId) -> String {
    id.as_str().to_string()
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), EvalStoreError> {
    if value.is_empty() {
        return Err(EvalStoreError::Validation {
            field,
            detail: "required eval-store field is empty".to_string(),
        });
    }
    Ok(())
}

fn usize_to_i64(value: usize, field: &'static str) -> Result<i64, EvalStoreError> {
    i64::try_from(value).map_err(|_| EvalStoreError::Validation {
        field,
        detail: format!("value {value} does not fit in Cozo Int"),
    })
}

fn option_string(value: &Option<String>) -> DataValue {
    value
        .clone()
        .map(DataValue::from)
        .unwrap_or(DataValue::Null)
}
