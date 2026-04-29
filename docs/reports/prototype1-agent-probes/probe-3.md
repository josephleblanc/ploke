# Prototype 1 Metrics Next-Patch Probe 3

Read set:

- `AGENTS.md`
- `docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md`

Focus:

- Improve the usefulness of `ploke-eval history metrics` without turning metrics
  projections into History authority.

Recommended next patch:

- Make the trajectory view explicitly cohort-aware and branch-aware.

Latent object:

- The useful object is not a generic metrics dashboard. It is a projection over
  parent-to-successor choices inside a runtime/artifact lineage, with enough
  diagnostics to show when the current single-lineage assumption stops holding.

Reduction to avoid:

- Do not silently collapse multiple selected successors into one
  selected-by-generation row.
- Do not add another broad report abstraction.
- Do not present dashboard heuristics as Parent policy, oracle truth, or sealed
  History evidence.

Concrete behavior:

1. For `history metrics --view trajectory`, group selected rows by the current
   available lineage surrogate: `(parent_node_id, generation)`.
2. Emit a focused trajectory payload/table that includes, per step:
   `generation`, `parent_node_id`, selected child/node id, disposition/status,
   branch/runtime refs, source refs, tool call totals/failures, patch attempts,
   patch apply/submission state, abort/repair-loop counts, rank, and score.
3. Detect ambiguity instead of hiding it:
   - more than one selected row for the same `(parent_node_id, generation)`
   - selected rows with missing parent coordinates
   - generation-only trajectory collapse that would discard branch structure
4. Surface those cases as diagnostics in both table and JSON output.
5. Keep the existing degraded-lineage diagnostic because current records lack a
   true lineage id.

Files I expect the patch to touch:

- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`
- the CLI tests or snapshot tests nearest the current metrics command, if they
  already exist

Invariants this patch must preserve:

- Metrics remain projections over current evidence, not sealed History entries.
- `history metrics` stays the primary command surface; nested monitor commands
  remain compatibility aliases only.
- The patch should preserve the claim boundary: useful projections and a
  History-shaped preview exist, but live Crown-gated sealed History does not.
- Names should carry structure through local types/modules rather than long
  helper names such as `prototype1_monitor_trajectory_*`.

Known gaps this patch will not solve:

- It will not add true lineage ids to records.
- It will not implement `Crown<Locked>`, `Parent<Ruling>`, or live block
  sealing.
- It will not decide whether `dashboard_score` should be aligned with
  `dashboard_rank`; that is the next useful metrics patch after trajectory
  ambiguity is visible.

Verification commands:

```text
cargo check -p ploke-eval
cargo test -p ploke-eval
ploke-eval history metrics --view trajectory
ploke-eval history metrics --view trajectory --format json
ploke-eval history metrics --view cohorts
```

Task stack after this probe:

- Focus: patch trajectory metrics so branch/cohort ambiguity is visible.
- Next: inspect `metrics.rs` and the nearest CLI tests, then implement focused
  trajectory diagnostics.
- Later: align or relabel `dashboard_score` and `dashboard_rank` once
  trajectory output no longer hides branch structure.
