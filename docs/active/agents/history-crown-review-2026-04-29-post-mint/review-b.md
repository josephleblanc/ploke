# Findings

## High: The live ruling-parent gate still does not enforce sealed-head successor admission

The commit narrows the in-process Crown constructor boundary, but it does not implement the current local claim that only a runtime whose current Artifact matches the successor Artifact committed by the sealed History head may enter the ruling parent path.

Evidence:

- `spawn_and_handoff_prototype1_successor` locks the Crown at `crates/ploke-eval/src/cli/prototype1_process.rs:862` and `crates/ploke-eval/src/cli/prototype1_process.rs:876`, then only logs `locked_crown.lineage()` at `crates/ploke-eval/src/cli/prototype1_process.rs:877`. The carrier is not passed into block sealing or persistence before successor spawn.
- The successor is admitted by invocation/identity checks and a ready file path, not by verifying a sealed History head: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3050`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3110`, and `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3122`.
- Parent startup checks checkout cleanliness, branch/commit-message identity, node generation/branch, and selected instance at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:347`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:353`, and `crates/ploke-eval/src/cli/prototype1_state/parent.rs:369`; it does not consult `BlockStore` or a sealed block.
- The docs correctly call this out as unimplemented: `crates/ploke-eval/src/cli/prototype1_state/history.rs:117`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:121`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:155`, and `crates/ploke-eval/src/cli/prototype1_state/history.rs:163`; also `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:83` and `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:91`.

Impact: the implementation currently proves "the live handoff path can move a `Parent<Selectable>` into `Parent<Retired>` and produce a lineage-bound `Crown<Locked>`." It does not prove "the successor runtime's current Artifact matches the selected successor committed by the sealed History head."

Next patch: wire `Parent<Selectable>` plus selected successor evidence into an atomic authority transition that seals and appends a `Block<Sealed>` before successor launch; then make successor startup derive the clean tree key and verify it against the current sealed head before `parent.ready()`.

## Medium: `LockCrown` exposes a lock-only transition that can discard the authority record

`LockCrown` is implemented in `inner` to keep raw `Crown<Ruling>` construction private, which is the right direction. The semantic shape is still too weak: it returns `(Parent<Retired>, Crown<Locked>)` without forcing the durable History projection of that transition.

Evidence:

- `LockCrown` is a crate-visible trait at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:141`, and its only production implementation returns a naked locked carrier at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:150`.
- The live caller immediately drops that carrier after logging: `crates/ploke-eval/src/cli/prototype1_process.rs:876` through `crates/ploke-eval/src/cli/prototype1_process.rs:883`.
- Sealing itself has the stronger API shape: `Block<Open>::seal` is private at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1734`, while the crate-visible sealing boundary requires `Crown<Locked>` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1916` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:1922`.
- The comment above `SealBlock` records the same gap: live handoff needs to pass the carrier into sealing and persist the sealed block before successor admission can verify it, at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1431` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:1438`.

Impact: durable History records are still not guaranteed to be projections of the allowed typed transition. The lock transition can happen without a sealed block, and the successor handoff journal entry remains caller-assembled evidence at `crates/ploke-eval/src/cli/prototype1_process.rs:943`.

Next patch: replace the public lock-only shape with a transition carrier that consumes `Parent<Selectable>`, the open block, selected successor Artifact, active Artifact commitment, policy/evidence refs, and store handle, then returns `Parent<Retired>` plus the stored `Block<Sealed>` or a typed handoff object. Keep the raw lock method module-private.

## Medium: `LineageKey` preserves the minting boundary but is not a real lineage authority key

`LineageKey` helps keep Crown code from depending directly on `String`, and because `Crown<Ruling>::for_lineage` is private to `inner`, it does not reopen external `Crown<Ruling>` minting. But the current type is still a debug-string wrapper, not an authenticated lineage/store coordinate.

Evidence:

- `LineageKey` stores a plain `String` at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:73`, is constructed by `from_debug_value` at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:83`, and compares to block lineage strings through `matches_debug_str` at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:94`.
- The live lineage is derived from `identity.campaign_id` at `crates/ploke-eval/src/cli/prototype1_state/parent.rs:423`, while the block API uses `LineageId` in `OpenBlock` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1404`.
- The stronger intended admission rule includes configured History surface, lineage coordinate, policy, current checkout tree key, and sealed head expected successor at `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:106` through `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:116`.

Impact: `LineageKey` preserves the constructor boundary in this commit, but it should not be cited as proof of lineage authenticity or store-scoped authority. It is a placeholder identity wrapper.

Next patch: either rename/comment it as a debug lineage coordinate until the real key exists, or replace it with a carrier that includes the configured History store/surface and canonical `LineageId`. Avoid APIs that compare it to arbitrary strings outside a narrow compatibility boundary.

# Implemented Guarantees

- Production code can no longer mint `Crown<Ruling>` from a raw lineage string outside `inner`: `Crown<crown::Ruling>::for_lineage` is private at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:117`; the test-only constructor is gated at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:167`.
- `Crown` fields and crown state marker fields are private: `crates/ploke-eval/src/cli/prototype1_state/inner.rs:42` and `crates/ploke-eval/src/cli/prototype1_state/inner.rs:60`.
- Locking is move-only for the Crown carrier: `lock(self)` consumes `Crown<Ruling>` at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:126`.
- The parent retirement transition is only implemented for `Parent<Selectable>`: `crates/ploke-eval/src/cli/prototype1_state/inner.rs:147`.
- Sealing a block through the crate-visible API requires a `Crown<Locked>` and checks the lineage before calling private `Block<Open>::seal`: `crates/ploke-eval/src/cli/prototype1_state/history.rs:1916` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:1934`.
- Tests cover the new narrow path: `cargo test -p ploke-eval selectable_parent_locks_crown_and_retires` and `cargo test -p ploke-eval locked_crown_must_match_block_lineage` both passed.

# Documented But Not Yet Implemented

- `Parent<Ruling>` / live ruling authority as the only writer of open block entries: documented as absent at `crates/ploke-eval/src/cli/prototype1_state/history.rs:163`.
- Live append of the handoff block through `BlockStore`: documented as absent at `crates/ploke-eval/src/cli/prototype1_state/history.rs:166`.
- Successor verification of predecessor `Block<Sealed>` before authority: documented as absent at `crates/ploke-eval/src/cli/prototype1_state/history.rs:167`.
- Ingress capture/import while Crown is locked: documented as absent at `crates/ploke-eval/src/cli/prototype1_state/history.rs:169`.
- Artifact commitment by clean tree key plus provenance manifest: documented as future work at `crates/ploke-eval/src/cli/prototype1_state/history.rs:148` and `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:223`.

# Naming / Structural Burden

- `LockCrown` names a transition, but as a public trait it becomes a generic capability surface rather than a sealed transition object. The stronger structure is a module-private transition method or `Parent<Selectable> -> Handoff<Locked>` carrier that emits the sealed block.
- `LineageKey` sounds stronger than its implementation. It is currently a debug/source string wrapper; the name should not be allowed to carry authenticated lineage/store semantics by implication.
- Legacy flattened records remain in nearby live/projection code: `SuccessorHandoffEntry`, `ChildArtifactCommittedEntry`, `ActiveCheckoutAdvancedEntry`, and `ChildReady`. The module docs correctly say these should normalize into role/state facts before becoming History entries at `crates/ploke-eval/src/cli/prototype1_state/history.rs:269` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:278`.

# Residual Risks

- The predecessor can retire and spawn a successor without producing a sealed block, leaving no History object for the successor to verify.
- A successor can enter the parent path from a valid invocation and parent identity even if no sealed History head names its current Artifact.
- The current string lineage comparison could drift from the eventual store/lineage/policy coordinate unless the next patch tightens the carrier.

# Recommended Next Patch

1. Introduce the live authority transition that consumes `Parent<Selectable>` and an open block, seals with the selected successor and active Artifact commitment, appends through `BlockStore`, and only then returns a successor handoff carrier.
2. Replace successor startup acknowledgement with sealed-head verification: derive `clean_tree_key`, load current head for the lineage/store, verify selected successor Artifact commitment, then enter the parent path.
3. Keep `Crown<Ruling>` construction private; reduce `LockCrown` visibility or remove the trait once the block-sealing transition owns the boundary.
