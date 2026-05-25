# Prototype 1 Record Pipeline Map

Date: 2026-05-15

Purpose: exhaustive working map for the current Prototype 1 self-improvement
loop record path:

```text
ploke-eval writer
-> ploke-records passive DTO
-> ploke-tree FsRunStore / RunRecordSet / Graph
-> ploke-egui import and views
```

This is a pipeline map, not a new authority model. `ploke-eval` keeps live
state transitions, validation, scheduling, filesystem mutation, History
admission, and runtime advancement. `ploke-records` owns passive persisted
schemas only. `ploke-tree` loads and indexes typed records for read-only
projection. `ploke-egui` imports from `ploke-tree` and should not parse
Prototype 1 files directly.

Primary source docs:

- [`2026-05-09_records-emission-clean-sweep-handoff.md`](2026-05-09_records-emission-clean-sweep-handoff.md)
- [`2026-05-09_ploke-records-protocol-handoff.md`](2026-05-09_ploke-records-protocol-handoff.md)
- [`2026-05-12_agent-turn-record-projection-handoff.md`](2026-05-12_agent-turn-record-projection-handoff.md)
- [`2026-05-11_ploke-egui-graph-import-boundary-handoff.md`](2026-05-11_ploke-egui-graph-import-boundary-handoff.md)
- [`2026-05-11_ploke-tree-graph-ingestion-inventory.md`](2026-05-11_ploke-tree-graph-ingestion-inventory.md)
- [`ploke-tree-graph-ingestion/2026-05-11_3a2ea733-persisted-surface-survey.md`](ploke-tree-graph-ingestion/2026-05-11_3a2ea733-persisted-surface-survey.md)

## Pipeline Table

