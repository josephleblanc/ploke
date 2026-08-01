# Run review: 090815 baseline eval/protocol tool-call correlation

Scope: `BurntSushi__ripgrep-2209` baseline run in campaign `p1-admissionfix-g35flash-p25flash-20260606-090815`.

Verdict: mechanically complete, with a patch/submission and complete protocol coverage, but not a clean benchmark-success signal. The tool ledger is internally joinable (52 provider tool calls, 52 recorded calls, 52 protocol call reviews), yet several protocol judgments over-credit completed/green tool results without checking `result.ok`, changed-file coverage, final-response capture, or expected-output edits. The candidate patch is plausible enough to be exported and package-validated, but it still needs benchmark/oracle review and should not be counted as a durable fix solely from protocol completion.

## Evidence roots

- Worker contract: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/WORKER_CONTRACT.md`
- Inventory: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/INVENTORY.json`
- Run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0`
- Record: `run-root/record.json.gz`
- Full provider trace: `run-root/llm-full-responses.jsonl`
- Agent event summary: `run-root/agent-turn-summary.json`
- Validation audit: `run-root/validation-audit.json`
- Patch projection: `run-root/benchmark-patch-projection.json`
- Submission: `run-root/multi-swe-bench-submission.jsonl`
- Protocol dir: `/home/brasides/.ploke-eval/protocol/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0`
- Protocol inventory counts: `total_calls=52`, `reviewed_calls=52`, `total_segments=7`, `usable_segments=7`.

I also ran the required trace audit:

```text
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py \
  /home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0 \
  --markdown
```

Audit summary:

```text
responses: 53
provider-emitted tool calls: 52
recorded tool calls: 52
missing recorded provider call ids: 0
extra recorded call ids: 0
finish reasons: {'tool_calls': 52, 'stop': 1}
recorded classifications: {'transport_failure': 8, 'completed': 18, 'read_with_content': 20, 'duplicate_request': 6}
```

## Execution path

The run artifacts identify this as the single-agent benchmark runner, not a broad headless-TUI slot:

```text
execution-log.json run_arm:
  id=structured-current-policy
  role=treatment
  command=run single agent
  execution=agent-single-turn
```

`execution-log.json` records the ordered path:

```text
load_manifest -> checkout_base_sha -> snapshot_expected_files_before_turn
-> write_repo_state -> init_runtime_db -> sandbox_config_home
-> embedding/index setup -> bootstrap_headless_runtime -> run_index_command
-> benchmark_turn_completed -> write_validation_audit
-> persist_full_response_trace -> snapshot_completed -> write_snapshot_status
-> write_msb_submission -> write_benchmark_patch_projection
```

Project code evidence points to:

```text
crates/ploke-eval/src/runner/msb_single.rs
  run single agent / structured-current-policy
  -> run_benchmark_turn(...)
  -> write_agent_turn_summary(...)
  -> build_agent_validation_audit(...)
  -> persist_full_response_trace_slice(...)
  -> run_record.add_turn_from_artifact(...)
  -> write_msb_submission_artifact(...)
  -> write_compressed_record(record.json.gz, ...)
  -> protocol projection over record/full-response artifacts
```

This matters because the relevant producer path is `runner/msb_single.rs::run_benchmark_turn` plus `record.rs` persistence and protocol projection. It is not the `tui_adapter::run_headless_with_model` broad-harness path.

## Closure state

The campaign inventory says eval and protocol are complete for this instance/run. The protocol surface is mechanically complete: 52/52 tool-call reviews and 7/7 segment reviews. This is a complete protocol pass over the tool sequence, not proof that every green call was semantically useful.

## Eval and patch output

Patch/submission artifacts exist:

- `benchmark-patch-projection.json`: `check.status=passed`, detail `fix_patch exported from the recorded checkout cwd`.
- `multi-swe-bench-submission.jsonl`: one submission with `fix_patch`, `line_count=104`, `byte_len=3745`, `diff_base=4dc6c73c5a9203c5a8a89ce2161feca542329812`.
- `validation-audit.json`: changed paths are `crates/printer/src/util.rs` and `crates/printer/src/standard.rs`.

The final patch changes `Replacer::replace_all` and adds `replacement_multi_line_look_around`. However, persisted checkout/source verification shows two quality concerns in the exported patch:

1. The doc comment on `replace_all` was removed and the replacement line is over-indented in the final checkout:

   ```text
   /home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/util.rs:50
   "        pub fn replace_all<'a>("
   ```

   This compiles, but it is a style/documentation regression that package tests do not catch.

2. The added test's expected output was changed after a failing test from `"1:x\n2-b\n"` to `"1:x\n"`. The final checkout and submission both contain `let expected = "1:x\n";` in `crates/printer/src/standard.rs`.

The validation audit also flagged this as suspicious:

```text
red_flags:
  edit request looks like a test expected-output/assertion change; inspect whether behavior changed
