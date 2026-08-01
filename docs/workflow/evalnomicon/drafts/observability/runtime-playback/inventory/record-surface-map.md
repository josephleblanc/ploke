# Record Surface Map

Status: first pass, code-backed as of 2026-05-21.

This map starts from the current read path:

```text
FsRunStore::load_record_set()
  -> RunRecordSet
  -> Graph::from_records(&RunRecordSet)
  -> RuntimePlaybackRef<'g, G>
```

The important boundary is that `FsRunStore` reads files into named record
types, `Graph::from_records` builds the read-side graph, and playback should
iterate graph facts. CLI and egui should not rediscover files independently.

## Loaded Record Set

`crates/ploke-tree/src/store/record_set.rs::RunRecordSet` currently carries:

| Field | Type | Source | Current graph use |
| --- | --- | --- | --- |
| `forest_input.scheduler` | `ploke_records::scheduler::SchedulerStateRecord` | `scheduler.json` | scheduler forest and legacy process context |
| `forest_input.node_records` | `Vec<NodeRecord>` | `nodes/<node-id>/node.json` | candidate branches, artifacts, operations, scheduler evidence |
| `forest_input.parent_identity` | `ParentIdentityRecord` | parent checkout `.ploke/prototype1/parent_identity.json` when supplied | parent identity evidence |
| `forest_input.successor_ready` | `Vec<SuccessorReadyRecord>` | `nodes/<node-id>/successor-ready/*.json` | successor/runtime evidence |
| `forest_input.successor_completion` | `Vec<SuccessorCompletionRecord>` | `nodes/<node-id>/successor-completion/*.json` | successor/runtime evidence |
| `forest_input.passive_evidence` | `PassiveEvidence` | several sidecar surfaces below | evidence, summaries, and some graph facts |
| `history_blocks` | `Vec<SealedBlockRecord>` | `history/blocks/segment-*.jsonl` | primary History/order/authority facts |
| `transition_journal` | `TransitionJournal` | `transition-journal.jsonl` | typed transition evidence attached to branches, runtimes, operations |
| `agent_turn_records` | `AgentTurnRecordSet` | `agent-turn-trace.json`, `agent-turn-summary.json` | full turn records carried through graph boundary for drilldown |

## Passive Evidence Inputs

`crates/ploke-tree/src/store/fs.rs::load_passive_evidence` currently loads
these surfaces.

