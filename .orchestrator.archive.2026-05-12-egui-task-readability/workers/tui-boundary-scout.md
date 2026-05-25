# Worker Packet: tui-boundary-scout

- role: Retainer
- refresh_after_questions: 5

## Active Task

### esm-map-tui-boundary: Map ploke-tui harness boundary to model.md

- lane: tui-boundary-scout
- priority: 2
- state: AssignedActive { worker: "tui-boundary-scout" }
- forbidden_edit:
  - docs/active/agents/2026-05-12_broad-bounded-surface-transition-plan.md
- docs:
  - docs/workflow/evalnomicon/drafts/edit-surface/model.md
  - crates/ploke-tui/src/tools/code_edit.rs
  - crates/ploke-tui/src/rag/tools.rs
  - crates/ploke-tui/src/rag/editing.rs
  - crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs
- acceptance:
  - Return exact file:line ranges where TUI stages EditProposal, resolves WriteSnippetData, applies edits, and eval lowers/checks TUI evidence.
  - Separate harness/projection facts from authority facts, using model.md terminology.
  - No edits; include smallest verification commands.

## Queued Tasks

No queued tasks.

## Report Contract

- Report changed files or `none`.
- Report verification commands and outcomes.
- Report blockers with exact file paths or evidence.
- Do not edit outside allowed surfaces.
- Do not touch forbidden files.
