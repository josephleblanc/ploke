# Agent 23: Child Evidence Core Review

Date: 2026-05-06

Scope:
- Reviewed reports `14` through `19`.
- Reviewed Agent 19 source changes in `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`, `history_preview.rs`, and `mod.rs`.
- Ran focused verification.
- Source code was not modified.

## Findings

### High: Conflicted joins remain usable as if unambiguous

`crates/ploke-eval/src/cli/prototype1_state/evidence.rs:190` diagnoses a conflicting runtime or branch join, but it leaves the first mapping in `runtime_to_node` / `branch_to_node`. Later, `node_for` still uses that mapping for sources that only have the runtime id or branch id (`evidence.rs:276`).

That is not conservative enough for child evidence grouping. Once `runtime_id -> node_id` or `branch_id -> node_id` conflicts, subsequent runtime-only or branch-only evidence can be attached to the first node even though the key is known ambiguous. The diagnostic is honest about the first conflict, but the projection can still make an unsupported join afterward.

Minimal fix recommendation: track conflicted runtime and branch keys, or remove the map entry on conflict. `node_for` should refuse ambiguous keys and leave those later sources unplaced with a diagnostic such as `ambiguous runtime join` or `ambiguous branch join`. Add tests for two node records or journal rows that bind the same runtime/branch to different nodes, followed by an evidence document that only carries that runtime/branch.

### Medium: Branch metadata conflicts are silently collapsed

`merge_branch` keeps the first `candidate_id`, `source_state_id`, and `target_relpath`, then fills only missing fields from later sources (`evidence.rs:382`). If two sources agree on `branch_id` but disagree on candidate/source/target coordinates, the grouping emits no diagnostic and preserves the first value.

This is weaker than the child-level `merge_option` behavior at `evidence.rs:315`, which reports conflicting `parent_node_id`, `generation`, and `branch_id`. Branch metadata is selection-relevant evidence, so silent first-writer-wins behavior can hide exactly the disagreement that later metrics or selection projections need to see.

Minimal fix recommendation: use a diagnostic-producing merge for `BranchEvidence` fields as well, either by reusing `merge_option` with a branch diagnostics sink or by adding branch-local diagnostics. A conflicting branch coordinate should not rewrite the chosen value, but it should be visible.

### Low: Test coverage proves the happy path but not the risk boundaries

The new unit test at `evidence.rs:720` verifies node/result/evaluation grouping, source refs, hashes, compared run paths, and a basic branch join. It does not cover:

- ambiguous runtime or branch joins;
- missing `branch_id` evaluation fallback diagnostics;
- unplaced degraded evidence;
- preservation of `projection_only` / degraded treatments;
- no accidental History/Crown authority claim in serialized output.

These are the exact boundaries this patch is meant to protect. The current test is meaningful, but it should be extended before downstream report, metrics, or selection code relies on this grouping.

## Checklist Review

The grouping is conceptually in the right layer. It extends `history_preview::EvidenceStore` with a read-only `child_evidence()` projection (`history_preview.rs:38`, `history_preview.rs:144`) and builds over existing `Document`, `Stored<T>`, `EvidencePointer`, and `EvidenceClass` rather than creating a second locator or authority store.

The implementation preserves source pointers, hashes, evidence class, and existing class treatment through `EvidenceSource` (`evidence.rs:89`). It does not write History blocks, create `Entry<Admitted>`, use `Crown`, or claim sealed authority. The module comment explicitly says it does not admit History entries or upgrade degraded/projection sources (`evidence.rs:1`). This is a good conceptual fit for reports 14 and 18.

The public surface is crate-private and read-only. `ChildEvidenceSet::from_sources` consumes existing evidence records and returns a projection. `FsEvidenceStore::child_evidence()` only composes `transition_journal()` and `documents()`. There is no mutation API and no admission API.

The naming is mostly structurally acceptable. `ChildEvidence`, `RuntimeEvidence`, and `BranchEvidence` are projection carriers, not typed protocol states, and they do not introduce flattened role/state transition names. Existing flattened journal variants such as `ChildReady` remain outside this patch.

Adding `Deserialize` to `EvidencePointer` and `EvidenceClass` does not materially weaken History invariants by itself. Both types are crate-private, `EvidencePointer` is a source citation rather than a recovery capability, and the grouping does not feed sealed History admission. The residual risk is future misuse: a deserialized pointer can be fabricated, so it must not later be treated like `history::Locator<T>` or verified authority.

One naming caveat: `EvidenceClass::treatment()` still returns labels such as `admitted_preview` and `admitted_preview_raw` (`history_preview.rs:622`). Agent 19 did not invent those labels, and the surrounding module documents preview-only status, but downstream projections should treat them as preview/source-treatment strings, not as sealed History admission status. A later enum with explicit degraded/projection/source-treatment variants would be safer than string labels containing `admitted`.

## Verification

Passed:

```text
cargo check -p ploke-eval
cargo test -p ploke-eval --lib groups_child_documents_with_source_refs
```

Both commands completed successfully. Existing warnings remain, mostly dead-code warnings in Prototype 1 History scaffolding and existing `syn_parser` warnings.
