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
- Commit: `2470842a feat: mirror prototype1 trace evidence rows`

### Post-Slice 6 cleanup — EvalStore module split

- Status: refactor-only cleanup before Slice 7; no storage behavior or external `prototype1_state::eval_store::...` API path changed.
- Assumptions:
  - Split by responsibility now to keep Slice 7 record-ref work out of a 2,700+ line monolith.
  - Keep authority semantics and backend behavior unchanged; this is not a DB schema or writer change.
- Code touched:
  - replaced `crates/ploke-eval/src/cli/prototype1_state/eval_store.rs` with `crates/ploke-eval/src/cli/prototype1_state/eval_store/{mod.rs,api.rs,cozo_store.rs,evidence.rs,error.rs,tests.rs}`.
- Commands run:
  - refreshed GitNexus index after Slice 6 so new symbols were visible.
  - `gitnexus impact ...` for `EvalStore`, `ConfiguredEvalStore`, `FsEvalStore`, `FileDbEvalStore`, `DbEvalStore`, `ParentStartedEvidence`, `TraceEventEvidence`, `prototype1_eval_store_db_path`, `load_owner_eval_database`, `write_trace_event_to_owner_db`, and `EvalStoreError`: LOW risk; no HIGH/CRITICAL warnings.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_observe_ -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_transition_contract_r4c_to_r5 -- --nocapture`
- Live API used: no; refactor-only cleanup.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: EvalStore storage code is split into focused modules while focused eval-store, observe, and R4c→R5 contract tests still pass.
- Commit: current cleanup commit, `refactor: split eval-store module`

### Slice 7a — Compatibility record-ref DB lane

- Status: complete for the typed compatibility-record-ref DB lane; scheduler/node/result/branch producers are not migrated yet.
- Assumptions:
  - Compatibility/projection records should enter `eval_record_ref` with explicit axes and hashes, not as semantic scheduler/node authority.
  - A record-ref row must not fabricate transition rows or satisfy MessageBox/channel/History/artifact gates.
  - This sub-slice should reuse the existing `eval_record_ref` relation and `EvalRecordRefRow` instead of introducing scheduler/node mirror DTOs.
  - Duplicate record-ref IDs with the same content hash are idempotent; duplicate IDs with different content hashes fail before overwrite.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/evidence.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_store.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_params.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - `prototype1_eval_store_record_ref_compatibility_import_round_trips_axes`
  - `prototype1_eval_store_record_ref_duplicate_identical_is_idempotent`
  - `prototype1_eval_store_record_ref_missing_axis_fails_without_rows`
  - `prototype1_storage_authority_negative_record_ref_cannot_replace_child_plan_box`
  - updated record-ref query assertions to include source/evidence/visibility/status axes.
- Commands run:
  - `npx gitnexus analyze` refreshed a stale index before impact analysis.
  - `gitnexus impact ...` for `DbEvalStore`, `EvalRecordRefRow`, `put_record_ref_row`, and `record_ref_id`: LOW risk; no HIGH/CRITICAL warnings.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref -- --nocapture` passed, 3 tests.
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start -- --nocapture` passed, 8 tests.
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative -- --nocapture` passed, 2 tests.
- Live API used: no; this sub-slice is local compatibility record-ref persistence and authority-negative coverage only.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: `DbEvalStore::put_record_ref` writes queryable compatibility `eval_record_ref` rows with store scope, producer role, source class, evidence class, visibility, validation status, source coordinates, payload hash, and payload JSON. Missing axes fail before row writes. Compatibility record refs do not create `eval_transition_event` rows, and an owner eval DB row claiming child-plan projection does not replace the child-plan `MessageBox` file gate.
- Commit: current slice commit, `feat: add compatibility eval record refs`.

### Slice 7b — Runner-request compatibility record refs

- Status: complete for runner-request projection refs only; node projections, runner results, and branch registry refs are intentionally left for later Slice 7 sub-slices.
- Assumptions:
  - `runner-request.json` remains the filesystem authority for runner request loading.
  - The eval-store row is a compatibility `eval_record_ref` beside the existing passive mirror, not a scheduler/request authority table.
  - The writer should be a no-op for default filesystem runs with no owner eval DB file, preserving current setup behavior.
  - If an owner eval DB file is already present and the compatibility row cannot be written, the projection write should fail loudly with repairable context instead of silently losing configured DB parity.
- Code touched:
  - `crates/ploke-eval/src/record_emission.rs`
  - `crates/ploke-eval/src/intervention/scheduler.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
- Tests added/changed:
  - `prototype1_eval_store_record_ref_runner_request_projection_writes_owner_db_row`
  - `prototype1_storage_authority_negative_runner_request_ref_cannot_replace_file`
- Commands run:
  - `npx gitnexus analyze` refreshed a stale index before impact analysis.
  - `gitnexus impact ... save_node_record`: CRITICAL risk, 5 direct callers; avoided in this sub-slice.
  - `gitnexus impact ... save_runner_request`: LOW risk.
  - `gitnexus impact ... write_node_projection`: HIGH risk; avoided in this sub-slice.
  - `gitnexus impact ... write_runner_request_projection`: HIGH risk because it feeds the loop controller; proceeded with the narrower lower-level additive write after warning.
  - `gitnexus impact ... write_record_ref_to_owner_db`: LOW risk.
  - `gitnexus impact ... JsonRecordFile`, `JsonRecordFile::emit`, and `record_family`: LOW risk.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref_runner_request_projection_writes_owner_db_row -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_runner_request_ref_cannot_replace_file -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval register_treatment_node_persists_scheduler_and_runner_request -- --nocapture`
- Live API used: no; this sub-slice only mirrors a deterministic local runner-request projection into an existing owner eval DB.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: `save_runner_request` still writes `runner-request.json` and the passive shared record first. When `prototype1/eval-store.cozo.sqlite` already exists, it also writes a compatibility `eval_record_ref` row with explicit axes, source coordinates, payload JSON, and payload hash. A DB row claiming a runner request does not replace the missing `runner-request.json` gate.
- Commit: current slice commit, `feat: mirror runner request record refs`.

### Slice 7c — Branch registry compatibility record refs

- Status: complete for append-only `branches.json` compatibility refs; candidate, evaluation, selection, and continuation facts remain deferred to Slice 9 typed relations.
- Assumptions:
  - `branches.json` remains the append-only compatibility source for current branch registry loaders.
  - DB rows are parent-visible `eval_record_ref` compatibility evidence only; they do not select branches, replace branch registry replay, or provide evaluation/selection authority.
  - The writer should remain a no-op for filesystem runs with no owner eval DB file.
  - JSONL source coordinates must use the actual appended line index so multiple branch records do not collide.
- Code touched:
  - `crates/ploke-eval/src/record_emission.rs`
  - `crates/ploke-eval/src/intervention/branch_registry.rs`
- Tests added/changed:
  - `prototype1_eval_store_record_ref_branch_registry_append_writes_owner_db_row`
  - `prototype1_storage_authority_negative_branch_ref_cannot_replace_registry_log`
