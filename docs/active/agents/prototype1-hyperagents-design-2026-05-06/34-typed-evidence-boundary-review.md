# Reviewer E: Typed Evidence Boundary Review

Date: 2026-05-06

## Findings

No blocking findings.

Worker D's cleanup moves the normal `history child-evidence` path onto typed
records. `FsEvidenceStore::child_evidence()` now calls `child_records()` and
then `ChildEvidenceSet::from_records()`, so the normal child evidence assembly
does not call `documents()` or assemble from loose `Document` payloads.

Typed deserialization is now the persisted evidence boundary for declared child
records. `load_typed_record<T>()` parses JSON into `T: EvidenceRecord` before it
constructs a `Stored<T>` pointer, and parse failures return
`PreviewError::ParseRecord`. The transition journal path is also typed through
`load_jsonl<T>()`; malformed journal rows abort before assembly through
`PreviewError::ParseLine`.

`Stored<T>` is constrained through `T: EvidenceRecord`, and `EvidenceRecord`
itself requires `Serialize + DeserializeOwned`. That satisfies the persisted
boundary requirement without keeping an unconstrained generic store carrier.

The normal child-evidence assembly no longer contains `ParsedDocument`,
`EvidenceParseStatus`, JSON fallback extraction, path/filename semantic
recovery, `str_field`, `path_field`, or JSON coordinate helpers. The grouped
facts now come from typed records and use `EvidenceFactOrigin::Typed`.

Source refs, paths, line numbers, hashes, and ref ids remain provenance for
successfully parsed typed records. They are attached after typed parse succeeds
and are not used to recover node, runtime, branch, evaluation, or metric
semantics in the child-evidence assembly path.

Selection projection remains below History authority and fails closed. The
projection returns `SelectionProjectionError` when required evidence is missing,
ambiguous, conflicted, duplicated, or malformed, and the child evidence module
still documents that selection projection does not admit History entries or
upgrade preview evidence into Crown authority.

## Loose Document Compatibility

Current correction: remaining loose `Document` use is confined to generic
`history_preview.rs` preview catalog/projection code. It must not feed typed
child evidence, metrics, selection, scoring, or authority semantics.

- `history_preview.rs` still has the broad preview/deferred-document projection
  over `Document`, including `str_field` and runtime id recovery for preview
  rendering.

This is not a leak into `FsEvidenceStore::child_evidence()`,
`ChildEvidenceSet::from_records()`, or the current metrics path. The earlier
review finding that `metrics.rs` still loaded `store.documents()` was resolved
by the later Worker F/H cleanup; current metrics row hydration uses typed
`Stored<T>` child/runtime/result/evaluation records and typed scheduler/registry
preview records for dashboard selection markers.

## Verification

Ran:

- `cargo fmt --all`
- `cargo test -p ploke-eval --lib prototype1_state::evidence`
- `cargo test -p ploke-eval cli::tests::history_child_evidence_should_parse`
- `cargo test -p ploke-eval cli::tests::prototype1_monitor_child_evidence_should_parse`
- `cargo check -p ploke-eval`
- `git diff --check`

All commands passed. Cargo emitted existing warning noise in `syn_parser` and
Prototype 1 dead-code surfaces.

## Recommendation

Accepted; the follow-up metrics cleanup was completed by later Worker F/H
changes.

The typed child-evidence boundary cleanup satisfies the review criteria for the
normal command path and aligns with the Prototype 1 invariant that corruption is
an error, not an imprecision vector. Current wording should not be read as
permission for loose `Document` semantic extraction in metrics, selection,
child evidence, scoring, or authority paths.
