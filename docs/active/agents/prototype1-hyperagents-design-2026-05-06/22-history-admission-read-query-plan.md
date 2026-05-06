# Agent 22: History Admission Read Query Plan

Date: 2026-05-06

Scope:
- Prior reports read: `14-history-evidence-access-operator-census.md`, `16-artifact-tree-backend-locator-census.md`, and `18-existing-type-expansion-recommendation.md`.
- Source inspected: `crates/ploke-eval/src/cli/prototype1_state/history.rs`, `history_preview.rs`, `inner.rs`, and `parent.rs`.
- Source code was not modified.

## Executive Plan

The later authority-side step should be a verified History read/query path plus an explicit Crown admission step. It should not be implemented inside the read-only child evidence operator.

The read-only operator should select and group child evidence bundles from `history_preview::FsEvidenceStore` as degraded or projection evidence. The authority step should then cite only the selected subset through the existing History algebra:

```text
selected child evidence bundle
-> Locator<T>
-> Verifiable<T, L>
-> Witnessed<RulerWitness, _>
-> claim::Admitted<Admission, _>
-> block::Claims and/or Entry<Admitted>
-> Block<Sealed>
-> FsBlockStore append
-> verified FsBlockStore reads/queries for later selection
```

That keeps selection, citation, verification, witnessing, admission, sealing, and later query as separate operations.

## Existing Authority Shape

`history::BlockStore` is the authority store port. Today it has only `append(expected, block)` and `lineage_state(lineage)`. `FsBlockStore::append` verifies the sealed block hash, checks the expected `LineageState`, verifies append against the absent/present `StoreHead`, writes the block stream and indexes, then updates `heads.json`. `heads.json` is a projection, not authority.

`FsBlockStore::sealed_head_block(head)` is the only concrete sealed-block read. It intentionally rejects stored blocks with entries because entry loading does not yet have its own verified transition. This is the immediate blocker for archive-side queries over admitted child evidence: a sealed block can contain `Entry<Admitted>`, but the filesystem store cannot yet load non-empty sealed blocks as verified authority.

The current startup path is also read/query-shaped but narrow. `Startup<Predecessor>::from_history` reads `LineageState`, loads the sealed head block, derives the current checkout tree key through `GitWorktreeBackend.clean_tree_key`, verifies the sealed artifact claim through `ArtifactLocator`, recomputes the current `SurfaceCommitment`, and only then yields `Startup<Validated>`. This is the model to copy: read from History, reconstruct current facts through the backend, verify through typed boundaries, then enter a stronger role/state.

## Query Path To Add Later

The authority-side query path should extend `BlockStore`/`FsBlockStore`, not `history_preview::EvidenceStore`.

Concrete read operations needed:

- `block_head(lineage, height)` or equivalent lineage-height lookup, returning a `BlockHead`.
- `sealed_block(head)` or `sealed_block(hash)`, loading and verifying the full `Block<block::Sealed>`, including entries.
- `lineage_blocks(lineage, range)` as an iterator/query over verified sealed blocks, built from the lineage-height index.
- Claim accessors on loaded sealed blocks that preserve the existing nested claim reconstruction: `block::Claims::{policy,surface,manifest,artifact}`.
- Entry access over `Entry<Admitted>` with enough stable projections for selection: entry id, kind, subject, input/output refs, payload ref/hash, observer/recorder/proposer/admitting authority, lineage id, block id, and height.

The loader should remain a transition, not `Deserialize` on authority carriers. For non-empty blocks, it must reconstruct `Entry<Admitted>`, verify each entry hash, recompute `entries_root`, check `entry_count`, and verify `block_hash`. Query results should carry the store-derived cursor (`StoredBlock`, `BlockHead`, location, or equivalent) so later reports can cite exactly which sealed block supplied the fact.

## Admission Shape For Selected Child Evidence

The selected bundle should first be a read-only evidence grouping. Admission should then split facts by semantic role:

- Artifact eligibility belongs in `block::Claims::artifact` through `ArtifactLocator`, `TreeKeyHash`, `Verifiable<Artifact, ArtifactLocator>`, `Witnessed<RulerWitness, _>`, and `claim::Admitted<Admission, _>`.
- Artifact-local manifest evidence should use the existing `Manifest` marker and the `block::Claims::manifest` slot once a real `Locator<Manifest>` exists.
- Bounded surface evidence should use `surface::Bounded` and the `block::Claims::surface` slot once a real bounded-surface locator exists.
- Policy/procedure material should use `Policy` and `block::Claims::policy` only when the material is located and digested through a `Locator<Policy>`.
- Child evaluation facts, judgments, decisions, observations, or transition evidence should become `Entry<Draft> -> Entry<Observed> -> Entry<Proposed> -> Entry<Admitted>` under `Crown<Ruling>::admit_entry`.
- Evidence that arrives after the prior block was sealed should use `Ingress<Open>::observe_late -> Ingress<Imported> -> Entry<Proposed> -> Entry<Admitted>`, preserving the prior block hash and import disposition.

The read-only bundle may supply `EvidenceRef` values derived from `EvidencePointer.ref_id()` and hashes derived from `EvidencePointer.hash()`. Those are citations. They do not become authority until admitted by the Crown and sealed into a block.

