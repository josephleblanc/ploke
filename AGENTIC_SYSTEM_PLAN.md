Ploke Agentic System: Evaluation, Alternatives, and Roadmap

Summary
- Primary goal: Help developers be better at what they do.
- Secondary goal: Build an agentic, multi‑agent framework that can edit the codebase safely and effectively in service of the primary goal.
- Outcome: This document surveys the current TUI + tool‑calling workflow, identifies strengths and gaps, defines success criteria, explores alternatives, and proposes a phased plan to reach a robust agentic system. It also lists concrete files/components to extend or modify.

Current Workflow Survey
- Crates
  - `crates/ploke-tui`: Terminal UI, event bus, LLM integration, tools dispatch, RAG orchestration glue.
  - `crates/ploke-rag`: RAG service (BM25 actor + dense + hybrid assembly, token budgeting).
  - `crates/ploke-db`: Database API (HNSW index, BM25 avgdl persistence, Cozo queries, typed helpers).
  - `crates/ploke-io`: IO manager (atomic writes, hash‑verified reads), used by edit tools.
  - `crates/ploke-core`: Shared types (rag_types, WriteSnippetData, TrackingHash, namespace UUIDs).
  - `crates/common`, `crates/ingest`, `crates/ploke-error`, `crates/ploke-ty-mcp`: supportive domains.

- Startup and runtime wiring
  - `ploke-tui/src/lib.rs::try_main`
    - Loads `UserConfig` and merges default `ModelRegistry` (OpenRouter first‑class).
    - Refreshes OpenRouter capabilities; resolves API keys.
    - Initializes DB, BM25 service (`ploke_db::bm25_service`), `RagService::new_full(...)`, `EventBus`, `FileManager`.
    - Spawns subsystems: state manager, LLM manager, event bus, observability, and TUI `App`.

- Terminal UI loop and message submission
  - `ploke-tui/src/app/mod.rs` (Action::Submit path ~ line 740+)
    - Sends in order:
      - `StateCommand::AddUserMessage { content, new_msg_id, completion_tx }`
      - `StateCommand::ScanForChange { scan_tx }` (detect code changes; partial reparse; DB update)
      - `StateCommand::EmbedMessage { new_msg_id, completion_rx, scan_rx }` (RAG pipeline)
      - Adds a SysInfo “Embedding User Message” child under the new user node.
    - UI scroll follows and input clears.

- RAG orchestration and prompt construction
  - `ploke-tui/src/rag/context.rs::process_with_rag`
    - Waits for user message write completion; then calls `RagService::get_context` with `RetrievalStrategy::Hybrid { rrf, mmr }` and `TokenBudget`.
    - Builds prompt: `PROMPT_HEADER` + tool instructions (`PROMPT_CODE`) + assembled code snippets (System) + conversation (User/Assistant).
    - Emits `AppEvent::Llm(Event::PromptConstructed { parent_id, prompt })`.
  - `crates/ploke-rag/src/core/mod.rs` (RagService)
    - Dense search via HNSW; Sparse search via BM25 actor with status/timeout/backoff and strict/lenient modes; hybrid via RRF with optional MMR; supports bm25 rebuild/save/load.

- LLM request, tool wiring, and two‑leg loop
  - `ploke-tui/src/llm/mod.rs::llm_manager`
    - On `AppEvent::Llm(Event::Request { parent_id, new_msg_id, ... })` stores pending request.
    - On `PromptConstructed { parent_id, prompt }` pairs and spawns `process_llm_request`.
  - `ploke-tui/src/llm/mod.rs::process_llm_request`
    - Fetches active `ModelConfig` from `user_config::ModelRegistry` with enforcement knob `require_tool_support`.
    - Includes tools if model is (cached) tool‑capable: `request_code_context`, `get_file_metadata`, `apply_code_edit`.
    - Delegates to `session::RequestSession::run` which:
      - Builds OpenRouter `chat/completions` request (`tools` + `tool_choice: "auto"`, provider.order when pinned).
      - Handles 404 “no tool support” fallback once without tools (policy controlled), else errors with actionable guidance.
      - Parses provider `tool_calls` (OpenAI schema), dispatches each via `EventBus` as `AppEvent::LlmTool(ToolEvent::Requested {...})`.
      - Awaits matching `ToolEvent::Completed/Failed` with `(request_id, call_id)` correlation; appends tool results as `tool` role messages; retries limited by `tool_max_retries`.
      - On completion without tool_calls, returns final assistant text. Errors are surfaced into chat via `StateCommand::UpdateMessage`.