- Commands run:
  - `gitnexus impact ... branch_log::append`: LOW risk; 2 direct callers; no affected processes.
  - `gitnexus impact ... save_branch_registry`: LOW risk; 4 direct callers; no affected processes.
  - `gitnexus impact ... record_parent_comparison`: LOW risk; 2 direct callers; no affected processes.
  - `gitnexus impact ... write_record_ref_to_owner_db`: LOW risk.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref_branch_registry_append_writes_owner_db_row -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_branch_ref_cannot_replace_registry_log -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval append_resolved_comparison_records_parent_comparison -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval record_synthesis_creates_source_node_and_selected_branch -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref -- --nocapture`
- Live API used: no; this sub-slice mirrors local branch registry JSONL appends into an existing owner eval DB.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: branch registry append still writes the JSONL line first. When `prototype1/eval-store.cozo.sqlite` already exists, the appended line is also mirrored as a `branch_registry` compatibility `eval_record_ref` row with source line/index, payload JSON, and payload hash. A DB row claiming branch registry evidence does not replace the missing `branches.json` log for branch selection or registry loading.
- Commit: current slice commit, `feat: mirror branch registry record refs`.

### Slice 7d — Child-local runner-result compatibility record refs

- Status: complete for child-local runner-result refs; parent-imported terminal channel facts remain deferred to the channel/import slices.
- Assumptions:
  - `runner-result.json` and `nodes/<node>/results/<runtime>.json` remain child-local/projection evidence.
  - Runner-result DB rows must use `store_scope = child_runtime`, `producer_role = child`, and `visibility_scope = local`; they are not parent-visible terminal evidence.
  - Parent-visible success still requires terminal channel `Result` plus treatment evidence, and stored runner results must agree with channel payloads before recovery/selection.
  - Default filesystem runs with no owner eval DB remain unchanged.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/evidence.rs`
  - `crates/ploke-eval/src/record_emission.rs`
  - `crates/ploke-eval/src/intervention/scheduler.rs`
- Tests added/changed:
  - `prototype1_eval_store_record_ref_runner_result_writes_child_local_rows`
  - `prototype1_storage_authority_negative_runner_result_ref_cannot_replace_file`
  - existing `observe_child_times_out_on_success_sidecar_without_channel_result` retained as the channel authority-negative gate.
- Commands run:
  - `npx gitnexus analyze` refreshed the index so Slice 7b/7c helper symbols were visible; generated AGENTS/CLAUDE count churn was removed before commit.
  - `gitnexus impact ... save_runner_result`: HIGH risk; 2 direct callers; no affected processes.
  - `gitnexus impact ... write_runner_result_at`: HIGH risk; affects `live_google_child_runner_success`; proceeded with additive/no-op-without-owner-DB mirror and local channel-authority coverage only.
  - `gitnexus impact ... record_attempt_runner_result`: LOW risk; affects `live_google_child_runner_success`.
  - `gitnexus impact ... RecordRefEvidence impl`: LOW risk.
  - `gitnexus impact ... emit_eval_record_ref_if_owner_db_exists`: CRITICAL risk because it feeds parent setup; avoided changing that existing helper's behavior and added a separate child-runtime helper.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref_runner_result_writes_child_local_rows -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_runner_result_ref_cannot_replace_file -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval runner_result_projection_parses_as_shared_passive_record -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval observe_child_times_out_on_success_sidecar_without_channel_result -- --nocapture`
- Live API used: no. This sub-slice touches the child-runner result path, but no live-provider gate was claimed; focused local tests cover DB rows and existing channel authority. A future provider-facing channel/import slice must run or explicitly block live Google confidence.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: `write_runner_result_at` still writes the passive shared `RunnerResultRecord` first. When `prototype1/eval-store.cozo.sqlite` exists, attempt-scoped and latest runner-result files are mirrored as child-local compatibility `eval_record_ref` rows with payload JSON and hashes. DB runner-result refs do not replace the file gate, and success sidecars still cannot advance C4 without terminal channel results.
- Commit: current slice commit, `feat: mirror child runner result refs`.

### Slice 7e — Parent-owned scheduler-node compatibility record refs

- Status: complete for deterministic parent-owned C1/C2/C3 scheduler node projection refs only; broad planning/root parent node refs, child terminal node refs, and legacy generic node writes remain deferred.
- Assumptions:
  - `node.json` remains the scheduler-node projection file and current loader authority.
  - DB rows are parent-visible `eval_record_ref` compatibility evidence only; they do not replace scheduler files, channel results, History, selection, or artifact authority.
  - The generic `write_node_projection` path is mixed-authority because it is used by parent deterministic transitions, broad planning, and child terminal/result paths, so this slice adds a narrower parent-owned wrapper instead of changing generic semantics.
  - Default filesystem runs with no owner eval DB remain unchanged.
- Code touched:
  - `crates/ploke-eval/src/record_emission.rs`
  - `crates/ploke-eval/src/intervention/scheduler.rs`
  - `crates/ploke-eval/src/intervention/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/c1.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/c2.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/c3.rs`
- Tests added/changed:
  - `prototype1_eval_store_record_ref_parent_node_projection_writes_owner_db_row`
  - `prototype1_storage_authority_negative_node_ref_cannot_replace_file`
  - existing `child_build_promotes_binary_and_cleans_scratch`
  - existing `child_spawn_observes_ready`
  - existing `child_spawn_observes_failed_result`
- Commands run:
  - `gitnexus impact ... write_node_projection`: HIGH risk; affects `run_prototype1_loop_controller` and `live_google_child_runner_success`; avoided changing generic writer behavior and added a parent-owned wrapper.
  - `gitnexus impact ... MaterializeBranch::transition`: LOW risk.
  - `gitnexus impact ... BuildChild.transition`: LOW risk.
  - `gitnexus impact ... SpawnChild.transition`: LOW risk.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref_parent_node_projection_writes_owner_db_row -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_node_ref_cannot_replace_file -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval child_build_promotes_binary_and_cleans_scratch -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval child_spawn_observes_ready -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval child_spawn_observes_failed_result -- --nocapture`
- Live API used: no. This sub-slice mirrors deterministic local parent-owned scheduler node projections only. Provider-facing broad planning node refs and child terminal/provider-result facts remain future slices and must run or explicitly block live Google confidence when touched.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: C1 materialization, C2 build status, and C3 spawn status now write `node.json` first and, when `prototype1/eval-store.cozo.sqlite` exists, mirror that exact JSON projection as a `scheduler_node` compatibility `eval_record_ref` row with parent scope, source coordinates, payload JSON, and payload hash. A DB row claiming a scheduler node does not replace the missing `node.json` file gate.
- Commit: current slice commit, `feat: mirror parent node projection refs`.

### Slice 7f — Root-parent scheduler-node compatibility record refs

- Status: complete for root-parent node registration refs only; broad planning child-node projections, child terminal node refs, and legacy process writes remain deferred.
- Assumptions:
  - `register_root_parent_node` is a parent-owned setup/registration writer, not a provider-facing producer.
  - `node.json` remains the scheduler-node projection file and current loader authority.
  - DB rows are parent-visible `eval_record_ref` compatibility evidence only; they do not replace parent identity, scheduler state, runner request files, History, channel, or artifact authority.
  - Default filesystem setup with no owner eval DB remains unchanged.
- Code touched:
  - `crates/ploke-eval/src/intervention/scheduler.rs`
- Tests added/changed:
  - `prototype1_eval_store_record_ref_root_parent_node_registration_writes_owner_db_row`
  - existing `prototype1_storage_authority_negative_node_ref_cannot_replace_file`
  - existing `register_treatment_node_persists_scheduler_and_runner_request`
- Commands run:
  - `gitnexus impact ... register_root_parent_node`: LOW risk; 1 direct caller, `prepare_prototype1_parent_setup`.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref_root_parent_node_registration_writes_owner_db_row -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_node_ref_cannot_replace_file -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval register_treatment_node_persists_scheduler_and_runner_request -- --nocapture`