| Surface | Owner type | Persisted path | Current graph representation | Join key | Main gap |
| --- | --- | --- | --- | --- | --- |
| Branch registry | `Prototype1BranchRegistry` or `BranchLogRecord` | `branches.json` | summary evidence only | `branch_id`, `node_id`, `campaign_id` | registry entries are not a playback step source |
| Transition journal summary | `JournalEntry` count | `transition-journal.jsonl` | summary plus loaded transition entries | line number, `runtime_id`, `branch_id`, `node_id` | needs typed playback family for journal events |
| History storage summary | `SealedBlockRecord` count | `history/blocks/segment-*.jsonl` | summary evidence and `history_blocks` | `lineage_id`, `block_id`, `block_hash`, height | index files are rebuildable projections, not loaded authority |
| Runtime channels | `Envelope<ToParent>` / `Envelope<ToChild>` | `nodes/<node-id>/channels/<runtime-id>/{child-to-parent,parent-to-child}.jsonl` | summary evidence only | `runtime_id`, `node_id`, `message_id`, cursor offset | per-envelope steps are not in graph/playback yet |
| Child plans | `ChildPlanRecord` | `messages/child-plan/*.json` | `graph.child_plans.plans` plus summary | `parent_node_id`, child `node_id`, `branch_id` | should join cleanly to operations/attempts/selection |
| Evaluations | `ploke_records::evaluation::Artifact` | `evaluations/*.json` | summary and branch evidence | `branch_id`, `instance_id`, compared record paths | comparison rows need better playback drilldown |
| Benchmark patch projection | `ploke_records::evaluation::BenchmarkPatchProjectionRecord` | `benchmark-patch-projection.json` beside a run's MBE submission | not loaded into graph by default today | run root, `record.json.gz`, submission path, benchmark target, candidate refs when present | should become a benchmark/evaluation witness joined to selection and artifact ancestry |
| Multi-SWE-bench submission | `ploke_eval::runner::MultiSweBenchSubmissionRecord` | `multi-swe-bench-submission.jsonl` under run roots and batch outputs | not loaded into graph by default today | org, repo, issue number, instance id, submission path | benchmark-facing patch export; should not be treated as freeform assistant output |
| Protocol artifacts | `ploke_records::protocol::Artifact` | `protocol-artifacts/*.json` and compared run dirs | summary and protocol evidence attachments | `procedure_name`, `subject_id`, `run_id`, path | nested procedure sequence is not graph/playback structure |
| Compressed run records | `ploke_records::run_record::RunRecord` | compared `record.json.gz` paths from evaluation artifacts | `RunRecordEvidence`, stats, refs by branch | `branch_id`, `record_key`, `manifest_id`, `instance_id` | full turn/tool sequence is loaded but not a first-class runtime playback iterator |
| Run profile | `RunProfileRecord`, `RunProfileCommitmentRecord` | `run-profile.toml`, `run-profile.commitment.json` | run-profile evidence | campaign/root profile facts | policy changes need playback frames and charts |
| Run attempts | `RunnerRequestRecord`, `RunnerResultRecord`, `InvocationRecord` | `nodes/<node-id>/runner-request.json`, `runner-result.json`, `invocations/*.json` | runtime, artifact, operation, branch evidence | `runtime_id`, `node_id`, `branch_id`, `operation_target` | should become a concrete attempt/invocation step family |
| Agent turns summary | `AgentTurnTraceRecord`, `AgentTurnSummaryRecord` via compact evidence | expected `agent-turn-*` files under run root or node dirs | summary and artifact evidence | path-derived `node_id`, `task_id`, `user_message_id` | metadata is graph evidence; ordered event content lives in `agent_turn_records` |
| Attempt results | `RunnerResultRecord` | `nodes/<node-id>/results/*.json` | branch evaluation evidence | `branch_id`, `runtime_id` from path/record context | path/runtime relation should be explicit |

## Current Graph Indexes

`crates/ploke-tree/src/graph/types.rs::Graph` currently has these relevant
indexes:

| Graph field | Meaning for playback |
| --- | --- |
| `forest` | scheduler/process tree and passive evidence carrier; useful for compatibility but not authority |
| `history` and `authority` | sealed History blocks, entries, lineage-local authority, and playback order source |
| `artifacts` | checkout states and Artifact identities from History and passive records |
| `runtimes` | concrete executions and runtime evidence |
| `operations` | runtime actions over targets, usually from typed coordinates |
| `candidates` and `selections` | candidate payloads, memberships, selected candidate, decision metadata |
| `metrics` | selection-time metric sets, imp@k rows, compared run metrics, formula records |
| `child_plans` | parent-published child plans and surface details |
| `evidence` | attached typed evidence and source locators |
| `agent_turn_records` | canonical agent-turn record set for lower-granularity turn drilldown |
| `warnings` | graph import warnings that should surface in playback/UI |

## History Payload Families

History files are read from `history/blocks/segment-*.jsonl` as
`ploke_records::history::SealedBlockRecord`. The block is the ordering spine,
but it also stores or references several lower-level semantic objects.

