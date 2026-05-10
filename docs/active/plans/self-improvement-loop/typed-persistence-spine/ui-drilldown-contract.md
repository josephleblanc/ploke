# UI Drilldown Data Contract

Updated: 2026-05-10

This document defines the typed data the interactive archive-graph UI must be able to reconstruct from Prototype 1 persisted records.

The inventory answers "what surfaces exist?" The implementation queue answers "what do we fix next?" This contract answers "what must the UI be able to explain?"

## Rule

Every visible UI drilldown must be backed by named Rust `Serialize` / `Deserialize` records and stable typed joins. The UI may render projections, but it must not recover meaning by parsing owned JSON through `serde_json::Value`, anonymous field walking, stringly JSON payloads, logs, or rendered CLI output.

## Primary Interaction Model

The primary UI is an interactive archive graph canvas, similar in interaction style to node-diagram tools such as LangChain, n8n, or React Flow. It is not a log viewer and not a static tree widget.

The canvas shows the growing archive of artifacts/runtimes/agents. Nodes and edges are selectable, hoverable, pannable, and zoomable. Selecting a node or edge opens inspector panels backed by typed records.

The primary graph shape is:

```text
genesis
  -> selected parent artifact/runtime
    -> child candidate artifact/runtime
      -> patch / self-modification edge
      -> evaluation result
    -> child candidate artifact/runtime
      -> patch / self-modification edge
      -> evaluation result
    -> selected successor edge
      -> next parent artifact/runtime
```

Sibling candidates remain visible as branches. Selected successor edges are visually distinguished from rejected or merely evaluated candidates.

Visual primitives:

- `GraphNode`: artifact/runtime/agent instance.
- `GraphEdge`: lineage, patch/self-modification attempt, candidate relation, successor selection, evaluation relation, or merge/composability relation.
- `NodeBadge`: score delta, evaluation status, selected/not-selected marker, running/failed state, evidence strength, confidence marker.
- `EdgeBadge`: patch id, changed file/item count, selection reason, compatibility/conflict status, evaluation outcome.
- `InspectorRef`: stable typed ids for loading protocol artifacts, tool calls, diffs, database context, metrics, evidence, and lineage.
- `TimelineSpanRef`: span ids that connect graph nodes/edges to the bottom timeline.

A minimal timeline strip is visible by default near the bottom of the screen, like a compact audio/mixing timeline. It can be expanded into a full timeline view or toggled off. Timeline segments are color-coded by kind/status, hoverable for details, and synced with selected graph nodes/edges.

The UI does not need one physical record per graph primitive. It needs enough typed source records and join keys to construct the graph and inspectors without guessing.

Projection constraints:

- Graph primitives are UI projections over typed records, not source authority.
- `GraphNode` projects artifact/runtime/agent-instance identity. Inline fields
  should stay small; detailed facts belong behind typed `InspectorRef` and
  `TimelineSpanRef` joins.
- `GraphEdge` preserves relation roles. A parent -> patch -> child path may be
  rendered as one visual route, but the data model must keep patch,
  candidate-membership, successor-selection, evaluation, History-lineage, and
  merge/composability relations distinguishable.
- Candidate-set selection uses candidate-set root/commitment plus membership
  ids. A standalone `CandidateSetId` is not required for current replay proofs.
- Timeline spans use causal order and typed source refs as the primary
  sequencing model. Timestamps support audit and rendering, but must not be the
  only way to infer nesting or concurrency.
- Patch/code-graph panels need hunk/range/code-item identities, not only raw
  unified diff text.
- Comparison panels expose observed deltas, repeated/baseline support, and
  evidence strength. They do not convert correlation into causality unless a
  later typed experimental record supports that claim.

## Projection Carrier Decisions

These decisions keep the UI contract inspectable without making projections
into source authority:

- `ParentIdentityRecord` labels a parent-capable artifact through node, parent,
  campaign, and branch fields. Runtime identity is joined from adjacent
  invocation, handoff, ready, completion, or evidence records.
- Protocol inspectors start from the protocol coordinate: run id, subject id,
  path, procedure, created timestamp, model, and provider. History, evaluation,
  tool-call, artifact, and patch joins are shown only when adjacent typed
  records provide them.
