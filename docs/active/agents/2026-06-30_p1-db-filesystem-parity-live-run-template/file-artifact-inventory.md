# Live-loop file artifact inventory and DB mirror map

**Status:** pre-run worksheet / full-inventory starter.
**Campaign:** `p1-gated-parent-3g1x3-p3-20260630-174316`
**Parent worktree:** `/home/brasides/.ploke-eval/worktrees/p1-gated-parent-3g1x3-p3-20260630-174316`

## Reading this inventory

- Paths are patterns unless the prepared campaign path is shown explicitly.
- “DB mirror relation(s)” names normalized owner-DB relations when implemented.
- “Macro?” means the DB relation is defined through `define_eval_schema!` in `crates/ploke-eval/src/cli/prototype1_state/eval_store/`. For current eval-store relations this is **yes**.
- A DB mirror does **not** replace the authority of History, MessageBox, Channel, bootstrap invocation, artifact/worktree mutation, or git checkout validation.

## Campaign/root configuration and setup files

| File / path pattern | Written item(s) | Written by / phase | Later gate or consumer | DB mirror relation(s) | Macro? | Run verification notes |
|---|---|---|---|---|---|---|
| `/home/brasides/.ploke-eval/campaigns/<campaign>/campaign.json` | `CampaignManifest`: campaign id, dataset sources, model/route, instance roots, budgets. | `prototype1-setup`; mirrored again in `R0->R1`. | Campaign resolution, run shape, db path derivation, protocol/eval closure. | `eval_campaign`, `eval_campaign_eval_policy`, `eval_campaign_eval_budget`, `eval_campaign_protocol_policy`. | yes | Verify row count and manifest hash/path. |
| `/home/brasides/.ploke-eval/campaigns/<campaign>/closure-state.json` | `ClosureState`: baseline eval/protocol closure and instance refs. | setup/R0; may be generated/refreshed during baseline. | `R5->R6` baseline establishment. | `eval_closure_ref`, `eval_closure_instance`, `eval_closure_artifact_ref`, `eval_closure_protocol_procedure`, `eval_closure_protocol_counts`. | yes | Compare closure rows vs closure file. |
| `/home/brasides/.ploke-eval/campaigns/<campaign>/slice.jsonl` | Dataset slice row(s), target benchmark instance input. | setup. | Baseline/treatment task selection. | none direct currently; campaign dataset_sources only. | n/a | File-only unless campaign row suffices. |
| `/home/brasides/.ploke-eval/campaigns/<campaign>/prototype1/eval-store.cozo.sqlite` | Owner eval DB backing normalized relations. | setup/schema installation and all DB writes. | Operator `walk db_query`; parity proof. | n/a (database file contains relations). | n/a | Do not inspect directly; use `walk db_query`. |
| `/home/brasides/.ploke-eval/campaigns/<campaign>/prototype1/run-profile.toml` | Admitted run profile: storage, target, model, search, generation, selection, oracle, execution, control. | setup. | R0/R6/R7/R9/R12 policy decisions. | `eval_profile_commitment`, `eval_run_profile_policy`. | yes | Critical: prove search/oracle/control policy in DB, not only file. |
| `/home/brasides/.ploke-eval/campaigns/<campaign>/prototype1/run-profile.commitment.json` | Profile digest/source/admission record. | setup. | Profile identity/provenance. | `eval_profile_commitment`. | yes | Verify sha256 matches file and source profile. |
| `/home/brasides/.ploke-eval/profiles/prototype1/<campaign>.toml` | Source profile copied/admitted into campaign. | operator/setup. | Provenance only after admission. | `eval_profile_commitment.source_path`; `eval_run_profile_policy` fields. | yes | Source file not authority after commitment except as provenance. |
| `/home/brasides/.ploke-eval/prototype1-monitor-target.json` | Active monitor target pointer. | `R0->R1` via `record_active_prototype1_monitor_target`. | Operator monitor/watch convenience. | none currently. | n/a | Projection only. |
| `/home/brasides/.ploke-eval/last-run.json` | Convenience pointer to latest/selected run. | Mixed operator/eval paths. | Operator convenience. | none. | n/a | Not correctness evidence. |

