# Phase 3 Edit Surface Integration Slice

Date: 2026-05-07

Task title: First bounded edit-surface candidate-generation integration slice

Task description: Add explicit Prototype 1 candidate-generator and edit-surface
selection knobs, route parent child-plan creation through the selected
generator, and fail closed for the `tui-edit-surface` path until backend-owned
after-Artifact validation can be provided.

Related planning files:

- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-model.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-handoff.md`
- `docs/archive/agents/2026-05/2026-05-07-edit-surface-phase2-authority-review.md`
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/`

## Slice Result

The CLI now exposes:

```text
--candidate-generator legacy
--candidate-generator tui-edit-surface
--edit-surface ploke-tui-tools
```

`legacy` remains the default and preserves the existing target-selection path.
When `tui-edit-surface` is selected, parent candidate generation dispatches to
the edit-surface path and fails before publishing `ChildPlan`.

The failure is intentional. Current checked adapter carriers can represent:

```text
SurfaceGrant -> surface::Check -> ArtifactDelta
```

but live candidate creation still lacks backend-owned derived Artifact evidence
after the proposed edit is applied. Accepting staged TUI proposal state as a
successful edit would weaken authority, so the path does not fall back to
legacy and does not reuse existing child plans that lack edit-surface evidence.

## Exact Backend Blocker

The next backend slice must provide a live operation that:

1. materializes a proposed edit into a derived Artifact checkout;
2. computes the derived Artifact identity from that checkout;
3. computes touched file hashes from the derived checkout contents;
4. constructs the post-apply `surface::Artifact` from backend-owned evidence;
5. passes that Artifact into `edit_surface::tui::Apply::validate`;
6. converts the resulting checked `ArtifactDelta` into current
   `ChildFiles` / `ResolvedTreatmentBranch` compatible single-file candidates
   inside the `ploke-tui-tools` surface.

Until those steps exist, `candidate-generator=tui-edit-surface` is not live
long-run ready.
