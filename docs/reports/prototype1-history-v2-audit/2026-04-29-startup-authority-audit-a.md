# Prototype 1 History v2 Startup Authority Audit A

Date: 2026-04-29

Scope: `AGENTS.md`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md`,
`crates/ploke-eval/src/cli/prototype1_state/history.rs`,
`crates/ploke-eval/src/cli/prototype1_state/backend.rs`, plus targeted nearby
startup/handoff call sites.

## Summary

The refined startup authority invariant in `history-blocks-v2.md` is mostly
marked as intended/prescriptive, not implemented. The code has real structural
pieces for Crown-gated block sealing, local block hashes, a filesystem
`BlockStore`, and backend-derived `TreeKey` commitments, but live startup still
enters `Parent<Ready>` through parent-identity checkout validation plus
successor invocation/ready files. It does not validate `Head(H, L, P)` against a
sealed block's selected successor artifact before entering the parent path.

The strongest current implementation claim is narrow: only a lineage-matching
`Crown<Locked>` carrier can call the crate-visible `seal` API for
`Block<Open> -> Block<Sealed>`. Startup admission through sealed History,
authenticated head proofs, policy/finality semantics, and selected-successor
artifact validation are still prescriptive.

## Claim Classification

| Checked claim | Classification | Evidence |
| --- | --- | --- |
| `history-blocks-v2.md` is a conceptual anchor; implementation claims remain in `history.rs`. | Implemented/descriptive as documentation status. | The doc says this note explains the larger model while canonical implementation claims remain in `history.rs` (`history-blocks-v2.md:5-9`). |
| Current Prototype 1 lacks distributed consensus, authenticated head-map proofs, full policy/finality semantics, and live startup admission through sealed History. | Implemented/descriptive. | The doc explicitly narrows the current implementation (`history-blocks-v2.md:39-42`); `history.rs` repeats the missing startup/head-map/consensus pieces (`history.rs:159-166`, `history.rs:137-142`). |
| For one store/lineage, only a valid typed Crown carrier may seal the next local block. | Implemented/descriptive for the local `seal` API, not for global runtime uniqueness. | `Block<Open>::seal` is private (`history.rs:1318`), `Crown<Locked>::seal` consumes the locked carrier and checks lineage (`history.rs:1455-1470`), and `Crown` has private fields/state markers with move-only `lock(self)` (`inner.rs:42-63`, `inner.rs:80-95`). |
| Sealed blocks are tamper-evident locally. | Implemented/descriptive. | Sealing commits entry hashes and header fields into a deterministic block hash (`history.rs:1318-1348`), and verification recomputes entry count, entries root, and block hash (`history.rs:1385-1423`). |
| `BlockStore::append` advances the lineage head from sealed blocks. | Implemented/descriptive but local and unauthenticated. | The trait says append is the semantic head advance (`history.rs:396-407`), `FsBlockStore::append` verifies the block then writes segment/index/head projections (`history.rs:536-565`), and `head` reads only `heads.json` (`history.rs:570-571`). The same comments state this is not an authenticated head-map proof or CAS transition (`history.rs:409-415`). |
| The refined invariant `MayEnterRuling(H,L,P,R_i)` requires `ProducedBy`, current checkout tree-key commitment, and sealed head expected successor commitment. | Intended/prescriptive. | The invariant is introduced as the "intended local admission rule" (`history-blocks-v2.md:102-112`). Current module docs say startup admission is intended and not fully implemented (`history.rs:52-56`) and that live successor validation still uses mutable scheduler/invocation state rather than clean tree key vs sealed History head (`history.rs:227-250`). |
| `TreeKey` is the backend-owned artifact identity witness startup should use. | Ambiguous/overclaiming if read as wired into startup; implemented only as staged substrate. | `WorkspaceBackend::TreeKey` and `clean_tree_key` exist (`backend.rs:264-266`, `backend.rs:374-380`), git derives it from a clean `HEAD^{tree}` after rejecting dirty checkouts (`backend.rs:1108-1137`), and History can hash only backend-owned keys (`history.rs:685-719`). But the backend trait marks `clean_tree_key` as staged for History admission (`backend.rs:379`), and startup call sites use `validate_parent_checkout`, not `clean_tree_key`/`TreeKeyHash` (`parent.rs:347-358`, `cli_facing.rs:3208-3222`). |
| A runtime may enter `Parent<Ruling>`/parent path only after sealed History admission. | Overclaiming unless phrased as intended. | `history-blocks-v2.md` says "A Runtime may enter `Parent<Ruling>` only after" the two relations (`history-blocks-v2.md:81-92`), but live startup loads `Parent<Unchecked>`, checks parent identity/scheduler agreement, then calls `acknowledge_prototype1_state_handoff` and returns `parent.ready()` (`cli_facing.rs:3208-3222`, `cli_facing.rs:3050-3122`). There is no sealed head lookup or selected successor artifact comparison in this path. |
| The current startup gate is artifact-mediated rather than process-id-mediated. | Intended/prescriptive for History; implemented only as weaker parent-identity checkout validation. | `history.rs` claims current Crown/History gates artifact eligibility (`history.rs:117-122`), but actual validation checks clean checkout, branch, parent identity commit message, identity path, and gen0 freshness (`backend.rs:1037-1105`). That is artifact/identity validation, but not History head admission. |
| `Crown<Locked>` does not prove OS-process uniqueness. | Implemented/descriptive and correctly narrowed. | `history-blocks-v2.md` explicitly excludes process uniqueness (`history-blocks-v2.md:114-127`), and `history.rs` says multiple runtimes may execute while authority is about typed carriers (`history.rs:113-122`). No process lease/lock/consensus mechanism appears in the checked startup/handoff code; successor handoff waits on ready files after spawning (`prototype1_process.rs:861-895`, `prototype1_process.rs:941-955`). |
| Live handoff seals and persists a History block before successor admission. | Intended/prescriptive; currently not implemented. | `history.rs` says live handoff locks a Crown but does not yet seal or persist a block (`history.rs:6-11`) and lists live append plus successor verification as not enforced (`history.rs:159-164`). Runtime handoff calls `parent.lock_crown()` and creates a successor invocation, but does not call `Crown<Locked>::seal` or `BlockStore::append` (`prototype1_process.rs:861-895`). |
| Block contents include policy/finality/head-state/stochastic evidence commitments. | Intended/prescriptive. | `history-blocks-v2.md` defines these as block content groups (`history-blocks-v2.md:181-193`, `history-blocks-v2.md:303-311`), while `history.rs` says current `Block` implements admitted entries plus sealed header material only and the larger grouping is the next type-slice target (`history.rs:1182-1218`). |

## Critiques

1. Startup invariant wording is ahead of the live gate.

   `history-blocks-v2.md` first states "A Runtime may enter `Parent<Ruling>` only after"
   `ProducedBy` and `AdmittedBy` (`history-blocks-v2.md:81-92`), then later marks the
   refined rule as intended (`history-blocks-v2.md:102-112`). The implemented startup
   path instead validates parent identity and invocation continuity before returning
   `Parent<Ready>` (`cli_facing.rs:3050-3122`, `cli_facing.rs:3208-3222`). Because no
   `BlockStore::head`, `Block<Sealed>`, `SuccessorRef`, or `TreeKeyHash` check is in
   that path, the first sentence should be read as prescriptive. It should be made
   explicit in the doc at the first claim site, not only in the later "intended" wording.

2. `Current claim updated 2026-04-29` in `history.rs` overstates current code if read
   literally.

   The module says a runtime may enter the ruling parent path only if the clean Artifact
   tree matches the Artifact committed by the current sealed History head
   (`history.rs:117-122`). Nearby lines correctly list successor verification of
   `Block<Sealed>` as not enforced (`history.rs:159-164`). Actual live validation is
   branch/parent-identity based (`backend.rs:1037-1105`) and the `clean_tree_key` API is
   staged (`backend.rs:374-380`, `backend.rs:1108-1137`). This claim should be narrowed
   to "intended current claim" or split into "implemented parent checkout gate" versus
   "History startup gate target."

3. The head store is a rebuildable local projection, not an authority proof.

   `FsBlockStore::append` writes a JSONL block stream and updates `heads.json`
   (`history.rs:536-565`), while `head` returns only the hash from that projection
   (`history.rs:570-571`). Comments already say there are no authenticated inclusion or
   absence proofs and no CAS-style head transition (`history.rs:409-415`). Any Crown/Block
   claim that relies on `Head(H,L,P)` must remain prescriptive until the head API returns
   enough authenticated head reference material for startup admission.

4. `SuccessorRef` currently commits generic artifact refs, not backend tree-key
   commitments.

   A sealed header stores `selected_successor: SuccessorRef` and `active_artifact:
   ArtifactRef` (`history.rs:1158-1168`), and `SuccessorRef` is just runtime plus
   `ArtifactRef` (`history.rs:789-799`). The backend-owned `TreeKeyHash` exists
   (`history.rs:685-719`), but it is used in `GenesisAuthority`, not in `SuccessorRef`
   or startup admission (`history.rs:747-765`). Therefore the refined
   `ExpectedSuccessor(A_i)`/`TreeKey(A_i)` rule is not grounded in current block shape.

5. Structural naming is mostly good in the new History core, but live records still
   expose flattened legacy surfaces.

   The new carriers use `Block<Open>`, `Block<Sealed>`, `Crown<Locked>`,
   `Parent<Ready>`, and `Parent<Retired>` (`history.rs:208-225`, `inner.rs:42-63`,
   `parent.rs:48-54`). That matches the AGENTS guidance to preserve role/state structure
   (`AGENTS.md:5-12`, `AGENTS.md:17-23`). The remaining live handoff still records
   `SuccessorHandoffEntry` and ready-file based records (`prototype1_process.rs:941-955`),
   which `history.rs` itself identifies as legacy records that should be normalized
   before becoming History entries (`history.rs:252-262`). Treat these names as legacy
   evidence/projections, not authority structure.

## Actionable Corrections

1. In `history-blocks-v2.md`, change the first startup paragraph from a bare invariant
   claim to "intended startup invariant" or add an immediate note that current startup
   does not enforce it (`history-blocks-v2.md:81-92`).

2. In `history.rs`, narrow `Current claim updated 2026-04-29` so it does not imply the
   sealed-head artifact gate is live; cross-reference the explicit non-enforcement list
   nearby (`history.rs:117-122`, `history.rs:159-164`).

3. Before relying on the refined invariant operationally, wire startup through:
   `clean_tree_key` (`backend.rs:1108-1137`), `TreeKeyHash` (`history.rs:685-719`), a
   sealed head lookup richer than `Option<BlockHash>` (`history.rs:417-427`), and a
   selected-successor commitment that names the expected tree key rather than only
   `ArtifactRef` text (`history.rs:789-799`, `history.rs:1158-1168`).

4. Keep legacy ready/handoff records out of authority claims until imported as typed
   History evidence under explicit policy; the current handoff path uses those records
   after `lock_crown` but before any block seal/store append (`prototype1_process.rs:861-895`,
   `prototype1_process.rs:941-955`).

