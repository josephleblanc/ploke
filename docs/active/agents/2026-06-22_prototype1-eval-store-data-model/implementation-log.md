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

- Status: complete for Slice 0 harness scope.
- Assumptions:
  - Parent transition inventory can be source-derived from `WalkPhase::next_steps()` plus explicit child C1-C5 rows until child walk metadata exists.
  - Seed F0/F1 checkpoint fixtures are manifest/hash fixtures only; they intentionally do not claim History/channel/MessageBox/artifact authority.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/transition_inventory.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/checkpoint.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
  - `crates/ploke-eval/tests/fixtures/prototype1-checkpoints/`
  - `docs/active/agents/2026-06-22_prototype1-eval-store-data-model/transition-inventory.generated.md`
- Tests added/changed:
  - `prototype1_transition_inventory_covers_source_edges`
  - `prototype1_transition_inventory_names_live_api_edges`
  - `prototype1_transition_inventory_generated_doc_matches_source`
  - `prototype1_checkpoint_seed_manifests_verify_hashes`
  - `prototype1_checkpoint_seed_fixtures_do_not_claim_authority`
  - `prototype1_checkpoint_restore_feeds_journal_consumer`
  - `prototype1_checkpoint_missing_required_file_fails`
  - `prototype1_checkpoint_hash_mismatch_fails`
  - `prototype1_storage_authority_negative_projection_cannot_replace_child_plan_box`
- Commands run:
  - `PLOKE_UPDATE_TRANSITION_INVENTORY=1 cargo test -p ploke-eval prototype1_transition_inventory_generated_doc_matches_source -- --nocapture`
  - `cargo fmt --all`
  - `cargo test -p ploke-eval prototype1_transition_inventory -- --nocapture`
  - `cargo test -p ploke-eval prototype1_checkpoint -- --nocapture`
  - `cargo fmt --all`
  - `cargo test -p ploke-eval prototype1_checkpoint -- --nocapture`
  - `cargo test -p ploke-eval prototype1_storage_authority_negative_projection_cannot_replace_child_plan_box -- --nocapture`
  - `cargo test -p ploke-eval prototype1_storage_authority_negative -- --nocapture`
- Live API used: no
- Checkpoints created/updated:
  - `F0_setup` seed manifest and required hash files.
  - `F1_ready_parent` seed manifest and required hash files.
- Artifacts retained:
  - checked-in generated inventory doc with row count 26.
  - checked-in checkpoint seed manifests/files only; no provider payloads or secrets.
- Result: focused inventory, checkpoint manifest/restore, restored journal consumer, and authority-negative MessageBox gate tests pass. Slice 0 names 26 transition/outcome rows; provider-facing rows remain live/API only in the confidence suite.
- Commit: `efbdf01a`, `3f41cf05`, `57b98b41`

### Slice 1 — Profile config shape only

- Status: complete.
- Assumptions:
  - Omitted `[storage]` and omitted `[storage.eval]` must continue to default to filesystem mode.
  - A `[storage.eval]` table may be present without explicitly repeating `worktree_root`; `worktree_root` keeps its prior default.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/profile.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
  - `crates/ploke-records/src/run_profile.rs`
  - `crates/ploke-eval/tests/fixtures/prototype1-checkpoints/F0_setup/`
- Tests added/changed:
  - `run_profile_storage_eval_backend_defaults_to_fs`
  - `run_profile_storage_eval_backend_roundtrips_kebab_case`
  - `run_profile_storage_eval_backend_rejects_unknown`
  - `state_run_shape_defaults_eval_storage_backend_to_fs`
  - `state_run_shape_prefers_admitted_campaign_profile`
  - `run_profile_toml_defaults_eval_storage_to_fs`
  - `run_profile_toml_roundtrips_eval_storage_backend`
  - existing `admitted_run_profile_carries_digest`
  - existing `prototype1_checkpoint_seed_manifests_verify_hashes`
- Commands run:
  - `cargo fmt --all`
  - `cargo test -p ploke-eval run_profile_storage_eval_backend -- --nocapture`
  - `cargo test -p ploke-eval state_run_shape -- --nocapture`
  - `cargo test -p ploke-records run_profile_toml -- --nocapture`
  - `cargo test -p ploke-eval admitted_run_profile_carries_digest -- --nocapture`
  - `cargo test -p ploke-eval prototype1_checkpoint_seed_manifests_verify_hashes -- --nocapture`