- Tool dispatch and handlers
  - `ploke-tui/src/rag/dispatcher.rs::handle_tool_call_requested`
    - Routes tool name → handler:
      - `request_code_context` → `rag/tools.rs::handle_request_context`
      - `get_file_metadata` → `rag/tools.rs::get_file_metadata_tool`
      - `apply_code_edit` → `rag/tools.rs::apply_code_edit_tool`
  - `rag/tools.rs::handle_request_context`
    - Takes `{ token_budget, hint? }`; resolves query from hint or last user message; computes `top_k` for budget; calls `RagService::get_context`; returns `RequestCodeContextResult` JSON.
  - `rag/tools.rs::get_file_metadata_tool`
    - Reads file bytes; computes tracking hash as UUIDv5 over bytes in `PROJECT_NAMESPACE_UUID`; returns size and mtime.
  - `rag/tools.rs::apply_code_edit_tool`
    - New LLM‑friendly schema (no offsets/hashes required by model). Two paths:
      - Canonical node rewrite: `{ file, canon, node_type, code }` → resolve span via DB helpers; construct `WriteSnippetData` with expected file hash.
      - Direct splice: `{ file_path, expected_file_hash, start_byte, end_byte, replacement }`.
    - Builds per‑file preview (diff or codeblock) and stores `EditProposal { request_id, parent_id, call_id, edits, files, preview }` in `state.proposals`.
    - Emits SysInfo summary; auto‑approves if `editing.auto_confirm_edits` is true.
  - `rag/editing.rs::{approve_edits, deny_edits}`
    - Approve applies edits via `ploke_io::IoManagerHandle::write_snippets_batch` (tempfile + fsync + rename; hash‑verified) and returns a JSON result; marks proposal Applied/Denied/Failed and emits bridging System + LlmTool events.

- Model selection UX
  - `app/view/components/model_browser.rs` and `app/mod.rs`
    - Overlay lists models with tools support, pricing, context; on expand, can fetch providers for a model (`ModelEndpointsRequest/Results`). Selecting a provider pins `provider_slug` and switches the active model.
  - `user_config.rs` / `llm/registry.rs` / `llm/openrouter_catalog.rs`
    - Registry tracks providers, aliases, strictness, capabilities cache (`supports_tools`), and allows pinning. API key resolution per provider type. Refresh pulls OpenRouter catalog and per‑provider capabilities.

Key Events and Actors
- Events: `AppEvent::{Ui, Llm(Event::{Request, PromptConstructed, ToolCall, …}), LlmTool(ToolEvent::{Requested,Completed,Failed}), System(SystemEvent::{ToolCallRequested,Completed,Failed,ModelSwitched,…}), Rag(RagEvent)}`
- Actors: TUI App, State Manager, LLM Manager, RAG Service (with BM25 actor), File Manager/IoManager, Event Bus, Observability, Database.

Assessment vs Goals
- Safety and correctness
  - Strong: Edit staging, preview, and atomic application via IoManager; direct mode requires `expected_file_hash`; canonical mode derives and validates hash. Good guardrails if code changes between index and apply (hash mismatch prevents corrupting).
  - Good: BM25 strict vs lenient fallback; dense fallback when sparse empty; timeouts/backoff; token budgeting.

- Tool‑calling viability
  - Mixed across OpenRouter providers. request_code_context and get_file_metadata succeed reliably; apply_code_edit frequently declined by providers due to policy. The system gracefully soft‑fails with logs and guidance.

- RAG and embeddings
  - Dense and sparse implemented with hybrid assembly; partial reparse/index update triggered on submit (`ScanForChange`). This reduces staleness between embedding and LLM call. Additional watchers/CDC would help for long sessions.

- Agentic editing capability
  - Present: LLM can stage edits via tools; auto‑approval toggle exists. However, no higher‑level agent loop (planning, multi‑tool strategies, self‑evaluation) yet. Edits are single‑shot proposals without test/build gates.

- Model/provider ergonomics
  - Good baseline: Model Browser overlay, endpoints fetch, pinning, supports_tools cache, enforcement knob. Still room to streamline selection and persistence.

