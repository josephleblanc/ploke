use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use cozo::DataValue;
use ploke_records::{
    agent_turn::{
        AgentTurnSummaryRecord, AgentTurnTraceRecord, MessageSnapshotRecord,
        ObservedTurnEventRecord, RequestMessageRecord, RequestRoleRecord, ToolCompletedRecord,
        ToolFailedRecord, ToolRequestRecord,
    },
    ids::CampaignId,
    llm_response::RawFullResponseRecord,
};
use serde::Serialize;

use super::{
    cozo_store::{EvalDb, mutate_owner_db},
    error::EvalStoreError,
    evidence::{hash_parts, sha256_bytes},
    schema::{EvalRelationSchema, EvalRow, define_eval_schema, put_eval_row},
};

define_eval_schema!(AgentTurnSchema {
    "eval_agent_turn",
    turn_id: "String" =>
    campaign_id: "String?",
    task_id: "String",
    request_id: "String?",
    selected_model: "String",
    provider: "String?",
    route_source: "String?",
    trace_ref: "String",
    summary_ref: "String",
    full_response_ref: "String?",
    trace_sha256: "String",
    summary_sha256: "String",
    full_response_sha256: "String?",
    event_count: "Int",
    response_count: "Int",
    terminal_outcome: "String?",
    final_message_id: "String?",
    recorded_at: "String",
});

define_eval_schema!(AgentTurnEventSchema {
    "eval_agent_turn_event",
    event_id: "String" =>
    turn_id: "String",
    source_event_index: "Int",
    event_kind: "String",
    payload_json: "String",
    payload_sha256: "String",
    tool_call_id: "String?",
    tool_name: "String?",
    message_id: "String?",
    outcome: "String?",
    recorded_at: "String",
});

define_eval_schema!(ModelExchangeSchema {
    "eval_model_exchange",
    exchange_id: "String" =>
    turn_id: "String",
    response_index: "Int",
    assistant_message_id: "String",
    provider_response_id: "String",
    model_id: "String",
    finish_reason: "String?",
    usage_prompt_tokens: "Int?",
    usage_completion_tokens: "Int?",
    usage_total_tokens: "Int?",
    response_ref: "String?",
    response_sha256: "String",
    source_ref: "String?",
    recorded_at: "String",
});

define_eval_schema!(MessageEventSchema {
    "eval_message_event",
    message_event_id: "String" =>
    turn_id: "String",
    source_event_index: "Int?",
    message_id: "String?",
    role: "String",
    status: "String?",
    tool_call_id: "String?",
    content_len: "Int?",
    content_preview: "String?",
    content_sha256: "String?",
    recorded_at: "String",
});

define_eval_schema!(ToolEventSchema {
    "eval_tool_event",
    tool_event_id: "String" =>
    turn_id: "String",
    source_event_index: "Int",
    tool_call_id: "String",
    tool_name: "String",
    status: "String",
    request_id: "String?",
    parent_id: "String?",
    args_json: "String?",
    result_json: "String?",
    ui_json: "String?",
    error: "String?",
    latency_ms: "Int?",
    recorded_at: "String",
});

pub(crate) const AGENT_TURN_REL: &str = AgentTurnSchema::RELATION;
pub(crate) const AGENT_TURN_EVENT_REL: &str = AgentTurnEventSchema::RELATION;
pub(crate) const MODEL_EXCHANGE_REL: &str = ModelExchangeSchema::RELATION;
pub(crate) const MESSAGE_EVENT_REL: &str = MessageEventSchema::RELATION;
pub(crate) const TOOL_EVENT_REL: &str = ToolEventSchema::RELATION;

