# Ploke Agentic System Roadmap (Production-Ready)

Purpose
- Build a production-ready agentic system inside Ploke that can autonomously implement user goals via code edits, while preserving user control, observability, and correctness.
- Deliver a flywheel: Ploke iteratively improves itself by planning, editing, evaluating, and learning from user interactions.

Guiding principles
- Autonomy with control: Human-in-the-loop by default; agentic autonomy unlocked progressively with clear safety rails.
- Observability: Every action auditable; logs, diffs, tool calls, and decisions are persisted and explainable.
- Validity: Automated checks (diff preview, compile, test, lint) precede any permanent changes; revert is always available.
- Composability: Tools, agents, and workflows are modular. LLMs can call tools and other LLMs.
- Determinism where possible: Stable IDs, reproducible runs, explicit configs.

Non-goals (for now)
- Infinite provider integrations; focus on the minimal set that enables the flywheel.
- IDE-level UX; keep the TUI focused, predictable, and scriptable.

Implementation logging and decision tracking
- Maintain sequential implementation logs named implementation-log-NNN.md under crates/ploke-tui/docs/, where each log records:
  - The problem statement, options considered, chosen approach, and rationale.
  - Cross-references to PRs/commits and any metrics or test evidence.
  - Any deviations from the plan and why (for observability).
- Maintain crates/ploke-tui/docs/decisions_required.md as the single queue of items that require USER review before proceeding.
  - Only add items that are blocking or significantly directional.
  - Each entry must include: context, options with tradeoffs, recommended default, and a deadline (if any).

-------------------------------------------------------------------------------
Milestone 0: Baseline hardening and observability
Goal: Make existing paths reliable, observable, and reversible.

Workstreams and tasks (granular)

A) Eventing: single source of truth for indexing and typed tool events
- A1. De-duplicate indexing completion:
  - Choose run_event_bus() as the single forwarder of IndexingStatus → AppEvent.
  - Remove direct AppEvent::IndexingCompleted/Failed emissions from handlers/indexing.rs; only emit IndexingStatus via event_bus.index_tx and let run_event_bus bridge.
  - Add a regression test: start a minimal IndexerTask, simulate Completed, and assert exactly one AppEvent::IndexingCompleted is received.
- A2. Introduce typed tool events (internal design for M0; implementation can land behind a feature flag):
  - Define AppEvent::LlmTool(ToolEvent) (e.g., ToolEvent::Requested, ::Completed, ::Failed) with structured fields {request_id, parent_id, call_id, name, vendor, args_hash}.
  - Keep a compatibility bridge from SystemEvent::ToolCallRequested → LlmToolEvent for one milestone to avoid breaking flows.
  - Emit both event types in M0 (behind cfg or config flag); remove SystemEvent path in M1.
- A3. Correlation IDs:
  - Standardize request_id (Uuid v4) and provider call_id (string) as correlation keys across LLM session → tool dispatch → handler → DB logging.
  - Update tracing spans to include %request_id and %call_id at all tool boundaries.

B) Persistence: conversations, tool calls, and chat history (atomic)
- B1. Database persistence (see ploke_db_contract.md):
  - ploke-db must expose functions to record ToolCallRequested/Completed/Failed with timestamps and latency, and to persist conversation turns (user/assistant/sysinfo).
  - Enforce idempotency on (request_id, call_id) upserts to prevent duplicates under retries.
- B2. Chat history persistence:
  - Fix FileManager::save_content to write to a final file path using atomic write (temp + fsync + rename).
  - Default save location: current working directory joined with ".ploke_history.md" (requires USER decision; tracked in decisions_required.md).
  - Emit SysInfo with the final file path on success; structured error on failure.

C) Logging and telemetry (structured, correlated)
- C1. Tool-call telemetry:
  - Log fields: request_id, call_id, vendor, tool_name, args_sha256 (of canonicalized JSON), started_at, ended_at, latency_ms, outcome (ok/err), error_kind, error_msg.
  - Use tracing::info_span to measure latency; record at completion.
- C2. Subsystem propagation:
  - Ensure request_id flows from llm::session through tool_call::dispatch_and_wait to handlers::rag::handle_tool_call_requested and down to ploke-io where applicable.
- C3. Toggle tracing:
  - Initialize tracing by default with EnvFilter; write file logs to logs/ploke.log (already scaffolded). Keep ANSI off in file logs.