- Tool-call panels display a typed argument variant or typed parse-failure
  record, the typed result, execution status, provider attempt link, and child
  or evaluation context.
- Provider panels display typed attempts, typed response summaries, typed
  errors/timeouts, and typed tool-bridge records. They do not walk provider
  response JSON directly.
- Timeline panels render derived `TimelineSpan` / `TimelineSpanRef` projections.
  Causal order and source refs drive nesting; timestamps and latencies refine
  placement when available.
- Patch impact panels separate diff rendering from impact facts. Impact facts
  include patch id, base/after artifact refs, touched file paths, ranges,
  hashes, directly modified code graph items, and composability inputs.
- Comparison panels use role-aware evidence envelopes. The UI can show observed
  correlation and repeated/baseline support today; it must reserve causal claims
  for future typed experimental records that actually support them.

## Archive Graph Model

The graph exposes the loop as typed archive exploration:

```text
Run archive
  Parent artifact/runtime node
    Child artifact/runtime candidate nodes
      Patch / proposal / edit surface edge
      Protocol artifact inspector
      LLM attempt and tool-call inspector
      Database context inspector
      Self-evaluation target and outcome inspector
    Successor selection edge
      Candidate evidence inspector
      Metrics and evaluation report inspector
      Selected artifact/runtime node
```

## UI Answer Contracts

Each UI question needs an explicit answer contract before a slice can claim it
is UI-ready. The contract is:

- Operator question: the question a person asks while inspecting the run.
- Answer object: the typed projection the UI can render.
- Source facts: named persisted records or typed projections.
- Joins: stable ids that connect the facts.
- Derived computation: replay, comparison, timeline folding, impact analysis,
  or compatibility analysis.
- Evidence strength: whether the answer is a fact, replay, correlation,
  baseline-supported result, or later causal claim.
- Closure test: the smallest test that reconstructs the answer without raw
  JSON, logs, or rendered CLI output.

The current answer contracts are:

### Lineage And Selection

- Given a child, what runtimes, artifacts, and patches led to it back to
  genesis?
  - Answer object: `LineagePath`, a derived graph path over artifacts,
    runtimes, patches, History entries, and successor selections.
  - Still to pin down in implementation: exact `GraphNode`/`GraphEdge`
    projection structs, the typed helper for joining `ArtifactId` to
    `ArtifactRefRecord`, and the first deterministic lineage replay test.
  - Closing slices: `runtime-artifact-lineage`,
    `evaluation-oracle-targets.identity-joins`,
    `evaluation-oracle-targets.selection-replay`.

- What are all candidates for the next successor?
  - Answer object: `CandidateFrontier`, the final typed candidate set plus
    evaluated or pending candidate artifacts/runtimes.
  - Still to pin down in implementation: frontier projection shape and strict
    separation of source-set membership from decision-set membership.
  - Closing slices: `candidate-frontier.replay`,
    `evaluation-oracle-targets.selection-replay`.

- How did this parent select the next successor, and from among which exact
  candidates?
  - Answer object: `SelectionReplay`, including candidate-set root/commitment,
    role-specific memberships, selected membership, metrics/evidence, and
    successor runtime/artifact.
  - Still to pin down in implementation: replay API shape and acceptance test
    proving selected membership belongs to the final decision candidate set.
  - Closing slices: `evaluation-oracle-targets.selection-replay`,
    `candidate-frontier.replay`.

### Patch And Edit Surface

- What evidence selected this write surface, and why did the parent produce
  this patch?
  - Answer object: `WriteSurfaceEvidencePath`, a replay path from evidence and
    diagnosis to grant/check, proposal, commitment, patch, and History/eval
    links.
  - Still to pin down in implementation: edit-surface source-record boundary,
    proposal registry projection boundary, and typed database-context bundle
    joins.
  - Closing slices: `edit-surface.source-records`,
    `database-context.prompt-evidence`,
    `evaluation-oracle-targets.identity-joins`.

