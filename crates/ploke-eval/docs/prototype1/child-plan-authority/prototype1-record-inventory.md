# Prototype 1 record inventory

This table is sourced from:

- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/record-surface-map.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/crate-boundary-inventory.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/playback-coverage-pass.md`
- `docs/workflow/evalnomicon/drafts/observability/runtime-playback/inventory/latest-run-emission-worksheet.md`

Authority order to preserve:

1. sealed History lineage order;
2. admitted History entry order;
3. typed transition, channel, protocol, invocation, and evaluation evidence;
4. runtime-local and attempt-local order;
5. agent-turn event order;
6. timestamps for profiling/correlation only.

| Step / family | Source owner | Reads | Writes / emits | Evidence class | Join keys | Playback status |
| --- | --- | --- | --- | --- | --- | --- |
| CLI dispatch | `crates/ploke-eval/src/cli.rs` | CLI args | none | render/control only | command variant | not a record source |
| run_turn context | `prototype1_state/cli_facing.rs`, `profile.rs`, `journal.rs` | `campaign.json`, `closure-state.json`, `prototype1/run-profile.toml`, active checkout | monitor target, transition journal handle | setup/context | campaign_id, repo_root | context for parent turn |
| gen0 identity | `cli_facing.rs`, `identity.rs`, `parent.rs`, backend git helpers | campaign manifest, selected node | active checkout `.ploke/prototype1/parent_identity.json`, identity commit, node projection | artifact-carried identity witness | campaign_id, parent_id, node_id, generation, branch_id | loaded as parent identity evidence |
| parent identity resolve | `cli_facing.rs`, `identity.rs`, `invocation.rs` | `parent_identity.json` or `nodes/<node>/invocations/<runtime>.json` | none | admission input | parent_id, runtime_id, node_id | graph parent identity / runtime evidence |
| startup admission | `parent.rs`, `history.rs`, `backend.rs`, `prototype1_process.rs` | History blocks, lineage head projection, active checkout tree/surface, handoff invocation | `Parent<Ready>` state | authority gate | lineage_id, block_hash, artifact/tree key, parent_id | History/authority graph |
| ParentStarted | `cli_facing.rs`, `journal.rs` | parent identity, pid, repo root | `transition-journal.jsonl` ParentStarted and resource samples | typed transition evidence | campaign_id, parent_id, runtime_id, node_id | transition journal step |
| baseline evidence | `cli_facing.rs`, closure/loaders, runner/eval records | `closure-state.json`, baseline run roots, `record.json.gz`, protocol/eval artifacts, run registry refs | baseline comparison inputs | evaluation evidence | campaign_id, instance_id, run_id, record_key | compared-run/evaluation evidence |
| child plan | `cli_facing.rs`, `parent.rs`, `edit_surface/**`, `intervention/**` | run profile, scheduler/node inventory, branches, target surface | `messages/child-plan/<parent-node-id>.json`, node records, runner requests, branch registry updates | typed parent-owned message box plus projections | parent_node_id, child node_id, branch_id, candidate_id | `Graph.child_plans`; scheduler/branches are context |
| schedule policy | `cli_facing.rs`, `profile.rs` | run profile, persisted node count | child budget/schedule mode in memory | policy decision | campaign_id, max_generations, max_total_nodes | should be playback frame/policy context |
| C1 child files | `c1.rs`, `parent.rs` | child-plan entry, node.json, runner-request, resolved branch | C1 typestate | attempt input | node_id, branch_id, source_state_id | operation/attempt evidence |
| C1 -> C2 materialize | `c1.rs`, `backend.rs`, `prototype1_process.rs` | active checkout, resolved branch, target content | `nodes/<node>/worktree/`, node status, runner request projection, ChildArtifactCommitted entry | typed transition evidence / artifact candidate | node_id, branch_id, artifact/tree key, target_relpath | artifact/operation evidence |
| C2 -> C3 build | `c2.rs` | child worktree, node record | `nodes/<node>/target/`, `nodes/<node>/bin/`, build/check journal entries, node status | typed transition evidence | node_id, runtime build path | runtime/attempt evidence |
| C3 -> C4 spawn | `c3.rs`, `invocation.rs`, `channel.rs` | binary path, node record | `invocations/<runtime>.json`, channel root, stream logs, spawn/ready journal entries | runtime invocation evidence | runtime_id, node_id, channel cursor/message_id | invocation graph; live channel carries Ready/Evaluating/Result |
| leaf child runtime | `cli.rs` runner command, `prototype1_process.rs`, `runner.rs`, `record.rs`, `ploke-tui`, `ploke-llm` | invocation, run setup, target workspace | run root: `execution-log.json`, `repo-state.json`, `record.json.gz`, `agent-turn-*`, `llm-full-responses.jsonl`, index/snapshot DB witnesses, protocol/benchmark artifacts | runtime-local and agent-turn evidence | run_id, instance_id, task_id, assistant_message_id, response_index, tool call id | run records/evidence; provider request incomplete |
| C4 -> C5 observe | `c4.rs`, `channel.rs`, `observe.rs` | child-to-parent channel, results path, runner-result | `results/<runtime>.json`, `runner-result.json`, completion journal entries | typed runtime/attempt evidence | runtime_id, node_id, branch_id | terminal channel Result is live authority; files/journal are reconstruction |
| compare child | `cli_facing.rs`, metrics/score/evaluation code | baseline/treatment run refs, treatment evidence, oracle/protocol metrics | `evaluations/<branch>.json`, `branches.json` parent-comparison records | evaluation/benchmark evidence | branch_id, instance_id, run_id, record_key | evaluation graph/evidence |
| selection input | `cli_facing.rs`, `history_preview.rs`, evidence modules, successor traversal | current child outcomes, rejected attempts, History candidates, eval/protocol/run evidence | selection material in memory | selection evidence preimage | node_id, branch_id, candidate ids, metric ids | graph candidates/selections after sealed |
| select successor | `cli_facing.rs`, `selection.rs`, `successor_selection/traversal.rs` | selection input and metric sources | `SelectionDecisionEntryRecord`, candidate set, memberships, formula, metric set, traversal evidence, projection failures | decision evidence, later admitted in History | entry_id, candidate_set_root, membership_id, occurrence_id, metric_set_id | best-covered selection graph family |
| continuation gate | `cli_facing.rs::live_successor_continuation_decision` | run profile, total node count, ParentStarted count, selected node, branch disposition | continuation decision | authority boundary before handoff | parent_node_id, selected node_id, generation, policy caps | should move into run core as typed gate |
| stopped successor | `cli_facing.rs`, `successor.rs`, `journal.rs` | continuation decision | Successor stopped record in transition journal | typed evidence of no handoff | node_id, decision/disposition | transition journal / successor evidence |
| install selected Artifact | `prototype1_process.rs`, `backend.rs`, `identity.rs` | selected child worktree/branch, active checkout | active checkout mutation, parent_identity update, ActiveCheckoutAdvanced journal entry, surface commitment | artifact transition evidence | artifact/tree key, selected branch, parent_id, node_id | artifact ancestry / transition evidence |
| seal History | `prototype1_process.rs`, `parent.rs`, `inner.rs`, `history.rs` | open lineage state, selected artifact, selection entry | `history/blocks/segment-*.jsonl`, admitted artifact claim, selection entry, operational environment | primary authority | lineage_id, block_id, block_hash, entry_id, artifact_ref | History/authority causal spine |
| successor spawn | `prototype1_process.rs`, `invocation.rs`, `channel.rs` | retired parent, binary path, active checkout | successor invocation, parent-to-child channel, streams, SuccessorRecord::spawned | runtime invocation evidence | runtime_id, node_id, invocation path | runtime/attempt evidence |
| SuccessorReady | `prototype1_process.rs`, `channel.rs`, `invocation.rs` | child-to-parent channel or ready file | SuccessorHandoff journal entry, successor-ready or timeout/exit records | handoff evidence | runtime_id, node_id, message_id | channel/transition evidence |
| successor re-entry | `cli_facing.rs`, `prototype1_process.rs`, `parent.rs`, `history.rs` | handoff invocation, active parent identity, predecessor sealed History head | successor-completion record after bounded turn | next-parent admission input | runtime_id, parent_id, lineage/block hash | loops back to parent admission |
| RuntimePlayback load path | `ploke-tree/src/store/fs.rs`, `record_set.rs`, graph builders | scheduler, node records, parent identity, successor ready/completion, passive evidence, history blocks, transition journal, agent-turn records | `RunRecordSet`, `Graph`, playback borrowed views | read-side projection, not active authority | all preserved join keys | target path for CLI/egui/replay |
| runtime channels | eval channel writer + `ploke_records::channel` passive shape | `channels/<runtime>/*.jsonl` | per-runtime JSONL envelopes; playback may load counts/passive views | lifecycle and supplementary runtime evidence | runtime_id, node_id, message_id, cursor offset | live authority for lifecycle messages; ordered playback still incomplete |
| protocol artifacts | `ploke-protocol`, `ploke-records::protocol` | protocol root artifacts | tool-call intent segmentation/review/segment-review artifacts | protocol evidence | procedure_name, subject_id, run_id, created_at_ms, segment id | summary today; needs procedure playback family |
| DB/index witnesses | `ploke-tui`, `ploke-db`, eval runner snapshots | index status, parse failure, DB snapshots, time markers | referenced paths and statuses | diagnostic witness | run root, db timestamp, snapshot path | explicit drilldown, not default graph load |
| TUI proposals | `ploke-tui` proposal persistence | redirected `config/ploke/proposals.json` when captured | proposal list | edit evidence only if ids/path recorded | proposal_id, run root, tool/edit ids | outside-run-root unless redirected |
| outside anchors | eval campaign/batch/registry/run-history | `run.json`, `registries/runs/*.json`, `last-run.json`, `campaign.json`, `closure-state.json`, `slice.jsonl`, `batch.json`, `batch-run-summary.json` | setup/projection/lifecycle records | mixed: registry authority, others context/projection/convenience | run_id, campaign_id, batch_id, instance_id | should be shown as anchors, not History authority |

## Reading note

Use this table for record and evidence accounting. It is not a diagram source of
truth, and every row should not be treated as a node in a stable operator map.
