# Agent 20: Metrics/Report Projection Integration Plan

Date: 2026-05-06

Scope:
- Primary: `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`,
  `report.rs`, and CLI projection code in `cli_facing.rs`.
- Input reports: `14` through `18`. Report `19` was not present in the
  directory at inspection time.
- Source code was not modified.

## Executive Finding

`metrics.rs` already consumes `history_preview::FsEvidenceStore`, preserves
`SourceRef` path/hash provenance, and contains the most complete current join
operator in `Assembly`. `report.rs` still performs a separate load path through
typed scheduler/registry/journal/evaluation readers, then summarizes away most
source provenance. CLI projection code adds more independent joins for loop,
timing, branch status, runner, and selection input views.

The integration plan should not move authority into `metrics` or `report`.
Instead, promote the current `metrics::Assembly` join shape into a shared child
evidence grouping at the `history_preview` evidence layer, then make metrics,
report, and CLI projections consume that grouping. The grouping should preserve
the existing read-only/degraded status of the evidence, not present itself as
sealed History.

## Existing Pieces To Preserve

Preserve these as the evidence discovery boundary:

- `history_preview::EvidenceStore`
- `history_preview::FsEvidenceStore`
- `history_preview::Stored<T>`
- `history_preview::Document`
- `history_preview::EvidencePointer`
- `history_preview::EvidenceClass`
- `EvidenceStore::transition_journal`
- `EvidenceStore::documents`
- `FsEvidenceStore::new`
- `EvidenceClass::as_str`
- `EvidenceClass::treatment`

These already discover scheduler, branch registry, evaluations, invocation
files, attempt results, successor ready/completion files, node records, runner
requests, latest runner results, and transition journal entries while carrying
class, ref id, path, line where applicable, and content hash.

Preserve these metrics projection entry points:

- `metrics::run`
- `metrics::MetricRequest`
- `metrics::Mode`
- `metrics::build`
- `metrics::Dashboard`
- `Dashboard::slice`
- `Dashboard::print`

The external shape of the metrics command should remain a projection. Its
schema version may need a later bump when rows are derived from the shared
grouping, but the command should still build a dashboard and print/table or
JSON slice output.

Preserve these report projection entry points:

- `report::run`
- `Report::load`
- `Report::print`
- `SchedulerView`
- `RegistryView`
- `JournalView`
- `EvaluationView`
- `DedupedFields`

The report should remain a provisional campaign report, not a source of facts.
The `*View` types can survive as view/fold outputs over the shared grouping.

Preserve these CLI projection consumers:

- `run_metric_slice`
- `run_history_preview`
- `print_prototype1_monitor_timing`
- timing `Report::load`
- timing `NodeEvidence`
- `Prototype1LoopReport`
- `Prototype1LoopBranchEvaluationSummary`
- `Prototype1BranchStatusReport`
- `Prototype1RunnerReport`
- `Prototype1StateReport`
- `Prototype1BranchEvaluationReport`
- `Prototype1ComparedInstanceReport`
- `selection_input_from_child_report`
- `select_most_promising_branch` as the legacy local heuristic

These should consume or cite grouped evidence where useful, but they should not
become the shared join layer.

## Joins That Should Move Or Be Shared

Move these joins out of `metrics::Assembly` into a shared child evidence
grouping:

- `branch_to_node: BTreeMap<String, String>` from branch id to node id.
- Node row population from `apply_node`.
- Runner request population from `apply_request`.
- Runtime id and role population from `apply_invocation`.
- Attempt/latest result population from `apply_result`.
- Evaluation totals keyed by branch id from `apply_evaluation`.
- Evaluation-to-node attachment from `apply_evaluation_to_node`.
- Selection evidence collection from `apply_journal`,
  `apply_selection_projection`, `record_selection`, and
  `strongest_selection_authority`.
- Fallback identity diagnostics currently implicit in `fallback_node_id`,
  `file_stem`, path-derived runtime ids, and evaluation filename stems.

Keep these in metrics as dashboard-only folds:

- `ScoreDerivation::from_row`
- `assign_ranks`
- `generations`
- `cohorts`
- `Trajectory::from_cohorts`
- dashboard score and rank fields
- table printers

Move or share these `report.rs` joins over the same grouping:

- `selected_trajectory`, currently scheduler-only through
  `last_continuation_decision` and `parent_node_id`.
- `DedupedFields::from_sources`, currently a provenance-free union of ids.
- `EvaluationView::from_reports`, currently loaded through separate
  filesystem scanning and losing document hashes.
- `summarize_compared_instances`, or its metric fold, so report and metrics
  count operational totals the same way.

Keep these in `report.rs` as view-only folds:

- `SchedulerView::from_state` counts.
- `RegistryView::from_registry` counts.
- `JournalView::from_entries` counts.
- `EvaluationView` aggregate and row layout.
- human table printing.

Share, but do not promote, these CLI joins:

- timing `Report::load` joins scheduler nodes, `load_node_record`,
  `load_runner_result`, observation steps, provider HTTP events, turn traces,
  stream timings, and run artifacts.
- `summarize_prototype1_branch_evaluation` folds
  `Prototype1BranchEvaluationReport` into compact loop summaries.
- `summarize_prototype1_failed_branch_evaluation` produces a runner-result-only
  reject summary.
- `selection_input_from_child_report` projects `Prototype1BranchEvaluationReport`
  into `SelectionInput`.

These are useful projections over child evidence but should not define the
shared evidence object because they already discard run record paths, source
hashes, per-instance reasons, model/provider context, or observation custody.

## Proposed Shared Grouping

Add the shared operator under `history_preview` or a sibling evidence module
inside `prototype1_state`, not under `metrics` or `report`.

Working shape:

```text
FsEvidenceStore
-> transition_journal + documents
-> ChildEvidenceSet
-> ChildEvidenceGroup keyed by node/branch/runtime where recoverable
-> metrics/report/CLI projections
```

Suggested type roles, with final names left to the implementation pass:

- `ChildEvidenceSet`: one campaign-level collection plus diagnostics.
- `ChildEvidenceGroup`: one candidate child/node/branch grouping.
- `ChildEvidenceSource`: source pointer, evidence class, authority treatment,
  parse status, and derivation note.
- `ChildIdentity`: campaign id, node id, parent node id, generation, branch id,
  candidate id, runtime ids, source state id, target relpath.
- `EvaluationEvidence`: evaluation document ref, branch disposition, per-instance
  comparison paths and operational metrics.
- `ResultEvidence`: invocation, attempt result, latest runner result, successor
  ready/completion refs.
- `SelectionEvidence`: branch id plus authority class such as
  `transition_journal` or `mutable_projection`.

This object should be read-only and degraded unless/until another History
transition admits a fact. It should not expose arbitrary mutation or sealed
authority APIs.

## Provenance That Must Not Be Lost

Every grouped fact should retain:

- `EvidenceClass`.
- `EvidencePointer.ref_id`.
- source `path`.
- source `line` for JSONL journal entries when available.
- source content `hash`.
- parse status: typed parsed, JSON parsed, raw/unparsed, or parse failed.
- authority treatment from `EvidenceClass::treatment`.
- whether an identity came from a typed field or a fallback path/filename.
- whether selection came from transition journal or mutable projection.
- whether a runner result is attempt-scoped or the mutable latest copy.
- evaluation artifact path and evaluation source hash.
- branch registry path and scheduler path when they contributed projected data.
- baseline and treatment `record.json.gz` paths from
  `Prototype1ComparedInstanceReport`.
- treatment campaign manifest and closure-state paths from
  `Prototype1BranchEvaluationReport`.
- runtime id, node id, branch id, candidate id, source state id, target relpath,
  generation, and parent node id where recoverable.
- diagnostics for conflicts, missing joins, path-stem fallbacks, duplicate
  selected branches, and unavailable lineage.

Do not collapse these into a single "selected" or "succeeded" boolean. The
projection may render booleans, but the grouping should retain the evidence
chain that produced them.

## Implementation Sequence