- Live API used: no; this sub-slice mirrors setup-time root-parent node projection evidence and does not touch provider-facing broad planning or child treatment execution.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: root-parent registration still writes the scheduler node file before scheduler state and runner request persistence. When `prototype1/eval-store.cozo.sqlite` exists, it also mirrors that root `node.json` projection as a parent-visible `scheduler_node` compatibility `eval_record_ref` with source coordinates, payload JSON, and payload hash. Existing node authority-negative coverage still proves DB rows cannot replace the missing `node.json` gate.
- Commit: current slice commit, `feat: mirror root parent node refs`.

### Slice 7g — Deterministic TUI-tools parent-node compatibility record refs

- Status: complete for deterministic TUI-tools parent Running/Failed node projections only; broad/default planning parent writes, child-plan child node projections, provider-facing broad planning outputs, and legacy process writes remain deferred.
- Assumptions:
  - `publish_deterministic_tui_tools_child_plan` is a deterministic/local child-plan producer and does not call a provider.
  - The parent `node.json` projection remains the scheduler-node file authority.
  - DB rows are parent-visible `eval_record_ref` compatibility evidence only; they do not replace the child-plan MessageBox, scheduler files, History, channel, or artifact/worktree authority.
  - Changing the shared `write_treatment_evaluation_projection` helper would also affect provider-facing broad planning outputs, so this slice does not change it.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - updated `tui_edit_surface_parent_selection_publishes_child_plan` with owner eval DB row assertions for the parent Running projection.
  - existing `prototype1_storage_authority_negative_node_ref_cannot_replace_file`
  - existing `broad_harness_rejects_unbound_existing_child_plan`
- Commands run:
  - `gitnexus impact ... publish_deterministic_tui_tools_child_plan`: LOW risk; direct caller `run_parent_target_selection`, no affected processes.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval tui_edit_surface_parent_selection_publishes_child_plan -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_record_ref -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_node_ref_cannot_replace_file -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval broad_harness_rejects_unbound_existing_child_plan -- --nocapture`
- Live API used: no; this sub-slice covers deterministic TUI-tools parent status projections only. Provider-facing broad planning remains unchanged and still requires live Google confidence when touched.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: deterministic TUI-tools child-plan publication still writes and validates the child-plan MessageBox plus child node/request projection files. When `prototype1/eval-store.cozo.sqlite` exists, the parent Running/Failed `node.json` projections are also mirrored as parent-visible `scheduler_node` compatibility `eval_record_ref` rows with source coordinates, payload JSON, and payload hash. The broad-harness rejection test still proves deterministic child plans are not accepted as request-bound broad harness evidence.
- Commit: current slice commit, `feat: mirror deterministic parent node refs`.

### Slice 8a — Child invocation eval mirror

- Status: complete for child executable invocation mirrors only; successor handoff invocation mirrors and channel message/receipt mirrors remain deferred.
- Assumptions:
  - `Invocation` remains the executable bootstrap contract; `eval_invocation` rows are query evidence only.
  - `write_child_invocation` must remain filesystem-first. A DB failure after file write fails the configured write loudly, but a DB row cannot launch or replace the invocation file.
  - The first Slice 8 invocation row should reuse the real persisted `Invocation` JSON and add only internal DB evidence/row carriers for axes, path, and content hash.
  - Successor invocation writes are a separate HIGH-risk handoff surface and are not changed in this sub-slice.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/evidence.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_params.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_store.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`
- Tests added/changed:
  - `prototype1_eval_store_child_invocation_writes_owner_db_row`
  - `prototype1_storage_authority_negative_invocation_row_cannot_replace_file`
  - existing `child_spawn_observes_ready`
  - existing `child_spawn_observes_failed_result`
  - existing `prototype1_eval_store_parent_start`
- Commands run:
  - `gitnexus impact ... write_child_invocation`: LOW risk; direct callers include child runner tests and `live_google_child_runner_success`.
  - `gitnexus impact ... write_successor_invocation_for_retired_parent`: HIGH risk through successor handoff and `r12_to_r13`; deferred.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_child_invocation_writes_owner_db_row -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_invocation_row_cannot_replace_file -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval child_spawn_observes_ready -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval child_spawn_observes_failed_result -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start -- --nocapture`
- Live API used: no. This sub-slice mirrors child invocation metadata after the executable invocation file is written; it does not change provider-dependent child treatment execution or terminal result production.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: `eval_invocation` schema now records child invocation campaign/node/runtime/role, store/source/evidence axes, invocation path, source ref, content hash, and timestamps. `write_child_invocation` still writes the JSON file first, then optionally mirrors to `prototype1/eval-store.cozo.sqlite` when present. A DB invocation row does not replace the missing executable invocation file gate.
- Commit: current slice commit, `feat: mirror child invocation rows`.

### Slice 8b — Child Ready/Evaluating channel message eval mirror

- Status: complete for child `Ready` and `Evaluating` channel-message mirrors only; terminal `Result`, `ResultWritten`, failure/exit, successor, receipt/import, and provider-dependent child terminal-result surfaces remain deferred.
- Assumptions:
  - The serialized `Envelope<ToParent>` JSONL channel record remains the transport and protocol authority.
  - `eval_channel_message` rows are query evidence only; they do not synthesize messages, advance cursors, replace channel files, or make a child selectable.
  - `send_ready` and `send_evaluating` remain filesystem-first. If an owner eval DB already exists, a DB mirror failure after the channel append fails the send loudly.
  - Channel rows use axes `store_scope=channel`, `producer_role=child`, `visibility_scope=parent_visible`, `source_class=direct_write`, `evidence_class=channel_message`.
  - The first channel mirror reuses the real persisted `Envelope<ToParent>` as the transport contract and adds only internal DB evidence/row carriers for indexes, hashes, and source coordinates.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/evidence.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_params.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_store.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
- Tests added/changed:
  - `prototype1_eval_store_channel_ready_writes_owner_db_row`
  - `prototype1_storage_authority_negative_channel_row_cannot_replace_envelope`
  - existing `file_transport_reads_only_new_complete_records`
  - existing `envelope_validation_rejects_wrong_body_hash`
  - existing `child_spawn_observes_ready`
  - existing `prototype1_eval_store_parent_start`
  - existing `prototype1_eval_store_child_invocation_writes_owner_db_row`
- Commands run:
  - `gitnexus impact ... send_ready`: LOW risk; direct callers include channel tests and `execute_prototype1_runner_invocation`; affected process `live_google_child_runner_success`.
  - `gitnexus impact ... send_evaluating`: LOW risk; direct callers include channel tests and `execute_prototype1_runner_invocation`; affected process `live_google_child_runner_success`.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_channel_ready_writes_owner_db_row -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_channel_row_cannot_replace_envelope -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval file_transport_reads_only_new_complete_records -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval envelope_validation_rejects_wrong_body_hash -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval child_spawn_observes_ready -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_child_invocation_writes_owner_db_row -- --nocapture`
- Live API used: no. This sub-slice mirrors local child ready/evaluating channel envelopes and does not touch provider-dependent child treatment execution or terminal result production.
- Checkpoints created/updated: none
- Artifacts retained: none
- Result: `eval_channel_message` schema now records child channel envelope identity, direction, message kind/id, source endpoint/cursor/byte counts, body hash, serialized envelope hash, and timestamps. `send_ready` and `send_evaluating` still append the JSONL envelope first, then optionally mirror to `prototype1/eval-store.cozo.sqlite` when present. A DB channel-message row does not replace or synthesize a missing channel envelope.
- Commit: current slice commit, `feat: mirror child channel message rows`.

### Post-8b hygiene — Channel and eval-store module split

