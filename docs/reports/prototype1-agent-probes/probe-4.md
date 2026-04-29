# Probe 4: Prototype 1 Metrics Next Patch Plan

Source context read:

- `AGENTS.md`
- `docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md`

## Recommended Next Patch

Make `history metrics --view trajectory` explicitly cohort-aware and ambiguity-aware.

The current trajectory projection is useful, but the brief says it assumes one selected successor per generation. That assumption matches the current single-lineage run shape, but it is not a safe general metric surface. The next patch should keep trajectory as a projection, not authority, and make it tell the operator when the evidence no longer fits the single-lineage shape.

## Latent Object

The latent object is a selected parent-successor chain inside a cohort, where the current available coordinate is degraded to `(parent_node_id, generation)` because lineage id is absent from current records.

This should not become a new authority layer. It is a metrics projection over transitional evidence.

## Reduction To Avoid

Avoid flattening the issue into names like `HistoryMetricsTrajectoryAmbiguity`, `SelectedSuccessorTrace`, or `ChildProgress`. The useful structure is:

- a cohort coordinate
- generation rows inside that cohort
- selected rows within a generation
- diagnostics when selection is missing or plural

The command can still be `history metrics --view trajectory`; the code should carry meaning through local types/modules rather than long helper names.

## Files I Expect To Touch

- `crates/ploke-eval/src/cli/prototype1_state/metrics.rs`
- possibly the CLI display code that formats `history metrics` output, if table/JSON slicing lives outside `metrics.rs`
- focused tests near the existing metrics tests, if present in or adjacent to that module

Do not broaden into History/Crown mutation in this patch.

## Concrete Patch Shape

1. Add an internal trajectory projection that groups selected candidates by cohort coordinate before deriving a chain.
2. Preserve the existing single-lineage output when exactly one selected row exists for each generation in the chosen cohort.
3. Emit diagnostics when a generation has zero selected rows, multiple selected rows, or selected rows appear in more than one cohort for the requested trajectory view.
4. For table output, make ambiguity visible in a compact column or footer instead of silently choosing one row.
5. For JSON output, include the focused trajectory payload plus diagnostics, so downstream probes do not have to infer ambiguity from dropped rows.
6. Keep diagnostic wording explicit that this is degraded by missing lineage id and grouped by `(parent_node_id, generation)`.

## Invariants This Patch Must Preserve

- Metrics remain read-only projections over current evidence.
- No CLI text should imply sealed History authority, Crown-gated mutation, or compiler-enforced transition validity.
- Existing compatibility aliases may keep working, but new docs and primary UX should use `history metrics`.
- Parent/child/Crown concepts stay structurally named; no new flattened event ontology should be introduced.
- `dashboard_score` should not be presented as explaining `dashboard_rank` unless that alignment is implemented in the same patch.

## Known Gaps This Patch Will Not Solve

- It will not add lineage id to source records.
- It will not make current JSON records authoritative History.
- It will not implement live `Crown<Locked>`, `Parent<Ruling>`, or `Successor<Admitted>`.
- It will not decide the historical Parent policy; it only makes the current projection harder to misread.

## Verification Commands

Use the narrowest checks that exercise the projection:

```text
cargo check -p ploke-eval
cargo test -p ploke-eval prototype1
ploke-eval history metrics --view trajectory
ploke-eval history metrics --view cohorts
```

If the repo does not expose a focused `prototype1` test filter, run the nearest existing metrics/prototype1 filter rather than expanding immediately to the full workspace.

## Task Stack

Focus:

- Patch `history metrics --view trajectory` so it exposes cohort and selection ambiguity instead of silently collapsing branch structure.

Blocked By:

- None known from the two-file brief.

Discovered:

- `dashboard_rank` and `dashboard_score` can mislead operators if displayed as equivalent. Keep this as a separate follow-up unless the trajectory patch naturally touches the same display code.
- `--view cohorts` and `--view trajectory` need more focused table/JSON payloads. Bundle only the trajectory-specific slice if it is small; otherwise leave general view slicing as the next patch.

Next:

- Read `crates/ploke-eval/src/cli/prototype1_state/mod.rs` and `metrics.rs`, then patch the trajectory projection and its display/tests.