- What diff was applied, and what files or code graph items did it directly
  modify?
  - Answer object: `PatchImpact`, separating diff display from impact facts.
    Impact facts include patch id, base/after artifact refs, touched files,
    ranges, hashes, code graph item refs, and direct-modified markers.
  - Still to pin down in implementation: first-class hunk/range/code-item
    projection type, stable code graph item id home, and typed diff inspector
    boundary.
  - Closing slices: `patch.diff-code-graph-impact`,
    `edit-surface.source-records`.

### Child Evaluation, Tools, And Provider Attempts

- After the patch was applied, what did the child do during self-evaluation?
  - Answer object: `EvaluationRunTrace`, an ordered trace of child eval
    actions, LLM attempts, tool calls/results, protocol outputs, metrics, and
    final outcome.
  - Still to pin down in implementation: persisted tool-argument variants over
    `ploke_records::tool_contracts` DTOs, typed tool parse-failure records,
    typed provider observation records, and the join from child/evaluation
    context to attempts and tool calls.
  - Closing slices: `tool.call.arguments`, `tool.result.trace.projection`,
    `llm-attempts.provider-observation-projection`,
    `llm-attempts.dto-tool-bridge`.

- What oracle target was used for child self-evaluation?
  - Answer object: `OracleTarget`, a typed projection over evaluation
    procedure, eval-set identity, branch/instance identity, artifact refs, and
    surface roots.
  - Still to pin down in implementation: compound evaluation identity helper
    and lookup path from evaluation artifacts to target artifact/runtime.
  - Closing slice: `evaluation-oracle-targets.identity-joins`.

- What additional database context was added?
  - Answer object: `ContextBundle`, linking prompt/evaluation/tool call to
    retrieved code graph nodes, embedding/search refs, and inclusion reasons.
  - Still to pin down in implementation: context bundle identity and typed
    prompt-evidence joins.
  - Closing slice: `database-context.prompt-evidence`.

### Protocol Outputs

- What protocol outputs were used to derive a choice?
  - Answer object: `ProtocolOutputPanel`, keyed by protocol coordinate and
    typed payload variant or typed parse-failure record.
  - Still to pin down in implementation: exact Rust parse/error record shape
    for malformed payloads and typed decode without `serde_json::Value`
    staging.
  - Closing slices: `protocol-artifacts.decode`,
    `protocol-artifacts.storage-aggregate`.

- Do protocol outputs themselves prove parent/child/patch lineage?
  - Answer: no. Protocol payloads are typed inspectable facts. Lineage,
    evaluation, tool-call, artifact, and patch joins come from adjacent typed
    records unless a later replay slice proves a dedicated protocol join record
    is necessary.

### Timeline

- What happened in parallel or sequentially, down to individual tool calls?
  - Answer object: `TimelineProjection`, a derived set of `TimelineSpan` /
    `TimelineSpanRef` records with span id, lane/kind, causal order, optional
    start/end, source ref, parent span, and evidence strength.
  - Still to pin down in implementation: exact span structs, handling of
    absolute spans vs duration-relative spans, zero-length point spans, and
    source refs for tool/LLM/eval events.
  - Closing slices: `timeline.concurrency`, `tool.result.trace.projection`,
    `llm-attempts.provider-observation-projection`.

### Comparison, Locus Analysis, And Mergeability

- How does this parent compare to other parents by value added to child
  scores?
  - Answer object: `ParentValueComparison`, a role-aware comparison over child
    score deltas, comparator cohort, baseline anchor, metric dimensions, and
    evidence strength.
  - Still to pin down in implementation: comparison envelope Rust shape and
    baseline/repeated-eval support records.
  - Closing slice: `score.value-locus-analysis`.

- Which files or code graph items correlate with improvements, and which other
  patches touch the same loci?
  - Answer object: `LocusEvidence`, joining patch impact, lineage, evaluation
    deltas, repeated/baseline runs, and other patches touching the same code
    graph items.
  - Still to pin down in implementation: patch impact projection first, then
    evidence-strength rules for correlation vs repeated-baseline support.
  - Closing slices: `patch.diff-code-graph-impact`,
    `score.value-locus-analysis`, `runtime-artifact-lineage`.

