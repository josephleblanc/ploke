# Typed Persistence Traceability Matrix

Updated: 2026-05-10

This matrix bridges the typed persistence inventory to the interactive archive-graph UI contract.

The inventory says which persisted/transmitted surfaces exist. The UI contract says what questions the archive graph must answer. This matrix says which typed facts, joins, and derived evidence are needed to answer each question.

The UI target is an interactive node graph canvas over the growing archive: artifact/runtime nodes, patch/evaluation/successor edges, inspector refs, badges, and a synced timeline strip. Matrix rows should preserve those visual primitives when naming facts and joins.

## Evidence Modes

- `fact`: directly persisted typed record or typed projection.
- `join`: stable typed link between records.
- `replay`: deterministic reconstruction from facts and joins.
- `derived`: computed view over facts, joins, and replay.
- `evidence-strength`: probabilistic or comparative evidence. This can support confidence, not causal certainty.

## Rule

No UI answer is complete merely because an individual record deserializes. A UI answer is complete only when its typed facts, joins, and replay or derived evidence can be produced without owned `serde_json::Value` field walking, stringly JSON payloads, logs, or rendered CLI output.

## Identity Spine

Stable carriers already exist for the core archive graph. Implementation slices
should preserve and reuse these carriers instead of minting local string ids or
new mirror types:

| Identity concept | Canonical carrier for new shared DTOs | UI use | Notes |
|---|---|---|---|
| Runtime instance | `ploke_records::ids::RuntimeId` | runtime node, timeline span owner, tool/LLM/eval context | `ploke-eval::loop_graph` has an older mirror; do not extend the mirror for new shared persisted shapes. |
| Recoverable artifact state | `ploke_records::ids::ArtifactId`; History `ArtifactRefRecord` | graph node, patch base, selected successor target | Current join is string identity between transparent carriers. Add typed helper/projection joins later; do not invent a second artifact identity. |
| Patch record | `ploke_records::ids::PatchId` | patch edge, diff inspector, composability input | The final parent -> patch -> child edge may combine patch, surface commitment, and History entry facts. |
| Candidate occurrence | `CandidateOccurrenceId` | candidate event in History/fine playback | Distinct from candidate-set membership. |
| Candidate membership and set | `CandidateMembershipId`, `CandidateSetRecord`, `CandidateSetMembershipRecord`, candidate-set root/commitment | selection edge and "selected from which set" proof | There is no standalone `CandidateSetId` today; do not invent one during cleanup slices unless a later replay slice proves it is needed. |
| History lineage | `EntryId`, `BlockId`, `LineageId`, `BlockHash`, `HistoryStateRoot` | sealed spine, genesis/backward replay | Existing sealed playback carriers are established; UI joins must not parse rendered playback output. |
| Parent identity | `ParentIdentityRecord`, `ParentIdentityRefRecord` | parent-capable artifact identity and parent label | Parent identity anchors on node/parent/campaign/branch fields. Runtime joins come from adjacent runtime-bearing records, not from `ParentIdentityRecord` itself. |
| Selected successor | `SuccessorRefRecord` plus History selection payload | selected successor edge | `SuccessorRefRecord` gives runtime/artifact; the selection payload gives selected occurrence/membership and candidate-set proof. |

## Protocol Artifact Pre-Slice Decisions

These decisions constrain `protocol-artifacts.decode`:

- The protocol artifact coordinate for slice 1 is `run_id`, `subject_id`,
  artifact file path or `ProtocolArtifactSummaryRecord.path`,
  `procedure_name`, and record metadata such as `created_at_ms`,
  model/provider when present. There is no standalone protocol-artifact id
  today.
- Protocol payload bodies are typed inspectable facts. They may support
  parent/child/patch reasoning only through explicit typed citations or
  adjacent typed records; the decode slice must not infer graph lineage from
  filenames, newest-artifact ordering, rendered reports, or anonymous JSON.
- Malformed owned protocol payloads should deserialize into a named typed
  parse/error record carrying the protocol coordinate, expected payload kind,
  and structured error detail. They should not be preserved as production
  `serde_json::Value` for later field walking.
