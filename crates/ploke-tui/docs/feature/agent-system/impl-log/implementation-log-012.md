# Implementation log 012 — M1 kickoff: approvals scaffolding and diff previews plan (2025-08-20)

Summary
- Created a granular plan for Milestone 1 (human-in-the-loop editing) focused on staging edit proposals, generating previews, and explicit approval/denial.
- Defined command surface (“edit approve <request_id>”, “edit deny <request_id>”), routing, proposal registry, and apply/deny semantics.
- Chose preview-first approach with code-blocks by default; unified diff optional via a small dependency.

Changes (docs only in this step)
- Added milestones/m1_granular_plan.md with a detailed WBS, acceptance, and risks.
- Expanded milestones/m1_context_files.md to align with the granular plan and call out context sufficiency.

Context sufficiency (assessment)
- ploke-tui:
  - Ready: apply_code_edit tool entrypoint (rag.rs), IoManager write_snippets_batch, event bus.
  - Needed: Approve/Deny commands and routing, in-memory proposal registry in AppState, preview generator, config flags.
- External (parallel/blockers):
  - ploke-db needs code_edit_proposal and code_edit_outcome relations + APIs for persistence (tracked separately).
  - Git wrapper is scoped for later in M1 but not required for core “approve/deny → apply” flow.

Plan of record (next PRs)
1) Wire StateCommands + parser/exec/dispatcher; add HELP_COMMANDS entries.
2) Add in-memory proposal store; change apply_code_edit path to stage proposals + emit SysInfo with preview.
3) Implement Approve/Deny handlers; call write_snippets_batch; summarize outcomes.
4) Add unit + E2E tests; add “similar” for optional unified diff if needed.
5) Optional: config knobs editing.auto_confirm_edits (default false) and editing.preview_mode (“codeblock” default).

Acceptance alignment
- Human-in-the-loop approval is explicit and auditable.
- No disk changes occur before approval.
- Outcomes are summarized; errors surfaced clearly (hash mismatch, IO errors).

Notes
- Maintain compatibility bridge via SystemEvent for one milestone; migrate to typed tool events in M2 if stable.
- DB persistence of proposals will follow once ploke-db exposes the schema and APIs.
