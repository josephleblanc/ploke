# Phase 2 Edit Surface Authority Review

Date: 2026-05-07

Task title: Review Phase 2 second-slice bounded edit-surface authority fixes

Task description: Review current uncommitted changes under
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/` for closure of the
previous Phase 2 adapter review blockers: `Apply::Reported` vs `Applied`,
post-Artifact validation before `ArtifactDelta`, per-touch after-hash checks,
canonical projection digest inputs, caller-supplied projection hashes, and
naming/visibility.

Related planning files:

- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md`
- `docs/workflow/evalnomicon/drafts/2026-05-07-prototype1-edit-surface-model.md`
- `docs/archive/agents/2026-05/2026-05-07-edit-surface-phase2-adapter-review.md`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/graph.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs`

## Findings

### Blocker: `Apply::Reported` can still bypass its constructor assumptions

The ordinary path is much better: `tui::Apply::from_results` now returns
`Apply::Reported` after all executor writes report success at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:647`, and
`ArtifactDelta::from_check` is reached only by `Apply::validate` at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:699` and
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:737`.

The authority barrier is not sealed, though. `tui::Apply` is a `pub(crate)`
enum with crate-visible variants at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:622`. A
crate-local caller can construct `Apply::Reported` directly with arbitrary
`touches` and `writes` while carrying a real `surface::Check`. `validate` then
iterates over the supplied `touches.iter().zip(writes.iter())` at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:720`; it does
not re-check the reported vectors against the consumed `surface::Check`, nor
does it independently enforce length equality. A forged `Reported` with empty
reported touches/writes can skip all per-touch after-hash checks and still mint
`Apply::Applied` from the checked delta.

This means the previous "executor-reported applied evidence becomes authority"
blocker is closed only for callers that use `from_results`. It is not closed as
a type/visibility boundary. Before this is committed as an authority fix, hide
the variants behind a private representation or make `validate` derive and
check the authoritative touch set from `surface::Check` instead of trusting the
reported vectors.

### Blocker Before Live History Admission: `after_artifact` is still a caller-supplied carrier

`Apply::validate` now checks that the supplied after Artifact reference matches
the proposal's `after` ref at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:713`, and it
checks each reported touched path's after hash against
`surface::Artifact::file_hash` at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:722`. That
closes the prior review issue at the adapter-state level.

It does not yet close the concrete adapter/live History issue. The
`surface::Artifact` value passed into `validate` can still be constructed from
arbitrary refs and hashes through
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/surface.rs:51`. There
is no current code in this slice that reads the derived checkout/tree, computes
the Artifact identity, computes touched file hashes from material contents, and
then calls `validate` with that backend-owned evidence.

For a bounded scaffold this is acceptable if the commit message stays narrow.
For Phase 2 as a concrete `ploke-tui` / `ploke-db` adapter, it remains a blocker
before History admission or live candidate creation.

### Medium: projection hashes are computed, but rule/source provenance is still caller-supplied

The previous arbitrary projection-hash blocker is mostly closed. The old
`Projection::new(id, hash, artifact)` constructor is gone, and
`Projector::project` computes the projection hash from the projector id,
source, rule digests, graph Artifact id/hash, graph spans, and graph edges at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:218` and
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:230`. The added
graph iterators are backed by ordered maps/sets at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/graph.rs:157`, so the
mock projection digest is deterministic for the current canonical graph shape.

The remaining gap is rule/source authority. `Projector::new` accepts arbitrary
`Source` and `Rule` values at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:206`, and
`Bounds::new` accepts an already-derived `graph::Bounds` at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tui.rs:272`. It copies
the projection's rule/source metadata into the bounds digest, but it does not
prove those `tui::Rule` definitions were the inputs used to derive that
`graph::Bounds`. The digest records final bounded spans, so it is useful
evidence; it is not yet a replayable proof that named/derived rules produced
the bounds.

This is not a regression, and it is stronger than the previous descriptive-only
shape. It should still be documented as scaffold-grade until the adapter has a
single projection/bounds constructor that evaluates versioned rules against a
canonical graph/database snapshot.

### Non-Issue: `Apply::Reported` vs `Applied` is structurally improved on the intended path

The tests now assert that all reported writes produce `Apply::Reported`, not
`Apply::Applied`, at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:707`. Separate
tests cover wrong after Artifact identity, wrong touched after hash, and valid
post-Artifact validation at
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:721`,
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:783`, and
`crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:853`.