- Live API used: no
- Checkpoints created/updated:
  - `F0_setup/prototype1/run-profile.toml` now explicitly includes `[storage.eval] backend = "fs"` and manifest hash was updated.
- Artifacts retained:
  - no provider artifacts; only the updated F0 seed manifest/profile fixture.
- Result: storage backend config parses, defaults to `fs`, round-trips through the passive `ploke-records` DTO, is carried into `Prototype1StateRunShape`, and unknown backend values are rejected.
- Commit: `647c1ab3`

### Slice 2 — EvalStore module, receipts, filesystem backend only

- Status: complete; production `R4c -> R5` not moved yet.
- Assumptions:
  - `append_with_receipt` must preserve the exact compact JSONL bytes produced by the existing `RecordStore::append` implementation.
  - Resource-sample construction can be split from append behavior without changing current best-effort callers.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/journal.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- Tests added/changed:
  - `prototype1_eval_store_parent_start_fs_appends_expected_entries`
  - `append_with_receipt_preserves_record_store_bytes`
  - existing `replay_all_collects_each_transition_family`
- Commands run:
  - `cargo fmt --all`
  - `cargo test -p ploke-eval prototype1_eval_store_parent_start_fs -- --nocapture`
  - `cargo test -p ploke-eval append_with_receipt_preserves_record_store_bytes -- --nocapture`
  - `cargo test -p ploke-eval replay_all_collects_each_transition_family -- --nocapture`
  - `cargo test -p ploke-eval prototype1_eval_store_parent_start_fs -- --nocapture`
- Live API used: no
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: `EvalStore`/`FsEvalStore` exists, returns journal append receipts, writes `ParentStarted` plus `Resource(parent_start)`, and existing journal replay smoke still passes. Default production path is unchanged until Slice 3.
- Commit: `2ce48b46`

### Slice 3 — Move `R4c -> R5` behind FsEvalStore

- Status: complete for `fs`; production `database`/`dual-strict` return explicit configuration errors until DB handle wiring. Transition-contract tests were added after review clarified that per-edge tests are required, not just store-unit tests.
- Assumptions:
  - `backend = fs` must preserve the existing parent-start/resource JSONL evidence path.
  - `database` and `dual-strict` must fail loudly rather than use the passive mirror or silently fall back.
  - Checkpoints accelerate setup and downstream coverage; they do not replace tests that execute the exact typestate transition.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
  - `docs/active/agents/2026-06-22_prototype1-eval-store-data-model/slice-by-slice-implementation-plan.md`
- Tests added/changed:
  - `prototype1_transition_contract_r4c_to_r5_fs_records_parent_start`
  - `prototype1_transition_contract_r4c_to_r5_db_backends_fail_loudly_without_writes`
  - existing `prototype1_eval_store_parent_start_fs_appends_expected_entries` covers the lower-level writer semantics used by `R4c -> R5`.
- Commands run:
  - `cargo fmt --all`
  - `cargo test -p ploke-eval prototype1_eval_store_parent_start_fs -- --nocapture`
  - `cargo fmt --all`
  - `cargo test -p ploke-eval prototype1_transition_contract_r4c_to_r5 -- --nocapture`
  - `cargo test -p ploke-eval prototype1_transition_contract -- --nocapture`
  - `cargo test -p ploke-eval prototype1_eval_store_parent_start_fs -- --nocapture`
  - `cargo check -p ploke-eval`
- Live API used: no; `R4c -> R5` is parent-owned and provider-free.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: `R4c -> R5` delegates parent-start evidence to `ConfiguredEvalStore::Fs`; the transition-contract test proves the output is `R5`, the journal contains exactly `ParentStarted` plus `Resource(parent_start)`, and no History/message/artifact authority surface is created by this edge. `database`/`dual-strict` produce explicit `DatabaseSetup` errors and write no parent-start journal evidence pending DB handle wiring.
- Commit: `9b8e3247` for production routing; `bca4eec2` for transition-contract tests.

### Slice 4 — First DB schema/backend for tests and injected stores