| Prototype 1 file or pattern | `ploke-eval` emitter | `ploke-records` passive type | `ploke-tree` ingestion | `ploke-egui` use/display | Authority and gap notes |
|---|---|---|---|---|---|
| `prototype1/history/blocks/segment-*.jsonl` | `cli/prototype1_state/history.rs` `FsBlockStore` sealed append | `history::SealedBlockRecord` | `RunRecordSet.history_blocks`; `Graph::from_records` indexes History lineages, blocks, entries, candidates, selections, memberships; playback/browser start here | Main graph content and artifact/history view input | Primary read-side ordering spine. Passive record does not prove hashes or advance Crown state. |
| `prototype1/history/index/{by-hash.jsonl,by-lineage-height.jsonl,heads.json}` | `cli/prototype1_state/history.rs` index writes | no exact passive owner | not loaded | not imported | Derived indexes over sealed blocks. If consumed, add typed rebuild/check metadata first. |
| `prototype1/transition-journal.jsonl` | `cli/prototype1_state/journal.rs`; handoff call sites in `prototype1_process.rs` | `journal::JournalEntry` | `RunRecordSet.transition_journal` plus `PassiveEvidence.transition_journal`; graph attaches loaded-entry and journal-line evidence | Evidence/diagnostics only when joined to runtime or branch | Append-only transition evidence, secondary to sealed History for ordering. |
| `prototype1/run-profile.toml` and `run-profile.commitment.json` | `cli/prototype1_state/profile.rs` admission writes | `run_profile::RunProfileRecord`, `RunProfileCommitmentRecord` | `PassiveEvidence.run_profile`; graph attaches run-profile summary and commitment evidence | Evidence/source plumbing; no dedicated panel found | Metadata and reproducibility evidence only. No lineage/runtime/artifact authority. |
| `prototype1/scheduler.json` | `intervention/scheduler.rs` scheduler projection | `scheduler::SchedulerStateRecord` | `RunForestInput.scheduler`; graph ingests scheduler campaign/status evidence; forest projection uses it | Imported through `graph_from_run_root`; visible in graph facts/view/diagnostics | Legacy scheduler/status projection. Not ordering or selection authority. |
| `prototype1/nodes/<node>/node.json` | `intervention/scheduler.rs::save_node_record` | `scheduler::NodeRecord` | `RunForestInput.node_records`; graph merges node metadata and branch/artifact evidence | View graph and inspector can show node/branch/artifact relations | Passive node/runtime metadata. Frontier/lane membership is not fully preserved as graph relations yet. |
| `prototype1/nodes/<node>/runner-request.json` | `intervention/scheduler.rs::save_runner_request` | `scheduler::RunnerRequestRecord` | `PassiveEvidence.run_attempts.runner_requests`; graph attaches run-attempt evidence | Runtime/operation/evidence display and counts | Passive run-attempt evidence, not execution authority. |
| `prototype1/nodes/<node>/runner-result.json` | `intervention/scheduler.rs::save_runner_result` | `scheduler::RunnerResultRecord` | `PassiveEvidence.run_attempts.runner_results`; graph attaches branch/artifact/operation evidence | Runtime/operation/evidence display and source refs | Latest mutable node result evidence, not sealed History authority. |
| `prototype1/nodes/<node>/results/<runtime>.json` | `prototype1_process.rs` attempt-scoped result write | `scheduler::RunnerResultRecord` | `PassiveEvidence.attempt_runner_results`; graph attaches attempt-scoped candidate/evaluation evidence | Evidence/source refs | Attempt-scoped result evidence. Uses same passive type as latest runner result. |
| `.ploke/prototype1/parent_identity.json` in active checkout | `cli/prototype1_state/identity.rs::write_parent_identity` | `identity::ParentIdentityRecord` | optional `RunForestInput.parent_identity`; graph attaches parent identity evidence | Evidence/inspector source refs | Artifact-carried parent identity witness. Evidence for parent-capable Artifact, not a passive bootstrap authority constructor. |
| `prototype1/nodes/<node>/invocations/<runtime>.json` | `cli/prototype1_state/invocation.rs` invocation writes | `invocation::InvocationRecord` | `PassiveEvidence.run_attempts.invocations`; graph attaches runtime/operation evidence | View graph can show runtimes/operations/evidence | Executable handoff boundary, loaded as passive evidence. |
| `prototype1/nodes/<node>/successor-ready/<runtime>.json` | `cli/prototype1_state/invocation.rs::write_successor_ready_record` | `invocation::SuccessorReadyRecord` | `RunForestInput.successor_ready`; graph attaches handoff/runtime evidence | Runtime/edge/evidence display | Handoff evidence only. |
| `prototype1/nodes/<node>/successor-completion/<runtime>.json` | `cli/prototype1_state/invocation.rs::write_successor_completion_record` | `invocation::SuccessorCompletionRecord` | `RunForestInput.successor_completion`; graph attaches completion/runtime evidence | Runtime/edge/evidence display | Completion evidence only. |
| `prototype1/nodes/<node>/channels/<runtime>/{parent-to-child,child-to-parent}.jsonl` | `cli/prototype1_state/channel.rs` envelope appenders | `channel::Envelope<ToChild>`, `Envelope<ToParent>` | `PassiveEvidence.channel_envelopes` summary counts | Evidence counts/source refs only | Transport evidence. Envelope contents are not first-class graph relations yet. |
| `prototype1/branches.json` | `intervention/branch_registry.rs` append/snapshot writes | `branch::BranchLogRecord` or `branch::Prototype1BranchRegistry` | `PassiveEvidence.branch_registry` summary | Evidence counts/source refs only | Append-only branch/comparison stream. Summary evidence, not selection authority. |
| `prototype1/evaluations/branch-*.json` | `cli/prototype1_state/cli_facing.rs` evaluation artifact write | `evaluation::Artifact` | `PassiveEvidence.evaluations`; graph attaches evaluation summary and branch candidate-evaluation evidence | Parent-create inspector shows child eval evidence counts; browser can attach metric snapshots | Metrics are present in records but not fully indexed as graph relations. |
| `prototype1/messages/child-plan/<parent-node-id>.json` | `cli/prototype1_state/cli_facing.rs::write_child_plan_file`; active shape in `parent.rs` | `child_plan::ChildPlanRecord` | `PassiveEvidence.child_plans`; graph stores `graph.child_plans.plans` and summary evidence | Parent-create inspector uses child plans, surface facts, branch joins, and agent-turn rollups | Parent-owned message box. Summary/evidence only; does not create child branch/runtime authority by itself. |
| `prototype1/messages/edit-harness-request/*.json` and `*.md` | `cli/prototype1_state/cli_facing.rs` broad harness request publication | no obvious passive owner | not loaded | not imported | Current broad-harness authority-path carrier. Needs passive record ownership before tree/UI consumption. |
| `prototype1/messages/edit-harness-result/*.json` and `*.headless-tui.json` | `cli/prototype1_state/cli_facing.rs` broad harness result/diagnostic writes | no obvious passive owner | not loaded | not imported | Submitted active evidence and diagnostics/projection surface. Needs passive ownership and loader if UI should show it. |
| configured `protocol-artifacts/*.json` | `protocol_artifacts.rs::write_protocol_artifact` | `protocol::Artifact` with `protocol` feature | `PassiveEvidence.protocol_artifacts` when `FsRunStore::with_protocol_artifacts_dir` is configured; graph attaches summary and artifact evidence | Evidence/source plumbing; no bespoke egui detail found | Not discovered from run root today. Evidence attachment, not run-root authority. |
| run dir `agent-turn-trace.json` | `runner.rs` writes `AgentTurnTraceRecord(artifact.to_record())` | `agent_turn::AgentTurnTraceRecord` with `tool-contracts` feature | `PassiveEvidence.agent_turns.traces`; graph attaches summary/artifact metadata evidence | Parent-create inspector renders LLM calls/tool counts when joined through branch evidence | Eval runner is live-to-record boundary. Tool-call/result decomposition is still pending. |
| run dir `agent-turn-summary.json` | `runner.rs` writes `AgentTurnSummaryRecord(artifact.to_record())` | `agent_turn::AgentTurnSummaryRecord` with `tool-contracts` feature | `PassiveEvidence.agent_turns.summaries`; graph attaches summary/artifact metadata evidence | Parent-create inspector renders LLM calls/tool counts when joined through branch evidence | Final turn artifact. Same pending tool/provider detail gap as trace. |
| run dir `llm-full-responses.jsonl` | `runner.rs` copies/reads raw full-response sidecar | no obvious passive owner | not loaded | not imported | Provider/full-response evidence gap. Do not parse directly in graph/UI without a typed owner. |
| run dir `record.json.gz` | `record.rs::write_compressed_record`; runner writes packaged `RunRecord` | missing exact passive owner | not loaded | not imported | Passive replay package gap. Add a passive owner/loader before graph use. |
| `prototype1/nodes/<node>/streams/<runtime>/{stdout,stderr}.log` | `prototype1_process.rs` stream log creation | none | not loaded | not imported | Log artifact. Needs typed locator/metadata if surfaced; not authority. |
| `prototype1_observation_*.jsonl` | `tracing_setup.rs` observation writer and `prototype1_state/observe.rs` helpers | no obvious passive owner | not loaded | not imported | Telemetry/projection surface. Should enter graph only as weak typed observation evidence after safe joins exist. |
| legacy `prototype1-loop-trace.json` and `slice.jsonl` | legacy/projection surfaces noted by survey | none | not loaded | not imported | Stale/debug projection. Do not use for new authority or graph construction. |
| setup/context `campaign.json`, `closure-state.json` | `campaign.rs`, `closure.rs` setup writes | no exact passive owner | not loaded | not imported | Setup metadata, outside current run graph unless a typed campaign/run context row is added. |
| model/provider/target/cache `registry.json`, `active-model.json`, `provider-preferences.json`, `registries/*.json`, `datasets/*.jsonl` | eval model/provider/target/cache modules | no Prototype 1 passive graph owner | not loaded | not imported | Environment/cache context. Attach only when tied to a concrete run through typed metadata. |

