# Prototype 1 Run Review: Isolated Gemini 3.5 Flash Direct, ripgrep-2209

Date: 2026-05-25

Campaign: `p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746`

Run: `run-1779709154252-structured-current-policy-72157381`

Task: `BurntSushi__ripgrep-2209`

Review status: complete. The run-review Quality Gate passes because the
provider/tool/edit/validation/export trace is reconstructable from durable
artifacts, and the suspicious degraded-context and terminal-warning behavior was
checked against the trace, patch artifact, validation audit, submission, protocol
directory, registry, and final checkout. One evidence gap remains: the exact
live stdout `ContentMismatch` lines are not persisted verbatim in the run
artifacts, so this review treats only the persisted model-visible stale-content
errors as confirmed.

## Short Verdict

The eval mechanically wrote the expected artifacts and exported a non-empty
Multi-SWE patch, but it should not be counted as benchmark-useful. The agent
turn aborted after 100 attempts with no final assistant message, and the last
cargo validation failed on the model's own added regression test.

The `TOOL_EXECUTION_FAILED` warning seen in live stdout was not found verbatim in
persisted artifacts. Persisted evidence shows the warning was terminal to the
agent turn, not terminal to artifact accounting: `agent-turn-summary.json`
records `outcome: aborted`, error id
`de135ac7-9a87-46bd-9273-a0dc85006d1c`, and `attempts: 100`, while
validation audit, full responses, record, submission, and benchmark patch
projection were still written.

The degraded-context failures mattered. They did not prevent the production edit
or patch export, and direct `read_file` calls remained available, but after the
final failed test the model repeatedly received stale semantic context for
`standard.rs` and did not recover enough to apply the intended test correction.

## Evidence Roots

- Run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746/BurntSushi__ripgrep-2209/runs/run-1779709154252-structured-current-policy-72157381`
- Run manifest:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746/BurntSushi__ripgrep-2209/run.json`
- Closure state:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746/closure-state.json`
- Run registration:
  `/home/brasides/.ploke-eval/registries/runs/run-1779709154252-structured-current-policy-72157381.json`
- Dataset slice:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746/slice.jsonl`
- Protocol dir:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-direct-15g2x3-isolated-20260525-113746/BurntSushi__ripgrep-2209/runs/run-1779709154252-structured-current-policy-72157381`
- Final checkout:
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`

The bundled audit was run read-only:

```text
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py <run-root> --markdown
```

Audit summary: 100 provider responses, 100 provider-emitted tool calls, 100
recorded tool calls, 0 missing provider call ids, 0 extra recorded call ids, and
100 `tool_calls` finishes. There was no final `stop` response. Recorded
classifications were 41 `read_with_content`, 25 `context_results`, 14
`empty_completed_read`, 11 `completed`, 5 `duplicate_request`, and 4
`transport_failure`.

## Execution Path

Artifact-proved path:

```text
prepared single run
-> structured-current-policy treatment
-> run single agent / agent-single-turn
-> crates/ploke-eval/src/runner.rs::run_benchmark_turn
-> agent-turn trace and summary
-> validation audit
-> full response trace
-> snapshot
-> Multi-SWE submission
-> benchmark patch projection
-> partial protocol projection
```

Evidence:

- `run.json` names task `BurntSushi__ripgrep-2209`, base
  `4dc6c73c5a9203c5a8a89ce2161feca542329812`, model
  `google/gemini-3.5-flash`, and expected changed file
  `crates/printer/src/util.rs`.
- `execution-log.json` lists `bootstrap_headless_runtime`,
  `benchmark_turn_completed`, `write_validation_audit`,
  `persist_full_response_trace`, `snapshot_completed`,
  `write_msb_submission`, and `write_benchmark_patch_projection`.
- The run registration freezes `run_arm_id: structured-current-policy`,
  `command: run single agent`, `execution: agent-single-turn`, and marks
  execution, patching, packaging, and submission complete.
