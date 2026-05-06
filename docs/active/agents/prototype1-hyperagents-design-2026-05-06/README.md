# Prototype 1 HyperAgents Design Reports

- [`01-child-admission-evaluation-status.md`](01-child-admission-evaluation-status.md)
  Status of child result/evaluation evidence and the missing archive-admission join record.
- [`02-selection-archive-parent-status.md`](02-selection-archive-parent-status.md)
  Status of generation-local successor selection and missing archive-wide parent selection.
- [`03-history-crown-archive-authority-status.md`](03-history-crown-archive-authority-status.md)
  Status of History/Crown authority, surface commitments, and archive-admission constraints.
- [`04-ploke-tui-semantic-edit-surface-status.md`](04-ploke-tui-semantic-edit-surface-status.md)
  Status of ploke-tui semantic edit mechanisms and reuse paths for bounded Prototype 1 surfaces.
- [`05-process-surfaces-memory-rubric-status.md`](05-process-surfaces-memory-rubric-status.md)
  Status of mutable process surfaces, memory/performance summaries, and triage/rubric implementation.
- [`06-runtime-reliability-telemetry-demo-readiness.md`](06-runtime-reliability-telemetry-demo-readiness.md)
  Status of telemetry, timeout semantics, fanout risks, and demo-readiness blockers.
- [`07-formal-procedure-protocol-metrics-status.md`](07-formal-procedure-protocol-metrics-status.md)
  Status of formal-procedure metric concepts in protocol evaluation crates.
- [`09-prototype1-runtime-output-census.md`](09-prototype1-runtime-output-census.md)
  Census of Prototype 1 runtime, child, campaign, History, and CLI output files.
- [`10-report-log-type-census.md`](10-report-log-type-census.md)
  Census of fractured report, record, artifact, log, journal, trace, manifest, and decision types.
- [`08-protocol-evaluation-data-census.md`](08-protocol-evaluation-data-census.md)
  Census of scoped protocol/evaluation data carriers, artifacts, projections, and selection signals.
- [`13-semantic-edit-surface-loop-readiness.md`](13-semantic-edit-surface-loop-readiness.md)
  Readiness of ploke-tui semantic edits as bounded Prototype 1 child patches.
- [`11-successor-selection-registry-archive-gap.md`](11-successor-selection-registry-archive-gap.md)
  Status of successor-selection registry contents, direct callers, and archive parent-selection gaps.
- [`12-hyperagents-selection-transfer-notes.md`](12-hyperagents-selection-transfer-notes.md)
  Extraction of HyperAgents parent selection, archive traversal, transfer scoring, and Ploke authority fit.
- [`17-run-protocol-artifact-access-census.md`](17-run-protocol-artifact-access-census.md)
  Census of run/protocol artifact locators, accessors, aggregates, reports, and child-selection evidence fit.
- [`14-history-evidence-access-operator-census.md`](14-history-evidence-access-operator-census.md)
  Census of History/Crown evidence access, locators, stores, refs, surface/tree operators, and typed unpacking.
- [`15-prototype1-report-accessor-census.md`](15-prototype1-report-accessor-census.md)
  Census of Prototype 1 path, load, join, summary, projection, and provenance-loss operators.
- [`16-artifact-tree-backend-locator-census.md`](16-artifact-tree-backend-locator-census.md)
  Census of artifact, patch, surface, tree-key, backend locator, and evidence-access concepts.
- [`18-existing-type-expansion-recommendation.md`](18-existing-type-expansion-recommendation.md)
  Recommendation to expand `history_preview::EvidenceStore`/`FsEvidenceStore` first for child evidence location and unpacking.
- [`19-child-evidence-core-implementation.md`](19-child-evidence-core-implementation.md)
  Implementation of the first read-only child evidence grouping over existing History preview sources.
- [`20-metrics-report-projection-integration-plan.md`](20-metrics-report-projection-integration-plan.md)
  Integration plan for making metrics, report, and CLI projections consume shared child evidence grouping instead of duplicating joins.
- [`21-selection-projection-integration-plan.md`](21-selection-projection-integration-plan.md)
  Plan for deriving generation-local `SelectionInput` from child evidence groupings while preserving run, protocol, and archive refs.
- [`22-history-admission-read-query-plan.md`](22-history-admission-read-query-plan.md)
  Plan for verified History read/query APIs and later Crown admission of selected child evidence bundles.
- [`23-child-evidence-core-review.md`](23-child-evidence-core-review.md)
  Review of Agent 19 child evidence grouping correctness, authority boundaries, join conservatism, and test gaps.
- [`24-child-evidence-review-fixes.md`](24-child-evidence-review-fixes.md)
  Focused fixes for ambiguous child evidence joins, branch metadata conflict diagnostics, and regression coverage.
- [`25-metrics-child-evidence-projection.md`](25-metrics-child-evidence-projection.md)
  First metrics/dashboard projection over shared child evidence grouping, preserving source treatments and dashboard behavior.
- [`27-child-evidence-operator-cli-exposure-plan.md`](27-child-evidence-operator-cli-exposure-plan.md)
  Plan for the smallest operator-facing CLI projection of grouped child evidence before selection consumes it.
- [`28-combined-child-evidence-review.md`](28-combined-child-evidence-review.md)
  Review of combined child evidence grouping, metrics consumption, selection-input coupling, ambiguity handling, and diagnostics.
- [`29-combined-review-fixes.md`](29-combined-review-fixes.md)
  Focused fixes for metrics branch ambiguity preservation, selection projection conflict failures, duplicate evaluation rejection, and diagnostics.
- [`30-typed-child-evidence-decoding.md`](30-typed-child-evidence-decoding.md)
  Typed-first child evidence decoding with parse provenance and fact-origin markers.
- [`31-child-evidence-cli-exposure.md`](31-child-evidence-cli-exposure.md)
  Read-only `history child-evidence` and monitor-alias CLI exposure for grouped child evidence.
- [`32-combined-typed-evidence-cli-review.md`](32-combined-typed-evidence-cli-review.md)
  Reviewer C assessment of typed evidence decoding, CLI projection boundaries, fail-closed behavior, and verification results.
- [`33-typed-evidence-boundary-cleanup.md`](33-typed-evidence-boundary-cleanup.md)
  Worker D cleanup making child evidence assembly consume typed `Stored<T>` records and abort on malformed declared records.
- [`34-typed-evidence-boundary-review.md`](34-typed-evidence-boundary-review.md)
  Reviewer E assessment of typed child-evidence boundaries, remaining loose document compatibility, and verification results.
- [`35-remaining-document-compat-cleanup.md`](35-remaining-document-compat-cleanup.md)
  Worker F cleanup moving metrics off loose Document child/evaluation projections and documenting remaining preview compatibility.
- [`36-remaining-document-compat-review.md`](36-remaining-document-compat-review.md)
  Reviewer G assessment of remaining Document compatibility cleanup, typed metrics inputs, and verification results.
