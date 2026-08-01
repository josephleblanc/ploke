# Prototype 1 Persistence Map Synthesis

Date: 2026-05-03

This synthesis joins the seven worker reports in this folder into one operator-facing inventory.
It treats CLI output as projection. The source objects are persisted files, typed records, journals,
logs, worktree artifacts, and their join keys.

## Main Shape

The live single-parent loop does not persist one uniform record. It writes several evidence families:

- loop control state under `~/.ploke-eval/campaigns/<campaign>/prototype1/`
- normal campaign/run evidence under `~/.ploke-eval/campaigns/<campaign>/` and `~/.ploke-eval/instances/...`
- protocol evidence under each run directory
- sealed local History under `prototype1/history/`
- diagnostic logs under campaign node streams and `~/.ploke-eval/logs/`
- artifact-carried identity inside each realized parent/child worktree

The useful join spine is:

```text
campaign_id
  -> node_id / generation / branch_id / runtime_id
  -> runner-result evaluation_artifact_path
  -> branch evaluation compared_instances[*].{baseline_record_path,treatment_record_path}
  -> run record / turn / tool / protocol artifact / provider response evidence
```

## Unified Table

| Family | Persisted item | Path pattern | Type / schema | Producer | Current CLI readers | Join keys | Evidence class | Duplication / notes |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| Campaign | Campaign manifest | `campaigns/<campaign>/campaign.json` | `CampaignManifest` | setup, treatment campaign creation, `campaign init` | `campaign show/validate`, closure, protocol/inspect campaign paths | `campaign_id`, roots, model/provider | durable config | Duplicates config roots later copied into closure and run manifests. |
| Campaign | Slice dataset | `campaigns/<campaign>/slice.jsonl` | selected JSONL dataset rows | Prototype 1 setup slice writer | campaign setup/load, campaign show indirectly | `instance_id`, dataset source label | campaign input evidence | Raw benchmark issue data; do not dump. |
| Campaign | Closure state | `campaigns/<campaign>/closure-state.json` | `ClosureState` | `recompute_closure_state`, `advance_eval_closure`, `advance_protocol_closure` | `closure status`, `inspect * --campaign`, branch evaluation compare | `campaign_id`, `instance_id`, `record_path`, `protocol_anchor` | projection/cache | Duplicates eval/protocol status derived from registrations, records, and protocol artifacts. |
| Campaign | Active selection | eval-home `selection.json` | `ActiveSelection` | `select` commands | defaults for several CLI commands | selected campaign/instance/model | operator convenience | Not loop authority; can be stale or inconsistent with campaign slice. |
| Campaign | Treatment campaign manifests | `campaigns/<baseline>-treatment-branch-*/campaign.json` | `CampaignManifest` | child branch evaluation | closure/protocol/inspect if targeted directly | baseline `campaign_id`, `branch_id`, treatment `campaign_id` | durable treatment config | Links to loop mostly through runner result and branch evaluation report. |
| Campaign | Treatment closure state | `campaigns/<baseline>-treatment-branch-*/closure-state.json` | `ClosureState` | child branch eval/protocol closure | branch evaluation builder, closure/inspect if targeted | treatment `campaign_id`, `instance_id`, `record_path` | projection/cache | Same reduced facts as baseline closure, scoped to one treatment branch. |
| Loop | Scheduler | `campaigns/<campaign>/prototype1/scheduler.json` | `Prototype1SchedulerState` | scheduler registration/status/result/continuation writers | `loop prototype1-monitor report/timing/history-*`, state startup | `campaign_id`, `node_id`, `branch_id`, `generation` | mutable loop projection | Duplicates node records and latest result/status. Projection-only for History purposes. |
| Loop | Node record | `prototype1/nodes/<node-id>/node.json` | `Prototype1NodeRecord` | scheduler `save_node_record` | runner, monitor, history preview/metrics | `node_id`, `parent_node_id`, `generation`, `branch_id`, paths | mutable node projection | Mirrors scheduler node list; can drift unless recovery checks reconcile. |
| Loop | Runner request | `prototype1/nodes/<node-id>/runner-request.json` | `Prototype1RunnerRequest` | scheduler node registration/workspace update | child runner, monitor, history preview | `campaign_id`, `node_id`, `branch_id`, `source_state_id` | runtime input contract | Duplicates node workspace/binary/branch fields from node record and scheduler. |
| Loop | Latest runner result | `prototype1/nodes/<node-id>/runner-result.json` | `Prototype1RunnerResult` | `record_runner_result` | monitor, runner inspect, parent reads success/failure | `node_id`, `branch_id`, `treatment_campaign_id`, `evaluation_artifact_path` | latest projection | Duplicates attempt result; prefer attempt-scoped result when available. |
| Loop | Attempt result | `prototype1/nodes/<node-id>/results/<runtime-id>.json` | `Prototype1RunnerResult` | `record_attempt_runner_result` | `ObserveChild`, history preview/metrics, monitor | `runtime_id`, `node_id`, `branch_id`, evaluation path | attempt-scoped evidence | Same payload as latest result but keyed by runtime attempt. Better recovery source. |
| Loop | Runtime invocation | `prototype1/nodes/<node-id>/invocations/<runtime-id>.json` | `Invocation` / child or successor role | `write_child_invocation`, `write_successor_invocation_for_retired_parent` | runner `--invocation`, successor `--handoff-invocation`, history preview | `runtime_id`, `node_id`, `role`, `journal_path` | attempt bootstrap contract | Carries paths already represented in node/request/journal. |
| Loop | Child plan message | `prototype1/messages/child-plan/<parent-node-id>.json` | `ChildPlanFiles` | parent planning path | state receive/validate child plan, history completeness | `parent_node_id`, child `node_id`, scheduler/branch paths | cross-runtime message | Binds file addresses but not content digests. Easy to miss. |
| Loop | Branch registry | `prototype1/branches.json` | `Prototype1BranchRegistry` | synthesis/apply/select/evaluation registry writers | `prototype1-branch`, monitor report/history | `source_state_id`, `target_relpath`, `candidate_id`, `branch_id` | mutable branch evidence/projection | Stores branch content plus selected/evaluation summaries; overlaps with evaluation reports. |
| Loop | Branch evaluation report | `prototype1/evaluations/<branch-id>.json` | `Prototype1BranchEvaluationReport` | child branch evaluation writer | monitor report/history, parent observes child result | `branch_id`, treatment campaign, compared record paths | derived comparison evidence | Copies metrics from baseline/treatment run records; join back to records for detail. |
| Loop | Transition journal | `prototype1/transition-journal.jsonl` | `JournalEntry` variants | `PrototypeJournal::append` call sites | monitor report/watch/history-preview/history-metrics | `transition_id`, `runtime_id`, `node_id`, `branch_id`, generation | append-only transition evidence | Separate from sealed History; not cryptographically sealed. |
| Loop | Successor ready | `prototype1/nodes/<node-id>/successor-ready/<runtime-id>.json` | `SuccessorReadyRecord` | successor runtime ready writer | predecessor wait, monitor/history | `runtime_id`, `node_id`, pid | successor ack evidence | Duplicates journal successor-ready event. |
| Loop | Successor completion | `prototype1/nodes/<node-id>/successor-completion/<runtime-id>.json` | `SuccessorCompletionRecord` | successor completion writer | monitor/history | `runtime_id`, `node_id`, status, trace path | successor terminal evidence | Duplicates/extends journal completion state. |
| Loop | Loop controller trace | `campaigns/<campaign>/prototype1-loop-trace.json` | `Prototype1LoopReport` / legacy trace | loop controller writer | monitor locations/report references | `campaign_id`, staged nodes, selected branch | overwritten projection | Convenience trace only; root-level placement differs from prototype subtree. |
| Artifact | Parent identity | `<worktree>/.ploke/prototype1/parent_identity.json` | `ParentIdentity` | setup, successor/child artifact prep | parent state startup/validation | `campaign_id`, `parent_id`, `node_id`, `generation`, `branch_id` | artifact-carried identity witness | Lives in worktree, not campaign root; duplicated partly in scheduler/node/journal. |
| Artifact | Child worktree | `prototype1/nodes/<node-id>/worktree/` | filesystem artifact | materialize/build path | runner request, spawn validation, operator inspection | `node_id`, branch/workspace paths | runtime artifact | Huge mutable tree; identity needs parent identity and History/branch context. |
| Artifact | Child binary | `prototype1/nodes/<node-id>/bin/ploke-eval` | executable artifact | build/promote path | child spawn request | `node_id`, `binary_path` | build artifact | Size-dominant; not a typed record. |
| Artifact | Child target dir | `prototype1/nodes/<node-id>/target/` | build outputs | cargo build/check path | none primarily; operator forensics | `node_id` by path | build cache/artifact | Very large; do not include in evidence scans except counts/sizes. |
| History | Sealed block stream | `prototype1/history/blocks/segment-000000.jsonl` | `Block<Sealed>` | handoff seal and `FsBlockStore::append` | startup sealed head, History store internals | `lineage_id`, `block_hash`, height, parent hashes | local sealed History authority | Zero-entry blocks currently; not global consensus or process uniqueness. |
| History | By-hash index | `prototype1/history/index/by-hash.jsonl` | `StoredBlock` | `FsBlockStore::append` | sealed head lookup, lineage state | `block_hash`, location | rebuildable projection | Index over block stream; not independent authority. |
| History | By-lineage-height index | `prototype1/history/index/by-lineage-height.jsonl` | `LineageHeight` | `FsBlockStore::append` | lineage consistency checks | `lineage_id`, height, hash | rebuildable projection | Duplicates block header lineage/height. |
| History | Heads projection | `prototype1/history/index/heads.json` | `BTreeMap<LineageId, BlockHash>` | `FsBlockStore::append` | lineage state/head lookup | `lineage_id -> block_hash` | rebuildable projection | Checked by append/read, but block stream is authority. |
| History | History preview | stdout unless redirected | `HistoryPreview` | `history preview`, monitor history-preview | operator only | evidence pointers, node/branch/runtime ids | projection only | Reads journal/mutable docs, not sealed block store authority. |
| History | Metrics dashboard | stdout unless redirected | `Dashboard` | `history metrics`, monitor history-metrics | operator only | evidence pointers, generation/node/branch ids | projection only | Does not strengthen mutable records. |
| Run | Prepared run manifest | `instances/.../<instance>/run.json` and batch manifests | `PreparedSingleRun`, `PreparedMsbBatch` | normal eval preparation | runner, closure registry | `task_id`/`instance_id`, campaign context | run input evidence | Duplicates campaign/model/budget/source context. |
| Run | Run registration | `registries/runs/<run-id>.json` | `RunRegistration`, `RunArtifactRefs` | runner lifecycle persistence | closure, record resolution, protocol status sync | `run_id`, `task_id`, `run_root`, `record_path` | lifecycle/artifact registry | Names artifacts not embedded in `RunRecord`; duplicates closure refs. |
| Run | Compressed run record | `instances/.../runs/run-*/record.json.gz` | `RunRecord`, `run-record.v1` | normal runner final writer | many `inspect` commands, closure, branch evaluation, monitor timing | `manifest_id`, instance id, run arm, record path | primary eval execution evidence | Does not embed full-response sidecar; branch reports copy derived metrics from it. |
| Run | Agent turn trace | `runs/run-*/agent-turn-trace.json` | `AgentTurnArtifact`, `ObservedTurnEvent` | benchmark turn loop | inspect turn/tool, monitor timing | turn, `call_id`, `assistant_message_id` | structured turn evidence | Embedded into `RunRecord` summary too; trace may be more detailed/live. |
| Run | Agent turn summary | `runs/run-*/agent-turn-summary.json` | `AgentTurnArtifact` summary | runner after turn | run registry/inspect paths | turn/tool/message ids | sidecar projection | Duplicates terminal turn state. |
| Run | Full response sidecar | `runs/run-*/llm-full-responses.jsonl` | `RawFullResponseRecord` | sliced from full-response tracing log | `inspect turn --show responses`, monitor timing | `assistant_message_id`, `response_index`, response id | provider envelope evidence | Known possible undercount for final stop responses. |
| Run | Global full-response log | `logs/llm_full_response_<run_id>.log` | full-response tracing JSONL | tracing target from TUI/LLM session | sliced into run-local sidecar | response/message ids, file offset | diagnostic/source log | Run-local sidecar is the preferred per-run surface. |
| Run | Setup/phase sidecars | `runs/run-*/repo-state.json`, `execution-log.json`, `indexing-status.json`, `parse-failure.json`, `snapshot-status.json` | runner artifact structs | runner setup/phase writers | closure/run history/inspect | run dir, artifact refs | setup/phase evidence | May exist even when final record fails. |
| Run | MSB submission artifact | `runs/run-*/multi-swe-bench-submission.jsonl` and batch aggregate | submission artifact records | packaging/batch writer | closure/export | instance id, run dir | submission/export evidence | Campaign export is a projection over per-run artifacts. |
| Protocol | Protocol artifact envelope | `runs/run-*/protocol-artifacts/*.json` | `StoredProtocolArtifact`, `protocol-artifact.v1` | `write_protocol_artifact` | `inspect protocol-artifacts`, `protocol status`, aggregate builders | `run_id`, `subject_id`, procedure, created_at | authoritative protocol evidence | Envelope wraps all procedure-specific inputs/outputs. |
| Protocol | Intent segmentation | `*_tool_call_intent_segmentation_*.json` | `SegmentedToolCallSequence` | protocol segmentation procedure | protocol status/run, protocol overview | subject id, call indices, segment index | protocol anchor evidence | Latest anchor determines usable segment reviews. |
| Protocol | Tool call review | `*_tool_call_review_*.json` | `LocalAnalysisAssessment` over call neighborhood | protocol call-review procedure | protocol aggregate/overview, issue detection | focal call index, target id, calls | protocol review evidence | Latest accepted review per focal call is projected into aggregate. |
| Protocol | Segment review | `*_tool_call_segment_review_*.json` | `LocalAnalysisAssessment` over segment | protocol segment-review procedure | protocol aggregate/overview | segment index, target id, anchor basis | protocol review evidence | Persisted old reviews may be skipped if anchor mismatch. |
| Protocol | Issue detection | `*_intervention_issue_detection_*.json` | `IssueDetectionOutput` | protocol issue-detection helper/command | `inspect issue-overview`, synthesis inputs | run/subject id, reviewed call/segment indices, target tool | derived protocol synthesis evidence | Derived from run record + protocol aggregate. |
| Protocol | Intervention synthesis | `*_intervention_synthesis_*.json` | `InterventionSynthesisOutput` / candidate set | synthesis helper | generic protocol artifact inspect, branch registry ingestion | source state, target relpath, candidate id | derived intervention proposal | No dedicated high-level inspect command found. |
| Protocol | Protocol aggregate | not persisted | `ProtocolAggregate` | built on demand | `inspect protocol-overview`, `protocol status` | run id, subject id, artifact refs | in-memory projection | No aggregate file found. |
| Logs | Observation JSONL | `logs/prototype1_observation_<run-id>.jsonl` or explicit env path | tracing JSONL events | `PLOKE_PROTOTYPE1_TRACE_JSONL` layer | monitor timing | campaign/node/branch/runtime ids, span, request id | diagnostic telemetry | Outside campaign dir; best current structured timing/provider source. |
| Logs | `chat_http` events | observation JSONL and sometimes stderr | tracing events | `ploke-llm` session HTTP attempt wrapper | monitor timing parses observation JSONL, stderr fallback counts | request id, attempt, node/branch via spans | diagnostic provider transport evidence | No durable typed provider-attempt record; request id is process-local. |
| Logs | TimingTrace stderr | `prototype1/nodes/<node-id>/streams/<runtime-id>/stderr.log` | plain text start/end lines | `TimingTrace` | monitor timing regex parser | node/runtime path, labels, branch in label | diagnostic projection | Replaced/supplemented by observation JSONL when enabled. |
| Logs | Runtime streams | `prototype1/nodes/<node-id>/streams/<runtime-id>/{stdout,stderr}.log` | raw process logs | child/successor spawn | monitor peek/timing, operator `tail`/`rg` | node id, runtime id by path | diagnostic process evidence | Can contain useful JSON snippets and warnings but not stable schema. |
| Logs | General eval log | `logs/ploke_eval_<run-id>.log` | formatted tracing log | eval tracing setup | no structured Prototype 1 reader | weak text fields | diagnostic log | Avoid for machine joins except bounded forensic searches. |
| Config | Model/provider/target registries | eval config/home registry JSON | registry/config structs | model/provider/target commands | campaign setup and selection defaults | model/provider/target ids | configuration | Outside loop evidence; affects repeatability. |
| Config | Last run pointer | eval-home `last-run.json` | `LastRunRecord` | runner | default inspect resolution | run id/path | operator convenience | Not loop authority. |

