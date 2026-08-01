# Orchestration Workflow

Use `target/debug/xtask orchestrate` for live state. Markdown is the durable
packet, not the live queue.

## Current Board Posture

The current `.orchestrator/board.json` is mixed. It includes broad-harness and
older egui waves, plus active reviewer assignments and unresolved blockers.

Because the user has not yet approved a board reset:

- do not archive/remove `.orchestrator/` yet,
- use isolated `ui-*` lane ids and worker ids for this thread if work starts on
  the current board,
- prefer a clean board wave only after explicit approval.

## Bounded Status

Use this summary projection for routine checks:

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

Do not dump the full board JSON into chat or docs.

## Recommended Setup On The Current Board

If the board is not reset, add only isolated `ui-*` lanes:

```bash
target/debug/xtask orchestrate lane set ui-retainer \
  --doc docs/active/agents/ploke-ui-task-readability/lanes.md

target/debug/xtask orchestrate lane set ui-inventory \
  --own docs/active/agents/ploke-ui-task-readability/inventory \
  --own docs/active/agents/2026-05-11_ploke-tree-graph-ingestion-inventory.md \
  --doc docs/active/agents/ploke-ui-task-readability/inventory/README.md

target/debug/xtask orchestrate lane set ui-projection \
  --own crates/ploke-egui/src/ui/view/projection.rs \
  --own crates/ploke-egui/src/import/mod.rs \
  --own crates/ploke-egui/src/import/tests.rs \
  --doc docs/active/agents/ploke-ui-task-readability/lanes.md

target/debug/xtask orchestrate lane set ui-diagnostics \
  --own crates/ploke-egui/src/ui/view/diagnostics.rs \
  --own crates/ploke-egui/src/ui/view/geometry.rs \
  --own crates/ploke-egui/src/ui/view/label.rs \
  --doc docs/active/agents/ploke-ui-task-readability/lanes.md

target/debug/xtask orchestrate lane set ui-layout \
  --own crates/ploke-egui/src/ui/view/layout.rs \
  --own crates/ploke-egui/src/ui/view/style.rs \
  --own crates/ploke-egui/src/ui/view/edge.rs \
  --doc docs/active/agents/ploke-ui-task-readability/lanes.md

target/debug/xtask orchestrate lane set ui-inspector-docs \
  --own crates/ploke-egui/src/diagnostics/mod.rs \
  --own crates/ploke-egui/src/ui/app/mod.rs \
  --own crates/ploke-egui/src/native.rs \
  --own docs/active/agents/ploke-ui-task-readability \
  --doc docs/active/agents/ploke-ui-task-readability/lanes.md

target/debug/xtask orchestrate lane set ui-review \
  --doc docs/active/agents/ploke-ui-task-readability/reviews/boundary-audit-checklist.md

target/debug/xtask orchestrate lane validate
```

Open `ui-graph-gap` only when a worker proves a missing typed relation or
loader:

```bash
target/debug/xtask orchestrate lane set ui-graph-gap \
  --own crates/ploke-tree/src/graph \
  --own crates/ploke-tree/src/store \
  --own crates/ploke-records/src/selection.rs \
  --own crates/ploke-records/src/evaluation.rs \
  --doc docs/active/agents/ploke-ui-task-readability/inventory/README.md
```

## Recommended Setup After Approved Reset

The current wave is already a clean `ui-*` board. The previous mixed board was
archived at `.orchestrator-archive/2026-05-12-pre-ui-wave`.

If a later wave needs another reset:

1. archive or remove `.orchestrator/`,
2. run `target/debug/xtask orchestrate init`,
3. recreate only the `ui-*` lanes and workers for this thread,
4. keep the first status report compact.

Do not reset the board casually. The helper has no built-in archive flow yet.

## Worker Packet Rule

Every worker gets:

- a generated board packet,
- [`packets/shared-brief.md`](packets/shared-brief.md),
- [`lanes.md`](lanes.md) for lane ownership and shared-pressure surfaces,
- [`orchestration.md`](orchestration.md) when reviewing board or packet setup,
- the relevant lane prompt from [`packets/templates.md`](packets/templates.md),
- exact allow/forbid surfaces from the board task.

Workers do not mutate the board. The main thread owns `add`, `assign`,
`packet`, `complete`, `review`, `block`, and `lane validate`.

## Dispatch Order

1. Retainer answers exact location and clone-risk questions.
2. Inventory worker refreshes the stale coverage claims for the first UI wave.
3. Projection worker lands borrowed readability-facing handles.
4. Diagnostics worker measures failures and adds tests.
5. Inspector/docs worker persists accepted render-only diagnostics; governance
   docs stay in `ui-governance-docs` and are changed by the main thread or an
   explicitly assigned governance task.
6. Reviewer audits the slice.
7. Layout worker tunes only after review accepts the measured failure.

If diagnostics fields or shared view payload types change, serialize the
reviewed follow-up through the shared-pressure files instead of letting multiple
writers converge there at once.

Do not idle if an independent lane can move.

## Completion Reports

Each worker report must include:

- files changed,
- exact line ranges touched,
- commands run,
- blockers,
- whether any UI type could be mistaken for semantic authority,
- whether any semantic id or record was copied out of `Graph`,
- successor note if the lane is not fully closed.

The main thread verifies narrow claims, marks completion, then marks review only
after acceptance.
