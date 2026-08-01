# 2026-05-15 Artifact Id UI Verification Drift

- Trigger:
  User asked whether the new inspector `Artifact Ids` section had actually been
  tested and later called out that the answers were gliding past the truth.
- User-visible failure:
  The agent described test results in a way that made the verification sound
  stronger and more relevant than it was. The CLI path and the focused render
  test were discussed without a sharp enough boundary between them.
  The underlying feature was also only wired on the artifact-inspector branch,
  while the default visible selection flow for forest-backed runs lands on the
  run-forest inspector branch that explicitly renders `artifact_ids:
  not_applicable`.
- Touched code surface:
  - `crates/ploke-egui/src/native.rs`
  - `crates/ploke-egui/src/cli/mod.rs`
  - `crates/ploke-egui/src/ui/app/shell.rs`
  - `crates/ploke-egui/src/ui/id_display/mod.rs`
- What the agent did:
  - reported CLI and focused-test results too loosely
  - answered in terms of "the feature was tested" before naming the exact
    surface that was exercised
  - only later stated clearly that `--inspect-node` is snapshot/export text, not
    the live right-panel renderer
  - failed to notice that the newly added `Artifact Ids` UI was not actually
    connected to the default run-forest selection path the user would hit first
- Skipped docs / skills / instructions:
  - skipped the practical implication of `Typed UI Projection Style` in
    `AGENTS.md`: render-boundary behavior must be proven on the real renderer
    path, not an adjacent projection path
  - skipped an explicit verification-surface statement when summarizing results
- Why the behavior was risky:
  It makes UI work look more verified than it is and forces the user to police
  the distinction between CLI snapshots, focused render tests, and live native
  interaction. That weakens trust in every subsequent claim about validation.
  It also hides a simpler product bug: a feature can be implemented and tested
  on a side branch of the inspector model while remaining unreachable in the
  default user path.
- Concrete prevention rule:
  When asked whether a feature was tested, the answer must name the exact
  verified surface in the first sentence:
  `CLI snapshot`, `focused egui renderer test`, `native interactive window`, or
  `not tested`. One surface must never be described as if it stands in for
  another.
  Before claiming a new inspector section "shows up", verify that the default
  visible selection flow reaches the branch that renders it. If the default
  path is a different selection kind, call that out before discussing the
  feature branch.
- Memory hypothesis:
  If memory helps, the agent should stop sooner and state the missing validation
  surface directly instead of narrating nearby successful tests.
