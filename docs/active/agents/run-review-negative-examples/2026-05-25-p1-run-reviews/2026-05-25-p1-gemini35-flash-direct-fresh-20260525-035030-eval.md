# Prototype 1 Eval And Protocol Review: `p1-gemini35-flash-direct-fresh-20260525-035030`

## Verdict

This eval+protocol artifact is admissible evidence for continuing into child
planning, but not benchmark-clean evidence that the parent patch is correct.
The Prototype 1 eval path produced a non-empty Multi-SWE-bench submission for
`BurntSushi__ripgrep-2209`, and the submitted production change is plausibly
aimed at the benchmark bug: `Replacer::replace_all` now uses
`captures_iter_at`, stops replacement at the current multiline range, and
copies trailing bytes only up to that limit.

The follow-up protocol step completed mechanically and reviewed the whole trace:
73 of 73 tool calls, 17 of 17 intent segments, zero missing calls, and zero
missing segments. That is enough to advance to child planning if the next phase
treats this as a noisy training/evidence sample rather than oracle success.
Protocol did surface useful recovery chains, but it also over-credited final
validation and did not see the validation-audit manifest-scope warning.

The artifact is still suspicious for adjudication. The patch also adds a weak
`standard.rs` regression test after replacing an earlier unsupported-lookaround
test that failed at compile/runtime validation. The final model answer reports
useful package-qualified validation, but the last recorded cargo call was a
bare `cargo check` that resolved to the unrelated `globset` manifest. No
formatting check ran. The new validation audit caught the final-cargo and fmt
evidence gaps, and it flagged a test/assertion edit candidate, but its changed
path counts missed the exported `standard.rs` test change.

No blocker is established from this eval alone. The console-visible
`TOOL_EXECUTION_FAILED` / content-hash failures were recoverable in the durable
trace and did not corrupt patch packaging. They are still worth tracking as
protocol/adjudication candidates because they produced noisy internal errors
and stale semantic lookups after edits.

## Evidence Roots

- Repo branch checked for this review: `feature/ploke-loop`
- Campaign:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-fresh-20260525-035030`
- Parent worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-gemini35-flash-direct-fresh-20260525-035030`
- Parent node: `node-dfbca03c896b03ae`
- Run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260525-035030/BurntSushi__ripgrep-2209/runs/run-1779681074829-structured-current-policy-26ab7b57`
- Record:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-fresh-20260525-035030/BurntSushi__ripgrep-2209/runs/run-1779681074829-structured-current-policy-26ab7b57/record.json.gz`
- Validation audit:
  `validation-audit.json` in the same run root