## Active parent checkout / git artifact files

| File / path pattern | Written item(s) | Written by / phase | Later gate or consumer | DB mirror relation(s) | Macro? | Run verification notes |
|---|---|---|---|---|---|---|
| `<repo-root>/.ploke/prototype1/parent_identity.json` | `ParentIdentity`: campaign, parent/node id, generation, branch/artifact identity, predecessor refs. | setup/init; successor handoff. Committed to git branch. | R1/R3/R4 startup, checkout validation, successor startup. | `eval_parent_identity` when parent-start evidence is written; not a replacement for checkout identity. | yes | This is artifact authority; DB mirror is query evidence only. |
| Git branch/ref metadata in worktree/common dir | Parent branch, child artifact branches, selected successor branch, commits. | setup, materialize, broad harness, handoff. | Checkout validation, artifact install, History seal. | Partial: `eval_artifact.git_branch`, `eval_artifact.git_commit` when populated; often path/hash refs only. | yes | DB may lack exact git commit for some authority-bearing steps. |
| `<repo-root>/target/debug/ploke-eval` | Runtime binary used for parent/walk commands. | local `cargo build -p ploke-eval`. | Operator/liveness authority; child/successor may build own binaries. | `eval_binary_ref` only for child build/spawn mirrors, not initial parent operator binary unless explicitly mirrored. | yes | Current gap: authority-bearing parent binary digest not normalized by setup. |
| Source files under active checkout | Parent code; selected successor artifact after handoff. | normally read-only until handoff; handoff mutates checkout. | Checkout validation, successor startup. | `eval_artifact*` for selected active checkout artifact on handoff. | yes | Do not infer from DB alone. |

## Prototype1 projections, History, and journals

| File / path pattern | Written item(s) | Written by / phase | Later gate or consumer | DB mirror relation(s) | Macro? | Run verification notes |
|---|---|---|---|---|---|---|
| `prototype1/scheduler.json` | `Prototype1SchedulerState`: policy projection, frontier/completed/failed node ids, embedded node records. | setup and scheduler updates. | Operator status/reconstruction; some fallback helpers. | `eval_scheduler_node`, `eval_scheduler_node_status_event`, `eval_scheduler_node_target_part` for node projections; policy mostly in `eval_run_profile_policy`. | yes | Watch for scheduler-vs-node drift. |
| `prototype1/nodes/<node>/node.json` | `Prototype1NodeRecord`: node id, generation, status, branch, target, workspace, binary, paths. | setup; R7 parent status; C1-C4 status updates; terminal updates. | Reconstruction, fanout, build/spawn/report. | `eval_scheduler_node`, `eval_scheduler_node_status_event`, `eval_scheduler_node_target_part`; sometimes `eval_record_ref`. | yes | Projection, not sole authority. |
| `prototype1/transition-journal.jsonl` | `JournalEntry`: parent-start/resource, materialize/build/spawn/observe/successor events. | R4c onward; child transitions; handoff. | `walk replay`, reconstruction, successor/handoff diagnostics. | `eval_transition_event` for parent-start; `eval_record_ref`; some later events are not fully normalized yet. | yes/partial | JSONL remains important; DB parity gaps expected. |
| `prototype1/history/blocks/segment-*.jsonl` | Sealed History blocks and entries. | R12 handoff via `FsBlockStore::append`. | Successor startup validation, lineage authority. | No first-class eval-store replacement; may be cited by refs later. | n/a | History authority must stay separate. |
| `prototype1/history/index/by-hash.jsonl` | Rebuildable History index. | History append. | History lookup/replay. | none direct. | n/a | Projection. |
| `prototype1/history/index/by-lineage-height.jsonl` | Rebuildable lineage-height index. | History append. | History lookup/replay. | none direct. | n/a | Projection. |
| `prototype1/history/index/heads.json` | History head projection. | History append. | Startup/handoff validation reads History state. | none direct. | n/a | Projection over sealed blocks. |
| `prototype1/branches.json` | Branch registry records/comparison/selection info. | child generation/compare/selection. | Evaluation/selection/reconstruction. | Partial via `eval_evaluation*`, `eval_artifact*`, `eval_operation*`, `eval_selection*`; no full branch-registry relation. | yes/partial | File-only fields likely remain. |
| `prototype1/evaluations/<branch-id>.json` | `Prototype1BranchEvaluationReport`: baseline/treatment comparison, dispositions, MBE/oracle evidence when available. | C4/C5 compare. | Selection, report, next generation baseline promotion. | `eval_evaluation`, `eval_evaluation_instance`. | yes | Critical for DB/file parity. |
| `prototype1/prototype1-state-report*.json` or path from `prototype1_state_report_path` | `Prototype1StateReport`: final outcome, node status, workspace/binary/runtime ids, successor info. | R13->R14. | Operator final summary, completion review. | No dedicated normalized final-report relation yet; possible `eval_record_ref`/trace/log only. | partial | Gap candidate. |