#[derive(Debug, Clone)]
pub(crate) struct AgentTurnEvidence {
    pub(crate) campaign_id: Option<CampaignId>,
    pub(crate) trace_path: String,
    pub(crate) summary_path: String,
    pub(crate) full_response_path: Option<String>,
    pub(crate) trace_record: AgentTurnTraceRecord,
    pub(crate) summary_record: AgentTurnSummaryRecord,
    pub(crate) full_responses: Vec<RawFullResponseRecord>,
    pub(crate) recorded_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentTurnReceipt {
    pub(crate) turn_id: String,
    pub(crate) event_ids: Vec<String>,
    pub(crate) exchange_ids: Vec<String>,
    pub(crate) message_ids: Vec<String>,
    pub(crate) tool_ids: Vec<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct AgentTurnBundleEvidence {
    pub(crate) campaign_id: Option<CampaignId>,
    pub(crate) trace_path: PathBuf,
    pub(crate) summary_path: PathBuf,
    pub(crate) full_response_path: PathBuf,
    pub(crate) trace_record: AgentTurnTraceRecord,
    pub(crate) summary_record: AgentTurnSummaryRecord,
    pub(crate) full_responses: Vec<RawFullResponseRecord>,
    pub(crate) recorded_at: String,
}

impl AgentTurnBundleEvidence {
    pub(crate) fn db_evidence(&self) -> AgentTurnEvidence {
        AgentTurnEvidence {
            campaign_id: self.campaign_id.clone(),
            trace_path: self.trace_path.display().to_string(),
            summary_path: self.summary_path.display().to_string(),
            full_response_path: Some(self.full_response_path.display().to_string()),
            trace_record: self.trace_record.clone(),
            summary_record: self.summary_record.clone(),
            full_responses: self.full_responses.clone(),
            recorded_at: self.recorded_at.clone(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct AgentTurnBundleReceipt {
    pub(crate) trace_path: PathBuf,
    pub(crate) summary_path: PathBuf,
    pub(crate) full_response_path: PathBuf,
    pub(crate) db_receipt: Option<AgentTurnReceipt>,
}

struct EvalAgentTurnRow {
    turn_id: String,
    campaign_id: Option<String>,
    task_id: String,
    request_id: Option<String>,
    selected_model: String,
    provider: Option<String>,
    route_source: Option<String>,
    trace_ref: String,
    summary_ref: String,
    full_response_ref: Option<String>,
    trace_sha256: String,
    summary_sha256: String,
    full_response_sha256: Option<String>,
    event_count: i64,
    response_count: i64,
    terminal_outcome: Option<String>,
    final_message_id: Option<String>,
    recorded_at: String,
}

struct EvalAgentTurnEventRow {
    event_id: String,
    turn_id: String,
    source_event_index: i64,
    event_kind: String,
    payload_json: String,
    payload_sha256: String,
    tool_call_id: Option<String>,
    tool_name: Option<String>,
    message_id: Option<String>,
    outcome: Option<String>,
    recorded_at: String,
}

struct EvalModelExchangeRow {
    exchange_id: String,
    turn_id: String,
    response_index: i64,
    assistant_message_id: String,
    provider_response_id: String,
    model_id: String,
    finish_reason: Option<String>,
    usage_prompt_tokens: Option<i64>,
    usage_completion_tokens: Option<i64>,
    usage_total_tokens: Option<i64>,
    response_ref: Option<String>,
    response_sha256: String,
    source_ref: Option<String>,
    recorded_at: String,
}

struct EvalMessageEventRow {
    message_event_id: String,
    turn_id: String,
    source_event_index: Option<i64>,
    message_id: Option<String>,
    role: String,
    status: Option<String>,
    tool_call_id: Option<String>,
    content_len: Option<i64>,
    content_preview: Option<String>,
    content_sha256: Option<String>,
    recorded_at: String,
}

struct EvalToolEventRow {
    tool_event_id: String,
    turn_id: String,
    source_event_index: i64,
    tool_call_id: String,
    tool_name: String,
    status: String,
    request_id: Option<String>,
    parent_id: Option<String>,
    args_json: Option<String>,
    result_json: Option<String>,
    ui_json: Option<String>,
    error: Option<String>,
    latency_ms: Option<i64>,
    recorded_at: String,
}

impl EvalRow for EvalAgentTurnRow {
    type Schema = AgentTurnSchema;

    const PHASE: &'static str = "eval_agent_turn.put";

    fn schema() -> &'static Self::Schema {
        &AgentTurnSchema::SCHEMA
    }

    fn params(&self) -> BTreeMap<String, DataValue> {
        agent_turn_params(self, Self::schema())
    }
}

impl EvalRow for EvalAgentTurnEventRow {
    type Schema = AgentTurnEventSchema;

    const PHASE: &'static str = "eval_agent_turn_event.put";

    fn schema() -> &'static Self::Schema {
        &AgentTurnEventSchema::SCHEMA
    }

    fn params(&self) -> BTreeMap<String, DataValue> {
        event_params(self, Self::schema())
    }
}

impl EvalRow for EvalModelExchangeRow {
    type Schema = ModelExchangeSchema;

    const PHASE: &'static str = "eval_model_exchange.put";

    fn schema() -> &'static Self::Schema {
        &ModelExchangeSchema::SCHEMA
    }

    fn params(&self) -> BTreeMap<String, DataValue> {
        exchange_params(self, Self::schema())
    }
}

impl EvalRow for EvalMessageEventRow {
    type Schema = MessageEventSchema;

    const PHASE: &'static str = "eval_message_event.put";

    fn schema() -> &'static Self::Schema {
        &MessageEventSchema::SCHEMA
    }

    fn params(&self) -> BTreeMap<String, DataValue> {
        message_params(self, Self::schema())
    }
}

impl EvalRow for EvalToolEventRow {
    type Schema = ToolEventSchema;

    const PHASE: &'static str = "eval_tool_event.put";

    fn schema() -> &'static Self::Schema {
        &ToolEventSchema::SCHEMA
    }

    fn params(&self) -> BTreeMap<String, DataValue> {
        tool_params(self, Self::schema())
    }
}

pub(super) fn ensure_agent_turn_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    AgentTurnSchema::SCHEMA.ensure_installed(db, "schema.eval_agent_turn")?;
    AgentTurnEventSchema::SCHEMA.ensure_installed(db, "schema.eval_agent_turn_event")?;
    ModelExchangeSchema::SCHEMA.ensure_installed(db, "schema.eval_model_exchange")?;
    MessageEventSchema::SCHEMA.ensure_installed(db, "schema.eval_message_event")?;
    ToolEventSchema::SCHEMA.ensure_installed(db, "schema.eval_tool_event")?;
    Ok(())
}

