use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

use chrono::Utc;
use cozo::DataValue;
use serde_json::Value;

use crate::cli::prototype1_state::edit_surface::{
    harness_request::PublishedBroadHarnessRequest, tui_adapter,
};
use ploke_records::ids::CampaignId;

use super::{
    cozo_store::{DbEvalStore, EvalDb, mutate_owner_db, owner_eval_db_file_for_record_path},
    error::EvalStoreError,
    evidence::sha256_bytes,
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

pub(crate) const HARNESS_REQUEST_SCHEMA_VERSION: &str = "prototype1-harness-request.v1";
pub(crate) const HARNESS_DIAGNOSTIC_SCHEMA_VERSION: &str = "prototype1-harness-diagnostic.v1";
pub(crate) const HARNESS_WORKSPACE_SCHEMA_VERSION: &str = "prototype1-harness-workspace.v1";

define_eval_schema!(HarnessRequestSchema {
    "eval_harness_request",
    request_id: "String" =>
    campaign_id: "String",
    schema_version: "String",
    request_hash: "String",
    parent_node_id: "String",
    request_path: "String",
    prompt_path: "String",
    submitted_path: "String",
    workspace_path: "String",
    source_repo: "String",
    child_min: "Int",
    child_max: "Int",
    graph_nearest: "Int",
    edit_policy: "String",
    admission_policy: "String",
    request_sha256: "String",
    prompt_sha256: "String",
    ingested_at: "String",
});

define_eval_schema!(HarnessDiagnosticSchema {
    "eval_harness_diagnostic",
    request_id: "String",
    diagnostics_path: "String" =>
    campaign_id: "String",
    schema_version: "String",
    terminal_kind: "String?",
    terminal_detail: "String?",
    attempts: "Int",
    events: "Int",
    validations: "Int",
    prompts: "Int",
    tool_requests: "Int",
    tool_completed: "Int",
    tool_failed: "Int",
    turns: "Int",
    proposals: "Int",
    assistants: "Int",
    outcomes: "Int",
    first_tool: "String?",
    last_tool: "String?",
    content_sha256: "String",
    ingested_at: "String",
});

define_eval_schema!(HarnessWorkspaceSchema {
    "eval_harness_workspace",
    request_id: "String",
    diagnostics_path: "String" =>
    campaign_id: "String",
    schema_version: "String",
    workspace_path: "String",
    source_repo: "String",
    exists: "Bool",
    git_status_ok: "Bool",
    status_error: "String?",
    change_count: "Int",
    ingested_at: "String",
});

define_eval_schema!(HarnessWorkspaceChangeSchema {
    "eval_harness_workspace_change",
    request_id: "String",
    diagnostics_path: "String",
    change_index: "Int" =>
    campaign_id: "String",
    schema_version: "String",
    status_code: "String",
    path: "String",
    original_path: "String?",
});

struct RequestRow {
    campaign_id: String,
    request_id: String,
    request_hash: String,
    parent_node_id: String,
    request_path: String,
    prompt_path: String,
    submitted_path: String,
    workspace_path: String,
    source_repo: String,
    child_min: i64,
    child_max: i64,
    graph_nearest: i64,
    edit_policy: String,
    admission_policy: String,
    request_sha256: String,
    prompt_sha256: String,
    ingested_at: String,
}

struct DiagnosticRow {
    campaign_id: String,
    request_id: String,
    diagnostics_path: String,
    terminal_kind: Option<String>,
    terminal_detail: Option<String>,
    attempts: i64,
    events: i64,
    validations: i64,
    prompts: i64,
    tool_requests: i64,
    tool_completed: i64,
    tool_failed: i64,
    turns: i64,
    proposals: i64,
    assistants: i64,
    outcomes: i64,
    first_tool: Option<String>,
    last_tool: Option<String>,
    content_sha256: String,
    ingested_at: String,
}

struct WorkspaceRow {
    campaign_id: String,
    request_id: String,
    diagnostics_path: String,
    workspace_path: String,
    source_repo: String,
    exists: bool,
    git_status_ok: bool,
    status_error: Option<String>,
    change_count: i64,
    ingested_at: String,
}

struct WorkspaceChangeRow {
    campaign_id: String,
    request_id: String,
    diagnostics_path: String,
    change_index: i64,
    status_code: String,
    path: String,
    original_path: Option<String>,
}

#[derive(Default)]
struct EventCounts {
    events: i64,
    tool_requests: i64,
    tool_completed: i64,
    tool_failed: i64,
    turns: i64,
    proposals: i64,
    assistants: i64,
    outcomes: i64,
    first_tool: Option<String>,
    last_tool: Option<String>,
}

struct WorkspaceStatus {
    exists: bool,
    git_status_ok: bool,
    status_error: Option<String>,
    changes: Vec<WorkspaceChange>,
}

struct WorkspaceChange {
    status_code: String,
    path: String,
    original_path: Option<String>,
}

pub(super) fn ensure_harness_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    HarnessRequestSchema::SCHEMA.ensure_installed(db, "schema.eval_harness_request")?;
    HarnessDiagnosticSchema::SCHEMA.ensure_installed(db, "schema.eval_harness_diagnostic")?;
    HarnessWorkspaceSchema::SCHEMA.ensure_installed(db, "schema.eval_harness_workspace")?;
    HarnessWorkspaceChangeSchema::SCHEMA
        .ensure_installed(db, "schema.eval_harness_workspace_change")?;
    Ok(())
}