## Child-plan and broad-harness files

| File / path pattern | Written item(s) | Written by / phase | Later gate or consumer | DB mirror relation(s) | Macro? | Run verification notes |
|---|---|---|---|---|---|---|
| `prototype1/messages/child-plan/<parent-node-id>.json` | Child-plan MessageBox payload: parent id, child generation, planned children, rejected attempts. | R7->R8 lock/write/unlock. | R8 scheduling, R10 fanout, C1 child validation. | `eval_child_plan`, `eval_child_plan_child`, `eval_child_plan_rejected_attempt`. | yes | DB mirrors payload after file exists; MessageBox authority remains file/typed lock-unlock. |
| `prototype1/messages/edit-harness-request/<node[-rN]>.json` | `PublishedBroadHarnessRequest`: request id/hash, parent, workspace, child budget, graph restriction, edit/admission policy. | R7->R8 broad-harness request publication. | Headless TUI attempt, result binding, diagnostics. | `eval_harness_request`. | yes | Must exist before model attempt. |
| `prototype1/messages/edit-harness-request/<node[-rN]>.md` | Rendered broad-harness prompt text. | R7->R8 request publication. | LLM prompt audit/replay. | `eval_harness_request.prompt_path` + prompt sha; agent-turn prompt refs/summary. | yes/partial | Prompt content is still file. |
| `prototype1/messages/pre-child-planning/*.json` | Pre-child planning artifact/request metadata. | R7->R8 broad planning. | Prompt/model diagnostics; harness request construction. | Partial via harness/agent-turn rows; no dedicated pre-child-planning relation. | partial | Inventory during run. |
| `prototype1/messages/pre-child-planning/*.prompt.md` | Pre-child planning prompt text. | R7->R8. | Prompt audit. | Partial via path/hash refs if captured; likely file-only. | partial |  |
| `prototype1/messages/edit-harness-result/<node[-rN]>.json` | Submitted broad-harness result: candidate workspace, return evidence, changed files, citations, checks. | Headless TUI if model submits/admitted. | Child-plan admission, child node/request generation. | `eval_harness_submission`, `eval_harness_submission_change`, `eval_harness_submission_citation`, `eval_harness_submission_check`. | yes | Only present on submission, not timeout. |
| `prototype1/messages/edit-harness-result/<node[-rN]>.headless-tui.json` | Headless run diagnostics/evidence: terminal, attempts, events, validations, prompt diagnostics, tool counts. | R7->R8 broad attempt completion/timeout. | Rejected-attempt reason, debugging, workspace change capture. | `eval_harness_diagnostic`, `eval_harness_workspace`, `eval_harness_workspace_change`. | yes | Critical timeout/rejection evidence. |
| `prototype1/messages/edit-harness-result/<node[-rN]>.turn-live/agent-turn-trace.json` | Agent turn trace record: post-attempt bundle of messages/tool/model trace. | R7->R8 after headless run returns/terminates. | LLM replay/debug; not mid-turn telemetry. | `eval_agent_turn`, `eval_agent_turn_event`, `eval_model_exchange`, `eval_message_event`, `eval_tool_event`. | yes | Row only appears after bundle write. |
| `prototype1/messages/edit-harness-result/<node[-rN]>.turn-live/agent-turn-summary.json` | Agent turn summary record. | R7->R8 post-attempt bundle. | Run review/debug. | `eval_agent_turn.summary_ref`/sha; event rows. | yes |  |
| `prototype1/messages/edit-harness-result/<node[-rN]>.turn-live/llm-full-responses.jsonl` | Full provider response JSONL records. | R7->R8 post-attempt bundle. | Provider/debug replay. | `eval_agent_turn.full_response_ref`/sha; `eval_model_exchange` rows when parsed from bundle. | yes | If interrupted mid-turn, file may exist without DB rows. |
| `prototype1/debug/tool-loop/<session-id>/...` | Tool-loop checkpoint transcript, prompts, tool args/results, protocol hints. | `walk llm` debugger and headless tool loop paths. | Read-only LLM lane commands; debug branching. | Partial via `eval_agent_turn*`, `eval_tool_event`, `eval_model_exchange`; no complete checkpoint relation. | partial | Inventory if debug lane created. |
| `prototype1/workspaces/edit-harness/<parent-node-id>[-rN]/` | Candidate workspace files edited by headless TUI. | R7->R8 broad harness. | Submission/admission, workspace diff, child artifact materialization. | `eval_harness_workspace`, `eval_harness_workspace_change`; submitted change rows if admitted. | yes | Artifact authority remains git/workspace. |

