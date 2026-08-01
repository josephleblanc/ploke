# Prototype 1 Google Post-Apply Indexing Timeout

## Summary

Google can now drive the Prototype 1 broad headless TUI far enough to apply
edits, but recent live-loop attempts can stall after apply while dense indexing
keeps running and no submitted harness result is written. This blocks the
current self-improvement loop goal because Prototype 1 needs an applied
candidate Artifact plus Runtime evidence that can enter the append-only
Journal/History tree, not just a dirty checkout.

This report is scoped to the Google/live-loop stabilization effort for
`p1-google-live-run-20260521-3`.

## Current Evidence

Campaign:

```text
/home/brasides/.ploke-eval/campaigns/p1-google-live-run-20260521-3
```

Observed requests:

```text
prototype1/messages/edit-harness-request/node-0b42ecece6f571b0-r7.json
prototype1/messages/edit-harness-request/node-0b42ecece6f571b0-r8.json
```

Observed logs:

```text
/home/brasides/.ploke-eval/logs/ploke_eval_20260522_091854_1580654.log
/home/brasides/.ploke-eval/logs/llm_full_response_20260522_091854_1580654.log
/home/brasides/.ploke-eval/logs/ploke_eval_20260522_095117_1603168.log
/home/brasides/.ploke-eval/logs/llm_full_response_20260522_095117_1603168.log
```

`r7` applied edits under
`proc_macros/syn_parser/syn_parser_macros/src/lib.rs`. The candidate checkout
ended dirty, but no `r7` result file existed under
`prototype1/messages/edit-harness-result/`, and the log had no
`IndexingCompleted` event. The tail was still in dense embedding/indexer work at
about 179 seconds.

`r8` applied an edit under `crates/ploke-tree-egui/src/main.rs`. The candidate
checkout ended dirty, but no `r8` result file existed and the log again had no
`IndexingCompleted` event before the run ended in dense embedding/indexer work.

`r8` also exercised the same-file repair guard. After an earlier settled
proposal touched `crates/ploke-tree-egui/src/main.rs`, a later
`non_semantic_patch` against stale same-file content failed before staging with:

```text
patch matched only fuzzily after an earlier settled proposal touched the same file; refresh the file and submit a diff against the current content
```

That is the desired guard behavior for stale fuzzy same-file repair attempts,
but the attempt still consumed turn/time and did not produce a submitted
candidate result.

## r4 Replay Findings

The live `r4` attempt remains the positive broad-harness proof: Google produced
valid tool calls, reached `apply_code_edit`, and wrote a submitted result for
`crates/ploke-llm/src/types/meta.rs`.

Replay workspace:

```text
/home/brasides/.ploke-eval/replay-workspaces/p1-google-r4-replay-20260522
/home/brasides/.ploke-eval/replay-workspaces/p1-google-r4-replay-fresh-20260522-2
```

Replay logs:

```text
/home/brasides/.ploke-eval/logs/ploke_eval_20260522_100028_2.log
/home/brasides/.ploke-eval/logs/llm_full_response_20260522_100028_2.log
/home/brasides/.ploke-eval/logs/ploke_eval_20260522_102946_2.log
/home/brasides/.ploke-eval/logs/llm_full_response_20260522_102946_2.log
```

The recorded-provider replay drove the captured `r4` model output through the
current session/tool loop without live provider HTTP. It replayed the same
lookup/read/context/apply/stop sequence from a clean clone at the target
baseline and left the replay workspace dirty at
`crates/ploke-llm/src/types/meta.rs`.

The fresh replay trace reported `scan_barrier changed=none` after
`proposal_applied`, then `sparse_refresh bm25_status` and
`sparse_refresh bm25_ready docs=6398`. In this path, `changed=none` means the
semantic apply path had already updated file tracking before the post-apply scan
barrier. It does not mean the workspace already contained the `r4` edit.