- `crates/ploke-eval/src/runner.rs` contains the path that calls
  `run_benchmark_turn`, writes the validation audit, persists the full response
  trace, and writes the benchmark submission/projection.

## What The Model Saw

The initial prompt gave the model relevant but partial context:

- The issue text said the bug was duplicative replacement in multiline mode,
  caused by extending the search bytes for PCRE2 look-around and failing to
  reject replacement matches past the real endpoint.
- RAG snippets included `crates/printer/src/standard.rs` multiline replacement
  tests such as `replacement_multi_line`, `replacement_multi_line_combine_lines`,
  and `replacement_multi_line_diff_line_term`.
- RAG also included `crates/printer/src/util.rs::find_iter_at_in_context`,
  including the comment that explains the look-ahead kludge and the existing
  simple-search rule that stops when `m.start() >= range.end`.

The model then gathered enough production context to attempt the right class of
fix. It read `util.rs`, searched replacement APIs, read matcher replacement
helpers, localized `Replacer::replace_all`, and saw the existing
`find_iter_at_in_context` pattern. The last point where the model clearly had
enough information to make a useful production edit was before call
`function-call-dd98848e-6b0e-4cfd-b6a7-dc17538b80d6`, which replaced
`replace_with_captures_at` with inline bounded `try_captures_iter_at` logic in
`crates/printer/src/util.rs`.

## Trace Reconstruction

The main trace chain:

```text
initial RAG and util.rs reads
-> model localizes replacement bug to Replacer::replace_all
-> production edit to util.rs is applied
-> cargo check and cargo test pass before the new test exists
-> model adds a standard.rs regression test
-> final package-intended cargo test fails that new test
-> model tries to correct the test
-> stale semantic context and same-file edit verification block correction
-> the agent turn aborts at 100 attempts
-> submission/projection are still exported from the dirty checkout
```

Important call ids and events:

- Event 364, call `function-call-dd98848e-6b0e-4cfd-b6a7-dc17538b80d6`:
  `apply_code_edit` for `crates/printer/src/util.rs`. Later tool results show
  it staged and applied one edit, and the patch artifact records it as
  `Applied`.
- Event 381, call `function-call-97e9da16-a7e5-4347-9712-70c96dc56f53`:
  `cargo check` passed but resolved to focused `crates/globset/Cargo.toml`, so
  it did not cover the changed file.
- Event 389, call `function-call-d2f19c50-da6d-4947-8b07-0a7c17b57e4d`:
  `cargo test --package grep-printer` passed via the root manifest and covered
  changed files, but this was before the new regression test was added.
- Event 549, call `function-call-f5059462-03c8-49cb-83c2-771cd08eb085`:
  `apply_code_edit` for `crates/printer/src/standard.rs`. Tool results show it
  staged and applied one edit, and the patch artifact records it as `Applied`.
- Event 563, call `function-call-d8b30e9c-7fd8-4848-8791-1bc6aad7baa0`:
  `cargo test --package grep-printer`. The tool result records a failed root
  manifest test run: `standard::tests::replacement_multi_line_duplication`
  expected `1:z` then `3:z`, but got `1:z` then `2:z`. The result was
  `FAILED. 94 passed; 1 failed`.
- Event 653, call `function-call-7f3b6dc4-d80e-4cfd-8446-dfc8dec49b44`:
  model attempted to change the test haystack to `a\nb\nc\na\nb\n` and expected
  output to `1:z\n4:z\n`. Event 656 rejected the semantic edit because
  `standard.rs` content could not be verified: `Content changed ... Refresh or
  re-resolve the target before submitting another semantic edit`.
- Event 813, call `function-call-d94ee4b5-51bc-4f64-b78a-1977a77c1766`:
  `code_item_lookup` for `replacement` in `standard.rs` failed with
  `Internal compiler error: failed to read snippet: Content changed for
  ".../crates/printer/src/standard.rs"`.

