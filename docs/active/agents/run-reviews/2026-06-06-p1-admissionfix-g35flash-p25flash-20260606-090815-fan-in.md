# Fan-in synthesis: p1-admissionfix-g35flash-p25flash-20260606-090815

Status: final quality-gated fan-in for the 090815 run-review board. This report indexes the child reports, separates durable trace-bearing evidence from incomplete-state evidence, and states the next repair/rerun gate. It does not claim that the Prototype 1 campaign produced an admitted child or benchmark win.

## Short verdict

The 090815 campaign has two distinct evidence planes:

1. Baseline eval/protocol is mechanically complete and trace-bearing. The run root for `BurntSushi__ripgrep-2209` has `record.json.gz`, `llm-full-responses.jsonl`, validation audit, benchmark patch projection, Multi-SWE-bench submission, and protocol artifacts. The trace audit reconciles 53 provider responses, 52 provider-emitted tool calls, 52 recorded tool calls, and no missing recorded provider call ids.
2. Prototype 1 broad child admission is mechanically negative and benchmark-useless. The parent published 10 edit-harness requests and 10 clean candidate workspaces, but `prototype1/messages/edit-harness-result/` is absent, there are zero submitted results, zero `.headless-tui.json` diagnostics, and the child-plan rejected all 10 slots. These are request-only failures, not artifact-backed timeouts, provider failures, or model-quality failures.

The highest verified gate for this campaign is therefore: baseline `patch exported + package-level validation + complete protocol, pending semantic/oracle review`; broad harness `request-only / no diagnostics / zero admitted children`. Do not advance or count the campaign as benchmark-successful until the broad-headless diagnostics repair is in place and a fresh campaign or fixture proves typed slot outcomes.

## Evidence roots

Required method inputs read:

- Worker contract: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/WORKER_CONTRACT.md`
- Inventory: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/INVENTORY.json`
- Project run-review procedure: `docs/workflow/skills/ploke-run-review/SKILL.md`
- EvalOps procedure: `docs/workflow/skills/prototype1-evalops/SKILL.md`

Primary artifact roots:

- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815`
- Baseline run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0`
- Protocol root: `/home/brasides/.ploke-eval/protocol/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0`
- Child plan: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/child-plan/node-0cdf3741b09283fe.json`
- Broad requests: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/edit-harness-request/`
- Expected broad results: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/edit-harness-result/`

Child reports read and classified:

| report | gate classification | README/index treatment |
| --- | --- | --- |
| `2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-coverage-status.md` | passes as a coverage/status review; not a benchmark-success claim | index as durable coverage/status evidence |
| `2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-baseline-protocol-tool-correlation.md` | passes as a trace-bearing baseline/protocol run review | index as durable trace-bearing run review |
| `2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-parent-request-only-rca.md` | passes as incomplete-state RCA; explicitly not trace-bearing | index only as incomplete evidence/RCA |
| `2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-slots-base-r5-incomplete.md` | passes as incomplete-state slot evidence; explicitly no model/tool trace | index only as incomplete evidence |
| `2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-slots-r6-r10-incomplete.md` | passes as incomplete-state slot evidence; explicitly no model/tool trace | index only as incomplete evidence |
| `2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-bug-synthesis.md` | passes as RCA/bug-doc synthesis over the quality-gated child reports | index as durable synthesis evidence |
| this fan-in report | passes as final coverage/failure/blocker synthesis | index as durable fan-in synthesis |

## Run-review quality gate assessment

The child reports are usable because they satisfy the project-local run-review gate instead of stopping at counts:

- Exact execution path: baseline report names `msb_single.rs::RunMsbSingleRequest::run -> run_benchmark_turn -> validation/submission/record/protocol`; broad reports name the parent `prototype1-state` path, broad request publication, intended `run_broad_headless_tui_attempt_with_options -> tui_adapter::run_headless_with_model_capture_responses`, and child-plan rejection.
- Concrete trace chains: baseline report reconstructs edit/validation chains across calls 23-33, 44-49, and call 50's validation-scope mismatch. Broad reports correctly state that no model/tool trace exists and instead provide lifecycle chains from `transition-journal` and `runner-request` through request/workspace publication to missing result diagnostics and child-plan rejection.
- Mechanical completion versus usefulness: all reports keep closure/protocol completion separate from benchmark usefulness and broad child admission.
- Suspicious result verification: child reports verify expected-output edits, final patch content, false-negative inventory rows, absent result directories, and workspace cleanliness against persisted artifacts instead of trusting summaries.
- Action items: each action item is tied to an artifact gap, lifecycle gap, or protocol blind spot; none require speculative provider/model diagnoses.

The incomplete-state reports are valid evidence, but they are not durable successful run reviews. They should be cited only for request-only broad-harness coverage and missing-join facts.

## Independent verification performed in this fan-in

Trace audit was re-run from `/home/brasides/code/ploke`:

```text
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py \
  /home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0 \
  --markdown
