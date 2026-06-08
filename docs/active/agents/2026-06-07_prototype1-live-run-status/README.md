# 2026-06-07 Prototype 1 live run status

Date: 2026-06-07

Campaign: `p1-historyfix-handoff-5g1x2-a2-20260607-155010`

Companion files:

- [`experiments.md`](experiments.md) - regressions and validation commands for
  the stale broad-harness handoff fix.
- [`CHANGELOG.md`](CHANGELOG.md) - dated changes to this investigation packet
  and the associated code fix.

Parent worktree observed:
`/home/brasides/.ploke-eval/worktrees/p1-historyfix-handoff-5g1x2-a2-20260607-155010`

Primary campaign root:
`/home/brasides/.ploke-eval/campaigns/p1-historyfix-handoff-5g1x2-a2-20260607-155010`

Instances root:
`/home/brasides/.ploke-eval/instances/prototype1/p1-historyfix-handoff-5g1x2-a2-20260607-155010`

Protocol root:
`/home/brasides/.ploke-eval/protocol/prototype1/p1-historyfix-handoff-5g1x2-a2-20260607-155010`

Reference inventory used:
`crates/ploke-eval/docs/prototype1-child-plan-authority/prototype1-record-inventory.md`.
The sub-agent search found the skill copy at
`.codex/skills/prototype1-loop-run-status/references/prototype1-record-inventory.md`
is byte-identical and newer by mtime. The broader companion is
`.codex/skills/prototype1-loop-run-status/references/prototype1-state-loop-walkthrough-README.md`.

## Verdict

The loop made it through two parent-successor handoffs and completed two
generation-3 child runs, but it did not make it through all five generations.
It reached the successor selection after the generation-3 children, then failed
at successor artifact preparation.

The durable terminal record is in
`prototype1/transition-journal.jsonl` at `2026-06-07T17:33:29-07:00`:

```text
prototype1-state successor failed:
database setup failed during 'prototype1_successor_artifact_prepare':
worktree metadata exists for
'/home/brasides/.ploke-eval/campaigns/p1-historyfix-handoff-5g1x2-a2-20260607-155010/prototype1/workspaces/edit-harness/node-903feb19806d1dd8'
but the path is missing on disk
```

The parent runtime stderr also recorded three `WorkspacePathMismatch` values
before that terminal error. The concrete missing path is the un-suffixed
`node-903feb19806d1dd8` edit-harness workspace. The workspace directory listing
after failure contained:

```text
node-8e50799a4c796fd5-r2
node-903feb19806d1dd8-r2
node-2df04e70d97c9304
node-2df04e70d97c9304-r2
```

So the selected successor branch pointed at metadata for a workspace path that
had already been cleaned or was never preserved in that exact spelling.

## Liveness

Earlier host process evidence, collected before the no-escalation constraint,
showed the parent `prototype1-state` runtime and two child runners live. After
the no-escalation constraint, process inspection could not be refreshed from the
host namespace. The persisted run artifacts are enough for the current verdict:
the parent runtime channel reports `status=failed` at
`2026-06-07T17:33:29-07:00`, so this run should be treated as failed/terminal,
not actively advancing.

The final journal state was:

- `2026-06-07T17:29:45-07:00`: child `node-f76d459fe57da90c` wrote a result and
  was observed as succeeded.
- `2026-06-07T17:33:27-07:00`: child `node-b03707937aa5968b` wrote a result and
  was observed as succeeded.
- `2026-06-07T17:33:27-07:00`: parent selected successor
  `branch-7c33506afa12a2f3`.
- `2026-06-07T17:33:29-07:00`: successor runtime
  `b5555883-5928-4e07-b7b3-f1814377d36f` completed with `status=failed`.

## Configuration

The admitted profile matched the intended minimal long-ish run shape:

- `max_generations = 5`
- `max_total_nodes = 64`
- `stop_on_first_keep = false`
- `require_keep_for_continuation = false`
- `explore_from_rejected = true`
- `parallel_targets = 2`
- `observe_child_stale_after_secs = 1200`
- `max_attempts = 2`
- `fresh_slots_per_child = 2`

