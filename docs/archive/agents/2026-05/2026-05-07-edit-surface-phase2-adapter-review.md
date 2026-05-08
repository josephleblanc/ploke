# Phase 2 Edit Surface Adapter Review

Date: 2026-05-07

Task title: Review Phase 2 first-slice bounded edit-surface adapter

Task description: Review current uncommitted changes under
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/`, especially the new
`tui` adapter boundary, for authority, Artifact binding, digest evidence,
all-or-rejected apply evidence, naming/structure, `ArtifactDelta::from_check`
visibility, and commit blockers.

Related planning files:

- `docs/active/agents/2026-05-07-prototype1-edit-surface-implementation-plan.md`
- `docs/active/agents/2026-05-07-prototype1-edit-surface-model.md`
- `docs/active/agents/2026-05-07-prototype1-edit-surface-handoff.md`
- `docs/active/agents/2026-05-07-edit-surface-phase1-review.md`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs`

## Summary

The current slice is a reasonable `ploke-eval`-owned adapter scaffold. It does
not make `ploke-tui` proposal status or `ploke-db` resolved material spans into
authority by itself. The important Phase 1 visibility issue is improved:
`ArtifactDelta` can no longer be minted crate-wide from arbitrary surface data;
`ArtifactDelta::from_check` now consumes a `surface::Check` and is visible only
inside the `edit_surface` module family.

Do not treat this as a complete concrete Phase 2 adapter yet. The live
projection hash is still caller-supplied, rule/source evidence is descriptive
rather than reproducible authority, and `tui::Apply::from_results` can mint an
`Applied` delta from executor-reported write success without validating the
post-apply Artifact/tree state. Those are blockers before this path is admitted
to History or wired into live candidate creation.

## Findings

### Blocker Before Live History Admission: applied evidence trusts executor-reported write success

`tui::Apply::from_results` verifies that the consumed `surface::Check` matches
the staged proposal at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:519`.
It then rejects partial counts and rejected write results at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:530` and
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:542`.

That enforces an all-applied-or-rejected shape, but it does not prove the
material post-state. `Write::applied` is an executor-side report at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:480`, and
`Apply::from_results` mints `ArtifactDelta` at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:558` without
checking that the `after` Artifact actually exists, has the expected tree hash,
or contains the reported per-file after hashes.

If this result were admitted directly, executor output would become authority
over "applied" evidence. Before live wiring or History admission, the applied
path needs a ploke-eval-owned post-apply validation step: read/identify the
derived Artifact, verify its id/hash, verify touched material hashes, and only
then produce/admit `ArtifactDelta`. Otherwise rename this result as provisional
executor evidence and keep `ArtifactDelta` behind the post-Artifact validation.

### Blocker Before Claiming Concrete Projection Binding: projection hash is carried, not derived

`tui::Projection::new` accepts an arbitrary projection id, hash, and
`surface::Ref` at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:99`.
`Bounds::new` does check that the adapter projection Artifact matches the
`graph::Bounds` Artifact at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:145`,
and `Bounds::touch` checks the same binding against the material Artifact at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:183`.

That is a real Artifact id/hash binding for bounds and touch validation. It is
not yet a real binding between the projection hash and a `ploke-db` / code-graph
snapshot. The hash is not computed from canonical projection contents, DB index
metadata, parser input, or graph spans. The digest records the supplied
projection hash at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:209`,
but it cannot prove that the projection was derived from the Artifact it names.

For a first-slice scaffold this is acceptable if documented narrowly. For the
Phase 2 "project target Artifact into graph/database projection" claim, it is a
blocker. The adapter should eventually construct `Projection` through a
projector that computes the projection hash from a canonical projection/DB
snapshot and ties it to the Artifact id/hash in one move.

### Medium: rule and bounds digests are useful but not reproducible authority

`tui::Rule::named` and `tui::Rule::inline` hash the source tag and label at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:70`.
`Bounds::compute_digest` includes projection id/hash, Artifact id/hash,
source tag/label, rule digests, and bounded spans at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:202`.

This is meaningful enough to make test evidence and persisted scaffold records
less ambiguous. It is not yet enough to replay or audit a rule-derived surface:
`Source::Named` is only a name, `Source::Derived` is free text, and no rule
engine version, named-rule definition, query text, or canonical DB snapshot is
included. Before this becomes durable History evidence, named rules need a
stable resolved definition or versioned rule ref, and derived bounds should
record enough input to reproduce the digest.

### Medium: auto-apply is rejected only if the caller carries the flag correctly

The new adapter rejects `Stage { auto_apply: true }` at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:440`, and the
test at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:544`
covers that path. `LowerProposal` records TUI proposal status as telemetry at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:408`; that
status is not used as authority.

