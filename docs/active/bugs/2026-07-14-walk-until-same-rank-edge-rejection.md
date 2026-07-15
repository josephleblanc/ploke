# Walk Until Rejects Ordered Same-Rank Edges

Status: fixed, regression-covered, and live verified.

Canary worktree:

```text
/home/brasides/.ploke-eval/worktrees/p1-stage4-observe-g35f-oaiembed-3g1x3-p3-20260714-004446
```

## Broken Contract

When the walk service advertises a concrete next edge, `walk step --until` must
accept that target and drive the same typed transition. A coarse branch-layer
rank must not classify a real forward edge as an already-passed sibling branch.

## Evidence

After the worker-stack repair was live verified, operation
`d1b3a0cf-9bc9-49bf-ba52-caa4270d4cff` committed R4a-to-R4b with cursor evidence
`d48c0fed8fb31d28ad5f47308db555ad370c3e145aab6e0738c4173d2e42c55e`
at journal revision 19. Status then advertised exactly one enabled typestate
edge:

```text
r4b_to_r4c -> r4c
```

The corresponding command failed:

```text
ploke-eval loop walk step --until r4c --watch
```

Operation `1a11d886-1001-4944-83b8-0256da142096` reported:

```text
durable controller cursor is already at r4b, beyond requested target r4c
```

The authoritative controller journal retained R4b. Fence 7 contains only
`Acquired -> Released`, with no `AttemptBegan`, transition effect, cursor
commit, or blocker. Status remained active at R4b at journal revision 21, so
the same canary can replay the repaired edge without abandonment or salvage.

## Source Trace

`WalkController::step_at` routes an explicit `--until` target through
`advance_until`, `step_toward`, and `step_claimed` in
`crates/ploke-eval/src/cli/prototype1_state/walk/controller.rs`.

`step_claimed` rejects a target as already passed when its private
`phase_rank` is lower than the durable cursor, or when the ranks are equal but
the phases differ. That equality is the intended guard for incomparable branch
alternatives. The rank table incorrectly assigned equal ranks to two pairs that
have real directed edges:

```text
R2a == R3   despite r2a_to_r3
R4b == R4c despite r4b_to_r4c
```

The same defect also exists in `advance_until`'s post-edge overshoot check. A
multi-edge genesis request from R4a through R4b to R4c could therefore commit
the first edge and then misclassify R4b as the wrong branch.

## Docs and Policy Expectation

`docs/active/agents/2026-07-13_prototype1-loop-operator-control-plan.md`
requires CLI and UI to drive every typestate transition through the same typed
walk service. The service's advertised action and its guarded mutation command
must agree. `ControlEdge::ALL` is the closed source vocabulary for those
concrete transitions.

## Current Repro Coverage

The focused regression exhausts `ControlEdge::ALL` and requires every concrete
edge to strictly increase `phase_rank`:

```text
cargo test -p ploke-eval \
  cli::prototype1_state::walk::controller::tests::phase_rank_increases_across_every_control_edge \
  -- --exact --nocapture
```

Before the fix, the invariant failed for `r2a_to_r3` and `r4b_to_r4c`. The
rank table now separates those ordered phases and shifts later layers.

A second regression pins equal ranks only for known incomparable alternatives:

```text
R11a / R11
R13a / R13b / R13c
R14a / R14b
```

This prevents a future rank edit from making one unreachable sibling appear
forward-reachable. Rank remains a coarse topological layer; typed edges and
branch admission remain the reachability authority.

## Missing Repro Coverage

A deterministic fixture-backed controller regression for the complete journal
lifecycle and a multi-edge R4a-to-R4c request remain useful follow-up coverage.

## Live Validation

Commit `0ec681ebb` built binary SHA-256
`c21fe1331653eea5ad566e508bfa1eeee9d12b2531b9fd1533723a749d8d8acc`
with epoch mtime `1784088726084`. Operation
`bc97c546-564a-4717-b77b-4078de72b5a9` admitted only that rebuilt binary
epoch; the canary checkout and source-status hash were unchanged.

Operation `95a5041b-c1de-42a4-86c6-13edabeabf47` then replayed the exact
`walk step --until r4c --watch` request. The journal recorded
`AttemptBegan(R4b -> R4c)`, a committed `AttemptFinished`, and `Released`.
The controller advanced to R4c with evidence
`b2026c87a3e46bd0ea67993d7a3f62016ddaa28aad3494da27e6d54a049e728a`
at revision 28, authority remained active with no blocker, and the same server
subsequently committed R4c to R5.

## Fix Direction

Keep one strict topological ordering across every concrete control edge while
retaining equal ranks for true branch alternatives. This repairs both the
pre-edge target check and the post-edge overshoot check without relaxing either
guard, changing persisted schemas, or bypassing typed branch selection.

Do not fix this by dropping the equal-rank rejection. That would admit
unreachable sibling targets such as R11a-to-R11 far enough to execute an
unintended branch edge before detecting overshoot.

## Related Bugs

- [`2026-07-14-ploke-eval-walk-worker-stack-overflow.md`](./2026-07-14-ploke-eval-walk-worker-stack-overflow.md)
  records the immediately preceding server abort and recovery on the same
  canary.
