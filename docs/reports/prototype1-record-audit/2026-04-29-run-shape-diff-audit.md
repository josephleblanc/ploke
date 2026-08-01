# Prototype 1 Run Shape Diff Audit

Scope: `p1-crown-baseline-3gen-15nodes-2` compared with older Prototype 1 runs `p1-3gen-15nodes-run12` and `p1-crown-baseline-3gen-15nodes-1`, plus shared batch logs under `/home/brasides/.ploke-eval/batches`.

## Compared Shapes

`p1-crown-baseline-3gen-15nodes-2` and `p1-3gen-15nodes-run12` are schema-identical at the campaign layer:
- same file set: `campaign.json`, `closure-state.json`, `prototype1-loop-trace.json`, `prototype1/branches.json`, `prototype1/scheduler.json`, `prototype1/transition-journal.jsonl`, `slice.jsonl`
- same top-level keys in `campaign.json`, `closure-state.json`, `prototype1-loop-trace.json`, `prototype1/scheduler.json`, and `prototype1/branches.json`
- same scheduler counts: 10 nodes, 7 frontier, 3 completed, 0 failed
- same transition journal size and kind mix: 59 entries, with kinds `parent_started`, `spawn_child`, `build_child`, `child`, `observe_child`, `materialize_branch`, `child_artifact_committed`, `successor`, `successor_handoff`, `active_checkout_advanced`

`p1-crown-baseline-3gen-15nodes-1` is older and materially smaller:
- file set is only `campaign.json`, `prototype1/scheduler.json`, `slice.jsonl`
- `prototype1/scheduler.json` lacks `last_continuation_decision`
- scheduler counts are 1 node, 1 frontier, 0 completed, 0 failed
- `slice.jsonl` keeps the same record keys as later runs

`slice.jsonl` is stable across all three runs:
- keys: `base`, `body`, `f2p_tests`, `fix_patch`, `fix_patch_result`, `fixed_tests`, `hints`, `instance_id`, `n2p_tests`, `number`, `org`, `p2p_tests`, `repo`, `resolved_issues`, `run_result`, `s2p_tests`, `state`, `test_patch`, `test_patch_result`, `title`

The campaign `transition-journal.jsonl` records also have a stable shape in the newer runs:
- top-level keys: `kind`, `recorded_at`, `campaign_id`, `parent_identity`, `repo_root`, `pid`
- nested `parent_identity` keys: `schema_version`, `campaign_id`, `parent_id`, `node_id`, `generation`, `branch_id`, `artifact_branch`, `created_at`

## Shared Batch Log Drift

The shared batch files outside worktrees are a different schema family from campaign records.

`/home/brasides/.ploke-eval/batches/prototype1-smoke-clap3670/prototype1-loop-trace.json` tracks:
- `baseline_instances`, `baseline_summary_path`, `batch_id`, `batch_manifest`, `completed_instances`, `pending_stages`, `prepared_instances`, `protocol_failures`, `protocol_task_instances`, `protocol_tasks`, `selected_targets`, `stage_reached`, `trace_path`

The current campaign loop trace tracks instead:
- `baseline_instances`, `batch_id`, `batch_manifest`, `branch_evaluations`, `branch_registry_path`, `campaign_id`, `campaign_manifest`, `closure_state_path`, `completed_instances`, `continuation_decision`, `continued_from_branch_id`, `continued_from_campaign`, `dry_run`, `pending_stages`, `prepared_instances`, `protocol_failures`, `protocol_task_instances`, `scheduler_path`, `search_policy`, `selected_next_branch_id`, `selected_targets`, `slice_dataset_path`, `stage_reached`, `staged_nodes`, `trace_path`

`batch-run-summary.json` also has a distinct operational schema centered on instance counts and run mode (`instances_attempted`, `instances_failed`, `instances_succeeded`, `instances_total`, `mode`, `stopped_early`).

## Likely Code Changes Behind the Shape Shift

- `7653408c` (`Add Prototype 1 History framework and metrics CLI`) is the main inflection point. It added `history.rs`, `history_preview.rs`, `metrics.rs`, and `report.rs`, and it expanded `cli_facing.rs` and `journal.rs`. That lines up with the newer campaign-level `transition-journal.jsonl`, the richer loop trace, and the report projections.
- `d9b2b5b0` (`print prototype1 journal transitions in monitor`) explains the monitor-side journal visibility.
- `ef5d6074` (`Add prototype1 parent identity guardrails`) matches the nested `parent_identity` payload shape in `transition-journal.jsonl`.
- The newer `scheduler.json` field `last_continuation_decision` is present in the current and run12 snapshots and absent in run1, so the scheduler/report path is now carrying a continuation decision instead of only node collections.

## Gaps / Risks

- The current run family still spans multiple mutable projections rather than one sealed history object, so cross-file consistency is inferred, not enforced.
- `p1-crown-baseline-3gen-15nodes-1` is not directly comparable on parity with the later runs; it predates the richer record set.
- The shared batch logs and campaign logs should not be merged without schema tags; they are related operational artifacts, not the same record type.