pub(crate) fn write_harness_request_to_owner_db(
    campaign_id: &CampaignId,
    published: &PublishedBroadHarnessRequest,
) -> Result<(), EvalStoreError> {
    let db_path = required_owner_db_path(published.request_path())?;
    mutate_owner_db(&db_path, |db| {
        DbEvalStore::new(db).install_schema()?;
        put_request_row(db, &request_row(campaign_id, published)?)
    })
}

pub(crate) fn write_harness_diagnostic_to_owner_db(
    campaign_id: &CampaignId,
    published: &PublishedBroadHarnessRequest,
    diagnostics_path: &Path,
    run: &tui_adapter::HeadlessRun,
) -> Result<(), EvalStoreError> {
    let db_path = required_owner_db_path(diagnostics_path)?;
    mutate_owner_db(&db_path, |db| {
        DbEvalStore::new(db).install_schema()?;
        let summary =
            serde_json::to_value(run.evidence()).map_err(|source| EvalStoreError::Validation {
                field: "eval_harness_diagnostic.summary",
                detail: source.to_string(),
            })?;
        let diagnostic = diagnostic_row(campaign_id, published, diagnostics_path, &summary)?;
        put_diagnostic_row(db, &diagnostic)?;
        let status = capture_workspace_status(published.workspace_path());
        let workspace = workspace_row(campaign_id, published, diagnostics_path, &status)?;
        put_workspace_row(db, &workspace)?;
        for (index, change) in status.changes.iter().enumerate() {
            put_workspace_change_row(
                db,
                &WorkspaceChangeRow {
                    campaign_id: campaign_id.to_string(),
                    request_id: published.request_id().to_string(),
                    diagnostics_path: diagnostics_path.display().to_string(),
                    change_index: usize_to_i64(
                        index,
                        "eval_harness_workspace_change.change_index",
                    )?,
                    status_code: change.status_code.clone(),
                    path: change.path.clone(),
                    original_path: change.original_path.clone(),
                },
            )?;
        }
        Ok(())
    })
}

fn required_owner_db_path(record_path: &Path) -> Result<PathBuf, EvalStoreError> {
    let db_path = owner_eval_db_file_for_record_path(record_path)?;
    if !db_path.is_file() {
        return Err(EvalStoreError::DbSetup {
            phase: "owner_eval_db.required",
            detail: format!(
                "owner eval DB does not exist at '{}' for harness DB mirror",
                db_path.display()
            ),
        });
    }
    Ok(db_path)
}

fn request_row(
    campaign_id: &CampaignId,
    published: &PublishedBroadHarnessRequest,
) -> Result<RequestRow, EvalStoreError> {
    let request = published.request();
    Ok(RequestRow {
        campaign_id: campaign_id.to_string(),
        request_id: published.request_id().to_string(),
        request_hash: published.request_hash().to_string(),
        parent_node_id: request.parent_node_id.as_str().to_string(),
        request_path: published.request_path().display().to_string(),
        prompt_path: published.prompt_path().display().to_string(),
        submitted_path: published.submitted_result_path().display().to_string(),
        workspace_path: published.workspace_path().display().to_string(),
        source_repo: request
            .workspace
            .source_repository_path()
            .display()
            .to_string(),
        child_min: i64::from(request.child_budget.min_children),
        child_max: i64::from(request.child_budget.max_children),
        graph_nearest: usize_to_i64(
            request.graph_restriction.nearest_items,
            "eval_harness_request.graph_nearest",
        )?,
        edit_policy: request.edit_policy.label().to_string(),
        admission_policy: published
            .admission_binding()
            .policy_id()
            .as_str()
            .to_string(),
        request_sha256: file_sha256(published.request_path(), "eval_harness_request.request")?,
        prompt_sha256: file_sha256(published.prompt_path(), "eval_harness_request.prompt")?,
        ingested_at: Utc::now().to_rfc3339(),
    })
}