- Archive-graph joins from protocol artifacts to History entries, evaluations,
  tool calls, parent/child artifacts, or patches are not complete in slice 1.
  They are explicit follow-up facts for `protocol-artifacts.storage-aggregate`
  and later replay/identity slices.
- Later protocol joins should stay compound unless a replay slice proves a
  standalone protocol artifact id is necessary. Use History `EntryId` from
  admitted entries; evaluation identity from evaluation procedure, eval-set,
  branch, and artifact citation; tool-call identity from `run_id`,
  `subject_id`, and call index when no persisted call uuid exists; artifact
  identity from artifact refs and intervention base/derived artifact ids; patch
  identity from typed `PatchId` fields in surface/intervention records.

## Projection Identity Decisions

These decisions constrain later archive-graph and UI projection slices. They do
not block `protocol-artifacts.decode`.

- `GraphNode` and `GraphEdge` are projection primitives, not new source
  authority. The UI may materialize them from typed records and joins, but the
  durable facts remain the underlying History, scheduler, artifact, evaluation,
  patch, protocol, tool, and metric records.
- `GraphNode` should project artifact/runtime/agent-instance identity from
  existing typed carriers. Do not mint local string ids for nodes. The exact
  Rust projection shape is deferred to `evaluation-oracle-targets.identity-joins`
  and `runtime-artifact-lineage`.
- `GraphEdge` should preserve relation roles instead of flattening the archive
  into one parent-child edge. Parent -> patch -> child is a composed path across
  patch/self-modification, candidate membership, successor selection, History
  lineage, evaluation, and later merge/composability relations.
- Candidate set identity is candidate-set root/commitment plus membership
  records. There is no standalone `CandidateSetId` today. Do not add one until
  `candidate-frontier.replay` or `evaluation-oracle-targets.selection-replay`
  proves a separate set handle is necessary.
- Timeline projections should prefer causal order over timestamps. Timestamps
  are audit facts; nesting and sequencing should come from typed source refs,
  parent span refs, and causal order keys.
- Patch impact projections need first-class hunk/range/code-graph-item identity
  before the UI can explain changed loci. Existing patch/file/raw-diff records
  are not enough by themselves.
- Comparison projections need role-aware comparison envelopes: subject ids,
  comparator cohort, baseline anchor, metric or locus id, repeat/baseline
  evidence, provenance refs, and evidence strength. They must report observed
  deltas and support for experiments without claiming unsupported causality.

## Projection Carrier Decisions

These answers close the pre-implementation questions enough to guide later
slices without adding new source-authority records early:

- Shared persisted DTOs use `ploke-records::ids`. `ploke-eval::loop_graph`
  remains a live/local compatibility mirror until callers migrate behind narrow
  adapters. Do not extend it for new shared persisted shapes.
- Tool-call arguments should become a closed typed argument enum or record set
  keyed by tool name, with typed parse-failure records. `ToolCallRecord` and
  `ToolRequestRecord` should converge on that carrier for owned persisted
  reads; provider ingress may parse external JSON, but persisted Ploke-owned
  reads must not expose anonymous JSON as the contract.
  - Current code state for cold restart: `ploke-tui` now exposes its owned tool
    transport DTOs behind a `tool_contracts` feature, and
    `ploke-records` has a feature-gated `tool_contracts` re-export module for
    those DTOs. The next implementation step is to define the persisted
    tool-call carrier over those re-exported DTOs; do not mirror the DTOs in
    `ploke-records`, and do not split or feature-gate the `ploke-tui` runtime
    as part of this lane.
- LLM/provider observations split into typed provider attempts, typed response
  summaries, typed provider error/timeout records, and typed tool-bridge
  records. `ProviderAttempt` is already typed; logprobs, provider metadata, and
  function-call argument strings need typed projection records before UI/replay
  consumes them.
- Timeline spans are derived playback projections, not persisted authority.
  Shape them as `TimelineSpan` / `TimelineSpanRef` with span id, lane/kind,
  causal order, optional start/end, source ref, parent span, and evidence
  strength. Use wall-clock spans where facts exist, duration-relative spans
  where only latency exists, and zero-length point spans for timestamped facts.
