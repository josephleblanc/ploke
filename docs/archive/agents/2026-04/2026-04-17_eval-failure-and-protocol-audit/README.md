# Eval Failure And Protocol Audit

Focused control-plane docs for the 2026-04-17 audit pass over failed eval runs,
known-limitations coverage, blind trace review, and protocol-output
aggregation.

- [`control-plane.md`](./control-plane.md)
  Orchestrator state table, scope, and next actions for this audit pass.
- [`failure-audit-packet.md`](./failure-audit-packet.md)
  Packet for enumerating failed eval runs, clustering causes, and checking
  known-limitations / bug-report coverage.
- [`blind-trace-review-packet.md`](./blind-trace-review-packet.md)
  Blind reviewer packet for CLI-only trace review over a random sample of runs.
- [`failure-inventory.md`](./failure-inventory.md)
  Live inventory of the current failed eval runs, clustered causes, and
  documentation status.
- [`blind-trace-sample-summary.md`](./blind-trace-sample-summary.md)
  Blind-review consensus and protocol-comparison summary for the sampled
  completed runs.
- [`implementation-priority-packets.md`](./implementation-priority-packets.md)
  Prioritized follow-up packet list derived from the failure audit and blind
  trace sample.
- [`packet-P0-campaign-protocol-triage.md`](./packet-P0-campaign-protocol-triage.md)
  Top-priority packet for turning campaign protocol data into a compact
  operator-facing triage surface for tool/harness improvement work.
- [`2026-04-17_protocol-triage-implementation-report.md`](./2026-04-17_protocol-triage-implementation-report.md)
  Bounded implementation report for the first campaign-scoped protocol triage
  dashboard and drilldown flow.
- [`2026-04-17_workflow-trial-synthesis.md`](./2026-04-17_workflow-trial-synthesis.md)
  Synthesis of the parallel sub-agent workflow trial, including the main
  refinement: add an explicit ownership gate before code inspection.
- [`2026-04-17_protocol-diagnosis-workflow.md`](./2026-04-17_protocol-diagnosis-workflow.md)
  Formalized protocol-driven diagnosis workflow for sub-agents and
  orchestrators, including slice-selection rules, the ownership gate, and the
  fixed report schema.
- [`2026-04-17_protocol-diagnosis-subagent-template.md`](./2026-04-17_protocol-diagnosis-subagent-template.md)
  Copyable sub-agent launch template that packages the workflow, ownership
  gate, and fixed report schema into one reusable prompt.
- [`2026-04-17_edr-0003-downstream-validation-plan.md`](./2026-04-17_edr-0003-downstream-validation-plan.md)
  Concrete downstream validation plan for `EDR-0003`, using the local
  Multi-SWE-bench harness and a fixed protocol-derived cohort rather than an
  invented recommendation oracle.
- [`2026-04-17_edr-0003-baseline-cohort-report.md`](./2026-04-17_edr-0003-baseline-cohort-report.md)
  Hard Multi-SWE-bench baseline report for the fixed `EDR-0003` pilot cohort,
  including the `0/5` resolved starting point and benchmark-side invalidation
  pattern.
- [`2026-04-17_workflow-trial-search-thrash.md`](./2026-04-17_workflow-trial-search-thrash.md)
  Trial report for the `search_thrash` issue-family slice.
- [`2026-04-17_workflow-trial-request-code-context.md`](./2026-04-17_workflow-trial-request-code-context.md)
  Trial report for the `request_code_context` tool-family slice.
- [`2026-04-17_workflow-trial-read-file.md`](./2026-04-17_workflow-trial-read-file.md)
  Trial report for the `read_file` tool-family slice.
- [`2026-04-17_workflow-trial-partial-next-step.md`](./2026-04-17_workflow-trial-partial-next-step.md)
  Trial report for the `partial_next_step` issue-family slice.
- [`2026-04-17_workflow-trial-artifact-errors.md`](./2026-04-17_workflow-trial-artifact-errors.md)
  Trial report for the protocol artifact/schema error slice.
- [`2026-04-17_workflow-trial-read-file-partial-next-step.md`](./2026-04-17_workflow-trial-read-file-partial-next-step.md)
  Trial report for the combined `read_file` + `partial_next_step` slice.
- [`packet-P1-path-recovery.md`](./packet-P1-path-recovery.md)
  Highest-leverage tool recovery packet for missing-file and repo-root/path
  confusion.
- [`packet-P2-parser-blockers.md`](./packet-P2-parser-blockers.md)
  RCA packet for the active parser/indexing blocker families found in the
  failed-run audit.
- [`packet-P3-protocol-calibration.md`](./packet-P3-protocol-calibration.md)
  Packet for reducing run-level protocol false positives on short or clean
  traces.
- [`packet-P4-patch-format-recovery.md`](./packet-P4-patch-format-recovery.md)
  Packet for improving recoverability around malformed unified diffs.
