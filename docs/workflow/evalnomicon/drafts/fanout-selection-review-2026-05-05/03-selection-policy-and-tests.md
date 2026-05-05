# Prototype 1 Fanout Selection Review: Selection Policy And Tests

Date: 2026-05-05

Scope: current uncommitted generation child fanout/join selection implementation.
Focus: selection policy semantics, scheduler/CLI behavior, continuation decisions,
`--node-id` override behavior, wrapper/typed path consistency, and test coverage.

## Findings

### 1. Child task errors abort the whole fanout wave before selection can use successful sibling evidence

Severity: high

In `run_child_fanout`, each wave launches child tasks with `JoinSet`, collects
successful `PlannedChildOutcome`s, but stores the first task error and returns it
after the wave joins:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5441`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5461`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5484`

That means one failed child can fail the parent turn even if another child in
the same fanout wave completed and would be accepted. This is not a selection
policy decision; it is an execution error short-circuiting parent authority
before the parent can evaluate joined child evidence.

Suggested fix: represent task failure as a child outcome/evidence value whenever
the child node has reached a meaningful terminal or failed state, then let the
parent selection step inspect all joined outcomes. Reserve parent-turn failure
for failures that invalidate the parent authority context or make the child plan
unusable. If `stop_on_error` should still exist here, thread it explicitly into
fanout semantics and test both modes.

Suggested tests:

- A wave with child A task failure and child B accepted should select child B
  when `stop_on_error=false` or equivalent policy.
- A wave with child A accepted and child B task failure should not discard the
  accepted child unless explicit strict policy requires abort.
- A wave with all child tasks failing should produce a recorded terminal
  continuation decision rather than silently losing the generation context.

### 2. Live parent fanout behavior is mostly untested at the parent-turn level

Severity: high

The current tests cover CLI parsing, scheduler continuation, and pure
`decide_generation` behavior:

- `crates/ploke-eval/src/cli.rs:12126`
- `crates/ploke-eval/src/cli.rs:12801`
- `crates/ploke-eval/src/intervention/scheduler.rs:1216`
- `crates/ploke-eval/src/intervention/scheduler.rs:1279`
- `crates/ploke-eval/src/successor_selection/mod.rs:166`
- `crates/ploke-eval/src/successor_selection/mod.rs:193`

I did not find a test that exercises the live parent path from
`resolve_child_plan` through `run_child_fanout`, `generation_selection`,
`decide_continuation_with_selection`, and successor handoff gating:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5162`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5411`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5937`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5962`

This leaves the core regression surface uncovered: whether `min_children` is
actually fanout width, whether `max_children` is actually total generation
budget, whether accepted children stop later waves, and whether rejected
fallback produces the right continuation decision.

Suggested fix: factor enough of the parent-turn fanout path behind testable
helpers, or add a fixture-backed integration test that uses prebuilt child
reports and avoids live LLM calls. The test should assert both outcomes and the
recorded scheduler decision.

Suggested tests:

- With `min=2`, `max=6`, and planned children `[reject, keep, keep]`, only the
  first wave runs and the selected child is the keep child from that wave.
- With `min=2`, `max=6`, and first wave `[reject, reject]`, second wave
  launches.
- With no accepted children, `decide_generation` selects the highest scoring
  rejected child and records `ContinueExploreFromRejected` only when
  `explore_from_rejected=true`.
- With `explore_from_rejected=false`, the same rejected fallback evidence
  records `StopSelectedBranchRejected` and does not spawn a successor.

### 3. The wrapper loop and typed parent path use similar but not identical selection policy

Severity: medium

The wrapper loop path evaluates branches sequentially, then calls
`select_most_promising_branch`:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:992`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1037`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6476`

That helper first chooses the highest-scoring `Keep`, otherwise the
highest-scoring branch overall. The typed path instead calls
`accepted_selection` first, then `successor_selection::decide_generation`:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5502`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5512`
- `crates/ploke-eval/src/successor_selection/mod.rs:33`

These are close, but not the same authority projection. The wrapper path does
not use `SelectionInput`, `SuccessorDecision`, or `selection_policy_outcome`;
instead it derives `Prototype1SelectionPolicyOutcome` from the rendered branch
disposition string:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1061`

This is not immediately wrong, but it creates two places where "accepted first,
otherwise best rejected exploration coordinate" can drift.

Suggested fix: route wrapper generation selection through the same
`successor_selection` evidence type when branch evaluation summaries have enough
data, or explicitly document the wrapper path as a legacy projection with a
compatibility test proving the two policies agree for representative inputs.

Suggested tests:

- A wrapper-policy test and a typed-policy test over the same synthetic child
  evidence should select the same keep branch.
- A no-keep case should select the same rejected fallback branch and produce the
  same continuation disposition when `explore_from_rejected` is toggled.

### 4. `min_children` functions as fanout width, but CLI help still describes it as a minimum evaluation count

Severity: medium

The implementation uses `min_children` as the fanout width:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5426`