## Duplication And Overlap

| Duplicated fact | Copies | Preferred source when answering operator questions |
| --- | --- | --- |
| Node identity/status | `scheduler.json`, `node.json`, transition journal, latest/attempt result | For current frontier: scheduler. For transition proof: journal. For concrete child outcome: attempt result. |
| Runner result | `nodes/<node>/results/<runtime>.json`, `nodes/<node>/runner-result.json`, scheduler node status, branch evaluation summary | Attempt result first, latest result second, scheduler only for frontier/status projection. |
| Branch evaluation outcome | evaluation report, branch registry latest evaluation, runner result, scheduler status | Evaluation report for comparison detail; runner result for child attempt outcome; registry for branch-selection summary. |
| Baseline/treatment run status | closure-state, run registration, `record.json.gz`, protocol artifacts | `record.json.gz` for eval facts; protocol artifacts for protocol facts; closure for campaign progress only. |
| Tool calls | `RunRecord.phases.agent_turns[].tool_calls`, embedded/reconstructed turn events, `agent-turn-trace.json`, protocol artifact inputs | RunRecord for stable persisted run view; trace for event-level timing; protocol artifact input for reviewed subject basis. |
| Provider responses/usage | full-response sidecar, `RunRecord` turn/LLM records, provider HTTP logs | Full-response sidecar for raw envelopes, with undercount caveat; RunRecord for turn summary; logs for transport retries/timeouts. |
| Protocol status | protocol artifacts, `ProtocolAggregate` projection, closure protocol status | Artifacts for evidence; aggregate for normalized inspection; closure only for campaign progress. |
| History state | sealed block stream, indexes, heads, preview, metrics | Sealed block stream for local History authority; indexes/heads are rebuildable; preview/metrics are projections. |
| Parent identity | worktree parent identity file, scheduler/node/journal references, History artifact claim | Worktree identity validates the artifact; sealed History claim is stronger once present; scheduler/journal are operational joins. |
| Timing | observation JSONL, stderr TimingTrace, run timing summary, monitor timing projection | Observation JSONL for structured causal timing; run record for eval wall-clock; monitor projection for operator view only. |

