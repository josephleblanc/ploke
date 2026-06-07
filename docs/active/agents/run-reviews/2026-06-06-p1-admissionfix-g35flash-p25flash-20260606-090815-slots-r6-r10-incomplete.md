# Incomplete-state report: 090815 broad slots r6-r10

## Short verdict

Slots `node-0cdf3741b09283fe-r6` through `node-0cdf3741b09283fe-r10` are request-only broad headless-TUI attempts. Each slot has a published request JSON, prompt, clean candidate workspace, and child-plan rejection entry, but no submitted edit result, no `.headless-tui.json` diagnostics, no raw-provider/turn-live sidecar, and no per-slot process/log record found in the campaign artifacts reviewed here.

Do not classify these as timeouts. The available evidence proves absence of the expected result/diagnostic transition, not the runtime cause of that absence.

## Scope and classification

- Campaign: `p1-admissionfix-g35flash-p25flash-20260606-090815`
- Instance: `BurntSushi__ripgrep-2209`
- Parent node: `node-0cdf3741b09283fe`
- Reviewed slots: `node-0cdf3741b09283fe-r6`, `-r7`, `-r8`, `-r9`, `-r10`
- Classification: `incomplete / request-only / no submitted result / no headless diagnostics`

## Evidence roots

- Worker contract: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/WORKER_CONTRACT.md`
- Inventory: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/INVENTORY.json`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815`
- Prototype 1 root: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1`
- Child plan: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/child-plan/node-0cdf3741b09283fe.json`
- Request directory: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/edit-harness-request`
- Expected result directory: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/edit-harness-result`
- Workspace root: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/workspaces/edit-harness`
- Code path references read from repo checkout:
  - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2649-2695` publishes broad edit-harness requests.
  - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2412-2422` constructs the exact “produced no submitted result or diagnostics” rejection when the expected `.headless-tui.json` is missing.
  - `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:2611-2624` persists a rejected child plan and marks the parent failed when admitted children are below the required minimum.
  - `crates/ploke-eval/src/replay/self_edit.rs:104-133` shows the replay/self-edit headless path calls `tui_adapter::run_headless_with_model`; no slot evidence reached a persisted trace for that path.

## Execution path and evidence

Confirmed lifecycle path for this incomplete surface:

```text
prototype1-state parent start
-> publish broad edit-harness request JSON/MD for slot
-> create candidate workspace branch at artifact commit 2bcf9ead0e2ef53db740471c40056ec76b8926cf
-> expected broad headless-TUI result/diagnostic write does not appear
-> child-plan records rejected_surface_attempts with no children
-> parent node projection is marked failed
```

Evidence for that path:

- `transition-journal.jsonl` records only `parent_started` and a `resource` measurement for the parent. It names PID `2593254`; `ps -p 2593254` returned no live process during review, and a scoped `pgrep` for this campaign/headless/prototype1 terms returned no live matching process.
- `runner-request.json` shows the parent runner command was `loop prototype1-state --repo-root /home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260606-090815`.
- Each reviewed slot request has `edit_policy = workspace_except_ploke_eval`, protected core anchored at `crates/ploke-eval/src/cli/prototype1_state/backend/mod.rs`, and validation contract `cargo check -p ploke-eval` in the candidate workspace.
- Each reviewed workspace is a git worktree at the target artifact commit `2bcf9ead0e2ef53db740471c40056ec76b8926cf`, on its per-slot `prototype1-broad-broad-harness-request-...` branch, with `git status --short --branch` showing only the branch header and no changes.
- The child plan has `children: []` and five scoped rejected entries for r6-r10, each saying the slot produced no submitted result or diagnostics at its expected `.headless-tui.json` path.

Concrete trace chain available here:

```text
transition-journal parent_started for node-0cdf3741b09283fe
-> request JSON r6-r10 published with candidate workspace and expected submitted_result_path
-> workspace git state verifies branch exists at unmodified target artifact commit
-> expected edit-harness-result directory/path is absent
-> child-plan rejected_surface_attempts records “no submitted result or diagnostics”
-> node.json status becomes failed with no runner-result/results/invocations records
```

No model/tool trace chain exists for r6-r10 because no `.headless-tui.json`, raw full-response sidecar, agent-turn trace, proposal lifecycle, or submitted result was found for these slots.

## Slot evidence table

