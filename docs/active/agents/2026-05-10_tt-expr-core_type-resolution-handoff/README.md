# Type Resolution Handoffs

- Date: 2026-05-10
- Task title: Type resolution handoff index
- Task description: Durable index for ongoing handoff documents while revisiting typed type resolution across parser, transform, database, RAG, and TUI layers.
- Related planning files: `docs/active/agents/readme.md`

This directory tracks restart context for the `tt-expr-core` type-resolution work. Add future handoff notes here when the branch state or next task boundary changes enough that a new restart summary would be useful.

## Handoff Docs

- [`2026-05-10_current-type-resolution-state.md`](2026-05-10_current-type-resolution-state.md)
  Starting-point summary for revisiting the current branch: recent commit sequence, active feature-gated v2 architecture, cross-crate data flow, known docs, current uncommitted state, and likely next work.
- [`2026-05-10_current-type-resolution-workflow.md`](2026-05-10_current-type-resolution-workflow.md)
  Current workflow for continuing typed graph work: strict tests first, document red buckets, implement one semantic surface at a time, and keep `KL-008` in sync.
- [`2026-05-17_corpus-type-shape-matrix-handoff.md`](2026-05-17_corpus-type-shape-matrix-handoff.md)
  Cold-restart summary for the corpus-backed TypeNode matrix, OpenRouter searchable fixtures, focused DB/RAG/TUI verification, full workspace verification, and manual ignored live OpenRouter matrix verification.
- [`type-graph-tightening-review/`](type-graph-tightening-review/)
  Multi-reviewer audit of recent typed graph work for lax owner/context typing, DB projection contracts, and test hardening tasks.
