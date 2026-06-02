# Prototype 1 Eval And Protocol Review: `p1-gemini35-flash-direct-fresh-20260524-193515`

## Verdict

This baseline run is now mechanically complete through protocol, but it is not
benchmark-clean. The Prototype 1 state-machine eval produced a non-empty
Multi-SWE-bench submission, all provider-emitted tool calls were recorded, and
packaging exported a patch. After the direct-Google protocol reasoning-default
fix, the resumed campaign completed baseline protocol: closure now reports
`eval = complete`, `protocol = complete`, 80 reviewed calls, and 11 usable
segments.

The protocol completion arc is operationally useful: it proves the current
direct-Google JSON protocol path can finish this baseline without the previous
trailing-text parse blocker. It does not prove the patch is a successful fix.
The patch still changes only `crates/printer/src/util.rs`, adds the replacement
helper and a regression test, then changes that test's expected output from
`1:foo\n3:foo\n` to the observed `1:foo\n2:foo\n` after the test failed. The
final focused `grep-printer` test run passes, but the model's final
"workspace-wide tests" command actually resolved to the unrelated `globset`
manifest. No formatting command ran.

The protocol artifacts catch several eval concerns, including search thrash,
semantic-tool failures, and test-failure recovery. They also over-credit several
important points because the protocol packets carry previews and local
neighborhoods, not full patch semantics or cargo manifest scope. The most
important blind spots are the expected-output flip in the final test patch and
the final cargo scope mismatch.

## Evidence Roots

- Campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260524-193515`
- Worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-fresh-20260524-193515`
- Run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260524-193515/BurntSushi__ripgrep-2209/runs/run-1779676580929-structured-current-policy-7d7706cf`
- Record:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260524-193515/BurntSushi__ripgrep-2209/runs/run-1779676580929-structured-current-policy-7d7706cf/record.json.gz`
- Protocol artifacts:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-direct-fresh-20260524-193515/BurntSushi__ripgrep-2209/runs/run-1779676580929-structured-current-policy-7d7706cf`
- Target checkout recorded in artifacts:
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`

## Execution Path

This is the Prototype 1 eval state-machine path, not the broad headless TUI
path:

```text
prototype1-step -> run_planned_child -> runner.rs::run_benchmark_turn -> record.json.gz -> packaging
```

Evidence:

- `execution-log.json` records `run_arm.execution = agent-single-turn`.
- `execution-log.json` steps include `benchmark_turn_completed`,
  `persist_full_response_trace`, `write_msb_submission`, and
  `write_benchmark_patch_projection`.
- Repo code has the packaging path in `crates/ploke-eval/src/runner.rs` around
  `run_benchmark_turn`, `write_msb_submission_artifact`, and
  `write_benchmark_patch_projection`.

## Closure State

`closure-state.json` after the resumed protocol pass:

- `registry.status = complete`
- `eval.status = complete`
- `eval.complete_total = 1`
- `eval.failed_total = 0`
- `protocol.status = complete`
- `protocol.full_total = 1`
- `protocol.failed_total = 0`
- `protocol.missing_total = 0`
- required procedure statuses are all complete:
  `tool-call-intent-segments`, `tool-call-review`,
  `tool-call-segment-review`
- `protocol_counts.total_calls = 80`
- `protocol_counts.reviewed_calls = 80`
- `protocol_counts.total_segments = 11`
- `protocol_counts.usable_segments = 11`
- `protocol_counts.missing_segments = 0`

The protocol artifact directory contains exactly the expected families:

- 1 `tool_call_intent_segmentation` artifact
- 80 `tool_call_review` artifacts
- 11 `tool_call_segment_review` artifacts

The campaign scheduler now has one planned generation-0 child node
(`node-b19077fc35c373b5`), matching the post-baseline-protocol `child_plan`
state described by doctor.

The campaign route is the intended direct Google route:

- `model_id = google/gemini-3.5-flash`
- `route_source = direct_google`

The admitted run profile also records:

- `[model].provider = google`
- `[model].route_source = direct-google`
- `[execution].stop_after = complete`
- `[execution.mbe].enabled = false`
- `[selection.oracle].mode = record-only`
- `[selection.oracle].require_evidence = true`

## Protocol Completion Follow-Up

The direct-Google protocol pass completed after the reasoning-default fix. The
protocol artifacts show 274 `google_json_chat` adjudicator responses:

- all 274 have `finish_reason = stop`
- all 274 have non-missing usage records
- all 274 persisted `raw_content` values begin and end as JSON objects or
  arrays
- no persisted `raw_content` contains code fences or obvious trailing text
- no protocol artifact or campaign/run sidecar I checked records a parse
  failure, malformed JSON error, or retry ledger

That is enough to say the current protocol path no longer hits the earlier
direct-Google JSON/trailing-text failure mode for this run. It is not enough to
measure retry behavior. If malformed JSON retries happened before the final
accepted responses, the durable artifacts do not expose attempt counts, rejected
raw content, parse errors, or retry reasons.

The protocol aggregate is mechanically complete, not a semantic success auditor
by itself. The 11 segments cover every call, and all 80 focal call reviews are
present, but the reviews are based on bounded packets and result previews. They
do not consistently see the full cargo manifest path, complete test output, or
the semantic difference between a production fix and a test-expected-output
change.

## Eval And Patch Output

`benchmark-patch-projection.json`:

- `check.status = passed`
- `submission.byte_len = 3395`
- `submission.line_count = 109`
- `submission.sha256 =
  e3617ef5d07ffc390888b7bd7b4f225cf61e4daa74fde97d5dd633711f4e41bc`
- checkout head remained the benchmark base SHA:
  `4dc6c73c5a9203c5a8a89ce2161feca542329812`

`multi-swe-bench-submission.jsonl` contains one non-empty `fix_patch`.
Changed file:

- `crates/printer/src/util.rs`

No exported patch changes `crates/printer/src/standard.rs`. The run manifest's
expected file list also names only `crates/printer/src/util.rs`, so packaging
accepted this as an expected-file patch.

The exported patch:

- changes `Replacer::replace_all` to call
  `replace_with_captures_at_in_context`
- adds `replace_with_captures_at_in_context` in `util.rs`
- adds `util::regression_duplicative_replacement_multiline`
- later changes that regression's expected output from:

```text
1:foo
3:foo
```

to:

```text
1:foo
2:foo
```

That last adjustment is the main patch-quality concern. The model used the
failing test output to make the new test pass, but the resulting expected value
is not strong evidence that the duplicated-replacement bug is fixed correctly.

## Tool And LLM Trace

Trace audit from
`docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py`:

- provider responses: `81`
- provider-emitted tool calls: `80`
- recorded tool calls: `80`
- missing recorded provider call ids: `0`
- extra recorded call ids: `0`
- finish reasons: `80` `tool_calls`, `1` `stop`
- recorded classifications:
  - `completed`: `15`
  - `read_with_content`: `22`
  - `context_results`: `19`
  - `empty_completed_read`: `13`
  - `transport_failure`: `6`
  - `duplicate_request`: `5`

The initial RAG context was useful. It included the existing
`replacement_multi_line` tests in `standard.rs` and
`find_iter_at_in_context` in `util.rs`. The model then read the relevant
`util.rs`, searched matcher APIs, found `captures_iter_at`, searched
replacement callers, and eventually patched `Replacer::replace_all`.

The trace also has a lot of low-value churn:

- multiple empty successful reads past the end of `matcher/src/lib.rs` and
  `printer/src/standard.rs`
- broad or repeated searches such as `replace_all standard.rs`,
  `.replace_all(`, and `replacer standard.rs`
- duplicated cargo requests
- several semantic lookup/edit failures before falling back to
  `non_semantic_patch`

## Protocol Review Against Eval Concerns

The protocol catches several concerns from the eval review:

- search thrash around calls 27-29 and 33 is marked as `search_thrash` or
  `mixed`, with concrete rationale about repeated `Replacer`/`replace_all`
  search variants
- semantic-tool failures around calls 51, 52, and 54 are not treated as
  successful progress; call 52 is specifically `no_clear_recovery`
- the failed regression-test run at call 74 is recognized as `key_progress`
  with `clear_next_step`, which correctly treats the failing test as useful
  validation feedback
- the call 48 to 49 and call 61 to 62 retries are positive examples of
  following a model-visible tool hint from `node_kind=method` to
  `node_kind=function`

The protocol also misses or over-credits several issues that matter for
benchmark usefulness:

- call 77 is credited as a patch that addresses multiline counting behavior,
  but the proposal diff only changes the new test's expected output from
  `1:foo\n3:foo\n` to `1:foo\n2:foo\n`