| History surface | Type family | Stored or referenced facts | Playback placement |
| --- | --- | --- | --- |
| Block header/state | `SealedBlockHeaderRecord`, `SealedBlockStateRecord`, `BlockCommonRecord` | lineage id, block id/hash, height, active Artifact, opened-from Artifact, surface commitment, parent identity, authority refs | `HistoryBlocks` and artifact ancestry frames |
| Admitted entry | `AdmittedEntryRecord`, `EntryCoreRecord`, `AdmittedEntryStateRecord`, `ObservedEntryRecord` | entry id/kind, subject, executor, input/output evidence refs, occurred/observed/recorded/admitted order, operational environment | admitted-entry playback step under a History block |
| Operational environment | `OperationalEnvironmentRecord` | runtime, artifact ref, binary/tool surface/procedure/model/code graph/oracle/recorder evidence refs | join bridge from History entry to runtime, Artifact, procedure, and model-exchange evidence |
| Direct payload | `EntryPayloadRecord::Direct` | entry facts carried only through subject/input/output refs | History entry step with evidence drilldown |
| Selection decision payload | `SelectionDecisionEntryRecord` | selected candidate, considered candidates, candidate set/proofs, projection failures, traversal strategy, metric set, formula, final decision | selection decision step, candidate table, formula and metric drilldowns |
| Candidate payloads | `EvaluationPayloadRecord`, `CandidateEvidenceRecord`, `CandidateArtifactRecord` | candidate subject/procedure, source hashes, sealed evaluation/runtime/branch evidence, artifact node/resolved branch, surface attempt | candidate and artifact drilldowns under selection |
| Run/evaluation evidence | `RunEvidenceRecord`, `ComparedRunEvidenceRecord`, `EvaluationEvidenceRecord`, `RuntimeEvidenceRecord`, `BranchEvidenceRecord` | run artifact paths, compared baseline/treatment metrics, oracle evaluation, protocol metrics, runtime and branch citations | benchmark/evaluation aggregates plus runtime/evidence drilldown |
| Surface/edit evidence | `SurfaceEvidenceRecord`, `SurfaceAttemptRecord`, `RequestPolicyReceiptRecord` | producer/proposal/run ids, request policy, model/provider policy, payload hashes, target/touch/delta/apply/check facts | edit lifecycle and model-request policy drilldowns |
| Ingress payload | `IngressImportRecord` | imported entry chain of custody and imported block/lineage coordinates | ingress/import playback step and gap repair audit |

This table should stay aligned with `ploke_records::history` and
`ploke_records::history::payload`. If a new History payload appears, add it
here before adding a renderer for it.

## Runtime Channel Files

The live writer and passive reader are currently separate type families. The
file shape is still the same channel schema.

| Direction | Live writer type | Passive owner type | Path | Playback status |
| --- | --- | --- | --- | --- |
| parent to child | `ploke_eval::prototype1_state::channel::Envelope<ToChild>` | `ploke_records::channel::Envelope<ToChild>` | `nodes/<node-id>/channels/<runtime-id>/parent-to-child.jsonl` | loaded as counts/summary only |
| child to parent | `ploke_eval::prototype1_state::channel::Envelope<ToParent>` | `ploke_records::channel::Envelope<ToParent>` | `nodes/<node-id>/channels/<runtime-id>/child-to-parent.jsonl` | loaded as counts/summary only |

Channel join keys are `runtime_id`, `node_id`, `message_id`, direction, and
cursor offset. The intended playback family is runtime-channel envelope order.
Until channel migration is complete, channel envelopes should be treated as
supplementary runtime evidence beside the transition journal, invocation
records, and sealed History.

## Index And DB Snapshot Files

Prototype 1 invokes `ploke-tui` indexing/reindexing while preparing and running
attempts. The run root stores DB artifacts and status records rather than
loading Cozo state into `Graph`.