What happens if the codebase changes between embedding and send?
- On submit, the UI triggers `ScanForChange` followed by `EmbedMessage` (RAG). Partial reparsing and DB updates precede RAG assembly, substantially reducing drift. During editing, `get_file_metadata` and `apply_code_edit` validate with tracking hashes and fail safely if content has changed. BM25 has rebuild/save/load; dense index relies on DB index management commands.

Gaps to achieve the secondary goal
- Orchestration: No planner/executor loop spanning multiple tool cycles; no multi‑agent roles (planner, code editor, critic) orchestrated explicitly.
- Approvals: Proposals exist but lack a durable queue, granular policies, or a first‑class UI to inspect/approve/deny per‑file or per‑hunk (beyond text preview).
- CI/checks: No automatic test/build/format validation loop; no rollbacks/branching strategy.
- Observability: Good tracing and ad‑hoc diag artifacts; needs consolidated session trace view, counters, error taxonomy, and per‑tool metrics surfaced in the TUI.
- Provider robustness: Apply‑edit tool is often ignored by providers; needs allowlists, adaptive strategies, and clearer user feedback.
- Live index drift: No file watcher/CDC to keep indexes fresh during long sessions (beyond the on‑submit scan).

Preconditions for a robust agentic system
- Content‑lock editing: enforced tracking hashes on all write paths; stored proposals with provenance and status; atomic persistence.
- Tool schema stability: minimal, typed arguments with clear invariants and self‑description in prompts.
- Reliable retrieval: hybrid search ready and resilient; ability to reconstruct/bypass sparse when needed; deterministic token budgeting.
- Provider selection: known tools‑capable endpoints; user‑pinnable provider; enforcement knob visible.
- Observability: per‑session structured trace of request → tool‑calls → results → decisions; surfaced in TUI for debugging.
- Approval and policy: configurable gates for test/build/format success; explicit user control for auto‑apply thresholds.

Success Criteria (4–7)
1) Safety: Never corrupts workspace; edit attempts respect tracking hashes; clear approval workflows.
2) Provider Compatibility: Tool‑calling succeeds on an allowlisted set of providers; graceful fallbacks; actionable errors.
3) Developer Ergonomics: Few keystrokes to pick model/provider; previews are readable; actions are discoverable.
4) Observability: Session traces and metrics enable quick diagnosis; logs are structured and persisted.
5) Maintainability: Modular crates and typed APIs; minimal cross‑cutting concerns; tests for tool IO and dispatch.
6) Extensibility: New tools/agents added without touching core loops; configurable policies.

Alternative Approaches
1) Single‑agent TUI loop (baseline)
   - Pros: Simple, minimal moving parts; already working.
   - Cons: Limited planning/self‑critique; provider variability impacts edit tool consistency; weak CI gates.

2) External agent service (daemon) orchestrating tools
   - Pros: Decouples UI; can run longer workflows; richer state/metrics; easier multi‑agent.
   - Cons: More infra; IPC; versioning; adds latency/complexity.

3) Git‑patch proposal workflow
   - Pros: Leverages VCS; standard review/rollback; patch serialization portable.
   - Cons: Requires bridging spans → hunks robustly; may be heavier than IoManager atomic splices.

4) MCP integration (Model Context Protocol) as tool substrate
   - Pros: Standardized tool registry; broader model support; easier ecosystem interoperability.
   - Cons: Additional layer; mapping Ploke primitives to MCP; provider support varies.

5) Local function‑calling models first, remote fallback
   - Pros: Deterministic tool behavior; privacy; lower latency.
   - Cons: Hardware constraints; model quality; more local management.

Merged Strategy
- Keep the baseline single‑agent TUI loop but add a light “Agent Orchestrator” inside `ploke-tui` (or a new crate `ploke-agents`) that:
  - Plans tool steps (context → inspect → edit proposal → validate → apply) with retries and policy gates.
  - Encapsulates multi‑agent roles as strategies (Planner/Editor/Critic) without heavy IPC.
  - Exposes a durable proposal queue and an approvals UI.
  - Adds provider strategies (allowlist + fallback) and better errors.
  - Integrates simple CI hooks (format/build/test) pre‑approval.

