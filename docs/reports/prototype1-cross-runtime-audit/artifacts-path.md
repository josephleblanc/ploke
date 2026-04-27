# Prototype 1 Cross-Runtime Artifact Audit

Scope: artifact-first audit under `/home/brasides/.ploke-eval/campaigns`, with
`p1-3gen-15nodes-run10` as the primary run and older handoff evidence from
`p1-3gen-15nodes-run2` and `p1-3gen-15nodes-run5`. Code was read only where
needed to identify writers/readers. No code or runtime artifacts were changed.

## What Actually Happened

Run10 initialized a generation 0 parent in the active checkout
`/home/brasides/.ploke-eval/worktrees/p1-loop-run-3gen-15nodes-7`.
The active artifact contains `.ploke/prototype1/parent_identity.json` for:

```text
campaign_id: p1-3gen-15nodes-run10
node_id: node-f7b9c198a79ca49a
generation: 0
branch_id: prototype1-parent-p1-3gen-15nodes-run10-gen0
artifact_branch: prototype1-parent-p1-3gen-15nodes-run10-gen0
```

The journal records only `parent_started` for that parent. Target selection then
ran in dry-run mode and produced three candidate branches for
`crates/ploke-core/tool_text/read_file.md`. Those candidates were persisted in
`branches.json`, and three planned nodes were persisted in `scheduler.json` plus
per-node `node.json` and `runner-request.json`.

The attempted child selection failed before any run10 child was materialized,
built, spawned, or evaluated. Run10 has no `runner-result.json`,
`results/<runtime>.json`, `successor-ready`, `successor-completion`, or console
stream files. The key artifact mismatch is that the active parent is generation
0 node `node-f7b9c198a79ca49a`, but the candidate nodes are generation 2 with
`parent_node_id = node-c47370c1ae880bf4`. That synthetic parent is what
`prototype1_node_id("prototype1-parent-p1-3gen-15nodes-run10-gen0", 1)`
produces; it is not present as a scheduler node in run10.

Older runs show the later handoff path:

- Run2 selected and evaluated `node-1053b44a590be929` for
  `branch-51bc12724660825b`, recorded `overall_disposition = keep`, advanced
  the active checkout to generation 1, wrote `successor-ready`, and started the
  successor as the new parent.
- Run5 selected and evaluated `node-4bc34fe1b649386a` for
  `branch-6c7911b8eaf89c17`, recorded an evaluation artifact with
  `overall_disposition = reject`, still advanced the active checkout because of
  the temporary successor-eligibility short circuit, wrote `successor-ready`,
  then immediately wrote `successor-completion` with `status = failed`.

## Communication Points

