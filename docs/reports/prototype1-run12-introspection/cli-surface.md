# Prototype 1 Run12 CLI Surface

Campaign: `p1-3gen-15nodes-run12`

Run worktree: `/home/brasides/.ploke-eval/worktrees/p1-loop-run-3gen-15nodes-8`

Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run12/prototype1`

Scope: completed-run introspection only. I did not start the loop and did not run `prototype1-monitor watch`.

## Useful Commands Found

- `./target/debug/ploke-eval help`
  Shows the top-level inspector families: `inspect`, `campaign`, `closure`, `select`, and `loop`.
- `./target/debug/ploke-eval loop help`
  Shows the Prototype 1 families: `prototype1-state`, `prototype1-branch`, and `prototype1-monitor`.
- `./target/debug/ploke-eval loop prototype1-monitor --campaign <campaign> --repo-root <repo> list`
  Lists stable and volatile artifact locations for the campaign, including `scheduler.json`, `branches.json`, `transition-journal.jsonl`, evaluations, node records, child streams, and active parent identity.
- `./target/debug/ploke-eval loop prototype1-monitor --campaign <campaign> --repo-root <repo> peek --lines <n> --bytes <n>`
  Prints bounded excerpts from expected files. Useful for checking terminal journal entries, runner results, and parent identity without opening full logs.
- `./target/debug/ploke-eval loop prototype1-branch status --campaign <campaign>`
  Summarizes branch registry shape: source node count, active targets, branch ids, candidate ids, labels, statuses, selected branch ids, and parent/source lineage.
- `./target/debug/ploke-eval loop prototype1-branch show --campaign <campaign> --branch-id <branch>`
  Shows one branch's source state, parent branch, target, content hashes, and proposed content.
- `./target/debug/ploke-eval campaign show --campaign <campaign>`
  Shows campaign config, dataset source, model/provider, budget, protocol settings, and manifest path.
- `./target/debug/ploke-eval closure status --campaign <campaign>`
  Shows closure progress for registry, eval, and protocol.
- `./target/debug/ploke-eval inspect protocol-overview --campaign <campaign> --limit <n> --color never`
  Summarizes campaign protocol triage and issue families for protocol-tracked runs.
- `./target/debug/ploke-eval inspect operational --instance <instance> --format json`
  Summarizes mechanized metrics for the latest registered run for that instance. For this investigation it was not campaign-scoped enough to trust for Prototype 1 branch comparison.

## Exact Commands Run

CLI discovery:

```sh
./target/debug/ploke-eval help
./target/debug/ploke-eval loop help
./target/debug/ploke-eval help inspect
./target/debug/ploke-eval help select
./target/debug/ploke-eval loop prototype1-monitor help
./target/debug/ploke-eval loop prototype1-monitor list --help
./target/debug/ploke-eval loop prototype1-monitor peek --help
./target/debug/ploke-eval loop prototype1-branch help
./target/debug/ploke-eval loop prototype1-branch show --help
./target/debug/ploke-eval loop prototype1-state --help
./target/debug/ploke-eval inspect operational --help
./target/debug/ploke-eval inspect protocol-overview --help
./target/debug/ploke-eval inspect issue-overview --help
./target/debug/ploke-eval campaign help
```

Completed-run inspection:

```sh
./target/debug/ploke-eval loop prototype1-monitor --campaign p1-3gen-15nodes-run12 --repo-root /home/brasides/.ploke-eval/worktrees/p1-loop-run-3gen-15nodes-8 list
./target/debug/ploke-eval loop prototype1-monitor --campaign p1-3gen-15nodes-run12 --repo-root /home/brasides/.ploke-eval/worktrees/p1-loop-run-3gen-15nodes-8 peek
./target/debug/ploke-eval loop prototype1-monitor --campaign p1-3gen-15nodes-run12 --repo-root /home/brasides/.ploke-eval/worktrees/p1-loop-run-3gen-15nodes-8 peek --lines 6 --bytes 2048
./target/debug/ploke-eval loop prototype1-branch status --campaign p1-3gen-15nodes-run12
./target/debug/ploke-eval loop prototype1-branch show --campaign p1-3gen-15nodes-run12 --branch-id branch-50a418fb656c9e04
./target/debug/ploke-eval loop prototype1-branch show --campaign p1-3gen-15nodes-run12 --branch-id branch-5ee5d0b29179ce2f
./target/debug/ploke-eval loop prototype1-branch show --campaign p1-3gen-15nodes-run12 --branch-id branch-617f711f5d4a4a1e
./target/debug/ploke-eval campaign show --campaign p1-3gen-15nodes-run12
./target/debug/ploke-eval closure status --campaign p1-3gen-15nodes-run12
./target/debug/ploke-eval inspect protocol-overview --campaign p1-3gen-15nodes-run12 --limit 20 --color never
./target/debug/ploke-eval inspect operational --instance clap-rs__clap-3670 --format json
./target/debug/ploke-eval select status
```

Commands run to expose gaps:

```sh
./target/debug/ploke-eval loop prototype1-monitor list --campaign p1-3gen-15nodes-run12 --repo-root /home/brasides/.ploke-eval/worktrees/p1-loop-run-3gen-15nodes-8
./target/debug/ploke-eval loop prototype1-monitor peek --campaign p1-3gen-15nodes-run12 --repo-root /home/brasides/.ploke-eval/worktrees/p1-loop-run-3gen-15nodes-8
./target/debug/ploke-eval loop prototype1-state help
./target/debug/ploke-eval inspect issue-overview --campaign p1-3gen-15nodes-run12 --help
```

Bounded non-CLI reads used where no compact CLI report existed:

```sh
jq '{schema_version, campaign_id, completed_node_ids, failed_node_ids, frontier_node_ids, last_continuation_decision, policy}' /home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run12/prototype1/scheduler.json
jq '[.source_nodes[] | {source_state_id, selected_branch_id, branch_ids: [.branches[].branch_id]}]' /home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run12/prototype1/branches.json
jq '.nodes[] | {node_id,generation,status,source_state_id,parent_branch_id,branch_id,candidate_id,target_relpath,workspace_root,created_at,updated_at}' /home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run12/prototype1/scheduler.json
jq -s '[.[] | {branch_id, overall_disposition, reasons, treatment_campaign_id, compared_instances: [.compared_instances[] | {instance_id,status,baseline_metrics,treatment_metrics,evaluation}]}]' /home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run12/prototype1/evaluations/branch-50a418fb656c9e04.json /home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run12/prototype1/evaluations/branch-5ee5d0b29179ce2f.json /home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run12/prototype1/evaluations/branch-617f711f5d4a4a1e.json
tail -n 12 /home/brasides/.ploke-eval/campaigns/p1-3gen-15nodes-run12/prototype1/transition-journal.jsonl
```

## Exposed Data

Scores and metrics:

- The Prototype 1 CLI can point at evaluation artifacts through `prototype1-monitor list/peek` and branch ids through `prototype1-branch status`, but I did not find a compact CLI command that prints all branch comparison metrics.
- Direct bounded reads of `prototype1/evaluations/*.json` showed three compared selected branches:
  - `branch-50a418fb656c9e04`: `overall_disposition=keep`; baseline `tool_calls_total=27`, `tool_calls_failed=6`, `patch_apply_state=no`, `submission_artifact_state=empty`; treatment `tool_calls_total=27`, `tool_calls_failed=8`, `patch_apply_state=no`, `submission_artifact_state=empty`.
  - `branch-5ee5d0b29179ce2f`: `overall_disposition=reject`; reason `same_file_patch_max_streak regressed: 0 -> 1`; treatment `tool_calls_total=20`, `tool_calls_failed=6`, `patch_attempted=true`, `patch_apply_state=no`, `same_file_patch_max_streak=1`.
  - `branch-617f711f5d4a4a1e`: `overall_disposition=keep`; treatment `tool_calls_total=33`, `tool_calls_failed=9`, `patch_attempted=true`, `patch_apply_state=no`, `same_file_patch_max_streak=0`.
- No branch produced `nonempty_valid_patch`, `convergence`, or `oracle_eligible` in the comparison metrics.

Lineage:

- `prototype1-branch status` exposed three source states and nine branches. Each generation selected `candidate-1` / `minimal_rewrite`.
- Selected branch chain:
  - generation 1: `prototype1-parent-p1-3gen-15nodes-run12-gen0` -> `branch-50a418fb656c9e04`
  - generation 2: `branch-50a418fb656c9e04` -> `branch-5ee5d0b29179ce2f`
  - generation 3: `branch-5ee5d0b29179ce2f` -> `branch-617f711f5d4a4a1e`
- `prototype1-branch show` exposed source/proposed content hashes and the target `crates/ploke-core/tool_text/read_file.md` for each selected branch.

Trajectories:

- `scheduler.json` records ten nodes: one root, three succeeded child nodes, and six planned sibling candidates.
- Succeeded trajectory:
  - `node-bdfeda081f4451d4`, generation 1, `branch-50a418fb656c9e04`, succeeded at `2026-04-27T14:42:04.153605278+00:00`.
  - `node-d84bcd2e4ccb3976`, generation 2, `branch-5ee5d0b29179ce2f`, succeeded at `2026-04-27T14:48:02.411103176+00:00`.
  - `node-f755d6f7cb56d976`, generation 3, `branch-617f711f5d4a4a1e`, succeeded at `2026-04-27T14:57:12.088118363+00:00`.
- Frontier after completion contains seven planned nodes: root plus two unselected siblings at each generation.

Stop reason:

- `scheduler.json` and the terminal `transition-journal.jsonl` entries agree on `stop_max_generations`.
- Policy was `max_generations=3`, `max_total_nodes=15`, `stop_on_first_keep=false`, `require_keep_for_continuation=false`.
- Final continuation decision selected `branch-617f711f5d4a4a1e`, proposed `next_generation=4`, and stopped with `total_nodes_after_continue=10`.

Protocol:

- `inspect protocol-overview --campaign p1-3gen-15nodes-run12 --limit 20 --color never` reported one protocol-tracked run, full coverage, 27/27 call reviews, 4/4 usable segment reviews, and issue families including `mixed`, `search_thrash`, `partial_next_step`, and `useful_exploration`.
- `closure status` reported registry complete, eval complete `1/1`, and protocol complete `1/1`.

## CLI Ergonomics Gaps

- Parent options on `prototype1-monitor` must precede the subcommand. `prototype1-monitor list --campaign ...` fails even though `prototype1-monitor --campaign ... list` works. The subcommand help for `list` and `peek` does not display inherited `--campaign` or `--repo-root` options, so the accepted placement is easy to miss.
- `prototype1-monitor peek` is useful but broad. Even with `--lines 6 --bytes 2048`, it prints many files and can include noisy stream excerpts. It lacks filters such as `--only scheduler`, `--only evaluations`, `--only journal`, `--node`, or `--branch`.
- `prototype1-branch status` gives branch lineage but not comparison metrics. `prototype1-branch show` gives branch content and hashes but not evaluation disposition or mechanized deltas.
- `inspect operational --instance clap-rs__clap-3670` resolves the latest registered attempt, not a Prototype 1 branch evaluation chosen by campaign and branch. It also emitted a selection warning: selected campaign `p1-3gen-15nodes-run12` does not include selected instance `clap-rs__clap-3670`.
- `inspect issue-overview` has `--record` and `--instance`, but no `--campaign`; `inspect protocol-overview` does support `--campaign`. The two inspectors are inconsistent for campaign-focused workflows.
- I did not find a single finite CLI command that prints the completed Prototype 1 run as one bounded report: campaign config, scheduler terminal decision, selected branch chain, per-branch evaluation metrics, runner result paths, and protocol summary.

## Recommended Next CLI Report Command

Add a finite report command:

```sh
./target/debug/ploke-eval loop prototype1-report --campaign p1-3gen-15nodes-run12 --repo-root /home/brasides/.ploke-eval/worktrees/p1-loop-run-3gen-15nodes-8 --format markdown
```

Recommended output sections:

- `Run identity`: campaign, worktree, active parent identity, manifest, closure state.
- `Stop reason`: scheduler policy plus `last_continuation_decision`.
- `Selected trajectory`: node id, generation, source state, selected branch, candidate label, status, timestamps, runner result path.
- `Branch comparison`: baseline and treatment operational metrics for every compared branch, overall disposition, rejection reasons, treatment campaign id.
- `Frontier`: planned, succeeded, failed node counts plus the remaining candidate branches.
- `Protocol`: compact campaign protocol-overview counts and top issue families.
- `Artifact index`: paths to `scheduler.json`, `branches.json`, evaluations, transition journal, and node directories.