The generation model was `google/gemini-3.5-flash` through `direct_google`.
Protocol adjudication used `google/gemini-2.5-flash` through `direct_google`
with `max_tokens = 8000`.

## Directories Examined

Parent and generation roots:

- Parent worktree:
  `/home/brasides/.ploke-eval/worktrees/p1-historyfix-handoff-5g1x2-a2-20260607-155010`
- Campaign root:
  `/home/brasides/.ploke-eval/campaigns/p1-historyfix-handoff-5g1x2-a2-20260607-155010`
- Instance run root:
  `/home/brasides/.ploke-eval/instances/prototype1/p1-historyfix-handoff-5g1x2-a2-20260607-155010`
- Protocol run root:
  `/home/brasides/.ploke-eval/protocol/prototype1/p1-historyfix-handoff-5g1x2-a2-20260607-155010`

Child workspaces:

- Gen1: `prototype1/workspaces/edit-harness/node-8e50799a4c796fd5-r2`
- Gen2: `prototype1/workspaces/edit-harness/node-903feb19806d1dd8-r2`
- Gen3: `prototype1/workspaces/edit-harness/node-2df04e70d97c9304`
- Gen3 retry: `prototype1/workspaces/edit-harness/node-2df04e70d97c9304-r2`

Child node directories:

- `prototype1/nodes/node-3509413f9d0bef24`
- `prototype1/nodes/node-903feb19806d1dd8`
- `prototype1/nodes/node-d00c1b0d118a2ab6`
- `prototype1/nodes/node-2df04e70d97c9304`
- `prototype1/nodes/node-f76d459fe57da90c`
- `prototype1/nodes/node-b03707937aa5968b`

## Phase Timing

The main agent trace timings are from `agent-turn-trace.json` mtimes and
`llm-full-responses.jsonl` creation times. All times below are Pacific.

| Branch/run | Phase | Key timing | Notes |
| --- | --- | --- | --- |
| baseline `run-1780873283774-structured-current-policy-57a73c5a` | baseline self-eval | `repo-state` 16:01:23, LLM 16:01:37 to 16:10:35, trace 16:10:43, record 16:10:51 | 55 LLM responses, 464 trace events |
| baseline | protocol | 16:11:08 to 16:14:01 | 55 protocol artifacts: 54 call reviews, 1 segmentation |
| `branch-eed0761ea6384596` | child self-eval | LLM 16:28:00 to 16:30:29, trace 16:30:34, record 16:30:39 | 31 responses, 282 trace events |
| `branch-eed0761ea6384596` | protocol | 16:30:58 to 16:35:36 | 40 artifacts, 3 failed focal inputs |
| `branch-a7066dda07790eb7` | child self-eval | LLM 16:28:02 to 16:31:03, trace 16:31:08, record 16:31:12 | 29 responses, 265 trace events |
| `branch-a7066dda07790eb7` | protocol | 16:31:28 to 16:33:49 | 33 artifacts, 1 failed focal input |
| `branch-ad50f3e79db6d227` | child self-eval | LLM 16:48:44 to 16:52:37, trace 16:52:43, record 16:52:48 | 49 responses, 461 trace events |
| `branch-ad50f3e79db6d227` | protocol | no protocol artifacts found in the protocol root | treatment closure state remained protocol-missing |
| `branch-7c33506afa12a2f3` | child self-eval | LLM 16:48:44 to 16:58:05, trace 16:58:10, record 16:58:19 | 76 responses, 677 trace events |
| `branch-7c33506afa12a2f3` | protocol | 16:58:51 to 17:05:06 | 83 artifacts, 5 failed focal inputs |
| `branch-6bd866fcb23f3425` | gen3 child self-eval | LLM 17:18:07 to 17:22:56, trace 17:23:01, record 17:23:08 | 52 responses, 458 trace events |
| `branch-6bd866fcb23f3425` | protocol | 17:23:33 to 17:29:44 | 62 artifacts, 5 failed focal inputs |
| `branch-e6583d10838d128e` | gen3 child self-eval | LLM 17:18:04 to 17:25:10, trace 17:25:18, record 17:25:26 | 58 responses, 506 trace events |
| `branch-e6583d10838d128e` | protocol | 17:25:55 to 17:33:26 | 74 artifacts, 2 failed focal inputs |

