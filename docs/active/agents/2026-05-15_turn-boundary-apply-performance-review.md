# Turn-Boundary Apply Performance Review

Reviewed commit: `4af3e2fd` (`prototype1: apply broad harness edits after turn completion`)

Scope: `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs`, with nearby `ploke-tui` approval helpers for command semantics.

## Findings

### High: sequential staged apply can leave a partially mutated workspace on retry

`run_attempt` applies every staged item one at a time after `ChatTurnFinished` and returns `RetryFailure` immediately on the first apply error:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:590`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:591`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:596`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:602`

That means proposal 1 can be applied to disk, proposal 2 can fail or become stale, and the outer loop then starts the next isolated attempt from a workspace that is already modified:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:102`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:118`

This is the main risk introduced by moving apply to the turn boundary. It makes a multi-proposal turn behave like a partial transaction without rollback, a batch contract, or an explicit "partial success is terminal" state. It also means a later `Exhausted` or retry result can coexist with earlier applied edits in the same `HeadlessRun`, which is hard to reason about for History admission and smoke interpretation.

The nearby `ploke-tui` bulk edit path already acknowledges that multi-proposal apply needs a selection/overlap pass before application:

- `crates/ploke-tui/src/rag/editing.rs:651`
- `crates/ploke-tui/src/rag/editing.rs:670`
- `crates/ploke-tui/src/rag/editing.rs:674`
- `crates/ploke-tui/src/rag/editing.rs:702`

The adapter bypasses that shape and applies the staged vector directly. For the next slice, prefer one of these contracts:

- single candidate proposal per attempt, with the prompt/tool contract rejecting additional staged edits;
- a true batch candidate that validates all staged paths and overlap/conflict state before any disk mutation;
- explicit terminal evidence for partial apply, if partial mutation is intentionally allowed.

### Medium: apply waits add fixed per-proposal latency and clone full proposal payloads while polling

Both `apply_edit` and `apply_create` send one state command, then poll shared state every 100ms and clone the full proposal on every pass:

- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:635`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:637`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:640`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:647`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:715`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:717`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:720`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:727`

The lock scopes are short and no lock is held across the sleep, so I do not see a lock starvation issue. The inefficiency is the fixed sleep and the repeated `EditProposal` / `CreateProposal` clone. In a normal one-proposal smoke this is small, but the new multi-stage contract makes the cost linear in staged item count with at least 100ms of extra wall time per proposal.

The easiest improvement is a shared `await_terminal_status` helper that reads only status plus paths/reason into a small local enum. Better still, consume the approval completion event as the waiter signal and keep state polling as a fallback.

### Low: allocation/scanning overhead is currently acceptable but should stay bounded

The staging path uses linear scans and vector dedupe:

- duplicate detection with `staged.contains(...)` at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:325` and `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:392`
- cloning the staged vector at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:591`
- deduping changed paths with `changed_paths.contains(&path)` at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:607`
- path materialization through `proposal_paths` at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:1096`
- logging path joins through an intermediate `Vec<String>` at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs:2487`

These are not worth replacing with `HashSet`/`IndexSet` unless live runs show many staged items. The invariant to keep is that the adapter should stage only a small candidate set per turn. If that stops being true, this should move to a small staging accumulator that owns dedupe and path aggregation explicitly.

## Maintainability Notes

`run_attempt` is now doing four jobs: event observation, proposal staging, rejection policy, and post-turn apply orchestration. The useful split is not another generic helper name; it is a boundary around the staged candidate:

- `StageBook` or equivalent local carrier: records `Staged` items, handles duplicate detection, and owns changed-path aggregation.
- `stage_edit` / `stage_create`: validate paths, push run evidence, and return a typed staged item or retry feedback.
- `apply_staged_candidate`: owns the sequential/batch policy and returns either an applied terminal projection or retry/partial evidence.

That split would make the contract decision visible: single proposal, batch, or partial apply.

## Lock And Channel Behavior

The new code uses short read-lock scopes for proposal lookup and status polling. I did not find locks held across event waits or sleeps.

The approval completion events are sent on a broadcast bus, so not draining them inside `apply_edit` / `apply_create` should not block the sender. The risk is observability drift: the adapter returns as soon as it sees state status and may not record the approval-generated `ToolCallCompleted` event before dropping the runtime.

## Tests And Instrumentation To Add

- Unit/helper test for applying staged items where the first succeeds and the second fails: assert the intended terminal state and whether partial workspace mutation is allowed.
- Unit/helper test for overlapping same-file proposals staged in one turn: assert reject-before-apply, stale-before-apply, or explicit partial behavior.
- Lightweight live-smoke counters in observer output: `staged_count`, `applied_count`, `apply_wait_ms_total`, `apply_poll_count`, and `changed_path_count`.
- Keep the current bounded evidence tests; they cover output size but not the staging/apply contract.

## Next Live Smoke Invariants

- `staged_count` should normally be `1`; any higher count should be called out in smoke notes.
- `applied_count == staged_count` before returning `Applied`.
- No retry/exhausted terminal should follow an already-applied staged item unless partial apply is an intentional terminal class.
- `apply_wait_ms_total` should stay close to one status transition, not grow with repeated idle polling.
- Changed paths should be deduped and should all pass the broad edit surface policy before any apply command is sent.

## Verification

Ran:

```text
cargo test -p ploke-eval tui_adapter::tests 2>&1 | tail -n 40
```

Result: 14 passed, 7 ignored, 0 failed.