```

Observed summary:

```text
responses: 53
provider-emitted tool calls: 52
recorded tool calls: 52
missing recorded provider call ids: 0
extra recorded call ids: 0
finish reasons: {'tool_calls': 52, 'stop': 1}
recorded classifications: {'transport_failure': 8, 'completed': 18, 'read_with_content': 20, 'duplicate_request': 6}
```

I also verified suspicious and important surfaces directly against persisted artifacts:

```text
request_json_count = 10
edit_harness_result_dir_exists = false
child_plan_children_count = 0
child_plan_rejected_count = 10
node_status = failed
runner_request_exists = true
runner_request.runner_args = loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260606-090815
protocol_counts = {tool_call_intent_segmentation: 1, tool_call_review: 52, tool_call_segment_review: 7}
validation_audit.expected_output_edit_candidates = 1
submission patch contains `let expected = "1:x\n";` = true
submission patch contains prior `let expected = "1:x\n2-b\n";` = false
submission patch contains over-indented `        pub fn replace_all` = true
record.json.gz outcome = ToolCalls(count=52)
record.json.gz final_assistant_message = None
```

This verification preserves the important distinctions: the baseline trace is joinable, the broad slots have no result/diagnostic surface, and the exported baseline patch is a candidate requiring semantic review rather than a proven benchmark win.

## Exact execution paths

### Baseline eval/protocol path

```text
closure-state.json
-> instance run root run-1780762798969-structured-current-policy-b8dc71f0
-> execution-log.json run_arm: command="run single agent", execution="agent-single-turn"
-> crates/ploke-eval/src/runner/msb_single.rs::RunMsbSingleRequest::run
-> run_benchmark_turn
-> validation-audit.json + benchmark-patch-projection.json + multi-swe-bench-submission.jsonl
-> record.json.gz + llm-full-responses.jsonl
-> protocol tool-call-review / segment-review projection
```

This path proves a mechanically complete baseline eval/protocol run. It does not prove a clean benchmark fix because MBE/oracle evidence is absent and validation/patch review found suspicious expected-output and patch-hygiene signals.

### Prototype 1 parent/broad-harness path

```text
prototype1 transition-journal parent_started
-> nodes/node-0cdf3741b09283fe/runner-request.json runner_args: loop prototype1-state --repo-root ...090815
-> broad edit-harness request JSON/MD publication for 10 slots
-> candidate workspace materialization for 10 slots at target commit 2bcf9ead0e2ef53db740471c40056ec76b8926cf
-> intended broad headless path should write submitted result or .headless-tui.json diagnostics
-> prototype1/messages/edit-harness-result/ absent
-> child-plan children=[] and 10 rejected_surface_attempts
-> node.json status failed
```

This path proves request publication, workspace materialization, and negative admission accounting. It does not prove timeout, provider failure, model execution, validation failure, or bad patch quality for broad slots because the corresponding traces and diagnostics do not exist.

## Concrete trace chains

### Baseline: edit/validation recovery and expected-output concern

```text
call 23 apply_code_edit(crate::util::Replacer::replace_all)
-> applied util.rs edit
call 24 cargo check --package grep-printer
-> ToolCompleted but result.ok=false / compile_failed
call 25 apply_code_edit same target
-> content changed / refresh required
call 26 code_item_lookup replace_all
-> stale snippet read failure: Content changed
call 27 read_file util.rs
-> direct fresh content
calls 28-33 non_semantic_patch + cargo
-> package check/test green
call 44 insert_rust_item crate::standard::tests
-> fails: no inline module container
call 46 non_semantic_patch standard.rs
-> adds test with expected `1:x\n2-b\n`
call 47 cargo test --package grep-printer
-> ToolCompleted but result.ok=false / test failed
call 48 non_semantic_patch standard.rs
-> changes expected output to `1:x\n`
call 49 cargo test --package grep-printer
-> package tests pass
```

This is both a useful recovery trace and a benchmark-usefulness warning. The final submission contains the adjusted expectation and lacks oracle/MBE proof that the adjustment is semantically correct for the benchmark.

### Broad slots: request-only lifecycle

```text
transition-journal parent_started for node-0cdf3741b09283fe
-> runner-request records prototype1-state parent command
-> request JSON/MD files exist for base slot and r2-r10
-> candidate workspaces exist and are clean at 2bcf9ead
-> edit-harness-result directory absent
-> child-plan records children=[] and ten missing-diagnostics rejections
-> node projection says failed
```

This is the highest available trace for the broad side. It is not a model/tool trace; the trace never reached a persisted model-visible or terminal diagnostic surface.

## Closure/protocol completion

Closure/protocol completion is real but narrow:

- `closure-state.json` reports `eval.status = complete` and `protocol.status = complete` for one instance/run.
- Protocol root has 1 intent segmentation, 52 tool-call reviews, and 7 segment reviews.
- The baseline trace audit joins provider and record ledgers with no missing provider call ids.
- Broad child admission has only negative completion: child-plan rejection and parent failure projection.

Do not summarize the campaign as simply complete. The accurate phrase is: baseline eval/protocol complete; broad child admission failed closed with no trace-bearing broad attempts.

## Broad-harness request-only failures

All 10 broad slots are request-only:

- `node-0cdf3741b09283fe`
- `node-0cdf3741b09283fe-r2`
- `node-0cdf3741b09283fe-r3`
- `node-0cdf3741b09283fe-r4`
- `node-0cdf3741b09283fe-r5`
- `node-0cdf3741b09283fe-r6`
- `node-0cdf3741b09283fe-r7`
- `node-0cdf3741b09283fe-r8`
- `node-0cdf3741b09283fe-r9`
- `node-0cdf3741b09283fe-r10`

Common observed state:

- request JSON and prompt MD present;
- candidate workspace present and clean at the parent artifact commit;
- submitted result absent;
- `.headless-tui.json` diagnostic absent;
- per-slot model/tool trace absent;
- no candidate diff or validation output;
- child-plan rejection row present.

This should be treated as an observability/lifecycle failure in the broad-headless slot runner: the pipeline can publish requests and reject missing results, but it cannot explain why results/diagnostics were not written.

## Actual timeouts

No actual broad-slot timeout is proven for the 090815 campaign. None of the inspected broad slots has a headless trace, terminal log, process exit record, timeout marker, stderr/stdout locator, or submitted result. Reports and bug docs should not call the 10 broad slots timeouts.

The only timeout-related action item is prospective: when diagnostics are repaired, fallback records should include timeout status if a timeout happens.

## Stale metadata and inventory/projection gaps

Confirmed observability gaps:

1. `prototype1/run-profile.toml` has `name = p1-admissionfix-g35flash-p25flash-20260606-053302` inside the `...090815` campaign. Treat this as copied-profile provenance/stale metadata, not campaign authority.
2. `prototype1/run-profile.commitment.json` exists but was not represented in the handoff inventory.
3. `INVENTORY.json` reported the authoritative node's `runner_request` as null even though `prototype1/nodes/node-0cdf3741b09283fe/runner-request.json` exists and records the parent `prototype1-state` command.
4. `prototype1/scheduler.json` still projects the parent as planned/frontier while `nodes/node-0cdf3741b09283fe/node.json` says `failed`.
5. `record.json.gz` lacks the final assistant message even though `llm-full-responses.jsonl` has a final `finish_reason=stop` response; playback requires a manual join.

These are not benchmark failures by themselves, but they are protocol/read-side and operator-observability gaps that can mislead future fan-in unless labeled.

## Bug docs updated or created

Bug synthesis created or updated the appropriate active bug docs:

- Created `docs/active/bugs/2026-06-06-prototype1-broad-headless-request-only-no-diagnostics.md` for the distinct all-slots request-only/no launch-or-fallback diagnostics gap.
- Updated `docs/active/bugs/2026-06-06-code-item-lookup-impl-relation-missing-name-field.md` with the 090815 `impl.name` relation recurrence at call 11.
- Updated `docs/active/bugs/2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md` with the 090815 stale same-file edit/read-side recurrence at calls 23-27.
- Updated `docs/active/bugs/2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md` with completed-but-failed cargo calls, weak changed-file coverage, expected-output edit evidence, patch hygiene, and missing final-assistant capture.

Bug synthesis deliberately did not file broad-slot timeout, provider/auth/quota, model-quality, or runtime-OOM bugs for this campaign because the required evidence is absent.

## Next rerun / repair gate

Do not advance this 090815 campaign as a benchmark-success candidate. The next gate is repair first, rerun second:

1. Repair broad-headless observability so every published broad slot persists either a submitted result, a terminal `.headless-tui.json` diagnostic, or a minimal launch/fallback/no-start record joined to the child-plan rejection.
2. Include request id/hash, slot id, workspace path, command/entrypoint, start/end timestamps, PID/task id if available, exit/timeout status, and redacted provider route in that lifecycle record.
3. Teach child-plan rejection rows to include the launch/fallback record path/status plus submitted-result and diagnostic existence checks.
4. Repair inventory/read-side projections: include node `runner-request.json`, include `run-profile.commitment.json`, and label scheduler-vs-node disagreement.
5. Improve protocol/read-side scoring so cargo `ToolCompleted` is not counted as semantic success without `result.ok`, `status_reason`, manifest path, and changed-file coverage; surface expected-output edit candidates in protocol context.
6. After those repairs, run a fresh campaign or replay fixture to prove broad slots produce typed outcomes. Only then decide whether to re-run benchmark/admission evaluation.

## Bottom line

The board produced a coherent final picture. The baseline plane is a useful trace-bearing fixture for protocol/tool-review defects, but it is not an oracle-confirmed benchmark win. The broad-harness plane is incomplete-state evidence: all ten slots are request-only, child admission failed closed, and the missing broad-headless launch/fallback diagnostics are the blocker for any meaningful rerun or repair decision.
