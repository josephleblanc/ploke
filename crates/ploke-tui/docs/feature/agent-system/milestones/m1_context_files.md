# Milestone 1 — Context files checklist (Human-in-the-loop editing)

See also: milestones/m1_granular_plan.md for the full work breakdown.

Core TUI integration
- crates/ploke-tui/src/llm/mod.rs
  - Tool schema for apply_code_edit already defined; keep active.
- crates/ploke-tui/src/app_state/handlers/rag.rs
  - Change apply_code_edit path to STAGE proposals (no immediate apply).
  - Compute preview (code-blocks by default; unified diff optional).
  - Emit SysInfo with short summary and how to approve/deny.
  - Implement handlers:
    - approve_edits(request_id): apply via write_snippets_batch; emit per-file outcomes; update registry.
    - deny_edits(request_id): mark as Denied; emit SysInfo summary.
- crates/ploke-tui/src/app_state/commands.rs
  - Add:
    - ApproveEdits { request_id: Uuid }
    - DenyEdits { request_id: Uuid }
- crates/ploke-tui/src/app_state/dispatcher.rs
  - Route ApproveEdits/DenyEdits to rag handler.
- crates/ploke-tui/src/app/commands/parser.rs, src/app/commands/exec.rs
  - Parse:
    - "edit approve <request_id>"
    - "edit deny <request_id>"
  - Dispatch mapped StateCommands.
- crates/ploke-tui/src/app_state/core.rs (or AppState definition site)
  - Add proposals: HashMap<Uuid, EditProposal> protected by RwLock.

Preview generation
- New helper in ploke-tui:
  - fn build_preview(edits: &[WriteSnippetData]) -> Result<DiffPreview>
  - Strategy:
    - Load current content.
    - Apply edits in-memory to produce “after” content.
    - Produce:
      - CodeBlocks: fenced before/after per file.
      - Optional: Unified diff (add dependency “similar”).
- Config defaults:
  - editing.auto_confirm_edits = false
  - editing.preview_mode = "codeblock" | "diff" (default "codeblock")

IO and types
- crates/ploke-io (external crate): IoManagerHandle::write_snippets_batch; absolute-path policy enforced (already).
- crates/ploke-core/src/io_types.rs: WriteSnippetData, TrackingHash (already).
- Mapping utility (new, pure function) to build WriteSnippetData from tool JSON.

Eventing and observability
- Use existing LlmTool events or SystemEvent bridge for M1.
- Emit SysInfo audit messages for:
  - Proposal staged (short file list + how to approve/deny).
  - Approval outcome (per-file success/new hash; failures with reason).
  - Denial confirmation.

DB side (tracked separately)
- ploke-db: code_edit_proposal and code_edit_outcome relations (time-travel).
- Idempotent upserts keyed by (request_id, call_id).
- Not required to complete core M1 flow in ploke-tui; integrate when APIs land.

Tests to add
- Unit:
  - Mapping JSON → WriteSnippetData (pure).
  - build_preview produces non-empty previews; deterministic on simple input.
- E2E:
  - Stage: apply_code_edit → expect SysInfo preview + Pending registry entry.
  - Approve: expect files updated atomically, per-file new hashes.
  - Deny: expect no file changes; registry shows Denied.
- Concurrency:
  - Double approval/deny → single terminal state; remaining calls are no-ops with SysInfo notice.

Context sufficiency (quick check)
- Available now in this repo:
  - Tool entrypoint (rag.rs), IoManager handle, event bus plumbing.
- Needed additions in this repo:
  - New StateCommands, parser/exec wiring, dispatcher routes.
  - Proposal registry and preview generator.
  - Optional: config fields under editing.*.
- External dependencies:
  - ploke-db schema and APIs for proposals/outcomes (can follow M1 core).
  - Optional diff dependency (“similar”) for unified diff mode.

Notes
- Maintain compatibility bridge via SystemEvent for one milestone.
- Git integration and DB persistence can land after core approval flow if needed.