## Current Shape

`ploke-egui` imports through:

```text
ploke_egui::import::graph_from_run_root
-> ploke_tree::FsRunStore::load_record_set
-> ploke_tree::Graph::from_records
```

That means the UI should answer record questions from `ploke_tree::Graph`,
`RunRecordSet`, or typed drilldown projections over those structures. It should
not add direct readers for `prototype1` JSON, JSONL, logs, or compressed replay
files.

## Immediate Gaps

| Gap | Why it matters | Next owner before UI use |
|---|---|---|
| `edit-harness-request` and `edit-harness-result` messages | Broad harness is now part of the current loop, but these request/result records do not have a clear passive schema owner in `ploke-records`. | `ploke-records` passive DTOs, then `ploke-tree` store rows. |
| `llm-full-responses.jsonl` and provider attempts/retries/timeouts | Agent-turn records carry summary/tool counts, but provider-attempt provenance is not loaded as typed evidence. | Dedicated passive provider-attempt/full-response record family. |
| `record.json.gz` | Replay package exists but is not part of `RunRecordSet`. | Passive run-record package owner and `ploke-tree` loader or locator evidence. |
| stream logs and observation JSONL | Useful operator evidence, but currently weak/projection surfaces. | Typed locator/metadata or weak observation records; never graph authority. |
| History indexes | Useful for rebuild/check speed, but derived from sealed blocks. | Rebuildability/check metadata if the UI needs to report index health. |
| tool-call/tool-result decomposition | Agent-turn artifacts are loaded, but graph detail stops at summary/artifact metadata. | Graph-owned tool-call/result relations or typed answer objects borrowed by UI. |

## Update Rule

When a new Prototype 1 persisted surface starts feeding the UI, update this map
in the same change and answer four questions:

1. What `ploke-eval` transition or emitter writes it?
2. What passive `ploke-records` type owns the persisted bytes?
3. Where does `ploke-tree` load it and how does `Graph::from_records` represent
   it: core relation, evidence, metadata, locator, diagnostic, or explicit gap?
4. Which `ploke-egui` view consumes the `ploke-tree` object, without parsing the
   original file directly?