For the final parent turn, broad generation committed two candidate artifacts
despite model-response warnings:

- `17:11:52`: `INVALID_MODEL_RESPONSE`, then commit
  `prototype1-broad-broad-harness-request-node-2df04e70d97c9304`
  (`2c38649c`).
- `17:14:25`: `INVALID_MODEL_RESPONSE`, then commit
  `prototype1-broad-broad-harness-request-node-2df04e70d97c9304-r2`
  (`1d99b355`).
- `17:16:52` and `17:16:53`: generation-3 children spawned and were observed.

The two final child observe durations in the parent log were:

- `node-f76d459fe57da90c`: `duration_ms=772100`, about 12m52s.
- `node-b03707937aa5968b`: `duration_ms=994967`, about 16m35s.

Both are below `observe_child_stale_after_secs = 1200` seconds. The terminal
failure is therefore not a child-observe timeout.

## Tool Failures

There were obvious tool-call failures in every treatment run, while all
agent-turn terminal records still reported completed sessions.

Mechanical failure examples include:

- `code_item_lookup`: failed to read snippet because content changed for
  `crates/printer/src/standard.rs`.
- `insert_rust_item`: no inline module container found for
  `crate::standard::tests`.
- `apply_code_edit`: invalid method `canon` shape such as
  `crate::util::replace_all`.
- `apply_code_edit`: could not stage a semantic edit because another staged edit
  was pending.
- `apply_code_edit`: could not verify file version because content changed.

The latest kept child, `branch-e6583d10838d128e`, improved the mechanical
metrics compared with its immediate parent:

| Branch | Disposition | Parent failed tools | Child failed tools | Parent retry count | Child retry count | Parent max streak | Child max streak |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| `branch-6bd866fcb23f3425` | reject | 5 | 5 | 5 | 1 | 2 | 2 |
| `branch-e6583d10838d128e` | keep | 5 | 2 | 5 | 0 | 2 | 1 |

## Timeouts

No durable evidence showed a child stale timeout or timeout-based terminal
failure. The relevant configured stale threshold was 20 minutes. Both final
children were observed under that threshold.

There were `INVALID_MODEL_RESPONSE` warnings during broad generation, but the
parent recovered enough to commit both broad-harness result artifacts and launch
both children. Those model-response warnings are not the terminal blocker.

The terminal blocker is the missing workspace path during
`prototype1_successor_artifact_prepare`.

## Fixed-Source Rerun

Follow-up campaign `p1-handofffix-embed-5g1x2-a2-20260607-192954` was run from
the fixed source after the successor-artifact and embedding-override fixes. It
completed the intended parent-to-successor stability probe and stopped cleanly
at the historical traversal budget on `2026-06-07T21:58:24-07:00`.

Key confirmations:

- The rerun used `max_generations = 5`, `parallel_targets = 2`,
  `max_attempts = 2`, `fresh_slots_per_child = 2`,
  `require_keep_for_continuation = false`, and
  `explore_from_rejected = true`.
- The all-rejected generation under `node-8167e33daa3b9bc6` continued from a
  rejected historical candidate, `node-b56d3539fe294ed3`, proving the disabled
  keep gate in the live path.
- Successor handoffs were acknowledged for kept and rejected parents, including
  runtimes `1bdf24b7-34ed-46d6-9a15-03113415f5a4`,
  `637883b5-2843-45ad-9f63-ba2ec73c2cbf`,
  `f5ff0efd-bb58-4255-87f2-6a6b2e7e3000`,
  `6d5ffaef-ad28-4e17-9432-4754fc12e70c`, and
  `7efc6b1e-e943-403c-b52f-7d428b2b2971`.