expected_output_edit_candidates:
  call_id=function-call-ce5d18bc-f244-434d-81d0-9da71278b162
  path=crates/printer/src/standard.rs
  evidence=-        let expected = "1:x\n2-b\n";
           +        let expected = "1:x\n";
```

That expected-output change may be semantically correct for the intended test, but the trace only proves that the adjusted test passed; it does not provide oracle evidence that the patch fixes the benchmark issue.

## Oracle / MBE state

I found no oracle/MBE verdict artifact in the assigned run root. The available evidence is exported patch + cargo validation + protocol reviews. Therefore the benchmark outcome is `candidate patch produced`, not `oracle-confirmed benchmark fix`.

## LLM/tool behavior summary

Provider and record ledgers line up mechanically:

- 53 provider responses in `llm-full-responses.jsonl`.
- Responses 0-51 ended with one tool call each.
- Response 52 ended with `finish_reason=stop` and a final narrative.
- 52 provider-emitted tool calls are present in `record.json.gz`.
- The trace audit found no missing or extra provider call IDs.

Recorded tool classifications from the audit:

```text
transport_failure: 8
completed: 18
read_with_content: 20
duplicate_request: 6
```

The main confirmed tool/procedure failures were:

| call | tool | recorded result | protocol gist | RCA |
|---:|---|---|---|---|
| 0 | `request_code_context` | Failed: `Internal compiler error: RAG service unavailable` | call review: `recoverable_detour`; first segment overalls as `focused_progress` | RAG/service availability failure. Agent recovered with directory browsing, but the failed call itself had no information value. |
| 9, 14 | `code_item_lookup` | Failed `invalid_format`: no `replace_all` with `module_path=crate::util::Replacer`, hint says retry with function / better module path | mixed; call 9/10 search-thrash cluster; call 14 key-progress via hint | Model/tool-input mismatch, recoverable. Not a system bug by itself, but duplicate failed request shows avoidable thrash. |
| 11 | `code_item_lookup` | Failed `internal`: `stored relation 'impl' does not have field 'name'` | mixed; no clear recovery | System/tool bug in `code_item_lookup` over impl relation schema. Candidate active-bug update. |
| 22 | `apply_code_edit` | Failed `invalid_format`: method canon must look like `crate::module::Type::method` | segment 22-24 mixed, clear recovery | Model/tool-input mismatch, immediately recovered by call 23 with `crate::util::Replacer::replace_all`. |
| 24 | `cargo check --package grep-printer` | ToolCompleted but `ok=false`, `compile_failed`, E0616 private `Match` fields | protocol treats as key progress / clear next step | Correctly useful validation, but lifecycle status is `Completed`; protocol must inspect `result.ok/status_reason` to avoid counting green tool completion as pass. |
| 25 | `apply_code_edit` | Failed `io`: content changed; refresh/re-resolve target | segment 25-27 says no clear recovery | Edit lifecycle stale-file boundary after an earlier applied semantic edit. Expected lifecycle guard, but should be typed as stale/refresh rather than generic IO for protocol. |
| 26 | `code_item_lookup` | Failed `internal`: `failed to read snippet: Content changed` | segment 25-27 says no clear recovery | Same stale-file/read-side problem; this is a tool/protocol cause, not model behavior. |
| 29 | `cargo check --package grep-printer` | ToolCompleted but `ok=false`, `compile_failed`, E0282 type annotation needed | segment 28-33 focused progress | Useful validation failure, later fixed by call 31. Same `ToolCompleted` vs semantic failure split. |
| 44 | `insert_rust_item` | Failed `invalid_format`: no inline module container for `crate::standard::tests` | recoverable detour / segment 34-46 mixed | Structured insertion tool could not resolve test module container; direct patch recovered. Candidate docs/protocol improvement for container-path requirements. |
| 47 | `cargo test --package grep-printer` | ToolCompleted but `ok=false`, `tests_failed_or_runtime`; failing test `standard::tests::replacement_multi_line_look_around` | protocol calls this key progress with clear recovery | Correct useful validation failure. It triggered the suspicious expected-output edit at call 48. |
| 50 | `cargo test` | ToolCompleted `ok=true`, but `scope=focused`, `manifest_path=.../crates/globset/Cargo.toml`, `covers_changed_files=false` in validation audit | protocol overstates it as broad/workspace regression coverage | Tool result/agent claim mismatch. The command looked broad, but the result was focused on `globset`, not the changed `grep-printer` files. |

## Concrete trace chains

### Chain A: semantic edit -> compile failures -> direct patch recovery

```text
response 22 -> call 22 apply_code_edit(crate::util::replace_all)
  -> ToolFailed invalid canon: method targets must look like crate::module::Type::method
