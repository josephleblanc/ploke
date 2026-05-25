# Prototype 1 Protocol Follow-up Review: Gemini 3.5 Flash Direct, ripgrep-2209

Date: 2026-05-25

Campaign: `p1-gemini35-flash-direct-15g2x3-20260525-035000`

Run: `run-1779706500140-structured-current-policy-bc499ae1`

Task: `BurntSushi__ripgrep-2209`

Review status: complete. The run-review Quality Gate passes because the
protocol artifact family is now present, the eval trace can be joined to the
protocol judgments, and the suspicious protocol/cargo/JSON results below were
checked against persisted artifacts rather than inferred from status fields.

## Short Verdict

The prior eval review is now stale only on protocol availability. It correctly
reviewed the eval as mechanically complete and benchmark-useful but not
benchmark-proven. Its protocol warning was true when written, but after the
baseline protocol step completed the campaign closure state now reports eval
complete, protocol complete, and all three required protocol procedures
complete.

The completed protocol evidence is useful, but it should not be read as a
benchmark-correctness verdict. Protocol reconstructed a coherent workflow from
the eval trace, emitted full call and segment coverage, and correctly penalized
many failed same-file edit calls. It also smoothed over that edit-failure
cluster at segment level, under-credited the final successful patch call, lacked
resolved cargo-scope visibility for a misleading bare `cargo check`, and had no
hidden/gold benchmark oracle signal.

One adjudication branch persisted malformed raw JSON content, but the normalized
procedure output was present and the coverage counts were complete. Final
evidence was not missing because of that malformed branch. The remaining gap is
observability: the artifacts do not record whether a retry happened before the
repair-normalized output was persisted.

## Reconciliation With The Prior Eval Review

The prior review file:

`docs/active/agents/run-reviews/2026-05-25-p1-gemini35-flash-direct-15g2x3-20260525-035000-burntsushi-ripgrep-2209-run-1779706500140-eval.md`

said the required protocol suite was missing and that the protocol directory
contained only `tool_call_intent_segmentation`. That statement was a point-in-
time observation from before the baseline protocol step finished.

Current closure state at:

`/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-035000/closure-state.json`

now reports:

- registry status: complete, 1 of 1 mapped
- eval status: complete, 1 of 1 complete
- protocol status: complete, 1 of 1 full
- required procedure `tool-call-intent-segments`: complete, 1 of 1
- required procedure `tool-call-review`: complete, 1 of 1
- required procedure `tool-call-segment-review`: complete, 1 of 1

The prior eval conclusions about patch usefulness, cargo caveats, and benchmark
limits still stand. Only the "protocol missing" conclusion has been superseded
by newer artifacts.

## Evidence Roots

- Eval run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-20260525-035000/BurntSushi__ripgrep-2209/runs/run-1779706500140-structured-current-policy-bc499ae1`
- Protocol artifact root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-gemini35-flash-direct-15g2x3-20260525-035000/BurntSushi__ripgrep-2209/runs/run-1779706500140-structured-current-policy-bc499ae1`
- Closure state:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-035000/closure-state.json`
- Dataset slice:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-035000/slice.jsonl`
- Final checkout:
  `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep`

The read-only trace audit reported 87 provider responses, 86 provider-emitted
tool calls, 86 recorded tool calls, 0 missing provider call ids, 0 extra
recorded call ids, 86 `tool_calls` finishes, and 1 final `stop`.

## Execution Path

Artifact-proved path:

```text
Prototype 1 eval step
-> run_benchmark_turn
-> agent-turn trace and validation audit
-> full provider response trace
-> Multi-SWE submission and benchmark patch projection
-> protocol step
-> tool_call_intent_segmentation
-> 86 tool_call_review artifacts
-> 12 tool_call_segment_review artifacts
-> closure-state protocol complete
```

This matters because the protocol was reviewing the already-persisted eval
trace, not re-running the model and not executing the benchmark.

## Procedure Artifacts Emitted

