# Typed Persistence Operating Console

Updated: 2026-05-10

This is the entry point for typed-persistence-spine work.

Use this document to stay oriented. Use [`implementation-slices.md`](implementation-slices.md) to choose the current slice. Use the other docs only for the rows linked from that slice.

## Target

Build the typed persistence spine needed for an interactive archive graph UI over Prototype 1 self-improvement runs.

The central operator-facing object is the run execution graph described by
`crates/ploke-eval/src/cli/prototype1_state/mod.rs`: a generative graph of
Runtime, Artifact, OperationCoordinate, PatchAttempt, derived Artifact,
hydrated Runtime, selection, and successor handoff. Typed records, DTO cleanup,
replay projections, and browser-model exports are useful only insofar as they
preserve or expose that graph through nodes, edges, evidence attachments,
inspector refs, or timeline spans.

The target UI is an interactive node graph canvas over the growing archive:

- artifact/runtime/agent nodes
- operation edges from generator Runtime to target Artifact surface
- patch-attempt and derived-Artifact edges
- runtime-derivation edges from Artifact to hydrated Runtime
- candidate, evaluation, successor, and merge/composability edges
- inspector panels for selected nodes/edges
- badges for status, score deltas, selected successor state, confidence, and evidence strength
- a compact synced timeline strip, expandable into a full timeline view

The implementation target is not "remove some JSON parsing." The target is typed facts, joins, replay, derived evidence, and evidence-strength views that make the archive graph explainable.

Treat row-level typed cleanup as foundation work. A row is operator-complete
only when its facts can be attached to the run execution graph and inspected
through a graph/browser projection without reading logs or raw JSON by hand.

## Operating Rule

Start every work session here, then follow exactly one `next` slice in [`implementation-slices.md`](implementation-slices.md).

Do not use `.codex/task-stack.jsonl` to decide this lane's next task. The task stack may contain cross-thread local reminders; this operating console and the implementation queue are the durable lane source of truth.

## Reading Order

For each implementation session:

1. Read this file.
2. Read only the current `next` row in [`implementation-slices.md`](implementation-slices.md).
3. Read only the matching rows in [`traceability-matrix.md`](traceability-matrix.md).
4. Read only the matching rows in [`ui-drilldown-contract.md`](ui-drilldown-contract.md).
5. Read only the matching rows in [`inventory.md`](inventory.md) or the cited bounded survey report.
6. Then inspect code.

Do not scan every planning doc before implementing. The slice controls the context window.

## Roadmap

| Phase | Slices | Goal | Result |
|---|---|---|---|
| 1. Typed blocker cleanup | 1-7 | Remove direct typed-persistence violations in protocol artifacts, tool calls/results, LLM attempts, and projections. | Owned persisted/transmitted payloads have named typed readers instead of `serde_json::Value` staging or stringly JSON. |
| 2. Graph-spine landing | 8 | Establish the browser-facing generative execution graph spine, then attach already-typed tool-call, tool-result, and provider-attempt facts as evidence. | The operator can inspect how Runtime -> Surface(Artifact) -> PatchAttempt -> derived Artifact -> hydrated Runtime unfolded, with tool/provider evidence available from selected graph objects. |
| 3. Replay and ownership joins | 9-12 | Clean up DTO bridges, edit-surface source records, database prompt evidence, evaluation target identity, and successor selection replay. | The system can walk from parent to child, patch, evidence, target, candidate set, selected successor, and outcome. |
| 4. Archive graph primitives | 13-16 | Build lineage, candidate frontier, timeline, and patch/code-graph impact projections. | The UI can draw graph nodes/edges, timeline spans, patch diffs, and directly modified code graph items. |
| 5. Derived evidence and compatibility | 17-18 | Add score/locus evidence-strength views and patch/child composability checks. | The UI can compare parents, patches, loci, candidates, children, and merge/composability evidence without overclaiming causality. |

## Session Loop

For each slice:

1. Name the slice and the rows it advances in [`traceability-matrix.md`](traceability-matrix.md) and [`ui-drilldown-contract.md`](ui-drilldown-contract.md).
2. Identify the exact typed facts and joins needed.
   Name the run execution graph node, edge, evidence attachment,
   inspector ref, or timeline span the slice creates or improves.
3. Implement the smallest coherent code change that creates or fixes those facts/joins.
4. Add verification that reconstructs or derives the relevant UI answer from typed records.
5. Run the smallest useful verification command from [`implementation-slices.md`](implementation-slices.md), adjusting only when the cited command is stale.
6. Update only the affected docs:
   - [`implementation-slices.md`](implementation-slices.md) for status and next slice.
   - [`typed-data-coverage-report.md`](../typed-data-coverage-report.md) for typed coverage.
   - [`traceability-matrix.md`](traceability-matrix.md) for facts, joins, derived views, or evidence strength.
   - [`ui-drilldown-contract.md`](ui-drilldown-contract.md) for UI answer coverage or changed acceptance criteria.
   - [`inventory.md`](inventory.md) only when compliance or ownership changed.

## Doc Admission

Default to not adding new docs.

Before adding a new doc, answer all five questions:

1. Is this a new artifact type, or just more detail for an existing artifact?
2. Who reads it, and at what point in the workflow?
3. Which existing doc links to it as source of truth?
4. Which existing doc becomes shorter or clearer because this exists?
5. What is the update rule?

If the answer is unclear, add a section to an existing doc instead.

Allowed lane doc roles:

- `operating-console.md`: entry point, workflow, roadmap, doc admission.
- `implementation-slices.md`: ordered work queue and status.
- `ui-drilldown-contract.md`: archive graph UI questions and acceptance criteria.
- `traceability-matrix.md`: typed facts, joins, replay, derived evidence, and evidence strength for UI answers.
- `inventory.md` / `inventory.jsonl`: surveyed persisted/transmitted surfaces.
- `reports/*.jsonl`: bounded raw survey outputs.
- dated handoff docs: restart summaries.
- `todo-records.md`: temporary scratch for unresolved design questions and
  validation findings before they are promoted into the matrix, UI contract,
  inventory, or implementation queue. It is not a source of truth; retire or
  shorten sections after promotion.

Anything else needs a short justification in this file or in the handoff that creates it.

## Drift Checks

Stop and reorient if any of these happen:

- A code change removes `serde_json::Value` but does not improve a typed fact, join, replay, derived evidence, or evidence-strength path.
- A slice finishes foundation cleanup but leaves its facts unattached to the run
  execution graph without marking the remaining graph/browser landing work.
- A UI answer requires data not present in the inventory, matrix, or implementation queue.
- A new projection reads logs, rendered CLI output, filenames, or anonymous JSON to infer owned meaning.
- A slice starts spanning unrelated UI rows or families.
- A new doc is proposed without a reader, owner link, and update rule.
- Evidence-strength views start implying causality from one run.

## Definition Of Done

A slice is done only when:

1. Production owned JSON/JSONL reads for that slice use named Rust types.
2. Nested owned payloads have typed readers or typed error records.
3. Stable typed joins exist for the UI rows the slice advances.
4. Verification reconstructs deterministic answers or computes derived/evidence-strength answers from typed records.
5. The affected docs are updated.
6. The next queued slice is clear.

Operator completion additionally requires the slice's facts to be reachable
from the run execution graph and visible through a browser/egui projection, or
for the remaining graph/browser landing work to be explicitly queued.