- Protocol artifacts:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-direct-fresh-20260525-035030/BurntSushi__ripgrep-2209/runs/run-1779681074829-structured-current-policy-26ab7b57`

## Execution Path

This is the Prototype 1 eval state-machine path:

```text
prototype1-step -> run_planned_child -> runner.rs::run_benchmark_turn -> record.json.gz -> validation-audit -> packaging -> baseline_protocol
```

Evidence:

- `execution-log.json` records `run_arm.execution = agent-single-turn`.
- Recorded steps include `benchmark_turn_completed`,
  `write_validation_audit`, `write_msb_submission`, and
  `write_benchmark_patch_projection`.
- `closure-state.json` at `2026-05-25T04:04:25Z` has eval complete and
  protocol complete.

## Closure State

Closure state after the protocol step:

- `registry.status = complete`
- `eval.status = complete`
- `eval.complete_total = 1`
- `eval.failed_total = 0`
- `protocol.status = complete`
- required protocol procedures complete:
  `tool-call-intent-segments`, `tool-call-review`,
  `tool-call-segment-review`
- `protocol_counts.total_calls = 73`
- `protocol_counts.reviewed_calls = 73`
- `protocol_counts.total_segments = 17`
- `protocol_counts.usable_segments = 17`
- `protocol_counts.missing_segments = 0`

The campaign route is the intended direct Google route:

- `model_id = google/gemini-3.5-flash`
- `route_source = direct_google`
- run profile provider: `google`
- run profile route source: `direct-google`
- `execution.mbe.enabled = false`
- `selection.oracle.mode = record-only`
- `selection.oracle.require_evidence = true`

## Eval And Patch Output

`benchmark-patch-projection.json`:

- `check.status = passed`
- `submission.byte_len = 3698`
- `submission.line_count = 103`
- `submission.sha256 =
  a4f2c37781a886606bd8f23a2cc119f37ead92be17b20cb76894d84690509c21`
- checkout head stayed at base SHA
  `4dc6c73c5a9203c5a8a89ce2161feca542329812`

The exported patch changes two files:

- `crates/printer/src/util.rs`
- `crates/printer/src/standard.rs`

The production change in `util.rs` replaces the previous
`replace_with_captures_at` use with an explicit `captures_iter_at` loop. It
tracks `last_match`, stops when the match starts beyond a computed `limit`, and
copies trailing bytes only up to that limit. In multiline mode, `limit` is
`range.end`; otherwise it is the trimmed `subject.len()`.

Patch-quality concerns:

- The semantic edit removed the doc comment on `replace_all` and left odd
  indentation (`        pub fn replace_all<'a>(`). Cargo accepted this, but fmt
  was never checked.
- The first new regression test used `RegexMatcher::new(r"a\n(?=b)")`, which
  failed because the default regex engine does not support lookaround.
- The model replaced that failing test with `replacement_multi_line_simple`,
  using `RegexMatcher::new(r"b")` over `b\nb\n` and expecting
  `1:x\n2:x\n`. This is a passing test, but it is a weaker benchmark signal
  than the issue text's PCRE2-lookaround explanation.

## Validation Audit

The new validation-reporting prompt improved the model's final answer compared
to earlier runs: the final response names the package-qualified commands it
relies on and does not claim `cargo fmt` or rustfmt ran.

The new `validation-audit.json` also improved artifact evidence:

- It recorded every cargo call with resolved scope and manifest path.
- It correctly warned that the final cargo call was a bare `cargo check`
  resolving to `crates/globset/Cargo.toml`, which does not cover
  `crates/printer/src/util.rs`.
- It correctly warned that no formatting check was observed.
- It flagged an expected-output/assertion-style edit candidate for the
  `standard.rs` test rewrite at event index 587.

The audit is still incomplete:

- `patch_quality.changed_paths` lists only `crates/printer/src/util.rs`.
- `test_changed_path_count = 0`, even though the exported submission changes
  `crates/printer/src/standard.rs`.
- This appears to be using the benchmark expected-file surface rather than the
  final exported diff surface, so path-count and test-file fields are
  misleading for this run.

## LLM And Tool Behavior

Trace audit:

- provider responses: 74
- provider-emitted tool calls: 73
- recorded tool calls: 73
- missing provider call ids: 0
- extra recorded call ids: 0
- finish reasons: 73 `tool_calls`, 1 `stop`
- recorded classifications:
  - `context_results`: 23
  - `completed`: 16
  - `read_with_content`: 15
  - `empty_completed_read`: 8
  - `transport_failure`: 5
  - `duplicate_request`: 5
  - `empty_context`: 1

The model used useful tool output. Initial RAG context included
`replacement_multi_line` tests and `find_iter_at_in_context`; the model then
read `util.rs`, inspected matcher replacement APIs, validated with
`grep-printer` tests, responded to failures, and ended with a plausible
production patch.

Recovery examples:

- Calls 34-37: a wrong `code_item_lookup`/canon path failed, the model retried
  with `crate::util::Replacer::replace_all`, and an edit applied.
- Calls 39-42: a same-file semantic edit failed with `Content changed` for
  `util.rs`; the model-visible message said to refresh or re-resolve. The
  model read `util.rs` and used `non_semantic_patch` to apply the smaller
  limit correction.
