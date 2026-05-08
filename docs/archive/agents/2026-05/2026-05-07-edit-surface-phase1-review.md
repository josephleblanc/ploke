# Phase 1 Edit Surface Review

Date: 2026-05-07

Reviewed:

- `AGENTS.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-implementation-plan.md`
- `docs/workflow/evalnomicon/drafts/2026-05-07-prototype1-edit-surface-model.md`
- `docs/archive/agents/2026-05/2026-05-07-prototype1-edit-surface-handoff.md`
- `crates/ploke-eval/src/cli/prototype1_state/edit_surface/`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`

## Findings

### Blocker: rule-derived bounds can use graph topology that is not part of the projected Artifact view

`graph::Projection` stores `edges` at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/graph.rs:145`, and `project` copies the mock edge set into the projection at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/graph.rs:336`. But `bounds` does not use `projection.edges`; ancestor and descendant expansion call `self.ancestors` / `self.descendants` at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/graph.rs:364` and `crates/ploke-eval/src/cli/prototype1_state/edit_surface/graph.rs:370`, and those helpers read `self.edges` at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/graph.rs:279` and `crates/ploke-eval/src/cli/prototype1_state/edit_surface/graph.rs:296`.

That means a projection made from one mock graph can have rule-derived bounds computed by another mock graph instance with a different edge relation, as long as the target spans exist in the projection. This weakens the invariant from the model that the code graph projection is tied to the Artifact it projects. In authority terms, the semantic expansion of a grant can be widened by adapter state that was not captured in the projection.

This should block Phase 1 until `bounds` derives ancestor/descendant closure from the supplied `Projection`, or the type/API makes it impossible to evaluate rules against a graph relation different from the projected one. Add a stale/cross-graph test that projects with one edge set and tries to derive broader descendant/ancestor bounds with another.

### Blocker: `ArtifactDelta` can be minted without consuming a surface check

`ArtifactDelta` is documented as patch-shaped transition evidence produced after a surface check at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/mod.rs:14`, but `ArtifactDelta::new` is `pub(crate)` at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/mod.rs:22`. Any code in `ploke-eval` can construct checked-looking delta evidence from arbitrary `surface::Ref` and `surface::Touch` values without going through `surface::Grant::check` or `Harness::apply_checked`.

`surface::Check` itself has private fields, which is good, but the final evidence object is still public to the whole crate. That makes the durable authority boundary depend on convention rather than the transition carrier. This conflicts with the Phase 1 goal that the delta-shaped result be produced by the checked transition.

This should block Phase 1 until delta construction is restricted to the edit-surface transition path, for example by making the constructor private to the `edit_surface` module and exposing only a constructor that consumes `surface::Check`, or by moving delta production behind the checked apply transition. Add a test or compile-time visibility pattern that keeps unchecked callers from manufacturing deltas.

### Medium: tests do not exercise checked-apply rejection

`Harness::apply_checked` rejects a proposal/check mismatch at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/harness.rs:116`, but the test suite only covers the valid path at `crates/ploke-eval/src/cli/prototype1_state/edit_surface/tests.rs:179`. Given that `apply_checked` is the harness-side boundary that consumes `surface::Check`, Phase 1 should include a negative test that a check for one proposal/base/after/touch set cannot be applied to another proposal.

This is not independently a blocker if the two authority issues above are fixed, but it is a useful invariant test for the intended boundary.

## Non-Issues

The module export in `crates/ploke-eval/src/cli/prototype1_state/mod.rs:765` is appropriately crate-private for this phase. The new file split also follows the planned structure: `graph`, `surface`, `harness`, and module tests.

Most state carriers have private fields and accessor methods rather than public mutable records. `surface::Check` in particular is not directly constructible outside the `surface` module, which is the right direction for a checked transition token.

The local names are generally structural and module-scoped: `graph::View`, `graph::Projection`, `surface::Grant`, `surface::Check`, and `harness::Harness` match the plan and avoid flattened Prototype 1 prefix names.

## Test Evidence

Commands run:

```text
cargo test -p ploke-eval edit_surface --lib 2>&1 | tail -n 20
cargo check -p ploke-eval 2>&1 | tail -n 80
cargo fmt --all -- --check 2>&1 | tail -n 40
```

Results:

- `cargo test -p ploke-eval edit_surface --lib`: passed, 7 tests.
- `cargo check -p ploke-eval`: passed with existing warnings.
- `cargo fmt --all -- --check`: passed with no output.

## Commit Recommendation

Do not commit Phase 1 as-is. The implementation is close and the shape is mostly right, but the current boundary still allows semantic bounds to be derived from graph state outside the projection and allows unchecked delta evidence to be constructed inside `ploke-eval`.
