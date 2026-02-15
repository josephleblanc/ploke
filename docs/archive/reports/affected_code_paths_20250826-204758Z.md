Affected Code Paths — Implementation Guide (2025-08-26 20:47:58Z)

Overview
This guide enumerates all code paths to be modified or added per the plan, with control flows, key functions, events, observability, file locations, crates involved, pitfalls, and mitigations. Use it as a map for staged implementation.

Legend
- Files: repo‑relative paths
- Events: `AppEvent::...`, `SystemEvent::...`, `ToolEvent::...`
- Crates: ploke‑tui, ploke‑io, ploke‑db, ploke‑rag, ploke‑ty‑mcp, ploke‑core

1) SeaHash Migration (file‑level hash)
- Current flow
  - Hash used for file verification: `TrackingHash` (UUIDv5 over file bytes within `PROJECT_NAMESPACE_UUID`).
  - Where used:
    - Preview reads: `ploke-tui/src/rag/tools.rs` → `IoManagerHandle::read_full_verified(path, expected_hash, namespace)`.
    - Apply edits: `ploke-tui/src/rag/editing.rs::approve_edits` → `IoManagerHandle::write_snippets_batch(edits)`; each edit carries `expected_file_hash`.
    - get_file_metadata tool computes `tracking_hash` via UUIDv5 in same file.

- Proposed changes
  - ploke‑io (compute & return SeaHash)
    - Files: `crates/ploke-io/src/read.rs`, `write.rs`, `actor.rs`, `handle.rs`.
    - Add SeaHash computation helper (bytes + path component). On reads: return or verify SeaHash. On writes: compute and return new SeaHash in `WriteResult`.
    - Keep dual‑mode for a transition period: accept both TrackingHash and SeaHash; prefer SeaHash for equality checks.
  - Tools in ploke‑tui
    - `rag/tools.rs::get_file_metadata_tool`: return `file_hash` as SeaHash string (and optionally tracking_hash during migration).
    - `rag/tools.rs::apply_code_edit_tool`: validate `expected_file_hash` using SeaHash; in canonical mode, fetch current file SeaHash via verified read before preview; in direct splice mode, require SeaHash.
    - `rag/utils.rs` preview read path: switch verification to SeaHash (path‑aware), fail fast on mismatch and emit rebase guidance.
  - ploke‑db schema (file entity)
    - Add/replace file hash column to store SeaHash; keep module schema as‑is. Provide migration that populates SeaHash for known files from disk.

- Observability
  - Log both previous tracking hash and new SeaHash during the migration for auditability (info span on proposal/build and apply).
  - Add apply_result persistence (see section 6) with old/new SeaHash values.

- Pitfalls & mitigations
  - Pitfall: Platform‑dependent hashing or path normalization errors.
    - Mitigation: Normalize paths identically in hash helper; add tests for separators/case on supported platforms.
  - Pitfall: Mixed batches where some edits carry old TrackingHash and some SeaHash.
    - Mitigation: Accept both for a transition window; warn via tracing when using legacy hashes.

2) Critical Edit UX (Approvals, Open‑in‑Editor, Revert)
- Current control flow
  - Staging: `rag/tools.rs::apply_code_edit_tool` → builds `EditProposal` with preview into `state.proposals` and emits SysInfo summary.
  - Approval: `rag/editing.rs::approve_edits` applies via ploke‑io and emits ToolCallCompleted/Failed; `StateCommand::ApproveEdits` exists.

- New UI overlay
  - Files: `ploke-tui/src/app/view/components/approvals.rs` (new), `ploke-tui/src/app/mod.rs` (wire overlay + hotkeys), `ploke-tui/src/app/input/keymap.rs` (actions), `ploke-tui/src/app/events.rs` (if needed to structure UI events).
  - Actions: Approve, Deny, Apply on branch (Git), Revert (Git or backup), Rebase (refresh spans when SeaHash changed), Open‑in‑Editor.
  - State: Read from `state.proposals`; display per‑file unified diff or codeblock preview (already produced by staging tool).

- Open‑in‑Editor
  - Invoke user’s editor with file path and line (approximate via span context if available). Provide env‑configurable editor command; fallback to printing file path instructions.
  - Pitfall: Non‑portable editor commands; sandbox constraints.
    - Mitigation: Configurable command; no hard dependency. Log a friendly SysInfo if command fails.

- Git branch apply/revert (preferred)
  - Crate: ploke‑ty‑mcp (typed MCP Git client).
  - Prefer native Rust git crates over MCP for repo operations. Candidate crates: `git2` (libgit2 bindings) or pure-Rust `gix` (gitoxide). Choose one and wrap minimal operations we need: ensure repo/init, create branch, stage paths, commit, revert/checkout.
  - Files: add a new `ploke-git` helper module/crate to encapsulate operations; wire from `ploke-tui` approvals overlay actions.
  - Flow: Create branch → apply edits → stage+commit → allow revert (checkout prev) or a new revert commit; show diff via `git diff` rendered into overlay.
  - Pitfall: Non-git workspaces or detached HEAD.
    - Mitigation: Detect missing .git; offer backup/recover path; display clear UX messages. For detached HEAD, require user confirmation to create a new branch.

