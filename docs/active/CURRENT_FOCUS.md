# Current Focus

**Last Updated:** 2026-05-08

**Active planning surfaces:**

- In-repo Cursor plan: [`.cursor/plans/prototype1_history_admitted_evaluation_f5b1343c.plan.md`](../.cursor/plans/prototype1_history_admitted_evaluation_f5b1343c.plan.md) — Prototype 1 successor selection: History-sealed admitted evaluation / commitments (see completed todos there; follow-ups may be new plan items).
- `ploke-eval` inner rewrite cold-start: [`crates/ploke-eval/src/inner/HANDOFF.md`](../../crates/ploke-eval/src/inner/HANDOFF.md) (`RunIntent → FrozenRunSpec → RunRegistration`; next slice `RunRegistration → CheckedOutWorkspace`).

**Archived context (trajectory, not “active” paths):** Topical agent markdown that used to live beside this file was moved to [`docs/archive/agents/2026-04/`](../archive/agents/2026-04/README.md) and [`docs/archive/agents/2026-05/`](../archive/agents/2026-05/README.md) on 2026-05-08. Examples: eval/protocol closure sketch and control-plane audits in `2026-04/`, edit-surface / Prototype 1 note stack in `2026-05/`. The 2026-04-17 eval/protocol baseline narrative is still useful background: [Eval closure formal sketch](../archive/agents/2026-04/2026-04-16_eval-closure-formal-sketch.md), [protocol design reset](workflow/handoffs/2026-04-17_protocol-design-reset.md).

---

## What we're doing now

Two parallel threads are documented above: **(1)** sealing successor-selection evidence into Prototype 1 History blocks per the Cursor plan, and **(2)** the `ploke-eval` inner run-registration workspace handoff. Pick the thread that matches the branch or user request; do not assume the older “protocol frontier walking” doc set is still under `docs/active/agents/`.

---

## Immediate next step

1. For Prototype 1 / History: resume from the Cursor plan and [`crates/ploke-eval/src/cli/prototype1_state/history.rs`](../../crates/ploke-eval/src/cli/prototype1_state/history.rs); use archive links only for background (e.g. edit-surface phases under `docs/archive/agents/2026-05/`).
2. For inner eval: follow [`crates/ploke-eval/src/inner/HANDOFF.md`](../../crates/ploke-eval/src/inner/HANDOFF.md) verification (`cargo test -p ploke-eval inner::`) before the next slice.

---

## Quick links

| Ask about… | Open |
|------------|------|
| “What were we up to?” | This doc and [`workflow/handoffs/recent-activity.md`](workflow/handoffs/recent-activity.md) |
| Eval / protocol history (Apr 2026) | [Eval closure sketch](../archive/agents/2026-04/2026-04-16_eval-closure-formal-sketch.md), [failure/protocol audit dir](../archive/agents/2026-04/2026-04-17_eval-failure-and-protocol-audit/README.md), [design reset](workflow/handoffs/2026-04-17_protocol-design-reset.md) |
| Edit surface / Prototype 1 notes (May 2026) | [`docs/archive/agents/2026-05/`](../archive/agents/2026-05/README.md) |
| Target / run policy | [`workflow/target-capability-registry.md`](workflow/target-capability-registry.md) |
| Agent doc index | [`agents/readme.md`](agents/readme.md) |

---

## Update instructions (for agents)

When this doc changes: bump **Last Updated**, keep **Active planning surfaces** aligned with the real “read this next” artifact (Cursor plan path or `HANDOFF.md`), and prefer **archive links** for superseded topical notes instead of recreating `docs/active/agents/` clutter.
