# Agent 25: Metrics Child Evidence Projection

Date: 2026-05-06

## Scope

Implemented the first metrics/dashboard integration over the shared
`ChildEvidenceSet` grouping.

Files changed:

- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`
- `docs/active/agents/prototype1-hyperagents-design-2026-05-06/25-metrics-child-evidence-projection.md`
- `docs/active/agents/prototype1-hyperagents-design-2026-05-06/README.md`

No selection, History admission, report, or archive registry code was edited.
No `evidence.rs` accessor changes were needed.

## Implementation

`metrics::build` now calls `FsEvidenceStore::child_evidence()` and projects the
returned `ChildEvidenceSet` into the metrics assembly before the existing raw
document and transition-journal folds run.

The projection seeds dashboard rows from grouped child evidence:

- child `node_id`, `parent_node_id`, `generation`, and `branch_id`;
- runtime ids from grouped runtime evidence;
- branch-to-node joins from grouped branch and evaluation evidence;
- evaluation refs, disposition, compared-instance counts where present;
- source refs derived from grouped `EvidenceSource` pointers.

Existing raw JSON folds still compute operational metrics, status, mutable
selection projections, and transition-journal selection observations. This keeps
current dashboard behavior stable while moving the row identity/provenance path
toward the shared child evidence join.

## Public/API Changes

The serialized metrics dashboard and slices now include a `child_evidence`
summary field:

- `join: "history_preview.child_evidence"`
- child count
- unplaced source count
- diagnostic count
- document, journal, and evaluation source counts
- unique source treatment counts by evidence class and treatment string

The table output header now prints one compact `child_evidence` summary line.
No existing metric row fields were removed or renamed.

## Authority Boundaries

The metrics projection remains a read-only view. The child evidence summary and
row source refs are citations over preview evidence; they do not admit History
entries, select successors, rank HyperAgents, or upgrade scheduler/latest-result
files into stronger facts.

Selection authority remains exactly the existing metrics projection labels:
`transition_journal` and `mutable_projection`.

## Verification

Passed:

```text
cargo fmt --all
cargo check -p ploke-eval
cargo test -p ploke-eval --lib dashboard_exposes_child_evidence_projection_summary
cargo test -p ploke-eval --lib dashboard_ranks_selected_child_with_sources
cargo test -p ploke-eval --lib rows_include_richer_operational_metrics
```

Existing warnings remain, mostly dead-code warnings in Prototype 1 History
scaffolding and existing `syn_parser` warnings.

## Deferred Integration

- Metrics still loads the raw documents and transition journal after
  `child_evidence()` because operational metric totals are not yet carried by
  `EvaluationEvidence`.
- The old metrics joins remain as compatibility folds for status, operational
  counters, and selection projection. Later work can move more of those fields
  into the shared child evidence grouping.
- Report, successor selection, History admission, HyperAgents scoring, and
  archive registry integration remain unwired.