pub(crate) fn write_agent_turn_to_owner_db(
    db_path: &Path,
    evidence: AgentTurnEvidence,
) -> Result<AgentTurnReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| put_agent_turn(db, evidence))
}

pub(crate) fn write_agent_turn_bundle_files(
    evidence: &AgentTurnBundleEvidence,
) -> Result<(), EvalStoreError> {
    write_json_file(
        &evidence.trace_path,
        &evidence.trace_record,
        "agent_turn.trace_file_json",
    )?;
    write_json_file(
        &evidence.summary_path,
        &evidence.summary_record,
        "agent_turn.summary_file_json",
    )?;
    let jsonl = full_response_jsonl(&evidence.full_responses)?;
    write_bytes(
        &evidence.full_response_path,
        jsonl.as_bytes(),
        "agent_turn.full_response_jsonl",
    )
}

fn write_json_file<T: Serialize>(
    path: &Path,
    value: &T,
    field: &'static str,
) -> Result<(), EvalStoreError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|source| EvalStoreError::Validation {
        field,
        detail: source.to_string(),
    })?;
    write_bytes(path, &bytes, field)
}

fn write_bytes(path: &Path, bytes: &[u8], phase: &'static str) -> Result<(), EvalStoreError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| EvalStoreError::Io {
            phase,
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, bytes).map_err(|source| EvalStoreError::Io {
        phase,
        path: path.to_path_buf(),
        source,
    })
}

fn put_agent_turn<D: EvalDb + ?Sized>(
    db: &D,
    evidence: AgentTurnEvidence,
) -> Result<AgentTurnReceipt, EvalStoreError> {
    ensure_agent_turn_schema(db)?;
    let rows = agent_turn_rows(evidence)?;
    put_eval_row(db, &rows.turn)?;
    for row in &rows.events {
        put_eval_row(db, row)?;
    }
    for row in &rows.exchanges {
        put_eval_row(db, row)?;
    }
    for row in &rows.messages {
        put_eval_row(db, row)?;
    }
    for row in &rows.tools {
        put_eval_row(db, row)?;
    }
    Ok(AgentTurnReceipt {
        turn_id: rows.turn.turn_id,
        event_ids: rows.events.into_iter().map(|row| row.event_id).collect(),
        exchange_ids: rows
            .exchanges
            .into_iter()
            .map(|row| row.exchange_id)
            .collect(),
        message_ids: rows
            .messages
            .into_iter()
            .map(|row| row.message_event_id)
            .collect(),
        tool_ids: rows
            .tools
            .into_iter()
            .map(|row| row.tool_event_id)
            .collect(),
    })
}

struct AgentTurnRows {
    turn: EvalAgentTurnRow,
    events: Vec<EvalAgentTurnEventRow>,
    exchanges: Vec<EvalModelExchangeRow>,
    messages: Vec<EvalMessageEventRow>,
    tools: Vec<EvalToolEventRow>,
}

