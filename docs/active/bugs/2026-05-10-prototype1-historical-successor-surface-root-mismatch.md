# Prototype 1 Historical Successor Surface Root Mismatch

- date: 2026-05-10 local / 2026-05-10 UTC
- campaign: `p1-overnight-edit-surface-20260510-3`
- run root: `/home/brasides/.ploke-eval/campaigns/p1-overnight-edit-surface-20260510-3/prototype1`
- status: active

## Summary

The overnight edit-surface run progressed through two History handoffs, then
failed while starting the generation-2 successor runtime.

The successor checkout itself appears to be the selected historical Artifact,
but the sealed History block paired that selected Artifact with the wrong
mutated surface root. Successor startup recomputed the current surface from the
checked-out Artifact and correctly rejected the sealed expectation:

```text
prototype1-state successor failed: database setup failed during 'prototype1_history':
current mutated surface root does not match sealed expectation
```

This is not a scheduler/projection failure. It is a sealed History handoff
failure: the system admitted an invalid block by combining facts from two
different structural positions.

## Observed Failure

Transition record:

```text
/home/brasides/.ploke-eval/campaigns/p1-overnight-edit-surface-20260510-3/prototype1/transition-journal.jsonl
```

Bounded inspection showed:

```text
line_count=265
size=384K
mtime=2026-05-10 01:28
```

The tail records:

```text
kind=successor decision=continue_ready next_generation=2 selected_branch=branch-2218f572579e7d94
kind=successor checkout_phase=before
kind=successor checkout_phase=after
kind=active_checkout_advanced
kind=successor runtime_id=faa42527-02f1-4e0d-a62b-cf90513627dd spawned_pid=3876893
kind=successor runtime_id=faa42527-02f1-4e0d-a62b-cf90513627dd completed=failed
kind=successor runtime_id=faa42527-02f1-4e0d-a62b-cf90513627dd exited_before_ready exit_code=1
```

The successor completion channel has one bounded completion message:

```text
/home/brasides/.ploke-eval/campaigns/p1-overnight-edit-surface-20260510-3/prototype1/nodes/node-6b657fe2f06feed8/channels/faa42527-02f1-4e0d-a62b-cf90513627dd/child-to-parent.jsonl
```

with detail:

```text
prototype1-state successor failed: database setup failed during 'prototype1_history':
current mutated surface root does not match sealed expectation
```

## History Evidence

The sealed block segment contains three records:

```text
/home/brasides/.ploke-eval/campaigns/p1-overnight-edit-surface-20260510-3/prototype1/history/blocks/segment-000000.jsonl
```

The relevant block fields are:

```text
height=0
selected_successor.runtime=caa5dca9-6b10-483e-9cc7-6e31d2e49543
active_artifact=artifact:text-file-sha256:6b701f0d6c4fccd218eaf0b934adca9a9c36428bd2324ae7428360e4bd569a9b
surface.mutated.before=0eaa0f7b92bdb8cf62d48de74edf3a72fa2ef6014cb7707997068d5e33abffb5
surface.mutated.after=a308de9d496c03e0708470e37658396bfa29744f2122c1b6b7964516a1861c5f

height=1
selected_successor.runtime=9d6273f6-8c56-4895-8010-391b11247ccb
active_artifact=artifact:text-file-sha256:0684725d26d15896ec796bba93e2729b7d886133944ad64d6ac0a3a65ac596a1
surface.mutated.before=a308de9d496c03e0708470e37658396bfa29744f2122c1b6b7964516a1861c5f
surface.mutated.after=e137ffd4d133a847d9fa2f4ac4d208fa0d9b4b9731961fd4388082f09521a744

height=2
selected_successor.runtime=faa42527-02f1-4e0d-a62b-cf90513627dd
active_artifact=artifact:text-file-sha256:6b701f0d6c4fccd218eaf0b934adca9a9c36428bd2324ae7428360e4bd569a9b
surface.mutated.before=e137ffd4d133a847d9fa2f4ac4d208fa0d9b4b9731961fd4388082f09521a744
surface.mutated.after=e137ffd4d133a847d9fa2f4ac4d208fa0d9b4b9731961fd4388082f09521a744
```

Height 2 selects the same Artifact as height 0, but seals the height-1/current
mutated root as the selected Artifact's `after` surface root. That is the
invalid state.

## Failure Path

The code path is:

- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`
  - `ParentSelection::select_successor`
  - `select_artifact_for_handoff`
- `crates/ploke-eval/src/cli/prototype1_process.rs`
  - `spawn_and_handoff_prototype1_successor`
  - `prepare_prototype1_active_successor_runtime`
  - `install_prototype1_successor_artifact`
  - `install_committed_successor_artifact`
  - `handoff_block_fields`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
  - `Startup<Predecessor>::from_history`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  - `Block<Sealed>::verify_current_surface`
  - `SurfaceCommitment::verify_current`

For a current-generation candidate whose workspace still exists,
`install_prototype1_successor_artifact` computes:

```text
surface_commitment(active_parent_root, workspace.root)
```

before removing the child workspace and switching the active checkout.

For a historical candidate whose workspace has been cleaned up, the else branch
computes:

```text
surface_commitment(active_parent_root, active_parent_root)
```

before switching the active checkout to the selected historical branch. The
function then calls `install_committed_successor_artifact`, which switches the
checkout and returns the stale surface commitment.

`handoff_block_fields` computes the clean tree key after checkout, so the sealed
Artifact tree claim can be correct while the sealed surface commitment is from
the wrong checkout state.

Successor startup later recomputes:

```text
surface_commitment(active_parent_root, active_parent_root)
```

from the selected checkout, compares it against the sealed head, and fails on
the mutated root mismatch.

## Invariant Violated

A sealed successor handoff block must commit a coherent selected Artifact:

```text
selected Artifact tree key
selected Artifact surface roots
selected successor runtime identity
```

Those facts must all describe the same selected Artifact at the same handoff
position.

The invalid encoded state is:

```text
Artifact tree key: selected historical successor
mutated surface root: previous/current parent before checkout
```

The successor checkout was likely valid. It was marked invalid because the
sealed expectation was not the selected Artifact's surface expectation.

## Structural Naming Failure

The failing shape is not just an implementation typo. The types allowed a bare
`SurfaceCommitment` to be passed through the handoff path without carrying the
role/state relation that produced it.

Collapsed shape:

```text
SurfaceCommitment
active_parent_root
selected Artifact
```

Missing structure:

```text
SurfaceTransition<CurrentParentBefore, SelectedArtifactAfter>
SelectedArtifactCommitment {
  artifact: Artifact<Selected>,
  tree_key: TreeKey<Selected>,
  surface: SurfaceCommitment<Selected>,
}
```

or an equivalent typed carrier minted only by the backend transition that
installs the selected Artifact.

Preventing type constraint:

- `SealBlock::from_handoff` and `OpenBlock` construction must not accept a bare
  `ArtifactRef` plus a bare `SurfaceCommitment`.
- The handoff constructor must require a single carrier that proves the Artifact
  tree key and surface roots were derived from the same selected Artifact
  transition.
- The no-workspace historical path must either materialize a temporary selected
  checkout for simultaneous before/after sampling, or use a backend method that
  samples `before` before switching and `after` after switching without losing
  the typestate.

If this relation had been modeled structurally, the height-2 block could not
have paired the selected historical Artifact with the previous parent's surface
root.

## Why Existing Tests Missed It

Existing tests cover useful parts of the path:

- current-generation edit-surface candidates carry surface evidence
- mismatched surface Artifact evidence is rejected before sealing
- successor startup rejects a mismatched current surface

The missing regression is the historical-successor/no-workspace branch:

```text
current parent surface = B
selected historical Artifact surface = A
selected historical workspace no longer exists
handoff seals Artifact A but accidentally seals surface B
successor startup rejects A because sealed expectation says B
```

## Regression Test Target

Add a test that constructs or simulates:

1. Parent at surface root `B`.
2. Historical selected candidate whose Artifact/tree/surface root is `A`.
3. No child workspace for the selected historical candidate.
4. Handoff installation through the historical/no-workspace path.
5. Sealed block contains Artifact `A` and surface root `A`, not `B`.
6. `Startup<Predecessor>::from_history` accepts the successor checkout.

The test should fail on the current code by observing the stale `B` surface root
in the sealed block or the successor startup mismatch.

## Fix Direction

Do not patch this by weakening `verify_current_surface`. The startup rejection
is doing its job.

Fix the admission path by replacing the loose return value from
`install_prototype1_successor_artifact` with a typed handoff artifact carrier.
That carrier should be minted only after the selected Artifact has been
installed or otherwise materialized, and should contain:

- selected node identity
- selected branch/artifact identity
- selected clean tree key
- selected surface commitment
- source role, e.g. current generation vs admitted History

Then seal the History block from that carrier, not from independently computed
`successor_artifact`, `artifact_key`, and `surface` values.