## ContentMismatch And Context Degradation

The exact live stdout `ContentMismatch` strings for `standard.rs` and `util.rs`
were not present in `agent-turn-trace.json`, `agent-turn-summary.json`,
`execution-log.json`, `validation-audit.json`, `llm-full-responses.jsonl`, the
campaign files, or the run registry. Persisted evidence still shows the same
class of problem in the model-visible channel:

- `function-call-7d2615d6-62e5-433d-abe4-4f63d41d4989` returned degraded
  `request_code_context`: 7 snippets returned, 3 stale snippets skipped.
- `function-call-0ee5e527-10f5-4980-9917-b1fa76e5f2aa` returned degraded
  `request_code_context`: 2 snippets returned, 8 stale snippets skipped.
- `function-call-cdb83b9d-8e60-4181-9be1-659582e3651a` returned degraded
  `request_code_context`: 1 snippet returned, 9 stale snippets skipped.
- `function-call-7f3b6dc4-d80e-4cfd-8446-dfc8dec49b44` and
  `function-call-d94ee4b5-51bc-4f64-b78a-1977a77c1766` failed directly on
  stale `standard.rs` content verification.

For `util.rs`, the durable trace does not show a persisted `Content changed`
error after the production edit. The `util.rs` edit applied, and later cargo
calls ran against the changed checkout. Any live stdout `util.rs`
`ContentMismatch` was therefore either not persisted or not surfaced as a
model-visible hard failure in the durable artifacts reviewed here.

For `standard.rs`, the degraded context was recoverable in principle: the model
continued issuing direct `read_file` calls after the failed semantic edit.
Practically, it did not recover. The failing test result gave a concrete local
signal, and the next intended correction was blocked by stale same-file semantic
state. The run then spent the remaining attempts mostly reading broad
`standard.rs` ranges and ended without a final response or corrected patch.

## Validation And Export

`validation-audit.json` records five cargo calls:

| Event | Call id | Command | Resolved scope | Ok | Covers changed files |
| --- | --- | --- | --- | --- | --- |
| 32 | `function-call-695406fc-94ac-43a0-8219-23a494e994e3` | `check` | focused `crates/globset/Cargo.toml` | true | false |
| 48 | `function-call-34c0d61d-5b45-436e-b470-750fee3da3a6` | `test` | root workspace manifest | true | true |
| 384 | `function-call-97e9da16-a7e5-4347-9712-70c96dc56f53` | `check` | focused `crates/globset/Cargo.toml` | true | false |
| 392 | `function-call-d2f19c50-da6d-4947-8b07-0a7c17b57e4d` | `test` | root workspace manifest | true | true |
| 568 | `function-call-d8b30e9c-7fd8-4848-8791-1bc6aad7baa0` | `test` | root workspace manifest | false | true |

The audit therefore has both facts at once: there was earlier successful cargo
evidence covering changed files, and the final cargo evidence was a covering
failure. The final validation state is red.

The exported patch is non-empty:

- `benchmark-patch-projection.json` has `check.status: passed`, meaning the
  `fix_patch` was exported from the recorded checkout cwd.
- `multi-swe-bench-submission.jsonl` SHA-256:
  `db573acfc8ef67eb22191d1ea5f57b8824a6758f8387b98163957d40eaec79f3`.
- Export length: 3,861 bytes, 106 lines.
- Final checkout diff: `crates/printer/src/standard.rs` and
  `crates/printer/src/util.rs`, 51 insertions and 25 deletions.

The submitted patch changed `util.rs` production logic and added a
`replacement_multi_line_duplication` unit test in `standard.rs`. It is not the
gold patch: the slice's gold fix changes only `crates/printer/src/util.rs` with
a helper equivalent to bounded replacement in context, and the gold tests are
CLI regressions `r2095` and `r2208` under `tests/regression.rs`.

## Benchmark Usefulness