Phased Roadmap
Phase 1: Provider robustness and critical edit UX
- Harden tool routing and provider selection
  - Add an allowlist for known tools‑capable OpenRouter endpoints; surface this in the Model Browser and config.
  - Expose `require_tool_support` and provider pin in the status line and model overlay.
  - Summarize per‑request outcomes in the UI (success, no_tool_calls, 404, 429) with counts.
 - Critical first‑class edit UX (blocking)
  - Approvals overlay: list staged edits with unified diffs/codeblocks; Approve/Deny/Apply on branch/Revert/Rebase hotkeys.
  - Open‑in‑editor: provide a one‑keystroke way to open the file and location in the user’s editor before applying.
  - Revert: integrate with Git (via `ploke-ty-mcp` Git client) or local backups if Git absent.
  - Persist proposals across runs (serialize to `~/.config/ploke/proposals.json`).

Phase 2: Agent Orchestrator (single process)
- Add an internal orchestrator module (`ploke-tui/src/agent/`) with:
  - Strategies: Planner (decide next tool), Editor (edit proposal), Critic (request more context or adjust).
  - Policy gates: format/lint/build/test actions before approval; surface results in chat and approvals overlay.
  - Session memory: lightweight store keyed by conversation id (what tools/results already tried).
  - Commands: “/agent on/off”, “/agent plan”, “/agent apply”.

Phase 3: Live index freshness and CI hooks
- File watcher/CDC to trigger partial reparse and dense index updates over session lifetime.
- Add optional git branch workflows: proposals can be applied on a branch; show git diff; allow revert.
- Add simple runner integration for tests and formatters; pluggable per language (focus Rust first).

Phase 4: Optional MCP and service extraction
- Offer MCP adapter for tools so models that prefer MCP can interop.
- Optionally extract an `ploke-agents` service for longer multi‑step tasks shared by UI and other frontends.

Observability Plan
- Instrument spans and counters:
  - LLM request lifecycle: request → tool_calls observed → tool dispatch → tool results → completion.
  - RAG operations: dense/sparse search timings, fusion, token budgeting.
  - Edit pipeline: proposal creation, approvals, application results per file.
- Persist per‑session artifacts under `target/test-output/openrouter_e2e/` (already used) but add a compact index per session.
- TUI overlay: “Session Trace” view summarizing key steps and statuses with jump‑to logs.

Staging, Approvals, and Safety
- Default: Stage edits, require user approval; allow opt‑in `auto_confirm_edits` per policy (e.g., confidence >= threshold, tests passed).
- Always validate tracking hashes on writes; if mismatch, present “rebase” guidance (re‑request context and re‑resolve canon spans).
- Keep all destructive operations gated behind explicit commands or confirmations.

Files to Change or Add (short list)
- `crates/ploke-tui/src/app/view/components/`:
  - Add `approvals.rs` overlay to browse/approve staged proposals.
  - Add `trace.rs` for session trace overlay.
- `crates/ploke-tui/src/app/mod.rs`:
  - Wire new overlays and actions (e.g., hotkeys to open approvals/trace, agent commands).
- `crates/ploke-tui/src/llm/session.rs`:
  - Summarize tool outcomes per request; expose to UI via new events.
  - Provider allowlist + sticky fallback strategies.
- `crates/ploke-tui/src/llm/mod.rs`:
  - Centralize tool defs and typed schema comments; ensure stable names.
- `crates/ploke-tui/src/rag/tools.rs`:
  - Minor: enrich `RequestCodeContextResult` with scoring metadata for UI preview (optional).
- `crates/ploke-tui/src/rag/editing.rs` and `app_state/core.rs`:
  - Persist proposal registry; add status transitions and timestamps; serialize/deserialize.
- `crates/ploke-tui/src/user_config.rs`:
  - Config for approvals policy (auto thresholds), provider allowlist, and observability toggles.
- `crates/ploke-tui/src/agent/` (new):
  - `mod.rs`, `planner.rs`, `critic.rs`, `policy.rs`. Start simple: sequential strategies bound to current chat.
- SeaHash migration (file-level):
  - `crates/ploke-io`: compute/return SeaHash for reads/writes; update write result types.
  - `crates/ploke-tui/src/rag/tools.rs`: change get_file_metadata and apply_code_edit validation to SeaHash.
  - `crates/ploke-db`: add/replace file hash column with SeaHash; migration script; update helper queries that compare file hashes.
  - `crates/ploke-tui/src/rag/utils.rs`: verify/compare SeaHash in preview reads.

