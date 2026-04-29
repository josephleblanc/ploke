# Prototype 1 Metrics Probe 1

Recommended next patch: make the trajectory metrics view explicitly
cohort-aware and ambiguity-preserving.

## Why This Patch

The current metrics surface is useful, but the brief identifies one sharp
correctness risk: trajectory assumes one selected successor per generation.
That matches the current single-lineage run shape, but it becomes misleading
as soon as the evidence contains multiple selected rows in one generation or
multiple parent/generation cohorts.

This is the best next patch because it improves operator usefulness without
claiming sealed History authority. It also preserves the key boundary from the
brief: metrics are disposable projections over current evidence, not Crown
admitted History.

## Latent Object

The latent object is not a generic `HistoryMetrics` layer. It is a projection
over parent-successor selection chains, grouped by the evidence coordinates
currently available: `parent_node_id` and `generation`.

Since current records lack a lineage id, the projection should say exactly
when it is showing a single unambiguous chain and when it is showing a cohort
with branch ambiguity.

## Reduction To Avoid

Do not hide branch structure behind one row per generation.

Avoid:

- silently choosing one selected node when more than one selected node exists
- explaining trajectory rank as if `dashboard_score` fully determines it
- adding broad names like `HistoryMetrics`, `TrajectoryAnalysisReport`, or
  new authority-sounding records
- turning metrics output into History admission or Crown evidence

## Files I Expect To Touch

- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`
- the CLI display code that formats `history metrics --view trajectory`, if it
  is not already contained in `metrics.rs`
- focused tests or snapshots for the metrics projection, wherever current
  Prototype 1 CLI/metrics tests already live

Do not touch History/Crown authority code for this patch unless compilation
requires a small type adjustment.

## Concrete Behavior

For `ploke-eval history metrics --view trajectory`:

1. Group selected rows by `(parent_node_id, generation)`.
2. Emit a trajectory row only when a group has exactly one selected successor.
3. Emit a diagnostic when a group has zero selected successors.
4. Emit a diagnostic when a group has more than one selected successor, listing
   the parent/generation coordinate and the candidate node ids.
5. Add a top-level summary field such as `trajectory_status` with values like
   `single_lineage`, `branching`, or `incomplete`.
6. In table output, show diagnostics compactly after the trajectory rows.
7. In JSON output, include the grouped diagnostics in a stable structured
   field so agents can consume them without parsing display text.

For `--view cohorts`:

1. Keep cohort rows focused on parent/generation groups.
2. Include selected-count and candidate-count fields if they are not already
   present.
3. Use the same ambiguity detection as trajectory, so the two views agree.

## Invariants This Patch Must Preserve

- Metrics remain read-only projections.
- No output should imply live Crown-gated sealed History.
- Missing lineage id remains explicit; grouping by `(parent_node_id,
  generation)` is a degraded coordinate, not a permanent ontology.
- Names should preserve structure through modules and data carriers rather than
  long flattened helper names.
- Existing compatibility commands may keep working, but the primary UX remains
  `history metrics`.

## Known Gaps This Patch Will Not Solve

- It will not introduce a real lineage id.
- It will not implement `Crown<Locked>` or `Parent<Ruling>`.
- It will not make current JSON records authoritative.
- It will not decide historical Parent policy.
- It will not fully resolve the `dashboard_rank` versus `dashboard_score`
  relationship, except to avoid worsening the implication.

## Verification Commands

```text
cargo check -p ploke-eval
cargo test -p ploke-eval prototype1
ploke-eval history metrics --view trajectory
ploke-eval history metrics --view cohorts
```

If the exact test filter differs, use the narrowest existing metrics or
Prototype 1 CLI tests after locating them during implementation.

## Acceptance Criteria

- A run with one selected successor per parent/generation still produces the
  current useful trajectory, with an explicit single-lineage status.
- A run with multiple selected successors in the same parent/generation no
  longer collapses to one row without warning.
- JSON output exposes ambiguity as data, not only display text.
- Table output remains compact enough for operator use.
- Documentation or inline help does not overclaim beyond projection usefulness.
