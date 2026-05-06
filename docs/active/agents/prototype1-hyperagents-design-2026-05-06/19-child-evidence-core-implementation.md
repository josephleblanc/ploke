# Agent 19: Child Evidence Core Implementation

Date: 2026-05-06

## Scope

Implemented the first read-only evidence-layer patch for child evaluation
evidence grouping.

Files changed:

- `crates/ploke-eval/src/cli/prototype1_state/evidence.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `docs/active/agents/prototype1-hyperagents-design-2026-05-06/README.md`
- `docs/active/agents/prototype1-hyperagents-design-2026-05-06/19-child-evidence-core-implementation.md`

No report, selection, or History admission code was wired to consume the new
grouping in this patch.

## Public Types Added

In `prototype1_state::evidence`:

- `ChildEvidenceSet`
- `ChildEvidence`
- `RuntimeEvidence`
- `BranchEvidence`
- `EvaluationEvidence`
- `ComparedRunEvidence`
- `EvidenceSource`
- `EvidenceDiagnostic`

These structs are `Serialize`/`Deserialize` carriers. `EvidenceSource` embeds
the existing `EvidencePointer`, keeps the `EvidenceClass`, records the class
treatment string, and adds a modest source `kind` label for document classes or
journal variants.

## Public Functions And Methods Added

In `prototype1_state::evidence`:

- `ChildEvidenceSet::from_sources(journal, documents)`

In `prototype1_state::history_preview`:

- `EvidenceStore::child_evidence()`
- `FsEvidenceStore::child_evidence()`

Existing preview source types were extended as follows:

- `EvidencePointer` now derives `Deserialize`.
- `EvidenceClass` now derives `Deserialize`.
- `EvidenceClass::treatment()` is now `pub(crate)` so grouped evidence can
  preserve source classification treatment.

## Behavior

The new grouping consumes existing `EvidenceStore::transition_journal()` and
`EvidenceStore::documents()` output. It groups evidence by child node where a
node join can be recovered, then nests runtime, branch, and evaluation evidence
under that child.

Included documents and journal lines keep their original pointer, path, line
when present, payload hash, evidence class, and treatment. Evaluation evidence
preserves compared-instance baseline and treatment `record.json.gz` paths when
the source report contains them.

Diagnostics are emitted for:

- sources that cannot be joined to a child node;
- conflicting runtime-to-node or branch-to-node joins;
- conflicting child-level fields such as generation or branch id;
- evaluation documents that require a filename-stem branch fallback.

The grouping does not select a successor, rank candidates, write History
entries, or treat scheduler/latest-result projections as stronger than their
existing `EvidenceClass::treatment()` labels.

## Verification

Commands run:

- `cargo fmt --all`
- `cargo check -p ploke-eval` passed.
- `cargo test -p ploke-eval groups_child_documents_with_source_refs` passed
  once, then a repeat non-lib run failed during final binary linking with
  `cc`/`ld` terminated by signal 7 after compilation.
- `cargo test -p ploke-eval --lib groups_child_documents_with_source_refs`
  passed.

Existing repository warnings remain, mostly dead-code warnings in the History
prototype and an unused import warning in `syn_parser`.

## Deferred Invariants

- The new grouping is degraded/read-only evidence, not History authority.
- The join logic is intentionally conservative and JSON-field based where no
  typed reader is already exposed through `history_preview`.
- `child::Record` still exposes only runtime-level identity to this module, so
  some child journal rows may remain unplaced unless another source joins the
  runtime to a node.
- Scheduler and branch-registry projections are grouped as classified sources;
  no selection policy consumes them yet.
- Later projection work should derive metrics/report/selection inputs from this
  grouping instead of duplicating joins.