Notable Existing Strengths to Reuse (avoid re‑implementing)
- RAG service with hybrid fusion and bm25 actor, including strict/lenient modes and persistence.
- Tool definitions and dispatch bridge; typed handlers for context, metadata, and edits.
- Edit staging + atomic write pipeline with tracking hash verification.
- Model registry and browser overlay; tools‑capability cache and enforcement flag.
- Event bus and message update routing already in place; structured tracing and diagnostics.

What I wish existed earlier (retro for future work)
- A reusable “session trace” abstraction to collect all per‑request state for quick introspection.
- A persistent proposal store with schema versioning to enable resuming work seamlessly.
- A provider strategy registry with per‑provider quirks documented (timeouts, tool idiosyncrasies).
- Thin test harnesses for tool handlers (already partially outlined under docs/reports & tests, expand them).

Plan Evaluation Against Criteria
- Safety: Strong due to tracking hashes + staged approvals; improved with policy gates and CI checks.
- Provider Compatibility: Improved by allowlists, pinning, and clear UI cues; fallback maintained.
- Ergonomics: Approvals and trace overlays reduce guesswork; model selection already solid.
- Observability: Session trace + counters unify existing logs; keeps diag artifacts.
- Maintainability: Adds modules without breaking current flows; reuses existing event bus and state manager.
- Extensibility: Agent orchestrator is additive; MCP/service extraction left for later phase.

Decision Notes
- Keep edits staged by default; require user approval unless auto policy is explicitly enabled.
- Favor a light, in‑process orchestrator first; only extract to a service if warranted by complexity.
- Do not duplicate RAG or rewrite tool dispatch; extend with overlays, persistence, and policy.

Next Steps (minimal slice)
1) Add approvals overlay + commands; wire to existing proposal registry; persist proposals on disk.
2) Add per‑request outcome summary in the chat after a tool session (success/no tools/404/429).
3) Add provider allowlist configuration and surface it in the model browser; show a “tools‑capable” badge.
4) Add a minimal Agent Orchestrator that can “plan” request_code_context → (optional) get_file_metadata → apply_code_edit (staged) → prompt user to approve.

Clarifications And Answers
- Tracing and TUI UX:
  - Logging uses a rolling file appender at `logs/ploke.log` (`ploke-tui/src/tracing_setup.rs`). Console logging is disabled by default, so logs do not interfere with the ratatui UI. Diagnostics and per‑request artifacts are also written under `target/test-output/openrouter_e2e` by the LLM session.
  - Improvement: Add a “Session Trace” overlay to inspect recent events without leaving the TUI.

- Existing MCP Support:
  - Crate `crates/ploke-ty-mcp` provides a typed client on top of `rmcp`, a `McpManager`, config loader, and typed clients for Git and Context7 with E2E tests. Not yet integrated into `ploke-tui`; plan to leverage it for branch/diff/revert UX and optional tool substrate.

- Editing Feature and `ploke-io` Usage:
  - Reads for preview use `IoManagerHandle::read_full_verified(path, expected_hash, namespace)` ensuring the file matches the expected `TrackingHash` before previewing.
  - Applies use `IoManagerHandle::write_snippets_batch`, which performs temp‑write → fsync → atomic rename → best‑effort parent fsync, with per‑file async locks. This is the intended safe usage and is already correctly wired in `rag/editing.rs::approve_edits`.
  - Offset drift (newline at start): Canonical edits compute spans from the DB and rely on the file’s tracking hash; if the file changed, hash mismatch prevents apply. Direct splice path requires the latest hash. The model (or orchestrator) should call `get_file_metadata` or trigger a re‑scan to rebase.

File Hashing Strategy (Proposed Change)
- Objective: Replace file‑level TrackingHash usage with a SeaHash over the exact file bytes plus path for fast, deterministic change detection at the file level.
- Impacted components:
  - DB file schema: add/replace file hash column with SeaHash; keep file‑level module schema unchanged.
  - `ploke-io`: compute/return SeaHash on reads/writes; update `write_snippets_batch` result type to include the new hash; provide helper to compute from bytes+path.
  - Tools: `get_file_metadata` returns SeaHash; `apply_code_edit` validates against SeaHash for both canonical and direct splice paths.
  - Preview/verification: `read_full_verified` should verify SeaHash; on mismatch, fail early and guide rebase.
  - Migration: support dual‑hash period (TrackingHash + SeaHash) with logs; after population, tools and UI compare SeaHash exclusively.