response 23 -> call 23 apply_code_edit(crate::util::Replacer::replace_all)
  -> ToolCompleted applied=1 to crates/printer/src/util.rs
call 24 cargo check --package grep-printer
  -> ToolCompleted ok=false compile_failed E0616 private Match fields: m.start, m.end
call 25 apply_code_edit same target
  -> ToolFailed content changed; refresh or re-resolve target
call 26 code_item_lookup replace_all
  -> ToolFailed internal: failed to read snippet: Content changed
call 27 read_file crates/printer/src/util.rs:40-115
  -> ToolCompleted read_with_content
call 28 non_semantic_patch
  -> changes m.start/m.end to m.start()/m.end()
call 29 cargo check --package grep-printer
  -> ToolCompleted ok=false compile_failed E0282 type annotations needed at Ok(false)
call 30 read_file util.rs:80-105
  -> ToolCompleted read_with_content
call 31 non_semantic_patch
  -> adds closure return type `-> Result<bool, ()>`
call 32 cargo check --package grep-printer
  -> ok=true
call 33 cargo test --package grep-printer
  -> ok=true, 94 tests + doc tests pass
```

This is a useful recovery chain. Protocol correctly sees progress overall, but a protocol consumer must not treat calls 24/29 as successful cargo validation just because the tool lifecycle status is `Completed`.

### Chain B: test insertion -> failed test -> suspicious expected-output edit

```text
call 44 insert_rust_item crate::standard::tests
  -> ToolFailed: No inline module container found
call 45 read_file standard.rs:3280-3310
  -> ToolCompleted read_with_content
call 46 non_semantic_patch standard.rs
  -> adds `replacement_multi_line_look_around` with expected `1:x\n2-b\n`
call 47 cargo test --package grep-printer
  -> ToolCompleted ok=false; failing test `standard::tests::replacement_multi_line_look_around`
response 48 says: `b\n is not a match and context is not enabled`
call 48 non_semantic_patch standard.rs
  -> changes expected output to `1:x\n`
call 49 cargo test --package grep-printer
  -> ok=true, 95 tests + doc tests pass
```

I verified this suspicious result against persisted artifacts and checkout:

- `validation-audit.json` flags call 48 as an expected-output/assertion edit.
- `config/ploke/proposals.json` records call 46 adding `"1:x\n2-b\n"` and call 48 replacing it with `"1:x\n"`.
- The final checkout at `/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/printer/src/standard.rs:3315` contains `let expected = "1:x\n";`.
- The exported `multi-swe-bench-submission.jsonl` patch also contains `let expected = "1:x\n";`.

This proves the test was adjusted after failure. It does not prove the adjustment is wrong, but it makes the package-green signal weaker than a test written from independent oracle behavior.

### Chain C: final validation claim mismatch

```text
response 50 says it will run a full cargo test of the workspace
call 50 cargo {"command":"test"}
  -> ToolCompleted ok=true
  -> scope=focused
  -> manifest_path=/home/brasides/.ploke-eval/repos/BurntSushi/ripgrep/crates/globset/Cargo.toml
validation-audit.json call 50:
  -> covers_changed_files=false