The protocol directory now contains exactly the expected procedure families for
this 86-call trace:

- 1 `tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json`
- 86 `tool_call_review_BurntSushi__ripgrep-2209.json` artifacts
- 12 `tool_call_segment_review_BurntSushi__ripgrep-2209.json` artifacts

Closure counts agree with the directory:

- total calls: 86
- reviewed calls: 86
- total segments: 12
- usable segments: 12
- mismatched segments: 0
- missing segments: 0

The closure state's `parse_failure` path is named but no parse-failure file
exists at that path. The complete procedure counts are the authority-bearing
evidence for this review.

## What Protocol Saw From The Eval Trace

Intent segmentation covered every call:

- total calls: 86
- labeled calls: 86
- uncovered calls: 0
- ambiguous calls: 0
- labeled segments: 12
- ambiguous segments: 0

The protocol's own rationale described a structured workflow: locate
replacement logic, inspect matcher replacement behavior, inspect printer
integration, edit, recover from compiler and test failures, and validate with a
new test.

Segment labels:

| Segment | Calls | Label | Review result |
| --- | ---: | --- | --- |
| 0 | 0-3 | `inspect_candidate` | focused progress |
| 1 | 4-7 | `locate_target` | focused progress |
| 2 | 8-21 | `inspect_candidate` | mixed, search thrash |
| 3 | 22-23 | `inspect_candidate` | focused progress |
| 4 | 24-38 | `refine_search` | mixed, search thrash |
| 5 | 39-41 | `inspect_candidate` | focused progress |
| 6 | 42-58 | `edit_attempt` | focused progress |
| 7 | 59-65 | `recovery` | mixed, clear next step |
| 8 | 66-66 | `validate_hypothesis` | focused progress |
| 9 | 67-71 | `locate_target` | focused progress |
| 10 | 72-82 | `validate_hypothesis` | focused progress |
| 11 | 83-85 | `validate_hypothesis` | focused progress |

This segmentation is broadly faithful to the eval trace. The main caution is
that "focused progress" at segment level can hide the cost of repeated failed
edits inside a segment.

## Malformed Adjudication JSON And Retry Effect

The protocol artifacts contain 295 persisted `raw_content` judgment strings:

- 294 parse as strict JSON
- 1 does not parse as strict JSON

The malformed raw string is in:

`1779707595050_tool_call_review_BurntSushi__ripgrep-2209.json`

It belongs to focal call 23, a `cargo check` call for `grep-matcher`. The bad
raw branch is the redundancy judgment: it has a complete object body but is
missing the final object close. The artifact's normalized output still contains
a complete redundancy judgment:

- verdict: `distinct`
- confidence: `high`
- overall call review: `focused_progress`
- usefulness: `helpful_but_non_essential`
- recoverability: `no_recovery_needed`

Current code explains how this can happen. `ploke-protocol` first tries strict
JSON parsing, then applies small JSON repairs such as missing final object
close repair before returning a parse error. `ploke-eval` has retry loops for
tool-call and segment review branches, with a maximum of three attempts.

What can be said from artifacts:

- The malformed raw branch did not remove final evidence: all 86 call reviews
  and all 12 segment reviews exist.
- It did not make the procedure incomplete: closure reports full protocol
  completion.
- The normalized judgment for the malformed branch is present and usable.

What cannot be said from artifacts:

- Whether this run made a retry before the repair-normalized result was
  persisted.
- How many transient malformed responses, if any, occurred before final
  artifacts were written.

So the right conclusion is narrow: malformed adjudication JSON did not affect
final coverage for this run, but retry/repair provenance is not observable
enough to audit retry behavior after the fact.

## Same-File Edit Failure Credit

The trace's most important protocol-quality issue is the call 42-58 same-file
edit cluster.

Eval trace:

- Call 42 inserted the initial helper into `crates/printer/src/util.rs`.
- Calls 44, 46, 48, 50, 52, 54, 56, and 57 then hit stale or fuzzy same-file
  edit failures.
