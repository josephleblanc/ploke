# History Claims Audit: commit `0fd55d8d`

Reviewed commit: `0fd55d8dcf74ba1848c35cbb79a42bb3c84785c9` (`history: constrain block claim construction`)

Scope: `history.rs`, `inner.rs`, `mod.rs`, and the listed Evalnomicon History/Crown drafts. This report distinguishes implemented invariants from documented-but-not-implemented and potential future invariants.

## Findings

### 1. High: `Crown<Ruling>` uniqueness is claimed, but ruling Crowns can still be minted by sibling modules

Affected invariant: for one lineage, at most one valid typestate carrier may hold `Crown<Ruling>`.

The invariant is stated in the code docs at `crates/ploke-eval/src/cli/prototype1_state/history.rs:101` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:104`, and repeated in the v2 doc as the current local Crown claim at `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:68` and `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:291`.

The implementation does encode private Crown fields and move-only locking: `Crown<S>` stores private state at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:60`, and `Crown::lock(self)` consumes the ruling Crown at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:89`. However, `Crown<crown::Ruling>::for_lineage` is `pub(super)` at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:80`, so any sibling module under `prototype1_state` can mint a fresh `Crown<Ruling>` for an existing lineage. The live path uses this in `Parent<Selectable>::lock_crown` at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:423` and `crates/ploke-eval/src/cli/prototype1_state/parent.rs:430`, but the constructor is not limited to that transition.

Misuse example: a developer in another `prototype1_state` child module can call `Crown::for_lineage("lineage:a")` twice. Both values are valid Rust carriers. Either can call `admit_claim` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1888`, or be moved through `lock()` and `seal()` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1922`. That violates the stated uniqueness invariant even though each individual transition is typed.

Status: documented as stronger than implemented. The code implements private fields and move-only locking, but not uniqueness of Crown minting.

### 2. High: `block::Claims` is no longer a public DTO, but claims are not bound to the block epoch that seals them

Affected invariant: `block::Claims` should be admitted evidence for this block, under this ruling authority and policy, not an assembled report with compatible-looking fields.

The commit improves `block::Claims` substantially. It has private fields, no `Deserialize`, no `Default`, no public constructor, and stores flat fields only behind nested semantic accessors. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:1458`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1470`, and `crates/ploke-eval/src/cli/prototype1_state/history.rs:1476`. The `with_*` setters are limited to the parent `history` module at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1492`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1512`, and `crates/ploke-eval/src/cli/prototype1_state/history.rs:1532`.

The remaining problem is relational. `RulerWitness` and `Admission` carry actor, environment, policy, and time, but they do not carry lineage, block id, block height, or a typed relation to the `Block<Open>` being sealed (`crates/ploke-eval/src/cli/prototype1_state/history.rs:883`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:901`). `FlatClaim::from_admitted` stores only key, digest, witness, and admission (`crates/ploke-eval/src/cli/prototype1_state/history.rs:933`). `SealBlock` accepts an already-built `block::Claims` field bag at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1440`, and `Crown<Locked>::seal` only checks locked Crown lineage against block lineage at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1922` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:1927`.

Misuse example: inside `history.rs`, a helper can admit a policy claim with a `Crown<Ruling>` conceptually belonging to another lineage or actor, place it into `Claims`, then seal a block for `lineage:a` with a matching `Crown<Locked>`. The block hash verifies because the mismatched claim is committed as data. The type system does not encode "this claim was admitted for this block epoch".

Status: partially implemented invariant. `Claims` is no longer a crate-wide forgeable report/DTO, but same-epoch claim admission remains documented aspiration rather than enforced structure.

### 3. Medium: block opening and entry admission are callable without `Parent<Ruling>` or verified predecessor authority

Affected invariant: entries should be written while the runtime has ruling authority; non-genesis blocks should be opened from a verified sealed predecessor; genesis should be a configured-store absence claim.

`Block<Open>::open` validates the local shape of genesis and predecessor blocks: genesis height must be zero with no parents, non-genesis must have parents, bootstrap authority cannot open a child, and predecessor authority must cite one listed parent hash (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1643`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1649`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1655`). But `PredecessorAuthority::new` accepts a bare `BlockHash` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1077`, and `OpenBlock` is a crate-visible field bag at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1403`. A caller can therefore open a non-genesis block from an arbitrary hash that is merely repeated in `parent_block_hashes`.