- Status: complete for injected/test DB writer; production `database`/`dual-strict` modes remain explicitly unwired until the parent-owned DB handle slice.
- Assumptions:
  - Slice 4 should use Cozo through `ploke_db::Database`, not through a raw `cozo::DbInstance` opened by `EvalStore`.
  - DB rows for this first slice are projections of validated parent-start journal receipts; JSONL replay remains the filesystem authority until dual-strict is wired.
  - First-slice record refs can use non-null `producer_id`, `payload_json`, and `recorded_at` because `ParentStarted` and `Resource(parent_start)` receipts always provide them.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store.rs`
- Tests added/changed:
  - `prototype1_eval_store_parent_start_db_schema_installs_idempotently`
  - `prototype1_eval_store_parent_start_db_round_trips_rows`
  - `prototype1_eval_store_parent_start_db_duplicate_identical_is_idempotent`
  - `prototype1_eval_store_parent_start_db_duplicate_semantic_mismatch_fails`
  - `prototype1_eval_store_parent_start_db_missing_required_hash_fails_before_rows`
- Commands run:
  - `cargo fmt --all`
  - `cargo test -p ploke-eval prototype1_eval_store_parent_start_db -- --nocapture` (initial run hit `/tmp` quota during tempfile write)
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start_db -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
- Live API used: no; this is a parent-start DB projection slice.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: `DbEvalStore<D>` and narrow `EvalDb` trait now support idempotent schema install and deterministic parent-start DB rows in tests. The DB writer creates one `eval_transition_event` row plus two `eval_record_ref` rows from filesystem append receipts, treats duplicate identical semantic hashes as idempotent, rejects duplicate event IDs with semantic hash mismatches, and validates required source/hash fields before row writes.
- Commit: `3b45c47d`

### Slice 5 — Production DB construction and dual-strict for first slice

- Status: complete for local/storage/typestate gates; direct-Google broad headless-TUI canary was attempted eagerly and later retired as stale because current broad admission runs declared validation in the harness after applied batches, so the old `AppliedValidationMissing` expectation no longer matches the validation model.
- Assumptions:
  - Use an owner-scoped eval DB backup file under the campaign tree: `prototype1/eval-store.cozo.sqlite`.
  - Do not use `records/mirror.cozo.sqlite` for typed eval-store parity.
  - `database` mode for this first writer still appends compatibility JSONL first because DB rows and current consumers depend on journal source receipts.
  - `dual-strict` requires DB semantic hash parity after the filesystem append and reports repair data if DB persistence fails after journal evidence is written.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/live_edges.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - `prototype1_eval_store_parent_start_dual_strict_persists_owner_db`
  - `prototype1_eval_store_parent_start_dual_strict_failure_keeps_repairable_journal`
  - `prototype1_transition_contract_r4c_to_r5_db_backends_record_parent_start_rows`
  - updated `prototype1_transition_contract_r4c_to_r5_db_backends_fail_loudly_without_writes` into the DB/dual-strict success contract above.
  - updated DB duplicate semantic mismatch coverage to preserve deterministic event id while changing semantic evidence.
- Commands run:
  - `gitnexus status` reported stale index; `npx gitnexus analyze` refreshed the index.
  - `gitnexus impact ...` for `ConfiguredEvalStore`, `DbEvalStore`, `EvalStore`, `parent_started_rows`, and `r4c_to_r5`: LOW risk; no HIGH/CRITICAL warnings.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_transition_contract_r4c_to_r5 -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_transition_inventory -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_checkpoint -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative -- --nocapture`
  - `PLOKE_EVAL_LIVE_API_TESTS=1 PLOKE_RUN_LIVE_TESTS=1 PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID=google/gemini-2.5-flash-lite TMPDIR=$PWD/target/tmp cargo test -p ploke-eval --features live_api_tests -- --ignored prototype1_live_transition --nocapture` matched/runs 0 tests; not counted as a live pass.
  - `PLOKE_EVAL_LIVE_API_TESTS=1 PLOKE_RUN_LIVE_TESTS=1 PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID=google/gemini-2.5-flash-lite PLOKE_EVAL_HEADLESS_TUI_GOOGLE_MODEL_ID=google/gemini-2.5-flash-lite TMPDIR=$PWD/target/tmp cargo test -p ploke-eval --features live_api_tests live_google_direct_broad_headless_tui_rejects_applied_edit_missing_declared_validation -- --ignored --nocapture` failed after live Google call: model completed without staging an edit.
  - Same broad headless-TUI canary with `google/gemini-2.5-flash` failed after live Google call: model applied an edit and harness-owned declared validation failed, exposing that the canary's `AppliedValidationMissing` expectation was stale.
  - Same broad headless-TUI canary with `google/gemini-2.5-pro` failed with the same requested-validation path; a parent-commit repro showed this was not introduced by Slice 5.
  - `PLOKE_EVAL_LIVE_API_TESTS=1 PLOKE_RUN_LIVE_TESTS=1 PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID=google/gemini-2.5-flash-lite TMPDIR=$PWD/target/tmp cargo test -p ploke-eval --features live_api_tests live_google_protocol_json_adjudication_uses_direct_route_success_or_quota -- --ignored --nocapture` passed and returned sentinel JSON through the direct-Google route.
