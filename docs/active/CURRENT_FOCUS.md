# Current Focus

**Last Updated:** 2026-05-15

**Active planning surfaces:**

- Prototype 1 live broad-harness/context-building wave: [`agents/2026-05-15_hyperagents-context-building-handoff.md`](agents/2026-05-15_hyperagents-context-building-handoff.md), with background from [`agents/2026-05-08_bounded-edit-surface-implementation-orientation.md`](agents/2026-05-08_bounded-edit-surface-implementation-orientation.md) and [`agents/2026-05-12_hyperagents-broad-harness-orchestration-handoff.md`](agents/2026-05-12_hyperagents-broad-harness-orchestration-handoff.md) — current focus is landing a multi-generation broad-harness smoke campaign after disabling automatic initial source-code RAG and moving headless TUI apply to the `ChatTurnFinished` boundary.
- Self-improvement loop track index: [`plans/self-improvement-loop/handoffs.md`](plans/self-improvement-loop/handoffs.md) — current routing table for Prototype 1 records/playback, egui observability, and MBE/oracle calibration.
- Frontend observability: [`agents/ploke-ui-task-readability/README.md`](agents/ploke-ui-task-readability/README.md), especially [`artifact-tree-default/`](agents/ploke-ui-task-readability/artifact-tree-default/README.md) — current `ploke-egui` artifact-tree default-view source of truth and follow-up task area.
- MBE/oracle calibration: [`agents/2026-05-11_mbe-oracle-calibration-handoff.md`](agents/2026-05-11_mbe-oracle-calibration-handoff.md) — current questions around compile-failed loop candidates, gold/empty MBE controls, and benchmark-base patch export.

**Archived context (trajectory, not “active” paths):** Topical agent markdown that used to live beside this file was moved to [`docs/archive/agents/2026-04/`](../archive/agents/2026-04/README.md) and [`docs/archive/agents/2026-05/`](../archive/agents/2026-05/README.md) on 2026-05-08. Examples: eval/protocol closure sketch and control-plane audits in `2026-04/`, edit-surface / Prototype 1 note stack in `2026-05/`. The 2026-04-17 eval/protocol baseline narrative is still useful background: [Eval closure formal sketch](../archive/agents/2026-04/2026-04-16_eval-closure-formal-sketch.md), [protocol design reset](workflow/handoffs/2026-04-17_protocol-design-reset.md).

---

## What we're doing now

The primary active thread is **Prototype 1 broad-harness multi-generation proof**:

1. Keep broad HyperAgents-style edit permission; do not regress to deterministic target-file routing.
2. Use `ploke-tui`'s existing `context_management.mode = Off` for broad headless TUI attempts so the initial prompt has no automatic source-code RAG snippets.
3. Keep BM25/RAG services available so agents can request code intentionally through tools such as `request_code_context`.
4. Let the model finish its tool-call chain before candidate apply: staged edit/create proposals are recorded during tool completion and applied only after `ChatTurnFinished(outcome="completed")`.
5. Prove the loop can repeatedly publish children, apply/reject edits, select a successor, hydrate the next parent, and continue across generations.

Two secondary threads remain relevant but are not the current implementation focus:

1. **Frontend observability:** build the egui-facing semantic graph/playback surface for loop runs.
2. **MBE/oracle calibration:** make Multi-SWE-bench evidence trustworthy enough to integrate into loop evaluation and then surface in the frontend.

They are related through the self-improvement-loop track, but the implementation work is disjoint. Pick the thread that matches the branch or user request; do not assume older Prototype 1 or protocol docs are current unless the handoff index points to them.

---

## Immediate next step

1. Commit the current broad headless TUI adapter patch so the next Runtime/Artifact identity includes both context-mode Off and turn-boundary apply.
2. Rebuild `ploke-eval` in the active parent worktree before launching the next live loop.
3. Run one clean live splice to confirm prompt diagnostics show `context_mode = Off`, `included_rag_parts = 0`, ready BM25, explicit source inspection through tools, and no candidate apply before `ChatTurnFinished`.
4. Set up the next smoke campaign from a fresh worktree under `~/.ploke-eval/worktrees`, using the 2-generation/3-child broad-harness profile before attempting a larger 3x4 or 10-generation run.

---

## Quick links

| Ask about… | Open |
|------------|------|
| “What were we up to?” | This doc and [`workflow/handoffs/recent-activity.md`](workflow/handoffs/recent-activity.md) |
| Eval / protocol history (Apr 2026) | [Eval closure sketch](../archive/agents/2026-04/2026-04-16_eval-closure-formal-sketch.md), [failure/protocol audit dir](../archive/agents/2026-04/2026-04-17_eval-failure-and-protocol-audit/README.md), [design reset](workflow/handoffs/2026-04-17_protocol-design-reset.md) |
| Edit surface / Prototype 1 notes (May 2026) | [`docs/archive/agents/2026-05/`](../archive/agents/2026-05/README.md) |
| Self-improvement loop tracks | [`plans/self-improvement-loop/handoffs.md`](plans/self-improvement-loop/handoffs.md) |
| MBE / oracle calibration | [`agents/2026-05-11_mbe-oracle-calibration-handoff.md`](agents/2026-05-11_mbe-oracle-calibration-handoff.md) |
| Frontend observability | [`agents/ploke-ui-task-readability/README.md`](agents/ploke-ui-task-readability/README.md), [`artifact-tree-default`](agents/ploke-ui-task-readability/artifact-tree-default/README.md) |
| Target / run policy | [`workflow/target-capability-registry.md`](workflow/target-capability-registry.md) |
| Agent doc index | [`agents/readme.md`](agents/readme.md) |

---

## Update instructions (for agents)

When this doc changes: bump **Last Updated**, keep **Active planning surfaces** aligned with the real “read this next” artifact (Cursor plan path or `HANDOFF.md`), and prefer **archive links** for superseded topical notes instead of recreating `docs/active/agents/` clutter.
