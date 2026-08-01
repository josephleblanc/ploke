# Prototype 1 Deterministic Edit-Surface Generation Pretends to Know Patch Targets

- date: 2026-05-11 local / 2026-05-11 UTC
- campaign: `p1-mbe-provenance-4g3c-20260511-1`
- status: mitigated by material surface gate; rerun needed for live validation
- related bug: `docs/active/bugs/2026-05-11-prototype1-mbe-shared-instance-patch-provenance.md`
- related commit: `c2d0f1eb Fix MBE patch projection provenance`
- mitigation commit: `5f92eb6e Restrict prototype1 workspace edit surface`

## Summary

The live `edit-surface` candidate generator is using a deterministic EOF-comment
splice as if it were a real patch proposal mechanism. That makes
`workspace-except-ploke-eval` behave like random tracked text-file selection
minus authority prefixes, not like a meaningful diagnosis-to-surface route.

In the MBE provenance validation campaign, that mock generator produced three
generation-1 child candidates whose target paths were archive Markdown files:

```text
docs/archive/plans/agentic-system-plan/impl-log/impl_003.md
docs/archive/plans/agentic-system-plan/impl-log/impl_20250827-055621Z.md
docs/archive/plans/agentic-system-plan/impl-log/impl_004.md
```

Each child reached `binary_built`, but all three failed before child self-eval
at:

```text
prototype1_child_artifact_commit
git command 'git add -- <paths>' failed with status 1
```

Because the run never reached child self-eval, it never created the expected
child-owned MBE duplicate target directories:

```text
nodes/<child-node>/instance-targets/<treatment-campaign-id>/BurntSushi/ripgrep
```

## 2026-05-12 Mitigation

Commit `5f92eb6e` tightened the existing parent-side material surface gate for
`workspace-except-ploke-eval`. The broad surface now excludes tracked paths
under:

```text
.cargo
docs/archive
docs/active/bugs
```

and rejects dependency/toolchain capability files by filename:

```text
Cargo.toml
Cargo.lock
rust-toolchain.toml
```

The same predicate is used for both enumeration and validation, so tracked
archive docs should no longer be selected by the deterministic producer or
accepted later as valid parent mutation targets. Regression tests were added
for archive docs and capability files.

This is a mitigation, not a full design fix. The deterministic EOF-comment
producer still exists, and it still fabricates parent-side candidate edits
instead of using a real `ploke-tui` harness proposal. The remaining design debt
is to replace that mock live path with a real broad-minus-core parent mutation
route, where the parent supplies context/objective, `ploke-tui` proposes edits,
and `ploke-eval` gates the resulting touches.

## Impact

This blocked live validation of the MBE patch provenance fix. The commit
`c2d0f1eb` may still be structurally correct, but it has not yet been exercised
by a live child self-eval path after the `5f92eb6e` surface-gate mitigation.

The current `workspace-except-ploke-eval` behavior also makes Prototype 1 run
semantics misleading: the name suggests a meaningful editable workspace surface,
but the generator is not deriving a patch target from diagnostics, History,
protocol evidence, graph anchors, or a model proposal. It is picking text files
by sorted path order plus a hash offset, then appending a synthetic comment.
That is acceptable as a narrow splice fixture. It is not acceptable as live
self-improvement candidate generation.

## Observed Evidence

Campaign:

```text
/home/brasides/.ploke-eval/campaigns/p1-mbe-provenance-4g3c-20260511-1
```

Generation-1 child nodes:

```text
node-1d3b3c756611909c -> branch-6d50a6765f502a43 -> impl_003.md
node-3d8a6d5bf1cb46d3 -> branch-f8fbe6553cddb531 -> impl_20250827-055621Z.md
node-19743e3674884cc1 -> branch-4b51e29baa2dc78e -> impl_004.md
```

Child node records showed:

```text
status = binary_built
runner-result.json absent
instance-targets absent
```

The observation stream reported all three planned child tasks failed with:

```text
database setup failed during 'prototype1_child_artifact_commit':
git command 'git add -- <paths>' failed with status 1
```

## Expected Behavior

The loop must not mint live child Artifacts from a deterministic fake patch
target. A live candidate should come from one of two explicit routes:

1. A real proposal route: `Artifact + graph/evidence surface + grant + objective
   -> proposed touches -> checked proposal -> child Artifact`.
2. A blank-slate workspace route: child receives an ordinary editable checkout
   outside known no-edit invariant paths, the model edits freely, and the parent
   validates the resulting diff before admitting an Artifact.

Both routes still need one shared notion of an admitted edit target:

- normal repo-relative path
- inside the declared non-authority edit surface
- tracked by the Artifact checkout
- writable as an ordinary candidate edit
- persistable by normal `git add -- <path>` / artifact commit policy

The observed `docs/archive/**` paths are evidence that the fake generator and
the artifact admission relation are being conflated. They are not the complete
definition of the bug.

## Fix Direction

Do not patch this by forcing `git add -f`. Ignored or archive paths should not
be child artifact targets for this live validation run.

Do not fix this by making the random picker slightly less random. The
deterministic EOF-comment generator should be demoted to mock/splice-test use or
renamed so live profiles cannot mistake it for a real self-improvement
generator.

For the next live MBE validation run, either:

- switch back to the existing legacy parent-driven generation path if it lets
  the model produce an actual patch; or
- add a deliberately named blank-slate workspace generator that gives each child
  an editable checkout, forbids only known invariant/no-edit paths, captures the
  resulting diff, and validates/persists that diff through normal Artifact
  policy.

A later protocol-backed route can use the planned chain:

```text
History/scoring evidence -> failure classifier -> routing table -> graph surface
```

and then produce grants over `R/W/F` sets. Until that exists, the honest
prototype behavior is blank-slate plus post-hoc diff validation, not fabricated
target attribution.

After this mitigation, rerun the 4-generation / 3-child campaign and verify the
MBE provenance path reaches child self-eval and creates child-owned target
directories under:

```text
nodes/<child-node>/instance-targets/<treatment-campaign-id>/BurntSushi/ripgrep
```
