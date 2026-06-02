# Regression Test Tracker

This tracker records expected-failing regression tests, bug-pinning reproducers, and recently fixed regression tests retained as resolved handoff rows.

Use it to avoid losing known-red tests during broad runs, interrupted work, or test-failure triage, and to keep short-lived fixed-contract regression handoffs discoverable.

## Marker Convention

Every tracked regression test should have a nearby source comment:

```rust
// regr:<name>:DD-MM-YY_HH-MM
```

- `<name>` is a short one-word identifier.
- The timestamp is local time when the marker was added.
- Use `rg -n 'regr:'` from the repo root to find all marked tests.

## Active Tracker

| Marker | Status | File path | Exact test name or command | Expected result | Removal or update condition |
| --- | --- | --- | --- | --- | --- |
| `regr:googlevertex:23-05-26_19-10` | expected-failing | `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs` | `cargo test -p ploke-eval xfail_google_vertex_broad_headless_tui_attempt_applies_edit_from_published_request -- --ignored --nocapture` | Expected to fail before producing an admissible edit. This is a live counterexample for the unsupported Google Vertex/OpenAI-compatible broad headless-TUI path, not proof of the supported direct-Google route. | If the project intentionally supports this endpoint path later, rename/unignore the test, make it pass as a positive canary, update `docs/active/plans/self-improvement-loop/google-api.md`, and move this row to resolved. |

## Resolved Handoff Rows

| Marker | Status | File path | Exact test name or command | Expected result | Removal or update condition |
| --- | --- | --- | --- | --- | --- |
| `regr:samefile:19-05-26_06-42` | resolved | `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs` | `cargo test -p ploke-eval recorded_replay_rejects_stale_same_file_repair_after_first_apply` | Asserts stale same-file `non_semantic_patch` replay is rejected or invalidated before it becomes a second applied proposal. | Remove after the RF-05 handoff no longer needs a dedicated tracker row, or keep as fixed-contract replay coverage for same-file edit composition. |
| `regr:samefiletui:19-05-26_15-43` | resolved | `crates/ploke-tui/src/tools/tool_tests/patches.rs` | `cargo test -p ploke-tui ns_patch_rejects_fuzzy_same_file_repair_after_applied_proposal_before_staging` | Asserts a fuzzy stale same-file `non_semantic_patch` repair fails before staging after the first same-file proposal has applied. | Remove after the RF-05 handoff no longer needs a dedicated tracker row, or keep as lower-level fixed-contract coverage for same-file edit composition. |
| `regr:protectedstaged:19-05-26_15-43` | resolved | `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs` | `cargo test -p ploke-eval recorded_replay_rejects_protected_ns_patch_before_staged_success_reaches_model` | Asserts protected `non_semantic_patch` failures are model-visible as structured rejections, not replayed as staged-success payloads. | Remove after the run-review handoff no longer needs a dedicated tracker row, or keep as fixed-contract regression coverage for the staged-success ambiguity in `2026-05-18-tool-failures.md`. |
| `regr:protectedrepeat:19-05-26_15-43` | resolved | `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui_adapter.rs` | `cargo test -p ploke-eval historical_trace_replay_marks_repeated_protected_ns_patch_before_staging` | Replays the `node-01c9e8fdc70e3ee8` protected `Cargo.toml` retry shape from `/home/brasides/.ploke-eval/campaigns/p1-broad-batch-admission-20260518-2/.../node-01c9e8fdc70e3ee8.headless-tui.json` and asserts both attempts fail before staging, with the repeated denial marked. | Remove after the run-review handoff no longer needs a dedicated tracker row, or keep as fixed-contract regression coverage for repeated protected manifest denials. |
| `regr:protectedpreflight:19-05-26_15-43` | resolved | `crates/ploke-tui/src/tools/tool_tests/patches.rs` | `cargo test -p ploke-tui ns_patch_protected_path_preflight_rejects_repeat_before_staging` | Asserts repeated protected manifest/config edits fail in preflight, emit no `ToolCallCompleted`, stage no proposals, and mark repeat context. | Remove after the run-review handoff no longer needs a dedicated tracker row, or keep as lower-level fixed-contract coverage for protected-path policy preflight. |
| `regr:timeoutapplied:22-05-26_01-27` | resolved | `crates/ploke-eval/src/cli/prototype1_state/tests/cli_tests.rs` | `cargo test -p ploke-eval timed_out_headless_tui_applied_attempt_writes_submitted_result_for_admission` | Asserts a headless TUI run with `terminal=TimedOut` and an applied proposal still writes a request-bound submitted result and leaves a backend-admissible candidate diff. | Keep as fixed-contract coverage for the r3/r4 timeout-after-apply admission handoff regression. |
| `regr:jsonretry:22-05-26_04-26` | resolved | `crates/ploke-eval/src/cli.rs` | `cargo test -p ploke-eval intent_segmentation_truncated_json_parse_is_retryable` | Asserts Direct Google intent-segmentation JSON parse EOFs are classified as retryable instead of terminal no-progress failures. | Keep as fixed-contract coverage for `2026-05-22-prototype1-protocol-segmentation-truncated-json.md`. |
| `regr:cargotail:22-05-26_14-10` | resolved | `crates/ploke-tui/src/tools/cargo.rs` | `cargo test -p ploke-tui format_details_shows_latest_test_output_and_filters_success_progress` | Asserts cargo tool details show the newest retained test stdout, include the final test-result line, and filter successful Cargo progress stderr. | Keep as fixed-contract coverage for `2026-05-22-cargo-tool-tail-rendering-and-timeout.md`. |

## Triage Rules

- If a marked regression test fails, check this document before treating the failure as novel.
- If a test has a `regr:` marker but is missing from this tracker, add it before broad triage continues.
- If this tracker references a removed or renamed test, update the row in the same change that moves the test.
- If the underlying bug is fixed, either remove the row or move it to a resolved section with the commit/test evidence.
