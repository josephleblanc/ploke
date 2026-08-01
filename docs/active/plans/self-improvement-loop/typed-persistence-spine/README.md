# Typed Persistence Spine Survey

Machine-readable and human-readable survey artifacts for the Prototype 1 typed persistence spine.

- [`operating-console.md`](operating-console.md)
  Entry point for the lane: target, reading order, roadmap, session loop, doc admission, drift checks, and definition of done.
- [`survey-index.md`](survey-index.md)
  Current survey progress by execution-surface family.
- [`2026-05-10-survey-inventory-handoff.md`](2026-05-10-survey-inventory-handoff.md)
  Restart handoff for the completed first-pass inventory, validation, and blockers.
- [`implementation-slices.md`](implementation-slices.md)
  Queue of record for typed-persistence implementation. Use this instead of `.codex/task-stack.jsonl` to decide this lane's next slice.
- [`ui-drilldown-contract.md`](ui-drilldown-contract.md)
  Question-driven data contract for the interactive tree UI. Use this as acceptance criteria for replay/drilldown completeness.
- [`traceability-matrix.md`](traceability-matrix.md)
  Bridge from inventory rows to UI answers, separating typed facts, joins, deterministic replay, derived evidence, and evidence strength.
- [`todo-records.md`](todo-records.md)
  Temporary scratch for unresolved design questions and validation findings before promotion into the matrix, UI contract, inventory, or implementation queue.
- [`inventory.jsonl`](inventory.jsonl)
  Curated machine-readable surface inventory accepted by the main thread.
- [`inventory.md`](inventory.md)
  Human-facing rollup of accepted surfaces and blockers.
- [`reports/`](reports/README.md)
  Bounded raw JSONL reports written by sub-agents.
- [`families/`](families/README.md)
  Curated family drilldowns generated from accepted survey rows.

Workflow and schemas live in [`../typed-persistence-survey-orchestration.md`](../typed-persistence-survey-orchestration.md).
