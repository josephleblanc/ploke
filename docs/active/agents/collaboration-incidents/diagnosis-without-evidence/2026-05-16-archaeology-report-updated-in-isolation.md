# 2026-05-16 Archaeology Report Updated In Isolation

- Trigger:
  User pointed out that the latest archaeology update was lazy and did not
  follow the existing instructions or report standard, then explicitly asked
  whether `docs/active/archaeology/ploke-tree-graph/artifact-identity.md`
  existed and whether there were more archaeology documents I should have read.
- User-visible failure:
  The agent patched `artifact-promotion-continuity.md` mechanically without
  first re-reading the archaeology set already present in
  `docs/active/archaeology/`. That produced a shallower report update than the
  established standard visible in `artifact-identity.md`, and it made the
  archaeology pass look perfunctory instead of evidence-first.
- Touched code surface:
  - `docs/active/archaeology/ploke-tree-graph/artifact-promotion-continuity.md`
  - `docs/active/archaeology/INDEX.md`
  - `crates/ploke-tree/src/graph/artifact_tree.rs`
- What the agent did:
  - read the archaeology skill and the target report
  - updated that one report and added proof comments
  - did not re-read `docs/active/archaeology/README.md`, the group
    `README.md`, or the sibling archaeology reports before editing
  - therefore missed the fuller report depth and cross-report consistency that
    was already present in `artifact-identity.md`
- Skipped docs / skills / instructions:
  - skipped the practical example already available in
    `docs/active/archaeology/ploke-tree-graph/artifact-identity.md`
  - skipped the archaeology directory indexes as working context rather than
    just as navigation files:
    - `docs/active/archaeology/README.md`
    - `docs/active/archaeology/INDEX.md`
    - `docs/active/archaeology/ploke-tree-graph/README.md`
  - followed the letter of `ui-claim-archaeology` only partially instead of
    using the existing archaeology set as the concrete house style
- Why the behavior was risky:
  Archaeology docs are supposed to become reusable source-of-truth, not just
  local unblockers. Updating one report in isolation lets report quality drift,
  loses cross-claim consistency, and makes later semantic work easier to fake.
- Concrete prevention rule:
  Before editing an archaeology report in an existing group, re-read:
  - `docs/active/archaeology/README.md`
  - `docs/active/archaeology/INDEX.md`
  - the group `README.md`
  - at least one sibling report in the same group

  Then match the established report depth and explicitly state whether the new
  claim reuses, refines, or diverges from the existing sibling carrier
  decisions.
- Memory hypothesis:
  This was not a memory shortage problem. It was a workflow discipline problem:
  I treated the archaeology target file as the whole context instead of using
  the archaeology set itself as the required local context.