- Status: complete for behavior-preserving module split before continuing Slice 8.
- Reason:
  - `channel.rs`, `eval_store/evidence.rs`, and `eval_store/cozo_store.rs` had grown past the requested rough 800-1000 LOC hygiene bound after Slice 8b.
  - More Slice 8 channel/import relations would have compounded mixed responsibilities in those files.
- Assumptions:
  - This is a mechanical refactor only; no storage semantics, relation schemas, authority boundaries, or transition behavior should change.
  - Public/crate-visible API paths stay preserved through `eval_store/mod.rs` re-exports.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/channel/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_store.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_schema.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/evidence.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/observation.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
- Commands run:
  - `gitnexus impact ... Channel`: LOW risk; no affected processes.
  - `gitnexus impact ... ensure_eval_store_schema`: LOW risk; no affected processes.
  - `gitnexus impact ... parse_observation_jsonl`: LOW risk; no affected processes.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_channel_ready_writes_owner_db_row -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_channel_row_cannot_replace_envelope -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval file_transport_reads_only_new_complete_records -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_trace_observation_jsonl_imports_rows_idempotently -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_trace_observation_jsonl_invalid_line_fails_without_rows -- --nocapture`
- Live API used: no. This split does not touch provider-facing producers.
- Result: channel tests now live under `channel/tests.rs`; eval-store schema installation lives under `cozo_schema.rs`; observation JSONL parsing/import shaping lives under `observation.rs`; `channel.rs` and `cozo_store.rs` are back under the rough LOC bound, and `evidence.rs` is limited to shared evidence/row vocabulary plus row constructors.
- Commit: current hygiene commit, `refactor: split eval store channel modules`.

### Slice 8c — Parent channel receipt/import eval mirror

- Status: complete for parent-side `Ready`/`Evaluating` receive-path receipt/import mirrors; terminal `Result`, `ResultWritten`, failure/exit, successor handoff, and provider-dependent child terminal-result surfaces remain deferred.
- Assumptions:
  - `Channel<Parent<_>, T>::recv_from_child` remains the parent read authority. DB rows do not synthesize messages, advance cursors, validate payloads, or replace `FileTransport`/channel envelope files.
  - Receipt/import rows are written only after `read` has decoded the envelope and verified endpoint identity plus body hash.
  - A parent-visible import is represented separately from the channel message itself: `eval_channel_receipt` records the parent observation/validation, and `eval_import_event` records the child-runtime-to-parent-visible boundary.
  - This sub-slice uses `observed_by = "parent"` and `importer_id = "parent"` because the current file channel does not carry a separate parent runtime id. `source_runtime_id` remains the child runtime id and source/target scopes are explicit.
  - The new `ChannelReceiptEvidence` and `ImportEventEvidence` carriers are persisted relation contracts, not test-only wrappers; no existing typed carrier represented these schemas.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/channel/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/evidence.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_params.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_schema.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_store.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
- Tests added/changed:
  - `prototype1_eval_store_parent_ready_read_writes_receipt_and_import_rows`
  - `prototype1_storage_authority_negative_receipt_import_rows_cannot_replace_envelope`
  - `prototype1_storage_authority_negative_bad_body_hash_writes_no_receipt_or_import`
  - existing `prototype1_state::channel::tests`
  - existing `prototype1_eval_store_parent_start_db_schema_installs_idempotently`
- Commands run:
  - `gitnexus impact ... recv_from_child`: MEDIUM risk; 6 direct callers, 9 impacted symbols, no affected execution flows, all in `Prototype1_state`.
  - `gitnexus impact ... install_schema`: MEDIUM risk; 8 direct callers, 20 impacted symbols, no affected execution flows, all in `Eval_store`.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_state::channel::tests -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently -- --nocapture`
  - `gitnexus detect_changes`: LOW risk; 8 changed files, 0 affected execution flows.
- Live API used: no. This sub-slice mirrors local parent channel reads after validation and does not change provider execution, treatment evaluation, or terminal result production.
- Result: `eval_channel_receipt` and `eval_import_event` schemas now install idempotently. Parent `recv_from_child` writes receipt/import rows only after a valid child-to-parent envelope is read from the channel. DB receipt/import rows cannot replace a missing envelope, and a body-hash mismatch fails before any receipt/import row is written.
- Commit: current slice commit, `feat: mirror parent channel receipt imports`.

### Slice 8d — Remaining non-provider channel message mirrors

- Status: complete for non-provider `ToParent` channel-message mirrors: `ResultWritten`, `Failed`, `Exited`, `SuccessorReady`, and `SuccessorCompletion`. Direct terminal `Result` remains deferred because it is tied to provider-produced treatment evidence.
- Assumptions:
  - The JSONL `Envelope<ToParent>` remains the transport authority. DB `eval_channel_message` rows do not synthesize messages, advance cursors, replace `FileTransport`, or make a successor/child observable without a channel envelope.
  - These sends now reuse the same append-first, mirror-after-append behavior already used by `Ready` and `Evaluating`.
  - Successor ready/completion record payloads use the existing passive `record_runtime_id` projection where the persisted record schema requires `ploke_records::ids::RuntimeId`; the channel endpoint runtime id remains the transport identity.
  - Direct terminal `send_terminal_result` is not changed in this sub-slice; provider-facing terminal result confidence remains deferred to the provider-gated terminal-result sub-slice.
  - Channel mirror helpers were split into `channel/mirror.rs` because `channel.rs` crossed the rough LOC hygiene bound during this sub-slice.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/channel/mirror.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/channel/tests.rs`
- Tests added/changed:
  - `prototype1_eval_store_remaining_channel_messages_write_owner_db_rows`
  - `prototype1_storage_authority_negative_successor_channel_row_cannot_replace_envelope`
  - existing `prototype1_state::channel::tests`
  - existing `observe_child_ignores_result_written_projection_for_success`
- Commands run:
  - `gitnexus impact ... send_result_written`: LOW risk; 1 direct caller, 0 affected execution flows.
  - `gitnexus impact ... send_failed`: LOW risk; 0 direct callers, 0 affected execution flows.
  - `gitnexus impact ... send_exited`: LOW risk; 0 direct callers, 0 affected execution flows.
  - `gitnexus impact ... send_successor_ready`: LOW risk; 1 direct caller, affected process `r4a_to_r4b_or_r4c`.
  - `gitnexus impact ... send_successor_completion`: LOW risk; 1 direct caller, 0 affected execution flows.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_remaining_channel_messages_write_owner_db_rows -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_successor_channel_row_cannot_replace_envelope -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_state::channel::tests -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval observe_child_ignores_result_written_projection_for_success -- --nocapture`
  - `gitnexus detect_changes`: LOW risk; 0 affected execution flows.
- Live API used: no. This sub-slice mirrors existing local channel sends and does not change treatment execution or direct terminal `Result` production.
- Result: `ResultWritten`, `Failed`, `Exited`, `SuccessorReady`, and `SuccessorCompletion` now write `eval_channel_message` rows with explicit message kinds after the channel append succeeds. The result-written consumer guard still proves the compatibility projection does not become success authority. A successor-specific channel-message row still cannot replace the missing channel envelope. Channel mirror helpers now live in `channel/mirror.rs`, leaving `channel.rs` under the rough LOC bound.
- Commit: current slice commit, `feat: mirror remaining channel messages`.

### Slice 8e — Child invocation attempt mirror

