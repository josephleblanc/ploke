# Prototype 1 Persistence Map: Artifact-Surface Completeness

Date: 2026-05-03
Worker: Completeness Worker B

## Scope

Bounded artifact-surface check against existing local Prototype 1 campaign
directories. I used the known campaign `p1-10gen-5targets-20260502-2` and one
newer campaign, `p1-20gen-ripgrep-20260503-2`, only to check whether a newer
run introduced additional path families.

This pass intentionally did not dump raw `slice.jsonl`, runner records,
transition journals, observation streams, provider payloads, patches, or logs.
Evidence was limited to filename/path families, `wc -c` byte counts, `find
-printf` counts and sizes, and small `jq` key/count extraction.

## Campaigns Observed

`~/.ploke-eval/campaigns/p1-10gen-5targets-20260502-2`

- Present.
- `prototype1/scheduler.json`: policy reports `max_generations=10`,
  `max_total_nodes=64`, `stop_on_first_keep=false`,
  `require_keep_for_continuation=true`.
- Scheduler shape: 31 nodes, 30 frontier ids, 1 completed id, 0 failed ids.
- Scheduler statuses: 20 planned, 10 running, 1 succeeded.
- Non-worktree/non-target Prototype 1 file families include scheduler,
  branches, history, evaluations, child-plan messages, node records,
  runner requests/results, invocation/result records, streams, and
  successor-ready/completion records.

`~/.ploke-eval/campaigns/p1-10gen-5targets-20260502-2-treatment-branch-*`

- 10 sibling treatment campaign directories were present.
- Observed file family is only `campaign.json` plus `closure-state.json` at
  depth checked.
- Approximate bytes: 10 `campaign.json` files total 13,820 bytes; 10
  `closure-state.json` files total 59,574 bytes.
- These are baseline/treatment evaluation campaign surfaces, not
  campaign-local `prototype1/` state.

`~/.ploke-eval/campaigns/p1-20gen-ripgrep-20260503-2`

- Present and recent.
- It did not add a new path family compared with the known campaign.
- Early shape: 1 node, 1 frontier id, 0 completed, 0 failed, status running.
- Observed file families: `campaign.json`, `slice.jsonl`,
  `closure-state.json`, `prototype1/scheduler.json`,
  `prototype1/transition-journal.jsonl`, one `node.json`, one
  `runner-request.json`.

## Observed Families in Known Campaign

Approximate counts and sizes for `p1-10gen-5targets-20260502-2`:

| Path family | Count | Approx bytes observed | Likely owner / type |
| --- | ---: | ---: | --- |
| `campaign.json` | 1 | 1,246 | `CampaignManifest`; `campaign_manifest_path` in `crates/ploke-eval/src/campaign.rs` |
| `slice.jsonl` | 1 | 55,996 | benchmark/campaign slice input; setup/eval campaign surface |
| `closure-state.json` | 1 | 5,419 | `ClosureState`; `closure_state_path` / `campaign_closure_state_path` |
| `prototype1/scheduler.json` | 1 | 53,010 | `Prototype1SchedulerState`; `intervention::scheduler` |
| `prototype1/branches.json` | 1 | 75,448 | `Prototype1BranchRegistry`; `intervention::branch_registry` |
| `prototype1/transition-journal.jsonl` | 1 | 250,069 | `PrototypeJournal`; typed transition journal |
| `prototype1/history/blocks/segment-000000.jsonl` | 1 | 27,395 | `FsBlockStore`; sealed History block segment |
| `prototype1/history/index/by-hash.jsonl` | 1 | 1,836 | `FsBlockStore`; History block index projection |
| `prototype1/history/index/by-lineage-height.jsonl` | 1 | 1,287 | `FsBlockStore`; History lineage/height index projection |
| `prototype1/history/index/heads.json` | 1 | 104 | `FsBlockStore`; History heads projection |
| `prototype1/evaluations/branch-*.json` | 10 | 24,624 | `Prototype1BranchEvaluationReport`; treatment-vs-baseline branch reports |
| `prototype1/messages/child-plan/node-*.json` | 10 | 14,641 | `ChildPlanFile`; parent-owned child-plan message box |
| `prototype1/nodes/node-*/node.json` | 31 | 48,251 | `Prototype1NodeRecord`; scheduler-owned node summary |
| `prototype1/nodes/node-*/runner-request.json` | 31 | 36,699 | `Prototype1RunnerRequest`; node runner configuration |
| `prototype1/nodes/node-*/runner-result.json` | 10 | 5,891 | `Prototype1RunnerResult`; latest node result |
| `prototype1/nodes/node-*/invocations/<runtime>.json` | 19 | 7,707 | `Invocation`; child/successor runtime authority token |
| `prototype1/nodes/node-*/results/<runtime>.json` | 10 | 5,891 | attempt-scoped runner result |
| `prototype1/nodes/node-*/streams/<runtime>/stderr.log` | 19 | 215,907 | process stream file from child/successor spawn |
| `prototype1/nodes/node-*/streams/<runtime>/stdout.log` | 19 | 19,077 | process stream file from child/successor spawn |
| `prototype1/nodes/node-*/successor-ready/<runtime>.json` | 9 | 2,446 | `SuccessorReadyRecord`; detached successor acknowledgement |
| `prototype1/nodes/node-*/successor-completion/<runtime>.json` | 9 | 2,556 | `SuccessorCompletionRecord`; successor terminal status |
| `prototype1/nodes/node-*/bin/ploke-eval` | 1 | 839,004,152 | child runtime binary copied/built for evaluation |
| `prototype1/nodes/node-*/worktree/` | 1 realized tree, 2,297 files | at least 8,001,233 bytes at maxdepth 3; largest files include assets up to 3,286,659 bytes | backend-managed child workspace; not authoritative scheduler state |
| `prototype1/nodes/node-*/target/` | 1 realized target tree, 13,242 files | largest files include `debug/ploke-eval` at 839,004,152 bytes and many large incremental artifacts | build products; not durable identity |
| `prototype1/nodes/node-*/worktree/.ploke/prototype1/parent_identity.json` | 1 | 450 | artifact-carried parent identity witness inside realized worktree |

