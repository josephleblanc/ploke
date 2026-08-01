# Authority Status Scout: p1-selectfix-replay-5g1x2-a2-20260607-224502

Status: scout/incomplete. This is not a final run review.

Observed at: 2026-06-08T01:18:03-07:00.

## Verdict

The campaign was still live at observation time. Host process evidence showed the parent
`prototype1-state` process still running, with one live generation-4 child runner:

- PID 1753984, elapsed 20:50: `/home/brasides/.ploke-eval/worktrees/p1-selectfix-replay-5g1x2-a2-20260607-224502/target/debug/ploke-eval loop prototype1-state --campaign p1-selectfix-replay-5g1x2-a2-20260607-224502 --repo-root /home/brasides/.ploke-eval/worktrees/p1-selectfix-replay-5g1x2-a2-20260607-224502 --handoff-invocation /home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-86b020f1dfdcf8af/invocations/f17b3fb3-3701-4725-b7e3-855df00143e2.json --stop-after complete --format json`
- PID 1849654, parent 1753984, elapsed 08:34: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-eb38c0cf2ab7b58c/bin/ploke-eval loop prototype1-runner --invocation /home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-eb38c0cf2ab7b58c/invocations/22ee2807-a392-4720-acfc-085e53766429.json --execute --format json`

`node-72b640e0a78cafcc` finished during this scout pass and wrote a keep result for
`branch-4fa5078a62ae685e`. `node-eb38c0cf2ab7b58c` remained running, and its expected
runner result files were not present yet. The terminal campaign verdict is therefore
not available.

## Evidence Roots

- Campaign root: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Parent worktree: `/home/brasides/.ploke-eval/worktrees/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Instances root: `/home/brasides/.ploke-eval/instances/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Protocol root: `/home/brasides/.ploke-eval/protocol/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502`
- Run profile: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/run-profile.toml`
- Run profile commitment: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/run-profile.commitment.json`
- Parent identity file observed in active checkout: `/home/brasides/.ploke-eval/worktrees/p1-selectfix-replay-5g1x2-a2-20260607-224502/.ploke/prototype1/parent_identity.json`
- Transition journal: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/transition-journal.jsonl`
- Scheduler projection: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/scheduler.json`
- Branch registry: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/branches.json`
- Sealed History block: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/history/blocks/segment-000000.jsonl`
- Live node root: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-eb38c0cf2ab7b58c`
- Live child workspace: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/workspaces/edit-harness/node-86b020f1dfdcf8af`
- Completed sibling node root: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-72b640e0a78cafcc`
- Completed sibling workspace: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/workspaces/edit-harness/node-86b020f1dfdcf8af-r2`

## Compact Checklist

| Surface | Label | Status |
| --- | --- | --- |
| Host process liveness | operator/convenience record | Live parent PID 1753984 and live child PID 1849654 were present by `ps`. |
| Campaign manifest | present | `/campaign.json` names `google/gemini-3.5-flash`, `direct_google`; protocol uses `google/gemini-2.5-flash`, `direct_google`, `max_tokens=8000`. |
| Run profile and commitment | present | Profile admits 5 generations, 64 total nodes, full-batch search, `parallel_targets=2`, broad TUI `max_attempts=2`, `fresh_slots_per_child=2`, continuous control. |
| Parent identity | record present, manual join needed | Active checkout parent identity still names generation-0 `node-8cf286015fb78261`; current live parent is proven by later transition/handoff records for `node-86b020f1dfdcf8af`. |
| Transition journal | present | 210 JSONL records; latest observed records show `node-72b640e0a78cafcc` result written and observed after, while `node-eb38c0cf2ab7b58c` remains under observation. |
| Scheduler projection | operator/convenience record | Present but stale for current status: updated at initial setup and still lists only root frontier `node-8cf286015fb78261`. |
| Branch registry | present | 11 records; latest registry row is keep for `branch-4fa5078a62ae685e`. No registry row yet for live `branch-0b3c3b943489e60b`. |
| Sealed History selection rows | present | `segment-000000.jsonl` has 5 sealed decision entries through successor selection for generation 3. No generation-4 successor decision yet. |
| Current live child node record | present | `node-eb38c0cf2ab7b58c` is `running`, generation 4, branch `branch-0b3c3b943489e60b`. |
| Current live child result | record absent | Expected `runner-result.json` and `results/22ee2807-a392-4720-acfc-085e53766429.json` were absent while the child process was live. |
| Completed sibling result | present | `node-72b640e0a78cafcc` wrote `runner-result.json` and `results/997d6d39-0360-4cef-b0d2-a7662f2671a6.json`, status `succeeded`. |
| Baseline closure | present | Baseline eval is complete; baseline protocol is partial with only intent segmentation complete. |
| Live generation-4 protocol | not applicable | The live child has not reached a persisted terminal result, so absence of final protocol artifacts is not a missing-record claim. |

## Authority Spine

The admitted profile is continuous and broad: `max_generations=5`, `max_total_nodes=64`,
`stop_on_first_keep=false`, `require_keep_for_continuation=false`,
`explore_from_rejected=true`, child fanout max 2, `parallel_targets=2`, and broad TUI
`max_attempts=2`. Selection is `history-score-child-prop` with seed `0`, oracle mode
`record-only`, and protocol evidence routed to `google/gemini-2.5-flash` through
`direct-google`.

The active checkout parent identity file is present but not the current-live authority
by itself. It still records:

- `parent_id`: `node-8cf286015fb78261`
- `generation`: `0`
- `branch_id`: `prototype1-parent-p1-selectfix-replay-5g1x2-a2-20260607-224502-gen0`
- mtime: `2026-06-07 22:46:43.029072856 -0700`

The fresher authority is the transition journal plus the handoff invocation. The parent
process was launched with:

`--handoff-invocation /home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-86b020f1dfdcf8af/invocations/f17b3fb3-3701-4725-b7e3-855df00143e2.json`

The journal records a successor handoff to `node-86b020f1dfdcf8af`, then generation-4
materialization/build/spawn records for:

- `node-eb38c0cf2ab7b58c`, branch `branch-0b3c3b943489e60b`, candidate `broad-harness-g4-01`, target `crates/ploke-db/src/database.rs`, workspace `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/workspaces/edit-harness/node-86b020f1dfdcf8af`
- `node-72b640e0a78cafcc`, branch `branch-4fa5078a62ae685e`, candidate `broad-harness-g4-02`, target `crates/ploke-tui/src/app_state/database.rs`, workspace `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/workspaces/edit-harness/node-86b020f1dfdcf8af-r2`

The scheduler projection should not be used as current frontier authority for this
scout. Its file is present, but it is still at the initial setup state: updated
`2026-06-08T05:46:43.021054351+00:00`, frontier `node-8cf286015fb78261`, no completed
or failed nodes.

## Branch Registry And Selection

The branch registry had 11 records at observation time. Counts were 6 keep and
5 reject. The latest row was:

- `branch_id`: `branch-4fa5078a62ae685e`
- `parent_branch_id`: `branch-564844dfd9b65469`
- `candidate_id`: `broad-harness-g4-02`
- `target_relpath`: `crates/ploke-tui/src/app_state/database.rs`
- `disposition`: `keep`
- `evaluated_at`: `2026-06-08T08:15:26.553596355+00:00`
- treatment campaign id: `p1-selectfix-replay-5g1x2-a2-20260607-224502-treatment-branch-4fa5078a62ae685e-1780906168213`

There was no branch-registry row yet for the still-live `branch-0b3c3b943489e60b`.

Sealed History has five selection decision entries. The latest sealed selection is for
successor selection generation 3:

- entry id: `3bc6c6a9-b852-412a-b95f-be3bc59d0572`
- selected candidate: `candidate:node-86b020f1dfdcf8af:plan_index=0`
- selected node/branch: `node-86b020f1dfdcf8af`, `branch-564844dfd9b65469`
- total weight: `0.49939152206676907`
- sample: `0.3426439547636411`
- selected index: `4`
- selected row performance/weight: `19120` / `0.24981737356603065`

That selected row was also the highest-weight row in the latest sealed decision. The next
largest row was `node-44b3f97965446494` / `branch-e21d8f841040888f` with weight
`0.14321343244732065`. No sealed generation-4 successor decision existed yet.

## Closure And Protocol Surface

Baseline closure exists and is mechanically partial for protocol:

- closure file: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/closure-state.json`
- baseline run root: `/home/brasides/.ploke-eval/instances/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502/BurntSushi__ripgrep-2209/runs/run-1780897832425-structured-current-policy-31f7977c`
- baseline record: `/home/brasides/.ploke-eval/instances/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502/BurntSushi__ripgrep-2209/runs/run-1780897832425-structured-current-policy-31f7977c/record.json.gz`
- protocol anchor: `/home/brasides/.ploke-eval/protocol/prototype1/p1-selectfix-replay-5g1x2-a2-20260607-224502/BurntSushi__ripgrep-2209/runs/run-1780897832425-structured-current-policy-31f7977c/1780898108334_tool_call_intent_segmentation_BurntSushi__ripgrep-2209.json`