| File or reference | Current type or owner | Writer / reader | Playback placement |
| --- | --- | --- | --- |
| `indexing-status.json` | `IndexingStatusArtifact` in eval run artifacts / run record refs | written by eval runner around TUI indexing status | index/reindex status step and failure diagnosis |
| `parse-failure.json` | parse failure artifact in eval run artifacts / run record refs | written when indexing or parsing reports failures | index/reindex diagnostic step |
| `indexing-checkpoint.db` | Cozo backup file path in `RunArtifactRefsRecord` and eval `RunRecord` | persisted by eval runner during indexing debug snapshots | DB snapshot witness, not a graph node |
| `indexing-failure.db` | Cozo backup file path in `RunArtifactRefsRecord` and eval `RunRecord` | persisted by eval runner on indexing failure when enabled | DB snapshot witness for failure drilldown |
| `snapshot-status.json` | snapshot status artifact path in run evidence | written/read by eval snapshot helpers | final snapshot locator |
| `final-snapshot.db` | Cozo backup file path in run evidence and run-history helpers | persisted after the run/turn completes | DB-state drilldown at runtime or turn terminal frame |
| `db_time_travel_index` | `TimeTravelMarker` in eval/read-side run records | marked with Cozo validity timestamps, for example `turn_complete` | cursor-to-DB timestamp bridge |
| TUI `workspaces.toml` | `WorkspaceRegistry` / `WorkspaceRegistryEntry` | TUI save/load commands under user config | outside-run-root; evidence only if explicitly captured |
| TUI `proposals.json` | `Vec<ploke_tui::app_state::core::EditProposal>` | TUI save/load commands under config or `PLOKE_PROPOSALS_PATH` | outside-run-root unless the eval run redirects and records the path |

The graph should initially carry typed DB witnesses: path, source record, phase,
and time-travel marker. Loading the DB contents belongs behind an explicit
drilldown query, not default graph construction.

## Selection Data

Selection is currently the best-covered higher-level aggregate: live selection
builds the decision, History seals the decision payload, and `ploke-tree`
imports that payload into selection/candidate/metric indexes.

| Selection surface | Live or persisted type | Graph representation | Join key | Playback use |
| --- | --- | --- | --- | --- |
| selection input | `ploke_records::selection::Input` and eval `SelectionInput` | mostly sealed through candidate payloads and metric rows | candidate node/branch/generation, evaluation artifact path | explain what evidence was available |
| decision entry | `SelectionDecisionEntryRecord` | `SelectionNode` in `Graph.selections` | `entry_id`, `selected_occurrence_id`, `selected_membership_id` | decision step and selected-row highlight |
| candidate set | `CandidateSetRecord`, `CandidateSetMembershipRecord` | `CandidateMembershipNode`, membership keys | `candidate_set_root`, `membership_id`, `occurrence_id` | prove selection membership and avoid source/decision membership mixups |
| considered payloads | `EvaluationPayloadRecord` | `CandidateNode`, `CandidateBranchNode`, evidence attachments | `entry_id`, payload index, `node_id`, `branch_id`, `artifact_id` | candidate table and artifact ancestry bridge |
| metrics | `ploke_records::selection::MetricSet`, `MetricCandidate`, `ImpAtK` | `MetricSetNode`, `MetricCandidateNode`, compared-run metric nodes | `metric_set_id`, payload index, candidate set root | benchmark/proxy score charts and improvement@k views |
| formula | `SelectionFormulaRecord`, `ScoreChildPropRecord`, rows | `SelectionFormulaNode` | `(entry_id, metric_set_id)` | deterministic replay of selected score/weight/sample |
| traversal evidence | `TraversalEvidenceRecord`, strategy records | selection node metadata and warnings | selection `entry_id`, candidate set root | show policy, seed, oracle mode, and evidence requirements |
| projection failures | `ProjectionFailureRecord` | selection/candidate warning counts and graph warnings | `entry_id`, payload index, failure id | explain why candidates were excluded or downgraded |

The important refactor direction is not to add a second selector report shape.
Playback should expose borrowed views over `Graph.selections`,
`Graph.candidates`, and `Graph.metrics`, with evidence strength preserved from
the sealed History entry and graph warnings surfaced beside the selected row.

