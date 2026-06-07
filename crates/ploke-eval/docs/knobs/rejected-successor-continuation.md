# Prototype 1 Rejected Successor Continuation

Use this note when a Prototype 1 loop should continue from the highest-scoring
child even when that child's branch evaluation disposition is `reject`.

The narrow config knob is:

```toml
[search]
require_keep_for_continuation = true
explore_from_rejected = true
```

With `require_keep_for_continuation = true`, a selected rejected child normally
stops with `StopSelectedBranchRejected`. Setting
`explore_from_rejected = true` keeps the keep-only gate visible, but allows the
selected rejected branch to become the next exploration parent with
`ContinueExploreFromRejected`.

Do not change `require_keep_for_continuation` to `false` for this purpose unless
the intended policy is broader than rejected exploration. Setting it to `false`
removes the keep requirement entirely.

## 2026-06-07 Handoff Stability Run

Reference profile:
`~/.ploke-eval/profiles/p1-admissionfix-g35flash-p25flash-20260606-204954.toml`

Next run profile:
`~/.ploke-eval/profiles/p1-rejected-handoff-5g1x2-a2-20260607-175030.toml`

The next profile keeps the same dataset, eval model, protocol model, selection
strategy, metrics, oracle mode, protocol token budget, and broad TUI retry count
as the reference profile. The intentional shape changes are:

```toml
[search]
max_generations = 5
require_keep_for_continuation = true
explore_from_rejected = true

[search.children]
min = 1
max = 2
parallel_targets = 2

[execution.broad_tui]
max_attempts = 2
fresh_slots_per_child = 2

[control]
parallel_cap = 2
```

The expected evidence from a successful run is repeated parent to successor
parent handoff across several transitions, including continuation from a
selected rejected child when no keep-worthy child is available.
