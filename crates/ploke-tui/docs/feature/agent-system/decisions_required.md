# Decisions requiring USER review

Resolved decisions (2025-08-19)
- 1) Chat history persistence
  - Decision: Persist chat history to the database with metadata (not to a filesystem path) as the primary mechanism. File export remains optional and can be added later via a configurable command. M0 deliverables and tests should reflect DB persistence first. FileManager fixes remain useful as a fallback/export path only.
- 2) Typed LLM tool events
  - Decision: Adopt (a) bridge approach in M0. Introduce typed tool events and keep a compatibility bridge from SystemEvent::ToolCallRequested for one milestone. Remove the deprecated path in M1.
- 3) Tool-call payload persistence and paths
  - Decision: Default to storing hashes only (args_sha256) for privacy. Persist arguments_json/outcome_json as cozo Json only when explicitly enabled. Normalize file paths to project-relative (workspace-root-relative) paths for persistence; keep absolute-path requirement enforced at IO (ploke-io) level. Record root mapping for later reconstruction as needed.
- 4) EventBus capacities
  - Decision: Keep current defaults (realtime=100, background=1000, index=1000). Add metrics/logging for lag observations; consider config later.
- 5) Observability log retention
  - Decision: Daily rotation, keep 7 days by default; make configurable in a later milestone.

New questions (to resolve pre-M0 where feasible)
- Q1: Tool-call relation modeling in Cozo
  - Proposal: Make tool_call time-travel-enabled with the last key part typed as Validity (… , at: Validity). Record lifecycle via new asserted rows (requested → completed/failed), not in-place updates. Accept this as the contract?
    - USER: Yes, Accept.
- Q2: Relation naming (singular vs plural)
  - Proposal: Use singular relation names (tool_call, conversation_turn, code_edit_proposal) to align with current schema style. Accept?
    - USER: Yes, Accept.
- Q3: Namespace semantics
  - Proposal: Use PROJECT_NAMESPACE_UUID by default for observability rows; allow provider-specific overrides later. Accept?
    - USER: Yes, Accept. This is pending updates to the way the PROJECT_NAMESPACE_UUID is calculated in the `syn_parser` crate. Fine during development, consider as blocker before prod-ready. Required for future integrations between target parsed crates to be integrated in code graph in database (outside scope of `ploke-tui`).
- Q4: Redaction toggles
  - Proposal: A ploke-db config toggle (and/or per-call parameter) controlling whether arguments_json/outcome_json are stored as Json or redacted. Default: redacted (store only hash, status, and metadata). Accept?
    - USER: No, for now we will store everything for debugging as we rapidly develop the project. Consider this again during pre prod-ready checks and address as blocker to full prod-readiness.

Purpose
- This file is the single queue for blockers and directional decisions that require USER approval.
- Only add items that block progress or significantly determine future direction.
- For each item, include: context, options (with tradeoffs), recommended default, and any deadline.

Open decisions (M0 focus)
1) Chat history file path and naming
   - Context: FileManager::save_content currently writes a temp file; final path needs to be stable and user-visible.
   - Options:
     a) Save to CWD/.ploke_history.md (simple, visible, per-project).
     b) Save to XDG documents dir (~/.local/share/ploke/history.md) (centralized).
     c) Per-session timestamped files under logs/ (no overwrites, harder to find).
   - Recommended: (a) for M0; add config later.
   - Blocker: minor (unblock M0 once chosen).
    - USER: Neither, save to database with metadata instead. Later we can add config options for users that may wish to inspect/edit the files, etc.

2) Typed LLM tool events migration timing
   - Context: SystemEvent::ToolCallRequested is deprecated; migration risk to break routing.
   - Options:
     a) Land typed events in M0 with compatibility bridge; remove old in M1.
     b) Defer typed events entirely to M1.
   - Recommended: (a) to get telemetry attached to typed events early.
   - Blocker: none if bridge used; confirm direction.
    - USER: Agreed, get the longer-term single source of truth working early, avoid tech debt.