- Calls 50 and 64-70: `insert_rust_item` failed for the test module, so the
  model fell back to `non_semantic_patch`. The unsupported-lookaround test then
  failed under cargo, and the model replaced it with a simpler passing test.

The recovery was real, but not clean. The model spent many calls on broad
searches and empty reads in `standard.rs`, including stale line ranges past the
file end. Those calls did not corrupt the artifact, but they were low
information and should not be over-credited.

## Content/Hash Failures

The durable trace does not persist the exact console string
`TOOL_EXECUTION_FAILED`; it persists the underlying structured failures.

Model-visible and recoverable:

- `function-call-b3e986c6-0723-49b8-8ef7-4080c8c225c1` failed because the
  `util.rs` file version could not be verified after the previous edit:
  `Content changed ... Refresh or re-resolve the target`.
- `function-call-c693af8f-2c8d-4bca-91ac-970357057c39` then failed
  `code_item_lookup` with `Internal compiler error: failed to read snippet:
  Content changed ... util.rs`.
- Both failures were delivered as tool messages, and the later
  `non_semantic_patch` applied the intended correction.

Not artifact-corrupting:

- `patch_artifact.all_proposals_applied = true`.
- `multi-swe-bench-submission.jsonl` is non-empty and includes the final
  `util.rs` and `standard.rs` diff.
- `benchmark-patch-projection.check.status = passed`.

Still suspicious:

- The console's repeated `ploke-io`/content-mismatch dumps for stale snippets
  are not represented as first-class protocol evidence in the eval artifact.
- The many empty `standard.rs` reads and stale lookup failures show that
  post-edit semantic/index lookups still degrade after file changes.

## Validation Claims

Accurate enough:

- The final model response claims `cargo test --package grep-printer` passed
  against the root `Cargo.toml`. The recorded package cargo test at call 70
  passed 95 tests and 2 doctests.
- The final model response claims `cargo check --package grep-printer` passed
  against the root `Cargo.toml`. The recorded package cargo check at call 71
  passed.
- The final response does not claim formatting passed.
- There are no MBE/oracle success claims; MBE is disabled and oracle mode is
  record-only.

Misleading or easy to over-credit:

- The final recorded cargo call, call 72, was a bare `cargo check` resolving to
  `crates/globset/Cargo.toml`; it is not changed-code evidence.
- The audit correctly flags that final-cargo scope issue, but the model's final
  answer omits that last unrelated check.
- Any summary that describes the final state as workspace-wide validation would
  be wrong; the useful evidence is package-level `grep-printer` test/check.

## Positive Examples And Adjudication Candidates

- `tool hint -> retry -> success`: the model corrected a bad method canon and
  eventually patched `Replacer::replace_all`.
- `stale semantic edit -> refresh -> smaller patch`: the model recovered from
  the visible `Content changed` failure by reading the updated file and using a
  non-semantic patch.
- `failed validation -> test rewrite -> pass`: the model used the failing
  unsupported-lookaround test output to replace the invalid test. This should be
  adjudicated carefully: the recovery is useful, but the replacement test is
  weaker and may not prove the benchmark behavior.
- `validation audit catches final scope`: the audit correctly distinguishes the
  last bare `globset` check from the earlier useful `grep-printer` validation.
- `protocol catches local recovery`: segment 6 covers calls 39-42, where
  semantic apply and stale lookup failures were followed by a direct read and
  successful `non_semantic_patch`.
- `protocol catches failed-test recovery`: segment 13 marks call 65 as failed
  validation with a clear next step; segments 14-15 then cover the refined
  `sink_slow_multi_line` lookup, direct read, and applied patch.

## Protocol Review

Protocol completion is mechanically solid:

- intent segmentation covered all 73 calls with 17 labeled segments, no
  ambiguous calls, and no uncovered calls.
- tool-call review emitted one artifact per call.
- segment review emitted one artifact per segment.
- closure records the required procedures as complete, with 73 reviewed calls
  and 17 usable segments.

