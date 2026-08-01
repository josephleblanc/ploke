# RCA: 090815 parent broad-harness request-only / all slots rejected

Status: incomplete-state RCA for Prototype 1 parent child-planning. This report is evidence-backed, but it is not a trace-bearing run review because the broad headless-TUI traces are absent.

## 1. Short verdict

The 090815 Prototype 1 parent reached broad-harness child planning far enough to publish 10 edit-harness requests and materialize 10 clean candidate workspaces, but it did not persist any submitted edit result, headless-TUI diagnostic, raw-provider sidecar, invocation record, runner result, or per-slot launch/exit record. The parent child-plan correctly failed closed with zero children and 10 rejected surface attempts, each saying the corresponding broad slot produced no submitted result or diagnostics.

The earliest provable missing transition is after request/workspace publication and before the broad headless-TUI attempt wrote its normal terminal diagnostics at `prototype1/messages/edit-harness-result/<slot>.headless-tui.json`. Current artifacts do not distinguish whether the headless worker never launched, failed before `run_headless_with_model_capture_responses` returned, was interrupted, hit a provider/environment error before diagnostics, or was killed before persistence. The RCA root cause is therefore an observability/lifecycle persistence gap in the broad headless-TUI slot runner, not an artifact-backed provider, timeout, or model-quality diagnosis.

Existing bug docs partially cover adjacent behavior, but not this exact failure. `2026-05-25-prototype1-child-plan-zero-admission-timeout.md` covers the controller contract that a below-minimum batch must persist rejected child-plan evidence; this campaign shows that fixed behavior working. `2026-05-25-prototype1-broad-headless-google-401-slot-thrash.md` covers typed provider-unavailable handling when diagnostics exist. `2026-06-04-prototype1-headless-tui-runtime-actor-leak.md` covers runtime actor retention/OOM when attempts produce result sidecars. None cover the all-slots request-only/no-diagnostic gap verified here. A new active bug, or a narrow update under bug synthesis, is required for per-slot launch/fallback diagnostic persistence.

## 2. Evidence roots