| slot | request hash / run id | request + prompt | workspace git state | submitted result | headless diagnostics | per-slot notes |
|---|---|---|---|---|---|---|
| `node-0cdf3741b09283fe-r6` | `c5faf5fe5f16af6099ac6de168311c8da27642a01d4d268fefc6a83580a9ae69` | JSON and `.md` present | branch `prototype1-broad-broad-harness-request-node-0cdf3741b09283fe-r6`, HEAD `2bcf9ead`, clean | absent | absent | `.ploke` only contains `prototype1/parent_identity.json`; no root `turn-live`, `logs`, `agent-turn-trace.json`, `llm-full-responses.jsonl`, or `record.json.gz` |
| `node-0cdf3741b09283fe-r7` | `bd2f861a6313fbb17ba99e101cef5299fa38f306137b2ed3e138cc0d2dbcdee2` | JSON and `.md` present | branch `prototype1-broad-broad-harness-request-node-0cdf3741b09283fe-r7`, HEAD `2bcf9ead`, clean | absent | absent | same evidence pattern as r6 |
| `node-0cdf3741b09283fe-r8` | `9b586accb47cfee62e080ee748fedd6a58cb40eb2f1eab085cbd1f932d19c7bb` | JSON and `.md` present | branch `prototype1-broad-broad-harness-request-node-0cdf3741b09283fe-r8`, HEAD `2bcf9ead`, clean | absent | absent | same evidence pattern as r6 |
| `node-0cdf3741b09283fe-r9` | `74c6019dc58cea29f26920e3b255b3d31ef8f671ff906b75cf697bc15733022f` | JSON and `.md` present | branch `prototype1-broad-broad-harness-request-node-0cdf3741b09283fe-r9`, HEAD `2bcf9ead`, clean | absent | absent | same evidence pattern as r6 |
| `node-0cdf3741b09283fe-r10` | `afc30ccd94e86534d5c037140d3004cf953ad445674ac616d0056a8d9195f8a7` | JSON and `.md` present | branch `prototype1-broad-broad-harness-request-node-0cdf3741b09283fe-r10`, HEAD `2bcf9ead`, clean | absent | absent | same evidence pattern as r6; prompt is one byte longer because the slot id is `r10` |

The expected submitted-result paths are the request JSON `submitted_result_path` values:

```text
.../prototype1/messages/edit-harness-result/node-0cdf3741b09283fe-r6.json
.../prototype1/messages/edit-harness-result/node-0cdf3741b09283fe-r7.json
.../prototype1/messages/edit-harness-result/node-0cdf3741b09283fe-r8.json
.../prototype1/messages/edit-harness-result/node-0cdf3741b09283fe-r9.json
.../prototype1/messages/edit-harness-result/node-0cdf3741b09283fe-r10.json
```

The child-plan rejection checks the corresponding diagnostics paths:

```text
.../prototype1/messages/edit-harness-result/node-0cdf3741b09283fe-r6.headless-tui.json
.../prototype1/messages/edit-harness-result/node-0cdf3741b09283fe-r7.headless-tui.json
.../prototype1/messages/edit-harness-result/node-0cdf3741b09283fe-r8.headless-tui.json
.../prototype1/messages/edit-harness-result/node-0cdf3741b09283fe-r9.headless-tui.json
.../prototype1/messages/edit-harness-result/node-0cdf3741b09283fe-r10.headless-tui.json
```

The `edit-harness-result` directory itself was absent during review, which verifies the suspicious child-plan claim against persisted artifacts instead of trusting the rejection text alone.

## Mechanical completion vs benchmark usefulness

- Campaign closure is mechanically complete for the baseline/protocol run: `closure-state.json` reports eval complete and protocol complete for `BurntSushi__ripgrep-2209`, with 52 tool calls reviewed under the protocol artifact root named in closure.
- That mechanical closure does not make r6-r10 benchmark-useful. These broad slots produced no child nodes, no patch, no validation output, no Multi-SWE-bench submission, and no oracle/MBE evidence.
- For r6-r10 specifically, the only mechanical completion is negative accounting: the child plan persisted rejected surface attempts and the parent node projection is `failed`.
- Benchmark usefulness for r6-r10 is therefore zero/undetermined, not “bad patch” or “timeout”: no candidate edit result exists to evaluate.

## Common vs per-slot cause

Common observed cause across r6-r10:

- Requests and workspaces were generated successfully.
- Workspaces remained clean at the target artifact commit.
- The expected result/diagnostics surface was never written.
- The child plan rejected all slots from the same missing artifact condition.

Per-slot differences:

- Only slot ids, request hashes, branch names, and prompt-path sizes differ.
- No slot-specific trace, process log, tool failure, provider response, candidate diff, or validation output was found.
- There is no artifact-backed basis to assign a distinct per-slot cause to r6, r7, r8, r9, or r10.

Earliest wrong/absent transition:

```text
published request + clean workspace exists
-> expected headless worker/result writer should produce either submitted result or diagnostics
-> no result/diagnostics/log sidecar exists
```