The live path also truncates total planned children to `max_children`:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5923`

So the runtime behavior matches the intended fanout/budget model. However, the
CLI help says "Minimum direct child candidates to evaluate before
generation-level fallback selection":

- `crates/ploke-eval/src/cli.rs:908`

That text implies the parent will always evaluate at least `min_children` before
fallback. The current implementation instead treats `min_children` as concurrent
wave width, warns if fewer children are available, and may evaluate fewer than
`min_children` when the plan contains fewer nodes:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5427`

Suggested fix: update the help text to say "fanout width" or "initial concurrent
children per wave." Keep `max_children` as total generation budget.

Suggested tests:

- CLI parse or policy formatting should assert the name/description once the
  command surface stabilizes.
- A helper-level test should verify `min=3,max=5,nodes=5` runs in waves of 3
  then 2, while `min=3,max=5,nodes=2` runs 2 and emits/records the shortage
  behavior expected by policy.

### 5. Accepted-first semantics are plan-order-first after a joined wave, not completion-order-first

Severity: medium

`run_child_fanout` joins a whole wave, sorts completed outcomes by `plan_index`,
then `accepted_selection` returns the first accepted decision in that sorted
order:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5488`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5492`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5502`

This is a reasonable conservative policy because it avoids accepting a child
while sibling child runtimes are still unmanaged. But it should be explicit:
"first acceptable" currently means first by child plan order among joined
evidence, not first by wall-clock completion.

Suggested fix: document this in code or rename the helper to make the ordering
clear. If completion order matters later, `PlannedChildOutcome` needs a
completion timestamp or monotonic join index.

Suggested tests:

- Two accepted children in one wave select the lower `plan_index`.
- An accepted child in wave 1 prevents wave 2 from launching.

### 6. `--node-id` override sensibly disables fanout, but that behavior needs a regression test

Severity: low

When `--node-id` is present, `resolve_child_plan` validates that the selected
node is a direct child of the active parent and is included in the received plan,
then returns a one-node vector:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5194`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5212`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5220`

The later truncate is skipped when `self.node_id.is_some()`:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5923`

This behavior is sensible: explicit `--node-id` means "run this child," not
"start fanout from this child." But it should be covered because this is exactly
the kind of CLI/debug behavior that can regress during parent-turn refactors.

Suggested tests:

- `--node-id` with `min=2,max=6` runs exactly one planned child.
- `--node-id` for a sibling under another parent rejects before execution.
- `--node-id` for a node not present in the locked child plan rejects before
  execution.

### 7. Continuation decisions are computed against the selected child node, which is correct, but the no-selection path uses an inferred generation

Severity: low

When there is a selection decision, the implementation loads the selected child
node and passes `node.generation` into `decide_continuation_with_selection`:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5962`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5964`

That is the right generation coordinate for successor continuation. The
selected branch id and selected branch disposition also come from the
`SuccessorDecision`, not from a separately selected scheduler field:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5967`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:5968`

The no-selection path passes `parent_identity.generation.saturating_add(1)` as
the current generation:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6028`

Because `selected_next_branch_id` is `None`, this currently produces
`StopNoSelectedBranch` before generation limits matter. It is not a behavioral
bug today, but the name `current_generation` becomes misleading in this branch.

Suggested fix: either pass the last attempted child generation when available,
or keep the current behavior but add a comment that generation is irrelevant
when no branch is selected because `StopNoSelectedBranch` dominates.

## Open Questions And Assumptions

- I assume child fanout is intended to run child runtimes concurrently but keep
  successor authority entirely in the parent. The implementation mostly follows
  that: child tasks produce `PlannedChildOutcome`; only the parent records
  `SuccessorRecord`, records continuation, and calls
  `spawn_and_handoff_prototype1_successor`.
- I assume "first acceptable" means first by plan order after each wave joins.
  If it instead means first by completion time, the current implementation is
  not doing that.
- I assume `max_children` is total generation budget for one parent, not total
  planned nodes across the campaign. The typed parent path implements that
  locally by truncating child nodes to `child_budget.max`.
- I assume fewer available planned children than `min_children` should warn and
  continue. If the policy should require at least `min_children`, current
  behavior is too permissive.
- I did not validate filesystem-level concurrency safety of running multiple
  `MaterializeBranch`/`BuildChild`/`SpawnChild` transitions at once. This review
  only flags that task errors currently abort joined evidence; a separate
  runtime-safety review should check concurrent writes to scheduler/node files,
  child worktrees, build outputs, and the shared transition journal.

## Summary

The implementation does move the typed parent path from single-child selection
to a real fanout/join shape. `min_children` is used as wave fanout width,
`max_children` caps total children attempted for the parent generation, accepted
children are preferred before rejected fallback, and continuation/handoff remain
parent-owned.

The main correctness gap is that execution errors from one child can abort the
whole wave before selection can use successful sibling evidence. The main
coverage gap is that the live parent fanout path is not directly tested; current
tests cover the smaller pure functions and CLI parsing, not the joined runtime
policy. The wrapper loop path is close to the typed path but still uses a
separate selection projection, so it should either be unified through
`successor_selection` or pinned with compatibility tests.
