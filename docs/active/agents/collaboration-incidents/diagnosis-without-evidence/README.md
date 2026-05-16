# Diagnosis Without Evidence

Incidents where the agent presented a causal diagnosis before reading the code
that actually controlled the behavior, forcing the user to catch the gap and
re-ask for a real trace.

## Entries

- [`2026-05-15-eval-home-log-misdiagnosed-instead-of-measuring-targets.md`](2026-05-15-eval-home-log-misdiagnosed-instead-of-measuring-targets.md)
  The agent analyzed the tiny eval-home checkpoint log itself instead of first
  measuring the real storage driver inside `~/.ploke-eval`, which turned out to
  be accumulated `target/` directories.
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
- [`2026-05-16-archaeology-report-updated-in-isolation.md`](2026-05-16-archaeology-report-updated-in-isolation.md)
  The agent updated one archaeology report and its proof comments without
  re-reading the existing archaeology set, which produced a shallow,
  inconsistent report update despite stronger sibling examples already being on
  disk.
- [`2026-05-16-plausible-archaeology-filler-instead-of-grounded-references.md`](2026-05-16-plausible-archaeology-filler-instead-of-grounded-references.md)
  The agent used vague carrier-location prose in archaeology work instead of
  concrete type/field references, making the report look more grounded than it
  was.
- [`2026-05-16-minimum-compliance-archaeology-pass.md`](2026-05-16-minimum-compliance-archaeology-pass.md)
  The agent responded to archaeology criticism with a series of narrow
  compliance patches instead of redoing the whole report to the sibling-report
  standard in one exacting pass.
- [`2026-05-16-protocol-dir-rule-answered-before-tracing-write-target.md`](2026-05-16-protocol-dir-rule-answered-before-tracing-write-target.md)
  The agent answered from the generic `protocol-artifacts` rule before tracing
  the active Prototype 1 campaign's real `write_protocol_artifact(record_path,
  ...)` targets through evaluation artifacts and run registrations.
- [`2026-05-16-self-improvement-eval-underread-existing-metrics.md`](2026-05-16-self-improvement-eval-underread-existing-metrics.md)
  The agent gave a strategic self-improvement/MBE/consensus critique before
  fully inventorying existing typed metric, evaluation, oracle, selection, and
  History payload carriers, leading to overbroad "missing type" claims where
  the real gap was policy/admission integration.