- The final generation-4 children both completed mechanically with runner
  `exit_code = 0`, then were semantically rejected. The loop selected rejected
  `node-7815b0481a271a5e` as the final coordinate but stopped with
  `stop_historical_traversal_budget` instead of launching another successor.
- No `ploke-eval loop`, `prototype1-state`, or `prototype1-runner` host
  processes remained after the terminal budget stop.

Open follow-up from the rerun: History traversal can revisit already-expanded
parents and can include duplicate formula rows for the same node/branch. This
did not block the fixed-source rerun, but it is a selection-policy/design issue
to address before streamlining the loop.

## Protocol Adjudication

Protocol adjudication is doing useful work on both automatically failed tool
calls and semantic quality patterns.

Examples:

- A failed `apply_code_edit` call in `branch-6bd866fcb23f3425` was automatically
  marked failed. The protocol review then classified it as `overall=mixed`,
  `redundancy=search_thrash`, and `recoverability=clear_next_step`, with a
  rationale that the malformed method `canon` gave a clear recovery path but the
  next edit repeated the pattern without resolving the root cause.
- Clean locate-target segments were classified as `overall=focused_progress`,
  with `redundancy=distinct` and `recoverability=no_recovery_needed`.

Protocol review summaries for the relevant branches:

| Branch | Artifacts | Overall counts | Failed focal inputs |
| --- | ---: | --- | ---: |
| baseline | 55 | focused_progress 22, mixed 27, useful_exploration 3, recoverable_detour 2 | 0 |
| `branch-eed0761ea6384596` | 40 | focused_progress 23, mixed 12, useful_exploration 4 | 3 |
| `branch-a7066dda07790eb7` | 33 | focused_progress 22, mixed 9, recoverable_detour 1 | 1 |
| `branch-7c33506afa12a2f3` | 83 | focused_progress 36, mixed 37, useful_exploration 6, redundant_thrash 2, recoverable_detour 1 | 5 |
| `branch-6bd866fcb23f3425` | 62 | focused_progress 27, mixed 27, useful_exploration 4, recoverable_detour 2, redundant_thrash 1 | 5 |
| `branch-e6583d10838d128e` | 74 | focused_progress 40, mixed 26, useful_exploration 3, recoverable_detour 3, redundant_thrash 1 | 2 |

This answers the automatic-vs-adjudicated split: automatic result types catch
mechanical failed calls; protocol reviews also identify semantic shortcomings
such as search thrash, repeated tool clusters, and recoverable detours in calls
that may still be mechanically valid.

## Selection And Scores

The first two successor selections were sealed into
`prototype1/history/blocks/segment-000000.jsonl` with full `score_child_prop`
formula rows. The final selection at `17:33:27` is present in the transition
journal, but its complete six-candidate formula row table was not sealed into
History before the successor failure. The durable history files have mtimes
around `17:05:19`, while the final branch registry and transition-journal
updates are at `17:33:27-17:33:29`.

Fully persisted earlier score rows:

| Selection | Candidate | Branch | Disposition | Performance | Child count | Weight | Selected |
| --- | --- | --- | ---: | ---: | ---: | ---: | ---: |
| Gen1 | `node-903feb19806d1dd8` | `branch-eed0761ea6384596` | reject | 8395 | 0 | 0.993307149 | yes |
| Gen1 | `node-3509413f9d0bef24` | `branch-a7066dda07790eb7` | reject | 7865 | 0 | 0.006692851 | no |
| Gen2 | `node-903feb19806d1dd8` | `branch-eed0761ea6384596` | reject | 8395 | 2 | 0.059883443 | no |
| Gen2 | `node-3509413f9d0bef24` | `branch-a7066dda07790eb7` | reject | 7865 | 0 | 0.115211302 | no |
| Gen2 | `node-2df04e70d97c9304` | `branch-7c33506afa12a2f3` | reject | 13570 | 0 | 0.972274870 | yes |
| Gen2 | `node-d00c1b0d118a2ab6` | `branch-ad50f3e79db6d227` | reject | 3375 | 0 | 0.001589570 | no |