D) Safety envelope: IO path policy and user guidance
- D1. Enforce absolute paths and symlink policy in ploke-io (already the default); surface violations as user-friendly SysInfo with remediation.
- D2. Config surface (read-only in M0): document editing.roots and symlink policy; implementation of policy changes can wait for M1.

E) Backpressure and capacity hygiene
- E1. Broadcast channel capacities (EventBusCaps):
  - Validate defaults under load; add metrics for lag via RecvError::Lagged counters.
  - Recommendation: keep realtime small (100), background larger (1000), index large (1000).
- E2. Avoid .expect() on send paths; convert to warnings and continue.

F) Documentation and review artifacts
- F1. Observability guide:
  - Document how to trace a tool call end-to-end (grep request_id), where to find logs, and how to query DB for a given request_id/call_id.
- F2. Implementation logs:
  - Start with crates/ploke-tui/docs/implementation-log-001.md for M0. Record key decisions and the evidence used.
- F3. Decision queue:
  - Maintain crates/ploke-tui/docs/decisions_required.md with only blocking/directional items.

Deliverables
- Code:
  - Event dedup: IndexingCompleted/Failed fired once via run_event_bus.
  - Typed tool events defined and compatibility bridge in place (or flag-guarded).
  - FileManager::save_content fixed to persist to an actual file using atomic rename.
  - Telemetry fields logged for tool calls with correlation IDs.
- Docs:
  - milestone0_hardening_plan.md (this granular plan).
  - ploke_db_contract.md (DB function/behavior contract).
  - Observability guide (minimal).
  - decisions_required.md created and populated with initial items.

Acceptance (M0 exit)
- All tool executions are visible in logs and persisted in the DB with request_id/call_id correlation.
- No duplicate AppEvent::IndexingCompleted under normal and high-load test scenarios.
- Chat histories are persisted deterministically to a visible file path via atomic rename.
- Typed tool events exist (design complete), and a migration plan to retire SystemEvent::ToolCallRequested is documented.

Notes
- Some changes (typed LlmToolEvent migration) can be completed in M1 if needed; M0 must ship telemetry, persistence, and event dedup with minimal risk.
- See crates/ploke-tui/docs/ploke_db_contract.md for required DB functions and idempotency semantics.

-------------------------------------------------------------------------------
Milestone 1: Safe editing pipeline (human-in-the-loop)
Goal: Users can approve/deny code edits with confidence; edits are reversible.

Scope
- Tool: apply_code_edit (exists) hardened
  - Input schema: edits[], confidence?, namespace? (done in code; expand tests).
  - Preflight: diff previews from current on-disk to proposed splice.
  - Approval gate: Config.editing.auto_confirm_edits (default false); agent gating is off for now.
- Git integration (MVP):
  - Initialize repo if missing; create a working branch for edits (ploke/work-in-progress).
  - Stage edits, commit with structured message (tool-call metadata), lightweight tags.
  - One-click revert last edit set (soft rollback).
- UI/UX:
  - “edit approve <request_id>” / “edit deny <request_id>”.
  - SysInfo messages: file list, short diffs, expected hash mismatches, outcomes.
- Persistence:
  - Store edit proposals and outcomes (approved, denied, applied, reverted) in DB with pointers to git commit(s).

Deliverables
- Review-and-apply flow with diff previews and audit log.
- Basic git wrapper with branch+commit+revert.
- CLI/TUI commands for approve/deny.

Acceptance
- A user can request an LLM change, see diffs, approve, and get a committed change.
- Revert works within a single command, returning repo to prior state.

-------------------------------------------------------------------------------
Milestone 2: Context/navigation tools for LLM
Goal: Let models “look around” effectively without overfetching.

Scope
- Expand tool set:
  - request_code_context (exists) extensions: “containing module”, “siblings”, “implementations”, “callers/callees” (as available from current module tree + DB relations).
  - semantic_search tool: dense (existing RagService), BM25, hybrid (exists), with constraints (token budget).
  - file_content_range tool: fetch byte ranges by path for ground truth verification.
  - symbol_lookup tool: find items by name or fuzzy matching; return IDs and paths.
- Budget control:
  - Consistent token and char budgets for tooling responses.
- Observability:
  - Log tool rationale lines when present in LLM output (optional reflection).

Deliverables
- Tool definitions + handlers with strong argument validation.
- Tests for each tool: mapping JSON → DB/IO query → results.