## Child/successor node runtime files

| File / path pattern | Written item(s) | Written by / phase | Later gate or consumer | DB mirror relation(s) | Macro? | Run verification notes |
|---|---|---|---|---|---|---|
| `prototype1/nodes/<node>/runner-request.json` | `Prototype1RunnerRequest`: node, generation, target, workspace root, binary path, runner args. | setup root; R7 child planning; C1 workspace update. | Child invocation/build/spawn/reconstruction. | `eval_runner_request`, `eval_runner_request_arg`, `eval_runner_request_target_part`, `eval_record_ref`. | yes | For root node, binary path may be declared but node-local binary not materialized yet. |
| `prototype1/nodes/<node>/runner-result.json` | Latest `Prototype1RunnerResult`: status/disposition, treatment/evaluation refs, excerpts. | child terminal/failure paths. | Report/reconstruction/operator display. | `eval_runner_result`, `eval_record_ref`. | yes | Latest projection; attempt-specific result is stronger. |
| `prototype1/nodes/<node>/results/<runtime-id>.json` | Attempt-scoped runtime result mirror. | child process terminal result. | Parent observe/compare/reconstruction. | `eval_runner_result` if emitted through runner result writer; channel rows for terminal message. | yes | Channel is parent visibility authority. |
| `prototype1/nodes/<node>/invocations/<runtime-id>.json` | `Invocation` / child or successor bootstrap descriptor: role, runtime id, journal path, channel root, request/resolved branch, active parent root for successor. | C3 child spawn; R12 handoff successor spawn. | Child/successor process startup. | `eval_invocation`, `eval_attempt`. | yes | Bootstrap authority remains file/launch package. |
| `prototype1/nodes/<node>/channels/<runtime-id>/parent-to-child.jsonl` | Parent-to-child channel messages. | spawn/runtime communication. | Child runtime transport. | `eval_channel_message`, `eval_channel_receipt`, `eval_import_event` when mirrored. | yes | Transport authority remains channel. |
| `prototype1/nodes/<node>/channels/<runtime-id>/child-to-parent.jsonl` | Child/successor-to-parent channel messages: ready, evaluating, result, successor-ready/completion. | child/successor runtime. | Parent observe, predecessor handoff wait, successor completion. | `eval_channel_message`, `eval_channel_receipt`, `eval_import_event`. | yes | This is a key authority boundary. |
| `prototype1/nodes/<node>/streams/<runtime-id>/stdout.log` | Runtime stdout stream. | C3 spawn or successor spawn. | Debug; failure diagnosis. | `eval_log_ref` with `runtime_stdout` when mirror call runs. | yes | Content hash may be absent initially. |
| `prototype1/nodes/<node>/streams/<runtime-id>/stderr.log` | Runtime stderr stream. | C3 spawn or successor spawn. | Debug; failure diagnosis. | `eval_log_ref` with `runtime_stderr` when mirror call runs. | yes | Content hash may be absent initially. |
| `prototype1/nodes/<node>/bin/ploke-eval` | Built child runtime binary copied/promoted. | C2 build/promote. | C3 spawn. | `eval_binary_ref`, `eval_build_event`. | yes | Parent/root setup does not materialize this path initially. |
| `prototype1/nodes/<node>/target/` | Child build output/scratch. | C2 build. | Build/promote diagnostics only. | `eval_build_event`; no per-file mirror. | yes/partial | Build artifacts are not durable identity. |
| `prototype1/nodes/<node>/worktree/` | Direct child materialized workspace/artifact content. | C1 materialize. | C2 build, artifact comparison/handoff if selected. | `eval_artifact`, `eval_artifact_surface`, `eval_artifact_ref`, `eval_operation`, `eval_patch`, `eval_apply_event`. | yes | Artifact/worktree remains authority. |
| `prototype1/nodes/<node>/successor-ready/<runtime-id>.json` | Successor-ready acknowledgement record if path-based ready file is used. | Successor startup/predecessor wait. | R12 handoff acknowledgement. | Channel rows / `eval_invocation` / `eval_attempt` related; no dedicated ready relation. | partial | Verify actual path existence in handoff. |
| `prototype1/nodes/<node>/successor-completion/<runtime-id>.json` | Successor completion record. | R13/R14 successor completion. | Operator report/predecessor completion. | Channel/import rows if mirrored; no dedicated completion relation. | partial | Gap candidate. |

