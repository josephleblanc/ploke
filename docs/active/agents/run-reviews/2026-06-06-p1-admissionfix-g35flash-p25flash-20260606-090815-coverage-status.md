# Coverage/status: p1-admissionfix-g35flash-p25flash-20260606-090815

Status: evidence-backed coverage report for the latest loop run inventory. This is a coverage/status review, not a claim that the Prototype 1 campaign produced an admissible successor.

## Short verdict

`p1-admissionfix-g35flash-p25flash-20260606-090815` is the latest campaign in scope and its closure/protocol projection is mechanically complete for `BurntSushi__ripgrep-2209`, but the Prototype 1 broad-harness side did not admit any child: all 10 request slots are request-only and the child-plan rejected every broad slot because no submitted result or headless diagnostic file exists.

The handoff `INVENTORY.json` is directionally useful and covers the campaign root, closure, scheduler, journal, protocol run, child-plan, 10 JSON request slots, and candidate workspaces. It is not fully authoritative as written: it records the authoritative node's `runner_request` as `null` even though `prototype1/nodes/node-0cdf3741b09283fe/runner-request.json` exists; it also omits `run-profile.commitment.json` and does not surface the stale scheduler-vs-node status mismatch as a labeled gap.

The previous substantive campaign `p1-admissionfix-g35flash-p25flash-20260606-053302` should remain context only for this card. It should not be pulled into the evidence scope except as a provenance lead for the stale `prototype1/run-profile.toml` name mismatch.

## Evidence roots checked