- Safety: If SeaHash unchanged, the file bytes (and path) are unchanged; this increases confidence in pre‑apply validation and rebase logic.

Observability: Status And DB Audit
- A pair of audit reports has been created and referenced here for ongoing work:
  - docs/reports/observability_status_20250826-204758Z.md — survey of interaction paths and current observability signals, with gaps and proposals.
  - docs/reports/db_observability_audit_20250826-204758Z.md — survey of ploke‑db observability API (conversation turns, tool calls) and identified gaps (e.g., cost/usage, RAG retrieval events, edit approvals/change application records).
  - docs/reports/affected_code_paths_20250826-204758Z.md — implementation guide for all modified/added code paths, with control flows, files, events, observability, pitfalls, and crate APIs.
  - docs/reports/test_overview_20250826-204758Z.md — current test suite overview and ignores applied for sandbox stability.
  - docs/testing/TEST_GUIDELINES.md — testing strategy, gating, ratatui snapshot guidance, benchmarks, and phase stopping points.
  - docs/reports/production_readiness_notes_20250826-204758Z.md — suggestions to improve security, performance, and reliability.
  - docs/reports/agents_git_best_practices_20250826-204758Z.md — best practices for agent + Git integration and recommended approach.

- Editing Loop Details and UX:
  - Today: LLM proposes → tool stages proposal with diff/codeblock preview → user can approve/deny (auto‑approve available) → writes applied atomically.
  - No pre‑approval lint/test run yet. Plan adds lint/format/test gates and a dedicated approvals overlay with per‑file diff, status badges, “apply on branch” and “revert” actions (git via MCP or local backup).

- Database Recording and Embeddings:
  - Conversation turns and tool calls are persisted via `observability.rs` to `ploke_db` (conversation turn upserts; tool call requested/done records with timing and outcomes).
  - After edits are applied, there is no automatic DB re‑parse/re‑embed yet. Plan: trigger `ScanForChange` and kick the embedding/index update for affected files to keep HNSW/BM25 fresh.
  - Embeddings for chat messages are generated transiently for retrieval but not upserted as vector nodes; code graph embeddings are maintained by the DB indexers.

- Context Persistence and UX:
  - RAG context snippets are inserted as System messages (visible in the conversation and persisted as turns). There is no dedicated overlay to list/curate context items.
  - Plan: “Context Items” overlay to list retrieved files/snippets and allow whitelist/blacklist, with token budget and truncation indicators; feed preferences into the next `RagService::get_context` call.
  - Overflow: Managed by `LLMParameters.history_char_budget`, `max_tokens`, and the RAG `TokenBudget`. Also cap tool‑provided context via `tool_token_limit`.

- Price/Usage and Context Window Management:
  - Types exist for usage and cost (`LLMMetadata`, `TokenUsage`), and model pricing is cached in the registry, but current LLM flow does not attach usage/cost to messages.
  - Plan: Parse provider usage when present, compute cost using model/provider pricing, attach metadata in `StateCommand::UpdateMessage`, show per‑turn usage and a “Session Cost” indicator, with a soft budget and warnings.

- Conversation Persistence and Branching:
  - Chat history is a branching tree; users can navigate branches. Observability persists each turn with parent_id; this supports branch history without schema changes.
  - Future: A branch explorer overlay to visualize branches; multi‑response per user prompt is supported by creating multiple assistant children.

- Multi‑Agent Roles and Strategies:
  - Planner: scopes and orchestrates steps (request context with hints; decide when to fetch metadata; set budgets).
  - Editor: produces canonical `apply_code_edit` calls; asks for metadata to rebase when hashes mismatch; limits scope per proposal.
  - Critic: validates proposals via lint/format/test, requests adjustments or additional context if checks fail.
  - Workflow: Planner → request_code_context → Editor → apply_code_edit (staged) → Critic → gates/lints/tests; user approves in overlay; on apply, trigger re‑scan/re‑embed. Rebase flow handles hash mismatches.