## Main Gaps

- There is no single authoritative object tying `campaign_id`, `node_id`, `branch_id`, `runtime_id`,
  treatment campaign id, branch evaluation path, and compared run record paths together.
- `ProtocolAggregate` is recomputed, not persisted. That is fine for projection, but command discovery is hard because
  no one file says “this is the protocol state for this treatment eval.”
- Provider retry/timeout/backoff evidence is only telemetry/log evidence. It has no durable typed attempt record.
- TimingTrace is plain stderr text. The structured observation JSONL is better, but still diagnostic rather than a typed timing artifact.
- History preview/metrics do not read sealed History blocks as their authority source; they project mutable files and journals.
- Sealed History currently appears to write zero-entry blocks for handoff authority. Admitted entry persistence exists as type surface but is not live in the main loop path.
- Treatment eval artifacts live outside the base campaign `prototype1/` subtree. The current normal `inspect --campaign` surface sees baseline campaign closure, not the child treatment cohort.

## Candidate Operator Views

The persistence map suggests three first-class projections that would reduce CLI wandering:

1. `loop prototype1-monitor evidence`
   One row per node/runtime/branch with paths to scheduler node, invocation, attempt result, evaluation report, treatment campaign, and compared run records.

2. `loop prototype1-monitor cohort`
   One row per child treatment eval, grouped by generation/branch, aggregating run outcome, protocol issue family, tool-call failures, provider retries/timeouts, and disposition.

3. `loop prototype1-monitor files`
   A bounded path-family inventory for a campaign, with counts/sizes and evidence class. This should make hidden surfaces like child plans, History blocks, treatment campaigns, and observation logs visible without dumping payloads.
