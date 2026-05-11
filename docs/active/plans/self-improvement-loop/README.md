# Self-Improvement Loop Track

Shared planning area for Prototype 1 records, playback, loop evaluation, and frontend observability work.

- [`handoffs.md`](handoffs.md)
  Current handoff list for this track, ordered by use during restart.
- [`plan.md`](plan.md)
  Ordered implementation plan for records, playback roundtrips, loop-run evaluation, benchmark evidence, and frontend direction.
- [`implementation-status.md`](implementation-status.md)
  Current implementation evidence, open gaps, and next verification commands.
- [`artifact-surface-authority-design.md`](artifact-surface-authority-design.md)
  Design intent separating patch provenance, artifact surface measurement, and History authority succession.
- [`baseline-evidence-authority-plan.md`](baseline-evidence-authority-plan.md)
  Plan for making parent-local baseline evidence explicit before child fanout and successor handoff.
- [`typed-data-coverage-report.md`](typed-data-coverage-report.md)
  Coverage report for Prototype 1 persisted data typed deserialization, including nested payload gaps.
- [`typed-persistence-spine-plan.md`](typed-persistence-spine-plan.md)
  Repeatable lane for replacing owned JSON parsing with typed records and building full replay/UI drilldown.
- [`typed-persistence-survey-orchestration.md`](typed-persistence-survey-orchestration.md)
  JSONL-based sub-agent workflow for surveying typed persistence surfaces without overloading the main context.
- [`typed-persistence-spine/operating-console.md`](typed-persistence-spine/operating-console.md)
  Operating console for the typed-persistence lane: current workflow, roadmap, doc admission, and drift checks.
- [`typed-persistence-spine/implementation-slices.md`](typed-persistence-spine/implementation-slices.md)
  Queue of record for typed-persistence implementation slices. Use this instead of `.codex/task-stack.jsonl` for this lane's next task.
- [`typed-persistence-spine/ui-drilldown-contract.md`](typed-persistence-spine/ui-drilldown-contract.md)
  Data contract for the interactive tree UI questions each typed record/replay slice must make answerable.
- [`typed-persistence-spine/traceability-matrix.md`](typed-persistence-spine/traceability-matrix.md)
  Bridge from typed inventory rows to UI answers, including deterministic replay and probabilistic evidence-strength views.
- [`long-run-evaluation.md`](long-run-evaluation.md)
  Safe route for evaluating a recent long loop run without relying on `scheduler.json`.
- [`frontend-questions.md`](frontend-questions.md)
  Questions and feature checks the frontend should answer for multi-generation self-improvement runs.
- [`../../agents/2026-05-11_mbe-oracle-calibration-handoff.md`](../../agents/2026-05-11_mbe-oracle-calibration-handoff.md)
  Active oracle-calibration handoff for MBE controls, compile-failed loop candidates, and benchmark-base patch export questions.
