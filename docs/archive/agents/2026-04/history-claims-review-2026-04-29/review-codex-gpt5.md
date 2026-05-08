# Prototype 1 History Claims Review

Reviewer: Codex GPT-5
Date: 2026-04-29
Commit reviewed: `0fd55d8d` (`history: constrain block claim construction`)
Scope: static code/docs review only. I did not edit source files or design docs.

## Findings

### High: `Crown<Ruling>` can still be minted inside `prototype1_state` without the claimed predecessor transition

The strongest Crown invariant is documented as "for one lineage, at most one valid typestate carrier may hold `Crown<Ruling>`" in `crates/ploke-eval/src/cli/prototype1_state/history.rs:101` and as a local single-ruler authority claim in `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:287`.

The implementation does use private fields on `Crown<S>` and private state marker fields in `crates/ploke-eval/src/cli/prototype1_state/inner.rs:42`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:59`. However, the constructor is `pub(super)`:

- `Crown<crown::Ruling>::for_lineage(...)` is visible to sibling modules under `prototype1_state` at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:80`.
- `Crown<crown::Ruling>::lock(self)` is `pub(crate)` at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:89`.
- The intended live path uses `Parent<Selectable>::lock_crown(self)` at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:423`, but that method internally calls the same broad constructor at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:430`.

This means any module in `prototype1_state` can mint a fresh `Crown<Ruling>` for an arbitrary lineage string and immediately lock it. The fields prevent construction from outside the module tree, but the typestate invariant is not yet compiler-enforced inside the protocol implementation boundary.

Affected invariant: implemented code does not enforce "at most one valid `Crown<Ruling>` carrier" against all code that can participate in Prototype 1 state transitions. The current implemented invariant is narrower: code outside `prototype1_state` cannot construct `Crown<S>` by struct literal, and the ordinary live helper consumes `Parent<Selectable>` before returning `Crown<Locked>`.

Concrete misuse:

```rust
let crown = inner::Crown::for_lineage("lineage:a").lock();
let sealed = crown.seal(open_block, seal_fields)?;
```

That bypasses `Parent<Selectable>::lock_crown` and the parent retirement transition.

### High: sealing checks Crown lineage, but not the semantic contents of the sealed authority epoch

`Block<Open>::seal` is private to `history.rs` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1734`, and the public crate boundary is `Crown<Locked>::seal` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1916`. This is a real improvement: callers need a `Crown<Locked>` carrier to produce `Block<Sealed>`.

The sealing boundary currently validates only:

- the locked Crown lineage string matches the block lineage string at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1927`;
- the block hash and entries root are internally deterministic and verifiable at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1803`.

It does not validate that `SealBlock` fields are consequences of the Crown-lock transition:

- `crown_lock_transition`, `selected_successor`, `active_artifact`, `claims`, and `sealed_at` are supplied as data in `crates/ploke-eval/src/cli/prototype1_state/history.rs:1440`.
- `Crown<Locked>` carries only `lineage: String` in `crates/ploke-eval/src/cli/prototype1_state/inner.rs:60`; it carries no selected successor, active artifact, policy, block id, or lock evidence.
- Tests seal blocks with `block::Claims::empty_unchecked()` in `crates/ploke-eval/src/cli/prototype1_state/history.rs:2308` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:2372`.

Affected invariant: the code implements "a matching `Crown<Locked>` is required to seal a block", but it does not implement "the sealed header is the projection of the lock transition that selected this successor/artifact." This distinction matters because the docs say a sealed block should prove admitted evidence under authority, not merely that a hash is stable: `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:52`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:151`.

The docs partly acknowledge this gap in `crates/ploke-eval/src/cli/prototype1_state/history.rs:1433`, which says the `crown_lock_transition` reference is still header material and not an authority token. That caveat is accurate and should remain prominent until the Crown lock itself carries the selected successor/artifact commitments.

### High: open block and entry admission are not gated by `Parent<Ruling>` or another authority carrier

The docs say this is not implemented yet in `crates/ploke-eval/src/cli/prototype1_state/history.rs:163`, and the code matches that caveat.

Current implementation:

- `Block<Open>::open(OpenBlock)` is `pub(crate)` and accepts caller-supplied `ruling_authority`, `opened_by`, `policy_ref`, and `opening_authority` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1396` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:1643`.
- `Block<Open>::admit` is `pub(crate)` and accepts any `Entry<Proposed>` plus a caller-supplied `ActorRef` as admitting authority at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1692`.
- `Entry<Draft> -> Entry<Observed> -> Entry<Proposed>` transitions are public within the crate at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1324`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1357`.

