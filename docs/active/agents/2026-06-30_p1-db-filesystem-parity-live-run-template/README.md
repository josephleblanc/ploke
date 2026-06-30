# 2026-06-30 P1 DB/filesystem parity live-run report template

**Status:** pre-run template / live-run worksheet.
**Campaign prepared:** `p1-gated-parent-3g1x3-p3-20260630-174316`
**Parent worktree:** `/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316`
**Owner eval DB:** `/home/brasides/.ploke-eval/campaigns/p1-gated-parent-3g1x3-p3-20260630-174316/prototype1/eval-store.cozo.sqlite`
**Related prior report:** [`../2026-06-27_p1-runnerio-db-db-only-trajectory-qna.md`](../2026-06-27_p1-runnerio-db-db-only-trajectory-qna.md)

## Purpose

Use this worksheet while driving the live run with `ploke-eval loop walk`. The goal is to identify gaps between:

- facts visible from normalized owner-DB relations via `walk db_query`;
- facts visible only from filesystem artifacts/logs;
- facts visible from both but with different fidelity; and
- facts not persisted well enough by either path.

Keep DB-only and file-only claims separate. DB proof must use:

```bash
/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval \
  loop walk db_query \
  --repo-root /home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316 \
  --script '<cozo query>'
```

Do not directly read the SQLite file. `walk db_query` is the read-only DB proof surface.

## Run facts to fill before first `walk start`

| Fact | Value | Evidence |
|---|---|---|
| Campaign id | `p1-gated-parent-3g1x3-p3-20260630-174316` | setup output |
| Parent node id | `node-ec383aa38762a3d4` | `scheduler.json`, `node.json` |
| Parent branch | `prototype1-parent-p1-gated-parent-3g1x3-p3-20260630-174316-gen0` | setup output |
| Source commit before setup | `a8b21da4b Gate parent benchmark tool loop` | `git log` in worktree |
| Setup commit | `f906d59a8 prototype1: initializing gen 0 parent node-ec383aa38762a3d4` | `git log` in worktree |
| Binary path used for operator commands | `/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316/target/debug/ploke-eval` | local build |
| Binary sha256 | `51cd1a0696bd16024452360bff0f190a08333616711b8ae50365c73a83d09b13` | `sha256sum` |
| Profile sha256 | `efb89520ef9a50d4fe52d4f2923bbc9d02ed6f1bac017367b5656e57677f4ae4` | `run-profile.commitment.json` |
| Storage backend | `dual-strict` | `run-profile.toml` |
| Search policy | `generations<=3`, `children=1..=3`, `parallel_targets=3`, `parallel_cap=3`, `explore_from_rejected=true` | `run-profile.toml`, `scheduler.json` |
| Oracle policy | `record-only`, `require_evidence=true` (inert unless `relative-score`) | `run-profile.toml`; see run-profile docs |

## Step ledger

Append a row after each granular `walk` step. Prefer bounded shell `timeout`; avoid long blocking `walk step --until ... --watch` unless explicitly chosen.

| Step # | Command | Started | Ended | `walk show` phase before | `walk show` phase after | DB relation count deltas | Files/logs created/changed | Outcome / blocker |
|---:|---|---|---|---|---|---|---|---|
| 0 | setup only; no walk started | 2026-06-30 | 2026-06-30 | n/a | planned root node | DB file exists; counts TBD | setup files only | ready for review |
| 1 |  |  |  |  |  |  |  |  |

## Answerability matrix template

Legend for `DB answer` / `FS answer`: **Yes**, **Partial**, **No**, **Not tested yet**.