## Locator Plan

Do not turn `EvidencePointer` or `ArtifactRef` into recovery capabilities. Add or specialize `Locator<T>` implementations for authority objects that must be recovered and digest-checked:

- `Locator<Artifact>` already exists as `ArtifactLocator` over `TreeKeyHash`.
- Add a manifest locator for `Manifest` with `Key = ArtifactPath` only when the manifest is committed by the artifact tree or another accepted content-addressed context.
- Add bounded-surface locators for `surface::Bounded` only when the locator can recover the material from the admitted artifact/surface context and compute the same digest.
- If selected child evidence becomes a single bundle root, model it as a real target type, for example an artifact-local evaluation bundle target, with its own locator. Do not encode that as a generic string claim.

Admission should call `Crown<Ruling>::admit_claim(locator, key, ruler, environment, policy, at)`. The fallible locate/digest boundary belongs there: missing evidence, stale paths, digest mismatch, or backend disagreement must fail before the claim enters `block::Claims`.

## Entry Plan

Use entries for facts about child behavior and selection, not for block header commitments.

Good entry subjects are structural coordinates such as a child runtime, candidate artifact, branch/node coordinate, evaluation run, successor decision, or imported late observation. Keep the object structure in `SubjectRef`, `input_refs`, `output_refs`, and payload citations instead of inventing flattened event kinds such as `ChildEvidenceBundleAdmitted`.

Recommended entry mapping:

- `EntryKind::Observation`: raw child result, terminal status, provider/runtime health, or late evidence import.
- `EntryKind::ProcedureRun`: evaluation procedure execution, protocol aggregation, or benchmark run.
- `EntryKind::Judgment`: evaluator conclusion, score/rubric finding, or uncertainty summary.
- `EntryKind::Decision`: selected successor or archive parent recommendation when policy records it.
- `EntryKind::Transition`: parent/child role-state transition evidence normalized from older journal rows.
- `EntryKind::Projection`: report or metric output, only when the projection itself is worth preserving as a projection.

For every admitted entry, `payload_ref` should cite the selected evidence document or bundle root, and `payload_hash` should match the read-only evidence hash or a locator-derived digest. `input_refs` should cite the source files/blocks used by the fact; `output_refs` should cite resulting artifacts, run records, or decisions.

## What The Read-Only Evidence Operator Must Not Implement

The read-only operator must not:

- call `Crown<Ruling>::admit_claim`, `Crown<Ruling>::admit_entry`, `open_block`, `seal`, or `FsBlockStore::append`;
- construct `block::Claims`, `Entry<Admitted>`, `Admission`, or `RulerWitness`;
- advance `heads.json`, write `blocks/segment-*.jsonl`, or update History indexes;
- treat `EvidencePointer`, scheduler rows, branch registry rows, node records, runner results, reports, or metrics as sealed facts;
- derive parent authority, Crown authority, or successor eligibility from a file path, branch id, runtime id, generation number, or report score;
- deserialize authority carriers directly as a shortcut around verified load transitions;
- collapse "selected for consideration" into "admitted into History";
- mint broad `EvidenceRef` strings as if they were typed recovery capabilities.

Its correct output is a grouped evidence projection with source refs, hashes, classes, recovered child/runtime/branch coordinates when available, and an explicit authority status such as projection, degraded evidence, backend-verified, sealed, or sealed ingress.

## Concrete Later Slice

1. Extend the History read API.
   Add verified `FsBlockStore` reads for sealed blocks by head/hash and lineage-height range. Make non-empty block loading reconstruct and verify `Entry<Admitted>` rather than rejecting it.

2. Add query result carriers.
   Return block cursors plus verified headers, claims, entries, and verification status. Keep this under `history.rs`/store boundaries.

3. Keep child bundle discovery in `history_preview`.
   Expand `FsEvidenceStore` to group child evidence, but label it as degraded/projection unless it is backed by a sealed block query.

4. Define locators for admissible bundle roots.
   Start with manifest and bounded-surface locators if the bundle is artifact-local. Only add a child-evidence target type when there is a real recoverable object and digest rule.

5. Admit selected facts through the Crown.
   Header-level artifact/manifest/surface/policy commitments go into `block::Claims`; child observations, evaluations, judgments, decisions, and late evidence imports go into `Entry<Admitted>`.

6. Query admitted evidence for archive selection.
   Archive selection should read verified sealed blocks and entries through `FsBlockStore` queries, then join them with read-only projections for still-unadmitted context. The selector must know which facts are sealed and which are only evidence candidates.

## Main Gaps

- `FsBlockStore::sealed_head_block` cannot yet load non-empty sealed blocks.
- `BlockStore` has no query/range interface.
- `block::Claims` has slots for policy, surface, manifest, and artifact, but live population is currently narrow.
- `Locator<Manifest>`, `Locator<surface::Bounded>`, and any future child-evidence bundle locator do not exist yet.
- `Parent<Ruling>` is still not the structural supplier of all actor identity fields; `Crown<Ruling>::admit_claim` still accepts ruler identity as data.
- The read-only evidence layer can assemble useful child bundles now, but it must remain below the authority boundary until the Crown admission step exists.
