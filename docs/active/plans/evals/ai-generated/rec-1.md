Verification surface: not tested; this is the requested read-only survey/design synthesis from the orchestrated code/doc survey.

**1. Commands Surveyed**
The old `ploke-eval` observability surfaces are mainly:

- `protocol`: `status`, `run`, `issue-detection`, `tool-call-review`, `tool-call-intent-segments`, `tool-call-segment-review`
- `closure advance`: especially `advance protocol` and `advance all`
- `inspect`: `tool-calls`, `tool-overview`, `operational`, `protocol-artifacts`, `protocol-overview`, `issue-overview`
- `history`: `child-evidence`, `metrics`, `scores`, `score-selection-review`, `selection-show`

Treat these as a question catalog, not as UI architecture. The CLI commands currently mix discovery, projection, and rendering more than we want to preserve.

**2. Outputs And Source Facts**
The useful source facts are:

- Tool calls: `RunRecord -> TurnRecord -> ToolExecutionRecord -> ToolResult`
- Operational metrics: `OperationalRunMetrics`, patch attempted/applied, convergence, oracle eligibility, abort/failure counts
- Protocol evidence: protocol artifacts, segmentation/review outputs, issue detection, aggregate coverage
- Evaluation evidence: branch evaluation artifacts, baseline/treatment compared instances
- Selection evidence: `SelectionDecisionEntryRecord`, `MetricSet`, `MetricCandidate`, `imp@k`, traversal strategy, selected membership/candidate
- Runtime graph facts: scheduler nodes, child plans, History blocks, candidate artifacts, surface evidence

The strongest current path is tool-call drilldown. The weaker path is protocol and selection-scoped protocol metrics.

**3. Typed Boundary**
The UI boundary should be:

```text
ploke-eval writes typed records
-> ploke-records owns passive schemas
-> ploke-tree loads/folds into Graph
-> ploke-egui borrows graph facts and renders
```

Do not let `ploke-egui` parse `record.json.gz`, protocol JSON, CLI report JSON, rendered table text, or path strings directly.

Current good path:

```text
record.json.gz
-> ploke_records::run_record::RunRecord
-> PassiveEvidence.run_records
-> Graph::run_records() / Graph::run_record_refs_for_branch()
-> RunRecordBranchInspection
-> egui renderer
```

Missing graph witnesses:

- Protocol aggregate joined to branch/run/candidate/selection.
- Selection metric witness that carries operational and protocol inputs together.
- Chart-ready tool-call aggregate so egui does not rescan turns every frame.

**4. UI Representations**
Recommended panels:

- Evidence Overview: loaded runs, generations, branches, available/missing evidence.
- Tool Behavior: tool-call counts, failures, retries, search thrash, repeated-target loops.
- Protocol Reliability: coverage, reviewed/missing segments, issue families, confidence/status.
- Candidate Selection: selected vs considered candidates, score components, metric identity, traversal policy.
- Evaluation Comparison: treatment-first baseline/treatment deltas.
- Timeline: parent, child, eval, protocol, tool, provider, handoff spans.
- Patch/Locus Drilldown: changed files, patch validity, convergence, tool/protocol evidence around the patch.

**5. Chart Set**
Use simple, dense charts before adding a plotting dependency:

- Stacked horizontal bars: tool success/failure/partial/error by tool.
- Line charts: tool volume, failure rate, latency/cost by generation.
- Run sequence strips: tool order, edit attempts, retries per turn/run.
- Search-thrash bars: repeated same-tool/same-target streaks.
- Protocol coverage bars: reviewed/missing/mismatched/ineligible/error.
- Pareto bars: issue families by frequency and affected tool.
- Dumbbell/slope charts: treatment-first baseline vs treatment metrics.
- Evidence heatmap: generation/run x procedure coverage for Multi-SWE scaling.

**6. Cache And Allocation Plan**
Compute once at import, graph revision, filter change, or selection change. Render from slices.

Cache boundaries:

- `ploke-tree`: semantic facts and derived observability indexes.
- `ploke-egui`: numeric bins, chart geometry, visible-row label cache, hover payload cache.
- Renderer: `RichText`, labels, hover strings, decoded details.

Avoid:

- per-frame grouping/sorting
- per-frame `Vec` collection of all turns/tool calls
- repeated `format!` in chart loops
- repeated tool argument/result decoding
- cloned CLI report DTOs
- JSON parsing in egui

**7. Proof Claims**
Now, with one instance, the UI can support:

- mechanism evidence
- debugging transparency
- “this lineage improved or degraded on this observed instance”
- evidence completeness/weakness
- tool/protocol failure diagnosis

It cannot honestly support:

- general Multi-SWE-Bench improvement
- causal proof of framework efficacy
- held-out generalization
- robust claim against eval gaming

Later proof requires:

- multi-instance split identity
- evaluated-node coverage
- best-so-far and active-lineage score curves
- median/CI across instances
- periodic held-out checks
- budget-normalized `improvement@k`
- explicit evaluation-gaming indicators

**8. Implementation Slices**
Recommended order:

1. Graph-owned tool observability projection from `RunRecordEvidence`.
2. Egui Tool Behavior panel with stacked bars and sequence strip.
3. Native benchmark scenarios for expanded tool panel, filters, hover, drilldown.
4. Protocol aggregate witness in `ploke-tree`, joined to branch/run/candidate/selection.
5. Protocol Reliability panel.
6. Selection Metric panel with treatment-first operational/protocol score components.
7. Evidence Overview and proof-status labels.
8. Multi-instance evidence matrix and improvement curves.

That preserves the right authority boundary while giving you the UI surface you actually asked for: a performant, inspectable argument about what happened in the loop and how strong the evidence is.