- Status: complete for child invocation-backed `eval_attempt` rows. Terminal `ToParent::Result` remains blocked/deferred because the existing live Google terminal-result test is intentionally quarantined and cannot satisfy the provider-facing confidence gate.
- Assumptions:
  - `eval_attempt` is passive evidence. It records the attempt occurrence after the executable invocation file is written and must not replace invocation/bootstrap authority.
  - For this first implementation, `attempt_id == runtime_id`, matching the planning note that keeps the concepts distinct while allowing the initial row key to reuse runtime identity.
  - Child invocation rows are the source carrier for this sub-slice. Existing edit-surface/harness attempt types are flow-specific and do not represent the base `eval_attempt` relation.
  - Successor invocation mirroring to `eval_attempt` is not changed here; GitNexus reported HIGH risk through successor spawn/handoff flows, so that remains a separate bounded sub-slice.
  - Direct terminal `send_terminal_result` is provider-facing. The current `live_google_child_runner_success` test panics intentionally before exercising the live path, so this log treats terminal-result confidence as blocked rather than passed.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/evidence.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_params.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_schema.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_store.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`
- Tests added/changed:
  - existing `prototype1_eval_store_child_invocation_writes_owner_db_row` now asserts linked `eval_attempt` content.
  - existing `prototype1_storage_authority_negative_invocation_row_cannot_replace_file` now confirms a DB attempt row also cannot replace the executable invocation file.
  - existing `prototype1_eval_store_parent_start_db_schema_installs_idempotently` now asserts the `eval_attempt` relation installs idempotently.
- Commands run:
  - `gitnexus impact ... install_schema`: HIGH risk; 8 direct callers, 25 impacted symbols, no affected execution flows. Treated as requiring schema and focused eval-store verification.
  - `gitnexus impact ... write_child_invocation`: LOW risk; 3 direct callers, affected process `live_google_child_runner_success`.
  - `gitnexus impact ... write_successor_invocation`: HIGH risk through successor handoff/spawn flows; no successor edits made in this sub-slice.
  - `gitnexus impact ... send_terminal_result`: LOW risk; provider-facing terminal-result path inspected but not edited because live confidence is blocked by the quarantined test.
  - `cargo fmt --all`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_child_invocation_writes_owner_db_row -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_invocation_row_cannot_replace_file -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
- Live API used: no. This sub-slice mirrors local child invocation evidence. The provider-facing terminal-result path is explicitly blocked/deferred until a live Google/direct-Google gate can run instead of the quarantined `live_google_child_runner_success` test.
- Result: `eval_attempt` now installs in the owner eval DB schema. Child invocation writes derive a passive attempt row linked to the invocation id, with `attempt_id` equal to the runtime id and `status = invocation_written`. The authority-negative test still proves DB invocation/attempt rows cannot launch a missing executable invocation file.
- Commit: `feat: mirror child invocation attempts` (`7e57ff77`).

### Slice 8f — Successor invocation attempt mirror

- Status: complete for successor invocation-backed `eval_invocation`/`eval_attempt` rows. Terminal `ToParent::Result` remains blocked/deferred behind the provider-facing live gate.
- Assumptions:
  - The retired-parent gate for successor invocation construction remains the authority boundary. This sub-slice only mirrors after `write_invocation` has created the executable successor invocation file.
  - The DB mirror is passive evidence. It does not permit handoff, spawn, or successor startup without the executable invocation file.
  - The same first-pass `attempt_id == runtime_id` rule applies to successor bootstrap attempts.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/invocation.rs`
  - `docs/active/agents/2026-06-22_prototype1-eval-store-data-model/implementation-log.md`
- Tests added/changed:
  - `prototype1_eval_store_successor_invocation_writes_owner_db_rows`
  - `prototype1_storage_authority_negative_successor_attempt_row_cannot_replace_file`
  - existing `prototype1_state::invocation::tests`
- Commands run:
  - `gitnexus impact ... write_successor_invocation`: HIGH risk through successor handoff/spawn flows; edit kept to append-first passive mirroring after executable invocation write.
  - `cargo fmt --all`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_successor_invocation_writes_owner_db_rows -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_storage_authority_negative_successor_attempt_row_cannot_replace_file -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_state::invocation::tests -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
- Live API used: no. This sub-slice mirrors local successor bootstrap invocation evidence after the invocation file write and does not change provider execution or terminal result production.
- Result: successor invocation writes now use the same owner-DB mirror path as child invocations, with role-specific `eval_invocation` and `eval_attempt` rows. The successor authority-negative test proves DB rows cannot replace a missing executable invocation file.
- Commit: `feat: mirror successor invocation attempts` (`7534eae9`).

### Slice 8g — Terminal result live-gate blocker

- Status: blocked/deferred for direct terminal `ToParent::Result` channel-message mirroring.
- Assumptions:
  - `send_terminal_result` is the provider-facing terminal child result producer for successful treatment evidence and therefore requires `F5_child_terminal_result` live provider confidence before the sub-slice can be accepted.
  - Deterministic/local terminal channel tests can prove channel authority mechanics, but they cannot substitute for the provider-facing F5 confidence checkpoint.
  - The existing live Google child-runner success test remains intentionally quarantined with an unconditional panic before it exercises the child runner. That test cannot serve as live confidence for terminal-result production.
  - Ambient provider route variables are present for Google project/region/API key, but the suite opt-in variables are not ambient; even with opt-in, the current live F5 test body would panic before provider execution.
- Evidence:
  - `crates/ploke-eval/src/cli/prototype1_state/channel.rs`: `send_terminal_result` writes direct `ToParent::Result { runner_result, treatment }`.
  - `crates/ploke-eval/src/cli/prototype1_process.rs`: `live_google_child_runner_success` is ignored and starts with an unconditional quarantine panic.
  - `slice-by-slice-implementation-plan.md`: `F5_child_terminal_result` requires live API for provider-dependent treatment execution / terminal result production.
- Code touched: none.
- Tests run: none; the relevant live test is source-blocked by quarantine.
- Live API used: no. This entry records that missing provider confidence is not accepted.
- Result: terminal `ToParent::Result` mirroring remains an explicit provider-gated follow-up. Slice 9 downstream work may proceed only from existing/restored F5 evidence and must not claim terminal-result producer confidence until the live F5 gate is repaired and run.
- Commit: `docs: record terminal result live gate blocker` (`e4114429`).

### Slice 9a — Parent comparison evaluation summary rows

- Status: complete for `eval_evaluation` and `eval_evaluation_instance` rows written after parent comparison artifacts.
- Assumptions:
  - `Prototype1BranchEvaluationReport` remains the source carrier and `prototype1/evaluations/<branch-id>.json` remains the artifact authority. The DB rows are passive query evidence only.
  - This sub-slice starts from existing/restored F5 terminal treatment evidence. It does not modify terminal result production, so no live provider call is required for this writer.
  - `CompleteBaseline` carries parent node/branch ids, not `ParentIdentity.parent_id`. This first-pass row leaves `parent_id = null` rather than flattening node identity into the parent-id column.
  - Evaluation row identity includes the evaluation artifact content hash so rewritten branch reports do not silently overwrite earlier evidence.
  - Evaluation-specific schema/write code lives in `eval_store/evaluation.rs` to avoid growing the already large shared eval-store modules further.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/evaluation.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_schema.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_store.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/run/core.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - existing `parent_compares_treatment_evidence_against_owned_baseline` now calls the production comparison writer and asserts `eval_evaluation` / `eval_evaluation_instance` DB content.
  - `evaluation_db_rows_do_not_replace_file_backed_report`
  - existing `succeeded_child_without_evaluation_stays_in_observe`
  - existing `succeeded_child_with_evaluation_can_enter_selection`
  - existing `prototype1_eval_store_parent_start_db_schema_installs_idempotently`