- Observability/UI/DB
  - UI: SysInfo messages for each stage; overlay badges for lint/format/test (see 3) and branch/apply.
  - DB: Persist proposal and apply_result rows (see 6), including per‑file new SeaHash and status.

3) Lint/Format/Test Gates (pre‑approval)
- Flow (new)
  - Trigger on staged proposal or pre‑apply in overlay. Language: Rust initially.
  - Invoke `cargo fmt --check`, `cargo clippy`, `cargo test` (configurable); capture summaries; present in overlay.
  - Pitfall: Long‑running tasks blocking UI; noise.
    - Mitigation: Spawn in background; stream condensed summaries (SysInfo); full logs accessible via “open logs” action.

4) Post‑Apply Re‑Scan & Re‑Index
- Current
  - `app_state/handlers/database.rs::scan_for_change` rebuilds a partial graph and retracts embedded nodes for changed files; indexing commands exist (create/replace HNSW; BM25 actor handles sparse).

- Change
  - After `approve_edits` success, send a state command to re‑scan affected files (or their crate), then enqueue embedding/index update.
  - Files changed: `ploke-tui/src/rag/editing.rs` (post‑apply trigger), `ploke-tui/src/app_state/dispatcher.rs` (wire a new command if needed), `ploke-tui/src/app_state/handlers/db.rs` (re‑use existing scan flow).
  - Observability: SysInfo summary of rescan (files changed, time), and errors; persist a compact scan result.

5) Provider Allowlist & Per‑Request Outcome Summary
- Current
  - Model Browser overlay fetches providers; provider pin is supported. LLM session handles some 404/429 and logs artifacts.

- Change
  - Config: add allowlist to `user_config::ModelRegistry` and surface it in the overlay; prefer/force endpoints known to support tools.
  - Session: `llm/session.rs` to compute and emit a concise per‑request outcome summary (success/no_tool_calls/404/429) as a SysInfo message; include basic counts by run.
  - Observability: Persist summary per request in the session trace index (see 7) for triage.
  - Note: Thoroughly test all items that touch OpenRouter endpoints; the API is external and provider behavior varies. Align request/response types with official docs in `crates/ploke-tui/docs/openrouter/`.

6) Observability Persistence Extensions (DB)
- Current
  - `ploke-db/src/observability.rs`: upsert_conversation_turn, record_tool_call_requested/done, get_tool_call (with idempotency, latency computation).

- Add
  - retrieval_event (request_code_context): store query, strategy, top_k, budget, items (path, score, span) keyed by parent_id.
  - proposal + proposal_file: store request_id, parent_id, preview meta, file list.
  - apply_result: request_id, path, old/new SeaHash, status, error.
  - turn usage/cost: prompt/completion/total tokens and USD cost (from registry pricing) keyed to conversation turn.

- Wiring in ploke‑tui
  - `observability.rs`: on ToolEvent::Requested/Completed/Failed, continue to persist; on request_code_context, also persist retrieval_event; on staging, persist proposal; on apply success/failure, persist apply_result.
  - LLM usage/cost: compute from OpenRouter usage and registry pricing; attach to `MessageUpdate.metadata` (chat_history merges into totals) and persist via observability.
  - Strong typing: ensure all OpenRouter‑touching code uses strongly typed structs (f64 for costs, u32 for tokens, etc.) with Serialize/Deserialize (derive or manual), and consider GhostData for phased states if helpful.

7) Session Trace Overlay (new)
- Files: `ploke-tui/src/app/view/components/trace.rs` (new), `app/mod.rs` (wire overlay + hotkeys).
- Content: per‑request timeline (request plan, tool calls observed, tool results, final response, rescan summary, costs). Index files under target/test-output/openrouter_e2e.
- Pitfall: Too verbose; log churn.
  - Mitigation: Compact index with links to detailed artifacts; roll logs daily (already configured).

8) RAG Retrieval Curation UI (new)
- Current
  - RAG context is injected as System messages; no dedicated viewer/curation.

- New overlay
  - Files: `ploke-tui/src/app/view/components/context_items.rs` (new), `app/mod.rs` for keybindings.
  - Actions: View items, pin/blacklist items/files, adjust token budgets, re‑run request_code_context with preferences. Persist retrieval_event.
  - Display tiers in chat history: off/relative paths only/paths plus first X chars (configurable, default 120). Dedicated overlay supports toggling granularity; configurable window mode (separate overlay/window/buffer vs floating window).
  - Observability: Log decisions; persist curated retrieval_event.