fn agent_turn_rows(evidence: AgentTurnEvidence) -> Result<AgentTurnRows, EvalStoreError> {
    let trace_json = to_json(&evidence.trace_record, "agent_turn.trace_json")?;
    let summary_json = to_json(&evidence.summary_record, "agent_turn.summary_json")?;
    let full_json = full_response_jsonl(&evidence.full_responses)?;
    let trace_hash = sha256_bytes(trace_json.as_bytes());
    let summary_hash = sha256_bytes(summary_json.as_bytes());
    let full_hash = if evidence.full_responses.is_empty() {
        None
    } else {
        Some(sha256_bytes(full_json.as_bytes()))
    };
    let campaign = evidence.campaign_id.as_ref().map(ToString::to_string);
    let turn = &evidence.summary_record.0;
    require_non_empty("agent_turn.task_id", &turn.task_id)?;
    require_non_empty("agent_turn.selected_model", &turn.selected_model)?;
    require_non_empty("agent_turn.trace_ref", &evidence.trace_path)?;
    require_non_empty("agent_turn.summary_ref", &evidence.summary_path)?;
    let request_id = turn
        .terminal_record
        .as_ref()
        .map(|record| record.request_id.clone())
        .or_else(|| Some(turn.task_id.clone()));
    let terminal_outcome = turn
        .terminal_record
        .as_ref()
        .map(|record| record.outcome.clone());
    let final_message_id = turn
        .final_assistant_message
        .as_ref()
        .map(|record| record.id.clone());
    let route = turn.model_route.as_ref();
    let turn_id = hash_parts(&[
        "p1.eval.agent_turn.v1",
        campaign.as_deref().unwrap_or(""),
        &turn.task_id,
        request_id.as_deref().unwrap_or(""),
        &turn.user_message_id,
        &turn.selected_model,
        &trace_hash,
    ]);
    let turn_row = EvalAgentTurnRow {
        turn_id: turn_id.clone(),
        campaign_id: campaign,
        task_id: turn.task_id.clone(),
        request_id,
        selected_model: turn.selected_model.clone(),
        provider: route.and_then(|record| record.provider_slug.clone()),
        route_source: route.map(|record| record.route_source.clone()),
        trace_ref: evidence.trace_path,
        summary_ref: evidence.summary_path,
        full_response_ref: evidence.full_response_path,
        trace_sha256: trace_hash,
        summary_sha256: summary_hash,
        full_response_sha256: full_hash,
        event_count: to_i64(turn.events.len(), "agent_turn.event_count")?,
        response_count: to_i64(evidence.full_responses.len(), "agent_turn.response_count")?,
        terminal_outcome,
        final_message_id,
        recorded_at: evidence.recorded_at.clone(),
    };
    let events = turn
        .events
        .iter()
        .enumerate()
        .map(|(index, event)| event_row(&turn_id, index, event, &evidence.recorded_at))
        .collect::<Result<Vec<_>, _>>()?;
    let exchanges = evidence
        .full_responses
        .iter()
        .map(|record| {
            exchange_row(
                &turn_id,
                record,
                turn_row.full_response_ref.as_deref(),
                &evidence.recorded_at,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let mut messages = prompt_message_rows(&turn_id, &turn.llm_prompt, &evidence.recorded_at)?;
    messages.extend(event_message_rows(
        &turn_id,
        &turn.events,
        &evidence.recorded_at,
    )?);
    if let Some(message) = &turn.final_assistant_message {
        messages.push(snapshot_message_row(
            &turn_id,
            None,
            "final_assistant",
            message,
            &evidence.recorded_at,
        )?);
    }
    let tools = tool_rows(&turn_id, &turn.events, &evidence.recorded_at)?;
    Ok(AgentTurnRows {
        turn: turn_row,
        events,
        exchanges,
        messages,
        tools,
    })
}

fn event_row(
    turn_id: &str,
    index: usize,
    event: &ObservedTurnEventRecord,
    recorded_at: &str,
) -> Result<EvalAgentTurnEventRow, EvalStoreError> {
    let payload_json = to_json(event, "agent_turn.event_json")?;
    let payload_hash = sha256_bytes(payload_json.as_bytes());
    let meta = event_meta(event);
    let source_event_index = to_i64(index, "agent_turn.source_event_index")?;
    let event_id = hash_parts(&[
        "p1.eval.agent_turn_event.v1",
        turn_id,
        &source_event_index.to_string(),
        &payload_hash,
    ]);
    Ok(EvalAgentTurnEventRow {
        event_id,
        turn_id: turn_id.to_string(),
        source_event_index,
        event_kind: meta.kind.to_string(),
        payload_json,
        payload_sha256: payload_hash,
        tool_call_id: meta.tool_call_id,
        tool_name: meta.tool_name,
        message_id: meta.message_id,
        outcome: meta.outcome,
        recorded_at: recorded_at.to_string(),
    })
}

struct EventMeta {
    kind: &'static str,
    tool_call_id: Option<String>,
    tool_name: Option<String>,
    message_id: Option<String>,
    outcome: Option<String>,
}

fn event_meta(event: &ObservedTurnEventRecord) -> EventMeta {
    match event {
        ObservedTurnEventRecord::DebugCommand(_) => EventMeta {
            kind: "debug_command",
            tool_call_id: None,
            tool_name: None,
            message_id: None,
            outcome: None,
        },
        ObservedTurnEventRecord::LlmEvent(_) => EventMeta {
            kind: "llm_event",
            tool_call_id: None,
            tool_name: None,
            message_id: None,
            outcome: None,
        },
        ObservedTurnEventRecord::LlmResponse(record) => EventMeta {
            kind: "llm_response",
            tool_call_id: None,
            tool_name: None,
            message_id: None,
            outcome: record
                .finish_reason
                .as_ref()
                .map(|reason| format!("{reason:?}")),
        },
        ObservedTurnEventRecord::ToolRequested(record) => EventMeta {
            kind: "tool_requested",
            tool_call_id: Some(record.call_id.clone()),
            tool_name: Some(record.tool.clone()),
            message_id: None,
            outcome: Some("requested".to_string()),
        },
        ObservedTurnEventRecord::ToolCompleted(record) => EventMeta {
            kind: "tool_completed",
            tool_call_id: Some(record.call_id.clone()),
            tool_name: Some(record.tool.clone()),
            message_id: None,
            outcome: Some("completed".to_string()),
        },
        ObservedTurnEventRecord::ToolFailed(record) => EventMeta {
            kind: "tool_failed",
            tool_call_id: Some(record.call_id.clone()),
            tool_name: record.tool.clone(),
            message_id: None,
            outcome: Some("failed".to_string()),
        },
        ObservedTurnEventRecord::MessageUpdated(record) => EventMeta {
            kind: "message_updated",
            tool_call_id: record.tool_call_id.clone(),
            tool_name: None,
            message_id: Some(record.id.clone()),
            outcome: Some(record.status.clone()),
        },
        ObservedTurnEventRecord::TurnFinished(record) => EventMeta {
            kind: "turn_finished",
            tool_call_id: None,
            tool_name: None,
            message_id: Some(record.assistant_message_id.clone()),
            outcome: Some(record.outcome.clone()),
        },
    }
}

fn exchange_row(
    turn_id: &str,
    record: &RawFullResponseRecord,
    source_ref: Option<&str>,
    recorded_at: &str,
) -> Result<EvalModelExchangeRow, EvalStoreError> {
    let response_json = to_json(record, "agent_turn.full_response_json")?;
    let response_hash = sha256_bytes(response_json.as_bytes());
    let response = record.response();
    let response_index = to_i64(
        record.response_index().get(),
        "model_exchange.response_index",
    )?;
    let finish_reason = response
        .choices
        .first()
        .and_then(|choice| choice.finish_reason.as_ref())
        .map(|reason| format!("{reason:?}"));
    let usage = response.usage.as_ref();
    let exchange_id = hash_parts(&[
        "p1.eval.model_exchange.v1",
        turn_id,
        &response_index.to_string(),
        &record.assistant_message_id.to_string(),
        &response.id,
        &response_hash,
    ]);
    Ok(EvalModelExchangeRow {
        exchange_id,
        turn_id: turn_id.to_string(),
        response_index,
        assistant_message_id: record.assistant_message_id.to_string(),
        provider_response_id: response.id.clone(),
        model_id: response.model.clone(),
        finish_reason,
        usage_prompt_tokens: usage.map(|usage| usage.prompt_tokens as i64),
        usage_completion_tokens: usage.map(|usage| usage.completion_tokens as i64),
        usage_total_tokens: usage.map(|usage| usage.total_tokens as i64),
        response_ref: source_ref.map(|source| format!("{source}#response_index:{response_index}")),
        response_sha256: response_hash,
        source_ref: source_ref.map(ToString::to_string),
        recorded_at: recorded_at.to_string(),
    })
}

fn prompt_message_rows(
    turn_id: &str,
    messages: &[RequestMessageRecord],
    recorded_at: &str,
) -> Result<Vec<EvalMessageEventRow>, EvalStoreError> {
    messages
        .iter()
        .enumerate()
        .map(|(index, message)| prompt_message_row(turn_id, index, message, recorded_at))
        .collect()
}

fn prompt_message_row(
    turn_id: &str,
    index: usize,
    message: &RequestMessageRecord,
    recorded_at: &str,
) -> Result<EvalMessageEventRow, EvalStoreError> {
    let content_hash = sha256_bytes(message.content.as_bytes());
    let role = request_role(message.role);
    let source_event_index = to_i64(index, "message_event.prompt_index")?;
    let message_event_id = hash_parts(&[
        "p1.eval.message_event.prompt.v1",
        turn_id,
        &source_event_index.to_string(),
        role,
        &content_hash,
    ]);
    Ok(EvalMessageEventRow {
        message_event_id,
        turn_id: turn_id.to_string(),
        source_event_index: Some(source_event_index),
        message_id: None,
        role: role.to_string(),
        status: Some("prompt".to_string()),
        tool_call_id: message.tool_call_id.clone(),
        content_len: Some(to_i64(message.content.len(), "message_event.content_len")?),
        content_preview: Some(message.content.chars().take(200).collect()),
        content_sha256: Some(content_hash),
        recorded_at: recorded_at.to_string(),
    })
}

fn event_message_rows(
    turn_id: &str,
    events: &[ObservedTurnEventRecord],
    recorded_at: &str,
) -> Result<Vec<EvalMessageEventRow>, EvalStoreError> {
    let mut rows = Vec::new();
    for (index, event) in events.iter().enumerate() {
        if let ObservedTurnEventRecord::MessageUpdated(message) = event {
            rows.push(snapshot_message_row(
                turn_id,
                Some(to_i64(index, "message_event.source_event_index")?),
                "observed",
                message,
                recorded_at,
            )?);
        }
    }
    Ok(rows)
}

fn snapshot_message_row(
    turn_id: &str,
    source_event_index: Option<i64>,
    role: &str,
    message: &MessageSnapshotRecord,
    recorded_at: &str,
) -> Result<EvalMessageEventRow, EvalStoreError> {
    let content_hash = sha256_bytes(message.content_preview.as_bytes());
    let index = source_event_index
        .map(|value| value.to_string())
        .unwrap_or_else(|| "final".to_string());
    let message_event_id = hash_parts(&[
        "p1.eval.message_event.snapshot.v1",
        turn_id,
        &index,
        &message.id,
        &message.status,
        &content_hash,
    ]);
    Ok(EvalMessageEventRow {
        message_event_id,
        turn_id: turn_id.to_string(),
        source_event_index,
        message_id: Some(message.id.clone()),
        role: role.to_string(),
        status: Some(message.status.clone()),
        tool_call_id: message.tool_call_id.clone(),
        content_len: Some(to_i64(message.content_len, "message_event.content_len")?),
        content_preview: Some(message.content_preview.clone()),
        content_sha256: Some(content_hash),
        recorded_at: recorded_at.to_string(),
    })
}

fn request_role(role: RequestRoleRecord) -> &'static str {
    match role {
        RequestRoleRecord::User => "user",
        RequestRoleRecord::Assistant => "assistant",
        RequestRoleRecord::System => "system",
        RequestRoleRecord::Tool => "tool",
    }
}

fn tool_rows(
    turn_id: &str,
    events: &[ObservedTurnEventRecord],
    recorded_at: &str,
) -> Result<Vec<EvalToolEventRow>, EvalStoreError> {
    let mut rows = Vec::new();
    for (index, event) in events.iter().enumerate() {
        let source_event_index = to_i64(index, "tool_event.source_event_index")?;
        match event {
            ObservedTurnEventRecord::ToolRequested(record) => rows.push(tool_requested_row(
                turn_id,
                source_event_index,
                record,
                recorded_at,
            )?),
            ObservedTurnEventRecord::ToolCompleted(record) => rows.push(tool_completed_row(
                turn_id,
                source_event_index,
                record,
                recorded_at,
            )?),
            ObservedTurnEventRecord::ToolFailed(record) => rows.push(tool_failed_row(
                turn_id,
                source_event_index,
                record,
                recorded_at,
            )?),
            _ => {}
        }
    }
    Ok(rows)
}

fn tool_requested_row(
    turn_id: &str,
    index: i64,
    record: &ToolRequestRecord,
    recorded_at: &str,
) -> Result<EvalToolEventRow, EvalStoreError> {
    let args_json = to_json(&record.arguments, "tool_event.args_json")?;
    Ok(EvalToolEventRow {
        tool_event_id: tool_id(turn_id, index, &record.call_id, "requested", &args_json),
        turn_id: turn_id.to_string(),
        source_event_index: index,
        tool_call_id: record.call_id.clone(),
        tool_name: record.tool.clone(),
        status: "requested".to_string(),
        request_id: Some(record.request_id.clone()),
        parent_id: Some(record.parent_id.clone()),
        args_json: Some(args_json),
        result_json: None,
        ui_json: None,
        error: None,
        latency_ms: None,
        recorded_at: recorded_at.to_string(),
    })
}

fn tool_completed_row(
    turn_id: &str,
    index: i64,
    record: &ToolCompletedRecord,
    recorded_at: &str,
) -> Result<EvalToolEventRow, EvalStoreError> {
    let result_json = to_json(record, "tool_event.result_json")?;
    let ui_json = record
        .ui_payload
        .as_ref()
        .map(|ui| to_json(ui, "tool_event.ui_json"))
        .transpose()?;
    Ok(EvalToolEventRow {
        tool_event_id: tool_id(turn_id, index, &record.call_id, "completed", &result_json),
        turn_id: turn_id.to_string(),
        source_event_index: index,
        tool_call_id: record.call_id.clone(),
        tool_name: record.tool.clone(),
        status: "completed".to_string(),
        request_id: Some(record.request_id.clone()),
        parent_id: Some(record.parent_id.clone()),
        args_json: None,
        result_json: Some(result_json),
        ui_json,
        error: None,
        latency_ms: Some(record.latency_ms as i64),
        recorded_at: recorded_at.to_string(),
    })
}

fn tool_failed_row(
    turn_id: &str,
    index: i64,
    record: &ToolFailedRecord,
    recorded_at: &str,
) -> Result<EvalToolEventRow, EvalStoreError> {
    let result_json = to_json(record, "tool_event.failed_json")?;
    let ui_json = record
        .ui_payload
        .as_ref()
        .map(|ui| to_json(ui, "tool_event.ui_json"))
        .transpose()?;
    Ok(EvalToolEventRow {
        tool_event_id: tool_id(turn_id, index, &record.call_id, "failed", &result_json),
        turn_id: turn_id.to_string(),
        source_event_index: index,
        tool_call_id: record.call_id.clone(),
        tool_name: record.tool.clone().unwrap_or_else(|| "unknown".to_string()),
        status: "failed".to_string(),
        request_id: Some(record.request_id.clone()),
        parent_id: Some(record.parent_id.clone()),
        args_json: None,
        result_json: Some(result_json),
        ui_json,
        error: Some(record.error.clone()),
        latency_ms: Some(record.latency_ms as i64),
        recorded_at: recorded_at.to_string(),
    })
}

fn tool_id(turn_id: &str, index: i64, call_id: &str, status: &str, payload: &str) -> String {
    let payload_hash = sha256_bytes(payload.as_bytes());
    hash_parts(&[
        "p1.eval.tool_event.v1",
        turn_id,
        &index.to_string(),
        call_id,
        status,
        &payload_hash,
    ])
}

fn full_response_jsonl(records: &[RawFullResponseRecord]) -> Result<String, EvalStoreError> {
    let mut out = String::new();
    for record in records {
        out.push_str(&to_json(record, "agent_turn.full_response_jsonl")?);
        out.push('\n');
    }
    Ok(out)
}

fn to_json<T: Serialize>(value: &T, field: &'static str) -> Result<String, EvalStoreError> {
    serde_json::to_string(value).map_err(|source| EvalStoreError::Validation {
        field,
        detail: source.to_string(),
    })
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), EvalStoreError> {
    if value.trim().is_empty() {
        return Err(EvalStoreError::Validation {
            field,
            detail: "must not be empty".to_string(),
        });
    }
    Ok(())
}

fn to_i64(value: usize, field: &'static str) -> Result<i64, EvalStoreError> {
    i64::try_from(value).map_err(|source| EvalStoreError::Validation {
        field,
        detail: source.to_string(),
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

fn agent_turn_params(
    row: &EvalAgentTurnRow,
    schema: &AgentTurnSchema,
) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(schema.turn_id().to_string(), row.turn_id.clone().into());
    params.insert(
        schema.campaign_id().to_string(),
        option_string(&row.campaign_id),
    );
    params.insert(schema.task_id().to_string(), row.task_id.clone().into());
    params.insert(
        schema.request_id().to_string(),
        option_string(&row.request_id),
    );
    params.insert(
        schema.selected_model().to_string(),
        row.selected_model.clone().into(),
    );
    params.insert(schema.provider().to_string(), option_string(&row.provider));
    params.insert(
        schema.route_source().to_string(),
        option_string(&row.route_source),
    );
    params.insert(schema.trace_ref().to_string(), row.trace_ref.clone().into());
    params.insert(
        schema.summary_ref().to_string(),
        row.summary_ref.clone().into(),
    );
    params.insert(
        schema.full_response_ref().to_string(),
        option_string(&row.full_response_ref),
    );
    params.insert(
        schema.trace_sha256().to_string(),
        row.trace_sha256.clone().into(),
    );
    params.insert(
        schema.summary_sha256().to_string(),
        row.summary_sha256.clone().into(),
    );
    params.insert(
        schema.full_response_sha256().to_string(),
        option_string(&row.full_response_sha256),
    );
    params.insert(schema.event_count().to_string(), row.event_count.into());
    params.insert(
        schema.response_count().to_string(),
        row.response_count.into(),
    );
    params.insert(
        schema.terminal_outcome().to_string(),
        option_string(&row.terminal_outcome),
    );
    params.insert(
        schema.final_message_id().to_string(),
        option_string(&row.final_message_id),
    );
    params.insert(
        schema.recorded_at().to_string(),
        row.recorded_at.clone().into(),
    );
    params
}

fn event_params(
    row: &EvalAgentTurnEventRow,
    schema: &AgentTurnEventSchema,
) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(schema.event_id().to_string(), row.event_id.clone().into());
    params.insert(schema.turn_id().to_string(), row.turn_id.clone().into());
    params.insert(
        schema.source_event_index().to_string(),
        row.source_event_index.into(),
    );
    params.insert(
        schema.event_kind().to_string(),
        row.event_kind.clone().into(),
    );
    params.insert(
        schema.payload_json().to_string(),
        row.payload_json.clone().into(),
    );
    params.insert(
        schema.payload_sha256().to_string(),
        row.payload_sha256.clone().into(),
    );
    params.insert(
        schema.tool_call_id().to_string(),
        option_string(&row.tool_call_id),
    );
    params.insert(
        schema.tool_name().to_string(),
        option_string(&row.tool_name),
    );
    params.insert(
        schema.message_id().to_string(),
        option_string(&row.message_id),
    );
    params.insert(schema.outcome().to_string(), option_string(&row.outcome));
    params.insert(
        schema.recorded_at().to_string(),
        row.recorded_at.clone().into(),
    );
    params
}

fn exchange_params(
    row: &EvalModelExchangeRow,
    schema: &ModelExchangeSchema,
) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        schema.exchange_id().to_string(),
        row.exchange_id.clone().into(),
    );
    params.insert(schema.turn_id().to_string(), row.turn_id.clone().into());
    params.insert(
        schema.response_index().to_string(),
        row.response_index.into(),
    );
    params.insert(
        schema.assistant_message_id().to_string(),
        row.assistant_message_id.clone().into(),
    );
    params.insert(
        schema.provider_response_id().to_string(),
        row.provider_response_id.clone().into(),
    );
    params.insert(schema.model_id().to_string(), row.model_id.clone().into());
    params.insert(
        schema.finish_reason().to_string(),
        option_string(&row.finish_reason),
    );
    params.insert(
        schema.usage_prompt_tokens().to_string(),
        option_i64(row.usage_prompt_tokens),
    );
    params.insert(
        schema.usage_completion_tokens().to_string(),
        option_i64(row.usage_completion_tokens),
    );
    params.insert(
        schema.usage_total_tokens().to_string(),
        option_i64(row.usage_total_tokens),
    );
    params.insert(
        schema.response_ref().to_string(),
        option_string(&row.response_ref),
    );
    params.insert(
        schema.response_sha256().to_string(),
        row.response_sha256.clone().into(),
    );
    params.insert(
        schema.source_ref().to_string(),
        option_string(&row.source_ref),
    );
    params.insert(
        schema.recorded_at().to_string(),
        row.recorded_at.clone().into(),
    );
    params
}

fn message_params(
    row: &EvalMessageEventRow,
    schema: &MessageEventSchema,
) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        schema.message_event_id().to_string(),
        row.message_event_id.clone().into(),
    );
    params.insert(schema.turn_id().to_string(), row.turn_id.clone().into());
    params.insert(
        schema.source_event_index().to_string(),
        option_i64(row.source_event_index),
    );
    params.insert(
        schema.message_id().to_string(),
        option_string(&row.message_id),
    );
    params.insert(schema.role().to_string(), row.role.clone().into());
    params.insert(schema.status().to_string(), option_string(&row.status));
    params.insert(
        schema.tool_call_id().to_string(),
        option_string(&row.tool_call_id),
    );
    params.insert(
        schema.content_len().to_string(),
        option_i64(row.content_len),
    );
    params.insert(
        schema.content_preview().to_string(),
        option_string(&row.content_preview),
    );
    params.insert(
        schema.content_sha256().to_string(),
        option_string(&row.content_sha256),
    );
    params.insert(
        schema.recorded_at().to_string(),
        row.recorded_at.clone().into(),
    );
    params
}

