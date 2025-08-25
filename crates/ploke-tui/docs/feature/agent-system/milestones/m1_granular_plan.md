# Milestone 1 — Safe editing pipeline (human-in-the-loop), granular plan

Objective
- Stage code edits from the apply_code_edit tool as proposals.
- Present diff previews to the user.
- Apply only upon explicit approval; deny cleanly.
- Persist proposal lifecycle once ploke-db adds schema; until then, maintain an in-memory registry with SysInfo audit messages.

Outcome
- A user can “edit approve <request_id>” or “edit deny <request_id>” to control edits proposed by the LLM.
- Diff preview is shown before apply; default presentation uses code-block “before/after”, with an optional unified diff.
- Atomic apply via ploke-io write_snippets_batch with tracking-hash validation.

Work breakdown structure (WBS)

Phase 1: Command surface and routing
- Add StateCommand variants:
  - ApproveEdits { request_id: Uuid }
  - DenyEdits { request_id: Uuid }
- Parser and executor:
  - Parse “edit approve <request_id>”, “edit deny <request_id>”.
  - Dispatch via cmd_tx to StateCommand::{ApproveEdits, DenyEdits}.
- Dispatcher:
  - Route ApproveEdits/DenyEdits to rag handler functions.

Phase 2: Proposal capture and in-memory registry
- Data model (new in ploke-tui):
  - struct EditProposal { request_id, parent_id, call_id, proposed_at_ms, edits: Vec<WriteSnippetData>, files: Vec<PathBuf>, args_hash: String, preview: DiffPreview, status: Pending|Approved|Denied|Applied|Failed(String) }
  - enum DiffPreview { CodeBlocks { per_file: Vec<BeforeAfter> }, UnifiedDiff { text: String } }
- AppState:
  - Add proposals: HashMap<Uuid, EditProposal>
  - Thread-safety: hold inside RwLock on AppState or a dedicated Arc<RwLock<HashMap<…>>>.
- Handler change:
  - In handlers/rag.rs handle_tool_call_requested for name == "apply_code_edit":
    - Instead of immediate apply, build WriteSnippetData list, compute diff preview, store EditProposal with status=Pending.
    - Emit SysInfo message with a concise summary + how to approve/deny.

Phase 3: Diff preview generation
- Add dependency: similar = "2" (or similar/unified_diff style crate).
  - If unavailable, fall back to code-blocks only.
- Implement function:
  - fn build_preview(edits: &[WriteSnippetData]) -> Result<DiffPreview>
  - For each file, read current content, compute replaced content in-memory, and produce:
    - CodeBlocks: two fenced code blocks per file (before/after).
    - Optionally UnifiedDiff: single unified diff per file; concatenate.
- Config hook:
  - Editing.preview_mode: "codeblock" | "diff" with default "codeblock".
  - Editing.auto_confirm_edits: bool, default false.
  - If auto_confirm_edits = true, immediately run apply flow but still emit preview as audit.

Phase 4: Approval/denial flows
- On ApproveEdits:
  - Lookup proposal by request_id; if Pending, attempt apply via io_handle.write_snippets_batch.
  - Update status to Applied or Failed with error string.
  - Emit SysInfo with per-file outcomes and new hashes; emit LlmTool Completed/Failed accordingly (bridge for SystemEvent remains for M1).
- On DenyEdits:
  - Mark proposal as Denied.
  - Emit SysInfo summarizing denial and how to re-request.
- Idempotency:
  - If Approve called twice: no-op with SysInfo warning (status already Applied/Denied).
  - If Deny after Applied: no-op with SysInfo warning.

Phase 5: Persistence (blocked on external ploke-db)
- New relations (in ploke-db, tracked separately):
  - code_edit_proposal(request_id, at:Validity => parent_id, call_id, args_sha256, edits_json, preview_mode, status)
  - code_edit_outcome(request_id, at:Validity => applied:Bool, results_json, error_kind?, error_msg?)
- ploke-tui integration:
  - When available, mirror proposal and outcome writes to DB using an ObservabilityStore-like trait.
  - Until then, maintain in-memory only and audit via SysInfo.

Phase 6: Tests
- Unit: JSON → WriteSnippetData mapping (pure).
- Unit: build_preview produces non-empty previews; stable for trivial inputs.
- E2E: temp file, stage edits via apply_code_edit tool path; assert SysInfo preview emitted; approve; validate on-disk contents and success message.
- E2E: denial path; ensure no file change and correct SysInfo summary.
- Concurrency: Approve and Deny racing resolves with exactly one terminal status and deterministic SysInfo.

Phase 7: UX and ergonomics
- Help text: extend HELP_COMMANDS with approve/deny.
- Optional: keybindings Y/N when a pending proposal is selected (tracked post-M1 unless trivial).
- Tracing: include %request_id, %call_id when staging and applying.

Context sufficiency assessment

Sufficient in this repo (ploke-tui):
- Tool entrypoint exists: handlers/rag.rs handles apply_code_edit already.
- IO path available: state.io_handle.write_snippets_batch(edits) with TrackingHash validations.
- Eventing: SystemEvent compatibility path and LlmTool events exist; either can carry ToolCallCompleted/Failed.

