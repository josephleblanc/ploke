# Shared Agent Brief

## Goal

Survey one Prototype 1 loop-health domain and write the assigned report in this directory. The report should help design a small, ergonomic, uniform operational introspection stream, likely tracing-backed JSONL, without adding scattered persistence files or confusing telemetry with sealed History authority.

## Core Model

Prototype 1 is a self-propagating loop over Artifact/Runtime pairs:

```text
Parent R1 from active checkout A1
  -> create temporary child checkout A2
  -> hydrate/evaluate child runtime C2 from A2
  -> select A2 under policy
  -> install A2 into the stable active checkout
  -> hydrate successor runtime R2 from the active checkout
  -> hand off parent authority to R2
  -> R1 exits
  -> cleanup temporary child/build products
```

Do not treat a worktree path, branch name, process id, scheduler row, invocation JSON, ready file, or CLI report as Crown authority. History authority comes from sealed lineage-local blocks. Existing records are evidence, projections, transport, or diagnostics unless admitted by a typed History transition.

## Required Sources

Read these first:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `docs/reports/prototype1-record-audit/history-admission-map.md`

Then use these targeted sources as needed:

- `docs/reports/prototype1-record-audit/2026-04-29-history-crown-introspection-audit.md`
- `docs/reports/prototype1-record-audit/2026-04-29-monitor-report-coverage-audit.md`
- `docs/reports/prototype1-record-audit/2026-04-29-record-emission-sites-audit.md`
- `docs/reports/prototype1-record-audit/2026-04-29-run-shape-diff-audit.md`
- `crates/ploke-eval/src/cli/prototype1_state/inner.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/journal.rs`
- `crates/ploke-eval/src/cli/prototype1_state/report.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
- `crates/ploke-eval/src/cli/prototype1_process.rs`
- `crates/ploke-eval/src/intervention/scheduler.rs`
- `crates/ploke-eval/src/intervention/branch_registry.rs`

Prefer `rg`, targeted `sed`, and exact line references. Avoid broad dumps of live campaign data.

## Report Requirements

Fill the assigned template only. Preserve the section headings.

For your domain, provide:

- questions operators/LLMs should be able to ask during a 5-10 generation run;
- questions that become important for longer runs;
- which questions are already answered by persisted data;
- which are partially derivable from existing data;
- which require new logging;
- where new data should naturally be recorded if needed;
- a final classification into essential, nice-to-have, and too granular/noisy.

## Logging Direction

Assume the desired implementation is a small ergonomic logging surface, not large inline record construction at every call site. Prefer transition-boundary helpers such as `.log_step()` or `.log_result()` if that fits the code.

Assume persisted operational logs may use one uniform JSONL record type with optional fields and structured `tracing` spans/events. Your report should identify fields, not implement them.

Do not recommend adding another half-dozen domain-specific files. If a question needs new data, describe the minimal field(s) on a shared operational introspection event.

## Claim Boundary

Telemetry is not History authority. A log event can help observe liveness, performance, and consistency; it does not by itself admit a child, prove a Crown handoff, or advance a lineage head.