- segment 9 says the recovery patch directly addressed the issue, but the
  segment packet does not distinguish production code edits from test
  expectation edits
- call 79 is credited as a full workspace test, but the durable cargo payload
  resolved to `crates/globset/Cargo.toml`, not the changed `grep-printer`
  package or the whole ripgrep workspace
- the initial no-package `cargo check` at call 0 is similarly over-described as
  a workspace compile signal, even though the eval review found it resolved to
  the unrelated `globset` manifest
- the `standard.rs` content/hash mismatch is seen as an internal compiler error
  and fallback opportunity, but protocol does not name the stronger invariant:
  `standard.rs` reported a content-change failure even though no exported patch
  touched `standard.rs`

Positive adjudication candidates from this run:

- `tool error -> exact retry -> success`: calls 48/49 and 61/62 should become a
  field for "tool hint followed successfully."
- `semantic edit failure -> direct file read -> non-semantic patch -> focused
  tests pass`: calls 51-59 are a real recovery chain, even though the semantic
  edit tool failed.
- `new regression -> failed focused test -> follow-up edit -> focused tests
  pass`: calls 72-78 are a useful validation-repair chain, but should be paired
  with a separate field that asks whether the follow-up edit changed production
  behavior or merely changed expected output.

## Edit Lifecycle And Mismatch Signals

The durable record has the two content-change signals named in the prompt, but
they appear as recoverable tool failures rather than fatal eval failures.

For `crates/printer/src/util.rs` / `replace_all`:

- `insert_rust_item` first applied the helper to `util.rs`.
- `apply_code_edit` then failed once with an invalid canonical method target.
- the retry against `crate::util::Replacer::replace_all` failed with:

```text
Cannot stage apply_code_edit ... Content changed for
"/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/util.rs".
Refresh or re-resolve the target before submitting another semantic edit.
```

- a later `code_item_lookup` against `replace_all` also failed with
  `Content changed` for `util.rs`.
- the model recovered by reading the file and using `non_semantic_patch`.

For `crates/printer/src/standard.rs` / `replacement_multi_line`:

- lookup as a method failed correctly with an invalid-format hint.
- lookup as a free function succeeded for `crate::standard::replacement_multi_line`.
- a later lookup under `crate::standard::tests` failed with:

```text
Internal compiler error: failed to read snippet: Content changed for
"/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/standard.rs"
```

That `standard.rs` mismatch is suspicious because the exported patch never
changed `standard.rs`. It did not block packaging, but it pushed the model into
empty reads and contributed to putting the regression test into `util.rs`
instead of near the related `standard.rs` tests.

The `patch_artifact` summary reports four applied edit proposals, all against
`util.rs`, and `all_proposals_applied = true`.

## Cargo, Check, Fmt

Cargo/check commands recorded as model-visible tool outputs:

- initial `cargo check` with no package:
  - `ok = true`
  - `scope = focused`
  - manifest:
    `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/Cargo.toml`
  - not useful for `grep-printer`
- `cargo check -p grep-printer`:
  - recorded as `command = check`, `scope = workspace`
  - manifest:
    `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/Cargo.toml`
  - stderr showed `Checking grep-printer`
  - useful
- early `cargo test -p grep-printer` before edits:
  - `94` unit tests passed
  - `2` doc tests passed
  - useful baseline signal
- after adding the regression:
  - `cargo test -p grep-printer` failed
  - failure was visible to the model:

```text
test util::regression_duplicative_replacement_multiline ... FAILED
left: "1:foo\n3:foo\n"
right: "1:foo\n2:foo\n"
```

- after changing the expected output:
  - `cargo test -p grep-printer` passed
  - `95` unit tests passed
  - `2` doc tests passed
- final `cargo test` with no package:
  - `ok = true`
  - `scope = focused`
  - manifest:
    `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/Cargo.toml`
  - not useful for the changed files despite the model calling it
    "workspace-wide tests"

No `cargo fmt`, `cargo fmt -- --check`, or rustfmt command appears in the
recorded tool calls.

## Final Assistant Content

Final assistant content exists in `llm-full-responses.jsonl` at response index
`80` with finish reason `stop`. The same content is visible in
`agent-turn-trace.json` as a final assistant `MessageUpdated`.

`agent-turn-summary.json` still reports:

```json
"final_assistant_message": null
```

So this run has the same summary-surface parity gap seen in earlier reviews:
the final provider content exists, but the compact summary does not expose it.