## Instance, treatment, batch, and benchmark output files

| File / path pattern | Written item(s) | Written by / phase | Later gate or consumer | DB mirror relation(s) | Macro? | Run verification notes |
|---|---|---|---|---|---|---|
| `/home/brasides/.ploke-eval/instances/prototype1/<campaign>/<instance>/runs/<run-id>/run.json` | Eval run record/config/result metadata. | baseline/treatment eval runner. | Baseline closure, evaluation report, benchmark review. | `eval_baseline_instance`, `eval_evaluation_instance` refs; not a full run relation currently. | yes/partial | Often file-only for detailed run state. |
| `.../repo-state.json` | Git/workspace state for eval run. | eval runner. | Debug/replay. | Ref/path only via baseline/evaluation rows if any. | partial |  |
| `.../execution-log.json` | Eval execution log. | eval runner. | Debug/failure diagnosis. | `eval_log_ref` only if explicitly mirrored/imported. | yes/partial |  |
| `.../indexing-status.json` | Indexing status for treatment/baseline run. | eval runner/TUI. | Debug readiness/freshness. | none dedicated. | n/a | Likely file-only. |
| `.../snapshot-status.json` | Snapshot status for DB snapshots. | eval runner. | Debug/recovery. | none dedicated; path refs possible in baseline/evaluation. | partial |  |
| `.../multi-swe-bench-submission.jsonl` | Benchmark submission output. | eval runner/batch export. | External evaluator, campaign proof. | `eval_baseline_instance_metrics`/`eval_evaluation_instance` summaries; no full submission relation. | partial | File remains primary. |
| `.../record.json.gz` | Compressed run record. | eval runner. | Detailed run playback/debug. | Path refs in baseline/evaluation; no normalized full record. | partial |  |
| `.../indexing-checkpoint.db`, `.../final-snapshot.db` | Treatment/baseline local code graph DB snapshots. | eval runner / TUI runtime. | Debug/reproduction; not parent-owned by default. | none in owner eval DB except refs if present. | n/a | Child/treatment-local DB; do not import silently. |
| `/home/brasides/.ploke-eval/batches/<batch>/batch.json` | Batch manifest. | setup/eval runner. | Batch aggregation. | campaign/baseline rows only. | partial |  |
| `/home/brasides/.ploke-eval/batches/<batch>/batch-run-summary.json` | Batch summary. | eval runner. | Reporting. | none dedicated. | n/a |  |
| `/home/brasides/.ploke-eval/batches/<batch>/multi-swe-bench-submission.jsonl` | Batch submission aggregate. | eval runner/export. | External benchmark. | none dedicated. | n/a |  |
| `/home/brasides/.ploke-eval/registries/runs/<run-id>.json` | Run registry record. | eval runner. | Run lookup/recovery. | none dedicated; run refs in baseline/evaluation. | partial |  |
| `/home/brasides/.ploke-eval/cache/starting-dbs/<key>.sqlite` and `.json` | Starting DB cache/snapshot metadata. | eval runner/index setup. | Performance/repro cache. | none. | n/a | Cache, not authority. |