Benchmark usefulness is not supported.

The exported patch fails its own final visible validation. The agent did find
the right neighborhood and made a production edit aligned with the issue's
replacement-boundary problem, so the run is diagnostically useful for observing
where the tool loop breaks down. It is not useful as a benchmark submission:
there is no final green validation after the new test, no final assistant
message, no proof against the gold regression shape, and the exported test file
diff includes the failing `standard.rs` unit test.

This run should be classified as non-empty patch, final-validation-red,
agent-turn-aborted, benchmark-not-useful.

## Protocol And Accounting

This review originally ran before the baseline protocol step completed. At that
time, closure and registry disagreed:

- `closure-state.json` says eval complete, protocol missing, doctor phase
  `baseline_protocol`, and blockers empty.
- The run registry marks protocol `completed` with detail
  `protocol aggregate available`.
- The protocol directory contains only
  `tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json`.
- There is no durable `tool-call-review` or `tool-call-segment-review` artifact
  for this run.

The sole segmentation artifact covers 100 calls and reports 4 failed tool calls,
but it is not enough to adjudicate benchmark usefulness. It also reports
`patch_proposed: false` and `patch_applied: false`, which conflicts with the
patch artifact and checkout diff showing two applied edits. Treat the protocol
record as present manual-join evidence for tool-intent segmentation only, not as
a completed protocol suite.

Post-review note: the orchestrator later started an overlapping protocol step
against this same isolated campaign. Current closure now reports protocol
complete, but the protocol artifact directory is contaminated: it has one
intent-segmentation artifact, 200 `tool_call_review` artifacts, and 8
`tool_call_segment_review` artifacts for a 100-call run. That later state should
not be used as clean evidence for a single successful protocol pass.

## Working And Broken

Working:

- The runner preserved full provider/tool parity: no missing or extra recorded
  provider call ids.
- The model saw relevant initial RAG and found the right production area.
- `apply_code_edit` could apply one production edit and one test edit.
- Validation audit captured the important distinction between focused non-covering
  checks, earlier covering successes, and the final covering failure.
- Patch export faithfully reflected the final dirty checkout.

Broken or misleading:

- Eval completion artifacts can be written after an aborted agent turn and final
  failed validation without making that failure prominent in the projection.
- Live stdout `TOOL_EXECUTION_FAILED` was not persisted verbatim, leaving the
  terminal warning only inferable from `terminal_record` and trace shape.
- Stale semantic context after same-file edits degraded the model's recovery
  path at the exact point it needed to repair a failed test.
- The run registry said protocol completed even though closure said protocol
  missing and only one of three required protocol procedures existed at review
  time; later overlapping protocol execution made the current protocol artifact
  surface non-clean in the opposite direction.
- Segmentation protocol fields claim no patch was proposed or applied despite
  applied edit proposals and an exported patch.

## Action Items

1. Gate benchmark usefulness on the final covering validation result and
   terminal turn status. A non-empty exported patch from an aborted turn with
   final failed cargo should be labelled accordingly.
2. Persist live `ContentMismatch` and `TOOL_EXECUTION_FAILED` lines into the run
   artifact surface with call ids or event ids. The current artifacts preserve
   enough to diagnose `standard.rs`, but not enough to confirm the live stdout
   `util.rs` messages.
3. Fix protocol accounting and execution guards so registration cannot report
   protocol completed when required artifacts are absent, and so overlapping
   protocol steps cannot duplicate per-call review artifacts while closure still
   collapses the run to `complete`.
4. After an applied same-file edit, make semantic edit and code lookup failures
   point the model to the exact live diff or refreshed file span. In this run,
   direct reads remained possible but the recovery path was too diffuse to get
   back to a correct patch before the attempt cap.
5. Keep package intent and resolved cargo target visible together. The model
   requested `--package grep-printer`, while the persisted result records the
   root manifest and `scope: workspace`; both are needed to interpret validation
   correctly.
