# Worker D: Typed Evidence Boundary Cleanup

Date: 2026-05-06

## Scope

Owned cleanup files:

- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
- `docs/active/agents/prototype1-hyperagents-design-2026-05-06/33-typed-evidence-boundary-cleanup.md`
- `docs/active/agents/prototype1-hyperagents-design-2026-05-06/README.md`

Concurrent edits already existed in nearby Prototype 1 CLI files and were not
reverted.

## Boundary Invariant

Prototype 1 child evidence no longer accepts degraded JSON evidence at this
store boundary. Declared persisted records crossing into child evidence are
`Stored<T>` values where `T: Serialize + DeserializeOwned + EvidenceRecord`.

`FsEvidenceStore::child_records` now deserializes declared child evidence files
as their typed record before `evidence.rs` can assemble a grouping. If any
declared file cannot deserialize as the expected record, the store returns
`PreviewError::ParseRecord` with a typed evidence boundary error. The CLI path
therefore aborts through `history_preview::run_child_evidence` instead of
rendering a partial evidence set.

## Records Wired

Typed child evidence inputs now include:

- `JournalEntry`
- `Prototype1NodeRecord`
- `Prototype1RunnerRequest`
- `Prototype1RunnerResult` for both latest runner-result and attempt-result
  files
- `Invocation`
- `SuccessorReadyRecord`
- `SuccessorCompletionRecord`
- `Prototype1BranchEvaluationReport`

`EvidenceRecord` identifies record name and schema, and `Stored<T>` preserves
the source pointer, class, ref id, path, line, and hash for successfully typed
records. Paths and hashes are provenance only; they are not semantic recovery
sources.

## Removed Normal Paths

The child evidence assembly path no longer uses:

- `ParsedDocument`
- `EvidenceParseStatus`
- JSON fallback extraction
- `document_coordinates`
- `json_coordinates`
- `path_field`
- `str_field`
- runtime/node/branch recovery from filenames or directory names
- filename fallback for evaluation branch ids

`Document` remains in `history_preview.rs` only for broader read-only
`history preview` catalog/projection compatibility. It is outside this patch's
child-evidence boundary and must not be used for typed child evidence, metrics,
selection, scoring, or authority semantics.

## Test Changes

Tests that previously depended on malformed-but-recoverable JSON fixtures now
write full typed source records. The regression for corrupt input now asserts
that a malformed declared `node.json` fails at the typed boundary instead of
assembling degraded evidence.

Selection projection fail-closed behavior remains covered for missing required
fields, duplicate evaluations, conflicted identity fields, conflicted branch
metadata, and ambiguous runtime/branch joins.

## Remaining Loose JSON Outside This Patch

Known loose JSON surfaces at this point in the sequence remained in:

- `history_preview.rs` preview entry/deferred-document projection over
  `Document`
- unrelated CLI/report parsing helpers

Those surfaces were not used by the normal `history child-evidence` assembly
path after this cleanup. The then-current `metrics.rs` loose-`Document` surface
called out in early review was removed by the later Worker F/H cleanup; current
metrics child/runtime/result/evaluation facts use typed `Stored<T>` records.
