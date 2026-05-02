# Review: 8a1d198d `history: narrow crown minting boundary`

## Findings

1. **High: the current local admission claim is still not enforced by the live parent path.**  
   The claimed target is that only a runtime whose current Artifact matches the successor Artifact committed by the sealed History head may enter the ruling parent path. The live path still enters through `Parent::<Unchecked>::load(...).check(...)`, then acknowledges handoff from a successor invocation, and the continuation check only reads mutable scheduler/node state: `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3208`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3221`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3050`, `crates/ploke-eval/src/cli/prototype1_process.rs:378`, `crates/ploke-eval/src/cli/prototype1_process.rs:389`. The docs correctly disclose this gap in `crates/ploke-eval/src/cli/prototype1_state/history.rs:117` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:254`, and in `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:83`. The commit narrows `Crown<Ruling>` construction, but it does not yet make sealed History the gate into parent authority.

2. **Medium: the live Crown lock is still a mint-and-lock projection, not a transition from an owned ruling Crown.**  
   `LockCrown for Parent<Selectable>` consumes the parent and creates a fresh `Crown<Ruling>` from a `LineageKey`, then immediately locks it: `crates/ploke-eval/src/cli/prototype1_state/inner.rs:147`. The lineage value comes from `ParentIdentity.campaign_id`, not from an open block or sealed predecessor head: `crates/ploke-eval/src/cli/prototype1_state/parent.rs:423`. The handoff path calls this before successor spawn: `crates/ploke-eval/src/cli/prototype1_process.rs:876`. This is better than the pre-commit public-ish `Crown::for_lineage` boundary, but it still does not prove that the predecessor held `Crown<Ruling>`, wrote the open block, selected the successor, or sealed the handoff block. That is exactly the intended sequence described at `crates/ploke-eval/src/cli/prototype1_state/history.rs:58` and `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:51`.

3. **Medium: `LineageKey` preserves the minting boundary but remains too weak semantically.**  
   The new wrapper prevents ordinary callers from passing a raw `String` directly into `Crown::for_lineage`, because `Crown::for_lineage` is private to `inner.rs`: `crates/ploke-eval/src/cli/prototype1_state/inner.rs:83`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:117`. However, `LineageKey` is still a debug/source string, constructible by sibling modules, and sealing compares it to `LineageId` by string equality: `crates/ploke-eval/src/cli/prototype1_state/inner.rs:84`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:94`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1927`. That does not encode the documented configured store, policy, History head, or Artifact commitment dimensions described in `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:106`. The wrapper is a good staging move, but `LineageKey` should not be treated as the authority coordinate yet.

4. **Low: `LockCrown` has the wrong semantic shape for the eventual transition.**  
   The trait is crate-visible and verb-shaped, with no `Transition` association, no selected successor, no open block, and no durable History record: `crates/ploke-eval/src/cli/prototype1_state/inner.rs:135`. It currently exists mainly to hide `Crown<Ruling>` construction from sibling modules. That is useful, but the name and interface carry transition structure in text rather than encoding the domain object. The intended domain shape is closer to a concrete handoff transition over `Parent<Ruling>`/`Crown<Ruling>`/`Block<Open>` producing `Parent<Retired>`/`Crown<Locked>`/`Block<Sealed>`.

5. **Low: `Crown<Ruling>::admit_claim` still trusts caller-supplied ruler identity.**  
   The method requires possession of `Crown<Ruling>`, but accepts `ActorRef` as data and records it as both witness and admission authority: `crates/ploke-eval/src/cli/prototype1_state/history.rs:1883`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1888`. The comment already records the gap. Once live code can obtain a real ruling Crown, this should move behind a `Parent<Ruling>` or equivalent carrier so the actor identity is supplied structurally.

## Implemented Guarantees

- `Crown<S>` fields and crown state markers are private/module-private enough that production crate code cannot directly construct `Crown<Ruling>` by struct literal or raw lineage string: `crates/ploke-eval/src/cli/prototype1_state/inner.rs:42`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:59`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:117`.
- `Crown<Locked>::seal` consumes the locked Crown and checks lineage before sealing an open block: `crates/ploke-eval/src/cli/prototype1_state/history.rs:1916`.
- `Block<Open>::seal` remains private to `history.rs`, so crate callers must pass through the locked Crown API: `crates/ploke-eval/src/cli/prototype1_state/history.rs:1734`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1919`.
- Focused tests pass: `cargo test -p ploke-eval prototype1_state::history --no-fail-fast` and `cargo test -p ploke-eval prototype1_state::parent --no-fail-fast`.

## Documented But Not Yet Implemented

- `Startup<Validated> -> Parent<Ruling>` through sealed History head and current Artifact commitment.
- Live sealing and persistence of the handoff `Block<Sealed>`.
- `Parent<Ruling>` as the only writer of open block entries.
- Authenticated lineage-head map / present-absent proofs.
- Artifact-local provenance manifest committed by both Artifact and History.

## Structural Naming Notes

- `LineageKey` currently names a stronger semantic object than it implements. Until it includes store/head/policy/artifact coordinates, treat it as a debug lineage string wrapper.
- `LockCrown` is a flattened transition name. Prefer a concrete transition carrier or module boundary that makes the participating states explicit.
- Existing docs already call out legacy flattened records such as `ChildArtifactCommittedEntry`, `ActiveCheckoutAdvancedEntry`, and `SuccessorHandoffEntry` as evidence shapes to normalize rather than preserve as History entry kinds: `crates/ploke-eval/src/cli/prototype1_state/history.rs:269`.

## Next Patch Recommendations

1. Introduce the real startup/admission carrier: `Startup<Observed> -> Startup<Validated> -> Parent<Ruling>`, where validation reads the sealed History head and checks the current Artifact commitment.
2. Make `Parent<Ruling>` carry or borrow the only production `Crown<Ruling>`, then make handoff consume `Parent<Ruling>`, `Crown<Ruling>`, selected successor identity, and `Block<Open>` to produce `Parent<Retired>`, `Crown<Locked>`, and `Block<Sealed>`.
3. Replace `LineageKey::from_debug_value(campaign_id)` with a constructor derived from the configured History surface and `LineageId`; add policy/head/artifact dimensions before relying on it for admission.
4. Narrow or seal `LockCrown`, or replace it with a concrete handoff module API whose return value includes the durable record emitted by the transition.
5. Move `admit_claim` behind the eventual `Parent<Ruling>` carrier so actor/ruler identity is not caller-assembled data.
