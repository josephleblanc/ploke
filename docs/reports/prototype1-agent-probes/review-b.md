# Prototype 1 Agent Probe Review B

Review date: 2026-04-28 America/Los_Angeles.

Scope:

- `probe-1.md`
- `probe-2.md`
- `probe-3.md`
- `probe-4.md`
- `probe-5.md`

Context read:

- `AGENTS.md`
- `docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md`
- `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md`
- `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`
- `docs/workflow/evalnomicon/drafts/runtime-artifact-lineage.md`
- `docs/reports/prototype1-record-audit/history-admission-map.md`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`

## Summary

All five probes converge on the same next patch: make `history metrics --view
trajectory` cohort-aware and ambiguity-preserving. That recommendation is
sound. It targets a documented metrics risk without touching the live
History/Crown authority path, and it respects the current claim boundary:
Prototype 1 has useful projections and a History-shaped preview, not live
Crown-gated sealed History.

The probes are also mostly aligned with naming and structure discipline. They
avoid new History authority objects, do not promote mutable JSON records into
authority, and generally treat `(parent_node_id, generation)` as a degraded
coordinate rather than a true lineage id.

The main corrections are implementation-shape corrections:

- Do not let `trajectory_status: single_lineage` imply a proven lineage. Current
  records lack lineage id, and the metric can only say something like
  `unambiguous_under_degraded_coordinate`.
- Do not diagnose every zero-selection cohort as a correctness problem. In the
  broader model, not every cohort must produce a successor. Report zero
  selections as incomplete evidence only for the trajectory/cohort scope being
  inspected.
- Do not build the trajectory from generation summaries. The current
  `Generation` summary uses `find(|row| row.selected)`, which silently picks one
  selected row per generation. The patch must derive trajectory directly from
  `Row`s or from `Cohort`s.
- Do not treat `dashboard_score` as explaining `dashboard_rank`. The code ranks
  by `RankKey`, while score is a separate heuristic.
- Do not use focused JSON output as an accidental breaking API unless the CLI
  mode contract is intentionally changed. The current `Slice` always includes
  rows, generations, cohorts, selected-by-generation, and diagnostics.

## Model Checks

Projection vs authority:

- The probes consistently keep metrics as projections. This is correct.
- The implementation labels metrics as `prototype1 metrics projection` and
  derives from scheduler, branch registry, journal, node records, evaluations,
  and runner records. Those sources remain evidence or mutable projections, not
  sealed History.
- Selection source authority is already split between `transition_journal` and
  `mutable_projection`. The next patch should preserve that split and avoid a
  generic `selected=true` authority claim.

Cross-runtime Crown/LockBox handoff:

- The probes correctly avoid changing `history.rs` or live handoff. That is the
  right boundary for this metrics patch.
- `history.rs` explicitly records that live `Parent<Ruling>`,
  `Crown<Locked>`, and `Successor<Admitted>` do not yet gate handoff, and
  `SealBlock` commits a `crown_lock_transition` evidence ref rather than
  requiring a live Crown token.
- Therefore, trajectory output must not say or imply that selected rows are
  Crown-authorized successor handoffs. They are observed selections with source
  refs.

Lineage and generation:

- The conceptual model says runtime/artifact location is not just git ancestry
  or generation. It requires generator Runtime, target Artifact, derived
  Artifact, and hydrated Runtime when available.
- Current metrics cohorts use `lineage: None` and group by
  `(parent_node_id, generation)`. That is an intentional degraded coordinate.
- Any probe language that calls the result a "lineage projection" must be read
  as shorthand. The implementation should use wording such as "trajectory
  projection under degraded cohort coordinates" rather than "single lineage"
  unless a real lineage id is present.

Metrics semantics:

- `dashboard_rank` and `dashboard_score` are separate. `RankKey` includes
  evaluated, keep, oracle eligibility, convergence, submission, applied patch,
  fewer aborts, fewer repair loops, fewer failed calls, and tool activity.
  `dashboard_score` only includes a subset of those factors.
- Probes that display both fields are correct; probes that compute deltas from
  score should label those as heuristic deltas, not rank explanations.

Implementation feasibility:

- The patch is feasible in `metrics.rs`. The existing code already has `Row`,
  `Cohort`, `Key`, `Choice`, `SourceRef`, and tests near the metrics code.
- The safest implementation path is to add a small local projection over rows
  or cohorts, for example a `Trajectory` carrier with cohort keys, decisions,
  ambiguities, and diagnostics.
- Avoid overlong helper names. Module context already says this is
  `prototype1_state::metrics`; local methods can be short.

## Probe 1

Verdict: sound recommendation with two wording/shape risks.

What is sound:

- Correctly identifies the current sharp risk: selected-by-generation can hide
  multiple selected rows.
- Correctly keeps the patch read-only and out of History/Crown authority.
- Correctly requires JSON ambiguity as structured data rather than display-only
  text.
- Correctly states that missing lineage id must remain explicit.

Counter-model or risky implied invariants:

- `trajectory_status` values like `single_lineage` can imply the system proved a
  lineage. It has not. Use a status that names the evidence condition, not the
  ontology, such as `unambiguous`, `ambiguous`, `incomplete`, plus a separate
  `coordinate: degraded_parent_generation`.
- "Emit a diagnostic when a group has zero selected successors" is too broad if
  applied to every cohort. In the later multi-parent model, a cohort may be
  exploratory or incomplete without violating authority. This should be scoped
  to the requested trajectory view or candidate chain.

Residual risks:

- If implemented from `Generation`, it will preserve the existing collapse. It
  must group selected `Row`s before summarizing.

## Probe 2

Verdict: strongest and most complete plan, with minor API-scope risk.

What is sound:

- Correctly names the useful question: what selected path can the operator
  inspect, and where does evidence stop supporting a single path?
- Correctly separates chain, ambiguities, and diagnostics.
- Correctly includes source refs, runtime refs, status/disposition, operational
  counts, rank, and score for triage.
- Correctly warns against broad names and authority-sounding records.

Counter-model or risky implied invariants:

- "Lineage projection over observed campaign evidence" is acceptable only if
  the implementation keeps saying lineage is unavailable. Without lineage id,
  this is not a lineage proof.
- Returning `chain: []` on ambiguity is safe, but returning a partial chain
  needs precise semantics. A partial chain should not imply the omitted branch
  is invalid or less authoritative.

Implementation feasibility notes:

- The current JSON `Slice` is broad. Adding focused `chain` and `ambiguities`
  can be done inside the existing `Slice`, or by changing the shape for
  `--view trajectory`. If changing the shape, update tests and treat it as a CLI
  output contract change.

## Probe 3

Verdict: sound and concise, but under-specifies the output carrier.

What is sound:

- Correctly keeps the patch inside metrics.
- Correctly identifies branch ambiguity, missing parent coordinates, and
  generation-only collapse as the real hazards.
- Correctly leaves score/rank alignment as a follow-up.

Counter-model or risky implied invariants:

- "Selected rows with missing parent coordinates" should be diagnosed as
  degraded continuity, not automatically as ambiguous selection. Missing parent
  identity means the chain cannot prove continuity; it does not mean there are
  multiple selections.
- "Selected row exists but lacks stable lineage id" applies to all current
  rows. If treated as a per-row ambiguity, the output will become noisy. Keep
  this as one projection-level diagnostic.

Residual risks:

- The plan does not explicitly say to stop using generation summaries as the
  source of truth. That must be added before implementation.

## Probe 4

Verdict: sound plan with the best structural framing.

What is sound:

- Correctly decomposes the structure into cohort coordinate, generation rows,
  selected rows, and diagnostics.
- Correctly calls out flattened names such as
  `HistoryMetricsTrajectoryAmbiguity`.
- Correctly keeps dashboard score/rank wording constrained.

Counter-model or risky implied invariants:

- "Selected parent-successor chain inside a cohort" is slightly off. A cohort
  is a parent/generation candidate set. A chain crosses cohorts by following
  selected child to next parent coordinate. The implementation should model
  cohort decisions first, then derive chain continuity where parent ids support
  it.
- "Zero selected rows" should be a state of a cohort decision, not an invariant
  violation. The broader model allows incomplete or unselected candidate sets.

Implementation feasibility notes:

- This probe correctly suggests bundling only the trajectory-specific slice if
  small. That should be decided after seeing how invasive the current `Slice`
  shape is. A small additive field is less risky than replacing the JSON shape.

## Probe 5

Verdict: sound, with the clearest finite state suggestion for selection count.

What is sound:

- The `NoSelection`, `OneSelection`, `MultipleSelections` shape is a good local
  enum for the projection.
- Correctly says the patch should not add a database backend, replace records,
  or touch History authority.
- Correctly emphasizes candidate artifact/runtime refs and operational deltas
  already present in rows.

Counter-model or risky implied invariants:

- "Parent runtime/source coordinate when available" is correct as an aspiration,
  but current rows mostly have `parent_node_id`, generation, runtime id, branch
  id, and refs. Do not invent parent runtime/source coordinates from branch
  names or worktree paths.
- "Selected row exists but lacks stable lineage id" should not be classified as
  a selection-state failure. It is a projection-level degradation.
- "Focused JSON output rather than broad all-metrics payload" is useful but may
  exceed the smallest correct patch if it requires reshaping existing consumers.

Residual risks:

- The plan should require tests for multiple selected rows in one
  `(parent_node_id, generation)` and for two distinct parent cohorts in the same
  generation.

## Required Corrections For The Patch

The next implementation should satisfy these review constraints:

1. Build trajectory from selected `Row`s grouped by cohort key, not from
   `Generation::selected_node_id`.
2. Represent the degraded coordinate explicitly:
   `(lineage: None, parent_node_id, generation)`.
3. Add a local selection-count state such as no selection, one selection, and
   multiple selections.
4. Emit ambiguity diagnostics as structured JSON, not only strings.
5. Keep one projection-level diagnostic for missing lineage id.
6. Preserve selection source authority as `transition_journal` vs
   `mutable_projection`.
7. Display `dashboard_rank` and `dashboard_score` as separate heuristics.
8. Avoid new flattened authority names and avoid new History/Crown records.
9. Add tests for:
   - one selected row in one cohort;
   - multiple selected rows in one cohort;
   - selected rows in multiple parent cohorts for the same generation;
   - missing `parent_node_id` continuity degradation;
   - JSON ambiguity structure.

## Final Recommendation

Proceed with the shared probe recommendation, using Probe 2 or Probe 4 as the
base plan and Probe 5's local selection-state enum. Apply the corrections above
before coding. The work should stay in `metrics.rs` and nearby tests unless the
CLI output shape forces a narrow parser/display update.
