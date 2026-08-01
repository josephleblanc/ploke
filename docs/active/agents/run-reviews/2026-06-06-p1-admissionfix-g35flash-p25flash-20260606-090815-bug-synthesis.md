# Bug synthesis: 090815 tool/protocol/slot failures

Campaign: `p1-admissionfix-g35flash-p25flash-20260606-090815`
Task: RCA fan-in for tool-call/protocol/slot failures
Status: evidence-backed synthesis; broad-slot half remains incomplete-state because no per-slot headless traces exist

## Short verdict

The child reports agree on two separate planes:

1. The baseline eval/protocol run for `BurntSushi__ripgrep-2209` is mechanically complete and trace-bearing: the run root has provider responses, record/protocol artifacts, validation audit, patch projection, and submission. The trace audit reconciles 52 provider tool calls with 52 recorded tool calls and zero missing call ids.
2. The Prototype 1 broad-harness side is not trace-bearing and not benchmark-useful: the parent published ten edit-harness requests and ten clean candidate workspaces, but produced zero submitted edit results and zero `.headless-tui.json` diagnostics. The child-plan correctly rejected all ten slots, but the artifacts do not explain why no headless result/diagnostic was written.

Confirmed bug-doc actions from this synthesis:

- Created `docs/active/bugs/2026-06-06-prototype1-broad-headless-request-only-no-diagnostics.md` for the distinct all-slots request-only / no launch-or-fallback diagnostic gap.
- Updated `docs/active/bugs/2026-06-06-code-item-lookup-impl-relation-missing-name-field.md` with the 090815 recurrence at call 11.
- Updated `docs/active/bugs/2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md` with the 090815 same-file stale refresh/read-side recurrence at calls 23-27.
- Updated `docs/active/bugs/2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md` with 090815 evidence for completed-but-failed cargo calls, weak changed-file coverage on call 50, expected-output edit detection, patch hygiene, and missing final-assistant capture.

I did not file a broad-slot timeout/provider bug because the inventory and child reports found no timeout, provider, process, stderr/stdout, or `.headless-tui.json` evidence for the ten broad slots.

## Evidence roots

Required method inputs read:

- Worker contract: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/WORKER_CONTRACT.md`
- Inventory: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/INVENTORY.json`
- Project run-review procedure: `docs/workflow/skills/ploke-run-review/SKILL.md`
- EvalOps procedure: `docs/workflow/skills/prototype1-evalops/SKILL.md`

Child reports read:

- `docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-coverage-status.md`
- `docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-baseline-protocol-tool-correlation.md`
- `docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-parent-request-only-rca.md`
- `docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-slots-base-r5-incomplete.md`
- `docs/active/agents/run-reviews/2026-06-06-p1-admissionfix-g35flash-p25flash-20260606-090815-slots-r6-r10-incomplete.md`

Primary artifact roots:

- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815`
- Baseline run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0`
- Protocol root: `/home/brasides/.ploke-eval/protocol/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0`
- Child-plan: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/child-plan/node-0cdf3741b09283fe.json`
- Broad requests: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/edit-harness-request/`
- Expected broad results: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/edit-harness-result/`
- Broad workspaces: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/workspaces/edit-harness/`

## Exact execution paths

### Baseline eval/protocol path

The trace-bearing baseline run is the single-agent benchmark path, not a broad headless-TUI slot:

```text
closure-state.json
-> run root run-1780762798969-structured-current-policy-b8dc71f0
-> execution-log.json run_arm: command="run single agent", execution="agent-single-turn"
-> crates/ploke-eval/src/runner/msb_single.rs::RunMsbSingleRequest::run
-> run_benchmark_turn
-> validation-audit.json + benchmark-patch-projection.json + multi-swe-bench-submission.jsonl
-> record.json.gz + llm-full-responses.jsonl
-> protocol tool-call review / segment review projection
```

The bundled trace audit was re-run during synthesis:

```text
python3 docs/workflow/skills/ploke-run-review/scripts/run_trace_audit.py \
  /home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0 \
  --markdown
```

