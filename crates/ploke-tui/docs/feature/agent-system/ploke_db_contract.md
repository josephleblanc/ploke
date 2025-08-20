# ploke-db contract for Milestone 0 (observability and persistence)

Purpose

Update 2025-08-19: Cozo v0.7 semantics and decisions
- Time-travel via Validity (v0.7):
  - Any evolving relation should use a last key part typed as Validity. We will treat tool_call and conversation_turn as time-travel-enabled relations.
  - Use literals 'ASSERT'/'RETRACT' on insertion to get a transaction-stable timestamp for all rows in the same transaction. Use '@ NOW' or '@ <RFC3339>' for queries; 'END' is the logical end-of-time.
  - When two rows share identical non-Validity keys and identical timestamps but differ in assert flags, 'true' (assert) wins at that exact timestamp.
- Tool-call lifecycle modeling:
  - Relation key: (request_id, call_id, at: Validity). Lifecycle is represented by successive assertions with updated status and metadata. No in-place updates; status transitions are new facts.
  - Recommended columns: request_id (Uuid), call_id (String), parent_id (Uuid), vendor (String), tool_name (String), args_sha256 (String), arguments_json (Json?), status (String: "requested" | "completed" | "failed"), ended_at_ms (Int?), latency_ms (Int?), outcome_json (Json?), error_kind (String?), error_msg (String?).
  - The presence of an asserted row with status="completed"/"failed" at a later timestamp supersedes earlier "requested" facts when querying '@ NOW'.
- Json fields (cozo Json type):
  - Prefer typed Json columns over stringified JSON. Use json()/parse_json()/dump_json() as needed, and json path helpers (get/maybe_get/set_json_path/remove_json_path) for partial updates.
  - For privacy and minimal storage, default to redaction (store args_sha256 only); allow storing arguments_json/outcome_json when explicitly enabled.
- Paths:
  - Persist project-relative paths instead of absolute paths. ploke-io continues to enforce absolute-path and symlink policies; ploke-db stores normalized relative paths for portability and privacy.
- Practical query helpers:
  - Use format_timestamp(at) to render Validity timestamps; to_bool(at) and to_int(at) for flag/timestamp extraction.
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
// USER: Added typed timestamp for better cozo compat
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

// USER: Added typed ToolStatus
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

USER: Note the following `register_callback` and `run_multi_transaction` command for `cozo::Db`, which we can integrate for better observability of database actions for the in-memory, embedded Cozo database (taken from docs.rs/cozo/latest website):
```rust
    /// Run a multi-transaction. A command should be sent to `payloads`, and the result should be
280    /// retrieved from `results`. A transaction ends when it receives a `Commit` or `Abort`,
281    /// or when a query is not successful. After a transaction ends, sending / receiving from
282    /// the channels will fail.
283    ///
284    /// Write transactions _may_ block other reads, but we guarantee that this does not happen
285    /// for the RocksDB backend.
286    pub fn run_multi_transaction(
287        &'s self,
288        is_write: bool,
289        payloads: Receiver<TransactionPayload>,
290        results: Sender<Result<NamedRows>>,
291    ) {
292        let tx = if is_write {
293            self.transact_write()
294        } else {
295            self.transact()
296        };
297        let mut cleanups: Vec<(Vec<u8>, Vec<u8>)> = vec![];
298        let mut tx = match tx {
299            Ok(tx) => tx,
300            Err(err) => {
301                let _ = results.send(Err(err));
302                return;
303            }
304        };
305
306        let ts = current_validity();
307        let callback_targets = self.current_callback_targets();
308        let mut callback_collector = BTreeMap::new();
309        let mut write_locks = BTreeMap::new();
310
311        for payload in payloads {
312            match payload {
313                TransactionPayload::Commit => {
314                    for (lower, upper) in cleanups {
315                        if let Err(err) = tx.store_tx.del_range_from_persisted(&lower, &upper) {
316                            eprintln!("{err:?}")
317                        }
318                    }
319
320                    let _ = results.send(tx.commit_tx().map(|_| NamedRows::default()));
321                    #[cfg(not(target_arch = "wasm32"))]
322                    if !callback_collector.is_empty() {
323                        self.send_callbacks(callback_collector)
324                    }
325
326                    break;
327                }
328                TransactionPayload::Abort => {
329                    let _ = results.send(Ok(NamedRows::default()));
330                    break;
331                }
332                TransactionPayload::Query((script, params)) => {
333                    let p =
334                        match parse_script(&script, &params, &self.fixed_rules.read().unwrap(), ts)
335                        {
336                            Ok(p) => p,
337                            Err(err) => {
338                                if results.send(Err(err)).is_err() {
339                                    break;
340                                } else {
341                                    continue;
342                                }
343                            }
344                        };
345
346                    let p = match p.get_single_program() {
347                        Ok(p) => p,
348                        Err(err) => {
349                            if results.send(Err(err)).is_err() {
350                                break;
351                            } else {
352                                continue;
353                            }
354                        }
355                    };
356                    if let Some(write_lock_name) = p.needs_write_lock() {
357                        match write_locks.entry(write_lock_name) {
358                            Entry::Vacant(e) => {
359                                let lock = self
360                                    .obtain_relation_locks(iter::once(e.key()))
361                                    .pop()
362                                    .unwrap();
363                                e.insert(lock);
364                            }
365                            Entry::Occupied(_) => {}
366                        }
367                    }
368
369                    let res = self.execute_single_program(
370                        p,
371                        &mut tx,
372                        &mut cleanups,
373                        ts,
374                        &callback_targets,
375                        &mut callback_collector,
376                    );
377                    if results.send(res).is_err() {
378                        break;
379                    }
380                }
381            }
382        }
383    }
//...
/// Register callback channel to receive changes when the requested relation are successfully committed.
752    /// The returned ID can be used to unregister the callback channel.
753    #[cfg(not(target_arch = "wasm32"))]
754    pub fn register_callback(
755        &self,
756        relation: &str,
757        capacity: Option<usize>,
758    ) -> (u32, Receiver<(CallbackOp, NamedRows, NamedRows)>) {
759        let (sender, receiver) = if let Some(c) = capacity {
760            bounded(c)
761        } else {
762            unbounded()
763        };
764        let cb = CallbackDeclaration {
765            dependent: SmartString::from(relation),
766            sender,
767        };
768
769        let mut guard = self.event_callbacks.write().unwrap();
770        let new_id = self.callback_count.fetch_add(1, Ordering::SeqCst);
771        guard
772            .1
773            .entry(SmartString::from(relation))
774            .or_default()
775            .insert(new_id);
776
777        guard.0.insert(new_id, cb);
778        (new_id, receiver)
779    }
```