## Known Duplicate Or Partial Shapes

These are not all bugs. Some are deliberate writer/read-side boundaries. They
are the places to audit before adding new record types.

| Area | Current duplicate pressure | Refactor direction |
| --- | --- | --- |
| Run record | `crates/ploke-eval/src/record.rs::RunRecord` is the active writer shape; `ploke_records::run_record::RunRecord` is the shared read-side shape. | Keep one canonical persisted schema in `ploke-records` once the writer can depend on it cleanly. Until then, avoid adding another smaller run-record view. |
| Agent-turn artifact | `ploke-eval::runner::AgentTurnArtifact` projects via `ToRecord` into `ploke_records::agent_turn::AgentTurnArtifactRecord`; `AgentTurnTraceProjection` also exists for replay/UI summaries. | Move durable schema additions into `ploke-records::agent_turn`; make replay/CLI use borrowed accessors over that owner shape. |
| Runtime channel | `ploke-eval::prototype1_state::channel::{Envelope, ToParent, ToChild}` writes live envelopes; `ploke_records::channel::{Envelope, ToParent, ToChild}` mirrors persisted envelopes for passive loading. | Decide whether the passive record can become the direct persisted schema for the eval channel writer without losing live typestate authority. |
| Protocol artifacts | `ploke-records::protocol::artifacts` contains `*Mirror` aliases over `ploke_protocol` procedure types and provenance mirrors. | Audit which mirrors are true persisted schemas and which can become borrowed typed views over protocol-owned records. |
| Run profile | `ploke-records::run_profile` documents passive DTOs that mirror `run-profile.toml` and `run-profile.commitment.json`. | Keep as passive schema, but avoid reusing it as runtime policy authority. |
| Selection material | Live selection uses `ploke-eval::successor_selection`; sealed/read-side shapes live in `ploke_records::{history::payload, selection}` and `ploke-tree::graph::selection`. | Keep History payloads as the durable decision source; graph should expose borrowed witnesses for UI, not parse History JSON in egui. |
| Model request | Live TUI/session code sends `ploke_llm::router_only::ChatCompRequest<R>`; older run records carry `ChatCompReqCore` and agent-turn artifacts carry `llm_prompt`. The replay request tap currently captures request messages, not the full router request. | Add a canonical provider-request record only if playback needs exact request replay. It should be owned in `ploke-records` and joined to agent-turn/provider-response records, not rediscovered from logs. |
| Provider response | `ploke_records::llm_response::RawFullResponseRecord` owns `llm-full-responses.jsonl`; replay and probe code load it directly today. | Load it through the record store as a typed sidecar and join by `(assistant_message_id, response_index)` under the agent-turn/model-exchange playback family. |
| LLM registry caches | `ploke_llm::registry::cache::{EndpointCache, ModelCache}` serialize local `.json` cache files for router discovery. | Treat caches as local configuration/cache surfaces, not run playback authority. Persist selected endpoint/model provenance into run or turn records when a run depends on it. |
| TUI workspace registry | `ploke_tui::user_config::{WorkspaceRegistry, WorkspaceRegistryEntry}` persists saved workspace DB snapshots in `workspaces.toml` under config or `PLOKE_WORKSPACE_REGISTRY_PATH`. | Playback should use eval-run snapshot paths and DB witnesses. It should not depend on the user's global workspace registry unless that registry path is itself recorded as explicit evidence. |

## Surfaces Not Yet Fully In Playback

- `llm-full-responses.jsonl` has an owner type
  `ploke_records::llm_response::RawFullResponseRecord`, but current graph load
  does not carry it beside `agent_turn_records`. Replay code loads it separately.
- The exact model request is still fragmented. `AgentTurnArtifact` captures
  request messages in `llm_prompt`, older run records carry `ChatCompReqCore`,
  and the debug request tap captures messages. That is enough for prompt
  debugging, but not yet enough to prove the complete router-specific request,
  selected endpoint, tools list, or tool-choice policy for every provider call.
