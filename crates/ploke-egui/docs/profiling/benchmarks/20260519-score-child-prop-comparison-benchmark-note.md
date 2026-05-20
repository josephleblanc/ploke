cargo test benchmark filter verified the ploke-egui benchmark test surface, not a native interactive window.

Change summary:

- Added selection-entry-scoped `score_child_prop` formula records to the typed record/graph path.
- Extended the Candidate Comparison inspector witness and table to show the formula summary and side-by-side per-candidate selector fields: score components, alpha, exploitation, exploration, weight, cumulative range, sample hit, inclusion state, and existing metric drilldown fields.

Commands or verification surfaces used:

- `cargo check -p ploke-egui 2>&1 | tail -n 80`
- `cargo test -p ploke-egui selected_child_artifact_exposes_candidate_comparison_from_parent_plan 2>&1 | tail -n 100`
- `cargo test -p ploke-egui benchmark 2>&1 | tail -n 120`

Baseline report compared:

- No benchmark baseline comparison was valid for this default check; only the bounded benchmark test filter was run.

Allocation/performance measurements:

- not measured

Measured improvements/regressions:

- not measured

Unmeasured risk and next action:

- The wider Candidate Comparison table may need native-window layout inspection with real run data before claiming interactive UI polish.