Likewise, `Block<Open>::admit` takes `&mut self`, an `Entry<Proposed>`, and an `ActorRef` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1692`. It does not require `Parent<Ruling>`, `Crown<Ruling>`, or a policy carrier. It does bind ingress imports to the target block (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1697`), records `ruling_authority` from `BlockCommon` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1725`), and rejects duplicate entry ids (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1709`), but admission authority is still caller-supplied data.

The implementation docs correctly warn that live `Parent<Ruling>` as the only writer, live append, successor verification, and ingress capture are not yet enforced at `crates/ploke-eval/src/cli/prototype1_state/history.rs:163` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:165`. The handoff doc says the same for live startup, append, successor verification, and genesis absence at `docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:177`.

Status: documented-but-not-implemented invariant, with a crate-visible API surface that can be misused.

### 4. Medium: `FsBlockStore` verifies block content but not lineage-head transition validity

Affected invariant: History head should be derived from accepted sealed blocks and should support startup admission under a configured store scope.

`FsBlockStore::append` calls `block.verify_hash()` before writing at `crates/ploke-eval/src/cli/prototype1_state/history.rs:541`, which protects the block's internal hash commitment. It then appends the block and indexes and unconditionally overwrites the lineage head in `heads.json` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:557` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:568`.

There is no compare-and-swap, no check that the appended block height advances the previous head, and no check that the block's parent hashes include the current stored head. A stale or parallel local writer can append a valid sealed block and move the projection head. The code comments explicitly acknowledge this limitation: `head` and `heads.json` are not authenticated inclusion/absence proofs and `append` does not validate a CAS-style lineage-head transition (`crates/ploke-eval/src/cli/prototype1_state/history.rs:414`). The handoff doc also calls `heads.json` a rebuildable projection rather than an authenticated proof at `docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:159`.

Status: correctly documented as not implemented. It remains a concrete invariant gap before startup admission can rely on the store.

### 5. Low: deserializable digest/ref wrappers are safe only while authoritative carriers remain non-deserializable

Affected invariant: fallible filesystem/tree/backend boundaries should be quarantined in `Locator<T>` and verified loading, not bypassed by deserializing authoritative facts.

`Digest<T>` derives `Deserialize` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:698`, as do `HistoryHash`, `BlockHash`, and several reference wrappers at `crates/ploke-eval/src/cli/prototype1_state/history.rs:318`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:348`, and `crates/ploke-eval/src/cli/prototype1_state/history.rs:662`. This does not currently forge a sealed block because `Block<S>`, `block::Claims`, `SealedBlockHeader`, `Entry<S>`, and the state carriers intentionally do not derive `Deserialize` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:225`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1476`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1568`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1637`).

The future risk is API drift: if a later constructor accepts a deserialized `Digest<T>` or `BlockHash` as proof instead of requiring `Locator<T>::locate` and `Locator<T>::digest`, the fallible boundary can be bypassed. The relevant locator boundary is at `crates/ploke-eval/src/cli/prototype1_state/history.rs:732` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:771`. The current code keeps that boundary mostly intact by making `Verifiable::new` private and exposing `verify_with` for recovery at `crates/ploke-eval/src/cli/prototype1_state/history.rs:762` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:785`.

Status: implemented invariant currently holds, but only while verified loading remains a separate transition and future APIs do not treat deserialized digests as authority.

## Claims Correctly Encoded

- `Crown<S>` and its state markers have private fields, and `Crown::lock(self)` is move-only (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:43`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:60`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:89`).
- `Crown<Locked>::seal` is the crate-facing route from `Block<Open>` to `Block<Sealed>` and rejects mismatched Crown/block lineage (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1916`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1927`).
- `Block<Open>` and `Block<Sealed>` fields are private through their state payloads; authoritative carriers serialize but do not deserialize (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1553`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1560`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1637`).
- Sealed block hashes commit to common header fields, Crown lock transition ref, selected successor, active artifact, block claims, sealed time, entry count, and entries root (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1734`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1742`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1752`).
- `verify_hash` recomputes entry count, entries root, and block hash from stored contents (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1803`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1812`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1823`).
- `block::Claims` is constrained relative to the pre-commit risk: no public constructor, no `Default`, no `Deserialize`, private flat fields, and nested `claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<T, L>>>` extraction (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1470`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1476`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1503`).
- Ingress import preserves original observation custody and is checked against the target block during admission (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1998`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2014`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1697`).

## Docs Alignment

The strongest docs are aligned with the implementation limits:

- `history.rs` says live handoff locks a lineage-bound Crown but does not yet seal or persist a block (`crates/ploke-eval/src/cli/prototype1_state/history.rs:6`).
- `history.rs` explicitly lists current enforcement and non-enforcement (`crates/ploke-eval/src/cli/prototype1_state/history.rs:155`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:163`).
- `mod.rs` describes History as a tamper-evident local model, not consensus or judgment proof (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:77`).
- `history-blocks-v2.md` marks startup admission, consensus, authenticated head-map proofs, and full policy/finality semantics as not implemented (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:39`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:83`).
- `prototype1-history-handoff-2026-04-29.md` gives a clear implemented-versus-intended list (`docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:163`).
- `history-blocks-and-crown-authority.md` is labeled older background and says conflicts should resolve in favor of `history.rs` and `history-blocks-v2.md` (`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:3`).

Two claims should be read narrowly:

- "At most one valid typestate carrier may hold `Crown<Ruling>`" is presented as a core invariant (`crates/ploke-eval/src/cli/prototype1_state/history.rs:101`), but current constructor visibility allows duplicate carriers inside `prototype1_state`.
- The block proof language "these claims/evidence were admitted by this authority under this policy" (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:52`) is directionally right, but current `block::Claims` does not encode same-block or same-lineage admission.

