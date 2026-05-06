# Agent 24: Child Evidence Review Fixes

Date: 2026-05-06

Scope:
- Implemented focused fixes from Agent 23 review in `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`.
- Kept the change inside read-only child evidence grouping.
- Did not add selection, report, metrics, or History admission wiring.

## Changes

Conflicted runtime and branch joins now become unusable for indirect placement. When a `runtime_id` or `branch_id` is observed with more than one `node_id`, the grouping:

- emits the existing conflict diagnostic;
- removes the key from the indirect join map;
- records the key as conflicted;
- refuses later runtime-only or branch-only placement through that key with a warning diagnostic;
- leaves those later sources in `unplaced`.

Directly attached node facts still group under their explicit `node_id`, even when they helped reveal an ambiguous runtime or branch key.

Branch metadata merges now diagnose conflicts for:

- `candidate_id`;
- `source_state_id`;
- `target_relpath`.

The first retained branch metadata value is still preserved, but conflicting later values are visible in child diagnostics.

## Tests

Added focused unit coverage for:

- ambiguous runtime and branch joins refusing later indirect evidence;
- branch metadata conflict diagnostics for candidate, source-state, and target path disagreements.

The tests use `FsEvidenceStore` so they cover the real preview document and transition-journal loading path.

## Verification

Passed:

```text
cargo fmt --all
cargo test -p ploke-eval --lib ambiguous_runtime_and_branch_joins_are_not_reused
cargo test -p ploke-eval --lib branch_metadata_conflicts_are_diagnostic
cargo test -p ploke-eval --lib groups_child_documents_with_source_refs
cargo check -p ploke-eval
```

Existing warnings remain, mostly dead-code warnings in Prototype 1 History scaffolding and existing `syn_parser` warnings.
