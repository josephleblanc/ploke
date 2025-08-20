use std::collections::BTreeMap;

use cozo::{DataValue, ScriptMutability, UuidWrapper};
use serde::{Deserialize, Serialize};

use crate::{
    database::{to_string, to_uuid},
    Database, DbError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd)]
pub struct Validity {
    pub at: i64,        // epoch millis
    pub is_valid: bool, // asserted or retracted
}

#[derive(Debug, Clone)]
pub struct ConversationTurn {
    pub id: uuid::Uuid,
    pub parent_id: Option<uuid::Uuid>,
    pub message_id: uuid::Uuid,
    pub kind: String, // "user" | "assistant" | "system" | "sysinfo" | "tool"
    pub content: String,
    pub created_at: Validity,
    pub thread_id: Option<uuid::Uuid>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd)]
pub struct ToolCallReq {
    pub request_id: uuid::Uuid,
    pub call_id: String,
    pub parent_id: uuid::Uuid,
    pub vendor: String,
    pub tool_name: String,
    pub args_sha256: String,
    pub arguments_json: Option<String>,
    pub started_at: Validity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Serialize, Deserialize)]
pub enum ToolStatus {
    Completed,
    Failed,
}

impl ToolStatus {
    fn as_str(&self) -> &'static str {
        match self {
            ToolStatus::Completed => "completed",
            ToolStatus::Failed => "failed",
        }
    }
    fn from_str(s: &str) -> Option<Self> {
        match s {
            "completed" => Some(ToolStatus::Completed),
            "failed" => Some(ToolStatus::Failed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct ToolCallDone {
    pub request_id: uuid::Uuid,
    pub call_id: String,
    pub ended_at: Validity,
    pub latency_ms: i64,
    pub outcome_json: Option<String>, // on completed
    pub error_kind: Option<String>,   // on failed
    pub error_msg: Option<String>,    // on failed
    pub status: ToolStatus,           // Completed | Failed
}

pub trait ObservabilityStore {
    // Conversation
    fn upsert_conversation_turn(&self, turn: ConversationTurn) -> Result<(), DbError>;
    fn list_conversation_since(
        &self,
        since_ms: i64,
        limit: usize,
    ) -> Result<Vec<ConversationTurn>, DbError>;

    // Tool calls
    fn record_tool_call_requested(&self, req: ToolCallReq) -> Result<(), DbError>;
    fn record_tool_call_done(&self, done: ToolCallDone) -> Result<(), DbError>;
    fn get_tool_call(
        &self,
        request_id: uuid::Uuid,
        call_id: &str,
    ) -> Result<Option<(ToolCallReq, Option<ToolCallDone>)>, DbError>;
    fn list_tool_calls_by_parent(
        &self,
        parent_id: uuid::Uuid,
        limit: usize,
    ) -> Result<Vec<(ToolCallReq, Option<ToolCallDone>)>, DbError>;
}

impl Database {
    fn ensure_observability_schema(&self) -> Result<(), DbError> {
        // conversation_turn
        let create_conversation = r#"
:create conversation_turn {
    id: Uuid,
    at: Validity
    =>
    parent_id: Uuid?,
    message_id: Uuid,
    kind: String,
    content: String,
    thread_id: Uuid?
}
"#;
        // tool_call (time-travel)
        let create_tool_call = r#"
:create tool_call {
    request_id: Uuid,
    call_id: String,
    at: Validity
    =>
    parent_id: Uuid,
    vendor: String,
    tool_name: String,
    args_sha256: String,
    arguments_json: Json?,
    status: String,
    ended_at_ms: Int?,
    latency_ms: Int?,
    outcome_json: Json?,
    error_kind: String?,
    error_msg: String?
}
"#;

        // Attempt to create; ignore errors if already exist
        for script in [create_conversation, create_tool_call] {
            if let Err(e) = self.run_script(script, BTreeMap::new(), ScriptMutability::Mutable) {
                let msg = e.to_string();
                // Best-effort idempotency: ignore "exists"/"duplicate"/"conflict" errors
                if !(msg.contains("exists")
                    || msg.contains("duplicate")
                    || msg.contains("Duplicate")
                    || msg.contains("already")
                    || msg.contains("conflicts with an existing one")
                    || msg.to_lowercase().contains("conflict"))
                {
                    return Err(DbError::Cozo(msg));
                }
            }
        }
        Ok(())
    }
}

impl ObservabilityStore for Database {
    fn upsert_conversation_turn(&self, turn: ConversationTurn) -> Result<(), DbError> {
        self.ensure_observability_schema()?;

        let mut params = BTreeMap::new();
        params.insert("id".into(), DataValue::Uuid(UuidWrapper(turn.id)));
        params.insert(
            "parent_id".into(),
            turn.parent_id
                .map(|u| DataValue::Uuid(UuidWrapper(u)))
                .unwrap_or(DataValue::Null),
        );
        params.insert(
            "message_id".into(),
            DataValue::Uuid(UuidWrapper(turn.message_id)),
        );
        params.insert("kind".into(), DataValue::Str(turn.kind.into()));
        params.insert("content".into(), DataValue::Str(turn.content.into()));
        params.insert(
            "thread_id".into(),
            turn.thread_id
                .map(|u| DataValue::Uuid(UuidWrapper(u)))
                .unwrap_or(DataValue::Null),
        );

        let script = r#"
{
    ?[id, at, parent_id, message_id, kind, content, thread_id] :=
        id = $id,
        at = 'ASSERT',
        parent_id = $parent_id,
        message_id = $message_id,
        kind = $kind,
        content = $content,
        thread_id = $thread_id
    :put conversation_turn { id, at => parent_id, message_id, kind, content, thread_id }
}
"#;
        self.run_script(script, params, ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|e| DbError::Cozo(e.to_string()))
    }

    fn list_conversation_since(
        &self,
        since_ms: i64,
        limit: usize,
    ) -> Result<Vec<ConversationTurn>, DbError> {
        self.ensure_observability_schema()?;

        let mut params = BTreeMap::new();
        params.insert("since".into(), DataValue::from(since_ms));
        params.insert("limit".into(), DataValue::from(limit as i64));

        let script = r#"
?[id, parent_id, message_id, kind, content, thread_id, at_ms, at_valid] :=
    *conversation_turn{ id, at, parent_id, message_id, kind, content, thread_id @ 'NOW' },
    at_ms = to_int(at),
    at_valid = to_bool(at),
    at_ms >= $since
    :sort at_ms
    :limit $limit
"#;
        let rows = self
            .run_script(script, params, ScriptMutability::Immutable)
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        let hid = rows
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| (h.clone(), i))
            .collect::<std::collections::HashMap<_, _>>();

        let mut out = Vec::new();
        for row in rows.rows {
            let id = to_uuid(&row[*hid.get("id").unwrap()])?;
            let parent_id = match &row[*hid.get("parent_id").unwrap()] {
                DataValue::Uuid(UuidWrapper(u)) => Some(*u),
                DataValue::Null => None,
                _ => None,
            };
            let message_id = to_uuid(&row[*hid.get("message_id").unwrap()])?;
            let kind = to_string(&row[*hid.get("kind").unwrap()])?;
            let content = to_string(&row[*hid.get("content").unwrap()])?;
            let thread_id = match &row[*hid.get("thread_id").unwrap()] {
                DataValue::Uuid(UuidWrapper(u)) => Some(*u),
                DataValue::Null => None,
                _ => None,
            };
            let at_ms = row[*hid.get("at_ms").unwrap()]
                .get_int()
                .unwrap_or_default();
            let at_valid = row[*hid.get("at_valid").unwrap()]
                .get_bool()
                .unwrap_or(true);

            out.push(ConversationTurn {
                id,
                parent_id,
                message_id,
                kind,
                content,
                created_at: Validity {
                    at: at_ms,
                    is_valid: at_valid,
                },
                thread_id,
            });
        }
        Ok(out)
    }

    fn record_tool_call_requested(&self, req: ToolCallReq) -> Result<(), DbError> {
        self.ensure_observability_schema()?;

        // Check current state for idempotency/upsert semantics
        if let Some((existing_req, existing_done)) =
            self.get_tool_call(req.request_id, &req.call_id)?
        {
            // If already completed/failed, it's a no-op
            if existing_done.is_some() {
                return Ok(());
            }
            // If the requested state matches (ignoring started_at which is volatile), it's a no-op
            if existing_req.request_id == req.request_id
                && existing_req.call_id == req.call_id
                && existing_req.parent_id == req.parent_id
                && existing_req.vendor == req.vendor
                && existing_req.tool_name == req.tool_name
                && existing_req.args_sha256 == req.args_sha256
                && existing_req.arguments_json == req.arguments_json
            {
                return Ok(());
            }
            // Otherwise, we will assert a new "requested" fact with updated metadata below.
        }

        // Use DataValue::Json for JSON data, or Null when absent
        let arguments_json_value = match &req.arguments_json {
            Some(json_str) => {
                // Try to parse as JSON, if it fails, store as string
                match serde_json::from_str::<serde_json::Value>(json_str) {
                    Ok(_) => DataValue::Str(json_str.clone()),
                    Err(_) => DataValue::Null,
                }
            }
            None => DataValue::Null,
        };

        let mut params = BTreeMap::new();
        params.insert(
            "request_id".into(),
            DataValue::Uuid(UuidWrapper(req.request_id)),
        );
        params.insert("call_id".into(), DataValue::Str(req.call_id.into()));
        params.insert(
            "parent_id".into(),
            DataValue::Uuid(UuidWrapper(req.parent_id)),
        );
        params.insert("vendor".into(), DataValue::Str(req.vendor.into()));
        params.insert("tool_name".into(), DataValue::Str(req.tool_name.into()));
        params.insert("args_sha256".into(), DataValue::Str(req.args_sha256.into()));
        params.insert("arguments_json".into(), arguments_json_value);

        // Upsert requested state
        let script = r#"
{
    ?[request_id, call_id, at, parent_id, vendor, tool_name, args_sha256, arguments_json, status, ended_at_ms, latency_ms, outcome_json, error_kind, error_msg] :=
        request_id = $request_id,
        call_id = $call_id,
        at = 'ASSERT',
        parent_id = $parent_id,
        vendor = $vendor,
        tool_name = $tool_name,
        args_sha256 = $args_sha256,
        arguments_json = $arguments_json,
        status = "requested",
        ended_at_ms = null,
        latency_ms = null,
        outcome_json = null,
        error_kind = null,
        error_msg = null
    :put tool_call { request_id, call_id, at => parent_id, vendor, tool_name, args_sha256, arguments_json, status, ended_at_ms, latency_ms, outcome_json, error_kind, error_msg }
}
"#;

        self.run_script(script, params, ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|e| DbError::Cozo(e.to_string()))
    }

    fn record_tool_call_done(&self, done: ToolCallDone) -> Result<(), DbError> {
        self.ensure_observability_schema()?;

        // Enforce lifecycle rules and idempotency, and capture request metadata
        let req_meta = match self.get_tool_call(done.request_id, &done.call_id)? {
            None => {
                // Must have an existing requested row to transition to done
                return Err(DbError::InvalidLifecycle(
                    "Cannot record completion without a prior requested row".into(),
                ));
            }
            Some((req, Some(existing_done))) => {
                // Already completed/failed
                if existing_done == done {
                    // exact same payload -> no-op
                    return Ok(());
                }
                if existing_done.status != done.status {
                    // requested → completed → failed (or vice versa) is invalid
                    return Err(DbError::InvalidLifecycle(
                        "Cannot change terminal status once recorded".into(),
                    ));
                }
                // Same terminal status but different payload: proceed with update
                req
            }
            Some((req, None)) => {
                // Ok to proceed and assert terminal status below
                req
            }
        };

        // Prepare params for commit
        let mut params = BTreeMap::new();
        params.insert(
            "request_id".into(),
            DataValue::Uuid(UuidWrapper(done.request_id)),
        );
        params.insert("call_id".into(), DataValue::Str(done.call_id.into()));
        params.insert("ended_at_ms".into(), DataValue::from(done.ended_at.at));
        params.insert("latency_ms".into(), DataValue::from(done.latency_ms));
        params.insert("status".into(), DataValue::Str(done.status.as_str().into()));

        // Carry forward metadata from the original requested call
        params.insert(
            "parent_id".into(),
            DataValue::Uuid(UuidWrapper(req_meta.parent_id)),
        );
        params.insert("vendor".into(), DataValue::Str(req_meta.vendor.into()));
        params.insert(
            "tool_name".into(),
            DataValue::Str(req_meta.tool_name.into()),
        );
        params.insert(
            "args_sha256".into(),
            DataValue::Str(req_meta.args_sha256.into()),
        );

        // Handle JSON data properly
        let arguments_json_value = match &req_meta.arguments_json {
            Some(json_str) => {
                match serde_json::from_str::<serde_json::Value>(json_str) {
                    Ok(_) => DataValue::Str(json_str.clone()),
                    Err(_) => DataValue::Null,
                }
            }
            None => DataValue::Null,
        };
        params.insert("arguments_json".into(), arguments_json_value);

        // Handle outcome JSON properly
        let outcome_json_value = match &done.outcome_json {
            Some(json_str) => {
                match serde_json::from_str::<serde_json::Value>(json_str) {
                    Ok(_) => DataValue::Str(json_str.clone()),
                    Err(_) => DataValue::Null,
                }
            }
            None => DataValue::Null,
        };
        params.insert("outcome_json".into(), outcome_json_value);

        params.insert(
            "error_kind".into(),
            done.error_kind
                .clone()
                .map(|s| DataValue::Str(s.into()))
                .unwrap_or(DataValue::Null),
        );
        params.insert(
            "error_msg".into(),
            done.error_msg
                .clone()
                .map(|s| DataValue::Str(s.into()))
                .unwrap_or(DataValue::Null),
        );

        let script = r#"
{
    ?[request_id, call_id, at, parent_id, vendor, tool_name, args_sha256, arguments_json, status, ended_at_ms, latency_ms, outcome_json, error_kind, error_msg] :=
        at = 'ASSERT',
        request_id = $request_id,
        call_id = $call_id,
        parent_id = $parent_id,
        vendor = $vendor,
        tool_name = $tool_name,
        args_sha256 = $args_sha256,
        arguments_json = $arguments_json,
        status = $status,
        ended_at_ms = $ended_at_ms,
        latency_ms = $latency_ms,
        outcome_json = $outcome_json,
        error_kind = $error_kind,
        error_msg = $error_msg
    :put tool_call { request_id, call_id, at => parent_id, vendor, tool_name, args_sha256, arguments_json, status, ended_at_ms, latency_ms, outcome_json, error_kind, error_msg }
}
"#;

        self.run_script(script, params, ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|e| DbError::Cozo(e.to_string()))
    }

    fn get_tool_call(
        &self,
        request_id: uuid::Uuid,
        call_id: &str,
    ) -> Result<Option<(ToolCallReq, Option<ToolCallDone>)>, DbError> {
        self.ensure_observability_schema()?;

        let mut params = BTreeMap::new();
        params.insert(
            "request_id".into(),
            DataValue::Uuid(UuidWrapper(request_id)),
        );
        params.insert("call_id".into(), DataValue::Str(call_id.into()));

        // Use dump_json to return JSON strings to the client
        let script = r#"
?[request_id, call_id, parent_id, vendor, tool_name, args_sha256, arguments_json_s, status, ended_at_ms, latency_ms, outcome_json_s, error_kind, error_msg, at_ms, at_valid] :=
    *tool_call{
        request_id, call_id, at, parent_id, vendor, tool_name, args_sha256, arguments_json, status, ended_at_ms, latency_ms, outcome_json, error_kind, error_msg
        @ 'NOW'
    },
    request_id = $request_id,
    call_id = $call_id,
    arguments_json_s = if(is_null(arguments_json), null, dump_json(arguments_json)),
    outcome_json_s = if(is_null(outcome_json), null, dump_json(outcome_json)),
    at_ms = to_int(at),
    at_valid = to_bool(at)
"#;

        let rows = self
            .run_script(script, params, ScriptMutability::Immutable)
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        if rows.rows.is_empty() {
            return Ok(None);
        }

        let hid = rows
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| (h.clone(), i))
            .collect::<std::collections::HashMap<_, _>>();
        let row = &rows.rows[0];

        let req = ToolCallReq {
            request_id: to_uuid(&row[*hid.get("request_id").unwrap()])?,
            call_id: to_string(&row[*hid.get("call_id").unwrap()])?,
            parent_id: to_uuid(&row[*hid.get("parent_id").unwrap()])?,
            vendor: to_string(&row[*hid.get("vendor").unwrap()])?,
            tool_name: to_string(&row[*hid.get("tool_name").unwrap()])?,
            args_sha256: to_string(&row[*hid.get("args_sha256").unwrap()])?,
            arguments_json: row[*hid.get("arguments_json_s").unwrap()]
                .get_str()
                .map(|s| s.to_string())
                .filter(|s| s != "null"),
            started_at: Validity {
                at: row[*hid.get("at_ms").unwrap()]
                    .get_int()
                    .unwrap_or_default(),
                is_valid: row[*hid.get("at_valid").unwrap()]
                    .get_bool()
                    .unwrap_or(true),
            },
        };

        let status = to_string(&row[*hid.get("status").unwrap()])?;
        let done = if let Some(st) = ToolStatus::from_str(&status) {
            Some(ToolCallDone {
                request_id: req.request_id,
                call_id: req.call_id.clone(),
                ended_at: Validity {
                    at: row[*hid.get("ended_at_ms").unwrap()]
                        .get_int()
                        .unwrap_or_default(),
                    is_valid: true,
                },
                latency_ms: row[*hid.get("latency_ms").unwrap()]
                    .get_int()
                    .unwrap_or_default(),
                outcome_json: row[*hid.get("outcome_json_s").unwrap()]
                    .get_str()
                    .map(|s| s.to_string())
                    .filter(|s| s != "null"),
                error_kind: row[*hid.get("error_kind").unwrap()]
                    .get_str()
                    .map(|s| s.to_string()),
                error_msg: row[*hid.get("error_msg").unwrap()]
                    .get_str()
                    .map(|s| s.to_string()),
                status: st,
            })
        } else {
            None
        };

        Ok(Some((req, done)))
    }

    fn list_tool_calls_by_parent(
        &self,
        parent_id: uuid::Uuid,
        limit: usize,
    ) -> Result<Vec<(ToolCallReq, Option<ToolCallDone>)>, DbError> {
        self.ensure_observability_schema()?;

        let mut params = BTreeMap::new();
        params.insert("parent_id".into(), DataValue::Uuid(UuidWrapper(parent_id)));
        params.insert("limit".into(), DataValue::from(limit as i64));

        let script = r#"
?[request_id, call_id, parent_id, vendor, tool_name, args_sha256, arguments_json_s, status, ended_at_ms, latency_ms, outcome_json_s, error_kind, error_msg, at_ms, at_valid] :=
    *tool_call{
        request_id, call_id, at, parent_id, vendor, tool_name, args_sha256, arguments_json, status, ended_at_ms, latency_ms, outcome_json, error_kind, error_msg
        @ 'NOW'
    },
    parent_id = $parent_id,
    arguments_json_s = if(is_null(arguments_json), null, dump_json(arguments_json)),
    outcome_json_s = if(is_null(outcome_json), null, dump_json(outcome_json)),
    at_ms = to_int(at),
    at_valid = to_bool(at)
    :sort at_ms desc
    :limit $limit
"#;

        let rows = self
            .run_script(script, params, ScriptMutability::Immutable)
            .map_err(|e| DbError::Cozo(e.to_string()))?;

        // Group by (request_id, call_id)
        let hid = rows
            .headers
            .iter()
            .enumerate()
            .map(|(i, h)| (h.clone(), i))
            .collect::<std::collections::HashMap<_, _>>();

        let mut out: Vec<(ToolCallReq, Option<ToolCallDone>)> = Vec::new();
        for row in rows.rows {
            let req = ToolCallReq {
                request_id: to_uuid(&row[*hid.get("request_id").unwrap()])?,
                call_id: to_string(&row[*hid.get("call_id").unwrap()])?,
                parent_id: to_uuid(&row[*hid.get("parent_id").unwrap()])?,
                vendor: to_string(&row[*hid.get("vendor").unwrap()])?,
                tool_name: to_string(&row[*hid.get("tool_name").unwrap()])?,
                args_sha256: to_string(&row[*hid.get("args_sha256").unwrap()])?,
                arguments_json: row[*hid.get("arguments_json_s").unwrap()]
                    .get_str()
                    .map(|s| s.to_string())
                    .filter(|s| s != "null"),
                started_at: Validity {
                    at: row[*hid.get("at_ms").unwrap()]
                        .get_int()
                        .unwrap_or_default(),
                    is_valid: row[*hid.get("at_valid").unwrap()]
                        .get_bool()
                        .unwrap_or(true),
                },
            };

            let status = to_string(&row[*hid.get("status").unwrap()])?;
            let maybe_done = if let Some(st) = ToolStatus::from_str(&status) {
                Some(ToolCallDone {
                    request_id: req.request_id,
                    call_id: req.call_id.clone(),
                    ended_at: Validity {
                        at: row[*hid.get("ended_at_ms").unwrap()]
                            .get_int()
                            .unwrap_or_default(),
                        is_valid: true,
                    },
                    latency_ms: row[*hid.get("latency_ms").unwrap()]
                        .get_int()
                        .unwrap_or_default(),
                    outcome_json: row[*hid.get("outcome_json_s").unwrap()]
                        .get_str()
                        .map(|s| s.to_string())
                        .filter(|s| s != "null"),
                    error_kind: row[*hid.get("error_kind").unwrap()]
                        .get_str()
                        .map(|s| s.to_string()),
                    error_msg: row[*hid.get("error_msg").unwrap()]
                        .get_str()
                        .map(|s| s.to_string()),
                    status: st,
                })
            } else {
                None
            };

            out.push((req, maybe_done));
        }

        Ok(out)
    }
}