- Commands run:
  - `gitnexus impact ... compare_observed_child_treatment`: HIGH risk; 4 direct callers, 18 impacted symbols, no affected execution flows. Edit kept artifact/branch-log authority first and DB mirror second.
  - `gitnexus impact ... install_schema`: HIGH risk; 8 direct callers, 25 impacted symbols, no affected execution flows. Schema coverage added.
  - `cargo fmt --all`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval parent_compares_treatment_evidence_against_owned_baseline -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval evaluation_db_rows_do_not_replace_file_backed_report -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval succeeded_child_without_evaluation_stays_in_observe -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval succeeded_child_with_evaluation_can_enter_selection -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
- Live API used: no. Parent comparison is local when starting from existing terminal treatment evidence; provider confidence for terminal result production remains blocked in Slice 8g.
- Result: parent comparison now writes queryable evaluation summary rows after the JSON evaluation artifact and branch comparison log succeed. Selection reconstruction still refuses to proceed without a file-backed/in-memory evaluation report, proving DB evaluation rows do not replace evaluation authority.
- Commit: `feat: mirror parent evaluation summaries`.

### Slice 9b — Continuation decision summary rows

- Status: complete for `eval_continuation_decision` rows written after `live_successor_continuation_decision` computes the typed continuation result.
- Assumptions:
  - `Prototype1ContinuationDecision` remains the source carrier. The new eval-store evidence struct is a private DB row carrier, not a second continuation model.
  - Continuation rows are passive query evidence. They do not replace transition-journal successor authority, History traversal, active checkout advancement, or successor startup validation.
  - The mirror is owner-DB opportunistic: if the campaign-local `prototype1/eval-store.cozo.sqlite` file does not exist, continuation decision behavior remains unchanged.
  - Selection decision/candidate/finding/score rows remain a separate Slice 9 follow-up because they depend on `SelectionSealMaterial` and History candidate-set evidence, not only the continuation result.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/continuation.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_schema.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - `continuation_decision_mirrors_owned_eval_store_row_without_successor_authority`
  - existing `prototype1_eval_store_parent_start_db_schema_installs_idempotently`
  - existing `historical_selection_can_continue_when_unspent_and_bounded`
- Commands run:
  - `gitnexus impact ... live_successor_continuation_decision`: HIGH risk; 4 direct callers, affected processes `r12_to_r13` and `reconstruct_early`. Edit kept decision computation unchanged and added only a post-decision passive mirror.
  - `gitnexus impact ... install_schema`: HIGH risk; 8 direct callers, no affected execution flows. Schema coverage added.
  - `cargo fmt --all`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval continuation_decision_mirrors_owned_eval_store_row_without_successor_authority -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval historical_selection_can_continue_when_unspent_and_bounded -- --nocapture`
  - `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
- Live API used: no. This sub-slice mirrors local parent-side continuation decisions and does not modify provider execution or terminal result production.
- Result: continuation decisions now install and write queryable `eval_continuation_decision` rows when the owner eval DB exists. The authority-negative test proves the passive row does not append a successor journal entry or replace successor transition authority.
- Commit: `feat: mirror continuation decision summaries`.

### Slice 9c — Selection decision and candidate summary rows

- Status: complete for `eval_selection_decision` and `eval_selection_candidate` rows written after `select_successor_for_profile` returns a typed successor selection.
- Assumptions:
  - `SelectionDecisionEntry` remains the source carrier for sealed selection evidence. The eval-store evidence type is only a private DB row carrier for Cozo writes.
  - Selection rows are passive query evidence. They do not replace sealed History admission, transition-journal successor authority, active checkout advancement, or successor startup validation.
  - This sub-slice persists the decision and candidate membership rows only. `eval_selection_finding` and `eval_selection_score` remain follow-up work because they need a separate mapping from domain findings and metric formula rows.
  - `set_id` is the candidate-set root when available, otherwise the considered-order hash. Candidate `member_id` prefers membership id, then occurrence id, then payload hash.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/selection.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_schema.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - existing `historical_node_150_channel_treatment_reaches_current_generation_handoff` now seeds an owner eval DB, asserts selection decision/candidate rows, and proves those passive rows do not append a successor journal entry.
  - existing `prototype1_eval_store_parent_start_db_schema_installs_idempotently`
- Commands run:
  - `gitnexus impact ... select_successor_for_profile`: LOW risk; 3 direct callers, affected processes `live_google_step_child_plan` and `live_google_parallel_slots`.
  - `gitnexus impact ... install_schema`: HIGH risk; 8 direct callers, no affected execution flows. Schema coverage added.
  - `gitnexus impact ... SelectionSealMaterial`: HIGH risk through selection/reconstruction. Change limited to deriving `Clone` so the existing sealed carrier can be reused for passive DB rows without consuming the live handoff material.
  - `cargo fmt --all`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval historical_node_150_channel_treatment_reaches_current_generation_handoff -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently -- --nocapture`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval`
- Live API used: no. This sub-slice mirrors local parent-side selection evidence and does not modify provider execution or terminal result production.
- Result: selection now writes queryable decision and candidate rows when the owner eval DB exists. The handoff test still exercises the real artifact selection path, and the journal assertion proves DB rows do not replace successor transition authority.
- Commit: `feat: mirror selection decision summaries`.

### Slice 9d — Selection finding and score summary rows

- Status: complete for `eval_selection_finding` and `eval_selection_score` rows derived from sealed selection entries.
- Assumptions:
  - `DomainFinding` remains the source carrier for finding rows, and `SelectionFormula` remains the source carrier for score rows. The eval-store row structs are private Cozo carriers only.
  - Score rows are emitted when the sealed selection entry carries a formula. Strategies without formula rows remain valid and simply write no `eval_selection_score` rows.
  - Finding and score rows are passive query evidence. They do not replace selection decisions, History admission, successor transition authority, or artifact handoff validation.
  - `eval_selection_score.rank` is a first-pass stable payload index for scored rows, not a semantic ordering claim across formulas.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/selection.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - existing `historical_node_150_channel_treatment_reaches_current_generation_handoff` now uses `history-score-child-prop` and asserts persisted operational finding plus selected score rows.
  - existing `prototype1_eval_store_parent_start_db_schema_installs_idempotently`
- Commands run:
  - `gitnexus impact ... install_schema`: HIGH risk; 8 direct callers, no affected execution flows. Schema coverage extended for finding and score relations.
  - `cargo fmt --all`
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval historical_node_150_channel_treatment_reaches_current_generation_handoff -- --nocapture` failed first with `E0382` from moving `decision_id`; fixed by cloning for earlier query params.
  - test-runner sub-agent: same focused handoff test failed next because finding rows had count 2 while the test assumed 1; fixed by querying the operational finding row explicitly.
  - test-runner sub-agent: same focused handoff test passed.
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently -- --nocapture` passed.
  - test-runner sub-agent: `TMPDIR=$PWD/target/tmp cargo check -p ploke-eval` passed.
- Live API used: no. This sub-slice mirrors sealed local selection evidence and does not modify provider execution or terminal result production.
- Result: selection finding and score evidence now lands in queryable eval-store relations when the owner DB exists. The handoff test still validates the real artifact selection path and still asserts passive DB rows do not replace successor journal authority.
- Commit: `feat: mirror selection finding scores`.

### Slice 10a — Broad harness materialization artifact provenance rows

