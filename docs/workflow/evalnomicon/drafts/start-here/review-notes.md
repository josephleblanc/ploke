# Review Notes

These notes summarize the documentation review behind this draft set.

## Main Synthesis

Prototype 1 is best documented as a two-axis loop:

```text
Artifact axis = file/checkouts/surfaces that can hydrate a runtime
Runtime axis  = executing binary with role and authority state
```

The already-running parent can create and inspect candidate Artifacts, but it
cannot fully evaluate semantics that only exist in a newly built child binary.
That is why the loop stages need materialize, build, spawn, and observe rather
than a single "run treatment" box.

The concrete operator stage list should come from `DiagnosedPhase` in
`run/core.rs`, not from older prose. The conceptual C-state vocabulary is still
useful, but it should explain the child transition phases rather than replace
the doctor phase names.

## Implemented Claims

- `prototype1-doctor` diagnoses a current parent phase from active parent
  identity, admitted profile, closure state, prompt preflight, child-plan state,
  child node status, and successor markers.
- `prototype1-step` refuses `blocked`, advances one non-terminal phase, then
  re-diagnoses.
- `prototype1-continue` repeats the same phase loop with a 256-advance guard.
- The admitted `run-profile.toml` owns search, generation, selection,
  execution, protocol, model defaults, and control policy.
- Child planning is parent-bound by parent node id and child generation.
- Child invocation and successor invocation have separate runtime roles.
- Handoff seals and appends a History block before successor runtime spawn.

## Partially Implemented Claims

- The `C1` through `C5` typestate files model the desired child transition
  carriers, but their module headers still mark parts of the scaffold as not
  fully replacing older paths.
- History is the intended authority surface for lineage facts, but many current
  records remain projections or evidence sources until explicitly admitted.
- Successor handoff validates sealed-head continuity in the handoff path, but
  bootstrap and broader startup admission are still narrower than the intended
  full startup model.
- Broad-harness generation is the current source-level candidate path, but its
  prompt markdown is presentation. The durable contract is the typed request
  and admitted result JSON.

## Not Claimed

These drafts should avoid implying:

- scheduler state is Crown or History authority;
- a child can self-promote by writing a good final message;
- protocol completion proves semantic improvement;
- a non-empty patch proves a clean benchmark result;
- a temporary child worktree is the next long-lived parent home;
- the current History model provides distributed consensus or global process
  uniqueness;
- a stale source-side appendix is current without rechecking source.

## Documentation Gaps To Work Next

1. Add a diagram for the two-axis Artifact/Runtime model.
2. Add a diagram for `doctor -> step -> doctor` and `continue`.
3. Split broad-harness planning into a dedicated lifecycle doc:
   request publication, TUI attempt, submitted result, admission, child-plan
   binding, materialization.
4. Add a History import note explaining how transition journal evidence should
   become admitted History facts.
5. Add a compact stage-to-command cheat sheet after the stage vocabulary
   stabilizes.
6. Compare `prototype1-state` older complete-mode control flow with the
   `doctor`/`step`/`continue` path and document which path is current for which
   operator task.

## Review Discipline

Before promoting any of this into the book, refresh the source map and check:

- the `DiagnosedPhase` enum and phase labels;
- `advance` dispatch;
- child status to phase mapping;
- successor marker to terminal phase mapping;
- admitted run-profile schema;
- broad-harness request/result schemas;
- command help output.

If those have drifted, update the drafts first rather than copying stale
language forward.

