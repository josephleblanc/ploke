# Plan: LLM Tool-Driven Code Edits — Evals, UX, and Integration

Status: draft
Scope: ploke-tui (UI/eventing), ploke-io (I/O), ploke-embed + ploke-rag (indexing + search), ploke-db (graph/embeddings)

This document analyzes the current implementation, identifies gaps, and proposes a phased plan to:
- Build evals for LLM tool-call adherence and safety for code edits.
- Design the user experience for proposing, approving, and applying edits with diffs.
- Integrate accepted edits into the broader Ploke system (database, embeddings, sparse index).

---

## 0) Current Implementation Summary

Observations from the code:
- Tool definitions:
  - request_code_context and apply_code_edit are defined as OpenAI-style tool functions in ploke-tui/src/llm/mod.rs.
- Tool execution path (deprecated shim):
  - LLM tool-call events are routed through AppEvent::System(SystemEvent::ToolCallRequested) and handled in app_state/handlers/rag.rs:handle_tool_call_requested.
  - This handler parses arguments for apply_code_edit, builds ploke_core::WriteSnippetData items, and calls IoManagerHandle::write_snippets_batch.
  - The handler returns a ToolCallCompleted/ToolCallFailed SystemEvent with a JSON payload summarizing outcomes.
- Test coverage:
  - e2e_apply_code_edit.rs performs a real API call (env-gated) and forces a deterministic tool execution via a SystemEvent injection. It validates that an edit applies to a temp file.
- Indexing follow-ups:
  - There is infrastructure for indexing and BM25 services; update flows are present (e.g., UpdateDatabase, bm25 rebuild/status), but no direct coupling from “edit accepted” to “refresh the code graph/embeddings” is implemented yet.
- UX:
  - Edits are not yet surfaced as user-approvable “proposals” in the conversation history. No diff view/collapsible blocks exist yet. No config toggle for autoconfirm.

---

## 1) Gaps and Risks

- Event model:
  - Tool calls are routed via a deprecated SystemEvent path; a purpose-built tool event channel would be clearer and safer (less coupling, better typing).
- UX:
  - No explicit “edit proposal” message type.
  - No collapsible diff UI or code-edit presentation options (unified diff, split view, inline before/after blocks).
  - No approval/deny workflow; no autoconfirm configuration or per-edit policy gates (e.g., require min confidence).
  - No partial acceptance (multi-edit batch where user approves subset).
  - No rollback/undo mechanism (helpful for quick revert).
- Evals/metrics:
  - No harness to compare prompt variants, models, and context sizes.
  - No metrics on malformed tool calls (missing fields, invalid UUIDs, bad offsets, cross-file ops, etc.).
  - No artifact export (JSON/CSV) for regression tracking in CI.
- Safety/robustness:
  - No preflight validation beyond expected hash and byte ranges at the UI layer.
  - Namespace handling is permissive; path policy enforcement relies on ploke-io but we should also report violations prominently.
  - Large-file edits, overlapping edits, or multi-file atomic batches lack dedicated test coverage.
- Integration:
  - No automatic graph/embedding updates upon acceptance; “update” exists but isn’t wired to the approval flow.
  - No BM25 incremental update after edits (full rebuild is available, but heavy).
- Observability:
  - Sparse metrics for tool-call success/failure causes and latencies.

---

## 2) Evals (Tests) Plan

Goals:
- Measure correctness of tool-call shape and content under various prompts and models.
- Evaluate adherence to instructions with different prompt variants, rank by success rate and tie-break by prompt brevity.
- Explore alternative tool interfaces that minimize cognitive load on the LLM (e.g., no byte offsets).

Test Modes:
- Offline deterministic mode:
  - Bypass live API by injecting ToolCallRequested events with controlled arguments (already done in the e2e test).
  - Use synthetic conversations with controlled size to simulate low/high token contexts.
- Online mode (env-gated, non-CI):
  - Run live API calls for a sample of models; still force deterministic tool execution with our SystemEvent injection to validate the full stack.
  - Skip by default unless PLOKE_TUI_E2E_LLM=1.

Edge Cases to Cover:
- JSON shape errors (missing fields, wrong types).
- Invalid expected_file_hash (non-UUID, mismatched).
- Byte range issues (out-of-bounds, inverted start/end, overlapping edits).
- Multi-edit batches across files (success + partial failures).
- Path policy violations (cross-root, non-absolute paths if policy requires).
- Large replacements and UTF-8 boundaries (ensure no invalid slicing).
- Concurrency: two edits on same file serialized by ploke-io (lock correctness).
- Atomicity: temp-write + fsync + rename integrity simulation.