Those are the right tests for the ordinary path. They need one more negative
test for direct `Apply::Reported` construction or the visibility should be
changed so such a test is impossible outside the module.

### Non-Issue: naming mostly preserves the local structure

The new names remain local to `edit_surface::tui`: `Def`, `Source`, `Rule`,
`Projector`, `Projection`, `Bounds`, `Apply::Reported`, and `Apply::Applied`.
They do not introduce flattened Prototype 1 role/state names. `Def` is a little
generic, but it is a small rule/source definition carrier inside the adapter
module, not a protocol role name.

The naming issue is visibility, not vocabulary: authority-bearing transition
constructors need private state, not just better names.

## Closure Checklist

- `Apply::Reported` vs `Applied`: partially closed. The intended path reports
  executor success first and validates later, but crate-visible variants still
  let callers bypass constructor invariants.
- `validate(after_artifact)` before `ArtifactDelta`: closed for the intended
  path. Not closed for live authority until `after_artifact` is backend-owned
  evidence rather than a caller-built carrier.
- Per-touch after hash checks: present in `validate`; not sealed because direct
  `Reported` construction can supply a different or shorter touch/write list.
- Projection digest from canonical graph/rule/source inputs: improved. The hash
  is derived from ordered graph spans/edges plus source/rule metadata, but the
  rule/source metadata is still not proven to be the actual derivation input for
  `graph::Bounds`.
- Arbitrary caller-supplied projection hash: closed in this module; no current
  crate-visible `tui::Projection` constructor accepts a hash.
- Naming/visibility: names are acceptable; `Apply` variant visibility is the
  main authority blocker.

## Test Evidence

Commands run:

```text
cargo test -p ploke-eval edit_surface --lib 2>&1 | tail -n 20
cargo check -p ploke-eval 2>&1 | tail -n 80
cargo fmt --all -- --check 2>&1 | tail -n 40
git diff --check -- crates/ploke-eval/src/cli/prototype1_state/edit_surface docs/active/agents
```

Results:

- `cargo test -p ploke-eval edit_surface --lib`: passed, 23 tests.
- `cargo check -p ploke-eval`: passed with existing warnings.
- `cargo fmt --all -- --check`: passed with no output.
- `git diff --check`: passed with no output.

## Commit Recommendation

Do not commit this as "previous blockers closed" until the `Apply` visibility
hole is fixed or explicitly documented as still open. A narrow scaffold commit
is defensible only if it says the intended path is improved but live
History-ready authority remains blocked on sealed `Apply` construction and
backend-owned after-Artifact validation.

## Follow-Up Patch

After this review, the `Apply` visibility blocker was patched in the main
thread:

- `tui::Apply` is now an opaque struct with a private `State` enum.
- `Reported`, `Applied`, and `Rejected` states are no longer constructible
  crate-wide.
- tests use state predicates instead of matching public variants.

Validation after the patch:

```text
cargo fmt --all
cargo test -p ploke-eval edit_surface --lib 2>&1 | tail -n 20
cargo check -p ploke-eval 2>&1 | tail -n 80
```

Results:

- `cargo test -p ploke-eval edit_surface --lib`: passed, 23 tests.
- `cargo check -p ploke-eval`: passed with existing warnings.

Remaining limitation before live History admission: `validate(after_artifact)`
still needs to receive backend-owned Artifact evidence in the live wiring path,
not a caller-constructed `surface::Artifact`.
