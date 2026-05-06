# Worker F: Remaining Document Compatibility Cleanup

Date: 2026-05-06

## Scope

Owned cleanup files:

- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`
- `docs/active/agents/prototype1-hyperagents-design-2026-05-06/35-remaining-document-compat-cleanup.md`
- `docs/active/agents/prototype1-hyperagents-design-2026-05-06/README.md`

Concurrent Prototype 1 edits already existed in the worktree and were not
reverted.

## Boundary Result

`history metrics` no longer hydrates child, runtime, result, or evaluation
facts from loose `Document` JSON. The normal metrics build path now loads
`ChildEvidenceRecords`, constructs `ChildEvidenceSet` from those typed records,
and folds typed child evidence into dashboard rows.

Evaluation metrics now come from typed `EvaluationEvidence` compared-run
records, including typed `OperationalRunMetrics`. Node status, runner status,
runner disposition, result refs, branch ids, generations, and parent ids come
from typed records or the typed child evidence grouping. Paths remain source
provenance for successfully parsed records.

## Document Compatibility Left

Loose `Document` remains only in `history_preview.rs` for generic
`history preview` import/report compatibility:

- `EvidenceStore::documents()`
- `EvidenceIndex::from_documents()`
- `document_entries()`
- `project_document()`
- `deferred_documents()`
- `source_summary()`

Those paths still do preview-only JSON field extraction and path-derived
subjects so `history preview` can catalog older adjacent files. The trait and
`Document` docs now explicitly say this is not valid typed child evidence,
metrics scoring input, or successor-selection evidence.

## Metrics Selection Preview

Metrics still shows operator-facing selected-row markers from mutable scheduler
and branch-registry state, but this no longer uses loose `Document` scanning.
`FsEvidenceStore::preview_selection_records()` loads typed
`Prototype1SchedulerState` and typed `Prototype1BranchRegistry` records as
`Stored<T>`.

This remains mutable preview evidence below History/Crown authority. It is used
for dashboard selection display and trajectory projection only; it does not feed
`ChildEvidenceSet`, `SelectionInput`, child-evidence assembly, or scoring facts.

## Review Questions

- What loose `Document` consumers remain after the patch?
  Only `history_preview.rs` generic preview/report compatibility consumers:
  `documents()`, `EvidenceIndex::from_documents()`, `document_entries()`,
  `project_document()`, `deferred_documents()`, and `source_summary()`.

- Do any remaining loose `Document` paths feed metrics/selection/child-evidence
  semantics?
  No. `metrics.rs` no longer calls `documents()` and does not consume
  `Document`. Scheduler/registry selection markers are typed mutable preview
  records, not loose document projections, and they do not feed child evidence
  or `SelectionInput`.

- Are path/filename fallbacks still used for semantic child/evaluation facts
  anywhere in the normal projection path?
  No. Metrics child/evaluation placement no longer has node-id, runtime-id, or
  branch-id fallback from paths/filenames. Path and hash fields are provenance
  for typed records.

- What was intentionally left as operator preview compatibility?
  `history preview` still catalogs generic JSON documents and may use path-based
  preview subjects. Metrics intentionally retains typed mutable scheduler and
  branch-registry selected-row display as operator preview compatibility.

## Verification

Initial metrics tests failed because old fixtures wrote partial JSON declared
records. The fixtures were updated to write full typed node, runner-result, and
evaluation records instead of relaxing production parsing.

Commands run successfully during this patch:

- `cargo fmt --all`
- `cargo test -p ploke-eval --lib prototype1_state::evidence`
- `cargo test -p ploke-eval --lib prototype1_state::metrics`
- `cargo test -p ploke-eval cli::tests::history_child_evidence_should_parse`
- `cargo test -p ploke-eval cli::tests::prototype1_monitor_child_evidence_should_parse`
- `cargo check -p ploke-eval`
- `git diff --check`
