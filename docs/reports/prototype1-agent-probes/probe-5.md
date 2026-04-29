# Prototype 1 Metrics Next-Patch Plan

Context read: `AGENTS.md` and
`docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md`
only.

## Recommendation

Next patch: make `history metrics --view trajectory` explicitly
cohort-aware and branch-aware instead of silently presenting a single selected
successor per generation.

This is the highest-value metrics patch because the brief identifies the
current trajectory assumption as only valid for the present single-lineage run
shape. That assumption directly affects operator interpretation: a trajectory
view that collapses multiple selected rows can make a branching campaign look
cleaner, more linear, or more decided than the evidence proves.

## Latent Object

The latent object is not a generic metrics report. It is a read-only projection
of parent-to-child candidate selection within a campaign, scoped by cohort:

- parent runtime/source coordinate when available
- generation
- candidate artifact/runtime refs
- selected/disposition status
- key operational deltas already present in node rows
- diagnostics when the projection cannot prove a single successor

The patch should model this as a trajectory projection over cohorts, not as a
new authority layer and not as History admission.

## Reduction To Avoid

Do not invent a broad `HistoryMetrics`, `TrajectoryAnalysisReport`, or similar
flattened object that pretends the projection is a durable History fact.

Avoid making the CLI output look authoritative by choosing one selected row
when multiple rows match. The useful behavior is to expose ambiguity:

- zero selected rows for a cohort/generation
- exactly one selected row
- multiple selected rows
- selected row exists but lacks stable lineage id

## Files I Expect To Touch

- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`
- the nearby CLI display code for `history metrics --view trajectory`, if the
  table/JSON rendering is separated from metric derivation
- focused tests for trajectory/cohort projection behavior, wherever existing
  Prototype 1 metrics tests live

I would avoid touching History authority code in this patch.

## Patch Shape

1. Add an internal trajectory projection grouped by `(parent_node_id,
   generation)` until real lineage ids exist.
2. For each cohort/generation, compute a compact state:
   `NoSelection`, `OneSelection`, or `MultipleSelections`.
3. Include enough row data to be operationally useful:
   generation, parent node id, selected candidate id/ref, status,
   disposition, branch/runtime refs, source refs, dashboard rank/score, patch
   apply state, submission state, abort count, and repair-loop count.
4. Emit diagnostics when grouping is degraded because lineage id is unavailable.
5. Change table output so ambiguous generations are visible, not hidden behind
   a single row.
6. Change JSON output so `--view trajectory` returns a focused trajectory
   payload rather than a broad all-metrics payload, if the current implementation
   still returns too much surrounding data.

## Invariants This Patch Must Preserve

- Metrics remain disposable projections over current evidence.
- No output should claim sealed History, Crown authority, or Parent policy.
- Compatibility aliases may keep working, but `history metrics` remains the
  primary surface.
- New names should preserve structure through modules/types/enums instead of
  flattened helper names.
- Existing node rows and cohort summaries should continue to work unless their
  output must be narrowed for the requested view.

## Known Gaps This Patch Will Not Solve

- It will not add live `Crown<Locked>` gating.
- It will not make current JSON records authoritative.
- It will not solve missing lineage ids; it will diagnose the degraded grouping.
- It will not align `dashboard_score` with `dashboard_rank`, except to avoid
  wording that implies one fully explains the other.
- It will not add a database backend or replace scheduler/journal files.

## Verification Commands

Use the narrowest available checks after inspecting the actual test layout:

```text
cargo test -p ploke-eval prototype1
cargo check -p ploke-eval --tests
```

Then run representative CLI checks against an existing campaign fixture or the
latest campaign fallback:

```text
ploke-eval history metrics --view trajectory
ploke-eval history metrics --view trajectory --generation 2 --rows 12
ploke-eval history metrics --view cohorts
```

The key manual verification is that trajectory output reports ambiguity instead
of collapsing it, and that any degraded lineage grouping is explicitly
diagnosed.