Result:

```text
responses: 53
provider-emitted tool calls: 52
recorded tool calls: 52
missing recorded provider call ids: 0
extra recorded call ids: 0
finish reasons: {'tool_calls': 52, 'stop': 1}
recorded classifications: {'transport_failure': 8, 'completed': 18, 'read_with_content': 20, 'duplicate_request': 6}
```

### Prototype 1 parent / broad-harness path

The broad-slot path is separate:

```text
prototype1 transition-journal parent_started
-> nodes/node-0cdf3741b09283fe/runner-request.json: loop prototype1-state --repo-root ...090815
-> cli_facing.rs broad request publication and workspace materialization
-> intended broad headless path: run_broad_headless_tui_attempt_with_options
-> tui_adapter::run_headless_with_model_capture_responses
-> expected submitted result or edit-harness-result/<slot>.headless-tui.json
-> child-plan admission/rejection
```

Source inspection shows normal broad diagnostics are written only after `run_headless_with_model_capture_responses` returns a run with a terminal outcome:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1610-1630
run_headless_with_model_capture_responses(...).await?
-> terminal = run.terminal().ok_or(...)?
-> write_broad_headless_tui_diagnostics(slot, &run)?
-> write_broad_headless_tui_turn_live_bundle(slot, &run, ...)?
```

The child-plan rejection path only reports missing diagnostics when the expected file is absent:

```text
crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2412-2422
slot_rejection(...): "produced no submitted result or diagnostics at <path>"
```

## Mechanical completion versus benchmark usefulness

Mechanical completion:

- Baseline closure/protocol completed for one instance/run.
- Protocol artifacts cover 52/52 tool calls and 7/7 segments.
- The run root has record, full provider responses, validation audit, patch projection, and submission artifacts.
- Broad child admission also has negative accounting: child-plan `children=[]` and ten rejected surface attempts.

Benchmark usefulness:

- The broad Prototype 1 parent admitted no child and produced no successor. The broad slots have no patch, no submitted result, no model/tool trace, no validation output, and no oracle/MBE evidence.
- The baseline run produced an exported patch and package-level validation, but it is not an oracle-confirmed benchmark win. The validation audit flags an expected-output edit; direct patch verification confirms the final submission contains `let expected = "1:x\n";` and not the earlier `"1:x\n2-b\n"` expectation, and contains an over-indented `        pub fn replace_all` line.
- Therefore the highest verified gate is: baseline `patch exported + package validation + complete protocol`, with semantic review still required; broad harness `request-only failure with missing diagnostics`.

## Concrete trace chains

### Chain A: broad-slot request-only lifecycle

```text
transition-journal parent_started for node-0cdf3741b09283fe
-> runner-request records `loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260606-090815`
-> ten request JSON/Markdown pairs published under messages/edit-harness-request
-> ten candidate workspaces created at target commit 2bcf9ead0e2ef53db740471c40056ec76b8926cf
-> messages/edit-harness-result/ is absent
-> child-plan records children=[] and ten rejected_surface_attempts for missing `.headless-tui.json` paths
-> node.json status is failed
```

Synthesis verification against persisted artifacts:

```text
request_json_count = 10
edit_harness_result_dir_exists = false
child_plan_children = 0
child_plan_rejected = 10
node_status = failed
runner_request_exists = true
```

This proves request/workspace publication and negative admission accounting. It does not prove any model/tool execution, timeout, provider failure, or candidate patch behavior for the broad slots.

### Chain B: baseline tool/protocol failures and recovery

```text
call 23 apply_code_edit(crate::util::Replacer::replace_all)
-> applied one edit to crates/printer/src/util.rs
call 24 cargo check --package grep-printer
-> ToolCompleted but semantic result ok=false / compile_failed
call 25 apply_code_edit same semantic target
-> fails: content changed; refresh/re-resolve required
call 26 code_item_lookup replace_all
-> fails internally while reading stale snippet: Content changed
call 27 read_file util.rs
-> model gets fresh content through a direct read
calls 28-33 non_semantic_patch + cargo
-> compile/test recovery reaches package-green state
```

This is a positive recovery chain, but it also confirms two active gaps: same-file semantic edit/lookup paths need a typed stale-refresh boundary, and protocol/read-side consumers must not treat `ToolCompleted` as semantic success without inspecting `result.ok` and diagnostics.

### Chain C: suspicious test-output edit

```text
call 44 insert_rust_item crate::standard::tests
-> fails: no inline module container
call 46 non_semantic_patch standard.rs
-> adds replacement_multi_line_look_around with expected `1:x\n2-b\n`
call 47 cargo test --package grep-printer
-> ToolCompleted but ok=false; new test fails
call 48 non_semantic_patch standard.rs
-> changes expected output to `1:x\n`
call 49 cargo test --package grep-printer
-> package tests pass
```

Suspicious-result verification performed during synthesis:

- `validation-audit.json` contains `patch_quality.expected_output_edit_candidates` for call `function-call-ce5d18bc-f244-434d-81d0-9da71278b162`, showing `- let expected = "1:x\n2-b\n";` and `+ let expected = "1:x\n";`.
- `multi-swe-bench-submission.jsonl` confirms the exported patch contains `let expected = "1:x\n";`, does not contain `let expected = "1:x\n2-b\n";`, and contains the over-indented `        pub fn replace_all` line.

This does not prove the expectation is semantically wrong, but it downgrades the signal from “benchmark success” to “candidate patch requiring semantic review.”

### Chain D: protocol over-credit / weak validation surface

```text
call 50 cargo {"command":"test"}
-> ToolCompleted ok=true
-> validation-audit cargo_calls[7].manifest_path = .../crates/globset/Cargo.toml
-> validation-audit cargo_calls[7].covers_changed_files = false
```

Protocol/segment summaries should not treat this as broad coverage over the changed `crates/printer` files. The run does have other cargo calls that cover changed files, but call 50 specifically is not workspace-wide regression evidence.

## RCA fan-in

### Confirmed root causes / gaps

1. Broad-slot launch/fallback diagnostics are missing for pre-terminal failures.
   - Observed artifact gap: ten requests and ten workspaces exist, but the result directory and all `.headless-tui.json` diagnostics are absent.
   - Source gap: normal diagnostics are written only after a terminal run is returned.
   - Bug doc: `docs/active/bugs/2026-06-06-prototype1-broad-headless-request-only-no-diagnostics.md`.

2. `code_item_lookup` impl relation queries assume an unavailable `name` field.
   - Observed tool failure: call 11 `code_item_lookup` with `node_kind=impl` failed with `stored relation 'impl' does not have field 'name'`.
   - Matching existing bug updated: `docs/active/bugs/2026-06-06-code-item-lookup-impl-relation-missing-name-field.md`.

3. Same-file semantic edit/read-side paths can hit stale content after an applied edit.
   - Observed tool failures: calls 25 and 26 after call 23 applied a util.rs edit.
   - Matching existing bug updated: `docs/active/bugs/2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md`.

4. Protocol/read-side cargo validation needs semantic fields, not lifecycle-only completion.
   - Observed protocol blind spots: calls 24/29/47 are completed-but-failed cargo calls; call 50 is unrelated focused manifest coverage; expected-output edit is flagged by validation audit but can still be over-credited; final assistant response requires a manual full-response join.
   - Matching existing bug updated: `docs/active/bugs/2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md`.

5. Inventory/projection gaps can mislead reviewers.
   - Inventory says parent `runner_request: null`, while `nodes/node-0cdf3741b09283fe/runner-request.json` exists.
   - Scheduler still reports the parent as planned/frontier while node.json says failed.
   - Run profile name/source points to earlier `...053302` provenance inside the `...090815` campaign.
   - These were recorded in the synthesis action items below; I did not create a separate bug doc here because the assigned scope was tool/protocol/slot failures and the child reports present these as observability/inventory follow-ups rather than a single confirmed tool failure.

### Not proven / not filed

- Broad-slot timeout: not proven; no headless trace or process log exists.
- Broad-slot provider/auth/quota failure: not proven; no provider response or `.headless-tui.json` exists for the slots.
- Broad-slot model-quality failure: not proven; no model/tool trace exists for any slot.
- Runtime actor/OOM recurrence in 090815: not proven; no OOM/process/partial sidecar evidence exists for this campaign.
- Baseline patch benchmark win: not proven; no oracle/MBE artifact and patch still needs semantic review.

## Active bug report updates

### Created

`docs/active/bugs/2026-06-06-prototype1-broad-headless-request-only-no-diagnostics.md`

Covers the distinct lifecycle contract: every published broad slot needs submitted result, terminal diagnostic, or minimal launch/fallback/no-start record joined to child-plan rejection. This is adjacent to but not covered by older zero-admission, provider-401, or runtime-actor-leak docs.

### Updated

`docs/active/bugs/2026-06-06-code-item-lookup-impl-relation-missing-name-field.md`

Added the 090815 recurrence for call 11.

`docs/active/bugs/2026-05-25-prototype1-same-file-semantic-edit-stale-anchor.md`

Added the 090815 recurrence for calls 23-27.

`docs/active/bugs/2026-05-24-cargo-tool-validation-and-trace-summary-ambiguity.md`

Added the 090815 recurrence for completed-but-failed cargo calls, call 50 changed-file coverage mismatch, expected-output edit verification, over-indented exported patch line, and missing final-assistant capture in summary/record.

## Action items tied to observed gaps

1. Artifact gap: persist a per-slot broad headless launch record before invoking `run_headless_with_model_capture_responses`, keyed by request id/hash, slot id, workspace, command/entrypoint, start time, PID/task id if available, and redacted provider route. Tied to all ten 090815 request-only slots.
2. Lifecycle gap: write a terminal/fallback diagnostic for every pre-normal-diagnostic exit path: workspace prep, prompt read, budget build, adapter error, no-terminal outcome, task join failure, timeout, panic/kill/interruption, or provider/environment blocker. Tied to the absent `messages/edit-harness-result/` directory.
3. Protocol blind spot: include launch/fallback record status/path in `child-plan.rejected_surface_attempts`. Tied to child-plan rows that currently say only “no submitted result or diagnostics.”
4. Tool/schema gap: fix or deliberately reject `code_item_lookup node_kind=impl` so it cannot issue a Cozo query that references `impl.name`. Tied to baseline call 11 and the earlier 053302 recurrence.
5. Tool lifecycle gap: type content-changed semantic edit/lookup results as `stale_file_version` / `refresh_required` instead of generic `io`/`internal` failures, and force refreshed lookup/read boundaries after same-file applied edits. Tied to calls 25 and 26.
6. Protocol/read-side gap: make tool-call review consume cargo `result.ok`, `status_reason`, `manifest_path`, and changed-file coverage. Tied to calls 24, 29, 47, and 50.
7. Protocol/read-side gap: promote validation-audit expected-output edit candidates into protocol context. Tied to call 48 and the exported submission patch.
8. Artifact/playback gap: capture the final `finish_reason=stop` assistant response in `record.json.gz` / `agent-turn-summary.json`, or explicitly label it as a manual join to `llm-full-responses.jsonl`. Tied to response 52 versus `final_assistant_message=null`.
9. Inventory gap: repair handoff inventory so existing node `runner-request.json` is not reported as null, add `run-profile.commitment.json`, and label scheduler/node status disagreement. Tied to the verified 090815 inventory false negative and stale projections.
10. Rerun gate: do not advance this campaign as a benchmark-success candidate; after diagnostic fixes, run a fresh campaign or replay fixture to prove broad slots produce typed outcomes.

## Bottom line

This campaign is useful as a two-plane fixture. The baseline plane is trace-bearing and exposes protocol/tool-review defects that can be fixed against concrete calls. The broad-harness plane is not trace-bearing and must be treated as incomplete-state evidence: it proves the pipeline can publish requests and reject missing results, but it also proves reviewers lack the launch/fallback diagnostics needed to classify why every broad slot produced no result.