Gaps to fill in ploke-tui:
- Proposal registry in AppState (new).
- Commands and routing for Approve/Deny (new).
- Diff preview generation utility (new) + dependency (“similar” or equivalent).
- Config: editing.auto_confirm_edits, editing.preview_mode (new).

External gaps (blocked or parallel work):
- ploke-db: code_edit_proposal and code_edit_outcome relations + API.
- Optional: Git wrapper (init, branch, commit, revert) — planned but can land after core M1.

Risk and mitigations
- Risk: Large diffs in TUI rendering.
  - Mitigation: cap preview lines per file; paginate or truncate with a hint.
- Risk: Hash mismatch on apply.
  - Mitigation: show clear SysInfo per-file error; suggest refresh/update.
- Risk: Race conditions on repeated approvals.
  - Mitigation: status check and atomic update of proposal state.

Acceptance criteria
- Users see diff previews and can approve or deny by request_id.
- Approved edits apply atomically; outcomes are summarized with new hashes.
- Denied edits do not touch disk; registry reflects terminal state.
- Tests cover mapping, previews, and E2E apply/deny flows.

Implementation order (recommended)
1) Add StateCommands + parser/exec/dispatcher routing.
2) Add in-memory proposal store + staging in rag handler.
3) Add preview generator (code-blocks first; unified diff optional).
4) Implement Approve/Deny handlers.
5) Tests (unit + E2E).
6) Optional: config surface and unified diff path.
7) DB integration once available.

Progress update (2025-08-20)
- Phase 2 complete: in-memory proposal registry added to AppState.
- Phase 3 initial: code-block previews implemented with truncation; optional unified diff path added via "similar" crate (config-driven).
- Phase 4 partial: approval flow implemented; optional auto-approval gating added via config (disabled by default).
- Dispatcher and command wiring for Approve/Deny complete.
- Back-compat fix: introduced app_state::Config wrapper and made ConfigState::new generic over Into<core::Config>; this allows legacy tests constructing Config via struct literal to compile without specifying the new `editing` field.

References
- Tool schema: crates/ploke-tui/src/llm/mod.rs (apply_code_edit tool).
- Current handler: crates/ploke-tui/src/app_state/handlers/rag.rs (apply_code_edit path).
- Types: ploke-core WriteSnippetData; ploke-io write_snippets_batch.
- Related doc: milestones/m1_context_files.md

Status summary (checkpoint 2025-08-20)
- Implemented:
  - Proposal staging: apply_code_edit requests are staged, not immediately applied; preview generated (code-blocks by default, unified diff optional via "similar").
  - Approval/denial: approve_edits and deny_edits handlers wired; idempotent terminal states with SysInfo confirmations.
  - Runtime controls: edit preview mode, preview line cap, and auto-approval toggles; commands parsed and dispatched.
  - Eventing: SystemEvent::ToolCallRequested compatibility path retained; LlmTool bridge present.
  - Tests: added unit and flow tests for parser, staging, approve, and deny.
- Deviations and rationale:
  - Kept SystemEvent bridge in M1 for stability and reduced risk; removal planned for M2 once typed tool events are primary.
  - Preview default set to code-blocks with truncation for readability; unified diff supported but opt-in (performance/UX considerations).
  - Persistence to ploke-db for proposals/outcomes deferred until ObservabilityStore APIs are fully stabilized; tracked in decisions_required.md.

Pre-M2 checklist
- Automated tests to add/verify:
  - Hash-mismatch handling in approve_edits returns per-file errors and preserves disk state.
  - Overlapping edit ranges rejected per file (already validated in handler; add unit to assert).
  - Auto-approval path applies successfully under config.editing.auto_confirm_edits=true.
  - Large previews are truncated deterministically according to config.editing.max_preview_lines.
- Manual validation (one pass per platform recommended):
  - Stage small rename edit; verify preview and explicit approval apply change atomically.
  - Stage multi-file edits; verify non-overlapping validation and unified diff mode.
  - Deny staged edits; verify no on-disk changes; status transitions to Denied.
  - Stage with incorrect expected_file_hash; verify clear error on apply and no changes.
  - Toggle preview mode and line caps at runtime; confirm SysInfo confirmations and preview output.
  - Exercise idempotency: double approval or double denial yields a no-op with SysInfo notice.
  - Hybrid/BM25 searches still functional; no regressions in RAG features.
- Readiness gates before M2:
  - Add DB persistence for conversation turns and tool-call lifecycle (requested/completed/failed) per ploke_db_contract.md.
  - Wire code_edit_proposal/outcome persistence once APIs land.
  - Basic git branch/commit/revert wrapper for applied edits (M1 acceptance).
  - Trace correlation IDs end-to-end; confirm in logs with request_id and call_id.

Notes for M2
- With M1 stable, proceed to richer context/navigation tools and budget controls.
- Plan validation gates (fmt/clippy/test) to run automatically before apply in M3; design interface now to minimize churn later.