`runtime-artifact-lineage.md` and `formal-procedure-notation.md` are conceptual drafts. They support the current direction around artifact/runtime provenance and typed procedure states, but they should not be treated as implemented API claims.

## Recommended Next Patch

1. Make `Crown<Ruling>` minting structurally exclusive. Move `for_lineage` behind a narrower bootstrap/startup or `Parent<Ruling>` transition boundary, or replace it with constructors that consume a `Startup<Validated>` or `BootstrapPolicy<ValidatedAbsence>` carrier.
2. Tie block claims to an epoch. A practical next shape is a `ClaimsBuilder<'block>` or `Block<Open>::admit_claim(...)` method that requires the open block and `Crown<Ruling>` together, records `lineage_id`, `block_id`, `block_height`, `ruling_authority`, and `policy_ref`, and only yields `block::Claims` for that block.
3. Gate `Block<Open>::admit` behind a ruling authority carrier instead of accepting an arbitrary `ActorRef`.
4. Replace `PredecessorAuthority::new(BlockHash)` in crate-facing flows with construction from a verified `Block<Sealed>` or verified store head. Keep the bare constructor test-only or private.
5. Add a store-head transition API before live startup admission: append should verify expected prior head, block height, and parent hash relationship before updating the head projection.
6. Keep `Deserialize` off authoritative carriers. When block loading is added, make it a `Loaded<Unverified> -> Verified<Block<Sealed>>` transition rather than a derive.

## Verification

Ran:

```text
cargo test -p ploke-eval prototype1_state::history --locked
```

Result: passed. The run executed 19 matching tests successfully, with existing warnings.
