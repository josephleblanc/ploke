# Changelog

Date: 2026-06-08.

## 2026-06-08

- Added ADR packet for Prototype 1 guided edit surface behavior.
- Recorded current baseline gate status: `cargo test --workspace 2>&1 | rg -A
  8 E0` returned pipeline exit `1`; cargo itself exited `101`; `rg` found no
  `E0` lines and therefore hid the concrete cargo failure. An unfiltered
  follow-up test probe is required before implementation proceeds.
- Unfiltered follow-up showed dependency resolution failed before tests ran:
  `ploke-db` requested `ploke-transform/typed_type_graph`, but
  `ploke-transform` no longer declared that feature. The downstream feature
  contract also still requests `syn_parser/typed_type_graph`, so the baseline
  repair restored the historical `typed_type_graph` feature declaration in
  `syn_parser` and the `ploke-transform -> syn_parser` feature bridge.
- After the feature repair, `cargo test --workspace` advanced through multiple
  crates and then failed in `ploke-llm` live Google tests because Google
  application default credentials could not be resolved. The first failing test
  was
  `router_only::google::tests::live_google_chat_completions_smoke_success_or_quota`
  at `crates/ploke-llm/src/router_only/google/mod.rs:1220`.
- Type-reuse decision: reuse existing `edit_surface` carriers, run-profile
  DTOs, History/selection DTOs, and persisted record shapes. Do not introduce a
  parallel request JSON schema for the planning stage unless an existing
  carrier cannot represent the contract.
- Implemented request-carrier support for guided broad harness planning:
  `PublishedBroadHarnessRequest` now carries structured planning guidance,
  graph-restriction metadata, and a parent-side pre-child planning artifact
  path. Published prompts direct the patching model to read the planner
  artifact and keep edits inside the graph-neighborhood policy.
- Implemented the parent-side pre-child planning sequence in the broad harness
  path. The parent publishes the request batch, writes a planner prompt, then
  runs a direct-Google planner call before broad TUI patch generation starts.
  Test builds use a deterministic `test_stub` artifact instead of a live
  provider call.
- Added `execution.broad_tui.graph_nearest` to the active run profile and the
  passive `ploke-records` run-profile DTO. The value controls the N in the
  request graph-neighborhood policy and rejects zero at profile validation.
- Added a parallel Cozo record mirror under
  `$PLOKE_EVAL_HOME/records/mirror.cozo.sqlite`. `JsonRecordFile` writes the
  original file authority first, then mirrors the record family, schema,
  format, path, content hash, timestamp, and JSON payload into the separate
  Cozo DB.
- Added report-only missing-data aggregation to `prototype1 score-selection`
  review output: total missing diagnostics, rows with missing data,
  missing-selection rows, projection failures, and missing counts by field.
- Verification after implementation:
  - `cargo check -p ploke-eval` passed.
  - `cargo test -p ploke-eval graph_limited_publication_carries_planning_prompt_contract -- --nocapture` passed.
  - `cargo test -p ploke-eval published_request_binds_isolated_workspace_and_submitted_result -- --nocapture` passed.
  - `cargo test -p ploke-eval run_profile_execution_rejects_zero_broad_tui_graph_nearest -- --nocapture` passed.
  - `cargo test -p ploke-eval score_selection_report_summarizes_missing_runtime_data -- --nocapture` passed.
  - `cargo test -p ploke-eval json_record_emission_writes_parallel_cozo_mirror -- --nocapture` passed.
  - `cargo test -p ploke-eval pre_child_planning_review_writes_prompt_and_artifact_before_admission -- --nocapture` passed.
  - `cargo test -p ploke-records run_profile_toml_roundtrips_current_shape -- --nocapture` passed.
- A full `cargo test --workspace` probe then failed in `ploke-eval --lib`
  with parallel Cozo SQLite contention: first listed failure was
  `cli::prototype1_state::cli_facing::tests::broad_harness_rejects_unbound_existing_child_plan`
  and the highest-signal error was `database is locked`. Added a process-local
  mutex around the record-mirror Cozo open/schema/write path. Focused reruns of
  the previously first failing test, the record mirror test, and the planner
  sequencing test passed after the lock.
- `cargo test -p ploke-eval --lib` passed after the lock:
  `798 passed; 0 failed; 26 ignored`.
- Full workspace verification passed after the lock: `cargo test --workspace`
  exited `0`. The previous Cozo SQLite lock failure did not recur, and the
  Google direct live tests that ran in the workspace pass were successful.
- Fresh handoff validation campaign
  `p1-handofffix2-g35flash-p25flash-5g1x2-a2-20260608-102457` reached direct
  Google but stopped on provider quota during baseline eval:
  `agent-turn-summary.json` recorded `TurnFinished.outcome = aborted` with
  `HTTP_429` / `RESOURCE_EXHAUSTED`, while
  `multi-swe-bench-submission.jsonl` contained an empty `fix_patch`.
- Fixed the downstream closure classifier in `crates/ploke-eval/src/closure.rs`:
  `classify_eval_status` and `classify_eval_status_from_registration` now
  classify aborted/timeout terminal records as failed instead of treating
  `record.json.gz` existence as eval completion. Completed records with an empty
  submission but no aborted terminal turn remain complete.
- Verification for the closure classifier guard:
  - `cargo test -p ploke-eval classify_ -- --nocapture` passed and covered the
    aborted-record, completed-registration, and completed-empty-submission
    regressions.
  - `cargo test --workspace 2>&1 | rg -A 8 E0` passed after this classifier
    guard: cargo exited `0`; `rg` exited `1` because no `E0` lines matched.
- Current graph-policy boundary: the new graph-neighborhood policy is
  persisted, prompt-visible, and planner-visible, but final broad harness
  admission still rejects only against the existing
  `WorkspaceExceptPlokeEval` protected-core surface. A follow-up hardening
  step must make admission reject changed paths outside the materialized Cozo
  graph-neighborhood result instead of relying on prompt compliance.
