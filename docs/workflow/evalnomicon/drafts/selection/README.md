# Prototype 1 Selection: Current Approach

Status: current as of 2026-06-07 by code inspection. Older selection
identity, fanout, and impact-audit drafts were archived under
[`docs/archive/workflow/evalnomicon/drafts/selection-2026-05-21/`](../../../../archive/workflow/evalnomicon/drafts/selection-2026-05-21/).

This document describes the current implementation shape, not the earlier
draft intent.

## Primary Code Anchors

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `Prototype1SuccessorSelection::active_strategy`
  - `select_successor_for_profile`
  - `ParentSelection::select_successor`
  - `SelectionSealMaterial`
- `crates/ploke-eval/src/successor_selection/traversal.rs`
  - `Candidates::traverse_with_policy`
  - `select_score_child_prop`
  - `score_child_prop_formula`
- `crates/ploke-eval/src/successor_selection/metrics.rs`
  - `Set::from_considered`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  - `CandidateOccurrenceId`
  - `CandidateMembershipId`
  - `CandidateSetCommitment`
  - `SelectionDecisionEntry::new_with_traversal_identity_metrics`
- `crates/ploke-tree/src/graph/build/selection.rs`
  - `Builder::ingest_selection`
  - `Builder::ingest_selection_metrics`
- `crates/ploke-tree/src/graph/types/selection.rs`
  - `SelectionNode`
  - `MetricSetNode`
  - `MetricCandidateNode`
  - `SelectionFormulaNode`
- `crates/ploke-egui/src/ui/inspector.rs`
  - `CandidateComparisonSlot`
  - `CandidateComparisonCandidate`
- `crates/ploke-egui/src/ui/app/shell.rs`
  - `render_candidate_comparison_for_inspector`

## Code-Rooted Flow

1. Runtime selection starts from `select_successor_for_profile` in
   `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`.

   The run profile chooses a candidate scope and traversal strategy through
   `Prototype1SuccessorSelection::active_strategy`:

   - `GenerationLocal` uses the current generation scope with `score_child_prop`.
   - `HistoryFrontierMax` uses all admitted History candidates with frontier max.
   - `HistoryScoreChildProp` uses all admitted History candidates with
     `score_child_prop`.

2. `ParentSelection::select_successor` builds the candidate universe.

   It loads History candidates for the requested scope, removes the exact active
   parent, projects current-generation child outcomes into candidate payloads,
   replaces matching History payloads for the same node/branch coordinate only
   when the current-generation payload is decision-grade, then calls
   `successor_selection::traversal::select_with_policy`.

3. `Candidates::traverse_with_policy` filters to decision-grade candidates.

   The traversal layer records projection failures separately from the ordered
   considered list, excludes candidates that have already produced successful
   children, computes a selection metric set from the remaining considered
   payloads and source classes, and asks the selected strategy to choose a
   candidate. The chosen candidate is then bound back to a candidate-set
   membership.

4. `score_child_prop` is the main stochastic strategy.

   It computes per-candidate weights from decision outcome, operational or
   operational-plus-protocol metric inputs, optional oracle policy, expansion
   count, and `imp@k` score deltas. Expansion count is the candidate's own
   successful-child count plus the successful-child count of its parent
   coordinate. Candidates with successful children are excluded before sampling;
   their descendants remain eligible but carry the parent-expansion penalty. The
   strategy samples with a deterministic seed and records rationale fields such
   as total weight, sample, selected weight, performance, child count, alpha,
   exploitation, and exploration.

5. `SelectionSealMaterial` carries the selected result to History sealing.

   It preserves:

   - procedure id and selection scope
   - legacy selected `SubjectRef`
   - selected occurrence and membership ids when available
   - ordered considered payloads and their source classes
   - projection failures
   - traversal replay parameters, including pre-pruning child counts
   - selection metrics

   Pre-pruning child counts are carried in `TraversalEvidence` so debugger
   formula and replay views use the same expansion context that live
   `score_child_prop` used before expanded candidates were removed from the
   final considered set.

   Payload lookup prefers selected membership id, then selected occurrence id,
   then legacy selected candidate label. The label is compatibility data, not
   the preferred authority surface.

6. `SelectionDecisionEntry::new_with_traversal_identity_metrics` is the sealed
   History boundary.

   It reconstructs the candidate-set commitment, derives or validates the
   selected candidate projection, validates the decision against occurrence or
   membership identity, validates metric bindings against the considered-order
   hash and candidate-set root, then stores:

   - candidate set and considered-order hash
   - traversal evidence
   - selection metrics
   - score formula rows when the traversal supports them
   - final successor decision

7. `ploke-tree` ingests the sealed selection entry into typed graph indexes.

   `SelectionNode` carries the reduced selection header. `MetricSetNode`,
   `MetricCandidateNode`, and `SelectionFormulaNode` carry selection-time
   metrics, per-candidate `imp@k` and compared-run metric inputs, and
   `score_child_prop` formula rows. Metric ingestion warns when metric-set
   bindings do not match the sealed decision input.

8. `ploke-egui` renders from `ploke_tree::Graph`.

   The Candidate Comparison inspector joins child plans to candidates, then to
   selection nodes, metric candidates, and formula rows. The renderer displays
   `not_recorded` when a typed graph fact is absent or when upstream sealed
   evidence is sparse. It should not reparse History JSON or infer selection
   facts from rendered strings.

## Current Defaults

The run profile maps selection configuration into traversal and metric policy.
The current profile example preserves these defaults:

- selection metrics persist by default
- score profile is `operational-quality-v1`
- `imp@k` is enabled with budget `50`
- `imp@k` score weight is `0`, so it is recorded without changing scores unless
  policy changes
- oracle mode is record-only unless configured otherwise

## Practical Debug Expectations

When the graph join is healthy, Candidate Comparison should normally show:

- selected marker
- child id
- candidate id
- payload index
- score row fields from `score_child_prop`
- metric-set header fields
- `imp@k` counts when metric candidates were persisted

Expected sparse fields include oracle rate under record-only oracle mode and
protocol deltas when baseline or treatment protocol metrics were not sealed for
both arms of the compared run.

If child and candidate columns appear but most score-row fields are
`not_recorded`, check the `SelectionFormulaNode` join. If metric and `imp@k`
fields are missing while metrics persistence is enabled, check
`MetricCandidateNode` ingestion and the metric-set binding warnings.

## Archived Drafts

The archived May 2026 drafts still have historical value:

- occurrence selection plan: rationale and regression checklist for occurrence
  and membership ids
- fanout review: runtime risk and test backlog for multi-child execution
- impact audit: older projection-vs-authority framing

They should not be read as current implementation docs without refreshing their
claims against the code above.