Root-level `prototype1-loop-trace.json` was also present in the known campaign
at 8,354 bytes. Code comments and monitor-location metadata identify it as a
legacy loop-controller trace that is overwritten per run. It is not under the
`prototype1/` subtree in this campaign; it sits at campaign root.

## Producer / Owner Cross-Check

The observed families line up with the module-level persistence inventory in
`crates/ploke-eval/src/cli/prototype1_state/mod.rs`, which names the main
durable files under `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`:
`scheduler.json`, `branches.json`, `evaluations/<branch-id>.json`,
`transition-journal.jsonl`, `messages/child-plan/<parent-node-id>.json`,
node records, runner request/result records, invocation/result records,
successor-ready/completion records, node `worktree/`, node `bin/`, and node
`target/`.

More specific ownership points:

- `crates/ploke-eval/src/intervention/scheduler.rs` owns path construction and
  persistence for `scheduler.json`, `nodes/<node-id>/node.json`,
  `runner-request.json`, and `runner-result.json`.
- `crates/ploke-eval/src/intervention/branch_registry.rs` owns
  `prototype1/branches.json`.
- `crates/ploke-eval/src/cli/prototype1_process.rs` writes branch evaluation
  reports to `prototype1/evaluations/branch-*.json`, records attempt-scoped
  runner results, and creates child/successor stream files.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs` models
  `ChildPlanFile`, `SchedulerFile`, `BranchesFile`, `NodeFile`, and
  `RunnerRequestFile` as typed parent/child protocol files.
- `crates/ploke-eval/src/cli/prototype1_state/invocation.rs` owns
  `invocations/`, `results/`, `successor-ready/`, and
  `successor-completion/` path construction and JSON persistence.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs` owns
  `history/blocks/segment-000000.jsonl` plus `history/index/by-hash.jsonl`,
  `by-lineage-height.jsonl`, and `heads.json` through `FsBlockStore`.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs` writes
  `prototype1-loop-trace.json` on the legacy loop-controller path and exposes
  monitor location metadata for most operator-facing surfaces.
- `crates/ploke-eval/src/tracing_setup.rs` can write observation streams under
  `~/.ploke-eval/logs/prototype1_observation_<run_id>.jsonl`. Recent local
  logs exist, including a 2026-05-03 file of 290,200 bytes, but those logs are
  outside campaign directories and were not counted as campaign-local state.

## Persisted Surfaces Easy To Miss

These files are present or code-produced but are not as visible as the obvious
`prototype1-monitor list/report` state families:

- `prototype1/messages/child-plan/node-*.json`: present in the known campaign
  as 10 files. This is a typed parent-owned message box, not merely derived
  scheduler output.
- `prototype1/history/blocks/*` and `prototype1/history/index/*`: present in
  the known campaign. These are History block/index projections and should be
  included in persistence completeness checks separately from
  `transition-journal.jsonl`.
- `prototype1/nodes/node-*/streams/<runtime>/{stdout,stderr}.log`: present as
  19 runtime stream directories. They are process evidence, not structured
  protocol records, and can grow much larger than the JSON control-plane files.
- `prototype1/nodes/node-*/worktree/.ploke/prototype1/parent_identity.json`:
  present inside the realized child worktree. This is artifact-carried identity
  evidence and may be missed if checks prune all worktree contents.
- Campaign-root `prototype1-loop-trace.json`: present and code-produced, but
  legacy and overwritten per run. Its root placement differs from the monitor
  metadata wording that joins it to `prototype_root`.
- Sibling treatment campaign directories:
  `p1-10gen-5targets-20260502-2-treatment-branch-*` contain
  `campaign.json` and `closure-state.json` only at the checked depth. These are
  part of evaluation evidence, but they are outside the base campaign's
  `prototype1/` subtree.
- Observation streams under `~/.ploke-eval/logs/prototype1_observation_*.jsonl`
  are code-produced and locally present, but are not campaign-local and are not
  obviously recoverable from a campaign directory alone.

## Completeness Read

For the checked campaigns, the observed base campaign artifact families are
covered by obvious producer code or module documentation. The newer campaign
does not introduce a new family; it is only an earlier/shallower instance of
the same scheduler-node-journal shape.

The main completeness risk is not an unknown file family in these directories.
It is classification drift: worktree/target/bin/stream surfaces are persisted
on disk and can dominate size, but they are not the same kind of evidence as
typed scheduler, invocation, History, or journal records. A persistence map
should keep those families explicit instead of folding them into generic
`node artifacts`.
