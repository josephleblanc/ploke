# Shared Worker Brief

## Focus

Improve UI task readability without moving semantic authority out of
`ploke-tree::graph::Graph`.

## Reduction To Avoid

Do not solve a missing graph or loader fact by creating a UI-owned semantic
mirror, copied id set, copied record, or ad hoc report/status/info payload.

## Invariants

- Typed persisted data must stay behind named Rust record types.
- UI semantics must stay reference-based.
- Any semantic id, ref, record, partial record, or semantic wrapper allocated
  out of `ploke_tree::Graph` for later semantic use is a hard blocker.
- Mirror-type workarounds are also blockers. Renaming a copied semantic carrier
  to `Report`, `Status`, `Info`, `Summary`, or `Payload` does not make it
  derived.
- Layout, diagnostics, and render-only strings may be owned by the UI.
- Drilldown paths must line up with the typed answer-contract direction, even if
  the current slice is only readability.
- If a fact is missing, classify the blocker instead of guessing.

Allowed derived values are things like positions, bounds, overlap scores, summed
durations, average token counts, and render-only strings that are not used as
semantic join keys.

## No-Goals

- no broad redesign of future browser/gui surfaces in the first readability
  slice,
- no board mutations by workers,
- no unrelated cleanup outside the assigned lane,
- no raw JSON/value walking for owned persisted data,
- no long reports when a compact handoff will do.

## Verification Style

Use small verification commands and bounded test output, for example:

```bash
cargo check -p ploke-egui 2>&1 | tail -n 80
cargo test -p ploke-egui <filter> 2>&1 | tail -n 80
```

If code inspection is needed, report exact file paths and line ranges worth
reading locally.

## Required Final Report

- files changed,
- exact line ranges touched,
- commands run,
- blockers or deferred invariants,
- whether any semantic value was cloned or copied out of `Graph`,
- whether any mirror carrier or wrapper workaround was introduced,
- successor handoff note if the task remains open.