- Call 58 finally succeeded with a precise three-hunk `non_semantic_patch`.

Per-call protocol review mostly saw the failures:

| Call | Tool | Failed | Overall | Usefulness | Redundancy | Recoverability |
| ---: | --- | --- | --- | --- | --- | --- |
| 44 | `apply_code_edit` | true | useful exploration | low value | distinct | clear next step |
| 46 | `non_semantic_patch` | true | recoverable detour | no value | search thrash | clear next step |
| 48 | `non_semantic_patch` | true | recoverable detour | no value | search thrash | clear next step |
| 50 | `apply_code_edit` | true | recoverable detour | no value | distinct | clear next step |
| 52 | `non_semantic_patch` | true | recoverable detour | no value | distinct | clear next step |
| 54 | `non_semantic_patch` | true | recoverable detour | no value | overlapping | clear next step |
| 56 | `apply_code_edit` | true | recoverable detour | no value | distinct | clear next step |
| 57 | `non_semantic_patch` | true | useful exploration | low value | distinct | clear next step |

That is a useful signal. The protocol did not simply count failed tool calls as
success.

The segment review then over-smoothed the same evidence. Segment 6, covering
calls 42-58, records 8 failed calls in scope and candidate concerns
`FilePivot` and `RecoveryOpportunity`, but its final segment judgment is:

- overall: `focused_progress`
- usefulness: `key_progress`
- redundancy: `distinct`
- recoverability: `no_recovery_needed`

That segment verdict over-credits the edit cluster if used as a run-quality
summary. A segment with eight same-file edit failures should retain recovery
pressure even when the final call succeeds.

There is also an under-credit: call 58, the successful patch that ended the
edit loop, was judged only `useful_exploration` and
`helpful_but_non_essential`. In trace terms it was essential: without call 58
the implementation was still blocked on failed same-file edits.

## Cargo Visibility

The protocol saw final validation activity, but it lacked the resolved cargo
scope needed to judge one important misleading call.

Eval trace:

- Call 83 requested a bare `cargo check`.
- Event 735 resolved that request to focused scope under
  `crates/globset/Cargo.toml`, which did not cover the changed printer files.
- Call 84 requested `cargo check` for package `grep-printer`.
- Event 742 succeeded from the root manifest and stderr included
  `Checking grep-printer v0.1.6`.
- Call 85 requested `cargo test` for package `grep-printer`.
- Event 750 succeeded from the root manifest and stdout included
  `standard::tests::replacement_multi_line_lookahead_bug ... ok`.
- `validation-audit.json` reports `successful_cargo_covering_changed_files:
  true`, `final_cargo_covers_changed_files: true`, and no formatting check
  observed.

Protocol judgments:

- Call 83 was summarized as a successful `cargo check` and reviewed as mixed
  but `helpful_but_non_essential`; its rationale calls it a global cargo check.
- Call 84 was judged `redundant_thrash` and `low_value`.
- Call 85 was judged `focused_progress` and `key_progress`.

The protocol was directionally right that final validation included redundant
cargo pressure and a useful final test. It still over-credited call 83 because
the protocol packet did not carry the resolved manifest path or changed-file
coverage result. The eval-local validation audit had that information; the
protocol adjudicator did not.

## Benchmark Usefulness

The eval output remains benchmark-useful:

- The submitted patch is non-empty.
- It changes `crates/printer/src/util.rs`, the expected production target.
- It also adds a local regression-style unit test under
  `crates/printer/src/standard.rs`.
- Final local cargo check/test evidence covers the changed files.
- `benchmark-patch-projection.json` reports the submission projection passed.

It is not benchmark-proven:

- The local profile did not run an MBE oracle.
- The protocol artifacts do not contain hidden-test or gold-oracle evidence.
- The gold slice changes `crates/printer/src/util.rs` and adds CLI regression
  tests `r2095` and `r2208` under `tests/regression.rs`.