Closure says baseline eval is complete and baseline protocol is partial:
`tool-call-intent-segments=complete`, `tool-call-review=missing`,
`tool-call-segment-review=missing`, with `total_calls=53` and `reviewed_calls=0`.
This is baseline closure only; it does not decide the live generation-4 child.

## Live Child Notes

`node-eb38c0cf2ab7b58c` has these live authority files:

- node record: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-eb38c0cf2ab7b58c/node.json`
- runner request: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-eb38c0cf2ab7b58c/runner-request.json`
- invocation: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-eb38c0cf2ab7b58c/invocations/22ee2807-a392-4720-acfc-085e53766429.json`
- stdout: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-eb38c0cf2ab7b58c/streams/22ee2807-a392-4720-acfc-085e53766429/stdout.log`
- stderr: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-eb38c0cf2ab7b58c/streams/22ee2807-a392-4720-acfc-085e53766429/stderr.log`
- child-to-parent channel: `/home/brasides/.ploke-eval/campaigns/p1-selectfix-replay-5g1x2-a2-20260607-224502/prototype1/nodes/node-eb38c0cf2ab7b58c/channels/22ee2807-a392-4720-acfc-085e53766429/child-to-parent.jsonl`

The live stdout tail included a non-terminal warning at `2026-06-08T01:15:07.829-07:00`
with code `TOOL_EXECUTION_FAILED`. The same log showed earlier benchmark prompt
construction and repeated embedding-context warnings. Because the process was still
live and no runner result had been written, this scout treats that as a live warning,
not a terminal failure.

## Boundaries

This scout did not run `prototype1-state`, `prototype1-step`, or `prototype1-continue`.
It did not open sqlite or database files. It used read-only process probes, `jq`,
`find`, `stat`, `tail`, and the read-only Prototype 1 run summary helper as a first-pass
map. The report is intentionally incomplete while the host process remains live.
