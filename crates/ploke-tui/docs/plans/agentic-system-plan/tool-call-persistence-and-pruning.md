# Tool-Call Persistence & Pruning — Implementation Plan

Purpose
- Persist tool call outputs as first-class chat messages and include them in prompt construction for future turns.
- Add pruning mechanisms to keep conversations lean without losing auditability.
- Maintain current feature capability; no regressions or ad hoc reductions allowed.

Notes
- After each step, write unit tests for any new functionality introduced in that step, then run tests with: `cargo test -q -p ploke-tui --lib 2>&1 | tail -n 20`.
- If failures occur, follow the “If tests fail” guidance per step and fix before proceeding.
- Use strongly-typed structs/enums; avoid stringly-typed code.
- Keep edits minimal, focused, and safe; do not break unrelated areas.

---

## ✅/⬜ Progress Legend
- ⬜ pending
- ✅ completed

---

## Step 0 — Establish Baseline (no code change)
- [ ] Run the test suite to capture baseline behavior and to quickly spot regressions later.

How to test
- Command: `cargo test -q -p ploke-tui --lib 2>&1 | tail -n 20`
- If tests fail: investigate environment issues first; do not proceed until baseline is green.

Implementation notes
- Add any environment setup notes here.

---

$1
Tests to add
- Unit: construct `Message` with/without `tool_call_id`; assert `serde` round-trip and default `None` preservation.
- Unit: `add_tool_message` helper sets `MessageKind::Tool` and `tool_call_id` correctly and appends under parent.

Implementation notes
- Keep `tool_call_id` optional to avoid touching unrelated code paths immediately.

Status
- [ ]

---

$1
Tests to add
- Unit: `current_path_as_llm_request_messages` maps `MessageKind::Tool` with `tool_call_id` → `Role::Tool` message.
- Unit: ensure tool messages without `tool_call_id` are skipped (no `Role::Tool`).
- Unit: order stability with surrounding User/Assistant/System messages.

Implementation notes
- Maintain order and stability of existing roles.

Status
- [ ]

---

$1
Tests to add
- Unit: `StateCommand::AddToolMessage` variant discriminant uniqueness and Debug/serde if applicable.
- Unit: handler adds a Tool message under the specified parent and emits expected events.
- Unit: error handling when parent not found (returns expected error).

Implementation notes
- Use `MessageKind::Tool` and store `call_id` as `tool_call_id`.

Status
- [ ]

---

$1
Tests to add
- Unit: `RequestSession` initializes with `assistant_message_id` and forwards to tool completion path.
- Unit/Integration (offline): simulate tool completion event and assert `AddToolMessage` dispatched with correct IDs.
- Unit: ensure `req.core.messages.push(RequestMessage::new_tool(...))` still occurs for in-turn continuation.

Implementation notes
- Preserve the existing behavior for in-flight request; this change only adds persistent chat nodes.

Status
- [ ]

---

$1
Tests to add
- Unit: pruning keeps only last `k` Tool messages on current path; assert survivors and deleted set.
- Unit: non-Tool messages are never pruned.
- Unit: tree integrity preserved (children remain reachable or handled per contract).

Implementation notes
- Do not prune System/User/Assistant messages.

Status
- [ ]

---

$1
Tests to add
- Unit: TTL-based pruning removes Tool messages older than threshold; uses message timestamps deterministically in tests.
- Unit: budget pruning removes oldest Tool messages until token estimate ≤ budget.
- Unit: multiple policies combine with strictest semantics.
- Unit: config knobs deserialize with defaults and optionality.

Implementation notes
- If multiple policies are set, apply the strictest result (prune if any policy triggers).

Status
- [ ]

---

$1
Tests to add
- Unit/Integration (offline): after inserting Tool messages, handler triggers `PruneToolCalls` with policy from config.
- Unit: no-op when config is None; ensure no unexpected state changes.

Implementation notes
- Keep pruning light; avoid heavy operations in the realtime path.

Status
- [ ]

---

$1
Tests to add
- Unit: database methods compile and validate inputs; use in-memory or temp DB behind `#[cfg(test)]` if available.
- Unit: redaction behavior stores only hashed args by default.

Implementation notes
- Redact payloads by default; store `args_sha256` and minimal metadata unless configured otherwise.

Status
- [ ]

---

$1
Tests to add
- Unit: structured save/load round-trips `tool_call_id` and `kind: Tool`.
- Unit: backward compatibility for older saves (missing fields default correctly).

Implementation notes
- Keep current Markdown export; adding structured persistence can be staged later if needed.

Status
- [ ]

---

$1
Tests to add
- Integration: end-to-end offline flow produces Tool messages in prompt mapping and respects pruning.
- Snapshot (if applicable): UI reflects Tool messages appropriately.

Implementation notes
- Keep changes minimal and well-scoped; maintain safety-first editing and strong typing.

Status
- [ ]

---

## Non-Regression Checklist
- [ ] LLM tool-calls continue to execute and in-turn `RequestMessage::new_tool` continues to be pushed.
- [ ] Conversation navigation and rendering unaffected for non-tool messages.
- [ ] Safe-edit pipeline (apply_code_edit) unchanged except for additional Tool messages.
- [ ] No performance regressions in normal message flows.

## References
- `crates/ploke-tui/src/chat_history.rs`
- `crates/ploke-tui/src/app_state/{commands.rs, handlers/chat.rs, dispatcher.rs}`
- `crates/ploke-tui/src/llm/manager/{mod.rs, session.rs}`
- `crates/ploke-tui/src/user_config.rs`
- `docs/feature/agent-system/ploke_db_contract.md`
- `docs/workflows/message_and_tool_call_lifecycle.md`