The rebuilt local `ploke-eval` binary reproduced that success on
`p1-google-r4-replay-fresh-20260522-3`: the clone was clean before replay,
ended dirty only at `crates/ploke-llm/src/types/meta.rs`, and the trace ended
`terminal applied` with one changed path.

The replay is tool-loop proof, not fresh provider/model-quality proof: it proves
the current headless TUI/session/tool loop can replay and apply the recorded
Google output, but it does not ask Google to produce a new patch.

## r8 Raw-Sidecar Replay Findings

The `r8` attempt did not write `.headless-tui.json`, but its raw provider
sidecar is sufficient to replay the model path through the current headless TUI
tools.

Raw sidecar:

```text
/home/brasides/.ploke-eval/logs/llm_full_response_20260522_095117_1603168.log
```

Full raw replay command:

```text
cargo run -p ploke-eval -- run replay self-edit-live \
  --request /home/brasides/.ploke-eval/campaigns/p1-google-live-run-20260521-3/prototype1/messages/edit-harness-request/node-0b42ecece6f571b0-r8.json \
  --raw-full-response /home/brasides/.ploke-eval/logs/llm_full_response_20260522_095117_1603168.log \
  --workspace /tmp/p1-loop-raw-replay-r8.UPU89e \
  --tail stop --max-attempts 1 --timeout-secs 300 --format table
```

Result: `selected_tool_requests: 16`, `installed_records: 17`, and
`terminal: applied changed_paths=1`. The replay reproduced the stale same-file
`non_semantic_patch` rejections, then applied the final patch to
`crates/ploke-tree-egui/src/main.rs`.

The same raw-sidecar path now supports a provider-response breakpoint:

```text
cargo run -p ploke-eval -- run replay self-edit-live \
  --request /home/brasides/.ploke-eval/campaigns/p1-google-live-run-20260521-3/prototype1/messages/edit-harness-request/node-0b42ecece6f571b0-r8.json \
  --raw-full-response /home/brasides/.ploke-eval/logs/llm_full_response_20260522_095117_1603168.log \
  --workspace /tmp/p1-loop-raw-replay-r8-prefix.3Y8waY \
  --through-response-index 6 \
  --tail stop --max-attempts 1 --timeout-secs 300 --format table
```

Result: `selected_response_records: 7`, `selected_tool_requests: 7`,
`installed_records: 8`, and `terminal: applied changed_paths=1`. This provides
a bounded step-through probe for the first applied edit without requiring a
fresh live provider call.

Raw-sidecar replay uses provider response indexes as its breakpoint coordinate.
Headless event slicing flags (`--event-index` and `--through-event`) are now
rejected with `--raw-full-response` so operators do not accidentally replay a
full raw sidecar when they intended to stop at a historical event prefix.

## Fix And Regression Evidence

The stale same-file repair part has existing regression anchors:

```text
cargo test -p ploke-eval recorded_replay_rejects_stale_same_file_repair_after_first_apply -- --nocapture
cargo test -p ploke-tui ns_patch_rejects_fuzzy_same_file_repair_after_applied_proposal_before_staging -- --nocapture
```

Those tests cover the lower-level contract that a stale fuzzy same-file repair
after a settled first edit is rejected before a second proposal is staged or
materialized. This docs worker did not rerun them.

The remaining blocker is separate: after a valid edit is applied, the harness
needs a bounded way to settle the search index expected by the active runtime
mode enough to emit the attempt result and preserve the candidate
Artifact/Runtime evidence. The current patch makes sparse-strict broad-harness
attempts settle on BM25 readiness instead of dense `IndexingCompleted`.

The same runtime patch also preserves compact diagnostics for observed failures:
if the headless adapter has already seen prompt/tool/apply activity, an internal
adapter error now becomes a terminal failure in the `HeadlessRun` evidence
instead of escaping before `.headless-tui.json` can be written. Startup failures
with no observations still return as hard errors.

The starting DB cache now includes `PreparedSingleRun.repo_root` in its
metadata and cache key. This prevents a cached starting database created for one
checkout root from being reused for a different child-owned checkout root with
the same task, base SHA, and embedding model.

