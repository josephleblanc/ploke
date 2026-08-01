# Prototype 1 Eval Store — Live Typestate Transition Test Plan

Status: active planning note / implementation gate.

Related files:

- [`storage-plan.md`](storage-plan.md)
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md)
- [`relational-data-model.md`](relational-data-model.md)
- [`database-planning-notes.md`](database-planning-notes.md)
- [`open-questions.md`](open-questions.md)

## Decision

Before and after moving Domain-C persistence from filesystem-only to configurable `fs | database | dual-strict`, test the full current Prototype 1 typestate transition inventory in isolation.

Use live provider/API calls for transitions that normally call providers. Cost is explicitly accepted because live validity and operator confidence are more important than minimizing API spend for this migration.

This does not mean every transition must call an API. Pure local transitions should stay local. It means provider-facing transitions must not be replaced by mocks/stubs in the confidence suite.

## Scope

The tested inventory is the current live typestate transition/outcome set, derived from source rather than hand-maintained prose:

- `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
- `crates/ploke-eval/src/cli/prototype1_state/typestate/aliases.rs`
- the `loop walk` phase/step surfaces that expose the same live edges

Treat the current source-derived transition/outcome inventory as frozen for this migration. Add an inventory-count assertion generated from source so a future transition split/merge fails the suite until the test matrix is updated intentionally. Do not hard-code the count in prose.

## Test modes

### Baseline filesystem mode

Run the isolated transition suite before adding storage functionality.

Purpose:

- prove current live behavior;
- record expected files/evidence for each edge;
- catch accidental authority or persistence changes during refactor.

### Filesystem mode after abstraction

After introducing `EvalStore`, `backend = fs` must preserve current on-disk behavior.

Assertions:

- same transition result/state branch;
- same authority gates;
- same required files and JSON schemas;
- no DB dependency for `fs` mode.

### Dual-strict mode

After `DbEvalStore` exists for a slice, run the same transition tests with `backend = dual-strict`.

Assertions:

- filesystem writes still happen;
- DB rows are written for the slice under test;
- row hashes/semantic envelopes match the filesystem evidence;
- DB write failure or semantic mismatch fails loudly;
- ordinary DB rows do not become History, Channel, MessageBox, or artifact authority.

### Database-only mode

Defer database-only transition tests until the relevant production read paths have migrated or been explicitly classified as compatibility reads.

Do not claim DB-only readiness while active reads still depend on files such as `scheduler.json`, latest `runner-result.json`, `branches.json`, `node.json`, channel JSONL, message-box files, or sealed History blocks.

## Live API policy

Live API tests should be explicit and reproducible, not accidental CI defaults.

Requirements:

- use an explicit ignored test target and suite-specific environment opt-in; do not rely on the `live_api_tests` feature alone, because it may be enabled by default in this crate;
- require provider credentials plus a suite-specific variable such as `PLOKE_EVAL_LIVE_API_TESTS=1`;
- document required provider/model variables near the tests;
- persist run artifacts and profile commitments for review;
- run serially or with bounded parallelism so failures are attributable;
- do not pass tests on placeholder “not implemented” or provider-skipped branches.

Provider-facing transitions should use the same production adapter path as the loop. Test validity comes from exercising real model/provider behavior, real tool outputs, and real persisted evidence.

## Persistence dependency coverage

Use [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md) to choose downstream tests when a storage writer changes. The goal is not to run a 30–35 minute full loop after every narrow change; the goal is to prove every persisted producer/consumer contract that the full loop depends on.

For provider-facing transitions, run the producer transition with live APIs. Then checkpoint the resulting persisted evidence and reuse it for downstream consumer tests that do not themselves call providers.

## Isolation rule

Each transition test should advance exactly one live edge or one branch outcome of a live edge.

For each case:

1. Build or restore the minimal admitted pre-state.
2. Assert the expected preconditions before the transition.
3. Execute only the target transition.
4. Assert the output typestate/phase/branch.
5. Assert required authority/evidence surfaces.
6. Assert no downstream transition was silently advanced.
7. Preserve artifacts/logs needed for failure review.

Where direct edge construction is cheaper and faithful, test the edge function. Where the CLI/walk surface is the only faithful setup, drive `loop walk step --until <phase>` but assert that only the intended edge advanced.

## Per-transition assertion template

For each transition/outcome:

- **Input state:** phase/state alias, parent/runtime/campaign ids, admitted profile, artifact identity, and any required prior evidence.
- **Live dependencies:** provider/model required or none; filesystem/git requirements; channel/message-box/history requirements.
- **Writes in `fs`:** exact files or journal/channel/message-box/log refs expected.
- **Rows in `dual-strict`:** relation(s), stable ids, common scope/source/evidence axes, hashes.
- **Authority guardrail:** which authority surface remains decisive.
- **Failure check:** one negative/precondition case where practical.

## Storage migration gates

Do not move from one storage slice to the next until:

1. baseline `fs` transition tests pass for the affected producer transitions;
2. downstream consumer tests from the persistence dependency matrix pass from restored checkpoints;
3. post-abstraction `fs` tests still pass;
4. `dual-strict` tests prove DB parity for the slice;
5. authority-negative checks prove DB rows did not replace History/Channel/MessageBox/artifact gates;
6. run artifacts are reviewed when live providers were involved.

## Suggested implementation order

1. Add source-derived transition inventory and inventory-count assertion.
2. Add/collect baseline live `fs` tests for all current transition outcomes.
3. Add test harness support for running one transition in a fresh or restored campaign fixture.
4. Add checkpoint fixtures from the persistence dependency matrix so downstream consumers can be tested without rerunning provider-facing producers.
5. Add `EvalStore` scaffolding with `backend = fs` only.
6. Re-run the full live transition suite in `fs` mode.
7. Add the first DB slice and `dual-strict`.
8. Re-run affected producer transitions and downstream consumer contracts in `dual-strict`.
9. Expand DB slices transition-by-transition, never broadening authority scope as part of persistence migration.

## Non-goals

- Do not replace sealed History tests with eval-row checks.
- Do not replace channel transport tests with channel-message mirror rows.
- Do not replace MessageBox lock/unlock tests with payload mirror rows.
- Do not use mocks/stubs for provider-facing confidence tests in this migration suite.
- Do not use DB-only mode as a success criterion until read-path parity exists.
