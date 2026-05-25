# Prototype 1 History, Crown, And Surface Digest Reference

Use this reference before diagnosing Prototype 1 state-transition blockers,
successor handoff, missing treatment evidence, History records, Crown state, or
surface-digest failures.

Primary sources:

- `docs/workflow/evalnomicon/drafts/history/README.md`
- `docs/workflow/evalnomicon/drafts/history/crown-authority-background.md`
- `docs/workflow/evalnomicon/drafts/edit-surface/model.md`
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `crates/ploke-eval/src/cli/prototype1_state/inner.rs`
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- `crates/ploke-eval/src/cli/prototype1_state/backend.rs`
- `crates/ploke-eval/src/cli/prototype1_process.rs`

## Why The Gates Are Strict

Prototype 1 is a self-editing loop. A candidate runtime can propose edits and
evaluate its own descendant, so the loop must not let mutable projections,
terminal summaries, scheduler files, branch names, or model self-reports become
promotion authority.

The strict gates preserve this invariant:

```text
Only a runtime admitted as Parent<Ruling> for one lineage may mutate that
lineage, admit History entries, and choose the next active Parent.
```

That is why state-transition failures are not just operational errors. If a
required eval/protocol/oracle/surface/handoff artifact is malformed,
contradictory, absent, or unverifiable, continuing the same run may import an
untrusted transition into the lineage. The normal response is to preserve the
evidence, repair the source workflow or replay path, and start a fresh run.

## Authority Objects

Important objects and their current meaning:

- `Artifact`: checkout/tree state that can hydrate a runtime.
- `Runtime`: executing process hydrated from an Artifact.
- `Parent<Ruling>`: runtime role with authority to mutate one lineage.
- `Crown<Ruling>`: capability for one lineage authority epoch.
- `Crown<Locked>`: prior Parent has locked succession for one selected
  successor.
- `Block<Open>`: History block while a Parent holds the Crown.
- `Block<Sealed>`: committed authority epoch. Its block hash commits to header
  material, entries root, Crown lock transition, selected successor, selected
  parent identity, active artifact, claims, and surface commitment.
- `SurfaceCommitment`: immutable root plus before/after mutated and ambient
  roots.
- `FsBlockStore`: local append-only sealed-block store plus projections. The
  projection files are not themselves the authority.

Mutable files such as `scheduler.json`, branch registries, node records,
invocation files, ready/completion files, channel files, and doctor reports are
evidence or projections. They become authority only if admitted into a sealed
History block or imported later under an explicit ingress policy.

## Surface Digests

The current concrete surface partition is:

```text
Immutable = crates/ploke-eval
Mutated   = ordinary tool/edit surface, including tool-description text files
Ambient   = empty declared surface
```

The digest matters because it is the induction step for ordinary descendants.
If the first admitted Parent has immutable digest `D`, and every child and
successor must prove the same immutable digest `D` before execution/admission,
then every admitted descendant in this transition system runs with the same
policy-bearing loop code.

This does not prove that no external process can run incompatible code. It says
that such a process is outside the admitted History/Crown mutation path.

Current code surfaces:

- `backend.rs::GitWorktreeBackend::surface_commitment` hashes the immutable,
  mutated, and ambient surfaces across a before/after Artifact transition.
- `SurfaceCommitment::from_artifact_surfaces` rejects immutable root drift.
- child execution validates the surface before hydration/build and again after
  child Artifact persistence.
- successor startup recomputes the current checkout surface and compares it
  with the sealed History head before entering the parent path.

The longer-term plan is an authenticated shared ledger. Multiple versions of
this program may run against a shared git tree, with lineage-local authority
chains recorded into a shared History substrate. The current local block store
is not distributed consensus, but its block hashes, parent hashes, state-root
slot, and sealed surface/artifact claims are deliberately shaped toward an
authenticated ledger model.

## Edit Surface Model

The TUI/edit harness is an executor and projection surface, not authority.

Do not collapse this:

```text
Runtime -> SurfaceGrant(Artifact) -> EditProposal
EditProposal + boundary check + apply -> derived Artifact
derived Artifact -> child Runtime self-evaluation
```

into this:

```text
model produced patch, therefore candidate is admitted
```

Important negative relations:

```text
EditProposal(q) does not imply ArtifactTransition(q)
EditProposal(q) does not imply Admit(q)
UIApproval(q) does not imply HistoryAuthority(q)
Preview(q) does not imply Evidence(q)
```

The code graph and database are derived views over an Artifact. A stale
`DatabaseIndex` must not authorize edits against a different Artifact unless
the relevant file/node hashes still match and the projection has been
explicitly accepted for that target Artifact.

## Seal And Unseal Path

Current successor seal path:

1. Parent selects a candidate Artifact.
2. `install_prototype1_successor_artifact` installs the selected Artifact into
   the stable active checkout.
3. `install_committed_successor_artifact` verifies the target, records before
   and after checkout journal entries, validates parent identity, recomputes
   selected surface, and builds a `SurfaceCommitment`.
4. `spawn_and_handoff_prototype1_successor` creates `SealBlock::from_handoff`.
5. `Parent<Selectable>::seal_block_with_artifact` opens the block under
   `Crown<Ruling>`, admits the artifact claim and selection decision, locks the
   Crown, and produces `Block<Sealed>`.
6. `FsBlockStore::append` verifies the expected lineage state, verifies the
   block hash, appends the sealed block, and updates rebuildable projections.
7. Only after the History block is sealed and appended does the parent spawn
   the detached successor runtime.

Current successor admission path:

1. Successor command starts through the `prototype1-state` handoff path.
2. `validate_prototype1_successor_continuation` loads the sealed head and
   checks runtime identity and selected successor/active artifact consistency.
3. `Startup<Predecessor>::from_history` loads the sealed head, checks selected
   parent identity, recomputes the current clean tree key, verifies the sealed
   artifact claim, recomputes current surface, and verifies the sealed surface.
4. Only then may the runtime enter the parent path as the next
   `Parent<Ruling>`.

Invocation and ready files are transport/debug evidence for the handoff. They
are not the handoff authority.

## Operational Rules For Agents

- Do not run overlapping loop/eval commands against the same campaign.
- Do not "repair" an invalid sealed/handoff state in place unless the user
  explicitly asks for artifact-store surgery.
- If required transition evidence is missing, malformed, contradictory, or
  unverifiable, treat the current run as blocked or abandoned.
- Do not interpret `runner-result.json`, channel messages, scheduler state,
  doctor output, or terminal summaries as authoritative by themselves.
- If artifacts disagree, inspect the typed transition and History surfaces
  first.
- If a candidate touched the immutable surface during ordinary succession, it
  must fail before becoming an admitted descendant.
- If stale same-file edit failures or `ContentMismatch` appear, remember the
  edit-surface invariant: the code graph projection must still match the target
  Artifact before it can authorize a write.
- If the current model needs to support policy-bearing edits to
  `crates/ploke-eval`, that is a protocol-upgrade transition, not an ordinary
  child edit.

## Current Implementation Limits

The implementation is intentionally local and partial:

- it is a tamper-evident lineage-scoped block model, not distributed consensus;
- bootstrap startup still uses configured-store absence, while predecessor
  startup verifies a sealed head;
- open-block entry authority is partially type-gated by Crown, but several
  actor identities are still passed as data;
- block-store projection files are rebuildable indexes, not proof-bearing
  consensus state;
- ingress capture/import while the Crown is locked is still future work.

These limits matter during diagnosis. Preserve the current invariant before
adding convenience paths. A missing concrete transition should become a typed
box/transition, not another ad hoc acknowledgement file.