- Status: complete for broad-harness `C1 -> C2` materialization provenance rows: `eval_artifact`, `eval_artifact_surface`, and `eval_artifact_ref`.
- Assumptions:
  - This sub-slice only mirrors artifact provenance after `transition_with_harness` validates the harness workspace, validates changed paths, measures the artifact surface, updates child projections, and appends the After journal entry.
  - The generic `WorkspaceBackend` C1 path, child build/spawn binary refs, patch/apply/build operation rows, and selected-artifact active-checkout install remain later Slice 10 work.
  - Owner DB presence is the opt-in boundary for this mirror. If `prototype1/eval-store.cozo.sqlite` does not already exist, materialization remains unchanged and no DB file is created by C1.
  - DB artifact rows are passive evidence only. They do not prove candidate workspace existence, backend materialization, surface validity, selected artifact admission, or active checkout install.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/c1.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/artifact.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_schema.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - existing `broad_harness_multi_file_admission_mints_one_artifact_child` now seeds an owner eval DB and asserts the derived artifact row, artifact surface row, and child artifact ref row.
  - new `broad_harness_artifact_rows_do_not_replace_missing_candidate_workspace` seeds matching artifact rows, removes the candidate workspace, and asserts `HarnessWorkspaceMissing` before any C1 journal entry is written.
  - existing `prototype1_eval_store_parent_start_db_schema_installs_idempotently` now asserts artifact relation installation.
- Commands run:
  - `gitnexus impact ... transition_with_harness`: HIGH risk; 4 direct callers, affected modules `Prototype1_state`, `Run`, and tests. Edit kept the mirror after existing harness workspace/surface validation and after journaled C2 materialization.
  - `gitnexus impact ... install_schema`: HIGH risk; 8 direct callers, no affected execution flows. Schema coverage extended with idempotent artifact relations.
  - `cargo fmt --all`
  - test-runner sub-agent: `cargo check -p ploke-eval` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently -- --nocapture` first failed on missing test imports, then passed after importing `CommitError` and `MaterializeBranchError`.
  - test-runner sub-agent: `cargo test -p ploke-eval broad_harness_multi_file_admission_mints_one_artifact_child -- --nocapture` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval broad_harness_artifact_rows_do_not_replace_missing_candidate_workspace -- --nocapture` passed.
- Live API used: no. This sub-slice mirrors local backend/harness materialization evidence and does not modify provider execution, child generation, or handoff/selected-artifact install.
- Result: broad-harness child materialization now writes queryable artifact provenance rows when the owner eval DB exists. The authority-negative test proves artifact rows do not replace candidate workspace/backend authority.
- Commit: `feat: mirror materialized artifact provenance`.

### Slice 10b — Child build and promoted binary provenance rows

- Status: complete for successful `C2 -> C3` build/promotion provenance rows: `eval_build_event` and `eval_binary_ref`.
- Assumptions:
  - This sub-slice mirrors only successful child build promotion after the promoted binary exists and the C2 After journal entry is appended.
  - Failed cargo-check/build events, C3 spawn runtime refs, patch/apply operation rows, and selected-artifact active-checkout install remain later Slice 10 work.
  - Owner DB presence remains the opt-in boundary. If `prototype1/eval-store.cozo.sqlite` does not already exist, C2 build behavior is unchanged and no DB file is created by the build transition.
  - DB binary refs are passive evidence only. They do not prove a binary file exists or allow C3 spawn/invocation bootstrap to proceed without filesystem validation.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/c2.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/c3.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/build.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_schema.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - existing `child_build_promotes_binary_and_cleans_scratch` now seeds an owner eval DB and asserts the build event row and binary ref row.
  - new `binary_ref_rows_do_not_replace_missing_promoted_binary` seeds matching binary/build rows, deletes the promoted binary, and asserts `MissingChildBinary` before any spawn journal or invocation bootstrap write.
  - existing `prototype1_eval_store_parent_start_db_schema_installs_idempotently` now asserts build/binary relation installation.
- Commands run:
  - `gitnexus impact ... BuildChild.transition`: LOW risk; no direct upstream callers found by the index.
  - `gitnexus impact ... SpawnChild.transition`: LOW risk; no direct upstream callers found by the index.
  - `gitnexus impact ... install_schema`: HIGH risk; 8 direct callers, no affected execution flows. Schema coverage extended with idempotent build/binary relations.
  - `cargo fmt --all`
  - test-runner sub-agent: `cargo check -p ploke-eval` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently -- --nocapture` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval child_build_promotes_binary_and_cleans_scratch -- --nocapture` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval binary_ref_rows_do_not_replace_missing_promoted_binary -- --nocapture` passed.
- Live API used: no. This sub-slice uses local fake-cargo build tests and does not modify provider execution, child generation, terminal result production, or selected-artifact handoff.
- Result: successful C2 child build promotion now writes queryable build and binary provenance rows when the owner eval DB exists. The authority-negative test proves binary refs do not replace filesystem binary authority for C3 spawn.
- Commit: `feat: mirror child build provenance`.

### Slice 10c — Broad harness patch and apply operation provenance rows

- Status: complete for broad-harness `C1 -> C2` materialization provenance rows: `eval_operation`, `eval_patch`, and `eval_apply_event`.
- Assumptions:
  - This sub-slice only mirrors operation, patch, and apply-event provenance for the broad-harness materialization path after the candidate workspace is validated, artifact surface evidence is accepted, child projections are updated, and the C1 After journal entry is appended.
  - Existing `Prototype1NodeRecord`, `ResolvedTreatmentBranch`, and harness child evidence remain the authority carriers for patch/apply/materialization facts. The new eval-store structs are private row evidence for normalized Cozo writes only.
  - Owner DB presence remains the opt-in boundary. If `prototype1/eval-store.cozo.sqlite` does not already exist, C1 behavior is unchanged and no DB file is created by materialization.
  - Operation, patch, and apply rows are passive evidence only. They do not prove candidate workspace existence, backend materialization, changed-path validity, or selected-artifact install authority.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/c1.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/operation.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/cozo_schema.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/eval_store/tests.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - existing `broad_harness_multi_file_admission_mints_one_artifact_child` now asserts operation, patch, and apply-event rows alongside the artifact provenance rows.
  - renamed/extended authority-negative test `broad_harness_eval_store_rows_do_not_replace_missing_candidate_workspace` seeds artifact plus operation/patch/apply rows, removes the candidate workspace, and asserts `HarnessWorkspaceMissing` before any C1 journal entry is written.
  - existing `prototype1_eval_store_parent_start_db_schema_installs_idempotently` now asserts operation, patch, and apply-event relation installation.
- Commands run:
  - `gitnexus impact ... transition_with_harness`: HIGH risk; 4 direct callers, impacted count 17, no affected execution flows. Edit kept the new mirror after existing harness workspace/surface validation and after journaled C2 materialization.
  - `gitnexus impact ... install_schema`: HIGH risk; 8 direct callers, no affected execution flows. Schema coverage extended with idempotent operation/patch/apply-event relations.
  - `cargo fmt --all`
  - test-runner sub-agent: `cargo check -p ploke-eval` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval prototype1_eval_store_parent_start_db_schema_installs_idempotently -- --nocapture` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval broad_harness_multi_file_admission_mints_one_artifact_child -- --nocapture` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval broad_harness_eval_store_rows_do_not_replace_missing_candidate_workspace -- --nocapture` passed.
- Live API used: no. This sub-slice mirrors local backend/harness materialization evidence and does not modify provider execution, child generation, terminal result production, or selected-artifact handoff.
- Result: broad-harness child materialization now writes queryable operation, patch, and apply-event provenance rows when the owner eval DB exists. The authority-negative test proves eval-store rows do not replace candidate workspace/backend authority.
- Commit: `feat: mirror broad harness operation provenance`.

