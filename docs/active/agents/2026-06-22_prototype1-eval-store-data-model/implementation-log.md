# Prototype 1 Eval Store Implementation Log

Status: active implementation log template; fill as slices are implemented.

Related files:

- [`slice-by-slice-implementation-plan.md`](slice-by-slice-implementation-plan.md)
- [`live-transition-test-plan.md`](live-transition-test-plan.md)
- [`transition-persistence-dependency-matrix.md`](transition-persistence-dependency-matrix.md)
- [`database-planning-notes.md`](database-planning-notes.md)

## Operating contract

Use this log during implementation so another agent/operator can resume, review, or revert at slice boundaries.

For each slice:

1. Record assumptions and any unresolved decisions before editing.
2. Run impact analysis for symbols before modifying code.
3. Update tests first when the slice changes behavior.
4. Run the slice's local tests and required checkpoint/live tests.
5. Record exact commands, environment variables, and artifact/checkpoint paths.
6. Run `git status --short` and `git diff --stat` before committing.
7. Run GitNexus change detection before each commit when available in the harness.
8. Commit after each validated slice with a slice-scoped message.

Do not commit secrets, provider credentials, live run payloads with secrets, or large generated artifacts. Persist only manifests, hashes, and intentional small fixtures.

## Live API policy for implementation

Provider-facing producer transitions may make live API calls when the operator has exported the required opt-in variables and credentials.

Required migration-suite opt-in:

```text
PLOKE_EVAL_LIVE_API_TESTS=1
```

Recommended strict mode for avoiding accidental skips:

```text
PLOKE_RUN_LIVE_TESTS=1
```

Direct-Google defaults and credentials:

```text
GOOGLE_PROJECT_ID=<project>      # code can default if absent, but log the effective value
GOOGLE_REGION=<region>           # code can default if absent, but log the effective value
PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID=google/gemini-2.5-flash-lite  # or explicit override
# Google ADC auth must be available to the process.
```

Provider-facing producer tests must not pass through skips or mocks when `PLOKE_RUN_LIVE_TESTS=1` is set.

## Commit cadence

Suggested commit boundaries:

- `docs: add eval-store implementation validation gates` — planning/log-only update.
- `test: add prototype1 transition inventory and checkpoints` — Slice 0.
- `feat: add prototype1 eval storage config` — Slice 1.
- `feat: add fs eval-store parent-start scaffold` — Slices 2/3 if small enough, otherwise split.
- `feat: add eval-store parent-start db schema` — Slice 4.
- `feat: enable dual-strict parent-start eval storage` — Slice 5.
- Continue one commit per later storage slice.

Commit only after the slice's required tests pass or, for expected-failing tests added before implementation, after documenting the expected failure and not claiming the slice is complete.

## Slice entries

### Slice 0 — Transition inventory and checkpoint harness

- Status: in progress; inventory + checkpoint manifest foundation added. Remaining before full Slice 0 exit: real restored-checkpoint consumer edge and authority-negative fixture at a real gate.
- Assumptions:
  - Parent transition inventory can be source-derived from `WalkPhase::next_steps()` plus explicit child C1-C5 rows until child walk metadata exists.
  - Seed F0/F1 checkpoint fixtures are manifest/hash fixtures only; they intentionally do not claim History/channel/MessageBox/artifact authority.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/transition_inventory.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/checkpoint.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
  - `crates/ploke-eval/tests/fixtures/prototype1-checkpoints/`
  - `docs/active/agents/2026-06-22_prototype1-eval-store-data-model/transition-inventory.generated.md`
- Tests added/changed:
  - `prototype1_transition_inventory_covers_source_edges`
  - `prototype1_transition_inventory_names_live_api_edges`
  - `prototype1_transition_inventory_generated_doc_matches_source`
  - `prototype1_checkpoint_seed_manifests_verify_hashes`
  - `prototype1_checkpoint_missing_required_file_fails`
  - `prototype1_checkpoint_hash_mismatch_fails`
- Commands run:
  - `PLOKE_UPDATE_TRANSITION_INVENTORY=1 cargo test -p ploke-eval prototype1_transition_inventory_generated_doc_matches_source -- --nocapture`
  - `cargo fmt --all`
  - `cargo test -p ploke-eval prototype1_transition_inventory -- --nocapture`
  - `cargo test -p ploke-eval prototype1_checkpoint -- --nocapture`
- Live API used: no
- Checkpoints created/updated:
  - `F0_setup` seed manifest and required hash files.
  - `F1_ready_parent` seed manifest and required hash files.
- Artifacts retained:
  - checked-in generated inventory doc with row count 26.
  - checked-in checkpoint seed manifests/files only; no provider payloads or secrets.
- Result: focused inventory and checkpoint manifest tests pass; full Slice 0 still requires restored consumer and authority-negative gate tests.
- Commit: pending

### Slice 1 — Profile config shape only

- Status: not started
- Assumptions:
- Code touched:
- Tests added/changed:
- Commands run:
- Live API used: no
- Checkpoints created/updated:
- Artifacts retained:
- Result:
- Commit:

### Slice 2 — EvalStore module, receipts, filesystem backend only

- Status: not started
- Assumptions:
- Code touched:
- Tests added/changed:
- Commands run:
- Live API used: no
- Checkpoints created/updated:
- Artifacts retained:
- Result:
- Commit:

### Slice 3 — Move `R4c -> R5` behind FsEvalStore

- Status: not started
- Assumptions:
- Code touched:
- Tests added/changed:
- Commands run:
- Live API used: no
- Checkpoints created/updated:
- Artifacts retained:
- Result:
- Commit:

### Slice 4 — First DB schema/backend for tests and injected stores

- Status: not started
- Assumptions:
- Code touched:
- Tests added/changed:
- Commands run:
- Live API used: no
- Checkpoints created/updated:
- Artifacts retained:
- Result:
- Commit:

### Slice 5 — Production DB construction and dual-strict for first slice

- Status: not started
- Assumptions:
- Code touched:
- Tests added/changed:
- Commands run:
- Live API used: no for parent-start slice
- Checkpoints created/updated:
- Artifacts retained:
- Result:
- Commit:

### Slice 6+ — Later evidence slices

Create a new subsection per slice before editing.