fn diagnostic_row(
    campaign_id: &CampaignId,
    published: &PublishedBroadHarnessRequest,
    diagnostics_path: &Path,
    summary: &Value,
) -> Result<DiagnosticRow, EvalStoreError> {
    let (terminal_kind, terminal_detail) = terminal_parts(summary.get("terminal"));
    let counts = event_counts(summary.get("events"));
    Ok(DiagnosticRow {
        campaign_id: campaign_id.to_string(),
        request_id: published.request_id().to_string(),
        diagnostics_path: diagnostics_path.display().to_string(),
        terminal_kind,
        terminal_detail,
        attempts: array_len(summary.get("attempts"), "eval_harness_diagnostic.attempts")?,
        events: counts.events,
        validations: array_len(
            summary.get("validations"),
            "eval_harness_diagnostic.validations",
        )?,
        prompts: array_len(
            summary.get("prompt_diagnostics"),
            "eval_harness_diagnostic.prompts",
        )?,
        tool_requests: counts.tool_requests,
        tool_completed: counts.tool_completed,
        tool_failed: counts.tool_failed,
        turns: counts.turns,
        proposals: counts.proposals,
        assistants: counts.assistants,
        outcomes: counts.outcomes,
        first_tool: counts.first_tool,
        last_tool: counts.last_tool,
        content_sha256: file_sha256(diagnostics_path, "eval_harness_diagnostic.content")?,
        ingested_at: Utc::now().to_rfc3339(),
    })
}

fn workspace_row(
    campaign_id: &CampaignId,
    published: &PublishedBroadHarnessRequest,
    diagnostics_path: &Path,
    status: &WorkspaceStatus,
) -> Result<WorkspaceRow, EvalStoreError> {
    Ok(WorkspaceRow {
        campaign_id: campaign_id.to_string(),
        request_id: published.request_id().to_string(),
        diagnostics_path: diagnostics_path.display().to_string(),
        workspace_path: published.workspace_path().display().to_string(),
        source_repo: published
            .request()
            .workspace
            .source_repository_path()
            .display()
            .to_string(),
        exists: status.exists,
        git_status_ok: status.git_status_ok,
        status_error: status.status_error.clone(),
        change_count: usize_to_i64(status.changes.len(), "eval_harness_workspace.change_count")?,
        ingested_at: Utc::now().to_rfc3339(),
    })
}

fn put_request_row<D: EvalDb + ?Sized>(db: &D, row: &RequestRow) -> Result<(), EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("request_id".to_string(), row.request_id.clone().into());
    params.insert(
        "schema_version".to_string(),
        HARNESS_REQUEST_SCHEMA_VERSION.into(),
    );
    params.insert("request_hash".to_string(), row.request_hash.clone().into());
    params.insert(
        "parent_node_id".to_string(),
        row.parent_node_id.clone().into(),
    );
    params.insert("request_path".to_string(), row.request_path.clone().into());
    params.insert("prompt_path".to_string(), row.prompt_path.clone().into());
    params.insert(
        "submitted_path".to_string(),
        row.submitted_path.clone().into(),
    );
    params.insert(
        "workspace_path".to_string(),
        row.workspace_path.clone().into(),
    );
    params.insert("source_repo".to_string(), row.source_repo.clone().into());
    params.insert("child_min".to_string(), row.child_min.into());
    params.insert("child_max".to_string(), row.child_max.into());
    params.insert("graph_nearest".to_string(), row.graph_nearest.into());
    params.insert("edit_policy".to_string(), row.edit_policy.clone().into());
    params.insert(
        "admission_policy".to_string(),
        row.admission_policy.clone().into(),
    );
    params.insert(
        "request_sha256".to_string(),
        row.request_sha256.clone().into(),
    );
    params.insert(
        "prompt_sha256".to_string(),
        row.prompt_sha256.clone().into(),
    );
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    put_eval_params(
        db,
        &HarnessRequestSchema::SCHEMA,
        params,
        "put.eval_harness_request",
    )
}