| ID | Question / fact | DB answer | FS answer | DB relation(s) / query target | File artifact(s) / command target | First phase expected | Verified step | Gap / notes |
|---:|---|---:|---:|---|---|---|---|---|
| Q01 | Campaign, parent node, generation, branch, worktree, configured binary path |  |  |  |  |  |  |  |
| Q02 | Exact git commit / artifact coordinate |  |  |  |  |  |  |  |
| Q03 | Exact authority-bearing binary digest |  |  |  |  |  |  |  |
| Q04 | Reconstructed phase before each operator step |  |  |  |  |  |  |  |
| Q05 | Which R transitions completed |  |  |  |  |  |  |  |
| Q06 | Failed transition and whether durable evidence was written first |  |  |  |  |  |  |  |
| Q07 | Operator guard vs real live-edge failure |  |  |  |  |  |  |  |
| Q08 | Parent identity source: active checkout vs successor invocation |  |  |  |  |  |  |  |
| Q09 | Parent identity / scheduler / branch / generation agreement |  |  |  |  |  |  |  |
| Q10 | Genesis vs predecessor startup |  |  |  |  |  |  |  |
| Q11 | Startup validation path: History predecessor vs genesis absence |  |  |  |  |  |  |  |
| Q12 | Parent-start evidence recorded once |  |  |  |  |  |  |  |
| Q13 | Admitted run profile identity |  |  |  |  |  |  |  |
| Q14 | Search policy: max generations / max nodes / child min-max / schedule |  |  |  |  |  |  |  |
| Q15 | Broad-TUI retries / timeout |  |  |  |  |  |  |  |
| Q16 | Why `child_budget.min = 1` |  |  |  |  |  |  |  |
| Q17 | Generation source: deterministic vs broad-harness |  |  |  |  |  |  |  |
| Q18 | Deterministic no-op planner disabled globally |  |  |  |  |  |  |  |
| Q19 | Whether child-plan message existed before the step |  |  |  |  |  |  |  |
| Q20 | Child-plan binding to parent and expected child generation |  |  |  |  |  |  |  |
| Q21 | Published request id / slot |  |  |  |  |  |  |  |
| Q22 | Prompt/model/provider/route used |  |  |  |  |  |  |  |
| Q23 | Headless TUI started |  |  |  |  |  |  |  |
| Q24 | Provider call happened |  |  |  |  |  |  |  |
| Q25 | Tools executed |  |  |  |  |  |  |  |
| Q26 | Timeout vs crash vs submitted result |  |  |  |  |  |  |  |
| Q27 | Candidate workspace path |  |  |  |  |  |  |  |
| Q28 | Changed paths / diff |  |  |  |  |  |  |  |
| Q29 | Whether rejection was empty/protected/malformed/unbound |  |  |  |  |  |  |  |
| Q30 | Rejected child-plan persisted before below-minimum error |  |  |  |  |  |  |  |
| Q31 | Rejected attempt reason and producer id |  |  |  |  |  |  |  |
| Q32 | Number of candidate attempts |  |  |  |  |  |  |  |
| Q33 | Attempt target/workspace/candidate/branch/status |  |  |  |  |  |  |  |
| Q34 | Why each attempt was rejected |  |  |  |  |  |  |  |
| Q35 | Protected-write denial |  |  |  |  |  |  |  |
| Q36 | “No admitted changes” semantics for this run |  |  |  |  |  |  |  |
| Q37 | Last tool/model event before timeout |  |  |  |  |  |  |  |
| Q38 | Child-plan DB rows agree with message |  |  |  |  |  |  |  |
| Q39 | Scheduler/node status root rows |  |  |  |  |  |  |  |
| Q40 | Generation-1 child rows absent because no child admitted |  |  |  |  |  |  |  |
| Q41 | Root runner request |  |  |  |  |  |  |  |
| Q42 | Runner result absent due no child spawn |  |  |  |  |  |  |  |
| Q43 | Invalid schema rows / current schema |  |  |  |  |  |  |  |
| Q44 | Owner DB fresh/current-schema |  |  |  |  |  |  |  |
| Q45 | Claims relying only on `eval_record_ref` |  |  |  |  |  |  |  |
| Q46 | Agent turn exists |  |  |  |  |  |  |  |
| Q47 | Tool-event rows / persisted tool events |  |  |  |  |  |  |  |
| Q48 | Model/message/trace event absence |  |  |  |  |  |  |  |
| Q49 | Why event/model/tool trace rows are absent |  |  |  |  |  |  |  |
| Q50 | Logs/checkpoints for rejected attempt |  |  |  |  |  |  |  |
| Q51 | Elapsed request-to-timeout timeline |  |  |  |  |  |  |  |
| Q52 | No R8 selectable state with runnable children |  |  |  |  |  |  |  |
| Q53 | No R9 schedule / R10 selection / R11 fanout |  |  |  |  |  |  |  |
| Q54 | No materialize/build/spawn/observe |  |  |  |  |  |  |  |
| Q55 | No child invocation/channel/result |  |  |  |  |  |  |  |
| Q56 | No runner-result proof |  |  |  |  |  |  |  |
| Q57 | No selection/handoff/History seal |  |  |  |  |  |  |  |

