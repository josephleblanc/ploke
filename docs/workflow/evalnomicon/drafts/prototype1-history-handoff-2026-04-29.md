# Prototype 1 History Handoff

Recorded: 2026-04-29 03:49 PDT

## Current State

Latest committed checkpoint:

```text
2851650e prototype1: tighten history crown authority
```

That commit contains the current History/Crown typestate tightening, review
reports, record audits, genesis authority type shape, backend-owned tree key
shape, and Crown-gated sealing API work.

Uncommitted at handoff:

```text
crates/ploke-eval/src/cli/prototype1_state/history.rs
crates/ploke-eval/src/cli/prototype1_state/mod.rs
```

These uncommitted changes are documentation-only. They record the newer
conceptual direction around startup admission, lineage-local History height,
configured-store scoped genesis absence, authenticated lineage-head maps, and
artifact-local provenance manifests. `cargo fmt --all` and
`cargo check -p ploke-eval` passed after these doc updates, with only
pre-existing warnings.

The local agent-facing task stack was also updated with:

```text
history-block-transaction-manifest-slice
```

The stack file lives under `.codex/` and is ignored by git.

## Working Model

History should be treated as an authenticated substrate over lineage-local
authority chains, not as the policy itself and not as one global linear chain.

The current startup admission target is:

```text
Startup<Observed>
  -> Startup<Genesis> | Startup<Predecessor>
  -> Startup<Validated>
  -> Parent<Ruling>
```

Before entering the ruling parent path, the runtime should establish:

```text
ProducedBy(SelfRuntime, CurrentArtifact)
AdmittedBy(CurrentArtifact, Lineage, Policy, History)
```

Genesis is a local, configured-store scoped absence claim:

```text
No valid associated authority for this lineage/artifact is present in the
configured History store/root.
```

It is not a global claim that no authority exists anywhere. If the configured
store is unreadable, ambiguous, or inconsistent with the checkout, startup
should reject rather than silently bootstrap.

Predecessor admission should use a sealed History head that names the current
clean artifact tree. Invocation files, ready files, scheduler state, and branch
state remain launch/projection evidence until admitted by History.

## Important Conceptual Split

`TreeKey` does not define lineage. It helps establish the runtime/artifact
identity relation:

```text
ProducedBy(Runtime, Artifact)
```

Lineage is a policy-governed projection over admitted continuity facts in
History:

```text
AdmittedBy(Artifact, Lineage, Policy, History)
```

This distinction matters for selfling, hetero-runtime, and multi-runtime
authoring. A runtime may be produced by one artifact while operating over a
different artifact substrate. Therefore a simple patch chain is too weak as
the long-term lineage model.

The likely durable model is hypergraph-shaped:

```text
artifacts + runtimes + surfaces + interventions + evidence + policy
  -> admitted artifact/head updates
```

Blocks should commit to admitted references and relations, not assume every
input is a local git tree we own.

## Block Contents Direction

The next block-content slice should define enough structure for:

- artifact references and backend tree commitments;
- artifact-local provenance manifest digest;
- minimal explicit policy reference;
- admitted entries as the current implemented unit;
- future admitted transactions/relations over typed references;
- patch composition that also composes authorship and lineage provenance.

Terminology warning: `transaction`, `relation`, `intervention`, `policy`, and
`lineage projection` are not fully defined yet in the current docs/code. The
new comments intentionally mark them as underspecified. Do not promote those
words into public API or durable schema names until their invariants are
written down.

## Artifact-Local Manifest Direction

A produced artifact should eventually carry or reference a provenance manifest
inside the artifact tree. History should admit:

```text
tree_key + manifest_digest
```

The manifest is the natural home for reconstructive evidence:

- parent/runtime identity;
- production provenance;
- intervention refs;
- self-evaluation refs;
- build/runtime refs;
- later validator or consensus attestations.

History should not inline every payload. It should commit to the digest/ref and
record the authority decision that admitted it.

## Authenticated Head Map Direction

The forward-facing store object is an authenticated lineage-head map, likely a
Merkle-Patricia trie or equivalent authenticated map:

```text
LineageId -> HeadState
```

It should eventually support:

- present proof: lineage has head H;
- absent proof: lineage has no head under this committed root;
- accepted head update proof.

Current `FsBlockStore` does not implement this. `heads.json` is only a
rebuildable projection. It is not an authenticated proof, and `head() ->
Option<BlockHash>` is not enough for final startup admission semantics.

## Implemented Versus Intended

Implemented now:

- `GenesisAuthority` and `OpeningAuthority` exist in `history.rs`.
- `Block<Open>` validates genesis versus predecessor shape by lineage-local
  height and parent hashes.
- backend-owned `WorkspaceBackend::TreeKey` and `GitTreeKey` exist.
- `TreeKeyCommitment` allows History to commit a deterministic digest of a
  backend-owned clean tree key.
- `Crown<Locked>::seal` exists as the authority-gated sealing API.
- `Parent<Selectable>::lock_crown` retires the parent and produces
  `Crown<Locked>`.

Not implemented yet:

- live startup gate `Startup<Validated> -> Parent<Ruling>`;
- live append of sealed History block at handoff;
- successor startup verification against sealed History head plus current clean
  tree key;
- live genesis opening/absence validation;
- artifact-local provenance manifest;
- authenticated lineage-head map / Merkle-Patricia store;
- full `PolicyRef` distinct from procedure/runtime-contract assumptions;
- block contents as typed transactions/relations over references;
- patch-composition provenance invariants in code.

## Next Recommended Slice

Start with the documented task:

```text
history-block-transaction-manifest-slice
```

Suggested order:

1. Keep the new `history.rs` and `mod.rs` doc comments as the current
   authoritative direction, but review them once for overclaiming before
   committing.
2. Define a minimal `PolicyRef` without building a policy engine.
3. Define an artifact-local manifest type or sketch, with tree key plus manifest
   digest as the History join point.
4. Define block content types as admitted references/relations, while keeping
   `Entry<Admitted>` as the current implemented unit.
5. Decide whether to add a dependency for a Merkle-Patricia/authenticated map
   now or introduce a narrow adapter trait first.
6. Only then wire the first live slice: gen0/startup validation with configured
   store absence and artifact manifest/tree key validation.

## Documentation Follow-Up

Sub-agents identified these likely update/archive targets:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
  Keep authoritative; update comments as implementation catches up.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
  Keep as module-level operator/concept overview.
- `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md`
  Update. Still useful, but missing startup admission, local-store genesis
  absence, authenticated head map, artifact-local manifest, and minimal
  policy reference.
- `docs/workflow/evalnomicon/drafts/runtime-artifact-lineage.md`
  Likely archive after extracting any remaining artifact-local provenance
  language.
- `docs/workflow/evalnomicon/drafts/prototype1-history-metrics-agent-brief.md`
  Update before using as an agent brief again.
- `docs/reports/prototype1-record-audit/history-admission-map.md`
  Update after block transaction/manifest shape is clearer.

## Caution For Next Session

Avoid turning the new terms into names before the structure exists. Names like
`MergedPatchWithAuthors` or `GenesisTransactionAdmissionRecord` would be a sign
that role/state/policy/provenance structure is being flattened into text.

The invariant to preserve is:

```text
If patch/intervention operations compose, authorship and lineage provenance
must compose with them.
```

The block should make that relation recoverable by committed references and
witnesses, not by a caller-authored status blob.