- Patch/code graph impact is a patch-scoped read projection: patch id,
  base/after artifact refs, touched files, hunk/range spans, hashes,
  replacement or diff inspector refs, directly modified code graph items, and
  composability inputs. Keep diff display separate from impact facts.
- Comparison/evidence analysis uses role-aware envelopes over typed facts:
  subject/locus, comparator cohort, metrics and deltas, support kind,
  provenance refs, and evidence strength. Roles must distinguish observed
  correlation, baseline/repeated-eval support, and actual causal claims; current
  records support the first two but not causal proof.

## Matrix

| UI contract rows | Evidence mode | Typed facts needed | Required joins | Inventory rows | Closing slices | UI answer produced | Verification target |
|---|---|---|---|---|---|---|---|
| `ui.protocol.outputs`, `ui.parent.child.patch.reason` | `fact`, `join`, `replay` | Protocol artifact input/output payloads, typed protocol parse/error records, stored artifact records, aggregate outputs, parent/child artifact refs | protocol coordinate (`run_id`, `subject_id`, path, procedure), parent artifact id, child artifact id, history entry id | `protocol.artifact.decode`, `protocol.artifact.store`, `protocol.artifact.aggregate.output`, `protocol.artifact.playback` | `protocol-artifacts.decode`, `protocol-artifacts.storage-aggregate` | Show typed protocol inputs/outputs and how cited protocol facts feed parent-child patch reasoning without inferring graph lineage from artifact filenames or reports. | Typed protocol payload fixture deserializes into payload variants or typed parse/error records; replay test uses adjacent typed joins for any parent/child edge. |
| `ui.tool.calls`, `ui.child.self.eval.actions`, `ui.live.progress` | `fact`, `join`, `replay` | Tool call records, typed arguments, tool request/result records, tool execution records, typed trace projections | tool call id, request id, execution id, attempt id, evaluation id, child artifact id | `tool.call.record.arguments`, `tool.request.arguments.capture`, `tool.execution.record`, `tool.response.full_response_trace`, `tool.result.trace.projection` | `tool.call.arguments`, `tool.result.trace.projection` | Show each tool call with typed arguments, typed return, execution status, and parent evaluation context. | Real-run or fixture parse reconstructs child evaluation -> attempt -> tool call -> result. |
| `ui.provider.attempts`, `ui.child.self.eval.actions`, `ui.live.progress` | `fact`, `join`, `replay` | LLM request/response records, provider error records, timeout records, attempt timeline records, tool bridge records | attempt id, provider request id, response id, model id, tool call id, child/evaluation id | `llm.attempt.request`, `llm.attempt.response`, `llm.attempt.provider_error`, `llm.attempt.timeout`, `llm.attempt.timeline`, `llm.dto.openai_response`, `llm.tool_bridge.records` | `llm-attempts.provider-observation-projection`, `llm-attempts.dto-tool-bridge` | Show provider/model attempts, errors, timeouts, responses, and linked tool calls. | Typed attempt projection reconstructs request -> response/error/timeout -> tool bridge without JSON field walking. |
| `ui.write.surface.evidence`, `ui.approval.path`, `ui.patch.diff.metadata`, `ui.patch.diff.view` | `fact`, `join`, `replay` | Surface grant/check records, checked surface evidence, attempt/commitment records, patch artifact records, proposal refs | surface id, grant/check id, proposal id, commitment id, patch id, parent artifact id, invocation id | `edit_surface.grant_check`, `edit_surface.checked_surface_evidence`, `edit_surface.surface_evidence_record`, `edit_surface.surface_attempt_record`, `edit_surface.candidate_artifact_record`, `edit_surface.surface_commitment_record`, `edit_surface.parent_identity_record`, `edit_surface.invocation_record`, `edit_surface.proposal_registry`, `edit_surface.patch_artifact` | `edit-surface.source-records`, `patch.diff-code-graph-impact` | Show why a write surface was selected, what proposal/commitment approved it, and what diff was applied. | Typed replay links surface evidence -> proposal/check -> commitment -> patch diff. |
| `ui.database.context`, `ui.write.surface.evidence` | `fact`, `join`, `replay` | Intent/tool result records, context assembly records, embedding/node refs, UI context projection | context bundle id, query id, node id, prompt id, evaluation id, tool call id | `db.context.intent_and_tool_result`, `db.context.assembly`, `db.context.embedding_and_node_refs`, `db.context.ui_projection` | `database-context.prompt-evidence` | Show which database context was added and why it was included. | Typed replay links prompt/evaluation/tool call to context bundle and each cited node. |
| `ui.oracle.target`, `ui.successor.selection`, `ui.selection.candidate.set` | `fact`, `join`, `replay` | Eval target identity records, artifact branch records, instance registry, scheduler state, selection DTO, History selection payload, metrics/evidence records | evaluation id, target artifact id, instance id, branch id, candidate set root/commitment, candidate membership id, selected artifact id, metrics run id | `eval.artifact.branch`, `eval.metrics.run`, `eval.selection.dto`, `eval.history.payload.selection`, `eval.history.evidence`, `eval.instance.registry`, `eval.scheduler.state`, `eval.run_record.metadata_setup`, `eval.run_record.patch_packaging`, `eval.tree.playback` | `evaluation-oracle-targets.identity-joins`, `evaluation-oracle-targets.selection-replay`, `candidate-frontier.replay` | Show oracle target, complete candidate set, selected successor, and evidence used for selection. | Typed replay proves selected membership belongs to the final decision candidate set. |
| `ui.child.lineage.genesis` | `fact`, `join`, `replay` | Artifact branch records, runtime hydration records, parent/child History entries, patch records, successor selection records | child artifact id, parent artifact id, runtime id, patch id, predecessor artifact id, genesis artifact id, history entry id | `eval.artifact.branch`, `eval.selection.dto`, `eval.history.payload.selection`, `edit_surface.patch_artifact`, `eval.tree.playback` | `runtime-artifact-lineage`, `evaluation-oracle-targets.identity-joins`, `evaluation-oracle-targets.selection-replay` | For a child, show the runtime/artifact/patch chain back to genesis. | Typed lineage lookup starts at child artifact id and returns the full chain to genesis. |
| `ui.successor.candidate.frontier`, `ui.selection.candidate.set` | `fact`, `join`, `replay` | Candidate set records, candidate membership records, evaluation records, metrics records, child artifact records | parent artifact id, candidate set root/commitment, candidate membership id, child artifact id, evaluation id, metrics run id | `eval.selection.dto`, `eval.history.payload.selection`, `eval.history.evidence`, `eval.metrics.run`, `eval.scheduler.state` | `candidate-frontier.replay`, `evaluation-oracle-targets.selection-replay` | Show every candidate considered and the exact set from which the successor was selected. | Typed replay lists candidates before selection and verifies selected membership role. |
| `ui.timeline.concurrency` | `fact`, `join`, `replay`, `derived` | Runtime lifecycle records, scheduler records, child run records, LLM attempt records, tool execution records, metrics/evaluation records | run id, parent artifact id, child artifact id, evaluation id, attempt id, tool call id, span id, causal parent span id | `prototype1.scheduler_json`, `prototype1.node_request_projection`, `prototype1.runner_result_projection`, `prototype1.metrics_projection`, `prototype1.agent_turn_trace`, `prototype1.observation_jsonl`, `tool.execution.record`, `llm.attempt.timeline` | `timeline.concurrency`, `tool.result.trace.projection`, `llm-attempts.provider-observation-projection` | Render a bar-chart-like timeline showing parallel and sequential work down to tool calls. | Typed span projection renders nested/overlapping parent, child, attempt, and tool-call spans. |
| `ui.patch.code.graph.impact`, `ui.patch.diff.view` | `fact`, `join`, `replay`, `derived` | Patch artifact records, diff hunks, file snapshot refs, code graph node records, parsed item spans, artifact surface records | patch id, file path, byte range/span id, code graph node id, module/function/type id, artifact id, surface root id | `edit_surface.patch_artifact`, `edit_surface.candidate_artifact_record`, `eval.run_record.patch_packaging`, `eval.tree.playback` | `patch.diff-code-graph-impact`, `edit-surface.source-records` | Show the actual diff and the functions/types/modules directly modified by a patch. | Typed projection maps patch hunks to code graph items and returns diff refs/content. |
| `ui.parent.value.added`, `ui.patch.improvement.locus` | `fact`, `join`, `derived`, `evidence-strength` | Parent/child artifact records, patch impact records, evaluation metrics, baseline metrics, repeated eval records when available, descendant outcome records | parent artifact id, child artifact id, patch id, code graph node id, metric id, eval target id, baseline id, lineage edge id | `eval.metrics.run`, `eval.selection.dto`, `eval.history.evidence`, `eval.tree.playback`, patch/code graph impact rows after slice 14 | `score.value-locus-analysis`, `patch.diff-code-graph-impact`, `runtime-artifact-lineage` | Compare observed score deltas by parent, patch, and code locus; surface candidate experiments and evidence strength. | Typed analysis reports observed deltas and repeated/baseline distributions without claiming unsupported causality. |
| `ui.patch.composability`, `ui.child.mergeability` | `fact`, `join`, `derived` | Patch artifact records, diff hunks, code graph impact records, lineage records, artifact surface records, apply/check records | patch id A/B, child artifact id A/B, nearest common ancestor id, touched file paths, code graph node ids, surface root ids | `edit_surface.patch_artifact`, `eval.artifact.branch`, `eval.tree.playback`, patch/code graph impact rows after slice 14 | `patch.child-composability`, `runtime-artifact-lineage`, `patch.diff-code-graph-impact` | Classify whether patches or children are composable, conflicting, order-dependent, or unknown. | Typed compatibility check explains shared base, overlapping loci, conflicts, and check evidence. |