The final content overclaims the patch. It says the model implemented a
"robust and minimal fix" and lists the helper and regression test, but it does
not mention the earlier failing test, the expected-output change, the absent
format check, or the final `globset` scope mismatch.

## TOOL_EXECUTION_FAILED Signal

I searched the run root and campaign artifacts for `TOOL_EXECUTION_FAILED`.
That string was not present in:

- `agent-turn-summary.json`
- `agent-turn-trace.json`
- `llm-full-responses.jsonl`
- campaign `closure-state.json`
- other run-root JSON sidecars

The artifact-backed state instead says:

- `terminal_record.outcome = completed`
- `terminal_record.error_id = null`
- packaging wrote the submission and patch projection
- the last recorded tool call completed successfully

Current classification: this is not an artifact-backed benchmark blocker for
this completed eval. It is still worth tracking as console/runtime noise if it
recurs, because the durable record does contain recoverable `ToolFailed`
events and the console line may be coming from a generic chat-loop failure
path rather than a final failed tool.

## What Is Working

- The direct Google route produced a full 81-response trace and a final stop
  response.
- Provider and recorded tool-call ledgers match exactly for this eval.
- The resumed direct-Google protocol pass completed all required procedure
  families: segmentation, 80 focal call reviews, and 11 segment reviews.
- Persisted protocol adjudicator outputs are clean JSON in this run; I found no
  protocol-side malformed JSON or trailing-text artifact after the
  reasoning-default fix.
- Patch packaging produced a non-empty submission and passed the expected-file
  export check.
- The model saw useful initial context and localized the relevant replacement
  path.
- Failed validation output was model-visible, and the model did perform a
  follow-up edit plus a passing focused `grep-printer` test run.
- Protocol now preserves useful positive adjudication signals, including
  tool-hint-followed retries and fallback from semantic edit failure to direct
  file reads plus `non_semantic_patch`.

## What Is Not Working Yet

- The patch is behaviorally suspect: the regression was made green by changing
  the expected output to the observed output.
- Protocol over-credits that same recovery because it does not distinguish a
  production-code repair from a test expected-output change.
- Protocol over-credits final validation because call packets do not carry
  enough manifest/scope detail to see that final no-package `cargo test`
  resolved to `globset`.
- Malformed JSON retry behavior is not auditable from the persisted protocol
  artifacts; only the accepted raw JSON content is stored.
- Semantic edit and lookup still have stale content/hash failure modes after
  file edits, including a suspicious `standard.rs` mismatch even though
  `standard.rs` did not end up in the patch.
- Empty completed reads can waste many turns and look successful in aggregate
  counters.
- Final validation can silently resolve to an unrelated focused crate.
- The summary sidecar still loses the final assistant message even though the
  provider trace contains it.
- No formatting check ran.

## Blockers And Non-Blockers

Blockers for treating this eval as a successful benchmark patch:

- no MBE/oracle evidence exists for this run
- final patch quality is suspect because the new test was adjusted to observed
  output after failure
- final "workspace-wide" validation resolved to `globset`, not the changed
  `grep-printer` code
- protocol completed, but the current protocol fields do not catch those two
  patch-quality blockers strongly enough to be used as the only semantic gate

Non-blockers for continuing the loop:

- eval packaging completed and produced a non-empty submission
- provider and recorded tool-call ledgers have no missing ids
- baseline protocol is complete with full call and segment coverage
- the `TOOL_EXECUTION_FAILED` console line is not persisted as a final failed
  tool in the run artifacts I checked
- the `replace_all` and `replacement_multi_line` content-change errors were
  recoverable within this eval, though they should stay on the tooling bug
  list

## Action Items

- Do not advance this patch as benchmark-clean without oracle/MBE review or an
  independent patch-quality worker.
- Keep the content/hash mismatch lane open, especially the `standard.rs`
  mismatch where no exported `standard.rs` edit exists.
- Add a protocol/adjudication field for "test made green by changing expected
  output to observed output"; this should inspect the actual proposal diff, not
  only the local call rationale.
- Add a protocol/adjudication field for final validation scope mismatch when
  `cargo test` or `cargo check` resolves to an unrelated focused manifest.
- Persist malformed JSON retry telemetry for protocol adjudicators: rejected
  raw content, parse error, retry count, and final accepted attempt.
- Add a protocol packet field that distinguishes production-code edits from
  test-only or expected-output-only edits.
