# Diagnosis Without Evidence

Incidents where the agent presented a causal diagnosis before reading the code
that actually controlled the behavior, forcing the user to catch the gap and
re-ask for a real trace.

## Entries

- [`2026-05-15-prototype1-handoff-model-not-reconstructed-before-fix-talk.md`](2026-05-15-prototype1-handoff-model-not-reconstructed-before-fix-talk.md)
  The agent talked about repairing a Prototype 1 handoff/root mismatch before
  first reconstructing the active-checkout vs child-worktree vs successor
  model from the runtime docs and controlling carriers, which made the change
  look under-justified.
- [`2026-05-15-broad-harness-retry-diagnosis-without-code-check.md`](2026-05-15-broad-harness-retry-diagnosis-without-code-check.md)
  During broad-harness live-run triage, the agent described retry behavior
  without first reading the controlling `ploke-eval` / `ploke-tui` code paths,
  and only later discovered the real headless-TUI chain-limit and no-rescan
  causes.
- [`2026-05-15-artifact-view-model-not-read-before-graph-edits.md`](2026-05-15-artifact-view-model-not-read-before-graph-edits.md)
  The agent changed default artifact-graph semantics before re-reading the
  documented artifact-first model, which let renderer-local pressure drift the
  graph away from the intended product view.
- [`2026-05-15-run-artifact-claims-without-reading.md`](2026-05-15-run-artifact-claims-without-reading.md)
  The agent reasoned about which run artifacts mattered before proving, from
  the current run files themselves, what data those artifacts actually
  contained and what was missing.
