# Prototype 1 Successor Hydration Surface Mismatch

- date: 2026-05-10 local / 2026-05-10 UTC
- campaign: `p1-smoke-3x4-edit-surface-20260510-1`
- run root: `/home/brasides/.ploke-eval/campaigns/p1-smoke-3x4-edit-surface-20260510-1/prototype1`
- status: fix implemented, needs live rerun

## Summary

The 3-generation / 4-child edit-surface smoke run reached generation-1
selection, selected `node-964357d1a8177331`, then failed before successor
handoff completed:

```text
batch selection is invalid: selected successor Artifact surface mismatch after install: node=node-964357d1a8177331 branch=prototype1-node-964357d1a8177331
```

This appears to be a different failure than
`2026-05-10-prototype1-historical-successor-surface-root-mismatch.md`.
The previous bug paired the selected Artifact tree key with the wrong previous
parent surface. This failure appears to compare two different states of the
same selected node:

```text
selected child Artifact surface
hydrated successor Parent checkout surface
```

The comparison should not treat those as the same state if successor hydration
writes `.ploke/prototype1/parent_identity.json` and commits it.

## Observed Failure

Bounded terminal output from the live run:

```text
DEBUG p1 select assembled current-generation selection payloads from typed child outcomes
DEBUG p1 materialize node=node-964357d1 branch=branch-c3149e91 rt=f56eea71 hydrated selected Artifact payload for successor handoff
INFO  p1 outcome=succeeded 0ms step
[prototype1-node-964357d1a8177331 c5403e9d] prototype1: initializing gen 1 parent node-964357d1a8177331
 1 file changed, 1 insertion(+), 1 deletion(-)
INFO  p1 outcome=missing 0ms step
INFO  p1 outcome=missing 0ms step
INFO  p1 outcome=succeeded 9ms step
INFO  p1 outcome=succeeded 0ms step
batch selection is invalid: selected successor Artifact surface mismatch after install: node=node-964357d1a8177331 branch=prototype1-node-964357d1a8177331
```

The important clue is the parent identity initialization commit immediately
before the mismatch:

```text
prototype1: initializing gen 1 parent node-964357d1a8177331
1 file changed, 1 insertion(+), 1 deletion(-)
```

## Code Path

The relevant code is in `crates/ploke-eval/src/cli/prototype1_process.rs`.

`install_prototype1_successor_artifact` currently:

1. Measures `current_surface` from `active_parent_root`.
2. Resolves the selected child workspace.
3. If the child workspace still exists, commits the target artifact.
4. Writes `.ploke/prototype1/parent_identity.json` into that workspace.
5. Commits that parent identity file with
   `prototype1_successor_parent_identity_commit`.
6. Removes the child worktree.
7. Calls `install_committed_successor_artifact`.

`install_committed_successor_artifact` then:

1. Installs the selected branch into the active checkout.
2. Loads and validates the newly installed parent identity.
3. Measures `selected_surface` from the active checkout.
4. Compares `selected_surface` to `artifact.artifact_surface()`.
5. Fails if they differ.

The check that failed:

```rust
if &selected_surface != artifact.artifact_surface() {
    return Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "selected successor Artifact surface mismatch after install: node={} branch={}",
            node.node_id, workspace.branch.0
        ),
    });
}
```

## Suspected Invalid Comparison

The failure likely means the code is comparing:

```text
B_child_surface
B_parent_surface
```

as though they were the same thing.

But if successor hydration commits `parent_identity.json`, the selected child
Artifact `B` becomes a hydrated successor Parent checkout `B'`:

```text
Artifact<SelectedChild> B
  --write parent_identity.json-->
Parent<HydratedSuccessor> B'
```

Those two checkouts can have different tree keys and mutated/ambient surface
roots. The difference is expected if parent identity is part of the measured
surface.

## Relationship To Design Intent

`docs/active/plans/self-improvement-loop/artifact-surface-authority-design.md`
separates:

- Artifact Surface Measurement
- Authority Succession
- Startup Validation

This bug indicates the implementation still collapses at least two states that
the design intent treats as distinct:

```text
selected successor Artifact
hydrated successor Parent
```

The earlier fix introduced `ArtifactSurface` and required selected candidates
to carry successor-eligible artifact surface evidence. That was directionally
right, but the current handoff path appears to enforce the evidence after the
authority-hydration mutation instead of at the selected-child Artifact boundary.

## Structural Naming Failure

Collapsed shape:

```text
ArtifactSurface
InstalledSuccessorArtifact
selected_surface
```

The names do not encode whether the surface belongs to:

```text
Artifact<SelectedChild>
Parent<HydratedSuccessor>
```

Missing structure:

```text
ArtifactSurface<SelectedChild>
ArtifactSurface<HydratedSuccessorParent>
SuccessorHydration<SelectedChild, HydratedSuccessorParent>
```

or an equivalent typed transition carrier that proves when the parent identity
mutation happened.

Preventing type constraint:

- A selected child artifact surface must be validated before parent hydration.
- A hydrated successor parent surface must be minted by a hydration transition
  that consumes the selected child artifact and writes/commits parent identity.
- History sealing and successor startup must specify which surface state they
  expect. No API should compare a selected-child `ArtifactSurface` to a
  hydrated-parent `ArtifactSurface` as bare values.

## Why The Previous Fix Missed It

The previous fix correctly prevented:

```text
SurfaceCommitment(F, F)
artifact claim B_t
```

when the intended transition was:

```text
SurfaceCommitment(F, B)
artifact claim B_t
```

This run exposed the next collapsed boundary:

```text
B before successor parent identity
B' after successor parent identity
```

The code made the comparison stricter, which is good, but it did not preserve
the structural distinction between selected child Artifact state and hydrated
successor Parent state.

## Regression Test Target

Add a test that constructs or simulates:

1. A child Artifact with recorded `ArtifactSurface<SelectedChild>`.
2. Successor hydration that writes and commits
   `.ploke/prototype1/parent_identity.json`.
3. A measured `ArtifactSurface<HydratedSuccessorParent>` after hydration.
4. Validation that rejects accidental cross-state comparison.
5. Handoff sealing that uses the intended state explicitly.

The test should fail if the code compares selected-child surface evidence
directly against the post-hydration checkout surface without passing through a
typed hydration transition.

## Fix Direction

Do not weaken the surface mismatch check. The check is useful because it exposed
the missing state boundary.

Fix the handoff path by making successor hydration explicit:

```text
SelectedChildArtifact
  -> validate selected child surface
  -> hydrate successor parent identity
  -> measure hydrated successor parent surface
  -> seal handoff with the correct expected startup surface
```

The implementation should make it impossible to pass a bare `ArtifactSurface`
where the caller has not specified whether it is pre-hydration selected-child
surface or post-hydration successor-parent surface.

## Fix Implemented

Implemented on 2026-05-10:

- successor handoff no longer writes and commits
  `.ploke/prototype1/parent_identity.json` into the selected child worktree;
- child artifact persistence records `child_identity` as typed journal data but
  no longer mutates the artifact tree with a parent identity commit;
- sealed History blocks now carry `selected_parent_identity`;
- successor startup derives the next parent identity from the sealed History
  head and checks it against the successor invocation runtime/node before
  entering `Parent<Ready>`;
- predecessor startup rejects a parent identity that does not match the sealed
  History successor identity.

The remaining validation step is a fresh Prototype 1 run that reaches successor
handoff and confirms the selected Artifact surface remains stable across
startup.
