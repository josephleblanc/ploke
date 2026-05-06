# Reviewer G: Remaining Document Compatibility Review

Date: 2026-05-06

## Findings

No blocking findings.

Resolved follow-up: at review time the `Document` rustdoc in
`crates/ploke-eval/src/cli/prototype1_state/history_preview.rs` still implied a
metrics compatibility surface. The implementation no longer routes metrics
through `Document`; scheduler and branch-registry selection markers are loaded
as typed `SelectionPreviewRecords`. Worker H corrected the stale wording in
Report 37.

## Review Result

`history metrics` now avoids loose `Document` records for child/runtime/result
and evaluation facts. The normal build path calls `FsEvidenceStore::child_records()`,
constructs `ChildEvidenceSet::from_records(&child_records)`, applies typed child
evidence, applies typed child records, folds typed journal records, and then
applies typed selection preview records.

Metrics that need child/evaluation evidence now come from
`ChildEvidenceRecords` and `ChildEvidenceSet`. Evaluation metrics are folded from
typed `EvaluationEvidence` compared-run records and typed
`OperationalRunMetrics`, not from generic JSON values.

Path and filename semantic fallbacks are absent from the normal metrics
child/evaluation placement path. Paths remain in `SourceRef` and
`EvidencePointer` as provenance for successfully parsed typed records.

Scheduler and branch-registry selected-row markers are typed preview records:
`Stored<Prototype1SchedulerState>` and `Stored<Prototype1BranchRegistry>`.
Metrics labels those markers as `mutable_projection`, while transition-journal
selection keeps stronger `transition_journal` authority. These preview markers
drive dashboard display only; they do not feed child evidence assembly or typed
selection input construction.

Remaining loose `Document` consumers are confined to generic `history preview`
compatibility: `documents()`, `EvidenceIndex::from_documents()`,
`document_entries()`, `project_document()`, `deferred_documents()`, and
`source_summary()`. Those paths are explicitly labeled preview/report
compatibility and produce degraded/projection evidence, not authority.

I did not find a remaining code path where loose `Document` facts feed
selection-like, metrics-like, or child-evidence-like semantics. The one stale
rustdoc phrase above should be cleaned up, but the code path itself is typed.

The hard boundary invariant is preserved: typed file loading uses
`load_typed_record<T: EvidenceRecord>()`, and malformed declared records return
`PreviewError::ParseRecord` with a typed evidence boundary error. They are not
degraded into loose evidence.

## Verification

- `cargo fmt --all` passed.
- `cargo test -p ploke-eval --lib prototype1_state::evidence` passed.
- `cargo test -p ploke-eval --lib prototype1_state::metrics` passed.
- `cargo test -p ploke-eval cli::tests::history_child_evidence_should_parse` passed.
- `cargo test -p ploke-eval cli::tests::prototype1_monitor_child_evidence_should_parse` passed.
- `cargo check -p ploke-eval` passed.
- `git diff --check` passed.

Existing warning noise remains, including unused `Document` accessor warnings in
`history_preview.rs`, but it did not indicate a live compatibility leak.

## Recommendation

Accepted. The documentation-only follow-up was completed by Worker H in
Report 37: `Document` is now documented as `history preview` catalog/projection
compatibility only, never as metrics, selection, child-evidence, scoring, or
authority input.