fn tool_params(row: &EvalToolEventRow, schema: &ToolEventSchema) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        schema.tool_event_id().to_string(),
        row.tool_event_id.clone().into(),
    );
    params.insert(schema.turn_id().to_string(), row.turn_id.clone().into());
    params.insert(
        schema.source_event_index().to_string(),
        row.source_event_index.into(),
    );
    params.insert(
        schema.tool_call_id().to_string(),
        row.tool_call_id.clone().into(),
    );
    params.insert(schema.tool_name().to_string(), row.tool_name.clone().into());
    params.insert(schema.status().to_string(), row.status.clone().into());
    params.insert(
        schema.request_id().to_string(),
        option_string(&row.request_id),
    );
    params.insert(
        schema.parent_id().to_string(),
        option_string(&row.parent_id),
    );
    params.insert(
        schema.args_json().to_string(),
        option_string(&row.args_json),
    );
    params.insert(
        schema.result_json().to_string(),
        option_string(&row.result_json),
    );
    params.insert(schema.ui_json().to_string(), option_string(&row.ui_json));
    params.insert(schema.error().to_string(), option_string(&row.error));
    params.insert(schema.latency_ms().to_string(), option_i64(row.latency_ms));
    params.insert(
        schema.recorded_at().to_string(),
        row.recorded_at.clone().into(),
    );
    params
}