Final selection summary from the transition journal:

- selected candidate: `node-2df04e70d97c9304`
- selected branch: `branch-7c33506afa12a2f3`
- selected branch disposition: `reject`
- selected from: 6 decision-grade candidates
- strategy: `score_child_prop`
- total weight: `1.199797644`
- sample: `0.081471617`
- selected weight: `0.192131025`
- selected components:
  `performance=13570, child_count=2, alpha=0.724591329, exploitation=0.576393075, exploration=0.333333333`

This is the key selection-health finding: the newest generation-3 child
`branch-e6583d10838d128e` was recorded in `branches.json` as `keep` at
`17:33:27`, with improved failed-tool and retry metrics, but the final successor
selection still chose the older rejected generation-2 branch
`branch-7c33506afa12a2f3`. Because the final full formula rows are missing, this
report cannot prove the exact final rank order of all six candidates. It can
prove the selected next parent was not the latest kept child, and it can prove
the selected candidate had only `0.192131025` weight out of `1.199797644` total
weight in the final stochastic `score_child_prop` selection.

## Root Cause Summary

The immediate root cause of the run failure is not a provider timeout, child
admissibility failure, or protocol failure. It is a successor artifact-prep
failure caused by stale or non-preserved workspace metadata:

1. The parent completed the generation-3 child fanout.
2. `branch-e6583d10838d128e` was evaluated as `keep`.
3. Successor selection chose older rejected `branch-7c33506afa12a2f3`.
4. Preparing that successor artifact required the older edit-harness workspace
   `prototype1/workspaces/edit-harness/node-903feb19806d1dd8`.
5. Only `node-903feb19806d1dd8-r2` existed at the time of inspection.
6. The runtime failed with `worktree metadata exists ... but the path is missing
   on disk`.

System-health implications:

- The run demonstrated parent-to-successor handoff at least twice before this
  failure.
- The keep-gate was disabled in config, but selection behavior still needs
  review because a kept generation-3 child was not selected.
- The cleanup/preservation policy is unsafe for historical candidate selection
  if selected candidates can point at edit-harness workspaces that cleanup may
  remove or that are not normalized to preserved node-owned worktrees.
- History sealing did not capture the final six-candidate score table before
  failure, which made postmortem score comparison incomplete.

## Code Trace To Problem Area

The failure traces to successor handoff rehydration and artifact installation,
not to child admissibility, provider routing, protocol adjudication, or the
child-observe timeout.

Broad-harness materialization records the provisional edit-harness checkout as
the candidate artifact root. In
`crates/ploke-eval/src/cli/prototype1_state/c1.rs:720`, the transition writes
`workspace.candidate_root` into `Prototype1NodeRecord.workspace_root`; at
`c1.rs:724` it writes the same path into the runner request; and at
`c1.rs:746` it stores the same path as `Artifact.repo_root`.

That node record is then sealed as handoff material. `CandidateArtifact` stores
the full `Prototype1NodeRecord` and `ResolvedTreatmentBranch` in
`crates/ploke-eval/src/cli/prototype1_state/history/projection/mod.rs:2512`.
The type comment says this is the material used to reconstruct a typed
`Selection<Artifact>` from History without reading mutable node or branch
projections.

During successor selection, `select_artifact_for_handoff` rehydrates the chosen
payload by cloning the sealed `node` and `resolved` values in
`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6261`. It validates
the selected node id, branch id, sealed coordinate, and artifact surface, then
calls `Selection::artifact` at `cli_facing.rs:6365`. The selection wrapper only
checks the branch/source-state binding in
`crates/ploke-eval/src/cli/prototype1_state/selection.rs:46`; it does not
normalize `workspace_root` or prove the sealed artifact workspace still exists.