Acceptance
- Models can navigate and assemble relevant context in a handful of tool calls.
- Tool outputs remain within token budgets.

-------------------------------------------------------------------------------
Milestone 3: Automated validation gates
Goal: Every edit goes through safety checks before landing.

Scope
- Validation steps (configurable):
  - cargo fmt --check
  - cargo clippy -D warnings
  - cargo test (subset by filter first; full run optional)
  - Targeted compile checks for changed crates/modules.
- Gate policy:
  - Deny by default on validation failures unless overridden.
  - Attach validation reports to ToolCallCompleted result JSON.
- Fast path:
  - Heuristic “lint-only” gate for small refactors; configurable.

Deliverables
- Validation service (async tasks, timeouts, streaming logs).
- Config knobs: which gates to run, timeouts, pass/fail policies.
- Surfaced results into SysInfo and persisted.

Acceptance
- Failed validation prevents commit by default.
- Users can override and re-run with different policies.

-------------------------------------------------------------------------------
Milestone 4: Single-agent loop (plan → act → observe → reflect)
Goal: One agent can autonomously iterate toward a user goal under safety rails.

Scope
- Agent runtime:
  - Planner step: break user goal into small steps.
  - Action step: use tools (context, edit) to progress.
  - Observer step: parse validation and tests; summarize.
  - Reflector step: adjust plan; retry up to N times.
- Controls:
  - Global budget per request (time, tool calls, cost).
  - Cancellation token propagated to all tasks.
- Memory:
  - Append agent thoughts, actions, and outcomes into conversation + DB.

Deliverables
- Agent loop with explicit state machine and clear JSON-serializable step log.
- UI commands: “agent start <goal>”, “agent cancel <id>”, “agent status”.

Acceptance
- Agent can implement small refactors (rename symbols, update use paths) end-to-end.
- Full audit trail of decisions and actions.

-------------------------------------------------------------------------------
Milestone 5: Multi-path solution search (fan-out/fan-in)
Goal: Explore multiple candidate solutions in parallel; select or merge best.

Scope
- Fan-out:
  - Parallel branches with varied prompts, seeds, and models (configurable).
  - Shared context bus to exchange insights across branches (limited).
- Discriminator:
  - Heuristic scorer using signals: test pass %, diff size, lint score, code metrics, user profile relevance.
  - Optional LLM judge with constrained rubric.
- Fan-in:
  - Select best branch; or synthesize a merged solution (minimal conflict set).
  - Apply via the safe editing pipeline (Milestones 1–3).
- Git strategy:
  - Separate branches per candidate (ploke/cand-<n>); merge best into working branch after validation.

Deliverables
- Parallel orchestration and scoring.
- Report of candidates, scores, and decision rationale.

Acceptance
- Multi-branch generation runs concurrently; best solution is chosen and validated before landing.
- User can override and select a different candidate.

-------------------------------------------------------------------------------
Milestone 6: Conversation → knowledge graph + retrieval
Goal: Leverage conversations as first-class knowledge and memory.

Scope
- Data model:
  - Persist conversation turns, tool calls, outcomes, and code-change summaries to DB.
  - Edges to code items and commits: “talked-about”, “modified”, “regressed”, “reviewed”.
- Embeddings + search:
  - Dense embeddings for conversation turns; BM25 over transcripts.
  - Hybrid retrieval for “what was discussed/decided about <X>?”.
- Summarization:
  - Periodic summarization of threads into durable knowledge nodes (with provenance).

Deliverables
- Schemas and migration scripts in ploke-db.
- Retrieval tools for LLM: “find related discussions to <symbol>”.

Acceptance
- Conversations are queryable and help future agents make decisions.
- Summaries improve tool-call efficiency for recurring tasks.

-------------------------------------------------------------------------------
Milestone 7: User profile and personalization
Goal: Tailor generation and ranking to the user’s style and preferences.

Scope
- Profile model:
  - Collect signals: approved vs. denied edits, preferred models, typical patterns, language idioms.
  - Build embeddings and simple features.
- Personalization:
  - Re-rank candidates using profile similarity.
  - Adaptive prompting (e.g., more/less verbose, Rust edition idioms).
- Controls:
  - Privacy and opt-out; export/import profile file.

Deliverables
- Profile storage + embedding pipeline.
- Personalization hooks in scorer/selector.

Acceptance
- Solutions align better with the user’s accepted patterns.
- Measurable improvement in approval rate and reduced iteration count.