The admitted entry state itself is well protected: `Entry<Admitted>` has private fields and is only produced by `Block<Open>::admit`. The gap is that the open block is not itself owned by a `Parent<Ruling>` or similar carrier, so any crate-local caller that can get an open block can admit proposed entries under arbitrary actor labels.

Affected invariant: the implementation encodes "admitted entries are block-local and hash into the sealed block"; it does not encode "only the live ruling parent can write open block entries." This is documented-but-not-implemented, not a hidden overclaim, but it is still a correctness gap before relying on History for live authority.

Concrete misuse:

```rust
let mut block = Block::open(fields_with_arbitrary_ruling_authority)?;
let entry = Entry::draft(draft).observe(obs).propose(proposal);
block.admit(entry, ActorRef::Process("not-the-ruler".into()))?;
```

### Medium: opening authority and head advancement are records, not verified store transitions

`OpeningAuthority` distinguishes genesis and predecessor cases at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1091`, and `Block<Open>::open` validates the local shape:

- genesis height must be zero and parentless at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1648`;
- non-genesis must have parent hashes at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1652`;
- predecessor authority must cite one listed parent hash at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1661`.

That is useful, but it is not the same as a verified History-store transition:

- `GenesisAuthority::new` is `pub(crate)` and records bootstrap material without an absence proof at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1063`.
- `PredecessorAuthority::new` is `pub(crate)` and records a predecessor hash without proving it is the configured store head at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1083`.
- `FsBlockStore::append` verifies only the sealed block hash, appends it, then overwrites `heads.json` for the lineage at `crates/ploke-eval/src/cli/prototype1_state/history.rs:541` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:568`.
- There is no compare-and-swap or "current head must equal predecessor" check before `heads.insert(...)`.

Affected invariant: implemented code provides a rebuildable local projection over appended sealed blocks. It does not implement authenticated head-map proofs, configured-store genesis absence, or atomic accepted-head update. The docs correctly identify this limitation in `crates/ploke-eval/src/cli/prototype1_state/history.rs:414`, `docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:144`, and `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:39`.

Concrete misuse: append two different sealed genesis blocks for the same lineage. Both can be internally hash-valid; `heads.json` will point at the later append, and the store will not record that the second append violated the intended genesis absence/predecessor policy.

### Medium: the `Locator<T>` boundary is fallible, but not yet tied to a backend-owned artifact/tree authority

The new claim structure is:

```text
claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<T, L>>>
```

This is structurally better than a caller-authored report. The fallible boundary is `Locator<T>` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:719`, and `Verifiable::from_locator` calls `locate` and `digest` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:771`. `Crown<Ruling>::admit_claim` uses that path at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1888`.

The remaining gap is that `Locator<T>` is an unsealed crate-local trait. It does not carry:

- the current `ArtifactRef`;
- a backend `TreeKey`;
- a sealed head/hash expected by History;
- a store scope or policy scope.

So a crate-local caller can implement a locator that returns in-memory data and a matching digest without crossing the intended filesystem/tree/backend boundary. `Verifiable::verify_with` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:785` verifies consistency against the supplied locator, not against an authenticated artifact context.

Affected invariant: implemented code quarantines failures through a fallible trait call. It does not prove that the locator is the configured backend/tree for the active artifact. The docs should continue to describe backend/tree recovery as intended rather than fully implemented. `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:223` and `docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:123` already frame artifact manifests and tree-key admission as future work.

### Low: `Digest<T>` and loaded claim recovery remain future-sensitive

`Digest<T>` has a private constructor at `crates/ploke-eval/src/cli/prototype1_state/history.rs:706`, so ordinary code cannot directly pair arbitrary hashes with a target type. However, it derives `Deserialize` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:698`. That is not currently fatal because:

- `block::Claims` derives `Serialize`, not `Deserialize`, at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1476`;
- `Block<S>`, `Entry<S>`, and the advanced state carriers intentionally do not derive `Deserialize`;
- `FlatClaim` is private to `history.rs` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:925`.

The future risk is that adding a generic "load block from JSON" path by deriving `Deserialize` on `Claims`, `FlatClaim`, or `Block<Sealed>` would let serialized flat fields be rehydrated into `claim::Admitted` through `FlatClaim::to_admitted` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:951` before crossing a verification transition.

Affected invariant: current implementation is safe enough here, but only because deserialization stops before authoritative carriers. A future loading API must be a named `Verified` transition that recomputes block hashes and verifies each required claim against the appropriate locator/context.

### Low: names still carry some missing structure

The commit improves structure around `Claims`, `Verifiable`, `Witnessed`, and `claim::Admitted`, but several names still indicate deferred structure:

- `ProcedureRef` is used as both procedure/runtime-contract and policy reference in `OpenBlock::policy_ref` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1412`, while docs explicitly say policy should become distinct from procedure environment in `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:133`.
- `Crown` stores lineage as a `String` in `crates/ploke-eval/src/cli/prototype1_state/inner.rs:60`, while History uses `LineageId` in `crates/ploke-eval/src/cli/prototype1_state/history.rs:386`. The type system cannot distinguish campaign id, lineage id, branch id, or arbitrary label at the Crown boundary.
- `GenesisAuthority` records bootstrap material, but there is no `Genesis<AbsentHeadVerified>` or equivalent state carrier. The configured-store absence proof is still prose and policy, not a type.
- `BlockStore::head -> Option<BlockHash>` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:432` collapses "absent under a verified store root" into ordinary optional data. The docs already call this insufficient in `crates/ploke-eval/src/cli/prototype1_state/history.rs:414`.

These are mostly documented-but-not-implemented invariants, not contradictions. The next patch should avoid adding longer names such as `GenesisAbsenceAdmissionRecord` to compensate; the missing object is a typed startup/opening transition and an authenticated head proof.

## Claims Correctly Encoded

- `block::Claims` is no longer a simple public report/DTO. It has private fields, no public constructor, no `Default`, and no `Deserialize` derive at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1476`.
- `Claims` stores flat serialized fields but accepts and returns the nested admitted/witnessed/verifiable shape at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1492`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1503`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1512`, and `crates/ploke-eval/src/cli/prototype1_state/history.rs:1543`.
- `claim::Admitted<A, X>` has private-to-parent fields and no public crate constructor at `crates/ploke-eval/src/cli/prototype1_state/history.rs:858`.
- `Verifiable<T, L>` fields are private, and the normal construction path calls `Locator::locate` and `Locator::digest` before producing the carrier at `crates/ploke-eval/src/cli/prototype1_state/history.rs:771`.
- `Block<Open>::seal` is private to `history.rs`; crate callers seal through `Crown<Locked>::seal` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1734` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:1916`.
- `Crown<Locked>::seal` consumes the locked Crown and rejects lineage mismatch at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1922`.
- `Block<Sealed>::verify_hash` recomputes entry count, entries root, and block hash from the sealed contents at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1803`.
- `OpenBlock` enforces the basic genesis/predecessor shape, including "genesis has no parents" and "predecessor authority cites a listed parent hash" at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1648`.
- `Ingress<Open>::import` preserves original observation custody plus import policy and target block coordinate in its payload at `crates/ploke-eval/src/cli/prototype1_state/history.rs:2008`.

## Docs Alignment

The current docs are mostly aligned with the implementation if read carefully.

Implemented invariant:

- local, deterministic hashing and verification of sealed blocks;
- private advanced state fields and no direct deserialization of authoritative block/entry carriers;
- `Crown<Locked>` required at the API boundary for sealing;
- `block::Claims` construction constrained to nested admitted/witnessed/verifiable carriers inside `history.rs`.

Documented-but-not-implemented invariant:

- live startup admission through `Startup<Validated> -> Parent<Ruling>` is explicitly not implemented in `crates/ploke-eval/src/cli/prototype1_state/history.rs:52`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:81`, and `docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:177`.
- live append of the sealed History block at handoff is not implemented, as stated in `docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:177` and `crates/ploke-eval/src/cli/prototype1_state/mod.rs:658`.
- genesis absence under a configured store is a target invariant, not a current proof, as stated in `crates/ploke-eval/src/cli/prototype1_state/history.rs:80` and `docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:60`.
- successor startup verification against the sealed head and current clean tree key is not implemented, as stated in `crates/ploke-eval/src/cli/prototype1_state/history.rs:254` and `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:91`.
- `Parent<Ruling>` as the only writer of open block entries is explicitly not enforced yet in `crates/ploke-eval/src/cli/prototype1_state/history.rs:163`.

Potential future invariant:

- distributed consensus, cryptographic signatures, remote witnesses, process uniqueness, authenticated Merkle-style head maps, finality, rollback/fork policy, artifact-local manifests, stochastic evidence roots, and validator/reputation semantics are all future work. The docs generally label them that way in `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:39`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:118`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:285`, and `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:82`.

Docs issue:

- `docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:7` still names `2851650e` as the latest committed checkpoint. That was true for the handoff note, but after `0fd55d8d` it is stale. It does not create an invariant overclaim, but it should be updated or marked historical before being reused as an agent brief.
- `docs/workflow/evalnomicon/drafts/runtime-artifact-lineage.md:1` reads like raw planning notes and has no status block. Its artifact/runtime lineage ideas are consistent with the newer docs, but it should not be treated as an implementation claim.

## Recommended Next Patch

1. Narrow Crown construction. Make `Crown::for_lineage` inaccessible to sibling modules, or replace it with a constructor that requires a `Parent<Selectable>` or future `Startup<Validated>` carrier. Keep test-only constructors under `#[cfg(test)]`.
2. Make `Crown<Locked>` carry the lock payload structurally: lineage id, selected successor, active artifact/tree commitment, block id/height, policy/surface, and lock transition evidence. Then `seal` can project those fields rather than accepting them as an unrelated `SealBlock` DTO.
3. Gate `Block<Open>::open` behind typed opening authorities: `Genesis<AbsentHeadVerified>` and `Predecessor<HeadVerified>` or equivalent. Keep raw constructors private/test-only.
4. Gate `Block<Open>::admit` behind a ruler/admission carrier instead of accepting `ActorRef` labels as authority data. At minimum, thread a `Parent<Ruling>` or `Crown<Ruling>` capability into admission.
5. Replace `BlockStore::head -> Option<BlockHash>` with an explicit head proof/read model that distinguishes absent proof, present proof, ambiguous/corrupt store, and IO failure. Add append validation that checks predecessor/current-head transition before updating the head projection.
6. Keep `block::Claims` non-deserializable until there is a named load path such as `BlockBytes -> Block<Verified>`, which recomputes the block hash and verifies each required claim against the correct artifact/tree locator.
7. Split `ProcedureRef` from a minimal `PolicyRef`/`PolicyScope` rooted in `Surface`, and use `LineageId` rather than `String` at the Crown boundary.

## Bottom Line

Commit `0fd55d8d` materially improves `block::Claims`: it no longer behaves like a forgeable public DTO, and the nested claim shape is a real structural improvement. The remaining correctness risk is one layer higher. Crown creation, block opening, entry admission, and store head advancement are still broad crate-local APIs or data records rather than fully enforced authority transitions. The docs mostly state those gaps accurately, but the code should not yet be described as enforcing live History/Crown authority beyond local sealed-block hashing plus Crown-gated sealing by lineage.
