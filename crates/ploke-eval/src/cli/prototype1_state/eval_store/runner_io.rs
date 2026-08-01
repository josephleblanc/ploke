use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use cozo::DataValue;
use serde::Serialize;

use crate::{
    intervention::{
        Prototype1NodeStatus, Prototype1RunnerDisposition, Prototype1RunnerRequest,
        Prototype1RunnerResult,
    },
    loop_graph::{ArtifactId, OperationTarget, PatchId},
};

use super::{
    cozo_store::{DbEvalStore, EvalDb, mutate_owner_db, owner_eval_db_file_for_record_path},
    error::EvalStoreError,
    evidence::sha256_bytes,
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

define_eval_schema!(RunnerRequestSchema {
    "eval_runner_request",
    campaign_id: "String",
    node_id: "String" =>
    projection_schema_version: "String",
    request_schema_version: "String",
    generation: "Int",
    instance_id: "String",
    source_state_id: "String",
    operation_target_kind: "String?",
    base_artifact_id: "String?",
    patch_id: "String?",
    derived_artifact_id: "String?",
    branch_id: "String",
    target_relpath: "String",
    workspace_root: "String",
    binary_path: "String",
    request_path: "String",
    stop_on_error: "Bool",
    runner_arg_count: "Int",
    content_sha256: "String",
    ingested_at: "String",
});

define_eval_schema!(RunnerRequestArgSchema {
    "eval_runner_request_arg",
    campaign_id: "String",
    node_id: "String",
    content_sha256: "String",
    arg_index: "Int" =>
    projection_schema_version: "String",
    arg_value: "String",
});

define_eval_schema!(RunnerRequestTargetSchema {
    "eval_runner_request_target_part",
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

define_eval_schema!(RunnerResultSchema {
    "eval_runner_result",
    campaign_id: "String",
    node_id: "String",
    result_path: "String" =>
    projection_schema_version: "String",
    result_schema_version: "String",
    generation: "Int",
    branch_id: "String",
    status: "String",
    disposition: "String",
    treatment_campaign_id: "String?",
    evaluation_artifact_path: "String?",
    detail: "String?",
    exit_code: "Int?",
    stdout_excerpt: "String?",
    stderr_excerpt: "String?",
    runtime_id: "String?",
    path_kind: "String",
    content_sha256: "String",
    recorded_at: "String",
    ingested_at: "String",
});

pub(crate) const RUNNER_REQUEST_SCHEMA_VERSION: &str = "prototype1-runner-request.v1";
pub(crate) const RUNNER_RESULT_SCHEMA_VERSION: &str = "prototype1-runner-result.v1";
pub(crate) const RUNNER_REQUEST_REL: &str = RunnerRequestSchema::RELATION;
pub(crate) const RUNNER_REQUEST_ARG_REL: &str = RunnerRequestArgSchema::RELATION;
pub(crate) const RUNNER_REQUEST_TARGET_REL: &str = RunnerRequestTargetSchema::RELATION;
pub(crate) const RUNNER_RESULT_REL: &str = RunnerResultSchema::RELATION;

struct RequestRows {
    request: RequestRow,
    args: Vec<RequestArgRow>,
    targets: Vec<RequestTargetRow>,
}

struct RequestRow {
    campaign_id: String,
    node_id: String,
    projection_schema_version: String,
    request_schema_version: String,
    generation: i64,
    instance_id: String,
    source_state_id: String,
    operation_target_kind: Option<String>,
    base_artifact_id: Option<String>,
    patch_id: Option<String>,
    derived_artifact_id: Option<String>,
    branch_id: String,
    target_relpath: String,
    workspace_root: String,
    binary_path: String,
    request_path: String,
    stop_on_error: bool,
    runner_arg_count: i64,
    content_sha256: String,
    ingested_at: String,
}

struct RequestArgRow {
    campaign_id: String,
    node_id: String,
    content_sha256: String,
    arg_index: i64,
    projection_schema_version: String,
    arg_value: String,
}

struct RequestTargetRow {
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

struct ResultRow {
    campaign_id: String,
    node_id: String,
    result_path: String,
    projection_schema_version: String,
    result_schema_version: String,
    generation: i64,
    branch_id: String,
    status: String,
    disposition: String,
    treatment_campaign_id: Option<String>,
    evaluation_artifact_path: Option<String>,
    detail: Option<String>,
    exit_code: Option<i64>,
    stdout_excerpt: Option<String>,
    stderr_excerpt: Option<String>,
    runtime_id: Option<String>,
    path_kind: String,
    content_sha256: String,
    recorded_at: String,
    ingested_at: String,
}

pub(super) fn ensure_runner_io_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    RunnerRequestSchema::SCHEMA.ensure_installed(db, "schema.eval_runner_request")?;
    RunnerRequestArgSchema::SCHEMA.ensure_installed(db, "schema.eval_runner_request_arg")?;
    RunnerRequestTargetSchema::SCHEMA
        .ensure_installed(db, "schema.eval_runner_request_target_part")?;
    RunnerResultSchema::SCHEMA.ensure_installed(db, "schema.eval_runner_result")?;
    Ok(())
}

pub(crate) fn write_runner_request_if_owner_db_exists(
    request_path: &Path,
    request: &Prototype1RunnerRequest,
) -> Result<(), EvalStoreError> {
    let Some(db_path) = owner_db_path(request_path)? else {
        return Ok(());
    };
    if !db_path.is_file() {
        return Ok(());
    }
    mutate_owner_db(&db_path, |db| {
        DbEvalStore::new(db).install_schema()?;
        let rows = request_rows(request_path, request)?;
        put_request_row(db, &rows.request)?;
        for arg in &rows.args {
            put_request_arg_row(db, arg)?;
        }
        for target in &rows.targets {
            put_request_target_row(db, target)?;
        }
        Ok(())
    })
}

pub(crate) fn write_runner_result_if_owner_db_exists(
    result_path: &Path,
    result: &Prototype1RunnerResult,
) -> Result<(), EvalStoreError> {
    let Some(db_path) = owner_db_path(result_path)? else {
        return Ok(());
    };
    if !db_path.is_file() {
        return Ok(());
    }
    mutate_owner_db(&db_path, |db| {
        DbEvalStore::new(db).install_schema()?;
        let row = result_row(result_path, result)?;
        put_result_row(db, &row)
    })
}

fn request_rows(
    request_path: &Path,
    request: &Prototype1RunnerRequest,
) -> Result<RequestRows, EvalStoreError> {
    require_non_empty("runner_request.node_id", &request.node_id)?;
    require_non_empty("runner_request.schema_version", &request.schema_version)?;
    require_non_empty("runner_request.instance_id", &request.instance_id)?;
    require_non_empty("runner_request.source_state_id", &request.source_state_id)?;
    require_non_empty("runner_request.branch_id", &request.branch_id)?;
    let content = fs::read(request_path).map_err(|source| EvalStoreError::Io {
        phase: "runner_request.read_record",
        path: request_path.to_path_buf(),
        source,
    })?;
    let content_sha256 = sha256_bytes(&content);
    let campaign_id = request.campaign_id.to_string();
    let node_id = request.node_id.clone();
    let ingested_at = Utc::now().to_rfc3339();
    let request_row = RequestRow {
        campaign_id: campaign_id.clone(),
        node_id: node_id.clone(),
        projection_schema_version: RUNNER_REQUEST_SCHEMA_VERSION.to_string(),
        request_schema_version: request.schema_version.clone(),
        generation: i64::from(request.generation),
        instance_id: request.instance_id.clone(),
        source_state_id: request.source_state_id.clone(),
        operation_target_kind: request.operation_target.as_ref().map(operation_target_kind),
        base_artifact_id: request.base_artifact_id.as_ref().map(id_string),
        patch_id: request.patch_id.as_ref().map(patch_string),
        derived_artifact_id: request.derived_artifact_id.as_ref().map(id_string),
        branch_id: request.branch_id.clone(),
        target_relpath: request.target_relpath.display().to_string(),
        workspace_root: request.workspace_root.display().to_string(),
        binary_path: request.binary_path.display().to_string(),
        request_path: request_path.display().to_string(),
        stop_on_error: request.stop_on_error,
        runner_arg_count: usize_to_i64(
            request.runner_args.len(),
            "runner_request.runner_arg_count",
        )?,
        content_sha256: content_sha256.clone(),
        ingested_at,
    };
    let args = request
        .runner_args
        .iter()
        .enumerate()
        .map(|(index, value)| {
            Ok(RequestArgRow {
                campaign_id: campaign_id.clone(),
                node_id: node_id.clone(),
                content_sha256: content_sha256.clone(),
                arg_index: usize_to_i64(index, "runner_request_arg.arg_index")?,
                projection_schema_version: RUNNER_REQUEST_SCHEMA_VERSION.to_string(),
                arg_value: value.clone(),
            })
        })
        .collect::<Result<Vec<_>, EvalStoreError>>()?;
    let targets = request_target_rows(&campaign_id, &node_id, &content_sha256, request)?;
    Ok(RequestRows {
        request: request_row,
        args,
        targets,
    })
}

fn result_row(
    result_path: &Path,
    result: &Prototype1RunnerResult,
) -> Result<ResultRow, EvalStoreError> {
    require_non_empty("runner_result.node_id", &result.node_id)?;
    require_non_empty("runner_result.schema_version", &result.schema_version)?;
    require_non_empty("runner_result.branch_id", &result.branch_id)?;
    let content = fs::read(result_path).map_err(|source| EvalStoreError::Io {
        phase: "runner_result.read_record",
        path: result_path.to_path_buf(),
        source,
    })?;
    let content_sha256 = sha256_bytes(&content);
    Ok(ResultRow {
        campaign_id: result.campaign_id.to_string(),
        node_id: result.node_id.clone(),
        result_path: result_path.display().to_string(),
        projection_schema_version: RUNNER_RESULT_SCHEMA_VERSION.to_string(),
        result_schema_version: result.schema_version.clone(),
        generation: i64::from(result.generation),
        branch_id: result.branch_id.clone(),
        status: status_name(result.status)?,
        disposition: disposition_name(result.disposition)?,
        treatment_campaign_id: result
            .treatment_campaign_id
            .as_ref()
            .map(ToString::to_string),
        evaluation_artifact_path: result
            .evaluation_artifact_path
            .as_ref()
            .map(|path| path.display().to_string()),
        detail: result.detail.clone(),
        exit_code: result.exit_code.map(i64::from),
        stdout_excerpt: result.stdout_excerpt.clone(),
        stderr_excerpt: result.stderr_excerpt.clone(),
        runtime_id: runtime_id_from_result_path(result_path),
        path_kind: result_path_kind(result_path),
        content_sha256,
        recorded_at: result.recorded_at.clone(),
        ingested_at: Utc::now().to_rfc3339(),
    })
}

fn request_target_rows(
    campaign_id: &str,
    node_id: &str,
    content_sha256: &str,
    request: &Prototype1RunnerRequest,
) -> Result<Vec<RequestTargetRow>, EvalStoreError> {
    let Some(target) = request.operation_target.as_ref() else {
        return Ok(Vec::new());
    };
    let mut rows = Vec::new();
    match target {
        OperationTarget::Artifact { artifact_id } => rows.push(request_target_row(
            campaign_id,
            node_id,
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
            rows.push(request_target_row(
                campaign_id,
                node_id,
                content_sha256,
                "base",
                0,
                "patch_set",
                None,
                None,
                Some(base_artifact_id.as_str()),
            )?);
            for (index, patch_id) in patch_ids.iter().enumerate() {
                rows.push(request_target_row(
                    campaign_id,
                    node_id,
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
                rows.push(request_target_row(
                    campaign_id,
                    node_id,
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
                rows.push(request_target_row(
                    campaign_id,
                    node_id,
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
fn request_target_row(
    campaign_id: &str,
    node_id: &str,
    content_sha256: &str,
    target_part: &str,
    index: usize,
    target_kind: &str,
    artifact_id: Option<&str>,
    patch_id: Option<&str>,
    base_artifact_id: Option<&str>,
) -> Result<RequestTargetRow, EvalStoreError> {
    Ok(RequestTargetRow {
        campaign_id: campaign_id.to_string(),
        node_id: node_id.to_string(),
        content_sha256: content_sha256.to_string(),
        target_part: target_part.to_string(),
        target_index: usize_to_i64(index, "runner_request_target.target_index")?,
        projection_schema_version: RUNNER_REQUEST_SCHEMA_VERSION.to_string(),
        target_kind: target_kind.to_string(),
        artifact_id: artifact_id.map(ToOwned::to_owned),
        patch_id: patch_id.map(ToOwned::to_owned),
        base_artifact_id: base_artifact_id.map(ToOwned::to_owned),
    })
}

fn put_request_row<D: EvalDb + ?Sized>(db: &D, row: &RequestRow) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &RunnerRequestSchema::SCHEMA,
        request_params(row),
        "put.eval_runner_request",
    )
}

fn put_request_arg_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &RequestArgRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &RunnerRequestArgSchema::SCHEMA,
        request_arg_params(row),
        "put.eval_runner_request_arg",
    )
}

fn put_request_target_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &RequestTargetRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &RunnerRequestTargetSchema::SCHEMA,
        request_target_params(row),
        "put.eval_runner_request_target_part",
    )
}

fn put_result_row<D: EvalDb + ?Sized>(db: &D, row: &ResultRow) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &RunnerResultSchema::SCHEMA,
        result_params(row),
        "put.eval_runner_result",
    )
}

fn request_params(row: &RequestRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert(
        "projection_schema_version".to_string(),
        row.projection_schema_version.clone().into(),
    );
    params.insert(
        "request_schema_version".to_string(),
        row.request_schema_version.clone().into(),
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
    params.insert("branch_id".to_string(), row.branch_id.clone().into());
    params.insert(
        "target_relpath".to_string(),
        row.target_relpath.clone().into(),
    );
    params.insert(
        "workspace_root".to_string(),
        row.workspace_root.clone().into(),
    );
    params.insert("binary_path".to_string(), row.binary_path.clone().into());
    params.insert("request_path".to_string(), row.request_path.clone().into());
    params.insert("stop_on_error".to_string(), row.stop_on_error.into());
    params.insert("runner_arg_count".to_string(), row.runner_arg_count.into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

fn request_arg_params(row: &RequestArgRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("arg_index".to_string(), row.arg_index.into());
    params.insert(
        "projection_schema_version".to_string(),
        row.projection_schema_version.clone().into(),
    );
    params.insert("arg_value".to_string(), row.arg_value.clone().into());
    params
}

fn request_target_params(row: &RequestTargetRow) -> BTreeMap<String, DataValue> {
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

fn result_params(row: &ResultRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("result_path".to_string(), row.result_path.clone().into());
    params.insert(
        "projection_schema_version".to_string(),
        row.projection_schema_version.clone().into(),
    );
    params.insert(
        "result_schema_version".to_string(),
        row.result_schema_version.clone().into(),
    );
    params.insert("generation".to_string(), row.generation.into());
    params.insert("branch_id".to_string(), row.branch_id.clone().into());
    params.insert("status".to_string(), row.status.clone().into());
    params.insert("disposition".to_string(), row.disposition.clone().into());
    params.insert(
        "treatment_campaign_id".to_string(),
        option_string(&row.treatment_campaign_id),
    );
    params.insert(
        "evaluation_artifact_path".to_string(),
        option_string(&row.evaluation_artifact_path),
    );
    params.insert("detail".to_string(), option_string(&row.detail));
    params.insert("exit_code".to_string(), option_i64(row.exit_code));
    params.insert(
        "stdout_excerpt".to_string(),
        option_string(&row.stdout_excerpt),
    );
    params.insert(
        "stderr_excerpt".to_string(),
        option_string(&row.stderr_excerpt),
    );
    params.insert("runtime_id".to_string(), option_string(&row.runtime_id));
    params.insert("path_kind".to_string(), row.path_kind.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("recorded_at".to_string(), row.recorded_at.clone().into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

fn owner_db_path(path: &Path) -> Result<Option<PathBuf>, EvalStoreError> {
    if !path
        .ancestors()
        .any(|ancestor| ancestor.file_name().and_then(|name| name.to_str()) == Some("prototype1"))
    {
        return Ok(None);
    }
    owner_eval_db_file_for_record_path(path).map(Some)
}

fn operation_target_kind(target: &OperationTarget) -> String {
    match target {
        OperationTarget::Artifact { .. } => "artifact".to_string(),
        OperationTarget::PatchSet { .. } => "patch_set".to_string(),
        OperationTarget::ArtifactSet { .. } => "artifact_set".to_string(),
    }
}

fn result_path_kind(path: &Path) -> String {
    if path.file_name().and_then(|name| name.to_str()) == Some("runner-result.json") {
        "node_latest".to_string()
    } else if path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        == Some("results")
    {
        "attempt".to_string()
    } else {
        "custom".to_string()
    }
}

fn runtime_id_from_result_path(path: &Path) -> Option<String> {
    if path
        .parent()
        .and_then(|parent| parent.file_name())
        .and_then(|name| name.to_str())
        != Some("results")
    {
        return None;
    }
    path.file_stem()
        .and_then(|stem| stem.to_str())
        .map(ToOwned::to_owned)
}

fn status_name(status: Prototype1NodeStatus) -> Result<String, EvalStoreError> {
    serde_name(&status, "runner_result.status")
}

fn disposition_name(disposition: Prototype1RunnerDisposition) -> Result<String, EvalStoreError> {
    serde_name(&disposition, "runner_result.disposition")
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

fn option_i64(value: Option<i64>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}