- Worker contract: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/WORKER_CONTRACT.md`
- Inventory under review: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/INVENTORY.json`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815`
- Campaign files: `campaign.json`, `closure-state.json`, `prototype1/scheduler.json`, `prototype1/transition-journal.jsonl`, `prototype1/run-profile.toml`, `prototype1/run-profile.commitment.json`
- Authoritative node: `prototype1/nodes/node-0cdf3741b09283fe/`
- Parent broad-harness requests: `prototype1/messages/edit-harness-request/*.json`
- Parent child-plan: `prototype1/messages/child-plan/node-0cdf3741b09283fe.json`
- Broad candidate workspaces: `prototype1/workspaces/edit-harness/node-0cdf3741b09283fe{,-r2..-r10}/`
- Baseline eval run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0`
- Protocol run root: `/home/brasides/.ploke-eval/protocol/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0`
- Runtime-playback inventory docs read before missing-record claims:
  - `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/README.md`
  - `record-surface-map.md`
  - `record-persistence-checklist.md`
  - `latest-run-emission-worksheet.md`

## Exact execution paths proven

### Baseline eval/protocol path

Evidence path:

```text
closure-state.json
-> instance run root run-1780762798969-structured-current-policy-b8dc71f0
-> execution-log.json run_arm: agent-single-turn
-> record.json.gz + agent-turn-trace.json + llm-full-responses.jsonl
-> validation-audit.json + benchmark-patch-projection.json + multi-swe-bench-submission.jsonl
-> protocol root tool-call-intent-segmentation/tool-call-review/tool-call-segment-review artifacts
```

The run root's `execution-log.json` records `run_arm.command = "run single agent"`, `execution = "agent-single-turn"`, selected model `google/gemini-3.5-flash`, and the lifecycle steps through `benchmark_turn_completed`, `write_validation_audit`, `write_msb_submission`, and `write_benchmark_patch_projection`. The trace audit over that run root reported 53 provider responses, 52 provider-emitted tool calls, 52 recorded tool calls, and no missing recorded provider call IDs.

### Prototype 1 parent/broad-harness path

Evidence path:

```text
prototype1/transition-journal.jsonl parent_started
-> node runner-request command: loop prototype1-state --repo-root <campaign worktree>
-> prototype1/messages/edit-harness-request/*.json publishes 10 broad slots
-> prototype1/messages/edit-harness-result/ is absent
-> messages/child-plan/node-0cdf3741b09283fe.json rejects all 10 surface attempts
```

`runner-request.json` names the state-machine command:

```text
loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260606-090815
```

The broad-harness producer recorded in the child-plan is `prototype1:broad-headless-tui-adapter-v1`. Each rejected attempt points to an expected `.headless-tui.json` path under `prototype1/messages/edit-harness-result/`; that directory is absent in this campaign.

## Closure state versus child-plan state

Closure is mechanically complete for the baseline eval/protocol run:

- `closure-state.json` says registry complete: 1 expected, 1 mapped.
- `eval.status = complete`: 1 expected, 1 complete.
- `protocol.status = complete`: 1 expected, 1 full.
- Protocol artifacts in the protocol root are present: 1 tool-call intent segmentation, 52 tool-call reviews, and 7 tool-call segment reviews.

That does not mean Prototype 1 admitted a useful child or advanced the frontier:

- Scheduler projection still says the sole node is planned/frontier, with no completed or failed node IDs.
- The authoritative `nodes/node-0cdf3741b09283fe/node.json` says `status = failed`.
- The child-plan has `children = []` and 10 rejected surface attempts.
- There are 10 edit-harness request JSON files and 10 matching prompt markdown files, but no edit-harness result directory, no submitted results, and no headless TUI traces.
- All 10 broad workspaces exist and are clean git checkouts at `2bcf9ead`, but they do not contain admitted candidate results.

So the correct reconciliation is: baseline/protocol completed; broad child admission failed closed due missing submitted result/diagnostic artifacts; the campaign did not produce an admitted Prototype 1 child or successor.

## Inventory coverage check

| Surface | Inventory coverage | Verified state | Label |
| --- | --- | --- | --- |
| Campaign root / id | Present | matches latest campaign root and worker contract | operator/convenience record |
| Closure | Present | eval complete, protocol complete for one instance | operator/convenience record; manual join needed to run root |
| Scheduler | Present | scheduler says node planned/frontier | record present, manual join needed; stale versus node status |
| Transition journal | Present | 2 entries: `parent_started`, `resource` | present |
| Run profile | Present | `name = p1-admissionfix-g35flash-p25flash-20260606-053302` while campaign id is `...090815` | present; stale metadata/observability gap |
| Run profile commitment | Not represented in inventory | `prototype1/run-profile.commitment.json` exists | record present, playback gap / inventory gap |
| Authoritative node dir | Present | one node dir exists | present |
| Node `node.json` | Present | status `failed` | present |
| Node `runner-request.json` | Inventory says `runner_request: null` | file exists and names `loop prototype1-state --repo-root ...` | record present, manual join needed; inventory false negative |
| Node `runner-result.json` | Inventory says null | file absent | record absent |
| Node `results/*.json` | Present as empty list | no results files | record absent |
| Node `invocations/*.json` | Present as empty list | no invocation files | record absent |
| Runtime channels | Not surfaced for this node | no channel dirs observed | not applicable / record absent for this failed parent path |
| Child plan | Present | one child-plan, zero children, 10 rejected attempts | present |
| Edit-harness request slots | Present | 10 JSON requests plus 10 prompt markdown files | present; request markdowns are operator/convenience records |
| Edit-harness submitted results | Present as `submitted: null` for each slot | result directory absent | record absent |
| Headless TUI traces | Present as expected paths only | all 10 expected `.headless-tui.json` paths absent | record absent |
| Candidate workspaces | Present | 10 clean git workspaces at `2bcf9ead` | record present, manual join needed |
| History blocks | Not included in inventory | `prototype1/history` absent although request evidence roots point there | record absent |
| Evaluations under campaign prototype1 | Not included in inventory | `prototype1/evaluations` absent | not applicable for no admitted child; record absent if expected by request digest |
| Baseline run root | Present through closure | run artifacts exist: `record.json.gz`, `agent-turn-*`, `llm-full-responses.jsonl`, validation, patch projection, submission | record present, manual join needed |
| Protocol root | Present | 52 call reviews, 7 segment reviews, 1 segmentation | record present, playback gap |

## Concrete trace chains

### Baseline eval trace chain with suspicious test expectation change

Trace audit and raw provider responses show a coherent but benchmark-questionable chain:

```text
llm-full-responses response 44
  -> model tries to insert `replacement_multi_line_look_around` expecting `1:xb\n`
  -> insert_rust_item fails
response 46
  -> model uses non_semantic_patch to add the test expecting `1:x\n2-b\n`
response 47
  -> cargo test fails
response 48
  -> model changes only the test expected output to `1:x\n`
validation-audit event 433
  -> flags this as an expected-output/assertion edit candidate
multi-swe-bench-submission.jsonl
  -> persisted patch contains `let expected = "1:x\n";`
final cargo/check artifacts
  -> cargo checks pass mechanically, but validation audit warns no fmt evidence and the test expected-output edit needs inspection
```

Suspicious-result verification: I did not trust the `ok`/final response summaries alone. I checked the persisted `validation-audit.json` red flag and then verified the exported benchmark patch in `multi-swe-bench-submission.jsonl`: the patch changes `crates/printer/src/standard.rs` to add `replacement_multi_line_look_around` with `let expected = "1:x\n";`. The same patch also changes `crates/printer/src/util.rs` and removes the doc-comment block before `replace_all` while adding an over-indented `pub fn replace_all<'a>` line. This makes the baseline output mechanically complete but still requiring semantic review before treating it as a benchmark-useful fix.

### Prototype 1 child-admission trace chain

```text
transition-journal line 1: parent_started for node-0cdf3741b09283fe
  -> node runner-request records `loop prototype1-state --repo-root ...090815`
  -> 10 edit-harness request JSON files are published for node-0cdf3741b09283fe and r2-r10
  -> no `prototype1/messages/edit-harness-result/` directory exists
  -> child-plan records 10 rejected_surface_attempts and zero children
```

This is not a model/tool trace for the broad slots because the headless TUI traces are absent. It is the highest available lifecycle trace chain for the broad-harness side.

## Mechanical completion versus benchmark usefulness

Mechanical completion:

- The baseline eval run root is complete enough for trace/protocol review: full provider response sidecar, compressed record, agent-turn trace/summary, validation audit, benchmark patch projection, and MBE submission artifact all exist.
- Protocol completed over 52 recorded tool calls and wrote all expected protocol families.
- The trace audit found no provider-call-to-recorded-lifecycle mismatch.

Benchmark usefulness:

- MBE is disabled in the run profile (`[execution.mbe] enabled = false`), so there is no oracle/MBE verdict proving benchmark improvement.
- `validation-audit.json` warns that no formatting check evidence was recorded and that a test expected-output/assertion edit needs inspection.
- The final assistant response claims `cargo test --package grep-printer` all 95 tests passed, but the validation audit also records a later focused `cargo test` against `crates/globset/Cargo.toml` with `covers_changed_files = false`; review should prefer the audit table over the prose summary and not overclaim workspace-wide validation.
- The Prototype 1 loop produced no admitted child, no successor-ready record, and no successor-completion record.

## Previous campaign scope

`p1-admissionfix-g35flash-p25flash-20260606-053302` exists and is the source-looking value embedded in `prototype1/run-profile.toml` for this latest campaign. It is not evidence for the 090815 campaign's closure, protocol, child-plan, slots, or workspaces.

Use 053302 only as context for diagnosing how run-profile names are copied or templated. Do not pull its closure/run roots into this card's coverage scope unless a follow-up specifically investigates stale run-profile provenance.

## What is working

- The inventory correctly identifies the latest campaign id/root and previous-substantive-campaign context.
- It finds the closure run root, protocol root, protocol counts, one authoritative node, one child-plan, ten request-only broad slots, and ten candidate workspaces.
- The run-review audit script can reconcile provider-emitted versus recorded tool calls for the baseline run: 52 versus 52, no missing call IDs.
- Child admission failed closed instead of inventing children from request-only slots.

## What is not working yet

- Inventory has a concrete false negative: `nodes[0].runner_request` is null despite an existing `runner-request.json`.
- Scheduler projection remains stale (`planned/frontier`) while `node.json` says `failed`; the inventory reports both but does not label the authority conflict.
- No broad slot has a submitted result, headless trace, terminal record, invocation record, runner result, or diagnostics artifact. That makes the broad slots reviewable only as incomplete-state records.
- Closure/protocol completion can be misread as campaign success unless the report explicitly separates baseline eval/protocol completion from broad child admission.
- Run-profile metadata is stale: the profile `name` is `...053302` inside the `...090815` campaign.
- Protocol artifacts are present but remain a playback gap: they summarize/review the run but do not replace the underlying agent-turn/tool timeline.

## Action items

1. Fix the inventory generator to include existing node-level `runner-request.json` and to fail/warn when inventory says null for a file that exists. This is an observed artifact gap: `INVENTORY.json` versus `prototype1/nodes/node-0cdf3741b09283fe/runner-request.json`.
2. Add `prototype1/run-profile.commitment.json` to the inventory schema. This is a record-present inventory/playback gap for run policy provenance.
3. Label scheduler-vs-node disagreements explicitly. This is a lifecycle gap: `scheduler.json` says planned/frontier while `node.json` says failed.
4. Persist per-slot failure diagnostics for broad headless TUI attempts even when no submitted result is produced. This is the key lifecycle gap behind the 10 request-only slots: the child-plan can say "no submitted result or diagnostics," but reviewers cannot inspect a terminal/headless trace.
5. Keep closure/protocol status separate from child admission in dashboard/fan-in summaries. This is a protocol blind spot: protocol completed over the baseline run while Prototype 1 rejected all broad children.
6. Treat `prototype1/run-profile.toml`'s 053302 name inside the 090815 campaign as an observability bug/lead, not as scope authority. The campaign id, root paths, closure state, and request paths are the authority for this card.
7. For benchmark usefulness, require semantic review of the exported patch before calling the baseline fix useful: validation already flags an expected-output edit and no fmt evidence.