9) Agent Orchestrator (Planner/Editor/Critic)
- Files: `ploke-tui/src/agent/{mod.rs,planner.rs,editor.rs,critic.rs,policy.rs}` (new) wired from commands.
- Wiring: new commands “/agent on/off/plan/apply”; LLM events feed plan state; uses tools (request_code_context/get_file_metadata/apply_code_edit) via existing dispatcher.
- Observability: Persist agent decisions (step, rationale), linked to parent_id.

Crate Summaries (used functions & data structures)
- ploke‑tui
  - LLM
    - `llm/session.rs`: RequestSession::run (per‑request loop; tool calls; retries; fallback); awaits tool results via await_tool_result.
    - `llm/mod.rs`: OpenAiRequest/ToolDefinition/RequestMessage; build_openai_request; Usage types present (TokenUsage, LLMMetadata) but not fully wired.
    - `llm/tool_call.rs`: execute_tool_calls/dispatch_and_wait.
  - RAG
    - `rag/tools.rs`: request_code_context/get_file_metadata/apply_code_edit (staging, preview); `utils.rs` (calc_top_k, Action, ToolCallParams, ALLOWED_RELATIONS).
    - `rag/editing.rs`: approve_edits/deny_edits (apply via io_handle; emit System + LlmTool events).
    - `rag/context.rs`: process_with_rag (assemble context); PROMPT header/code; construct PromptConstructed.
  - AppState/Dispatcher/UI
    - `app_state/dispatcher.rs`: routes StateCommand to handlers, including Approve/DenyEdits, Rag search/ctx.
    - `app/mod.rs`: hotkeys; submit flow; render; model browser.
  - Observability
    - `observability.rs`: persists conversation turns and tool lifecycle; extend here for retrieval/proposal/apply/usage.

- ploke‑io
  - `handle.rs`: IoManagerHandle APIs (read/write/scan); `actor.rs` routes requests; `read.rs`/`write.rs` implement operations; add SeaHash support.

- ploke‑db
  - `src/observability.rs`: upsert_conversation_turn, record_tool_call_requested/done, get_tool_call; extend with new tables/methods.

- ploke‑rag
  - `core/mod.rs`: RagService: search (dense/sparse/hybrid), bm25 service; TokenBudget; ensure we can persist retrieval events.

- ploke‑ty‑mcp
  - `clients/git.rs`: status/diff/add/commit/branch for Git workflows; `manager.rs`: ensure_started/cancel.

Implementation Pitfalls & Solutions (cross‑cutting)
- Hash migration confusion (TrackingHash vs SeaHash): accept both for a time, prefer SeaHash, log deprecation, add tests.
- UI responsiveness: spawn heavy tasks (lint/test/scan) in background; stream concise SysInfo; overlays should poll state.
- Provider variability: maintain allowlists; present per‑request summary to manage expectations.
- Schema changes: add DB migrations with idempotency; keep JSON payloads for flexibility but index on key columns.
- Other Plan Enhancements (additions)
1) OpenRouter strict typing and conversions: audit all request/response types; ensure numeric fields (cost/tokens) use numeric types; remove ad‑hoc String parsing. Keep `crates/ploke-tui/docs/openrouter/` as the spec reference.
2) cfg‑gated SeaHash migration: add a temporary cargo cfg to enable SeaHash path; run both code paths during migration to compare; remove cfg and TrackingHash code after migration completes; regenerate fixture DB backups accordingly.
3) Approvals overlay scroll trait: define a ScrollableOverlay trait used by Model Browser and new overlays; snapshot tests should include scrolling behavior (fixed Rect, known viewport, assert items at offsets).
4) Provider allowlist persistence: store allowlist and pinned endpoints in user_config; display model price as USD per 1M tokens in UI; compute from pricing fields in registry.
5) Lint/format/test config: add a dedicated TOML for extra rustc/clippy flags; default to None and expose via UI/commands.
6) Post‑apply full rescan: rescan entire crate for now (integrity); note future incremental parse/update plan.
7) Ratatui UI test harness: add snapshot helpers (render to Buffer, assert lines); avoid terminal‑size dependency by fixing Rect.
8) Cost/caps: soft budget in UI; warn if cumulative cost exceeds threshold; persist per‑turn usage.
9) Session Trace index: create a compact JSON index per request under target/test-output/openrouter_e2e and link artifacts.
10) Error taxonomies: standardize error enums across layers; surface concise user messages and detailed logs.
11) File watcher: add single‑process watcher to trigger rescans on file changes (Phase 3), feature‑gated in CI.
12) Git conflicts: if workspace has uncommitted changes, show status and confirm before branch apply; detect merges and offer guidance.
13) Safe paths: reuse ploke‑io roots and symlink policy in higher layers to validate tool inputs.
14) CLI flags: add toggles to run only overlays/tests/benches; integrate with testing guidelines envs.
15) Benchmark pipeline: establish scripts to run criterion suites and publish into target/benchmarks/YYYYMMDD with summary report.