3) Tool-call payload persistence (privacy)
   - Context: Storing full arguments_json and outcome_json may include secrets/paths.
   - Options:
     a) Store full JSON; rely on logs redaction patterns later.
     b) Store only hashes by default; full payload behind debug flag/config.
   - Recommended: (b) for M0; full payload optional via config in M1.
   - Blocker: affects ploke-db schema defaults.
    - USER: We should transition out of absolute paths and into local paths instead. This is a longer-term solution that will avoid issues with deserializing the hashed values. May require updating many files, ultimately worth it. Should unify our approach to file-handling in any case to make project more maintainable. Consider cost/benefit of moving all IO logic into `ploke-io` or keeping minimal layer as-is in `ploke-tui` for currently functioning operations.

4) EventBus channel capacities (defaults)
   - Context: Avoid lag; keep realtime snappy.
   - Options:
     a) realtime=100, background=1000, index=1000 (current).
     b) Lower realtime to 64; raise background to 4096 for heavy indexing.
   - Recommended: (a) retain; instrument lag metrics first.
   - Blocker: none.
    - USER: Confirm. Expose in config for power-users, use sane default (a), change later on evidence from benchmarks (early optimization is the source of all evil, etc - DK)

5) Observability log retention
   - Context: logs/logs/ploke.log rotation schedule and retention.
   - Options:
     a) daily rotation, keep 7 days.
     b) daily rotation, user-configurable retention (M1).
   - Recommended: (a) in M0; add config in M1.
   - Blocker: none.
    - USER: Agreed

How to use
- Add new items at the bottom; include the four required parts (context, options, recommendation, blocker).
- Reference the related implementation-log-XXX.md entries when a decision is finalized.

New items added 2025-08-19 (accelerated M0)
6) ObservabilityStore integration (BLOCKER)
   - Context: ploke-db needs to expose the ObservabilityStore contract (conversation_turn, tool_call with Validity and Json) to unlock DB-first persistence in TUI.
   - Options:
     a) Land minimal ObservabilityStore with required methods only; expand later.
     b) Gate behind feature flag; fall back to no-op when disabled.
   - Recommended: (a) minimal API first to unblock TUI integration.
   - Blocker: Yes — M0 requires this to persist chat history and tool-call lifecycle.
      - USER: Added `crates/ploke-db/src/observability.rs` as read-only to conversation, unblocked.
      - Further development coming in `ploke-db` to add tests, etc, but consider API stable

7) Chat history DB persistence trigger (NEEDS DECISION)
   - Context: TUI currently appends SysInfo/user/assistant in memory; FileManager export is optional. We need a consistent trigger for DB writes.
   - Options:
     a) Persist on every StateCommand::AddMessageImmediate/UpdateMessage.
     b) Batch per N updates or on idle (debounce).
   - Recommended: (a) for M0 simplicity; revisit batching in M1 with metrics.
   - Blocker: Minor — requires aligning AppState/state_manager to call into ploke-db when adding/updating messages.
    - USER: Agree on (a), added a TODO regarding adding a more ergonomic callback method on db to create a callback handler for the database to verify adding/updating messages in future db updates.

8) Tool-call payload persistence default (CLARIFY)
   - Context: Decision updated to store full arguments_json/outcome_json during fast iteration for debugging.
   - Options:
     a) Store full payloads now; add redaction toggles later.
     b) Keep only hashes in M0.
   - Recommended: (a) — store everything in M0; reintroduce redaction defaults pre prod-ready.
   - Blocker: None — but schema and API should accept both for forward-compat.
      - USER: Migration will occur, all current database items are prototype-only prior to prod-ready ploke-tui

9) EventBus readiness for tests (NICE-TO-HAVE)
   - Context: Broadcast channels only deliver to subscribed receivers; current tests use sleeps.
   - Options:
     a) Add a small Ready/Started event from run_event_bus after subscribing.
     b) Keep sleeps in tests, document caveat (current approach).
   - Recommended: (a) in a future PR; (b) acceptable for M0 timeframe.
   - Blocker: No.
      - USER: Follow recommendation for (b)

10) Channel capacities configurability (FOLLOW-UP)
   - Context: Defaults confirmed; power-user config requested.
   - Options:
     a) Add to user config with sane defaults; validate on load.
     b) Keep as compile-time defaults until benchmarks suggest changes.
   - Recommended: (a) in M1; keep (b) for M0.
   - Blocker: No.
      - USER: Agreed with recommendation

