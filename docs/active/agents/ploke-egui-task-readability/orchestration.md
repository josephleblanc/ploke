# Orchestration Workflow

Use `target/debug/xtask orchestrate` for live state. Do not use this directory
as the live queue.

## Current Board Caveat

As of this plan, the existing board is not clean. It contains older graph and
edit-surface lanes, several blocked tasks, many completed-unreviewed tasks, and
one active reviewer.

Before starting a new implementation wave, the main thread must choose one of:

1. add isolated `egui-*` lanes and tasks to the current board, or
2. archive/remove `.orchestrator` and initialize a clean wave.

The board is an agent-local coordination tool. The main thread may mutate,
archive, or replace it at its discretion, but should leave a compact note in
the final/handoff when doing so.

## Bounded Status

Use this projection for routine status:

```bash
target/debug/xtask orchestrate --format json status | jq '{
  board,
  blockers: (.blockers | length),
  lanes: [.lanes[] | {id, owned_edit}],
  workers: [.workers[] | {id, role, active}],
  tasks_by_state: (.tasks | group_by(.state.state) | map({
    state: .[0].state.state,
    count: length
  }))
}'
```

Do not dump the full board JSON into chat or docs during routine check-ins.

## New Wave Setup

Recommended commands for an isolated egui lane set:

```bash
target/debug/xtask orchestrate lane set egui-readability-retainer \
  --doc docs/active/agents/ploke-egui-task-readability/lanes.md

target/debug/xtask orchestrate lane set egui-projection \
  --own crates/ploke-egui/src/ui/view/projection.rs \
  --doc docs/active/agents/ploke-egui-task-readability/lanes.md

target/debug/xtask orchestrate lane set egui-diagnostics \
  --own crates/ploke-egui/src/ui/view/diagnostics.rs \
  --own crates/ploke-egui/src/ui/view/label.rs \
  --own crates/ploke-egui/src/ui/view/geometry.rs \
  --doc docs/active/agents/ploke-egui-task-readability/lanes.md

target/debug/xtask orchestrate lane set egui-layout \
  --own crates/ploke-egui/src/ui/view/layout.rs \
  --own crates/ploke-egui/src/ui/view/style.rs \
  --doc docs/active/agents/ploke-egui-task-readability/lanes.md

target/debug/xtask orchestrate lane set egui-snapshot-docs \
  --own crates/ploke-egui/src/diagnostics/mod.rs \
  --own crates/ploke-egui/src/ui/app/mod.rs \
  --own docs/active/agents/ploke-egui-task-readability \
  --doc docs/active/agents/ploke-egui-task-readability/lanes.md

target/debug/xtask orchestrate lane set egui-review \
  --doc docs/active/agents/ploke-egui-task-readability/reviews/boundary-audit-checklist.md

target/debug/xtask orchestrate lane validate
```

Add `egui-graph-gap` only if a worker proves a missing semantic relation:

```bash
target/debug/xtask orchestrate lane set egui-graph-gap \
  --own crates/ploke-tree/src/graph \
  --own docs/active/agents/2026-05-11_ploke-tree-graph-ingestion-inventory.md \
  --doc docs/active/agents/ploke-egui-task-readability/inventory/README.md
```

## Worker Packet Rule

Every worker gets a generated board packet plus:

- [`packets/shared-brief.md`](packets/shared-brief.md),
- the lane-specific prompt from [`packets/templates.md`](packets/templates.md),
- exact allowed/forbidden edit surfaces from the board task.

Workers do not run board commands. The main thread owns `add`, `assign`,
`complete`, `review`, `block`, `packet`, and `lane validate`.

## Dispatch Order

1. Retainer locates exact code ranges, stale inventory risks, and any existing
   semantic clones in the active egui path.
2. Projection worker implements borrowed selected-lineage/rank handles without
   owned semantic ids or wrapper reports.
3. Diagnostics worker adds rank/path metrics and tests.
4. Snapshot/docs worker persists snapshots and summary text.
5. Reviewer audits boundary discipline before layout tuning.
6. Layout worker tunes geometry using failing diagnostics.

Do not wait idly if an independent docs/inventory or retainer task can move.

## Completion Reports

Each worker report must include:

- files changed,
- exact line ranges touched,
- tests or commands run,
- blockers or semantic gaps,
- whether any egui type could be mistaken for semantic authority,
- whether any allocated value was cloned out of `ploke_tree::Graph`,
- handoff note for a successor if the task is not complete.

The main thread verifies only narrow claims, then marks completion/review in the
board.

## Refresh Policy

Refresh or close/respawn a sub-agent when:

- it has answered roughly five bounded questions,
- it has carried context from a superseded plan,
- its report relies on stale board state,
- it crosses from read-only retainer work into implementation,
- it starts proposing semantic carriers, copied ids, copied refs, wrapper
  reports, or mirror types in `ploke-egui`.

Before closing a useful but stale worker, ask it for a compact handoff using
[`handoffs/handoff-template.md`](handoffs/handoff-template.md). Store durable
successor context under `handoffs/` only when it adds information not already
captured in the board or plan.