fn put_diagnostic_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &DiagnosticRow,
) -> Result<(), EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("request_id".to_string(), row.request_id.clone().into());
    params.insert(
        "diagnostics_path".to_string(),
        row.diagnostics_path.clone().into(),
    );
    params.insert(
        "schema_version".to_string(),
        HARNESS_DIAGNOSTIC_SCHEMA_VERSION.into(),
    );
    params.insert(
        "terminal_kind".to_string(),
        option_string_param(row.terminal_kind.clone()),
    );
    params.insert(
        "terminal_detail".to_string(),
        option_string_param(row.terminal_detail.clone()),
    );
    params.insert("attempts".to_string(), row.attempts.into());
    params.insert("events".to_string(), row.events.into());
    params.insert("validations".to_string(), row.validations.into());
    params.insert("prompts".to_string(), row.prompts.into());
    params.insert("tool_requests".to_string(), row.tool_requests.into());
    params.insert("tool_completed".to_string(), row.tool_completed.into());
    params.insert("tool_failed".to_string(), row.tool_failed.into());
    params.insert("turns".to_string(), row.turns.into());
    params.insert("proposals".to_string(), row.proposals.into());
    params.insert("assistants".to_string(), row.assistants.into());
    params.insert("outcomes".to_string(), row.outcomes.into());
    params.insert(
        "first_tool".to_string(),
        option_string_param(row.first_tool.clone()),
    );
    params.insert(
        "last_tool".to_string(),
        option_string_param(row.last_tool.clone()),
    );
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    put_eval_params(
        db,
        &HarnessDiagnosticSchema::SCHEMA,
        params,
        "put.eval_harness_diagnostic",
    )
}

fn put_workspace_row<D: EvalDb + ?Sized>(db: &D, row: &WorkspaceRow) -> Result<(), EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("request_id".to_string(), row.request_id.clone().into());
    params.insert(
        "diagnostics_path".to_string(),
        row.diagnostics_path.clone().into(),
    );
    params.insert(
        "schema_version".to_string(),
        HARNESS_WORKSPACE_SCHEMA_VERSION.into(),
    );
    params.insert(
        "workspace_path".to_string(),
        row.workspace_path.clone().into(),
    );
    params.insert("source_repo".to_string(), row.source_repo.clone().into());
    params.insert("exists".to_string(), DataValue::Bool(row.exists));
    params.insert(
        "git_status_ok".to_string(),
        DataValue::Bool(row.git_status_ok),
    );
    params.insert(
        "status_error".to_string(),
        option_string_param(row.status_error.clone()),
    );
    params.insert("change_count".to_string(), row.change_count.into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    put_eval_params(
        db,
        &HarnessWorkspaceSchema::SCHEMA,
        params,
        "put.eval_harness_workspace",
    )
}

fn put_workspace_change_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &WorkspaceChangeRow,
) -> Result<(), EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("request_id".to_string(), row.request_id.clone().into());
    params.insert(
        "diagnostics_path".to_string(),
        row.diagnostics_path.clone().into(),
    );
    params.insert("change_index".to_string(), row.change_index.into());
    params.insert(
        "schema_version".to_string(),
        HARNESS_WORKSPACE_SCHEMA_VERSION.into(),
    );
    params.insert("status_code".to_string(), row.status_code.clone().into());
    params.insert("path".to_string(), row.path.clone().into());
    params.insert(
        "original_path".to_string(),
        option_string_param(row.original_path.clone()),
    );
    put_eval_params(
        db,
        &HarnessWorkspaceChangeSchema::SCHEMA,
        params,
        "put.eval_harness_workspace_change",
    )
}

fn terminal_parts(value: Option<&Value>) -> (Option<String>, Option<String>) {
    let Some(Value::Object(object)) = value else {
        return (None, None);
    };
    let kind = object
        .get("terminal")
        .and_then(Value::as_str)
        .map(str::to_string);
    let detail = kind.as_deref().and_then(|kind| match kind {
        "timed_out" | "applied_timed_out" => object
            .get("secs")
            .and_then(Value::as_u64)
            .map(|secs| format!("secs={secs}")),
        "setup_unavailable" => Some(format!(
            "phase={}; reason={}",
            string_field(object, "phase"),
            string_field(object, "reason")
        )),
        "provider_unavailable" | "context_unavailable" => object
            .get("reason")
            .and_then(Value::as_str)
            .map(str::to_string),
        "tool_failed" => object
            .get("error")
            .and_then(Value::as_str)
            .map(str::to_string),
        "exhausted" => Some(format!(
            "attempts={}; last_feedback={}",
            object.get("attempts").and_then(Value::as_u64).unwrap_or(0),
            string_field(object, "last_feedback")
        )),
        "completed_without_edit" => Some(format!(
            "outcome={}; summary={}",
            string_field(object, "outcome"),
            string_field(object, "summary")
        )),
        "applied"
        | "applied_validation_failed"
        | "applied_validation_missing"
        | "applied_turn_aborted" => object
            .get("changed_paths")
            .and_then(Value::as_array)
            .map(|paths| format!("changed_paths={}", paths.len())),
        "no_edit" => None,
        _ => None,
    });
    (kind, detail)
}

