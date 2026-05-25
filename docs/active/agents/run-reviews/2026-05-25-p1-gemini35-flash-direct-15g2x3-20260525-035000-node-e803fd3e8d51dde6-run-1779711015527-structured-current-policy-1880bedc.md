# Prototype 1 Run Review: Selected Child Handoff

Date: 2026-05-25

Campaign: `p1-gemini35-flash-direct-15g2x3-20260525-035000`

Selected node: `node-e803fd3e8d51dde6`

Runtime: `bc9a0f63-e627-4def-991e-773b5037c6e4`

Selected branch: `branch-821e452418987122`

Treatment run: `run-1779711015527-structured-current-policy-1880bedc`

Review status: complete with one explicit verification gap. The persisted
run artifacts reconstruct the provider/tool/edit/validation/export path, but
the recorded treatment checkout path no longer exists on disk, so final patch
verification is from the submitted patch, patch projection, and trace records
rather than a live `git diff` of the treatment worktree.

## Short Verdict

The selected child produced a non-empty patch and a valid benchmark patch
projection. It also showed a real recovery chain after one stale same-file
semantic edit failure: it refreshed `util.rs`, switched from semantic edit to
`non_semantic_patch`, then used cargo failures to repair the production change.

The controller's `keep` decision was not trace-grounded in the evaluation
artifact. The branch evaluation and successor selection justified `keep` with
surface operational metrics only: failed tool calls improved `9 -> 2`, same-file
retry count improved `7 -> 2`, and max same-file retry streak improved `4 -> 2`.
Independent trace review supports keeping the child as an operationally better
candidate, but not as a proven benchmark-correct or semantically superior patch.

The final validation supports "the final package compiles and its visible
`grep-printer` tests pass." It does not support stronger claims about PCRE2
look-around or the actual `#2095/#2208` CLI regression shape, because the added
test was weakened from `a(?=b)` to plain `a` after Rust regex rejected
look-around.

## Evidence Roots

- Node record:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-035000/prototype1/nodes/node-e803fd3e8d51dde6/node.json`
- Invocation:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-035000/prototype1/nodes/node-e803fd3e8d51dde6/invocations/bc9a0f63-e627-4def-991e-773b5037c6e4.json`
- Child channel:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-035000/prototype1/nodes/node-e803fd3e8d51dde6/channels/bc9a0f63-e627-4def-991e-773b5037c6e4/child-to-parent.jsonl`
- Runner result:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-035000/prototype1/nodes/node-e803fd3e8d51dde6/runner-result.json`
- Evaluation report:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-035000/prototype1/evaluations/branch-821e452418987122.json`
- Transition journal:
  `/home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-035000/prototype1/transition-journal.jsonl`
- Treatment registration:
  `/home/brasides/.ploke-eval/registries/runs/run-1779711015527-structured-current-policy-1880bedc.json`
- Treatment run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-20260525-035000/treatments/branch-821e452418987122/instances/BurntSushi__ripgrep-2209/runs/run-1779711015527-structured-current-policy-1880bedc`
- Patch projection:
  `<run-root>/benchmark-patch-projection.json`
- Submission:
  `<run-root>/multi-swe-bench-submission.jsonl`
- Validation audit:
  `<run-root>/validation-audit.json`
- Provider trace:
  `<run-root>/llm-full-responses.jsonl`
- Agent trace:
  `<run-root>/agent-turn-summary.json`

Record labels:

- `record present, manual join needed`: selected node, runtime invocation,
  child channel, treatment registration, treatment run root, evaluation report,
  and successor selection are all present, but the review had to join them by
  node id, branch id, runtime id, treatment campaign id, and run id.
- `record absent/current checkout unavailable`: the treatment checkout recorded
  in `run.json` and `benchmark-patch-projection.json` is not present now.
- `record absent`: treatment protocol artifacts are missing; treatment
  `closure-state.json` marks all three required protocol procedures missing.
- `operator/convenience record`: the trace audit script summarizes the run but
  is not the authority source.

## Execution Path

Artifact-proved path:

```text
successor selected broad-harness child
-> materialize_branch
-> build_child
-> child_artifact_committed
-> spawn_child --invocation <runtime>.json
-> child runner evaluates treatment campaign
-> structured-current-policy treatment run
-> agent-single-turn / run_benchmark_turn
-> validation audit
-> Multi-SWE submission
-> benchmark patch projection
-> branch evaluation
-> successor selection accepts branch disposition keep
```