### Slice 10d — Child spawn binary provenance rows

- Status: complete for successful `C3 -> C4` spawn provenance rows using `eval_build_event` and `eval_binary_ref`.
- Assumptions:
  - This sub-slice mirrors only successful child spawn acknowledgement after the promoted binary exists, invocation/bootstrap is written, the child ready message is observed, node status is updated to running, and the observed spawn journal entry is appended.
  - The spawn row uses `eval_build_event.phase = "spawn"` and `outcome = "acknowledged"` to keep build promotion evidence separate from runtime hydration evidence.
  - Owner DB presence remains the opt-in boundary. If `prototype1/eval-store.cozo.sqlite` does not already exist, C3 behavior is unchanged and no DB file is created by the spawn transition.
  - Spawn binary refs are passive evidence only. They do not prove the promoted binary file exists, do not replace invocation/bootstrap authority, and do not authorize C3 spawn.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_state/c3.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs`
- Tests added/changed:
  - existing `child_spawn_observes_ready` now seeds an owner eval DB and asserts the spawn build-event row plus runtime-specific binary ref row.
  - existing `binary_ref_rows_do_not_replace_missing_promoted_binary` now seeds both promote and spawn binary provenance rows, deletes the promoted binary, and still asserts `MissingChildBinary` before spawn journal or invocation bootstrap writes.
- Commands run:
  - `gitnexus impact ... SpawnChild.transition`: LOW risk; no direct upstream callers or affected execution flows found by the index.
  - `gitnexus impact ... child_build_promotes_binary_and_cleans_scratch`: LOW risk; test-only target.
  - `gitnexus impact ... binary_ref_rows_do_not_replace_missing_promoted_binary`: target not found by index; treated as local test-only scope.
  - `cargo fmt --all`
  - test-runner sub-agent: `cargo check -p ploke-eval` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval child_spawn_observes_ready -- --nocapture` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval binary_ref_rows_do_not_replace_missing_promoted_binary -- --nocapture` passed.
- Live API used: no. This sub-slice uses local fake-cargo spawn tests and does not modify provider execution, child generation, terminal result production, or selected-artifact handoff.
- Result: successful C3 child spawn now writes queryable spawn/binary provenance rows when the owner eval DB exists. The authority-negative test proves preexisting spawn/binary DB rows do not replace filesystem binary authority.
- Commit: `feat: mirror child spawn binary provenance`.

### Slice 10e — Selected successor active-checkout install provenance rows

- Status: complete for selected-artifact active-checkout install provenance rows using `eval_artifact`, `eval_artifact_surface`, and `eval_artifact_ref`.
- Assumptions:
  - This sub-slice mirrors only after `install_committed_successor_artifact` verifies the selected branch tree, verifies target content, switches the active checkout, commits selected parent identity, validates parent checkout authority, verifies target content again, appends successor checkout before/after records, and appends `ActiveCheckoutAdvanced`.
  - The active checkout install remains artifact/backend authority. Eval-store rows do not replace checkout existence, branch tree validation, parent identity validation, History sealing, successor invocation, or successor ready acknowledgement.
  - Owner DB presence remains the opt-in boundary. If `prototype1/eval-store.cozo.sqlite` does not already exist, handoff install behavior is unchanged and no DB file is created by install.
  - The mirror uses the existing selected `ArtifactRef` handle, installed branch, installed commit, selected artifact surface hash, and `active_checkout:<root>@<commit>` source ref.
- Code touched:
  - `crates/ploke-eval/src/cli/prototype1_process.rs`
- Tests added/changed:
  - existing `successor_install_commits_selected_parent_identity` now seeds an owner eval DB and asserts the selected successor artifact row plus active-checkout artifact ref row.
  - new `successor_install_rows_do_not_replace_missing_active_checkout` seeds matching install rows, deletes the active checkout, and asserts backend install still fails before any DB row can stand in for checkout authority.
- Commands run:
  - `gitnexus impact ... install_committed_successor_artifact`: HIGH risk; 3 direct callers, affected process `spawn_and_handoff_prototype1_successor`. Edit kept the mirror after existing backend validation and checkout journal writes.
  - `gitnexus impact ... install_prototype1_successor_artifact`: HIGH risk; affected processes `spawn_and_handoff_prototype1_successor` and `r12_to_r13`. The actual edit is in the lower helper after artifact install, not in selection or continuation decision logic.
  - `gitnexus impact ... successor_install_commits_selected_parent_identity`: LOW risk; test-only target.
  - `cargo fmt --all`
  - test-runner sub-agent: `cargo check -p ploke-eval` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval successor_install_commits_selected_parent_identity -- --nocapture` passed.
  - test-runner sub-agent: `cargo test -p ploke-eval successor_install_rows_do_not_replace_missing_active_checkout -- --nocapture` passed.
- Live API used: no. This sub-slice uses local git/checkout install tests and does not modify provider execution, child generation, child terminal result production, or successor ready behavior.
- Result: selected successor active-checkout install now writes queryable artifact provenance rows when the owner eval DB exists. The authority-negative test proves install rows do not replace active checkout/backend authority.
- Commit: `feat: mirror selected successor install provenance`.

### Slice 11 — DB-only readiness review

- Status: complete as an audit gate. DB-only runtime enablement is not ready and was not enabled.
- Assumptions:
  - Slice 11 is a read-path and authority-boundary review, not a producer transition slice.
  - Eval-store rows from Slices 1-10 are useful for query/review evidence, but they remain passive unless a production consumer is explicitly migrated and tested.
  - History, Channel, MessageBox, invocation/bootstrap, artifact/worktree mutation, and transition-journal replay remain protected authority domains and are not candidates for generic eval-store DB-only replacement.
- Documents touched:
  - `docs/active/agents/2026-06-22_prototype1-eval-store-data-model/slice-11-db-only-readiness-audit.md`
  - `docs/active/agents/2026-06-22_prototype1-eval-store-data-model/README.md`
  - `docs/active/agents/2026-06-22_prototype1-eval-store-data-model/implementation-log.md`
- Audit findings:
  - Active runnable-node discovery no longer reads `scheduler.json`, `branches.json`, or `selection.json` to infer candidates, but scheduler compatibility and admitted-profile fallback paths still retain filesystem reads.
  - `node.json` and latest `runner-result.json` remain filesystem compatibility authority; existing authority-negative tests prove DB record refs cannot replace the missing files.
  - `branches.json` remains the branch registry read surface through `load_or_default_branch_registry`.
  - Invocation JSON remains executable bootstrap authority; existing authority-negative tests prove invocation/attempt DB rows cannot replace the executable invocation file.
  - Channel terminal evidence still requires `Channel<FileTransport>` reads and agreement with runner results.
  - Child-plan MessageBox files, transition journal JSONL, sealed History, Crown, and artifact/worktree mutation remain protected authority paths.
- Commands run:
  - GitNexus query/read-path audit over scheduler, runner-result, branch registry, invocation, channel, MessageBox, transition journal, and History surfaces.
  - `cargo fmt --all`
  - `git diff --check`
  - test-runner sub-agent: `cargo check -p ploke-eval` passed.
- Live API used: no. This slice does not modify provider-facing producers and does not need live provider confidence.
- Result: no DB-only production runtime mode was enabled. The next implementation work should be consumer migration/checkpoint parity for selected compatibility surfaces, not a permissive DB-only flag.
- Commit: `docs: record db-only readiness audit`.

### Later slices

Create a new subsection per slice before editing.