- Live API used: yes, direct-Google. Provider route/auth was confirmed by the protocol JSON canary. The broad headless-TUI live canary was not treated as a pass and has been retired as stale in `docs/active/agents/expected-failing-regression-tests.md`.
- Checkpoints created/updated: none
- Artifacts retained:
  - Live broad headless-TUI artifacts under `~/.ploke-eval/probes/live-google-broad-headless/run-*`; no artifacts were checked in.
- Result: production `database` and `dual-strict` parent-start modes now write compatibility journal evidence plus owner-scoped eval DB rows. `dual-strict` fails loudly with journal path, source event indices, line hashes, expected semantic hash, and repair guidance if DB persistence fails after filesystem append. Focused local/storage/typestate/checkpoint/authority tests pass. Direct-Google protocol route passes; broad headless-TUI missing-validation live canary was stale relative to harness-owned validation and is no longer used as a storage-slice confidence gate.
- Commit: current slice commit, `feat: enable dual-strict parent-start eval storage`.

### Slice 6 — Trace/log/ref evidence lane

- Status: implemented a local trace/log/ref evidence slice; no transition authority or provider-facing producer changed.
- Assumptions:
  - Existing Prototype 1 observation JSONL remains importable as queryable evidence.
  - `observe::TransitionBuilder` and `observe::Step` can additionally mirror direct producer rows when an owner-scoped eval DB sink is configured by the run boundary.
  - `eval_trace_event` and `eval_log_ref` rows are diagnostic/evidence refs only; they do not replace History, channel transport, MessageBox receipts, artifact mutation, invocation/bootstrap authority, or transition success/failure decisions.
  - Invalid observation JSONL should fail loudly before writing trace/log rows; direct producer mirror failures are warned but do not make trace rows authoritative.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/observe.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/run/core.rs`
- Tests added/changed:
  - `prototype1_eval_store_trace_log_ref_round_trips_row`
  - `prototype1_eval_store_trace_observation_jsonl_imports_rows_idempotently`
  - `prototype1_eval_store_trace_observation_jsonl_invalid_line_fails_without_rows`
  - `prototype1_observe_transition_builder_mirrors_trace_row_to_eval_db`
  - `prototype1_observe_step_mirrors_trace_row_to_eval_db`
  - updated `prototype1_eval_store_parent_start_db_schema_installs_idempotently` to assert `eval_log_ref` and `eval_trace_event` relation installation.
- Commands run:
  - `gitnexus impact ... ensure_eval_store_schema`: LOW risk; 1 direct caller, 0 affected processes.
  - `gitnexus impact ... observe::TransitionBuilder`: LOW risk; 1 direct caller, 0 affected processes.
  - `gitnexus impact ... observe::Step`: LOW risk; 0 direct callers, 0 affected processes.
  - `gitnexus impact ... run/core.rs::advance`: LOW risk; 2 direct callers, 2 live test processes.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_trace -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_observe_ -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_trace -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_transition_contract_r4c_to_r5 -- --nocapture`
- Live API used: no; this slice imports local observation JSONL and mirrors local observe producers into queryable diagnostic refs.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: `DbEvalStore` installs `eval_log_ref`/`eval_trace_event`, writes deterministic log refs, imports observation JSONL into deterministic trace ids, maps common structured trace fields, is idempotent on reimport, rejects invalid JSONL without trace/log rows, and accepts direct `TraceEventEvidence` rows. `observe::TransitionBuilder` and `observe::Step` now mirror trace rows to the owner-scoped eval DB when `prototype1-step/continue` runs with `database` or `dual-strict`; `fs` remains unchanged. Existing parent-start DB/dual-strict tests still pass.
- Commit: pending

### Slice 7+ — Later evidence slices

Create a new subsection per slice before editing.