The remaining risk is in future live wiring. The existing TUI path can
auto-confirm edits from `editing_cfg.auto_confirm_edits` at
`crates/ploke-tui/src/rag/tools.rs:316`. The current adapter has no live
conversion that proves it extracted or disabled that configuration before
staging. The next slice should make the adapter own this boundary: either run
the TUI staging path with auto-confirm disabled, or derive `auto_apply` from the
actual TUI config/result and reject before any apply task can spawn.

### Non-Issue: `ArtifactDelta::from_check` visibility is now appropriately narrow

`ArtifactDelta::from_check` changed to `pub(super)` at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:32`.
That is visible to the `edit_surface` module family, including `tui`, but it
still requires a `surface::Check`. `surface::Check` has private fields at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs:180`, and
the only current call sites are the checked harness apply path and the new TUI
apply evidence path.

This is a better boundary than the Phase 1 reviewed shape. It prevents arbitrary
crate-wide delta construction while allowing adapter code to project a checked
transition into delta evidence.

### Non-Issue: naming and structure mostly preserve the model

The module keeps short local names under the `edit_surface::tui` boundary:
`Projection`, `Rule`, `Bounds`, `MaterialSpan`, `Proposal`, `Apply`, and
`Write`. This matches the plan better than flattened names carrying
`Prototype1Tui...` prefixes. `LowerRequest`, `LowerEdit`, and `LowerProposal`
are slightly generic, but they are local wrappers for imported TUI shapes rather
than new authority objects.

## Focus Checklist

- ploke-tui/ploke-db authority: not currently authority. TUI proposal status is
  telemetry; material spans become `surface::Touch` only after Artifact hash and
  bounds checks. Future live auto-confirm handling remains a required boundary.
- Projection binding: real for Artifact id/hash equality at bounds/touch time;
  not yet real for projection hash derivation from a DB/code-graph snapshot.
- Rule/bounds digest/source: meaningful scaffold evidence; not yet
  reproducible or authority-grade.
- All-or-rejected evidence: count/span/rejection shape is enforced; actual
  derived Artifact state is not verified before minting `ArtifactDelta`.
- Naming/structure: acceptable for the slice.
- `ArtifactDelta::from_check`: acceptable visibility after the change.

## Test Evidence

Commands run:

```text
cargo test -p ploke-eval edit_surface --lib 2>&1 | tail -n 20
cargo check -p ploke-eval 2>&1 | tail -n 80
cargo fmt --all -- --check 2>&1 | tail -n 40
git diff --check -- crates/ploke-eval/src/cli/prototype1_state/edit_surface
```

Results:

- `cargo test -p ploke-eval edit_surface --lib`: passed, 18 tests.
- `cargo check -p ploke-eval`: passed with existing warnings.
- `cargo fmt --all -- --check`: passed with no output.
- `git diff --check`: passed with no output.

## Commit Recommendation

Commit only if the commit is explicitly scoped as a first-slice adapter scaffold
and does not claim live bounded candidate generation, History-ready apply
evidence, or concrete DB projection hashing.

Do not commit it as the complete Phase 2 adapter. Before that, add
ploke-eval-owned post-apply Artifact validation and make projection/rule source
digests reproducible from concrete graph/database inputs.