- Are two patches composable, or can two children be merged?
  - Answer object: `CompatibilityResult`, explaining shared base, divergent
    patch chains, overlapping files/ranges/code items, apply-order constraints,
    conflicts, checks, and unknowns.
  - Still to pin down in implementation: compatibility classifier inputs and
    typed representation of unknown/order-dependent outcomes.
  - Closing slices: `patch.child-composability`,
    `patch.diff-code-graph-impact`, `runtime-artifact-lineage`.

## Required Drilldowns

| ID | UI question | Tree node or panel | Source records | Required join keys | Required payloads | Current coverage | Gap slice | Verification target |
|---|---|---|---|---|---|---|---|---|
| `ui.parent.child.patch.reason` | This parent produced a patch for this child. Why? | Parent -> child -> patch edge | Parent History entry, protocol artifacts, edit-surface evidence, patch artifact, candidate/evaluation records | run id, parent artifact id, child artifact id, history entry id, patch id, protocol coordinate | parent decision evidence, target surface, produced patch, candidate identity, evaluation summary | Partial: edit-surface and evaluation records are typed; protocol payloads still stage through raw JSON and protocol graph joins are incomplete. | `protocol-artifacts.decode`, `protocol-artifacts.storage-aggregate`, `edit-surface.source-records`, `evaluation-oracle-targets.selection-replay` | Typed reconstruction test can load a parent-child-patch path and display evidence, patch, and outcome using adjacent typed joins rather than inferring lineage from protocol artifact filenames. |
| `ui.write.surface.evidence` | What evidence selected this write surface? | Patch detail -> surface evidence | Surface grant/check records, checked surface evidence, DB context refs, diagnosis records, candidate membership/evidence | surface id, grant/check id, evidence ids, parent history entry id, context bundle id | evidence list, rejected/accepted surface reasons, protected-core path, diagnostic classification | Partial: edit-surface records are typed; ownership and replay joins remain scattered. | `edit-surface.source-records`, `database-context.prompt-evidence` | Typed projection can show surface choice and all cited evidence ids without reading TUI-local state or logs. |
| `ui.protocol.outputs` | What protocol outputs were used to derive the choice? | Protocol artifact panel | Protocol artifact records, stored artifact records, aggregate output records | protocol coordinate (`run_id`, `subject_id`, path, procedure), parent/child artifact id, history entry id | typed protocol input, typed protocol output, derived evidence, parse/error record if malformed | Non-compliant: decode/store/aggregate still retain or stage `serde_json::Value`; no standalone protocol-artifact id exists today. | `protocol-artifacts.decode`, `protocol-artifacts.storage-aggregate` | `ploke-records` test deserializes real protocol artifacts into typed payload variants or typed parse/error records and rejects mismatched nested shapes. |
| `ui.patch.diff.metadata` | What files did the patch touch, and what metadata was produced with it? | Patch detail -> diff/files panel | Patch artifact record, edit-surface attempt/commitment records, candidate artifact record, parent identity/invocation records | patch id, candidate artifact id, surface id, invocation id, parent artifact id | changed paths, diff summary or patch body ref, generated metadata, invocation provenance | Mostly typed; replay source ownership still needs cleanup. | `edit-surface.source-records`, `evaluation-oracle-targets.selection-replay` | Typed replay can map selected patch to files touched, candidate artifact, and parent invocation. |
| `ui.approval.path` | How did the parent decide to apply or approve this patch through the edit surface? | Patch -> approval path | Surface grant/check records, attempt/commitment records, History selection/admission records, ploke-tui proposal registry projection | grant/check id, proposal id, commitment id, history entry id, parent artifact id | grant result, checked surface evidence, proposal state, commitment outcome, parent admission link | Partial: typed edit-surface records exist; proposal registry remains local to TUI/eval boundary. | `edit-surface.source-records`, `evaluation-oracle-targets.identity-joins` | Typed path links proposal -> check -> commitment -> History/evaluation without treating TUI projection as source truth. |
| `ui.child.self.eval.actions` | After the patch was applied, what did the child do during self-evaluation? | Child -> self-evaluation timeline | Evaluation run records, LLM attempt records, tool request/result records, protocol artifacts, metrics records | child artifact id, evaluation id, attempt id, tool call id, protocol coordinate, metrics run id | ordered attempts, tool calls, responses, errors/timeouts, evaluation observations, final metrics | Mixed: evaluation records typed; LLM/tool projections still contain raw or stringly JSON. | `tool.call.arguments`, `tool.result.trace.projection`, `llm-attempts.provider-observation-projection`, `llm-attempts.dto-tool-bridge` | Typed replay can show child evaluation timeline with typed attempts, tool calls, results, and metrics. |
| `ui.oracle.target` | During child self-evaluation, what target instance was used as the oracle patch target? | Evaluation target panel | Eval target identity records, artifact branch records, instance registry, scheduler state, run metadata | evaluation id, target artifact id, instance id, branch id, surface root id | oracle target identity, selected artifact root, branch/worktree identity, target surface roots | Typed but scattered. | `evaluation-oracle-targets.identity-joins` | Typed lookup from evaluation id returns oracle target artifact/runtime and surface roots. |
| `ui.tool.calls` | What did each tool call look like, including typed arguments and typed return? | Tool-call panel | Tool call record, tool request record, tool execution record, tool result record, provider attempt record | tool call id, request id, execution id, attempt id, child/evaluation id | typed arguments, typed result, execution status, provider response link, typed error if malformed | Partially prepared: `ploke-tui` owned transport DTOs are exposed through `ploke_records::tool_contracts`; persisted records still need a closed carrier and parse-failure record instead of raw `String` / `serde_json::Value`. | `tool.call.arguments`, `tool.result.trace.projection`, `llm-attempts.dto-tool-bridge` | Roundtrip and real-run parse tests cover each owned tool argument/result shape through the `ploke-records` re-export. |
| `ui.database.context` | What additional context was added from the database? | Context panel | Intent/tool result records, context assembly records, embedding/node ref records, UI context projection | context bundle id, query id, node ids, prompt id, evaluation id, tool call id | query text or intent, retrieved node refs, embedding/search refs, assembled prompt context, inclusion reason | Typed surfaces exist; prompt-evidence and replay joins need unification. | `database-context.prompt-evidence` | Typed replay links prompt/evaluation/tool call to DB context bundle and each cited node. |
| `ui.provider.attempts` | What provider/model attempts happened, and how did errors or timeouts affect the run? | LLM attempt panel | LLM request/response records, provider error records, timeout records, attempt timeline records, full response logs | attempt id, provider request id, response id, model id, child/evaluation id | provider/model, request metadata, response summary, timeout/error type, typed observation timeline | Non-compliant: provider observation/timeline projections still use raw or stringly JSON. | `llm-attempts.provider-observation-projection`, `llm-attempts.dto-tool-bridge` | Typed attempt projection can show request, response, error/timeout, and timeline without JSON field walking. |
| `ui.successor.selection` | Why was this successor selected over other candidates? | Successor selection panel | Selection DTO, History selection payload, evidence records, metrics run, tree playback projection | run id, candidate set root/commitment, candidate membership id, selected artifact id, metrics run id, history entry id | candidate list, scores/metrics, evidence refs, selected membership, final selected artifact/runtime | Typed but scattered; recent work improved successor identity but replay joins still need consolidation. | `evaluation-oracle-targets.identity-joins`, `evaluation-oracle-targets.selection-replay` | Typed replay can show all candidates, their evidence/metrics, and the selected successor identity. |
| `ui.live.progress` | What is happening right now during a live run? | Live run tree/progress view | Typed monitor/projection records, scheduler/node/result/metrics projections, History-backed records where admitted | run id, parent artifact id, child artifact id, node request id, result id, metrics id | live status, current node/attempt, latest typed observation, provisional projection markers | Partial: scheduler/node/result/metrics projections typed; preview/trace/observation/slice readers still need typed projection records. | `tool.result.trace.projection`, `llm-attempts.provider-observation-projection` | Live monitor can consume typed projection records and mark them as projection, not source authority. |
| `ui.child.lineage.genesis` | For a given child, what combination of runtimes, artifacts, and patches led to that child, tracing back to genesis? | Child -> ancestry path | Artifact branch records, runtime hydration records, parent/child History entries, patch artifact records, successor selection records | child artifact id, parent artifact id, runtime id, patch id, predecessor artifact id, genesis artifact id, history entry id | ordered ancestry chain, patch per edge, runtime per artifact, selected successor per generation, genesis root | Partial: artifact/evaluation records exist; full cross-generation lineage projection is not yet a named typed view. | `runtime-artifact-lineage`, `evaluation-oracle-targets.identity-joins`, `evaluation-oracle-targets.selection-replay` | Typed lineage lookup can start from a child artifact id and reconstruct artifact/runtime/patch edges back to genesis. |
| `ui.successor.candidate.frontier` | What are all the candidates for a possible next successor? | Parent -> candidate frontier | Candidate set records, candidate membership records, scheduler state, evaluation records, metrics records, child artifact records | parent artifact id, candidate set root/commitment, candidate membership id, child artifact id, evaluation id, metrics run id | complete candidate list, candidate origin, artifact/runtime identity, evaluation status, metrics availability | Partial: selection DTOs are typed; candidate frontier and membership roles need explicit replay projection. | `candidate-frontier.replay`, `evaluation-oracle-targets.selection-replay` | Typed replay can list every candidate considered for a parent before showing the selected successor. |
| `ui.selection.candidate.set` | How did this parent select this successor, and from among which exact candidates? | Successor selection -> candidate comparison | Candidate set records, selection DTO, History selection payload, evidence records, metrics run, selected artifact commitment | parent artifact id, candidate set root/commitment, selected membership id, candidate membership ids, selected artifact id, history entry id | candidate set snapshot, rejected candidates, selected candidate, evidence and scores used for selection | Partial: selected identity is improving; source-set vs decision-set membership must stay typed and explicit. | `candidate-frontier.replay`, `evaluation-oracle-targets.selection-replay` | Typed selection replay proves the selected membership belongs to the final decision candidate set. |
| `ui.timeline.concurrency` | What is the timeline, and which parents, children, attempts, and tool calls ran in parallel or sequentially? | Run timeline / Gantt panel | Runtime lifecycle records, scheduler records, child run records, LLM attempt records, tool execution records, metrics/evaluation records | run id, parent artifact id, child artifact id, evaluation id, attempt id, tool call id, span id | start/end timestamps, nesting, causal parent span, parallel groups, status/error/timeout per segment | Partial: timing/projection work exists, but the UI needs a typed span model down to individual tool calls. | `timeline.concurrency`, `tool.result.trace.projection`, `llm-attempts.provider-observation-projection` | Typed timeline projection renders ordered/nested spans for parent, child, LLM attempt, and tool-call execution. |
| `ui.patch.diff.view` | What are the actual diffs for patches applied to child nodes? | Patch -> diff viewer | Patch artifact records, edit-surface attempt/commitment records, artifact surface records, file snapshot refs | patch id, artifact id, parent artifact id, child artifact id, file path, surface root id | unified diff or patch body ref, before/after file refs, apply status, affected paths | Partial: patch artifacts are typed; diff body/source refs need stable UI-facing replay joins. | `patch.diff-code-graph-impact`, `edit-surface.source-records` | Typed patch lookup returns diff content or stable refs for every applied child patch. |
| `ui.parent.value.added` | For this parent, how does it compare to other parents in value added to child scores? | Parent comparison / score delta panel | Parent artifact records, child evaluation metrics, successor records, baseline metrics, patch records | parent artifact id, child artifact ids, metrics run ids, score dimension ids, baseline artifact id | per-child score delta, aggregate observed value added, comparator parent cohort, evidence strength/provenance notes | Not yet covered as a named analysis projection; base metric records are typed but joins are scattered. | `score.value-locus-analysis`, `evaluation-oracle-targets.selection-replay` | Typed analysis can rank parents by observed child score deltas and trace each delta to evaluated children without claiming causality from one run. |
| `ui.patch.code.graph.impact` | For a given patch, what functions and code graph items does it directly modify? | Patch -> code graph impact panel | Patch artifact records, changed file records, code graph node records, parsed item spans, surface evidence records | patch id, file path, span/range id, code graph node id, module/function/type id, artifact id | touched functions/types/modules, changed ranges, before/after item identity, direct modification markers | Not yet covered as a named projection; requires joining patch diffs to ploke-tree/code graph spans. | `patch.diff-code-graph-impact` | Typed projection maps a patch to directly modified code graph items. |
| `ui.patch.improvement.locus` | For a patch improvement, which files and code graph items are correlated with the improvement, and which other patches affect the same area? | Patch locus / crosslist panel | Patch impact records, evaluation metrics, descendant outcome records, code graph node records, lineage records, repeated-eval/baseline records when available | patch id, code graph node id, file path, metric id, descendant artifact id, lineage edge id, eval target id, baseline id | observed score deltas, touched loci, other patches touching same loci, descendant outcome comparison, repeat/baseline distribution when available, evidence strength | Not yet covered; this is derived analysis over typed patch impact, lineage, scores, and repeated/comparative evals. | `score.value-locus-analysis`, `patch.diff-code-graph-impact`, `runtime-artifact-lineage` | Typed analysis can crosslist patches by shared code graph locus, compare observed score effects, and surface candidate experiments without claiming unsupported causality. |
| `ui.patch.composability` | Are these two patches composable? | Patch comparison panel | Patch artifact records, diff hunks, code graph impact records, artifact surface records, apply/check records | patch id A, patch id B, touched file paths, code graph node ids, base artifact id, surface root id | overlap/conflict classification, shared loci, required base, apply order constraints, test/check outcomes if available | Not yet covered; needs typed patch impact and compatibility records. | `patch.child-composability`, `patch.diff-code-graph-impact` | Typed check can classify two patches as composable, conflicting, order-dependent, or unknown with evidence. |
| `ui.child.mergeability` | Can these children be merged? | Child comparison / merge panel | Child artifact records, lineage records, patch chains, code graph impact records, artifact surface records, apply/check records | child artifact id A, child artifact id B, nearest common ancestor id, patch ids, surface root ids | common ancestor, divergent patch chains, impacted loci, merge conflicts, compatibility result, required checks | Not yet covered; depends on lineage, patch impact, and composability projections. | `patch.child-composability`, `runtime-artifact-lineage`, `patch.diff-code-graph-impact` | Typed mergeability check can compare two children from a common ancestor and explain conflict/compatibility evidence. |