1. Add source-level tests around the current behavior before moving code.
   Cover one campaign fixture with node, request, invocation, attempt result,
   latest runner result, evaluation, scheduler selection, and transition-journal
   selection. Assert that `metrics::build` rows keep current fields and
   `source_refs` hashes.

2. Introduce a shared child evidence grouping builder that only wraps the
   existing `FsEvidenceStore` data. Initial implementation can copy the
   extraction logic from `metrics::Assembly` without changing output. Keep all
   new types crate-private.

3. Move `SourceRef` semantics into the shared evidence layer. Either reuse the
   same fields or make a new internal source type with `From<&Document>` and
   `From<&EvidencePointer>`. Metrics can keep a serializable `SourceRef` view,
   but the grouping owns provenance.

4. Move `branch_to_node`, `apply_node`, `apply_request`, `apply_invocation`,
   `apply_result`, `apply_evaluation`, `apply_evaluation_to_node`,
   `apply_journal`, `apply_selection_projection`, and `record_selection` into
   the grouping builder. Keep `metrics::Assembly` temporarily as an adapter
   from grouped child rows to dashboard `Row`.

5. Make `metrics::build` call the grouping builder instead of directly iterating
   `store.documents()` and `store.transition_journal()`. Dashboard ranking,
   cohorting, trajectory, slicing, and printing remain in `metrics.rs`.

6. Convert `report::Report::load` to consume the same grouping while preserving
   typed scheduler and registry counts. The first low-risk step is to source
   `EvaluationView` and `DedupedFields` from grouped evidence and leave
   `SchedulerView`/`RegistryView` on the current typed readers.

7. Replace `report.rs::load_evaluations` with grouped evaluation evidence or
   demote it to a compatibility helper used only by tests. Evaluation rows
   should cite source hash/ref id rather than only `evaluation_artifact_path`.

8. Replace `report.rs::selected_trajectory` with a projection over grouped
   selected child evidence. Preserve the current scheduler-only trajectory as a
   degraded fallback and label it as mutable projection in diagnostics.

9. Update `selection_input_from_child_report` or add a sibling conversion from
   grouped evidence to `SelectionInput`. Keep the current function until the
   loop controller can pass grouped evidence. The conversion must preserve
   baseline/treatment record paths in an adjacent evidence ref even if
   `SelectionInput` cannot yet carry them.

10. Optionally feed timing and loop reports with grouped child identity so they
    share node/branch/runtime resolution. Keep observation-log and provider HTTP
    joins as timing-only evidence, especially where old unscoped attempts are
    attributed by compatibility heuristics.

11. Add diagnostics to both metrics and report JSON output that distinguish:
    missing evidence, parse failure, mutable projection, path fallback, duplicate
    branch selection, and conflict between attempt-scoped and latest runner
    result.

12. Only after the shared grouping is stable, add a separate History admission
    path that cites the same source refs through `EvidenceRef`, `Locator<T>`, or
    admitted entries. Do not make this grouping append sealed blocks.

## Low-Risk Migration Boundaries

The first code slice should not change command output intentionally. It should
only move where rows are assembled. A useful acceptance check is:

- `prototype1-state monitor history-metrics` produces the same dashboard rows
  for existing fixtures.
- `prototype1-state monitor report` still prints the same top-level sections.
- JSON output gains no new source claims unless the schema version is bumped.
- Existing diagnostics about degraded lineage remain present.

The second slice can add provenance fields to report JSON after the shared
grouping is the only source for evaluation and deduped fields.

The third slice can let successor selection use grouped evidence, but only as a
projection into the current generation-local `SelectionInput`. Archive-wide
parent selection is a separate design step and should not be smuggled into this
metrics/report migration.

## Preservation Check

The grouping must preserve the causal chain:

```text
filesystem evidence/ref/hash
-> child/node/branch/runtime grouping
-> metric/report/selection/timing projection
```

It must not reverse that chain by treating a dashboard row, report row, timing
row, latest runner-result copy, scheduler trajectory, or branch-status summary
as the fact source. Those are projections. The fact source remains the
classified evidence document, journal line, typed run/evaluation record, or
later sealed History entry that the projection cites.
