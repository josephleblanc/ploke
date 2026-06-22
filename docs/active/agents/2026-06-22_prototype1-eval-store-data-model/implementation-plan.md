# Prototype 1 Eval Store — File/DB/Dual-Strict Implementation Plan

Status: active implementation-prep plan; no storage code should move until Phase 0 gates pass.

Related files:

- [`storage-plan.md`](storage-plan.md) — authority boundaries, backend model, config sketch, and migration boundary guardrails.
- [`persistence-port-map.md`](persistence-port-map.md) — broader port/trait map for all persisted surfaces so implementation does not collapse everything into `EvalStore`.
- [`typestate-persistence-ledger.md`](typestate-persistence-ledger.md) — source-checked transition ledger and hard gates.
- [`live-transition-test-plan.md`](live-transition-test-plan.md) — isolated transition confidence suite, including live provider/API policy.
- [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md) — producer/consumer persisted-data matrix and checkpoint ladder.
- [`database-planning-notes.md`](database-planning-notes.md) — physical slice planning, key strategy, validation requirements.
- [`relational-data-model.md`](relational-data-model.md) — canonical logical relation model and common axes.
- [`record-usefulness-triage.md`](record-usefulness-triage.md) — compatibility/projection cleanup boundaries.

## Readiness call

We are ready to start **implementation preparation**: source-derived transition inventory, checkpoint harness, and baseline filesystem tests.

We are not yet ready to move broad production writers to DB-backed storage. The remaining prep is small but important:

1. freeze the exact live transition/outcome inventory;
2. build the per-transition fixture/checkpoint matrix;
3. define exact first-slice DDL and deterministic ids/hashes for the fixed `R4c -> R5` ParentStarted/resource evidence slice;
4. define dual-strict comparison/failure semantics.

After those are written down and baseline `fs` tests exist, implementation can proceed one narrow slice at a time.

## Non-negotiables

- Default remains `backend = fs` until parity is proven.
- `database` mode is experimental until production read paths have parity or explicit compatibility handling.
- `dual-strict` must fail loudly on DB write failure or semantic mismatch.
- DB rows do not replace History, Channel, MessageBox, bootstrap, journal, profile/config, or artifact/backend authority unless that domain gets its own explicit port/backing implementation.
- Provider-facing confidence tests use live API/provider calls.
- Full live loops are canaries, not the inner development loop; downstream consumers should run from checkpoints.

## Phase 0 — Test inventory, port map, and checkpoint harness, no storage behavior changes

### Implement / prepare

- Confirm the broader persistence port map in [`persistence-port-map.md`](persistence-port-map.md) against current code before moving writers.
- Source-derived transition/outcome inventory from:
  - `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/typestate/aliases.rs`
  - `loop walk` phase/step surfaces when needed for faithful execution.
- Inventory-count assertion generated from the current source-derived transition/outcome inventory; do not hard-code the count in prose.
- Per-transition test matrix using [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md).
- Checkpoint restore/copy/hash-verification strategy for:
  - campaign tree;
  - active checkout/artifact state;
  - child node dirs;
  - treatment run artifacts;
  - logs/streams/channel files;
  - DB snapshots where present.

### Tests to write/run

- Inventory unit test: source-derived inventory matches expected transition/outcome count.
- Checkpoint round-trip test: restore a checkpoint and verify declared files/hashes/ids.
- One local no-provider transition isolation test to prove the harness can execute exactly one edge.
- One negative authority fixture test, e.g. DB/projection present but MessageBox/channel/History artifact invalid still fails.

### Exit criteria

- We can name every transition/outcome under test.
- Every persisted surface touched by the first slice is assigned to a domain port or marked explicit compatibility/projection evidence.
- We can restore at least one checkpoint and run one isolated consumer transition from it.
- The test harness can distinguish “producer transition failed” from “downstream consumer contract failed.”

## Phase 1 — Baseline filesystem confidence suite

### Implement / prepare

- Baseline isolated `fs` tests for all transition outcomes.
- Live API opt-in wiring for provider-facing transitions.
- Artifact retention path for failed live runs/checkpoints.