fn event_counts(value: Option<&Value>) -> EventCounts {
    let mut counts = EventCounts::default();
    let Some(events) = value.and_then(Value::as_array) else {
        return counts;
    };
    counts.events = events.len() as i64;
    for event in events {
        let Some(object) = event.as_object() else {
            continue;
        };
        match object.get("kind").and_then(Value::as_str) {
            Some("tool_request") => {
                counts.tool_requests += 1;
                if let Some(tool) = object.get("tool").and_then(Value::as_str) {
                    counts.first_tool.get_or_insert_with(|| tool.to_string());
                    counts.last_tool = Some(tool.to_string());
                }
            }
            Some("tool_completed") => counts.tool_completed += 1,
            Some("tool_failed") => counts.tool_failed += 1,
            Some("turn") => counts.turns += 1,
            Some("proposal") => counts.proposals += 1,
            Some("assistant_message") => counts.assistants += 1,
            Some("outcome") => counts.outcomes += 1,
            _ => {}
        }
    }
    counts
}

fn capture_workspace_status(workspace: &Path) -> WorkspaceStatus {
    if !workspace.exists() {
        return WorkspaceStatus {
            exists: false,
            git_status_ok: false,
            status_error: Some("workspace path does not exist".to_string()),
            changes: Vec::new(),
        };
    }
    let output = Command::new("git")
        .arg("-C")
        .arg(workspace)
        .args(["status", "--short", "--porcelain=v1"])
        .output();
    let output = match output {
        Ok(output) => output,
        Err(source) => {
            return WorkspaceStatus {
                exists: true,
                git_status_ok: false,
                status_error: Some(format!("failed to run git status: {source}")),
                changes: Vec::new(),
            };
        }
    };
    if !output.status.success() {
        return WorkspaceStatus {
            exists: true,
            git_status_ok: false,
            status_error: Some(String::from_utf8_lossy(&output.stderr).trim().to_string()),
            changes: Vec::new(),
        };
    }
    let text = String::from_utf8_lossy(&output.stdout);
    WorkspaceStatus {
        exists: true,
        git_status_ok: true,
        status_error: None,
        changes: text.lines().filter_map(parse_status_line).collect(),
    }
}

fn parse_status_line(line: &str) -> Option<WorkspaceChange> {
    if line.len() < 4 {
        return None;
    }
    let status_code = line.get(0..2)?.to_string();
    let path_text = line.get(3..)?.trim();
    if path_text.is_empty() {
        return None;
    }
    let (path, original_path) = if let Some((old, new)) = path_text.split_once(" -> ") {
        (new.to_string(), Some(old.to_string()))
    } else {
        (path_text.to_string(), None)
    };
    Some(WorkspaceChange {
        status_code,
        path,
        original_path,
    })
}

fn array_len(value: Option<&Value>, field: &'static str) -> Result<i64, EvalStoreError> {
    let len = value.and_then(Value::as_array).map(Vec::len).unwrap_or(0);
    usize_to_i64(len, field)
}

fn string_field(object: &serde_json::Map<String, Value>, field: &str) -> String {
    object
        .get(field)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn option_string_param(value: Option<String>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}

fn usize_to_i64(value: usize, field: &'static str) -> Result<i64, EvalStoreError> {
    i64::try_from(value).map_err(|_| EvalStoreError::Validation {
        field,
        detail: format!("value {value} does not fit in Int"),
    })
}

fn file_sha256(path: &Path, field: &'static str) -> Result<String, EvalStoreError> {
    let bytes = fs::read(path).map_err(|source| EvalStoreError::Io {
        phase: field,
        path: path.to_path_buf(),
        source,
    })?;
    Ok(sha256_bytes(&bytes))
}