Focused regression checks:

```text
cargo test -p ploke-eval sparse_post_apply_refresh -- --nocapture
cargo test -p ploke-eval evidence_can_preserve_observed_headless_runtime_error -- --nocapture
cargo test -p ploke-eval replay::self_edit -- --nocapture
cargo test -p ploke-eval run_replay_self_edit_live_command_parses -- --nocapture
```

2026-05-22 rerun evidence:

```text
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval sparse_post_apply_refresh -- --nocapture
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval evidence_can_preserve_observed_headless_runtime_error -- --nocapture
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval replay::self_edit -- --nocapture
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval run_replay_self_edit_live_command_parses -- --nocapture
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval starting_db_cache_miss_when_repo_root_changes -- --nocapture
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval run_replay_self_edit_live_raw -- --nocapture
```

Results: sparse refresh passed 2 tests; observed-error preservation passed 1
test; self-edit replay passed 4 tests; replay CLI parsing passed 2 tests;
starting DB root-key coverage passed 1 test; raw replay slicing contract passed
2 tests.

The `r8` prefix raw replay was also rerun from a fresh local clone:

```text
RUSTFLAGS=-Awarnings cargo run -q -p ploke-eval -- run replay self-edit-live \
  --request /home/brasides/.ploke-eval/campaigns/p1-google-live-run-20260521-3/prototype1/messages/edit-harness-request/node-0b42ecece6f571b0-r8.json \
  --raw-full-response /home/brasides/.ploke-eval/logs/llm_full_response_20260522_095117_1603168.log \
  --workspace /tmp/p1-loop-raw-replay-r8-check.rlSPp1 \
  --through-response-index 6 \
  --tail stop --max-attempts 1 --timeout-secs 300 --format table
```

Result: `selected_response_records: 7`, `selected_tool_requests: 7`,
`installed_records: 8`, and `terminal: applied changed_paths=1`.

## Follow-Up: Campaign `p1-google-live-run-20260521-4`

Campaign `p1-google-live-run-20260521-4` reached a later blocker: parent patch
production succeeded for multiple children, the selected child was evaluated,
and successor startup then failed during `prototype1_history_store` with:

```text
sealed block failed verification before storage
```

That follow-up does not close this post-apply report until a fresh live run on
the current binary confirms applied slots reliably produce submitted result
evidence after sparse post-apply refresh. It does narrow the next known full-loop
blocker to successor History block sealing/verification:

```text
docs/active/bugs/2026-05-22-prototype1-successor-history-sealed-block-verification.md
```

After the raw replay CLI slicing contract was tightened, the same prefix replay
was rerun through the actual CLI from a fresh clone at:

```text
/tmp/p1-loop-raw-replay-r8-after-cli.EBV9vd/repo
```

Result: `selected_response_records: 7`, `selected_tool_requests: 7`,
`installed_records: 8`, and `terminal: applied changed_paths=1`.

Broad non-network validation also exposed a test-harness issue: `cargo test -q
-p ploke-eval --lib` reached
`live_tui_router_staged_proposal_lowers_to_checked_artifact_delta`, which was
compiled by the default `live_api_tests` feature and was not ignored. On the
Codex no-elevation path it attempted a provider request and failed with
`HTTP_SEND_FAILED`. The test is now explicitly ignored, matching the other live
provider checks, so local broad validation does not depend on network while the
live check remains runnable with `--ignored`.

Rerun result:

```text
RUSTFLAGS=-Awarnings cargo test -q -p ploke-eval --lib
```

Result on 2026-05-22: passed with 687 tests, 19 ignored, 0 failed. This confirms
the broad non-network `ploke-eval` library validation path no longer attempts a
provider request by default.

## Expected Next Check

Use `r7` or `r8` as the focused repro for the post-apply completion blocker. A
fix is not demonstrated until an applied edit produces a submitted harness
result, not only a dirty candidate checkout, and the result records the
candidate evidence needed by Prototype 1 History.