#[cfg(test)]
mod schema_tests {
    use std::collections::BTreeMap;

    use cozo::DataValue;

    use super::*;

    #[test]
    fn agent_turn_create_scripts_are_stable() {
        assert_eq!(
            AgentTurnSchema::SCHEMA.script_create(),
            ":create eval_agent_turn { turn_id: String => campaign_id: String?, task_id: String, request_id: String?, selected_model: String, provider: String?, route_source: String?, trace_ref: String, summary_ref: String, full_response_ref: String?, trace_sha256: String, summary_sha256: String, full_response_sha256: String?, event_count: Int, response_count: Int, terminal_outcome: String?, final_message_id: String?, recorded_at: String }"
        );
        assert_eq!(
            AgentTurnEventSchema::SCHEMA.script_create(),
            ":create eval_agent_turn_event { event_id: String => turn_id: String, source_event_index: Int, event_kind: String, payload_json: String, payload_sha256: String, tool_call_id: String?, tool_name: String?, message_id: String?, outcome: String?, recorded_at: String }"
        );
        assert_eq!(
            ModelExchangeSchema::SCHEMA.script_create(),
            ":create eval_model_exchange { exchange_id: String => turn_id: String, response_index: Int, assistant_message_id: String, provider_response_id: String, model_id: String, finish_reason: String?, usage_prompt_tokens: Int?, usage_completion_tokens: Int?, usage_total_tokens: Int?, response_ref: String?, response_sha256: String, source_ref: String?, recorded_at: String }"
        );
        assert_eq!(
            MessageEventSchema::SCHEMA.script_create(),
            ":create eval_message_event { message_event_id: String => turn_id: String, source_event_index: Int?, message_id: String?, role: String, status: String?, tool_call_id: String?, content_len: Int?, content_preview: String?, content_sha256: String?, recorded_at: String }"
        );
        assert_eq!(
            ToolEventSchema::SCHEMA.script_create(),
            ":create eval_tool_event { tool_event_id: String => turn_id: String, source_event_index: Int, tool_call_id: String, tool_name: String, status: String, request_id: String?, parent_id: String?, args_json: String?, result_json: String?, ui_json: String?, error: String?, latency_ms: Int?, recorded_at: String }"
        );
    }

    #[test]
    fn agent_turn_put_script_is_stable() {
        let schema = &AgentTurnSchema::SCHEMA;
        let mut params = BTreeMap::new();
        for field in schema.all_fields() {
            params.insert(field.name().to_string(), DataValue::Null);
        }

        assert_eq!(
            schema.script_put(&params),
            "?[turn_id, campaign_id, task_id, request_id, selected_model, provider, route_source, trace_ref, summary_ref, full_response_ref, trace_sha256, summary_sha256, full_response_sha256, event_count, response_count, terminal_outcome, final_message_id, recorded_at] <- [[$turn_id, $campaign_id, $task_id, $request_id, $selected_model, $provider, $route_source, $trace_ref, $summary_ref, $full_response_ref, $trace_sha256, $summary_sha256, $full_response_sha256, $event_count, $response_count, $terminal_outcome, $final_message_id, $recorded_at]] :put eval_agent_turn { turn_id => campaign_id, task_id, request_id, selected_model, provider, route_source, trace_ref, summary_ref, full_response_ref, trace_sha256, summary_sha256, full_response_sha256, event_count, response_count, terminal_outcome, final_message_id, recorded_at }"
        );
    }
}