-------------------------------------------------------------------------------
Milestone 8: Multi-agent orchestration
Goal: Pluggable roles collaborate toward complex goals.

Scope
- Roles:
  - Planner, Coder, Reviewer, Tester, Integrator
- Protocol:
  - Turn-taking with a shared blackboard (DB-backed).
  - Typed messages: Plan, Critique, Patch, TestReport, MergeProposal.
- Coordination:
  - Supervisor applies constraints (budget, deadlines, quality thresholds).
- Tooling:
  - “call-llm” tool for LLM-as-tool fan-out; supports model switching mid-run.

Deliverables
- Orchestrator with role registry and configurable topologies (linear, DAG).
- Minimal examples: “Introduce feature flag”, “Replace logging crate”, “Extract module”.

Acceptance
- Multi-agent runs can solve larger tasks than single-agent loops.
- Supervisor can pause/abort and present a structured status report.

-------------------------------------------------------------------------------
Milestone 9: Reliability, resilience, and ops
Goal: Make it robust under real usage.

Scope
- Checkpointing and resume:
  - Persist in-flight agent state; resume after crash/restart.
- Timeouts and circuit breakers:
  - Per-tool, per-agent; exponential backoff and fallbacks.
- Metrics:
  - Success rates, average iterations to completion, gate failure rates.
- Load:
  - Bound broadcast channels; track lagging; backpressure where needed.

Deliverables
- Resumable agent runs with durable checkpoints.
- Operator metrics and dashboard-friendly logs.

Acceptance
- Long-running operations can survive restarts.
- No unbounded channel growth under stress tests.

-------------------------------------------------------------------------------
Milestone 10: Packaging, docs, and templates
Goal: Smooth onboarding and repeatable success.

Scope
- Packaging:
  - Example configurations, model presets, and project templates (Rust crates).
- Docs:
  - End-to-end tutorial: “Ship a feature with Ploke”.
  - API docs for tools, events, and extending agents.
- Templates:
  - Ready-made workflows: “Refactor module”, “Add unit tests”, “Introduce CLI flag”.

Deliverables
- Templates and tutorials checked into repo.
- Release notes and change log.

Acceptance
- New users can follow a single doc to ship a small feature safely.

-------------------------------------------------------------------------------
Evaluation and success criteria (ongoing)
- Autonomy: % of tasks completed without manual edits after approval gating.
- Validity: test pass rate post-edit; lint error rate; build break rate.
- Observability: % of steps with complete logs and correlations.
- Control: mean time-to-cancel; revert success rate; user override frequency.
- Efficiency: avg iterations to completion; tool-call counts; token usage and latency.
- User satisfaction: approval rate; task acceptance on first pass.

-------------------------------------------------------------------------------
Risks and mitigations
- Tool misuse by LLM → Strict schemas, preflight validation, conservative defaults.
- Path policy violations → Deny and explain; require absolute paths; roots whitelist.
- Flaky tests/long runs → Timeouts, sharding tests, fast pre-checks.
- Cost escalation → Budget controls, cheaper models for exploration, switch to premium for finalization.
- Model drift → Versioned prompts, regression suite of tasks, deterministic seeds where feasible.

-------------------------------------------------------------------------------
Flywheel toward self-improvement
- The system logs plans, diffs, decisions, and outcomes in a structured manner.
- Agents generate refactor proposals for Ploke itself; humans approve at first; later, agent gates enable auto-apply when confidence and test coverage are high.
- Successes enrich the knowledge graph; subsequent tasks become faster and more accurate.

-------------------------------------------------------------------------------
Concrete next steps (prioritized)
1) M0 hardening and observability (small PRs):
   - Consolidate indexing completion events; remove deprecated tool-call routing in UI path.
   - Fix FileManager save_content to write final target file and log actual path.
   - Persist tool-call request/response rows to DB (simple schema).
2) M1 human-in-the-loop editing:
   - Implement diff previews + approve/deny commands fully wired (StateCommand + handler).
   - Minimal git wrapper: init, branch, commit, revert.
3) M3 validation gates:
   - Add cargo fmt/clippy/test gates with timeouts; surface outputs; deny on failure.
4) M4 single-agent loop (scoped):
   - One agent with plan → act → observe → reflect over small refactors.
   - Cancellation and per-run budgets.

Each subsequent milestone builds atop these foundations to reach multi-path and multi-agent autonomy with strong safety and observability.