- `ploke-llm` endpoint/model caches are serialized local cache files, but they
  are not run-root evidence. Runtime playback should record the model/provider
  decision made for a run or turn instead of treating current cache contents as
  historical truth.
- Runtime channel JSONL is counted but not represented as ordered envelopes.
- `indexing-checkpoint.db`, `indexing-failure.db`, `final-snapshot.db`, and
  Cozo time-travel markers are referenced by run records, but database snapshots
  are not graph facts.
- `ploke-tui` workspace registry files can point at saved DB snapshots, but the
  Prototype 1 loop should replay from run-root snapshots and recorded registry
  evidence. A developer's current `workspaces.toml` is not enough to reconstruct
  historical index/reindex state.
- Protocol artifacts are loaded and summarized, but their inner procedure
  steps are not exposed as runtime playback steps. The latest checked run wrote
  separate `tool_call_intent_segmentation`, `tool_call_review`, and
  `tool_call_segment_review` families; playback should preserve those procedure
  names and counts instead of flattening them into one protocol bucket.
- `benchmark-patch-projection.json` and `multi-swe-bench-submission.jsonl`
  exist in the latest checked run root, but they are not first-class graph
  inputs yet. They should attach to evaluation/benchmark playback as typed
  witnesses over the exported patch and projection check.
- `config/ploke/proposals.json` can appear under a run when the TUI proposal
  path is redirected. Its owner is the TUI `EditProposal` list, but playback
  should only treat it as run evidence when the redirected path or proposal ids
  are recorded by the run.
- Campaign, registry, batch, and `last-run.json` files sit outside the run
  root. The run registration should be discovery/lifecycle authority;
  campaign/batch files are scope inputs or projections; `last-run.json` is an
  operator convenience pointer.
- `record.json.gz` preserves turn/tool content, but runtime playback still
  needs explicit iterators that choose between branch run records and
  agent-turn artifacts without duplicating facts.
- History index files under `history/index/` are projections. The playback
  source should be sealed blocks plus graph indexes, not the projection files.
- The TUI indexing/reindex process emits events and mutates database state, but
  its durable playback surface is currently indirect: indexing status,
  parse-failure artifacts, DB snapshot paths, and run-record time markers.

## Semantic Object Mapping

| Semantic object | Current record families | Preferred graph/playback home |
| --- | --- | --- |
| History lineage and epoch | `SealedBlockRecord`, admitted entries, transition journal citations | `Graph.history`, `Graph.authority`, `RuntimePlaybackRef<HistoryBlocks>` |
| Artifact / Tree state | History artifact refs, `ArtifactId`, tree-key claims, surface evidence, run attempt records | `Graph.artifacts`, artifact ancestry scope |
| Runtime / role state | invocation records, channel envelopes, transition journal runtime events | `Graph.runtimes`, attempt/invocation/channel step families |
| Operation / intervention | `Coordinate`, operation targets in runner requests/invocations, surface records | `Graph.operations`, operation scope |
| Candidate / selection | `SelectionDecisionEntryRecord`, candidate set, memberships, metric set, formula | `Graph.candidates`, `Graph.selections`, `Graph.metrics` |
| Evaluation / benchmark | evaluation artifacts, compared run evidence, `RunMetrics`, oracle records | branch/candidate evidence plus benchmark aggregate views |
| Agent turn / model exchange | `AgentTurnArtifactRecord`, `RawFullResponseRecord`, future request snapshot | nested agent-turn timeline under runtime playback |
| Tool/edit lifecycle | agent-turn events, run-record tool executions, patch artifacts, surface evidence | agent-turn drilldown and edit lifecycle step family |
| Index / reindex state | indexing status, parse failures, Cozo snapshot DBs, time-travel markers | index health aggregate and DB-state drilldown |