```

Protocol segment review 47-51 describes call 50 as broader regression coverage. The persisted tool result contradicts that: it was focused on `globset` and did not cover changed files. The final model answer avoided the full-workspace claim and cited package-specific commands, but protocol still over-credited the segment.

## Protocol review correlation

Protocol review artifacts are present and complete, but their useful/suspicious labels need read-side qualification:

- Individual call reviews correctly identify many recovery opportunities and search-thrash clusters.
- Segment 0-3 overstates the RAG failure recovery as `focused_progress`; the actual failed call had `NoValue`, and the list-dir fallback only found a directory, not the target code.
- Segment 4-21 correctly flags search thrash around duplicate `code_item_lookup` and overlapping reads.
- Segment 22-24 properly sees a corrected edit attempt, but it relies on summary-level cargo information. The raw cargo result is `ok=false compile_failed` for call 24.
- Segment 25-27 correctly says no clear recovery; the root cause is stale-file/version verification after an applied edit plus a code lookup read-side failure, not an arbitrary model mistake.
- Segment 28-33 is a strong positive chain: compile error -> read -> patch -> check/test green.
- Segment 34-46 underplays repeated/scattered reads before the failed `insert_rust_item`. It does capture recovery through `non_semantic_patch`.
- Segment 47-51 overstates final validation. It sees successful `cargo test`/`cargo check`, but misses that call 50's `cargo test` resolved to `crates/globset` and did not cover changed files.

## Protocol / read-side blind spots

1. `ToolCompleted` is not semantic success. Cargo calls 24, 29, and 47 were lifecycle-completed but semantically failed (`ok=false`). Protocol must inspect `result.ok`, `status_reason`, diagnostics, and manifest path.
2. Tool result scope matters. Call 50 was protocol-reviewed as broad validation, but the persisted result says `scope=focused` and `manifest_path=.../crates/globset/Cargo.toml`.
3. Expected-output edits after failing tests need special treatment. The validation audit flagged call 48, but protocol segment review still treated the sequence as straightforward recovery.
4. Final assistant capture is incomplete in `record.json.gz`: `llm-full-responses.jsonl` contains response 52 with `finish_reason=stop` and a final answer, but `record.json.gz -> phases.agent_turns[0].agent_turn_artifact.final_assistant_message` is `null`, and `outcome` is `ToolCalls(count=52)`. This is a record/playback gap: the final answer exists, but record playback requires a manual join to `llm-full-responses.jsonl`.
5. Stale-file failures are not typed cleanly for protocol. Calls 25 and 26 report content-changed/stale state through `io` or `internal` error channels instead of a first-class stale-refresh lifecycle result.
6. RAG service outage is a tool/service availability failure. Protocol should not let an alternate directory walk erase the fact that the intended semantic context tool returned no value.

## What is working

- Provider call IDs and recorded call IDs match exactly.
- Protocol coverage is mechanically complete: all 52 calls reviewed, all 7 segments reviewed.
- Run artifacts are sufficient to reconstruct the tool chain manually.
- Validation audit usefully catches changed-file coverage and expected-output edits.
- The agent recovered from several failures: invalid edit canon, private field compile errors, closure type inference, and failed structured test insertion.
- A patch was exported and package-level `grep-printer` validation passed after the final relevant edit.

## What is not working yet

- Protocol completion is not a semantic success auditor.
- Cargo tool summaries can mislead when `scope` and `manifest_path` are not inspected.
- The final model response is absent from the main record despite being present in the full-response trace.
- Stale-file/version guard failures after edits are difficult to classify from protocol summaries.
- The exported patch has quality issues not caught by cargo: removed method docs and an over-indented `pub fn` line.
- No oracle/MBE artifact proves benchmark success.

## Action items

1. Protocol/read-side gap: Make tool-call review consume `result.ok`, `status_reason`, `manifest_path`, and changed-file coverage for cargo calls. This is tied to observed calls 24, 29, 47, and 50.
2. Protocol blind spot: Promote validation-audit `expected_output_edit_candidates` into protocol context, or add a protocol rule for test assertion edits immediately after failing tests. This is tied to call 48.
3. Artifact/playback gap: Fix `record.json.gz` final-answer capture so a `finish_reason=stop` final response in `llm-full-responses.jsonl` is represented in `agent_turn_artifact.final_assistant_message` / turn outcome. This is tied to response 52 vs `final_assistant_message=null`.
4. Tool/lifecycle bug candidate: Update or create an active bug for `code_item_lookup` over `node_kind=impl` failing with `stored relation 'impl' does not have field 'name'`. This is tied to call 11.
5. Tool/lifecycle gap: Type content-changed semantic edit and lookup failures as a first-class `stale_file_version` / `refresh_required` result instead of generic `io` or `internal`. This is tied to calls 25 and 26.
6. Protocol scoring adjustment: Do not score RAG service-unavailable recovery as focused progress unless the fallback retrieved target code. This is tied to call 0 and segment 0-3.
7. Benchmark-usefulness gate: Require oracle/MBE evidence, or at least independent reproduction evidence, before counting this patch as a durable benchmark fix. The current highest verified gate is `patch exported + grep-printer package validation`, not oracle pass.
8. Patch-quality follow-up: Review the exported patch for style/docs regression (`replace_all` doc removal and indentation) before using it as a candidate training/eval success example.

## Bottom line

This run is a good trace-bearing review fixture because all ledgers join, multiple recoveries are visible, and validation audit catches issues protocol misses. It should be counted as mechanically complete and protocol-complete. It should not be counted as a clean benchmark win without oracle/MBE or human patch review, and protocol should be improved to avoid over-crediting completed-but-failed cargo calls, focused validation mislabeled as workspace coverage, expected-output edits, and missing final-response capture.
