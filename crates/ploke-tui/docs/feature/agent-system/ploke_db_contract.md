# ploke-db contract for Milestone 0 (observability and persistence)

Purpose
- Define the minimal API and behavior that ploke-db must provide to support M0:
  - Persist conversation turns.
  - Persist tool call lifecycle (requested → completed/failed) with idempotency.
  - Enable audit/retrieval by request_id and call_id.

Key principles
- Idempotent upserts keyed on (request_id, call_id) for tool calls.
- Timestamps recorded at both request and completion.
- Stable, queryable fields for correlation (request_id, call_id, parent_id, vendor, tool_name).
- Avoid duplicating rows under retries.

Data model (logical)
- conversation_turns
  - id: Uuid (row id)
  - parent_id: Uuid (conversation parent message, optional)
  - message_id: Uuid (local message id from TUI)
  - kind: enum("user","assistant","system","sysinfo","tool")
  - content: string
  - at: Validity
  - ~~created_at: timestamp~~
    - superseded by cozo's Validity field type for time-travel
  - thread_id: Uuid (optional: future multi-thread support)
  - indexes: by (message_id), by (created_at DESC), by (thread_id, created_at)
- tool_calls
  - request_id: Uuid (correlation id)
  - call_id: string (provider id)
  - parent_id: Uuid (message id that triggered the call)
  - vendor: string ("openai", etc.)
  - tool_name: string ("request_code_context", "apply_code_edit", …)
  - args_sha256: string (hash of canonicalized arguments JSON)
  - arguments_json: string (optional, may be redacted)
  - status: enum("requested","completed","failed")
  - started_at: Validity
  - ~~started_at: timestamp~~
    - superseded by cozo's Validity field type for time-travel
  - ended_at: timestamp (nullable until completion)
  - latency_ms: integer (nullable until completion)
  - outcome_json: Json (on completed; redacted if needed)
  - error_kind: string (on failed)
  - error_msg: string (on failed)
  - indexes: unique (request_id, call_id), by (parent_id, started_at DESC), by (status)
- code_edit_proposals (for M1, defined now to stabilize schema)
  - request_id: Uuid
  - diffs_json: Json (cozo datatype)
  - confidence: float (nullable)
  - status: enum("pending","approved","denied","applied","reverted")
  - created_at: Validity
  - ~~created_at~~, decided_at, applied_at: timestamps (nullable)
    - created_at superseded by cozo's Validity field type for time-travel
  - commit_hash: string (nullable; git integration later)
  - indexes: by (status), by (created_at DESC)
- re: Cozo's treatment of Json, see cozo docs copied [here](../../../../../docs/dependency_details/cozo/types/json.md)
- re: Cozo's treatment of Validity in time-travel, see cozo docs copied [here](../../../../../docs/dependency_details/cozo/types/time-travel.md)


Rust API (trait sketch)
```rust
// Should implement From on cozo datatype
pub struct Validity {
    pub at: i64, // epoch millis
    pub is_valid: bool, // is asserted or retracted statement
}
pub struct ConversationTurn {
    pub id: uuid::Uuid,
    pub parent_id: Option<uuid::Uuid>,
    pub message_id: uuid::Uuid,
    pub kind: String,      // "user" | "assistant" | "system" | "sysinfo" | "tool"
    pub content: String,
    pub created_at: Validity,   // epoch millis
    pub thread_id: Option<uuid::Uuid>,
}

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

// Should implement serialize/deserialize for strongly typed database conversion
pub enum ToolStatus {
    Completed,
    Failed
}

pub struct ToolCallDone {
    pub request_id: uuid::Uuid,
    pub call_id: String,
    pub ended_at: Validity,
    pub latency_ms: i64,
    pub outcome_json: Option<String>,  // on completed
    pub error_kind: Option<String>,    // on failed
    pub error_msg: Option<String>,     // on failed
    pub status: ToolStatus,                // "completed" | "failed"
}

pub trait ObservabilityStore {
    // Conversation
    fn upsert_conversation_turn(&self, turn: ConversationTurn) -> Result<(), DbError>;
    fn list_conversation_since(&self, since_ms: i64, limit: usize) -> Result<Vec<ConversationTurn>, DbError>;

    // Tool calls
    fn record_tool_call_requested(&self, req: ToolCallReq) -> Result<(), DbError>;
    fn record_tool_call_done(&self, done: ToolCallDone) -> Result<(), DbError>;
    fn get_tool_call(&self, request_id: uuid::Uuid, call_id: &str) -> Result<Option<(ToolCallReq, Option<ToolCallDone>)>, DbError>;
    fn list_tool_calls_by_parent(&self, parent_id: uuid::Uuid, limit: usize) -> Result<Vec<(ToolCallReq, Option<ToolCallDone>)>, DbError>;
}
```

Behavioral requirements
- Idempotency:
  - record_tool_call_requested must act as upsert on (request_id, call_id). If a row exists in status in {"requested","completed","failed"}, do not duplicate; update non-key fields if needed.
  - record_tool_call_done must find an existing requested row and set status + ended_at + latency_ms + outcome/error fields; it must be safe to call multiple times with identical data.
- Timestamps:
  - Use provider-local monotonic source; store as epoch millis (i64).
- Redaction:
  - arguments_json and outcome_json may be None to avoid logging secrets; args_sha256 is always required and is used for correlation.
- Query performance:
  - Provide indexes described above to support audits and UI timelines.

Cozo implementation notes
- Represent relations with appropriate Cozo schemas; ensure unique constraint on (request_id, call_id) via relation key design.
- Provide raw_query helpers for debugging, but expose typed functions for the app.
- Add migrations idempotently; tolerate re-running initialization.

Testing (M0)
- Unit tests:
  - Idempotent upsert for requested; done with same data twice → unchanged.
  - requested → completed → failed is invalid (reject); completed → completed with same payload is no-op.
- Integration:
  - End-to-end from TUI mock: emit ToolCallRequested/Completed, then query ploke-db for the pair and assert fields populated.

Future extensions (beyond M0)
- code_edit_proposals lifecycle and git commit linkage.
- retention policies for logs and conversation turns.
- richer retrieval (by tool_name, vendor, time windows, etc.).

Blocking decisions (tracked in decisions_required.md)
- Whether to persist full arguments_json/outcome_json by default or store only hashes (privacy/PII).
- Default retention period for tool_calls and conversation_turns.