11) Type-safety across ploke-db ObservabilityStore (FOLLOW-UP)
   - Context: TUI now uses typed params and serde_json::Value for tool-call lifecycle. The current ploke-db API accepts Option<String> for arguments_json/outcome_json, requiring string round-trips.
   - Options:
     a) Extend ObservabilityStore to accept serde_json::Value (or a Json newtype) and convert internally to Cozo Json; keep String for backward compatibility during transition.
     b) Keep String-only for M0 and revisit in M1.
   - Recommended: (a) Adopt typed JSON inputs to reduce error surface and improve compile-time guarantees; provide From<Value> and TryFrom<String> conversions where helpful. Consider newtypes for Validity timestamps and ToolStatus.
   - Blocker: Not for M0 (strings work), but desirable for M1 to improve type safety and reduce incidental bugs.

Reference: See implementation-log-007.md for related changes and accelerated-pace requirement.

New items added 2025-08-20 (M1 start)
12) Preview mode defaults and truncation (NEEDS DECISION)
   - Context: Diff previews can be large. We plan code-block previews by default with optional unified diff.
   - Options:
     a) Default "codeblock" (before/after) and cap to N lines per file (e.g., 300), with a "...truncated" footer.
     b) Default "diff" (unified) and cap total preview size (e.g., 2000 lines), with per-file folding.
   - Recommended: (a) for readability and minimal deps; add unified diff as opt-in later.
   - Blocker: Minor — affects preview implementation and UI rendering.

13) Auto-confirm edits threshold (QUESTION)
   - Context: Tool payload can include a "confidence" value (0–1). We may allow auto-confirming above a threshold.
   - Options:
     a) Keep manual approval only in M1; ignore confidence.
     b) Add config editing.auto_confirm_edits and editing.min_confidence (default disabled).
   - Recommended: (b) as opt-in; default to manual approval.
   - Blocker: None — requires adding config fields.

14) Tool-call outcome on denial (CONFIRM)
   - Context: On user denial, we currently emit ToolCallFailed with error_kind "denied by user".
   - Options:
     a) Keep as "failed" for observability simplicity.
     b) Introduce a new "denied" terminal status in DB and eventing.
   - Recommended: (a) for M1; revisit status semantics when ploke-db adds code_edit_proposal.
   - Blocker: None — consistency decision for dashboards.

15) Proposal retention policy (QUESTION)
   - Context: After applied/denied, proposals remain in memory with terminal status.
   - Options:
     a) Keep them until session end; allow "edit clear <request_id>" later.
     b) Evict after N minutes or when count exceeds a cap.
   - Recommended: (a) for M1 simplicity.
   - Blocker: None — minor memory considerations.

16) Path normalization for previews (FOLLOW-UP)
   - Context: Absolute paths can be noisy; we plan workspace-relative paths for display.
   - Options:
     a) Normalize paths against current crate focus or configured workspace root in TUI.
     b) Keep absolute paths; rely on later normalization in DB layer.
   - Recommended: (a) for UX; not blocking.
   - Blocker: None.

New items added 2025-08-20 (continued)
17) Unified diff preview dependency (QUESTION)
   - Context: Code-block previews are implemented; unified diff is optional for power users and reviewers.
   - Options:
     a) Add "similar" crate to generate unified diffs behind a config flag.
     b) Keep code-blocks only in M1; revisit unified diff in M2.
   - Recommended: (a) optional, default off; implement when time permits.
   - Blocker: None.

18) DRY AppState construction (FOLLOW-UP)
   - Context: Multiple callsites construct AppState directly, which can drift when fields change (e.g., proposals).
   - Options:
     a) Introduce an AppState::builder() or AppState::new_full(...) to centralize required fields.
     b) Keep ad-hoc construction; update callsites as fields evolve.
   - Recommended: (a) in a follow-up PR to reduce technical debt.
   - Blocker: None.

19) Observability typed JSON inputs (FOLLOW-UP)
   - Context: ObservabilityStore currently accepts Option<String> for JSON payloads.
   - Options:
     a) Extend APIs to accept serde_json::Value/newtypes with internal conversion to Cozo Json.
     b) Keep String for M1; migrate later.
   - Recommended: (a) for type-safety and fewer round-trips.
   - Blocker: None (strings still work).
