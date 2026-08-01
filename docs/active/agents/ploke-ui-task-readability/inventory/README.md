# Inventory

Refresh policy for UI-relevant graph and loader coverage.

The active source-family tracker is still
[`../../2026-05-11_ploke-tree-graph-ingestion-inventory.md`](../../2026-05-11_ploke-tree-graph-ingestion-inventory.md).
It is useful but stale enough that no new UI coverage claim should cite it
without re-checking the code.

## First-Wave Refresh Scope

Refresh only coverage needed for:

- artifact lineage and genesis,
- candidate frontier and candidate-set replay,
- successor selection and selected membership proof,
- patch/detail joins needed for first inspector paths,
- child self-evaluation, tool-call, and provider-attempt drilldowns,
- snapshot/app summary claims made by the current UI wave.

Do not broaden the refresh into unrelated provider, database, timeline, or
mergeability rows unless a task explicitly opens that slice.

## Classification

Classify every discovered issue as exactly one of:

- `graph-semantic-gap`
  The graph lacks a real relation or evidence attachment.
- `typed-loader-gap`
  The record exists or should exist, but `RunRecordSet` does not load it.
- `ui-projection-gap`
  The graph has the fact, but the current UI path does not expose it.
- `view-diagnostic-gap`
  The fact is purely about layout/readability and belongs in UI code.
- `deferred-drilldown`
  It matters later, but not for the current readability/first-inspector wave.

## Priority Rows

The first refresh pass should focus on rows and slices that map to:

- `ui.child.lineage.genesis`
- `ui.successor.candidate.frontier`
- `ui.selection.candidate.set`
- `ui.successor.selection`
- `ui.child.self.eval.actions`
- `ui.tool.calls`
- `ui.provider.attempts`
- `ui.patch.diff.view`

## First-Wave Coverage Refresh

| UI slice | Current source of truth | First-wave issue class | Refresh note |
|---|---|---|---|
| `ui.child.lineage.genesis` | `ploke_tree::graph::HistoryIndex` already stores `lineage_id`, `parent_block_hashes`, `opened_from_artifact`, and `OpeningAuthorityNode::Genesis` / `::Predecessor`. | `ui-projection-gap` | The graph already owns lineage and genesis facts. `ploke-egui` currently turns them into artifact-tree layout only, so first inspector lineage/genesis detail should be documented as missing UI projection rather than missing graph ingestion. |
| `ui.successor.candidate.frontier` | Scheduler state records are loaded and attached as `SchedulerStateSummary` / `SchedulerNodeRecord` evidence; `Graph` does not preserve frontier-lane membership as a typed graph relation. | `graph-semantic-gap` | Treat current scheduler evidence as supporting evidence only. A frontier answer must come from a graph-owned lane/frontier relation, not from a UI-local copied id set. |
| `ui.selection.candidate.set` | `Graph::from_records` ingests `SelectionNode`, set-scoped `CandidateMembershipKey`, and `CandidateMembershipNode` from sealed History selection payloads. | `ui-projection-gap` | Selected membership proof and candidate-set root already exist in the graph, but the current egui surface uses them only to color selected candidates. Replay/detail exposure is still missing in the UI path. |
| `ui.successor.selection` | `SelectionNode` already stores procedure/policy, scope, decision outcome, selected candidate, occurrence id, membership id, candidate-set root, and considered count. | `ui-projection-gap` | Selection replay is graph-backed today; the remaining first-wave gap is exposing that selection state in a readable inspector path. |
| `ui.child.self.eval.actions` | Evaluation artifacts are loaded through `PassiveEvidence.evaluations`; `Graph::from_records` attaches branch evidence, and `ploke-tree` browser projection can already derive `EvaluationSnapshot` metrics from the typed evaluation artifact. | `graph-semantic-gap` | Metrics are no longer a loader gap. The first-wave blocker is that the graph does not yet index evaluation-action detail strongly enough for egui to borrow it directly. |
| `ui.tool.calls` | Typed `ploke_records::agent_turn` trace/summary artifacts are loaded at run root and attached as node-scoped passive evidence with summary counts and artifact metadata. | `graph-semantic-gap` | The graph now has agent-turn evidence, but it still lacks tool-call/tool-result decomposition as first-class graph relations or answer objects. Do not paper over that by copying tool facts into UI-owned carriers. |
| `ui.provider.attempts` | Provider attempts/timeouts/full-response sidecars are still outside the loaded graph input families. | `typed-loader-gap` | This slice remains blocked on a typed passive owner plus `RunRecordSet` loading; agent-turn evidence does not cover provider-attempt provenance. |
| `ui.patch.diff.view` | Selection payloads and journal-derived branch joins already provide patch ids plus source/proposed content hashes to `ploke-tree` browser projections. | `deferred-drilldown` | First-wave readability can cite patch/detail joins as graph-backed upstream evidence, but egui does not yet expose a patch drilldown and this packet does not open that deeper inspector implementation. |

## Known Stale Starting Points

Re-check these before relying on the older inventory:

- the `Graph::from_records` input summary understates current passive inputs;
  `run_profile`, `run_attempts`, `agent_turns`, and `attempt_runner_results`
  are already loaded and ingested as evidence paths,
- `prototype1.metrics_projection` is not a pure `not-loaded` gap; metrics are
  already loaded through evaluation artifacts even if separate graph indexing is
  still missing,
- `tool.call.record.arguments`, `tool.request.arguments.capture`, and parts of
  `tool.result.trace.projection` are no longer pure loader gaps because typed
  agent-turn records already load those facts; the missing layer is graph-node
  decomposition and UI exposure,
- the older note that agent-turn evidence is only artifact-level passive
  evidence is stale; current evidence attaches at node/branch/runtime-adjacent
  passive paths, though it still does not yield full tool-call graph nodes.
- first-wave egui currently renders graph counts plus artifact/candidate
  projections from `ploke_tree::Graph`; it does not yet surface lineage/genesis,
  selection replay, evaluation action detail, tool-call detail, provider
  attempts, or patch inspectors as first-class panels, so classify those gaps
  precisely instead of downgrading them all to loader failures.

## Update Rule

When a change alters first-wave coverage:

- update the stale inventory in the same change,
- say which typed record or loader is the source,
- say whether the graph creates a core relation or evidence attachment,
- say what graph object or answer object the join feeds,
- preserve ambiguity explicitly when the join fails.