The tool-call reviews are directionally useful. Their verdict counts were:

- usefulness: 44 `key_progress`, 20 `helpful_but_non_essential`, 7
  `low_value`, 2 `no_value`
- redundancy: 49 `distinct`, 12 `overlapping`, 10 `search_thrash`, 2
  `redundant_repeat`
- recoverability: 52 `no_recovery_needed`, 20 `clear_next_step`, 1
  `no_clear_recovery`
- overall: 46 `focused_progress`, 14 `mixed`, 8 `useful_exploration`, 3
  `recoverable_detour`, 2 `redundant_thrash`

Positive protocol behavior:

- Segment 4, calls 33-37, correctly treats the failed `code_item_lookup` and
  invalid `apply_code_edit` canon as recovered by a later successful
  `apply_code_edit` on `Replacer::replace_all`.
- Segment 6, calls 39-42, correctly records the stale semantic edit path:
  `apply_code_edit` failed, `code_item_lookup` hit a content-changed/internal
  read failure, the model refreshed with `read_file`, then applied the smaller
  `non_semantic_patch`.
- Segment 13, call 65, correctly treats the failed `grep-printer` test as
  useful validation feedback with a clear next step.
- Segment 14-15, calls 66-69, correctly distinguishes the empty
  `sink_slow_multi_line` search from the successful refined search, read, and
  later patch.
- Individual call review flags some low-information work, including call 60 as
  a redundant `standard.rs` read and call 66 as low-value empty context.

Protocol blind spots:

- Final validation is over-credited. Segment 16 says calls 70-72 ran final
  checks "across the codebase" and that the final `cargo check` was
  workspace-wide. `validation-audit.json` contradicts that for call 72: the
  final bare `cargo check` resolved to
  `crates/globset/Cargo.toml`, `covers_changed_files = false`. The protocol
  packet did not include the resolved manifest or changed-file coverage, so the
  adjudicator guessed from the short tool summary.
- The segmentation overall rationale says the agent corrected `standard.rs`
  replacement logic and confirmed the test suite passed completely. That is too
  strong. The exported diff includes a `standard.rs` test rewrite, and the
  audit's expected-output red flag is the right caveat.
- Protocol did not distinguish production edits from test/assertion rewrites.
  It treats `standard.rs` patches as direct key progress, but does not surface
  the possibility that the final passing test was weaker than the earlier
  failing lookaround test.
- The malformed-JSON retry behavior is not first-class evidence in the final
  artifact set. The orchestration log observed retries for tool-call reviews
  1, 25, 27, 36, 33, 37 and segment review 1, and the completed artifacts prove
  the retry path recovered. I did not find persisted retry-attempt metadata in
  the protocol artifact JSONs themselves.

Protocol is therefore useful as a trace reviewer, not a final semantic auditor.
For child planning, this run can be used as evidence that the loop can produce
a patch, run protocol to completion, and extract concrete recovery signals. It
should not be used as evidence that the benchmark oracle was satisfied or that
the final validation was strong.

## Action Items

- Non-blocker: extend `validation-audit.json` path accounting to use the final
  exported diff, not only benchmark expected-file changes, so test-file edits
  like `standard.rs` are counted.
- Non-blocker: feed validation-audit cargo resolution into protocol packets so
  adjudication can see `manifest_path`, `resolved_scope`, and
  `covers_changed_files`.
- Non-blocker: add an adjudication field for "test failure was repaired by
  weakening/replacing the test" versus "test failure caused a production fix."
- Non-blocker: protocol review should not treat the repeated
  `standard.rs` empty reads as useful just because they completed.
- Non-blocker: protocol review should preserve the difference between the
  useful package-level `grep-printer` validation and the final unrelated
  `globset` check.
- Non-blocker: persist protocol retry-attempt metadata, including malformed JSON
  target, retry count, and final recovered artifact path.