Adherence Experiments:
- Prompt variants:
  - Minimal imperative instruction.
  - With examples (few-shot).
  - “You MUST call the tool” variants with different safety phrasing.
  - Explicit JSON schema excerpt vs. short name-only guidance.
- Context size:
  - Small vs. large context history.
- Models:
  - Curated short-list via ProviderRegistry defaults.
  - Additional user-configured entries (e.g., OpenRouter small/fast vs. larger models).

Metrics:
- Primary:
  - Tool call correctness rate (valid JSON shape + all required fields + passes validation).
  - Edit application success rate (per-edit).
- Secondary:
  - Latency to tool call emission.
  - Frequency of hallucinated fields/keys.
- Ranking:
  - Rank prompts by correctness; tie-break by prompt character length.
- Artifacts:
  - Write JSON/CSV into tests/artifacts/llm_tool_evals/YYYYMMDD-HHMM/ with:
    - model, prompt_id, context_size, trial_id
    - ok, reason_if_fail, counts, durations
    - captured payloads (anonymized paths)

Implementation Notes:
- Provide a small test harness in ploke-tui/tests (unit + integration) to synthesize conversations, inject ToolCallRequested, and capture ToolCallCompleted/Failed.
- Make the harness reusable to iterate prompt variants and models driven by a TOML/JSON config under tests/cases/.
- Keep CI to offline mode only; online gated via env.

Future Alternative Tool Designs to Evaluate:
- apply_code_edit_v2 (minimal interface):
  - LLM provides: canonical module path + rename intent + new name.
  - System computes offsets using AST lookup and code graph; LLM no longer provides byte offsets or hashes.
- apply_text_patch:
  - LLM provides multi-hunk unified diff; system validates and applies to file (requires robust patching and conflict handling).
- apply_node_edit:
  - LLM specifies node identifier (from our graph) and a mutation; the system locates and edits precisely.

---

## 3) UX Plan

Objectives:
- Treat edits as proposals in the chat UI.
- Offer an approval dialog with diff presentation; persist decisions and context.
- Honor autoconfirm from config with optional confidence threshold.

Proposed Changes (high-level):
- State/Events:
  - New MessageKind: CodeEditProposal (or reuse Tool with clear formatting).
  - New AppEvent:
    - CodeEditsProposed { proposal_id, edits: Vec<EditSummary>, confidence, source_request_id }
    - CodeEditsApplied { proposal_id, applied: usize, results: Vec<ResultSummary> }
    - CodeEditsRejected { proposal_id }
  - New StateCommand:
    - ShowEditProposal { proposal_id }
    - ApproveEdits { proposal_id }
    - RejectEdits { proposal_id }
    - ToggleAutoConfirm(bool) or configuration via user_config.
- Rendering:
  - Collapsible message block summarizing proposed edits:
    - Per-file summary with counts and short previews.
    - Expand to show:
      - Unified diff (default)
      - Inline before/after blocks
      - Configurable view style in user_config
  - Keyboard shortcuts:
    - a: approve proposal
    - r: reject proposal
    - v: toggle view style
    - o: open/close proposal details
- Config:
  - EditingConfig {
      auto_confirm_edits: bool,
      agent: { enabled: bool, min_confidence: f32 }
    }
  - If auto_confirm_edits && (confidence >= min_confidence) then auto-apply; otherwise queue as proposal.

Lifecycle:
1) LLM initiates tool call (or we inject for offline test).
2) System emits CodeEditsProposed with diff previews (requires computing diffs before application).
3) UI shows collapsible proposal message.
4a) On approval:
    - Apply edits via IoManager.
    - Show CodeEditsApplied with results, update chat message.
    - Trigger indexing/embedding updates (see Integration).
4b) On rejection:
    - Emit CodeEditsRejected, log and archive proposal.

Diff Generation:
- Before apply, read file content and synthesize diffs from planned splices.
- Support configurable diff styles:
  - Unified diff (textpatch-like)
  - Side-by-side inline blocks (smaller)
- For very large diffs, default to collapsed with a short preview and controls.

Undo:
- Keep a backup of original bytes and new bytes per file (or rely on git if repo).
- Offer quick “revert edit” command for last accepted proposal.