## DB query snippets to use during the run

### Relation inventory/counts

Use targeted count scripts. Keep each script in `queries/` or paste below with timestamp and command output.

Pattern:

```cozo
?[count(node_id)] := *eval_scheduler_node{node_id}
```

For keyed relations with composite keys, count one required key field.

### High-signal count checklist

Run count queries for these relation groups after each major phase:

- setup/profile/closure: `eval_campaign`, `eval_profile_commitment`, `eval_run_profile_policy`, `eval_closure_ref`, `eval_closure_instance`, `eval_closure_artifact_ref`, `eval_closure_protocol_procedure`, `eval_closure_protocol_counts`
- parent/start/baseline: `eval_parent_identity`, `eval_parent_start`, `eval_transition_event`, `eval_baseline`, `eval_baseline_instance`, `eval_baseline_instance_metrics`
- scheduler/runner: `eval_scheduler_node`, `eval_scheduler_node_status_event`, `eval_runner_request`, `eval_runner_request_arg`, `eval_runner_result`
- broad harness and agent turn: `eval_harness_request`, `eval_harness_diagnostic`, `eval_harness_workspace`, `eval_harness_workspace_change`, `eval_harness_submission*`, `eval_agent_turn`, `eval_model_exchange`, `eval_tool_event`, `eval_message_event`, `eval_trace_event`
- child plan: `eval_child_plan`, `eval_child_plan_child`, `eval_child_plan_rejected_attempt`
- materialize/build/spawn/observe: `eval_artifact*`, `eval_operation`, `eval_patch`, `eval_apply_event`, `eval_build_event`, `eval_binary_ref`, `eval_invocation`, `eval_channel_message`, `eval_channel_receipt`, `eval_import_event`, `eval_log_ref`, `eval_runner_result`
- compare/selection/handoff: `eval_evaluation`, `eval_evaluation_instance`, `eval_selection_decision`, `eval_selection_candidate`, `eval_selection_finding`, `eval_selection_score`, `eval_continuation_decision`
- operator walk: `eval_walk_event`, `eval_walk_event_transition`

## Findings summary to fill after run

| Gap id | Phase | Missing from DB | Present in files? | Relation candidate / existing relation | Authority weakened if ignored? | Proposed next action |
|---:|---|---|---|---|---|---|
| G01 |  |  |  |  |  |  |

## Notes

- Agent-turn DB rows are currently post-attempt bundle writes, not mid-turn telemetry.
- `eval_record_ref` is provenance only; do not use it as proof where normalized rows are expected.
- Policy/search/profile facts should come from `eval_run_profile_policy` / admitted profile rows where available, not stale scheduler projection alone.
- Treat History, MessageBox, Channel, artifact/worktree mutation, and bootstrap files as separate authority domains. DB rows may mirror or cite them but do not replace their authority unless a dedicated backend is implemented.