The root cause is not fully diagnosable from current artifacts; the missing transition is observable, but the process-level reason is not persisted.

## Missing joins and record labels

Using the project record-persistence labels:

- `present`: published edit request JSON/MD for each slot.
- `present`: candidate workspace and `.ploke/prototype1/parent_identity.json` for each slot.
- `present`: child plan with rejected surface attempts for each slot.
- `present`: parent `runner-request.json` and `node.json`.
- `record absent`: submitted edit result JSON for each slot.
- `record absent`: `.headless-tui.json` diagnostics for each slot.
- `record absent`: per-slot raw full-response sidecar / `llm_full_response*.log` evidence for these attempts.
- `record absent`: per-slot terminal/process invocation record proving the actual headless command, PID, start/end time, exit status, timeout, or provider failure.
- `record absent`: per-slot proposal lifecycle (`staged`, `applied`, `failed`, `stale`, `denied`) and model-visible validation-command output.
- `record present, manual join needed`: workspace git state had to be verified manually against each candidate workspace branch and request target artifact.
- `operator/convenience record`: `INVENTORY.json` is useful as a review handoff and matched the inspected paths, but the authority for missing r6-r10 results is the request/child-plan/workspace filesystem state.
- `record present, manual join needed`: closure/protocol artifacts exist for the baseline run, but must be manually kept separate from the missing broad-harness slots.

Additional observability mismatches:

- `prototype1/run-profile.toml` has `name = "p1-admissionfix-g35flash-p25flash-20260606-053302"` while the campaign id is `p1-admissionfix-g35flash-p25flash-20260606-090815`. `run-profile.commitment.json` says this profile came from the earlier campaign path. Treat this as stale metadata/observability evidence, not as the authority for the current campaign id.
- `scheduler.json` still lists the parent node as `planned`/frontier, while `nodes/node-0cdf3741b09283fe/node.json` is `failed`. That is a projection/lifecycle consistency gap for operators trying to infer state from scheduler alone.

## What is working

- Broad request publication works: r6-r10 request JSON/MD files exist and are internally consistent.
- Workspace materialization works: each slot has a candidate worktree with parent identity and the expected target artifact commit.
- Rejection accounting works at the child-plan layer: `children` is empty and each rejected slot records a reason tied to the missing diagnostics path.
- The read-side inventory correctly classifies these as request-only and warns not to invent timeout causes.

## What is not working yet

- The broad headless-TUI attempt lifecycle is not observable after request/workspace creation for these slots.
- The result directory is not present, and no per-slot fallback diagnostics explain whether the headless worker never launched, launched and crashed, hit a provider/tool issue, timed out, or failed before writing.
- The child-plan rejection reason names the missing diagnostics path, but it cannot explain the process-level cause because no launch/exit record is joined to the slot.
- The scheduler projection and node projection disagree after failure, increasing the chance that a later reviewer trusts stale `planned`/frontier metadata.

## Action items

1. Persist a per-slot headless launch/exit record before and after invoking the broad headless-TUI path. Tie it to `request_id`, `request_hash`, workspace path, command/argv, PID, start/end timestamps, exit status, timeout status, and redacted provider route. This addresses the observed artifact gap where r6-r10 have no process-level evidence.
2. Make the headless adapter write a minimal `.headless-tui.json` diagnostic on every terminal failure class, including launch failure, provider failure, timeout, panic, no-edit exhaustion, and interrupted process. This addresses the lifecycle gap where the child plan can only say “no submitted result or diagnostics.”
3. Add a raw-provider/turn sidecar join for broad slots when no compact diagnostics exists, or record explicitly that no provider request was made. This addresses the missing join between request publication and model/tool evidence.
4. Update the child-plan rejection payload to include checked sibling artifacts: submitted result existence, diagnostics existence, workspace git HEAD/status, and launch-record path/status. This turns the current manual filesystem verification into first-class evidence.
5. Repair or flag the scheduler/node projection mismatch after rejected broad-harness admission. `scheduler.json` should not remain the only visible `planned`/frontier surface when `node.json` has been updated to `failed`.
6. Fix the run-profile name/source observability issue or make copied profile provenance explicit in the campaign UI/reporting. This prevents the stale `053302` profile name from being mistaken for the current `090815` campaign authority.

## Follow-up handoff

This report should be consumed as incomplete evidence, not as a durable successful run review. The next repair/synthesis step should classify the finding as an observability/lifecycle gap in the broad headless-TUI slot runner: the pipeline can publish requests and reject missing results, but it does not persist enough per-slot diagnostics to explain why r6-r10 produced no result.
