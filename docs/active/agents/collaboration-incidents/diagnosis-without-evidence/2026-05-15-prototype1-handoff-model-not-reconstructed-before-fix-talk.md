# 2026-05-15 Prototype1 Handoff Model Not Reconstructed Before Fix Talk

- Trigger:
  User asked how the parent selects a child, how the child becomes the next
  parent, which directory that happens in, and how the git/worktree handoff is
  supposed to work across `prototype1_state` and the runtime loop docs.
- User-visible failure:
  The agent kept repeating the local path-mismatch explanation before fully
  reconstructing the parent/child/successor handoff model from the runtime
  docs and the controlling `ploke-eval` carriers. That made the proposed fix
  sound under-justified and forced the user to question whether the change was
  safe.
- Touched code surface:
  - `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/c1.rs`
  - `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
  - `crates/ploke-eval/src/cli/prototype1_process.rs`
  - `docs/workflow/evalnomicon/drafts/runtime/loop.md`
- What the agent did:
  - diagnosed the immediate broad-harness failure correctly as a root witness
    mismatch
  - but explained the fix before first re-deriving the larger runtime model:
    stable active parent checkout, temporary child worktrees, selected artifact
    installation back into the active checkout, then successor startup from
    that active checkout
  - left the user to ask, repeatedly, whether the reasoning behind the
    existing structure had actually been understood
- Skipped docs / skills / instructions:
  - skipped the required semantic reconstruction step implied by the user's
    question
  - skipped the main discipline of the light-thread/runtime workflow: recover
    the authority model before changing a handoff boundary
- Why the behavior was risky:
  A local path fix in this area is only safe if it preserves the real role of
  the active parent checkout versus temporary child worktrees. Speaking about
  the patch before proving that model made the change look like blind
  normalization instead of a repair to a specific provenance witness.
- Concrete prevention rule:
  Before proposing or applying a fix in Prototype 1 handoff/materialization
  code, first restate the active-checkout/child-worktree/successor-startup
  model from `prototype1_state/mod.rs`, `prototype1_process.rs`, and the
  runtime loop docs. Only then explain which witness is wrong and why the fix
  preserves the larger handoff contract.
- Memory hypothesis:
  If memory helps, the agent should treat “worktree path mismatch” in
  Prototype 1 as a two-step task:
  `reconstruct handoff model -> identify broken witness`, never the reverse.