### Tests to write/run

- Full isolated transition suite in `fs` mode.
- Live provider tests for provider-facing producers only.
- Downstream consumer tests from checkpoints for non-provider transitions.
- One full live loop canary after the isolated suite is stable.

### Exit criteria

- Current filesystem-only behavior is captured before abstraction.
- Required persisted producer/consumer contracts are known and test-covered.
- We have at least one reviewed live checkpoint for provider-dependent evidence.

## Phase 2 — Config shape only

### Implement

Add storage backend selection to the admitted run profile without changing runtime behavior.

Likely shape:

```toml
[storage.eval]
backend = "fs" # fs | database | dual-strict
```

Rust shape should extend the existing `Storage` profile struct without disturbing `worktree_root` defaults.

### Tests to write/run

- Profile parse/default test: omitted storage eval config defaults to `fs`.
- Profile validation test: unknown backend is rejected.
- Profile commitment test: storage config is included or intentionally excluded with documented reasoning.
- Baseline no-provider transition smoke in `fs` mode.

### Exit criteria

- Existing profiles still parse.
- Default behavior remains filesystem-only.
- Config is available to later store construction.

## Phase 3 — EvalStore scaffolding with filesystem backend only

### Implement

- Narrow first-slice `EvalStore` trait; do not implement the full future trait up front.
- `ConfiguredEvalStore::Fs` wrapper.
- `FsEvalStore` that delegates to current file writers and preserves bytes/schemas/paths.
- Store construction from admitted profile/config.
- Test-only recording store if useful for assertions.

First-slice method decision:

- Add the narrowest method needed for parent-owned `R4c -> R5` ParentStarted/resource evidence.
- Candidate names are `put_parent_started(...)` or `put_transition_event(...)`; choose the name from the final first-slice envelope shape, not from a future generic store API.
- Do not start with `JsonRecordFile::emit`: the generic emitter is broader than needed for proving config, receipts, and dual-strict behavior.

### Tests to write/run

- `FsEvalStore` writes the same file bytes/paths as the old writer for the chosen slice.
- Store construction chooses `Fs` by default.
- Affected transition producer test in `fs` mode.
- Downstream consumer tests from the matrix for surfaces touched by the slice.

### Exit criteria

- One writer is behind `EvalStore` with no behavior change.
- No DB dependency exists in `fs` mode.
- Authority-negative tests still pass.

## Phase 4 — First DB schema slice

### Implement

Define exact DDL for the first DB slice only. The first production writer is `R4c -> R5` ParentStarted/resource evidence, so the minimal relation set should be chosen from:

```text
eval_runtime              -- only if the writer can name a concrete runtime cheaply
eval_transition_event     -- parent-start semantic transition/event fact
eval_trace_event          -- only for structured observe/trace fields captured in the same slice
eval_record_ref           -- only for source/ref/hash backpointer to existing filesystem evidence
```

`eval_log_ref` belongs in the next trace/log/ref slice unless the first writer creates or imports a concrete log ref.

Do not add code-graph overlay relations in this phase.

For each relation define:

- key columns;
- value columns;
- schema version / relation description;
- deterministic event/ref id scheme;
- payload hash scheme;
- `store_scope`, `producer_role`, `visibility_scope`, `source_class`, `evidence_class`, and `validation_status` fields needed by this slice;
- evidence/source class fields;
- idempotent insert behavior.

### Tests to write/run

- Schema install is idempotent.
- Insert/query round trip for first relation.
- Duplicate deterministic id import behavior matches spec.
- Missing required scope/source/evidence/hash fields fail before write.
- DB row cannot satisfy a MessageBox/Channel/History/artifact gate.

### Exit criteria

- DB writes work for one narrow evidence slice.
- Rows carry enough scope/source/evidence/hash data for future imports.
- Schema is not shaped by legacy `scheduler.json`/`node.json` authority assumptions.

## Phase 5 — Dual-strict for the first slice

### Implement

- `ConfiguredEvalStore::DualStrict`.
- Write both filesystem and DB forms for the first slice.
- Compare deterministic semantic envelope/hash.
- Fail loudly on DB error or mismatch.

