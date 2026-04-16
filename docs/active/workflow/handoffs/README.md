# Workflow Handoffs

- owning_branch: `refactor/tool-calls`
- review_cadence: update when work is handed off, compacted, or resumed after a pause
- update_trigger: add or update an entry whenever recent context would otherwise be lost

Use this directory for short-lived handoff and bootstrap documents that help another person or agent resume work quickly.

Start from [recent-activity.md](recent-activity.md) for the rolling current-state board, then use this directory for task-specific handoff notes when needed.

## Files

- [recent-activity.md](recent-activity.md)
  Rolling summary of the most recently touched workflow docs and why they changed.
- [handoff-template.md](../../../workflow/handoff-template.md)
  Durable template for task-specific handoff notes.
- [2026-04-09-doc-review-followups.md](2026-04-09-doc-review-followups.md)
  Short closeout note from the workflow doc review pass and why the active indices look the way they do.
- [2026-04-09_run-record-design-handoff.md](2026-04-09_run-record-design-handoff.md)
  Early run-record and manifest split design note for eval persistence.
- [2026-04-10_conversation-capture-design.md](2026-04-10_conversation-capture-design.md)
  Design note for capturing LLM traffic via events instead of reconstructing it later.
- [2026-04-10_phase-1d-structured-llm-capture.md](2026-04-10_phase-1d-structured-llm-capture.md)
  Follow-up note on moving from debug-string capture toward structured LLM response capture.
- [2026-04-11_dual-syn-implementation-handoff.md](2026-04-11_dual-syn-implementation-handoff.md)
  Handoff for Rust 2015 `syn` compatibility work and the first introspection slice.
- [2026-04-12_introspection-implementation-progress.md](2026-04-12_introspection-implementation-progress.md)
  Compact implementation progress note for the early introspection path.

## Rules

- Keep these files operational and current, not archival.
- Put durable decisions in EDRs, durable beliefs in the evidence ledger, and method or conceptual synthesis in the [evalnomicon](../../../workflow/evalnomicon/README.md).
- If a handoff becomes historically important, summarize the stable conclusion elsewhere and let the handoff stay short.