The selected artifact then enters successor installation through
`spawn_and_handoff_prototype1_successor` in
`crates/ploke-eval/src/cli/prototype1_process.rs:2478`, which calls
`prepare_prototype1_active_successor_runtime`. In
`install_prototype1_successor_artifact`, the install path calls
`child_artifact_workspace` at `prototype1_process.rs:1423`.
`child_artifact_workspace` first tries the node-owned worktree path and, for a
broad-harness workspace mismatch, falls back to
`workspace_for_artifact_root(&node.workspace_root)` at
`prototype1_process.rs:1487`.

That fallback requires the historical broad-harness checkout path to still
exist. `GitWorktreeBackend::workspace_for_artifact_root` returns
`BackendError::MissingPath` immediately when `workspace_root.exists()` is false
in `crates/ploke-eval/src/cli/prototype1_state/backend/git_worktree.rs:58`.
The displayed error text comes from
`crates/ploke-eval/src/cli/prototype1_state/backend/mod.rs:630`, which says
`worktree metadata exists ... but the path is missing on disk`; in this fallback
path, that wording is misleading because the first failing check is the missing
directory itself.

The same successor install path also removes the selected candidate worktree
after successfully persisting and promoting it. At
`crates/ploke-eval/src/cli/prototype1_process.rs:1436`, if
`node.workspace_root.exists()`, the code commits the target and then calls
`backend.remove(active_parent_root, &workspace)` at
`prototype1_process.rs:1447` before installing the committed artifact into the
active parent checkout. That explains why a historical broad-harness artifact
can still have a valid branch/ref while its original edit-harness checkout no
longer exists.

For this run, the final selector chose `node-2df04e70d97c9304` /
`branch-7c33506afa12a2f3` again after that branch had already been promoted to
the active parent. Its sealed artifact still pointed at the earlier
`prototype1/workspaces/edit-harness/node-903feb19806d1dd8` broad-harness
checkout. That path had already been removed, while the artifact branch and
commit still existed in the active parent checkout. The problematic area is
therefore the boundary between historical artifact selection and successor
installation:

- `ParentSelection::select_successor` in
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6136` can select
  from all admitted history.
- `select_artifact_for_handoff` in
  `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:6257` preserves the
  historical `workspace_root`.
- `install_prototype1_successor_artifact` and `child_artifact_workspace` in
  `crates/ploke-eval/src/cli/prototype1_process.rs:1410` require that preserved
  workspace path to be live.

The likely regression test should reproduce that contract mismatch: select an
already-promoted historical broad-harness artifact whose branch/ref still
exists, remove its original edit-harness checkout, and assert that successor
handoff does not fail by trying to reopen the removed provisional workspace.

## Helper Script

This directory includes a reusable read-only collector:

```bash
python3 docs/active/agents/2026-06-07_prototype1-live-run-status/prototype1_run_summary.py \
  --campaign p1-historyfix-handoff-5g1x2-a2-20260607-155010
```

It summarizes:

- admitted profile settings
- transition journal tail
- node states
- branch registry dispositions
- evaluation metrics
- agent trace and LLM response timings
- protocol artifact counts and review summaries
- sealed History selection formula rows
- timeout/error markers from campaign-owned stream logs

The script does not open sqlite/db files.

## Skill Update

The repo-local skill at `.codex/skills/prototype1-loop-run-status/SKILL.md` now
includes the run-status lessons from this incident:

- prefer the stable record inventory under `crates/ploke-eval/docs/`;
- compare `branches.json` dispositions against successor-selection records;
- report missing final History score rows explicitly;
- inspect parent stderr for successor failures before blaming provider,
  protocol, admissibility, or child-observe timeouts;
- treat `WorkspacePathMismatch` and missing edit-harness workspaces as
  artifact-prep evidence.

The read-only helper script is installed at
`.codex/skills/prototype1-loop-run-status/scripts/prototype1_run_summary.py`.