---

## 4) Integration Plan (Graph, Embeddings, BM25)

Minimal viable (Phase 1):
- Upon acceptance, call IoManager::write_snippets_batch (already implemented).
- Then trigger a file-level scan + update:
  - Use existing StateCommand::UpdateDatabase or StateCommand::ScanForChange to recompute hashes and detect changed files.
  - For each changed file, pass through the IndexerTask to recompute embeddings.
- Update BM25:
  - Option 1: call bm25_rebuild (heavy but simple).
  - Option 2: incremental add/update for changed files (preferred; implement when ready).
- Notify UI with IndexingStarted/Progress/Completed events.
- After completion, emit a short summary message.

Refinements (Phase 2+):
- Incremental sparse index updates.
- AST-aware updates: if the LLM edited only a node, update embeddings only for impacted nodes (requires better node-level write API).
- Consistency checks:
  - Confirm post-apply file hash matches what ploke-io returns; if not, surface error.

---

## 5) Observability and Safety

- Add structured logs:
  - tool_call_incoming: vendor, name, confidence, namespace, num_edits.
  - tool_edit_result: per file (ok/error, reason).
  - approval_decision: user, autoconfirm, confidence.
- Metrics:
  - counters for proposals, approvals, rejections, applied edits, failures.
  - histograms for tool call latency, apply latency, indexing latency.
- Safety gates:
  - Validate paths against configured roots.
  - Enforce absolute paths if required; deny cross-root by default (as in ploke-io).
  - Provide clear UI errors when path policy violations occur.

---

## 6) Roadmap

Milestone A: Evals Harness
- Implement offline eval tests in ploke-tui/tests:
  - Synthetic conversations and forced ToolCallRequested events.
  - Edge cases and correctness metrics.
  - Artifact dumps (JSON/CSV).
- Add gating and documentation for online evals (PLOKE_TUI_E2E_LLM=1).

Milestone B: Basic UX for Proposals
- New state commands/events and a minimal proposal message with summary + approve/reject.
- Config: auto_confirm_edits + min_confidence; default off.
- Apply path hooks: on approval, run write_snippets_batch and show results in chat.

Milestone C: Diff Presentation
- Collapsible UI with unified diff rendering and optional inline blocks.
- Keybindings for approve/reject/toggle view.

Milestone D: Integration with Indexing
- After acceptance, run file-level scan + re-embed changed files.
- Rebuild/refresh BM25 (initially full rebuild; later incremental).

Milestone E: Event Model Cleanup
- Replace deprecated SystemEvent::ToolCallRequested path with dedicated tool event(s).
- Move apply_code_edit handling out of rag.rs into a dedicated tool module.

Milestone F: Alternative Tool Interfaces
- Prototype apply_code_edit_v2 with reduced LLM complexity (no byte offsets).
- Evaluate success rates vs. current approach in the eval harness.

---

## 7) Implementation Notes (Low-Risk First Steps)

- Tests/evals:
  - Add a helper to generate diff previews for a set of planned edits against current file contents.
  - Build an artifact writer module to dump results to tests/artifacts/ with timestamps.
- UX:
  - Start by adding a single “proposal” message with a simple summary and a command for approval.
  - Defer complex diff visualization to Milestone C.
- Integration:
  - Wire acceptance to existing UpdateDatabase / ScanForChange + IndexerTask.
  - Start with BM25 rebuild (explicit user confirmation), then move to incremental.
- Event model:
  - Keep current deprecated path until new tool events are in place; avoid churn during evals.

---

## 8) Acceptance Criteria

- Evals:
  - A suite of offline tests covering edge cases and measuring correctness produces stable artifacts.
  - Online tests run manually with a clear README and pass on at least one modern model.
- UX:
  - Users see a proposal with basic details and can approve/reject.
  - Autoconfirm works with min confidence threshold.
- Integration:
  - After approval, changed files are detected and embeddings refreshed; BM25 updated via rebuild (Phase 1).
- Observability:
  - Basic metrics and structured logs exist for tool calls and edit outcomes.

---

## 9) Open Questions

- Should proposals persist across sessions? If yes, add serialization for pending proposals.
- Should we limit edits to files within configured roots only (enforced at UI layer too)?
- How to handle git worktrees or non-git environments for diff/undo?
- UI ergonomics for multi-edit batches: partial apply or all-or-nothing UX?

---
