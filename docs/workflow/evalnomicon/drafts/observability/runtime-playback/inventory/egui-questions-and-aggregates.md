# Egui Questions And Aggregates

Status: draft consumer map.

The GUI should be able to show several views at the same cursor position
without each view building its own ordering. These aggregates should be
computed from `RuntimePlaybackRef<'g, G>` or from graph-backed drilldowns under
the selected playback step.

## Questions We Want To Ask

- Which Artifact is current at this cursor, and which History block/entry made
  it current?
- Which Runtime produced or evaluated this Artifact?
- Which operation or intervention was attempted, over which Artifact or target?
- Which candidates were available to selection, and why was one chosen?
- Did the selected path improve benchmark behavior over its ancestors?
- Was the result real benchmark improvement, a proxy improvement, or only a
  reduction in tool-loop failure?
- Where did a failed candidate break down: setup, indexing, model request, tool
  use, patch generation, patch application, validation, packaging, evaluation,
  selection, or successor handoff?
- Which model requests were expensive or low-value?
- Which model, provider, endpoint, tool list, and tool-choice policy were active
  for a given request?
- Which tools failed, repeated, or recovered after repair prompts?
- Did indexing and DB state match the workspace the model was asked to edit?
- Did channel messages and transition journal entries agree about runtime
  readiness, completion, and failure?
- Did a protocol review or selection formula influence the chosen successor?
- Are we accumulating useful self-improvement infrastructure, such as better
  search, better tools, better prompts, or better evaluation records?
- Are we overfitting to the visible benchmark or improving under held-out or
  policy-stable evaluation?

## Candidate Aggregates

| Aggregate | Source facts | GUI representation | Cursor behavior |
| --- | --- | --- | --- |
| `ArtifactAncestryPlayback` | History heads, Artifact refs, scheduler branches, surface evidence | artifact-first tree with dim/reveal state | frame shows visible Artifact set and selected current Artifact |
| `HistoryAuthorityPanel` | sealed blocks, admitted entries, History warnings | lineage table and selected block detail | updates by block/entry cursor |
| `SelectionDecisionPanel` | selection node, candidate payloads, membership ids, formula rows, metric set | candidate table, selected row, formula detail | filters to selection entry at cursor |
| `BenchmarkProgressSeries` | compared run metrics, oracle records, eval set/evaluator identity, imp@k rows | line chart or compact bar chart by generation/lineage | cursor highlights current Artifact and cumulative best |
| `AgentTurnTimelineAggregate` | `AgentTurnRecordSet`, run-record turn records, provider sidecars | turn timeline with model/tool/edit lanes | nested cursor steps through events inside selected turn |
| `ModelExchangeAggregate` | agent-turn prompts, future provider-request records, `RawFullResponseRecord`, selected model/provider evidence | request/response inspector with tool policy and token/cost detail | cursor follows provider calls inside the selected turn |
| `ToolBehaviorAggregate` | tool request/completed/failed events, tool outputs, repair prompts | per-tool counts plus expandable result snippets | updates by runtime, attempt, or agent-turn scope |
| `CostAndLatencyLedger` | token usage, cost, processing time, queue time, wall clock | stacked cost/tokens/time bars | cursor shows cumulative and step-local cost |
| `IndexHealthAggregate` | indexing status, parse failures, DB snapshot paths, time-travel markers | parse/index status strip and DB snapshot detail | cursor shows DB state available to the model/tool loop |
| `ProtocolProcedureAggregate` | protocol artifacts, protocol metrics, review counts | procedure timeline and review outcome table | filters by run/procedure/subject |
| `RuntimeChannelAggregate` | channel envelopes, invocation records, transition journal runtime events | parent-child message lane | cursor shows readiness/evaluation/result handoff |
| `StorageFootprintAggregate` | record paths, compressed/decompressed sizes where available, DB snapshot paths | storage/cost table | cursor shows artifact size pressure when available |

## Benchmark Improvement Evidence

To claim improvement on a benchmark, the playback model should carry:

- benchmark family and dataset/eval-set identity;
- evaluator identity and version;
- baseline and treatment record paths for the same instance;
- comparable run metrics for baseline and treatment;
- oracle eligibility and oracle evaluation when available;
- patch projection check state and submission artifact state;
- selection policy, formula, metric set, candidate set root, and selected
  membership/occurrence ids;
- lineage/Artifact ancestry proving which selected candidate descended from
  which prior Artifact;
- held-out or policy-stable evaluation markers when available.

For now, the durable claim should be phrased conservatively:

```text
Under evaluator E and eval set S, candidate C improved measured metric M over
ancestor/base A for instance set I, with evidence class K.
```

That keeps room for multiple evidence strengths: true benchmark pass/fail,
oracle-backed comparison, operational proxy, protocol review, or diagnostic
tool-loop quality.

## Current Proxy Metrics

Useful near-term metrics already exist across current records:

- `RunMetrics.tool_calls_total` and `tool_calls_failed`;
- patch attempted / apply state / projection check state;
- partial patch failures and same-file retry streaks;
- aborted, repair-loop aborted, convergence, and oracle eligibility;
- token usage, cost, processing time, queue time, and tokens-per-second from
  model response metadata;
- selected model/provider, request message count, tool definition count, and
  tool-choice policy once provider-request evidence is made durable;
- run-record wall-clock timing when present;
- selection metric rows, imp@k data, formula weights, projection failures, and
  selected-source facts;
- protocol artifact counts, reviewed/missing segment counts, confidence counts,
  and review signal totals;
- indexing status, parse failures, snapshot paths, and DB time-travel markers;
- channel message counts and eventual per-envelope timing/order once added.

These are not all equivalent to benchmark success. They are useful because
they tell us whether the loop has a chance to improve coding behavior: the
model can inspect the right files, call the right tools, apply plausible
patches, validate them, package them, and make selection decisions from
comparable evidence.

## HyperAgents-Informed Signals

The local HyperAgents notes emphasize sustained improvement, open-ended
exploration, evaluation cost, held-out evaluation, and improvement@k-style
measurement. For Prototype 1, the closest practical signals are:

- performance over a lineage rather than one isolated child;
- best descendant improvement under a bounded child budget;
- ability to recover from failed tool or patch attempts;
- cost per useful candidate and cost per selected improvement;
- whether modifications improve future generation tools, prompts, memory,
  evaluation, or selection, not only one benchmark answer;
- held-out or policy-stable evaluations before treating an improvement as
  durable.

## Design Constraint For Egui

Each aggregate should declare:

- its playback scope;
- its required graph indexes;
- its evidence class;
- its cache key, usually `(graph_id, scope, cursor, aggregate_kind)`;
- its drilldown target, if any.

The aggregate may own render-friendly cached rows at the egui boundary. It must
not become the source record, parse raw run files, or invent a separate cursor.
