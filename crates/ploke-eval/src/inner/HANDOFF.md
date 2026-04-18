# inner Handoff

- date: 2026-04-18
- branch: `refactor/tool-calls`
- anchor commit: `b770120c`
- current scope: slice 1 only, `RunIntent -> FrozenRunSpec -> RunRegistration`

## Read First

1. `docs/active/agents/2026-04-18_ploke-eval-procedure-model.md`
2. `crates/ploke-eval/src/inner/core.rs`
3. `crates/ploke-eval/src/inner/registry.rs`

## What Exists

- `RunStorageRoots`, `RunIntent`, and `FrozenRunSpec` in `inner/core.rs`
- `RunRegistration` and `RunRegistrationError` in `inner/registry.rs`
- task-readable stable run ids
- focused tests for freeze semantics, artifact-path derivation, registration round-trip, and path-provenance errors

## Verify From Cold Start

Run this first:

```bash
cargo test -p ploke-eval inner:: -- --nocapture
```

If this is not green, do not start the next slice.

## Next Slice

Implement only:

```text
RunRegistration -> CheckedOutWorkspace
```

Recommended file:

```text
crates/ploke-eval/src/inner/checkout.rs
```

Target for that slice:

- consume `RunRegistration`
- materialize the requested repo state
- return a `CheckedOutWorkspace`
- record repo-state evidence and keep error provenance explicit

Do not include runtime boot, workspace preparation, inquiry, validation, packaging, or emitters in that step.

## Guardrails

- One seam at a time. Stop after the slice is real, tested, and reviewed.
- Do not build a generic pipeline carrier or typestate framework up front.
- Prefer direct structs and functions over abstraction layers.
- Keep rustdoc short and concrete.
- Tests should stay narrow and deterministic.
- Error types must carry path or operation provenance.
- `inner` is the execution-core rewrite, not the CLI, replay, protocol, or campaign surface.
- Update `docs/active/workflow/handoffs/recent-activity.md` after each meaningful step with at most 100 words.
- Do not commit `crates/ploke-eval/src/inner/recon-reports/` unless you are explicitly working on recon docs.
- The repo has unrelated worktree changes outside `inner`; do not revert them.

## Resume Here

1. Run the `cargo test -p ploke-eval inner:: -- --nocapture` check.
2. Implement `CheckedOutWorkspace` and the `RunRegistration -> CheckedOutWorkspace` transition.
3. Add focused tests for successful checkout and failure-path provenance.
4. Review the slice for clarity, concision, and simpler alternatives before moving on.
