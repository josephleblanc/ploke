# Incomplete-state review: 090815 broad slots base-r5

## Short verdict

The five scoped broad-harness slots are request-only, not timeout-bearing or trace-bearing attempts. For `node-0cdf3741b09283fe`, `-r2`, `-r3`, `-r4`, and `-r5`, the campaign persisted edit-harness request JSON/Markdown files and clean candidate workspaces, then the child-plan rejected each slot because there was no submitted result and no expected `*.headless-tui.json` diagnostic. I found no slot-specific model/tool trace, terminal log, `turn-live` directory, submitted result, candidate diff, or validation output for these five slots.

Common cause classification: common lifecycle/artifact gap after request publication and workspace creation. Per-slot cause classification: no distinct per-slot cause is evidenced; the slots differ only by request id/hash, candidate workspace branch, and child-plan rejection row.

## Evidence roots

- Worker contract: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/WORKER_CONTRACT.md`
- Inventory: `/home/brasides/.ploke-eval/review-handoffs/p1-admissionfix-g35flash-p25flash-20260606-090815/INVENTORY.json`
- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815`
- Repo / review checkout: `/home/brasides/code/ploke`
- Child plan: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/child-plan/node-0cdf3741b09283fe.json`
- Request directory: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/edit-harness-request`
- Expected result directory: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/messages/edit-harness-result`
- Candidate workspace root: `/home/brasides/.ploke-eval/campaigns/p1-admissionfix-g35flash-p25flash-20260606-090815/prototype1/workspaces/edit-harness`
- Baseline/protocol run root, for contrast only: `/home/brasides/.ploke-eval/instances/prototype1/p1-admissionfix-g35flash-p25flash-20260606-090815/BurntSushi__ripgrep-2209/runs/run-1780762798969-structured-current-policy-b8dc71f0`

## Exact execution path and evidence

The relevant intended path is:

```text
prototype1 parent/state loop
-> broad-harness-request generation
-> messages/edit-harness-request/<slot>.json + .md
-> candidate workspace creation from target artifact 2bcf9ead0e2ef53db740471c40056ec76b8926cf
-> expected headless-TUI adapter path: tui_adapter::run_headless_with_model
-> expected submitted result or messages/edit-harness-result/<slot>.headless-tui.json diagnostic
-> child-plan admission or rejection
```

The persisted evidence proves the path stopped before any trace-bearing headless-TUI attempt for these slots:

- `prototype1/transition-journal.jsonl` has only `parent_started` and `resource` rows for parent `node-0cdf3741b09283fe`; no child/runtime/slot event appears there.
- `prototype1/messages/edit-harness-request/` has 10 request JSON/Markdown pairs, including all five scoped slots.
- `prototype1/workspaces/edit-harness/` has 10 slot workspaces, including all five scoped slots.
- `prototype1/messages/edit-harness-result/` does not exist.
- `node-0cdf3741b09283fe`'s child plan has `children: []` and 10 `rejected_surface_attempts`; the first five rejection rows point to the missing expected `*.headless-tui.json` paths listed below.

Because the trace never reached `tui_adapter::run_headless_with_model` evidence, there is no model/tool event chain for these five slots. The concrete lifecycle trace chain available here is:

```text
transition-journal parent_started/resource
-> per-slot edit-harness request JSON and prompt MD persisted
-> per-slot candidate workspace persisted at target commit 2bcf9ead
-> child-plan rejected_surface_attempt row names expected headless diagnostic path
-> filesystem check confirms result directory and expected headless files are absent; git check confirms workspace has no candidate diff
```

## Closure state: mechanical completion is not slot usefulness

Campaign closure is mechanically complete for the baseline eval/protocol row:

- `closure-state.json` reports `eval.status = complete` and `protocol.status = complete` for `BurntSushi__ripgrep-2209`.
- The protocol run has 52 total calls and 52 reviewed calls.
- The bundled trace audit for the baseline/protocol run root reported 53 responses, 52 provider-emitted tool calls, 52 recorded tool calls, and zero missing recorded provider call ids.

That baseline/protocol completion should not be counted as broad-slot success. These five broad slots produced no patch, no submitted edit result, no headless diagnostic, no candidate commit beyond the initialized parent commit, and no model-visible validation output. They are benchmark-useless surfaces, even though the separate baseline eval/protocol run is mechanically complete.

## Per-slot inspection

All five request JSONs have the same normalized contract after removing slot-specific ids and paths:

- `edit_policy`: `workspace_except_ploke_eval`
- protected core policy: `workspace_except_ploke_eval_authority_set`
- source repository path: `/home/brasides/.ploke-eval/worktrees/p1-admissionfix-g35flash-p25flash-20260606-090815`
- target artifact: `artifact:git-commit:2bcf9ead0e2ef53db740471c40056ec76b8926cf`
- validation command requested of the child: `cargo check -p ploke-eval` in the candidate workspace
- instructions: inspect repository/evidence, treat protocol diagnostics as guidance, edit broad surface outside protected core, choose likely descendant improvement, stage candidate change

| slot | request id | request hash / child-plan run id | workspace branch | workspace HEAD | git state | submitted result | expected headless diagnostic |
| --- | --- | --- | --- | --- | --- | --- | --- |
| `node-0cdf3741b09283fe` | `broad-harness-request:node-0cdf3741b09283fe` | `a449b4bd1f025f71decd95041869fdff6ebc0e95b36c53eed3434d9ef4a8ef68` | `prototype1-broad-broad-harness-request-node-0cdf3741b09283fe` | `2bcf9ead0e2ef53db740471c40056ec76b8926cf` | clean, no diff | absent: `.../edit-harness-result/node-0cdf3741b09283fe.json` | absent: `.../edit-harness-result/node-0cdf3741b09283fe.headless-tui.json` |
| `node-0cdf3741b09283fe-r2` | `broad-harness-request:node-0cdf3741b09283fe:r2` | `a5694df61885d1aa835213aecac56e5aba8d850e8bd4f4c3a62bcc4db577eef4` | `prototype1-broad-broad-harness-request-node-0cdf3741b09283fe-r2` | `2bcf9ead0e2ef53db740471c40056ec76b8926cf` | clean, no diff | absent: `.../edit-harness-result/node-0cdf3741b09283fe-r2.json` | absent: `.../edit-harness-result/node-0cdf3741b09283fe-r2.headless-tui.json` |
| `node-0cdf3741b09283fe-r3` | `broad-harness-request:node-0cdf3741b09283fe:r3` | `19a4a244e15e4f6bcd2a56f9b928e7bbdf0136b9c9f53300c3f8b096eec88689` | `prototype1-broad-broad-harness-request-node-0cdf3741b09283fe-r3` | `2bcf9ead0e2ef53db740471c40056ec76b8926cf` | clean, no diff | absent: `.../edit-harness-result/node-0cdf3741b09283fe-r3.json` | absent: `.../edit-harness-result/node-0cdf3741b09283fe-r3.headless-tui.json` |
| `node-0cdf3741b09283fe-r4` | `broad-harness-request:node-0cdf3741b09283fe:r4` | `6ebd2442bf63d347c7470008fb60a5c494f895a861808d42a89a4a4f95d32e5e` | `prototype1-broad-broad-harness-request-node-0cdf3741b09283fe-r4` | `2bcf9ead0e2ef53db740471c40056ec76b8926cf` | clean, no diff | absent: `.../edit-harness-result/node-0cdf3741b09283fe-r4.json` | absent: `.../edit-harness-result/node-0cdf3741b09283fe-r4.headless-tui.json` |
| `node-0cdf3741b09283fe-r5` | `broad-harness-request:node-0cdf3741b09283fe:r5` | `303952375971447f9c448bcf63dcf153281bddaebcfb451eed3f6110bfeb5999` | `prototype1-broad-broad-harness-request-node-0cdf3741b09283fe-r5` | `2bcf9ead0e2ef53db740471c40056ec76b8926cf` | clean, no diff | absent: `.../edit-harness-result/node-0cdf3741b09283fe-r5.json` | absent: `.../edit-harness-result/node-0cdf3741b09283fe-r5.headless-tui.json` |

Additional workspace facts:

- Each scoped workspace is a standalone git worktree/repository with top-level git root equal to that slot workspace path.
- Each scoped workspace has last commit `2bcf9ead prototype1: initializing gen 0 parent node-0cdf3741b09283fe`.
- Each scoped workspace has empty `git status --porcelain=v1` and empty `git diff --stat`.
- Each scoped workspace includes copied development/config directories such as `.codex`, `.hermes`, and `.ploke`, but the inspected root-level evidence did not include `turn-live`, `logs`, or `tmp` directories. `.codex` contained skill files, and `.hermes` contained pre-existing plans/scripts copied with the repository, not slot runtime traces.

## Request, workspace, log, and result surface classification

### Present / manual join needed

- `messages/edit-harness-request/<slot>.json` and `.md`: present for all five scoped slots; manually joined by slot id/path.
- Candidate workspace: present for all five scoped slots; manually joined by request JSON `candidate_workspace_path` and verified with git.
- Child-plan rejection rows: present in `messages/child-plan/node-0cdf3741b09283fe.json`; manually joined by `proposal_id`, `run_id`, and expected diagnostic path.
- Parent runner request: present at `prototype1/nodes/node-0cdf3741b09283fe/runner-request.json`, even though the handoff inventory's node row listed `runner_request: null`. This is an inventory/join inconsistency, not evidence of a slot execution.

### Record absent

- `messages/edit-harness-result/` directory: absent.
- Per-slot submitted result JSONs: absent for all five scoped slots.
- Per-slot `*.headless-tui.json` diagnostics: absent for all five scoped slots.
- Per-slot terminal/log/turn-live artifacts: no root-level `turn-live`, `logs`, or analogous slot runtime directory found in the scoped workspaces or campaign result surface.
- Slot model/tool lifecycle: absent; no provider response, tool call, edit proposal lifecycle, cargo output, or validation record can be joined to these five slots.
- Campaign `prototype1/history/blocks` and `prototype1/evaluations`: absent in this campaign surface, despite being listed in the broad request evidence roots. That means the request pointed the child at evidence roots that had no local campaign files for the child to inspect.

### Operator/convenience or non-authority evidence

- The review handoff inventory is useful discovery evidence but not the highest authority for file existence; it disagreed with the filesystem on `runner-request.json`.
- `closure-state.json` is a completion projection for baseline eval/protocol status. It is not authority that broad-slot child attempts ran.
- Source files inside candidate workspaces with names containing `trace`, `log`, `diagnostic`, or `result` are repository content, not runtime diagnostics for these slots.

## Suspicious result verification

Two suspicious surfaces were checked against persisted artifacts/checkout instead of trusted directly:

1. A broad filename scan for trace/log-like paths returned thousands of hits because the copied repository contains source files and docs named `trace`, `diagnostic`, `log`, and `result`. A root-level workspace inspection showed no `turn-live`, `logs`, or slot runtime output directories; `.codex` contained only copied skills and `.hermes` contained copied plans/scripts. I therefore did not count those source-tree hits as diagnostics.
2. The handoff inventory reported the parent node row with `runner_request: null`, but the campaign filesystem contains `prototype1/nodes/node-0cdf3741b09283fe/runner-request.json`. Reading that file shows it is a parent state-loop runner request (`loop prototype1-state --repo-root ...`), not a per-slot headless attempt record. This is a missing join/inventory bug, not a slot success signal.

## What is working

- Broad request publication worked: all five scoped request JSON/Markdown pairs exist, with stable request hashes that match the child-plan `run_id` fields.
- Workspace materialization worked: all five candidate workspaces exist, are git repositories, and are clean at the intended target commit.
- Child-plan admission failed closed: it did not create children from request-only surfaces and recorded explicit rejected-surface rows.
- Baseline protocol accounting is internally consistent for the separate baseline run: the trace audit found 52 provider-emitted tool calls and 52 recorded tool calls.

## What is not working yet

- The broad headless-TUI lifecycle has no persisted no-start/no-spawn/no-trace diagnostic for these slots. The earliest wrong or absent transition is between request/workspace publication and the expected headless attempt result/diagnostic surface.
- Child-plan rejection explains the absence at the output path, but not the cause. It does not cite a process id, spawn command, terminal record, adapter error, timeout, provider failure, or workspace-level failure.
- Campaign closure/protocol status can look complete while broad slots are benchmark-useless. The review has to manually keep these planes separate.
- Request evidence roots include campaign `history/blocks` and `evaluations` paths that are absent in this campaign surface; the child prompts may have pointed at empty/nonexistent guidance roots.
- Inventory generation or handoff assembly missed the existing parent `runner-request.json`, which can mislead reviewers into thinking less parent-loop evidence exists than actually does.
- The copied workspace tree contains many files whose names look like diagnostics. Review tooling needs to distinguish execution artifacts from repository content before claiming logs/traces exist.

## Action items

1. Artifact gap: when a broad headless slot is requested but no `edit-harness-result` file is produced, persist a typed no-start/no-spawn/no-trace record with the attempted command path, adapter entrypoint, pid if any, exit status if any, and stderr/stdout locator if any. This closes the gap between request publication and child-plan rejection.
2. Lifecycle gap: teach child-plan rejection rows to join to the last observed lifecycle record for the slot, not only the expected missing output path. If no lifecycle record exists, say `no lifecycle record found` explicitly.
3. Missing join: repair the inventory/handoff generator so `nodes/<node>/runner-request.json` is detected when present. The current inventory row's `runner_request: null` disagrees with the persisted campaign file.
4. Protocol blind spot: keep baseline eval/protocol closure separate from broad-harness slot coverage in synthesized reports. `eval/protocol complete` should not imply child slot execution or benchmark usefulness.
5. Artifact gap: classify absent request evidence roots (`prototype1/history/blocks`, `prototype1/evaluations`) when request JSON points children there, so reviewers and child agents know whether guidance roots are present, absent, or intentionally not applicable.
6. Tooling blind spot: add log/trace discovery filters that ignore copied repository source files and only count whitelisted runtime artifact roots such as `messages/edit-harness-result`, slot-local `turn-live`, adapter terminal records, and typed node/channel/invocation directories.
7. Observability/staleness lead: surface the run-profile name/source mismatch (`run-profile.toml` name from the earlier `...053302` campaign, committed under the `...090815` campaign) as copied-profile metadata so it is not mistaken for campaign authority.

## Follow-up handoff

This report should be treated as an incomplete-state review, not a durable trace-bearing run review. It is suitable input for synthesis as evidence of a common broad-harness observability/lifecycle gap across slots base through r5. It should not be cited as proof of model timeout, provider failure, tool failure, validation failure, or bad patch quality for any scoped slot, because none of those traces exist in the inspected artifacts.
