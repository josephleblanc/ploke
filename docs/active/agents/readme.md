# Active Agent Docs

**Thread (2026-05-08):** Day-to-day topical notes that used to accumulate here now live under [`docs/archive/agents/2026-04/`](../../archive/agents/2026-04/README.md) and [`docs/archive/agents/2026-05/`](../../archive/agents/2026-05/README.md). This directory keeps only the shared **restart spine** plus living indexes.

If you read three things after a long break: [`CURRENT_FOCUS.md`](../CURRENT_FOCUS.md), [`workflow/handoffs/recent-activity.md`](../workflow/handoffs/recent-activity.md), and the in-repo Cursor plan for your current implementation thread (e.g. `.cursor/plans/prototype1_history_admitted_evaluation_*.plan.md`).

## Start here

- [`CURRENT_FOCUS.md`](../CURRENT_FOCUS.md) — primary restart pointer (code + workflow thread).
- [`workflow/handoffs/recent-activity.md`](../workflow/handoffs/recent-activity.md) — rolling activity log (links into archive where needed).
- [`workflow/handoffs/2026-04-17_protocol-design-reset.md`](../workflow/handoffs/2026-04-17_protocol-design-reset.md) — compact eval/protocol design pivot (historical baseline; follow-on work is often in archive + `ploke-eval` inner handoff).

## Archived topical material (trajectory)

- **April 2026 (eval infra, protocol checkpoints, History/Crown review trees, phase-1 audit):** [`docs/archive/agents/2026-04/`](../../archive/agents/2026-04/README.md) — includes e.g. [`2026-04-12_eval-infra-sprint/`](../../archive/agents/2026-04/2026-04-12_eval-infra-sprint/README.md), [`2026-04-17_eval-failure-and-protocol-audit/`](../../archive/agents/2026-04/2026-04-17_eval-failure-and-protocol-audit/README.md), and `history-*-review-*` / `prototype1-architecture-gap-audit-*` folders.
- **May 2026 (edit surface + Prototype 1 handoffs/reviews):** [`docs/archive/agents/2026-05/`](../../archive/agents/2026-05/README.md) — `2026-05-07-*` reviews, handoffs, and related History startup reviews.

The former `2026-05-06-prototype1-*` execution-surface inventory links that appeared in older TOCs are **not present in this tree**; use archive + git history if you need that exact filename set.

## Still in this folder

- [`2026-05-08_bounded-edit-surface-handoff.md`](2026-05-08_bounded-edit-surface-handoff.md) — restart spine for the current bounded edit-surface / parent-planning thread, including authoritative docs, blocker status, known code audits, and next questions.
- [`2026-05-08_bounded-edit-surface-implementation-orientation.md`](2026-05-08_bounded-edit-surface-implementation-orientation.md) — short operational packet for sub-agents implementing the bounded edit-surface plan, with core docs, invariants, task-stack ids, and slice prompt pattern.
- [`open-questions.md`](open-questions.md) — agent-to-agent questions (not direct user prompts).
- [`notable-inconsistencies.md`](notable-inconsistencies.md) — durable inconsistencies worth tracking.

## Conventions

- When creating a **new** topical agent doc, prefer a dated name (`yyyy-mm-dd_…`) under `docs/active/agents/` only if it is genuinely part of the current restart spine; otherwise add it under the appropriate `docs/archive/agents/YYYY-MM/` bucket as work completes.
- Headers should include date, title, short description, and related planning files (with archive-resolved paths once moved).
- Workspace rule set: [`AGENTS.md`](../../../AGENTS.md).