Define before coding:

- write order;
- rollback/compensation expectations if one side succeeds and the other fails;
- whether duplicate writes are idempotent or rejected;
- exact error type and transition behavior on mismatch.

### Tests to write/run

- Successful dual write creates unchanged filesystem output and expected DB row.
- DB failure fails the transition/slice loudly.
- Semantic mismatch fails.
- Repeated write/import behavior matches idempotency spec.
- Affected transition producer/consumer tests in `dual-strict`.

### Exit criteria

- Dual-strict provides confidence rather than silent divergence.
- Files remain usable as the compatibility source during migration.

## Phase 6 — Expand writer slices by dependency chain

Recommended order:

1. Parent-owned trace/log/ref evidence.
2. Parent-start/resource/final report projections.
3. Node status and runner request/result compatibility refs, split by attempt/runtime.
4. Child-plan payload mirror after `Received<ChildPlan>` only.
5. Channel receipt/import evidence mirrors, not transport replacement.
6. Evaluation/comparison/selection summaries.
7. Operation/artifact provenance refs needed by selection/handoff review.

For each slice:

- update relation DDL only as needed;
- run producer transition tests;
- run downstream consumer tests from the matrix;
- run authority-negative tests;
- add a full live loop canary only when the slice changes a cross-chain contract.

## Phase 7 — DB-only readiness review, not implementation by default

Before enabling DB-only mode for real runs, audit production reads of:

- `scheduler.json`;
- latest `runner-result.json`;
- `branches.json`;
- `node.json`;
- transition journal JSONL;
- child-plan MessageBox files;
- channel JSONL;
- sealed History files.

DB-only mode is ready only for surfaces whose reads have been migrated or explicitly remain filesystem authority/compatibility dependencies.

## First-slice decision

The first writer is fixed: **parent-start/resource evidence around `R4c -> R5`**.

Rationale:

- parent-owned;
- non-authority evidence/projection;
- no provider cost;
- exercises config, receipts, deterministic envelopes, DB insert/query, and dual-strict mismatch handling;
- avoids the broader blast radius of `JsonRecordFile::emit` until the store/error/config shape is proven.

`eval_record_ref` for one `JsonRecordFile::emit` family is the next likely slice after this first writer, not an alternative first slice.

### First-slice contract to finalize before coding

Keep the first-slice contract in one place and reference it from other docs. Before implementing Phase 3/4, write the exact contract into [`database-planning-notes.md`](database-planning-notes.md):

- the narrow method name and envelope type, e.g. a parent-start-specific method rather than a generic record emitter;
- the filesystem evidence covered by parity: `JournalEntry::ParentStarted` and the parent-start resource sample currently appended by `r4c_to_r5`;
- the DB relation set for this slice only;
- deterministic id and semantic envelope/hash inputs;
- minimal enum values for `store_scope`, `producer_role`, `visibility_scope`, `source_class`, `evidence_class`, and `validation_status`;
- dual-strict write order, duplicate handling, and failure behavior.

## Test command notes

Exact commands should be finalized when test targets exist. Expected families:

```text
cargo test -p ploke-eval prototype1_transition_inventory
cargo test -p ploke-eval prototype1_checkpoint
cargo test -p ploke-eval prototype1_eval_store
cargo test -p ploke-eval prototype1_storage_dual_strict
```

Live provider suite should be explicit and must not rely on a feature alone. In the current crate, `live_api_tests` may be enabled by default, so use an ignored test plus a suite-specific environment opt-in:

```text
PLOKE_EVAL_LIVE_API_TESTS=1 cargo test -p ploke-eval --features live_api_tests -- --ignored prototype1_live_transition
```

Keep exact provider/model/profile requirements beside the tests when implemented.

## Implementation stop conditions

Stop and revisit the plan if any proposed change would:

- make DB rows authoritative for History, Channel, MessageBox, or artifact mutation;
- silently tolerate missing expected filesystem or DB evidence in dual-strict;
- turn legacy projections into first-class decision authority;
- require repeated full live loops for ordinary narrow development iteration;
- make provider-facing confidence tests pass without real provider calls.