| Document/path | Writer role | Reader role | Fields used as authority | Fields duplicated elsewhere | Observed mismatch or absence |
| --- | --- | --- | --- | --- | --- |
| Active `.ploke/prototype1/parent_identity.json` in `/home/brasides/.ploke-eval/worktrees/p1-loop-run-3gen-15nodes-7` | Initial parent setup | `prototype1-state` parent resolver and backend checkout validator | `campaign_id`, `node_id`, `generation`, `branch_id`, `artifact_branch` | Same values appear in root `node.json`, `scheduler.json`, `transition-journal.jsonl` `parent_identity` | Good authority for the active parent, but downstream branch synthesis in run10 received only `branch_id`; `node_id` and `generation` did not cross into candidate registration. |
| `p1-3gen-15nodes-run10/prototype1/transition-journal.jsonl` `parent_started` | Parent runtime | Human/monitor/replay only; not used to authorize children | `parent_identity`, `repo_root`, `pid` | Duplicates active `parent_identity.json` and root scheduler node | Only `parent_started` exists in run10. There is no durable blocked/failed typed transition after target selection fails to yield a runnable child. |
| `p1-3gen-15nodes-run10/prototype1/branches.json` | Parent-side target selection/synthesis | Scheduler staging and branch resolver | `source_state_id`, `parent_branch_id`, `source_artifact_id`, `operation_target`, `selected_branch_id`, branch `patch_id` and content hashes | `source_state_id`, `operation_target`, `base_artifact_id`, `patch_id`, `branch_id`, `candidate_id`, `target_relpath` are duplicated into scheduler nodes and runner requests | `parent_branch_id` is `prototype1-parent-p1-3gen-15nodes-run10-gen0`, but no `parent_node_id` or parent generation is stored. The reader later reconstructs the parent and gets a nonexistent gen1 parent. |
| `p1-3gen-15nodes-run10/prototype1/scheduler.json` root node | Root parent registration | Parent checker, candidate resolver, scheduler UI | `node_id`, `generation`, `branch_id`, `workspace_root`, status lists | Duplicated by root `node.json`, root `runner-request.json`, active `parent_identity.json` | Root remains `status = planned` while the parent is already active and started. The root parent's durable role is carried by identity/journal, not by scheduler status. |
| `p1-3gen-15nodes-run10/prototype1/nodes/node-f7b9c198a79ca49a/node.json` | Root parent registration | Parent checker via `Parent<Unchecked>::load` | `node_id`, `generation`, `branch_id`, `instance_id`, `workspace_root` | Duplicates scheduler root entry and runner request | Root `status = planned` conflicts with observed `parent_started`. It is a scheduler planning status, not a typed parent lifecycle state. |
| `p1-3gen-15nodes-run10/prototype1/nodes/node-f7b9c198a79ca49a/runner-request.json` | Root parent registration | CLI/operator runner path | `runner_args`, `workspace_root`, `campaign_id`, `node_id` | Duplicates root node and scheduler path fields | Present but not the authority actually used by the already-running parent. It has no typed handoff token. |
| Run10 candidate `node.json` files for `node-a3b258a61220d9eb`, `node-d39773b381f66e41`, `node-ea09000c0ee5151c` | Scheduler staging from branch registry | Typed parent candidate resolver, C1 materialization | `generation`, `parent_node_id`, `branch_id`, `status`, `workspace_root`, artifact and patch fields | Duplicated into `scheduler.json` and each `runner-request.json`; artifact fields duplicate `branches.json` | All three are `generation = 2` with `parent_node_id = node-c47370c1ae880bf4`; the active parent can only consume generation 1 children with `parent_node_id = node-f7b9c198a79ca49a`. |
| Run10 candidate `runner-request.json` files | Scheduler staging | Child runner invocation path if spawned | `campaign_id`, `node_id`, `branch_id`, `workspace_root`, `binary_path`, `runner_args` | Duplicates corresponding `node.json`, minus `parent_node_id` and `parent_branch_id` | Requests inherit the wrong generation. They omit `parent_node_id`, so a child runner would not independently know the parent it is evaluating for. |
| Run10 `results/<runtime>.json`, latest `runner-result.json`, streams | Child runtime | Parent observer and scheduler status updater | Expected: `status`, `disposition`, `evaluation_artifact_path`, exit details | Expected latest result duplicates attempt-scoped result | Absent in run10, consistent with failure before child spawn. The absence is not itself represented by a typed terminal record. |
| Run2/run5 child invocation JSON under `nodes/<node>/invocations/<runtime>.json` | Parent runtime spawning child | Child runtime | `role = child`, `campaign_id`, `node_id`, `runtime_id`, `journal_path` | `campaign_id`, `node_id` duplicate node/request/scheduler; `runtime_id` duplicates journal child events and `results/<runtime>.json` path | Role split is clear, but invocation carries no parent identity or expected generation. It trusts the node record selected by the parent. |
| Run2/run5 child ready/evaluating/result journal records in `transition-journal.jsonl` | Child runtime and parent observer | Parent observer, human/replay | `runtime_id`, `refs.node_id`, `refs.branch_id`, `paths.workspace_root`, `state`, `runner_result_path` | Duplicates node/request paths and result path; branch refs duplicate branches/scheduler | These are event projections, not transition authority. They do not enforce that the child belongs to the active parent except by already-selected node identity. |
| Run2 `nodes/node-1053b44a590be929/results/9073a45e-...json` and latest `runner-result.json` | Child runtime | Parent observer, scheduler updater, later successor selector | `status = succeeded`, `disposition = succeeded`, `evaluation_artifact_path`, `branch_id` | Attempt-scoped result equals latest runner result; evaluation path points to `evaluations/branch-51bc12724660825b.json` | Result says runner success, while continuation eligibility comes from the separate evaluation artifact's `overall_disposition = keep`. No parent identity is recorded in the result. |
| Run5 `nodes/node-4bc34fe1b649386a/results/fbcb9ecb-...json` and latest `runner-result.json` | Child runtime | Parent observer, scheduler updater, successor selector | `status = succeeded`, `disposition = succeeded`, `evaluation_artifact_path`, `branch_id` | Attempt-scoped result equals latest runner result; evaluation path points to `evaluations/branch-6c7911b8eaf89c17.json` | Runner success is not branch acceptance: the evaluation artifact says `overall_disposition = reject`. Scheduler marks the node `succeeded` and completed anyway. |
| Run2/run5 `evaluations/branch-*.json` | Child runtime branch-evaluation path | Parent observer and branch registry summary writer | `overall_disposition`, compared instance rows, reasons | Summarized in `branches.json` `latest_evaluation`; result points to this path | Run5 shows `overall_disposition = reject` but successor continuation still happened because selection code temporarily treats any completed child evaluation as successor-eligible. |
| Run2/run5 `branches.json` after evaluation | Child runtime summary writer plus successor selection | Branch resolver, active-target reader | `selected_branch_id`, `active_targets[*].active_branch_id`, branch `latest_evaluation` | Duplicates scheduler `branch_id`, runner result `branch_id`, evaluation summary | In run5, `active_targets` selects a branch whose `latest_evaluation.overall_disposition = reject`; selected branch and accepted branch are conflated. |
| Run2/run5 `scheduler.json` after child observation | Parent observer/scheduler | Candidate resolver, successor validator | `completed_node_ids`, node `status`, `last_continuation_decision` | Duplicates latest `runner-result.json` status and branch id; continuation selected branch duplicates branches active target | `last_continuation_decision` is campaign-global and keyed by branch id, not by selected node id, parent id, child runtime id, or handoff attempt. |
| Run2/run5 `child_artifact_committed` journal entry | Parent runtime after child build | Human/replay; successor install relies on the committed branch, not directly on this record | `parent_identity`, `child_identity`, `child_branch`, `target_commit`, `identity_commit` | Child identity duplicates future active `parent_identity.json`; branch and node ids duplicate scheduler | Strongest provenance record observed, but produced after candidate selection. It cannot prevent run10's earlier wrong parent/generation registration. |
| Active `.ploke/prototype1/parent_identity.json` after Run2/Run5 advance | Predecessor parent installing successor artifact | Successor parent start | `campaign_id`, `node_id`, `generation`, `previous_parent_id`, `branch_id`, `artifact_branch` | Duplicates `active_checkout_advanced.selected_parent_identity`, child artifact identity, scheduler node | Good authority for successor becoming parent. In run5 it admits a rejected branch as parent because the temporary selection policy allowed it. |
| Run2/run5 `active_checkout_advanced` journal entry | Predecessor parent | Human/replay; successor parent validates active checkout separately | `previous_parent_identity`, `selected_parent_identity`, `selected_branch`, `installed_commit` | Duplicates active `parent_identity.json`, child artifact commit entry, branch registry active target | Observability record only. It does not itself token-bind the successor process to that installed commit. |
| Run2/run5 successor invocation JSON | Predecessor parent | Successor runtime during `--handoff-invocation` | `role = successor`, `campaign_id`, `node_id`, `runtime_id`, `journal_path`, `active_parent_root` | `campaign_id`, `node_id`, `runtime_id` duplicate ready/completion docs and handoff journal; active root duplicates handoff journal | This is the handoff token, but validation still consults mutable `scheduler.last_continuation_decision` instead of an immutable attempt-scoped selection record. |
| Run2/run5 `successor-ready/<runtime>.json` | Successor runtime before entering parent turn | Predecessor parent wait loop | `campaign_id`, `node_id`, `runtime_id`, `pid`, `recorded_at` | Duplicates successor invocation and `successor_handoff` journal | Ready only proves the successor process reached acknowledgement. It does not prove it completed a bounded generation. |
| Run5 `successor-completion/a13a0d1d-...json` | Successor runtime error handler | Human/monitor; predecessor has already accepted ready | `status = failed`, `runtime_id`, `detail` | Duplicates successor invocation runtime/node and stderr log detail | Predecessor recorded handoff as acknowledged before this failure. Failure says the live artifact did not match stored source content before materialization. |
| Run5 successor stderr log | Successor runtime | Human/monitor | Error string from failed rehydrated parent turn | Duplicates `successor-completion.detail` | Confirms the post-ready failure: `expected live artifact at './crates/ploke-core/tool_text/read_file.md' to match stored source content before materialization`. |