## Graph Primitive Coverage

Implementation slices should preserve these canvas primitives:

| Graph primitive | Required typed support | Main slices |
|---|---|---|
| `GraphNode` | Artifact/runtime identity, evaluation status, score summaries, parent/child lineage refs | `evaluation-oracle-targets.identity-joins`, `runtime-artifact-lineage`, `evaluation-oracle-targets.selection-replay` |
| `GraphEdge` | Patch/self-modification, candidate membership, successor selection, evaluation relation, merge/composability relation | `candidate-frontier.replay`, `evaluation-oracle-targets.selection-replay`, `patch.child-composability` |
| `NodeBadge` | Score delta, selected/not-selected state, running/failed status, evidence strength | `score.value-locus-analysis`, `timeline.concurrency`, `evaluation-oracle-targets.selection-replay` |
| `EdgeBadge` | Patch id, changed item count, selection reason, compatibility/conflict status | `patch.diff-code-graph-impact`, `candidate-frontier.replay`, `patch.child-composability` |
| `InspectorRef` | Stable ids for protocol artifacts, diffs, tool calls, DB context, metrics, evidence, and lineage | All typed persistence cleanup slices, especially `protocol-artifacts.*`, `tool.*`, `database-context.prompt-evidence`, `patch.diff-code-graph-impact` |
| `TimelineSpanRef` | Typed span ids connected to parent, child, attempt, tool call, evaluation, and patch-apply activity | `timeline.concurrency`, `tool.result.trace.projection`, `llm-attempts.provider-observation-projection` |

## How To Use This Matrix

For implementation work:

1. Pick the current `next` row from [`implementation-slices.md`](implementation-slices.md).
2. Find every matrix row that names that slice.
3. Implement the typed facts and joins first.
4. Add deterministic replay tests for `fact` / `join` / `replay` rows.
5. Add derived evidence tests for `derived` rows.
6. For `evidence-strength` rows, preserve uncertainty explicitly and avoid causal language unless a typed experimental record supports it.

For UI planning:

1. Pick a UI contract row from [`ui-drilldown-contract.md`](ui-drilldown-contract.md).
2. Use this matrix to identify its required typed facts, joins, and slices.
3. If a required fact or join is not in the inventory, add a new inventory row or implementation slice before designing the panel as if the data exists.