Evidence:

- `node.json` names `node-e803fd3e8d51dde6`, parent
  `node-f4cf695decef97df`, generation `1`, branch
  `branch-821e452418987122`, candidate `broad-harness-g1-01`, target relpath
  `crates/ploke-db/src/bm25_index/mod.rs`, and status `succeeded`.
- `transition-journal.jsonl` records `materialize_branch` before/after,
  `build_child` result `built`, `child_artifact_committed` with target commit
  `aa26d65e22e66272951ad2284591a327f37edcfc`, `spawn_child` with argv
  `loop prototype1-runner --invocation ... --execute --format json`, and
  later `observe_child` result `treatment_complete`.
- `child-to-parent.jsonl` records `ready`, `evaluating`, and a final
  `runner_result` with status `succeeded`, exit code `0`, and treatment
  campaign
  `p1-gemini35-flash-direct-15g2x3-20260525-035000-treatment-branch-821e452418987122-1779711014424`.
- The run registration freezes `run_role: treatment`, `command:
  run single agent`, `execution: agent-single-turn`, provider `google`, model
  `google/gemini-3.5-flash`, execution status `completed`, patching
  `completed`, packaging `completed`, validation `skipped`, protocol
  `not_started`, and submission status `nonempty_patch`.
- `execution-log.json` lists `bootstrap_headless_runtime`,
  `benchmark_turn_completed`, `write_validation_audit`,
  `persist_full_response_trace`, `write_msb_submission`, and
  `write_benchmark_patch_projection`.

## 1. Patch And Projection

Yes. The selected child produced a non-empty patch, and the patch projection
passed.

Command/result evidence:

```text
jq '.' <run-root>/benchmark-patch-projection.json
```

Result:

```text
submission.byte_len = 3567
submission.line_count = 102
submission.diff_base = 4dc6c73c5a9203c5a8a89ce2161feca542329812
check.status = passed
check.detail = fix_patch exported from the recorded checkout cwd
```

Additional evidence:

- `stat` showed `multi-swe-bench-submission.jsonl` exists at `3751 bytes`,
  `record.json.gz` at `143198 bytes`, and `llm-full-responses.jsonl` at
  `178963 bytes`.
- `agent-turn-summary.json` has `patch_artifact.applied: true` and
  `all_proposals_applied: true`.
- The submitted `fix_patch` changes two files:
  `crates/printer/src/util.rs` and `crates/printer/src/standard.rs`.
- The expected benchmark file was only `crates/printer/src/util.rs`; the
  `standard.rs` change is an added visible test, not an expected gold file.

The final patch is mechanically valid as a projection, but its shape is not the
gold patch. The gold slice adds a helper `replace_with_captures_in_context` in
`util.rs` and does not add `standard.rs` unit-test coverage. The selected child
instead rewrote `Replacer::replace_all` inline, deleted the existing doc comment
above `replace_all`, left odd indentation on `pub fn replace_all`, and added a
plain regex unit test.

Current-checkout gap:

```text
git -C <recorded-treatment-checkout> status --short
```

Result:

```text
fatal: cannot change to '<recorded-treatment-checkout>': No such file or directory
```

So the live checkout cannot be used now to re-run `git diff`; use the submitted
patch and persisted projection as the authority for this review.

## 2. Validation And Final Claims

The agent ran cargo repeatedly. The useful final validation was:

- `cargo test` requested with `package: "grep-printer"` at provider response 54.
- Tool result event `492`: command `test`, scope `workspace`, manifest
  `<recorded-checkout>/Cargo.toml`, exit code `0`, stderr tail includes
  `Compiling grep-printer v0.1.6` and `Doc-tests grep_printer`, and stdout
  includes `test result: ok. 95 passed; 0 failed` plus `2 passed` doctests.
- `cargo check` requested with `package: "grep-printer"` and
  `include_warnings: true` at provider response 55.
- Tool result event `500`: command `check`, scope `workspace`, manifest
  `<recorded-checkout>/Cargo.toml`, exit code `0`, summary `errors: 0`,
  `warnings: 0`, and stderr tail includes `Checking grep-printer v0.1.6`.

The validation audit records all cargo calls:

```text
event 23   check  focused   ok=true  covers_changed_files=false  crates/pcre2/Cargo.toml
event 207  test   focused   ok=true  covers_changed_files=false  crates/pcre2/Cargo.toml
event 223  test   workspace ok=true  covers_changed_files=true   Cargo.toml
event 256  check  focused   ok=true  covers_changed_files=false  crates/pcre2/Cargo.toml
event 263  check  workspace ok=true  covers_changed_files=true   Cargo.toml
event 271  test   workspace ok=false covers_changed_files=true   Cargo.toml
event 311  check  workspace ok=true  covers_changed_files=true   Cargo.toml
event 319  test   workspace ok=false covers_changed_files=true   Cargo.toml
event 350  check  workspace ok=true  covers_changed_files=true   Cargo.toml
event 358  test   workspace ok=true  covers_changed_files=true   Cargo.toml
event 469  test   workspace ok=false covers_changed_files=true   Cargo.toml
event 492  test   workspace ok=true  covers_changed_files=true   Cargo.toml
event 500  check  workspace ok=true  covers_changed_files=true   Cargo.toml
```

The validation supports these final claims:

- The final visible `grep-printer` package tests passed.
- The final check passed with zero recorded warnings.
- The model used validation failures productively: a bounds panic led to
  `search_end`, output-duplication failures led to `last_match = range.start`,
  and unsupported regex look-around led to a test regex change.

The validation does not support these stronger claims:

- It does not prove the PCRE2 look-around issue. The added test originally used
  `a(?=b)`, failed because Rust regex does not support look-around, then was
  changed to `r"a"`.
- It does not prove the CLI regression cases from `#2095/#2208`; the gold
  `test_patch` adds CLI regression tests under `tests/regression.rs`, but this
  selected child added only an internal `standard.rs` unit test.
- It does not prove formatting. `validation-audit.json` warns:
  `no formatting check evidence recorded; do not claim cargo fmt/rustfmt passed`.
- It does not prove no semantic regression beyond the visible package tests.
  The audit also flags an expected-output/assertion edit candidate for changing
  the added test's regex from `a(?=b)` to `a`.

## 3. Keep Decision Support

The controller's persisted `keep` decision is metric-driven.

Command/result evidence:

```text
jq -r '.overall_disposition, .compared_instances[].evaluation.reasons[]' \
  /home/brasides/.ploke-eval/campaigns/p1-gemini35-flash-direct-15g2x3-20260525-035000/prototype1/evaluations/branch-821e452418987122.json
```

Result:

```text
overall_disposition = keep
tool_calls_failed improved: 9 -> 2
same_file_patch_retry_count improved: 7 -> 2
same_file_patch_max_streak improved: 4 -> 2
```

The successor entry in `transition-journal.jsonl` is also metric-only for the
accepted branch. It records `domain: operational`, `verdict: better`,
`confidence: high`, the same three metric improvements, and one evidence ref:
the branch evaluation JSON. The same selection record marks oracle evidence
`inconclusive`, with `oracle_evaluated_instances=0` and
`oracle_resolved_instances=0`.

Independent trace evidence supports an operational `keep`:

- The treatment run completed, exported a non-empty patch, and passed patch
  projection.
- The provider/recorded call ledger was complete: trace audit found 57
  responses, 56 provider-emitted tool calls, 56 recorded tool calls, and 0
  missing provider call ids.
- The run had fewer failed tool calls and fewer same-file retry failures than
  the baseline metrics recorded in the evaluation report.
- The model used cargo failures to repair two production mistakes.

Independent trace evidence does not support a stronger `keep` claim:

- There is no treatment protocol adjudication. Treatment `closure-state.json`
  marks protocol status `missing` for all three required procedures.
- There is no oracle-resolved benchmark result.
- The final test is weaker than the issue text and gold test shape.
- The final patch includes non-behavioral damage: removed public doc comment
  and odd indentation in `util.rs`.

Conclusion: `keep` is supported as an operational selection, not as a
trace-adjudicated semantic win.

## 4. Same-File And Context Failure Recovery

The child both avoided most of the baseline same-file failure pattern and
recovered from one concrete stale same-file failure.

Concrete chain:

```text
response 27 apply_code_edit
-> events 241/246 staged and applied util.rs
-> event 271 cargo test failed: range end index out of range
-> response 31 apply_code_edit on util.rs
-> events 279/280 failed: Content changed; Refresh or re-resolve
-> response 32 read_file util.rs lines 40-140
-> response 33 non_semantic_patch
-> events 298/302 staged and applied util.rs
-> event 319 cargo test failed with duplicated output
-> response 37 non_semantic_patch last_match = range.start
-> events 337/341 staged and applied util.rs
-> event 358 cargo test passed
```

The stale failure was real:

```text
Cannot stage apply_code_edit ... because the file version could not be verified:
Content changed ... Refresh or re-resolve the target before submitting another
semantic edit.
```

The recovery was also real. The model read the changed file after the failure,
then switched to a unified `non_semantic_patch`, and the later cargo feedback
became actionable.

A separate non-stale tool failure also recovered:

```text
response 47 insert_rust_item
-> event 422 failed: No inline module container found for crate::standard::tests
-> responses 48-50 searched/looked up test structure
-> response 51 non_semantic_patch appended the test
-> event 469 cargo test failed because a(?=b) is unsupported
-> response 53 changed the regex to r"a"
-> event 492 cargo test passed
```

This should not be over-credited. The child had only one stale same-file
semantic failure in the treatment trace. The lower same-file retry count could
mean better behavior, but it could also mean the run simply encountered fewer
stale edit opportunities than the baseline. Future adjudication needs the
recovery chain, not only the aggregate count.

## 5. Future LLM Adjudication Signals

Future adjudication should track these signals from this case:

- `metric-only selection`: distinguish a controller `keep` justified by metric
  deltas from a trace-grounded semantic verdict.
- `oracle/protocol absence`: keep `oracle_evaluated_instances=0`,
  `oracle_resolved_instances=0`, and missing treatment protocol artifacts in
  the selection record.
- `provider request vs tool resolution`: preserve both the provider request
  (`package: "grep-printer"`) and the tool result (`scope: workspace`,
  root manifest, stderr naming `grep-printer`).
- `validation failure use`: credit cargo failures only when the next model
  action addresses the observed failure and a later validation passes.
- `same-file recovery`: require the chain `stale failure -> refresh/read ->
  successful later edit`, not just a smaller retry count.
- `test fidelity`: compare added tests to the issue/gold signal. A test that
  was weakened from PCRE2 look-around to plain Rust regex should not receive
  full benchmark-evidence credit.
- `expected-output edits`: flag edits that make tests easier or change the
  assertion target, especially when validation passes only after that edit.
- `patch shape`: check expected files and unintended damage such as deleted
  doc comments or formatting churn, even when cargo passes.
- `checkout durability`: record whether the treatment checkout remains
  available for later `git diff`/`git apply --check` verification.

## Commands Run

Trace audit:

```text
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py \
  /home/brasides/.ploke-eval/instances/prototype1/p1-gemini35-flash-direct-15g2x3-20260525-035000/treatments/branch-821e452418987122/instances/BurntSushi__ripgrep-2209/runs/run-1779711015527-structured-current-policy-1880bedc \
  --markdown
```

Result:

```text
responses: 57
provider-emitted tool calls: 56
recorded tool calls: 56
missing recorded provider call ids: 0
extra recorded call ids: 0
finish reasons: {'tool_calls': 56, 'stop': 1}
recorded classifications:
  completed: 13
  context_results: 8
  read_with_content: 15
  empty_completed_read: 10
  duplicate_request: 8
  transport_failure: 2
```

Patch projection:

```text
jq '.' <run-root>/benchmark-patch-projection.json
```

Result:

```text
check.status = passed
submission.byte_len = 3567
submission.line_count = 102
```

Validation audit:

```text
jq -r '.cargo_calls[] | [.event_index,.call_id,.command,
  (.requested_scope//"null"),.resolved_scope,(.package//"null"),
  .ok,.covers_changed_files,.manifest_path] | @tsv' \
  <run-root>/validation-audit.json
```

Result: the final covering commands were event `492` `test ok=true` and event
`500` `check ok=true`, both covering changed files, with no formatting check
observed.

Patch extraction:

```text
jq -r '.fix_patch' <run-root>/multi-swe-bench-submission.jsonl
```

Result: a non-empty diff for `crates/printer/src/util.rs` and
`crates/printer/src/standard.rs`.

Treatment checkout check:

```text
git -C <recorded-treatment-checkout> status --short
```

Result:

```text
fatal: cannot change to '<recorded-treatment-checkout>': No such file or directory
```