- Worker contract: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/WORKER_CONTRACT.md`
- Handoff inventory: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/INVENTORY.json`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815`
- Prototype 1 root: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1`
- Parent node: `prototype1/nodes/node-0cdf3741b09283fe/node.json`
- Parent runner request: `prototype1/nodes/node-0cdf3741b09283fe/runner-request.json`
- Transition journal: `prototype1/transition-journal.jsonl`
- Scheduler projection: `prototype1/scheduler.json`
- Run profile: `prototype1/run-profile.toml`
- Run profile commitment: `prototype1/run-profile.commitment.json`
- Child plan: `prototype1/messages/child-plan/node-0cdf3741b09283fe.json`
- Broad requests: `prototype1/messages/edit-harness-request/*.json` and `*.md`
- Expected result/diagnostic root: `prototype1/messages/edit-harness-result/`
- Broad workspaces: `prototype1/workspaces/edit-harness/node-0cdf3741b09283fe{,-r2..-r10}/`
- Baseline/protocol run root for contrast only: `/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0`
- Code path references:
  - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:1423-1449` resolves broad-TUI options and calls `run_broad_headless_tui_attempt_with_options`.
  - `cli_facing.rs:1550-1639` prepares the workspace, calls `tui_adapter::run_headless_with_model_capture_responses`, then writes diagnostics/turn-live only after a terminal run is returned.
  - `cli_facing.rs:2412-2422` constructs the exact `produced no submitted result or diagnostics` rejection when the expected diagnostic file is missing.
  - `cli_facing.rs:2611-2624` persists a rejected-attempt-only child plan and marks the parent failed when admitted children are below `child_budget.min`.
  - `cli_facing.rs:4464-4652` drives the parallel admission batch and publishes the child-plan from admitted/rejected attempt evidence.

Runtime-playback inventory docs were checked before missing-record claims: `record-surface-map.md`, `record-persistence-checklist.md`, and `latest-run-emission-worksheet.md`.

## 3. Closure and parent state

The campaign has two planes that must not be conflated.

Mechanical baseline/protocol plane:

- `closure-state.json` reports eval complete and protocol complete for `BurntSushi__ripgrep-2209`.
- The protocol root has 52 reviewed tool calls for the baseline eval/protocol run.
- That baseline/protocol completion is separate from broad child admission.

Prototype 1 parent/broad-harness plane:

- `transition-journal.jsonl` has two rows: `parent_started` for PID `2593254`, then a `resource` measurement for node `node-0cdf3741b09283fe` at `phase = parent_start`.
- A live-process check during review found no PID `2593254` and no active `ploke-eval`, `headless-tui`, or `prototype1-state` process for this campaign.
- `runner-request.json` exists and names the parent state-machine command: `loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260606-090815`.
- `node.json` says the parent node status is `failed`.
- `scheduler.json` is stale/manual-join evidence: it still lists the same node as `planned` in `frontier_node_ids`, with empty completed and failed node lists.
- The child-plan has `children = []` and 10 rejected surface attempts.

## 4. Eval and patch output

No broad-harness patch output exists for the parent slots.

Verified counts from the campaign surface:

- edit-harness request JSON files: 10
- edit-harness prompt markdown files: 10
- candidate workspaces: 10
- submitted result JSON files: 0
- `*.headless-tui.json` diagnostics: 0
- `prototype1/messages/edit-harness-result/` directory: absent
- node-level `runner-result.json`: absent
- node-level `results/*.json`: absent
- node-level `invocations/*.json`: absent
- campaign `prototype1/history`: absent
- campaign `prototype1/evaluations`: absent

The baseline eval run produced its own benchmark/protocol artifacts, but those are not broad-slot patch outputs and should not be counted as admitted child evidence.

## 5. Oracle/MBE state

`run-profile.toml` has `[execution.mbe] enabled = false`. There is no oracle/MBE verdict for a broad child because no child was admitted and no broad submitted result exists.

Mechanical completion is therefore: baseline/protocol artifacts exist and the parent child-plan persisted negative accounting. Benchmark usefulness for the Prototype 1 broad-harness parent is zero/undetermined: no admitted child, no child patch, no validation output, no successor-ready record, and no successor-completion record exist.

## 6. LLM and tool behavior

No per-slot LLM/tool behavior can be reconstructed for the 10 broad slots because the relevant records are absent:

- no `.headless-tui.json` summaries;
- no raw provider/`llm_full_response*.log` sidecars joined to slots;
- no `turn-live` bundles;
- no model-visible tool events;
- no proposal lifecycle events;
- no validation command output;
- no per-slot terminal record.

The code path explains why this absence matters. In `run_broad_headless_tui_attempt_with_options`, diagnostics and the turn-live bundle are written only after `run_headless_with_model_capture_responses` returns a run with a terminal outcome. Errors while preparing the workspace, reading the prompt, building the budget, executing the headless run, or returning without a terminal can exit the attempt before `write_broad_headless_tui_diagnostics` and `write_broad_headless_tui_turn_live_bundle` are called. This exact campaign has no fallback launch/exit record to tell which of those pre-diagnostic boundaries failed.

## 7. Positive examples and adjudication candidates

There is no positive model/tool adjudication candidate for these broad slots because no model/tool trace exists.

The positive controller signal is narrower: child admission failed closed. The system did not mint child nodes from request-only surfaces. It persisted a child-plan with zero children and 10 rejected surface attempts, then projected the parent node to `failed`. That is useful lifecycle evidence, but it is not evidence of a model attempt or benchmark-useful patch.

## 8. Trace reconstruction

Concrete lifecycle trace chain:

```text
transition-journal line 1: parent_started, pid 2593254
-> runner-request.json: loop prototype1-state --repo-root ...090815
-> broad request publication: 10 request JSON/MD pairs under messages/edit-harness-request
-> workspace materialization: 10 candidate workspaces at target commit 2bcf9ead0e2ef53db740471c40056ec76b8926cf
-> expected headless result/diagnostic surface: messages/edit-harness-result/<slot>.json or <slot>.headless-tui.json
-> result directory absent: no submitted result and no diagnostic for any slot
-> child-plan: children [] and 10 rejected_surface_attempts with `produced no submitted result or diagnostics`
-> node projection: parent node status failed
```

Suspicious-result verification against persisted artifacts:

1. I did not trust the child-plan rejection string by itself. A filesystem check verified that `prototype1/messages/edit-harness-result/` does not exist, so every expected submitted result and `.headless-tui.json` path named by the child-plan is absent.
2. I did not trust the handoff inventory's node row by itself. The inventory says `runner_request: null`, but the filesystem has `prototype1/nodes/node-0cdf3741b09283fe/runner-request.json`; reading it confirms it is the parent `loop prototype1-state` request, not a per-slot headless trace.
3. I verified workspace materialization with git instead of assuming request publication meant execution. The first and last sampled workspaces are clean git branches at `2bcf9ead0e2ef53db740471c40056ec76b8926cf`, with empty status and empty diffstat. This proves materialization, not model execution.

Last point where enough information existed to act:

- The parent controller had enough information to publish requests and materialize workspaces.
- The broad child agents never left model/tool traces, so there is no evidence that any model saw the prompt, used a tool result, edited a file, or saw validation output.
- The next actionable evidence point should have been a per-slot launch/terminal diagnostic or submitted result. That point is missing.

## 9. Protocol review and blind spots

Protocol completed for the separate baseline eval/protocol run. It did not audit the missing broad-harness slots because those slots have no model/tool records to review.

Blind spots observed here:

- `closure-state.json` and protocol completion can be misread as campaign success unless reports keep baseline eval/protocol separate from parent broad child admission.
- The child-plan rejection preserves the negative admission outcome but not the process-level reason for missing diagnostics.
- `scheduler.json` remains stale relative to `node.json`: scheduler says planned/frontier, node says failed.
- `run-profile.toml` has `name = "p1-admissionfix-g35flash-p25flash-20260606-053302"` inside the `...090815` campaign. `run-profile.commitment.json` confirms `source_path` was the earlier `...053302` campaign's run profile. This is copied-profile provenance/stale metadata, not authority for the current campaign id.
- The handoff inventory missed an existing parent `runner-request.json`, creating a false negative that reviewers must manually correct.

## 10. What is working

- Latest-campaign grounding works: the campaign id/root, closure run root, protocol root, parent node, child-plan, requests, and workspaces are discoverable.
- Broad request publication worked: all 10 request JSON/MD pairs exist.
- Workspace materialization worked: all 10 slot workspaces exist and are clean at the target artifact commit.
- The zero-admission child-plan persistence fix appears active: below-minimum admission produced durable rejected-attempt child-plan evidence instead of leaving no child-plan authority file.
- Child admission failed closed: no children were admitted from request-only slots.

## 11. What is not working yet

- No per-slot launch/exit evidence exists after request/workspace creation.
- No fallback diagnostic exists when the broad headless-TUI attempt fails before writing `.headless-tui.json`.
- No raw provider or turn-live sidecar records whether a provider request was made.
- The child-plan can say that diagnostics are missing, but it cannot explain why they are missing.
- Scheduler, run-profile, and inventory surfaces contain stale or false-negative metadata that can mislead operators unless manually reconciled.
- Existing bug docs do not cover the exact all-slots request-only/no-diagnostic state.

## 12. RCA classification matrix

- Provider/env failure: not proven. Direct-Google routes are configured, and historical 401 bugs exist, but this campaign has no per-slot diagnostic or raw provider sidecar showing 401/403/429 or any provider response.
- Worker spawn failure: plausible but not proven. There is no per-slot PID, argv, launch, exit status, or stderr/stdout locator.
- Headless adapter missing persistence: proven as an observability gap for pre-diagnostic failures. The code writes normal diagnostics only after a terminal run is returned; this campaign has no fallback record before that point.
- Parent ledger behavior: working for negative accounting. The parent wrote a rejected-attempt-only child-plan and failed the node instead of inventing children.
- Stale process: no live process was found; the parent PID from the journal is gone. No artifact proves whether it exited cleanly, crashed, was killed, or was interrupted.
- Stale metadata: proven. Scheduler planned/frontier disagrees with node failed; run-profile name/source points to 053302; inventory missed runner-request.

## 13. Bug-doc coverage decision

Existing docs are adjacent, not sufficient:

1. `2026-05-25-prototype1-child-plan-zero-admission-timeout.md`
   - Covers: below-minimum broad-harness batch must persist child-plan rejected evidence.
   - Current campaign: this behavior works; child-plan exists with 10 rejected attempts.
   - Does not cover: absence of per-slot diagnostics/launch records when every slot is request-only.
2. `2026-05-25-prototype1-broad-headless-google-401-slot-thrash.md`
   - Covers: provider 401 misclassified as no-edit/fresh-slot churn when diagnostics exist.
   - Current campaign: no diagnostic exists, so provider 401 cannot be asserted.
3. `2026-06-04-prototype1-headless-tui-runtime-actor-leak.md`
   - Covers: runtime actor retention/OOM and teardown issues around broad headless attempts.
   - Current campaign: no OOM log or partial result wave was found for this campaign.
4. Slot incomplete-state reports for this same campaign cover base-r5 and r6-r10 request-only facts.
   - They support this RCA, but they are reports, not active bug docs.

Decision: create a new active bug, or have the dependent bug-synthesis card update an existing umbrella if one is chosen, for `broad headless-TUI request-only all slots lack launch/fallback diagnostics`. The bug should be framed as an observability/lifecycle contract failure: every published broad slot must have either a submitted result, a terminal `.headless-tui.json` diagnostic, or a minimal launch/exit/no-start record joined to the child-plan rejection.

## 14. Action items

1. Artifact gap: persist a per-slot launch record before invoking `run_broad_headless_tui_attempt_with_options`, keyed by request id/hash, slot index, workspace path, model route, argv/entrypoint, PID/task id if available, and start timestamp.
2. Lifecycle gap: persist a terminal/fallback record on every pre-diagnostic exit path: workspace preparation failure, prompt read failure, budget construction failure, `run_headless_with_model_capture_responses` error, no-terminal error, task join failure, process interruption, timeout, and panic/kill detection when available.
3. Protocol blind spot: include the launch/fallback record path and status in child-plan `rejected_surface_attempts`, not only the expected missing `.headless-tui.json` path.
4. Artifact gap: write a minimal `.headless-tui.json` summary, or an adjacent typed `no-diagnostic.json`, for provider/environment failures even when the full run summary cannot be constructed. Redact endpoint auth, credentials, provider project ids, and credential paths.
5. Lifecycle gap: reconcile scheduler after rejected child-plan persistence so `scheduler.json` does not keep a failed parent as the sole planned/frontier node without an explicit stale/projection label.
6. Observability gap: fix the inventory generator's false negative for `runner-request.json` and add `run-profile.commitment.json` to the handoff schema.
7. Observability/staleness gap: surface copied run-profile provenance explicitly. The `name` from the 053302 source profile should not be mistaken for 090815 campaign authority.
8. Rerun gate: do not rerun or advance this campaign as a benchmark-success candidate until per-slot diagnostics are added or a fresh campaign proves whether broad headless-TUI attempts actually launch and terminate with typed evidence.

## 15. Follow-up handoff

This report should feed the dependent bug-synthesis card as the parent-level RCA. It should not be cited as proof of provider failure, model timeout, bad patch quality, or validation failure. The verified failure is narrower and stronger: all broad slots are request-only; the parent child-plan rejected them correctly; the system did not persist enough per-slot lifecycle evidence to explain why no result or diagnostic was written.