## Acceptance Rules For Implementation Slices

Each implementation slice must state which UI contract rows it advances.

For any row it touches, the slice must preserve:

- Typed source record names.
- Stable join keys.
- Typed nested payloads.
- A projection path suitable for UI drilldown.
- A smallest verification that reconstructs at least one relevant path from typed records.

Roundtrip tests are necessary but not sufficient for UI readiness. At least one verification per family should prove reconstruction across joins, such as parent -> patch -> evidence, child -> tool call -> result, evaluation -> target -> selected successor, child -> lineage -> genesis, patch -> code graph impact, or candidate set -> selected successor.

## Correlation And Causation

Patch, score, and code graph views must distinguish observed facts from causal claims.

The typed causal chain for one child is:

```text
parent -> patch -> changed files/items -> child artifact -> evaluation target -> score delta
```

A single run can show correlation between a patch, the code graph items it touched, and an observed score delta. It does not prove that those changed items caused the improvement.

The UI should use the tracked data to help find experiments and build stronger evidence:

- Repeat the same evaluation against the same baseline to estimate variance.
- Compare alternative patches that touch the same files or intersecting code graph items.
- Compare patches that touch disjoint files or code graph items.
- Compare descendants that share one changed locus but differ elsewhere.
- Run ablation or composability checks when a patch can be isolated safely.
- Track whether a score effect persists across evaluation targets instead of one oracle setup.

Represent this as evidence strength, not as binary causality. A locus panel may show "observed score deltas for patches touching this code graph item" or "higher-confidence correlation after repeated baseline comparisons." It must not say "this function caused the improvement" unless a later typed experimental record explicitly supports that claim.

## Coverage Gates

A drilldown row is `covered` only when all of these are true:

1. Every owned persisted/transmitted payload in the row has a named typed reader.
2. Nested payloads deserialize into typed records or typed error records.
3. Joins are explicit typed ids or refs, not inferred from filenames, logs, CLI output, or JSON field walking.
4. The UI-facing projection carries evidence strength or projection status without upgrading authority.
5. A test or fixture proves reconstruction of the row's tree path.

Until then, the row is only partially covered even if each individual record roundtrips.