## Contradictions And Hidden Assumptions

1. `parent_identity.json` is the real parent authority, but target synthesis and
   candidate registration use branch handles and reconstructed IDs. Run10 loses
   the active parent's `node_id` and `generation` before writing child records.

2. `branches.json` stores `parent_branch_id`, not a typed parent role/state.
   Scheduler then derives `parent_node_id` from `parent_branch_id` plus
   `generation - 1`; run10 proves this can mint a plausible parent that never
   existed.

3. `scheduler.json` mixes queues, node mirrors, and continuation authority.
   `last_continuation_decision` is not a handoff token; it is mutable campaign
   state keyed by selected branch id.

4. Runner success and evaluation acceptance are separate facts. In run5 the
   runner result says `succeeded`, while the evaluation artifact says `reject`;
   the temporary policy still promoted that child into the next parent.

5. Several durable records duplicate identity fields without a single owner:
   `campaign_id`, `node_id`, `generation`, `branch_id`, `target_relpath`,
   `workspace_root`, `patch_id`, and artifact ids appear across
   `branches.json`, `scheduler.json`, `node.json`, `runner-request.json`,
   result files, journal entries, and active `parent_identity.json`.

6. Worktree paths are recorded as durable handles even when cleanup removes the
   underlying worktree. In run2 and run5 the scheduler and node records still
   point to per-node `worktree` paths that are absent after active checkout
   advancement.

7. The predecessor accepts successor-ready before successor completion. Run5 has
   both `successor_handoff` and `successor-ready`, followed by
   `successor-completion.status = failed`.

## Top 5 Observed Record Mismatches

1. Run10 active parent identity is `node-f7b9c198a79ca49a`, generation 0, but
   all three staged child nodes are generation 2 children of nonexistent
   `node-c47370c1ae880bf4`.

2. Run10 root `node.json` and `scheduler.json` keep the active parent at
   `status = planned` even though `transition-journal.jsonl` records
   `parent_started` for that same node.

3. Run5 `runner-result.json` records `status = succeeded` and
   `disposition = succeeded`, while
   `evaluations/branch-6c7911b8eaf89c17.json` records
   `overall_disposition = reject`.

4. Run5 `branches.json` marks `active_targets.active_branch_id =
   branch-6c7911b8eaf89c17` even though that branch's latest evaluation is
   rejected.

5. Run2 and run5 scheduler/node records retain `workspace_root` values under
   `prototype1/nodes/<node>/worktree`, but those worktree directories are gone
   after active checkout advancement.