## Logs, traces, and operator/debug files

| File / path pattern | Written item(s) | Written by / phase | Later gate or consumer | DB mirror relation(s) | Macro? | Run verification notes |
|---|---|---|---|---|---|---|
| `/home/brasides/.ploke-eval/logs/ploke_eval_<timestamp>.log` | Tracing subscriber log for ploke-eval process. | Any CLI/run process with logging configured. | Debugging only unless cited. | `eval_log_ref` only if explicitly mirrored/imported; otherwise none. | yes/partial | Use `rg`, not sqlite. |
| `/home/brasides/.ploke-eval/logs/llm_full_response_<timestamp>.log` | Full LLM response JSONL target when configured. | LLM/provider manager. | Provider debugging. | `eval_log_ref` or agent-turn rows only if imported/bundled; not automatic for global log. | partial |  |
| `/home/brasides/.ploke-eval/logs/prototype1_observation_<timestamp>.jsonl` or `PLOKE_PROTOTYPE1_TRACE_JSONL` target | Structured observation JSONL. | observation/tracing path when enabled. | Debug/replay. | `eval_log_ref`, `eval_trace_event` when imported or trace sink active. | yes | For this run, verify whether trace sink writes DB rows directly. |
| `PLOKE_PROTOTYPE1_TRACE_JSONL` override path | Optional operator-chosen trace output. | environment-configured tracing. | Debug/replay. | `eval_trace_event` only if imported/mirrored. | yes/partial | May be outside eval home. |
| Walk socket/context files under `$XDG_RUNTIME_DIR/ploke-eval/walk` or `/tmp/ploke-eval-$USER/walk` | Walk server socket, context, selected repo/campaign state. | `walk start/use/stop`. | `walk` client/server IPC. | `eval_walk_event`, `eval_walk_event_transition` for walk event DB rows; socket files not mirrored. | yes/partial | Runtime IPC, not campaign evidence. |
| Operator-created shell logs/PID files | Redirected output, pid captures, timeout wrappers. | operator shell. | Human operations/debug. | none unless explicitly imported/refed. | n/a | Do not treat as ploke-eval evidence unless cited. |
| `/home/brasides/.ploke-eval/operator-logs/...` | Human/operator notes. | operator/agent. | Review context. | none. | n/a | Not run authority. |
| `/home/brasides/.ploke-eval/records/mirror.cozo.sqlite` | Passive record mirror compatibility DB. | older/passive record emission. | Debug/backfill only. | separate from owner eval DB; not proof of normalized parity. | n/a | Keep separate from `prototype1/eval-store.cozo.sqlite`. |

## File-to-DB gap checklist for live observations

Fill this as artifacts appear.

| Step | File observed | Item inside file | Later phase gated on it? | DB relation checked | Row present? | Normalized or ref-only? | Gap note |
|---:|---|---|---|---|---|---|---|
|  |  |  |  |  |  |  |  |

## Known pre-run gap hypotheses

1. Parent/root operator binary digest is file-only unless a build/binary relation is explicitly written for the parent binary.
2. Search policy is normalized in `eval_run_profile_policy`, but live policy decisions still read admitted profile/files, not DB-only rows.
3. History blocks and channel/message-box files remain authority surfaces; DB rows are mirrors/refs.
4. Agent-turn rows are post-attempt bundle writes, so interrupted mid-turn provider/tool progress can be file/log-only.
5. Some global logs and treatment-run artifacts have path refs but no normalized content rows.
6. Final parent report/completion records appear to lack a dedicated normalized relation beyond refs/traces/channel mirrors.