- The submitted test uses `RegexMatcher::new(r"a")`, not the issue's
  PCRE2/look-around CLI reproduction.

The protocol reviews trace behavior, not benchmark correctness. A protocol
summary that says the run made focused progress should not be promoted to "this
solves ripgrep-2209" without separate oracle or benchmark evidence.

## Last Point With Enough Information To Act

The implementation had enough trace evidence to act after call 66: the
production helper had been patched, the copied-prefix bug had been fixed, and
`cargo test` passed. The later test-writing sequence improved local evidence
but also introduced more churn.

For final eval acceptance, the strongest local stopping point was call 85:
package-intended `grep-printer` test passed from the root manifest and included
the newly added test. That still did not establish hidden benchmark correctness.

## Action Items For Protocol And Adjudication Signals

1. Persist adjudication retry and repair provenance in protocol artifacts.
   Artifact `1779707595050_tool_call_review_BurntSushi__ripgrep-2209.json`
   proves that a repaired malformed raw branch can be normalized successfully,
   but the artifact does not say whether repair was applied, whether retry was
   attempted, or which branch needed repair.
2. Add resolved cargo fields to protocol packets: requested package, resolved
   manifest path, resolved scope, changed-file coverage, and whether the final
   cargo call covered changed files. Call 83 was over-credited because protocol
   could not see that event 735 resolved to `crates/globset/Cargo.toml`.
3. Preserve recovery pressure in segment synthesis. Segment 6 had 8 failed
   calls and `RecoveryOpportunity` in its signals, but ended with
   `no_recovery_needed`. Segment-level summaries should not erase repeated
   stale same-file edit failures after a final success.
4. Credit successful unblock calls relative to preceding failures. Call 58 was
   the successful patch that ended the same-file edit loop, but its per-call
   usefulness was only `helpful_but_non_essential`.
5. Separate local-validation strength from benchmark-oracle strength in
   protocol outputs. This run has good local cargo evidence and a non-empty
   exported patch, but no MBE/gold/hidden-test evidence.

## Verification Commands

- `python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py <run-root> --markdown`
  - succeeded; reported 87 responses, 86 provider tool calls, and 86 recorded
    tool calls
- `jq '{campaign_id, status, registry, eval, protocol, required_procedures, updated_at_ms}' <closure-state.json>`
  - succeeded; eval and protocol are complete
- `find <protocol-root> -maxdepth 1 -type f -printf '%f\n' | sed 's/^[0-9]*_//' | sort | uniq -c`
  - succeeded; found 1 intent segmentation, 86 call reviews, and 12 segment
    reviews
- `jq -r '.. | objects | select(has("raw_content")) | .raw_content | try (fromjson | "ok") catch "bad"' <protocol-root>/*.json | sort | uniq -c`
  - succeeded; found 294 strict JSON raw branches and 1 malformed raw branch
- `jq '{changed_paths, final_cargo_call, final_cargo_covers_changed_files, fmt_check_observed, successful_cargo_covering_changed_files, warnings, cargo_calls}' <validation-audit.json>`
  - succeeded; final cargo covers changed files, with no formatting check
    observed
- `jq -r '.fix_patch' <multi-swe-bench-submission.jsonl> | rg '^diff --git|^\\+.*replacement_multi_line_lookahead_bug|^\\+.*RegexMatcher::new|^\\+fn replace_with'`
  - succeeded; confirmed the submitted patch shape
- `jq -r '.fix_patch, .test_patch' <slice.jsonl> | rg '^diff --git|^\\+rgtest!|^\\+fn replace_with|^\\+.*r2208|^\\+.*r2095'`
  - succeeded; confirmed the gold patch/test shape
- `jq -r '.fix_patch' <multi-swe-bench-submission.jsonl> | git -C /home/brasides/.ploke-eval/repos/BurntSushi/ripgrep apply --numstat --summary --check --cached --verbose -`
  - succeeded against the exported patch; no patch was applied
