# Review A: live History seal handoff

Reviewed commit: `3acf04db history: append sealed block during successor handoff`.

Scope: implementation and directly related docs in `crates/ploke-eval/src/cli/prototype1_state/history.rs`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs`, `crates/ploke-eval/src/cli/prototype1_process.rs`, and successor startup wiring in `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs`.

## Findings

### High: `BlockStore::append` can advance the lineage head without proving it extends the current head

`FsBlockStore::append` verifies the sealed block hash, appends the block and indexes, then unconditionally overwrites `heads.json` for the block lineage (`crates/ploke-eval/src/cli/prototype1_state/history.rs:573`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:589`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:600`). It does not check that a height-0 block is appended only when there is no existing head, that a non-genesis block cites the current stored head, or that the new height is exactly current height + 1. The docs accurately admit no CAS/authenticated map yet (`crates/ploke-eval/src/cli/prototype1_state/history.rs:424`), but live handoff now relies on this store as the durable head update before successor launch (`crates/ploke-eval/src/cli/prototype1_process.rs:899`).

Impact: a stale or forged sealed block from another crate module can regress or fork the live head by calling `append`, because the store treats block-local hash validity as sufficient to advance lineage authority. This undercuts the claim that `heads.json` is only a projection of accepted sealed blocks rather than an independent status update.

Recommended fix: make append a typed head transition. Require an expected predecessor head or explicit bootstrap absence proof, reject duplicate genesis, reject non-current predecessor hashes, reject nonconsecutive heights, and update segment/index/head atomically enough for the local prototype. Even before an authenticated map exists, `append` should validate the current projection it is about to advance.

### High: live handoff block contents are still caller-assembled authority data

The live path builds `OpenBlock` in `handoff_block_fields` from process-local strings and store projections (`crates/ploke-eval/src/cli/prototype1_process.rs:1029`, `crates/ploke-eval/src/cli/prototype1_process.rs:1040`, `crates/ploke-eval/src/cli/prototype1_process.rs:1070`). `OpenBlock` exposes all authority-bearing fields crate-wide (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1467`), including `lineage_id`, `parent_block_hashes`, `opening_authority`, `opened_by`, `opened_from_artifact`, `ruling_authority`, `policy_ref`, and `opened_at`. `SealBlock::from_handoff` similarly accepts successor/artifact/transition fields from the caller and uses empty unchecked claims (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1518`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1528`).

`Parent<Selectable>::seal_block` then proves only that the parent's lineage string matches the supplied `OpenBlock` lineage before sealing (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:183`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1983`). It does not derive the actor identity, current head, policy, selected successor, active artifact, or claim set from typed parent/successor carriers.

Impact: the commit improves the transition surface by moving from `lock_crown` to `seal_block`, but the durable block is still partly a caller-assembled status blob. Another module cannot directly call `Block::open`, but it can provide misleading authority fields through `OpenBlock` and obtain a sealed block if it has any `Parent<Selectable>` for the same campaign lineage.

Recommended fix: make `OpenBlock` fields private or module-private and move handoff construction behind a narrower typed transition, for example `Parent<Selectable>::seal_successor(store, selected: Successor<Selected>, policy: PolicyRef, evidence: HandoffEvidence)`. The parent transition should derive lineage and actor fields from `ParentIdentity`, derive predecessor from a validated store head transition, and make empty/unchecked claims explicit as a degraded prototype mode.

### Medium: genesis is inferred from missing `heads.json`, not from a validated store-scoped absence proof

The live handoff creates a genesis block whenever `store.head(&lineage_id)` returns `None` (`crates/ploke-eval/src/cli/prototype1_process.rs:1036`, `crates/ploke-eval/src/cli/prototype1_process.rs:1049`). `FsBlockStore::read_heads` returns an empty map when `heads.json` is missing (`crates/ploke-eval/src/cli/prototype1_state/history.rs:527`). It does not scan or validate the block segment/index before accepting absence. The docs state that genesis absence is local and store-scoped and that unreadable, ambiguous, or inconsistent stores must reject rather than silently bootstrap (`crates/ploke-eval/src/cli/prototype1_state/history.rs:87`).

Impact: removing or losing `index/heads.json` allows a later live handoff to create a second height-0 block for a lineage even if the append-only segment still contains prior blocks. That is a concrete bypass of the intended genesis absence rule.

Recommended fix: distinguish `NoStore`, `EmptyValidatedStore`, `HeadFound`, and `InconsistentStore` in the storage API. Bootstrap should require an explicit validated absence carrier, not `Option<BlockHead>`. At minimum, `head()` should rebuild or cross-check from `by-hash.jsonl`, `by-lineage-height.jsonl`, and the segment before returning `None`.

### Medium: successor startup still does not consume the sealed head, and the live invocation does not carry it

The parent seals/appends before spawning (`crates/ploke-eval/src/cli/prototype1_process.rs:896`, `crates/ploke-eval/src/cli/prototype1_process.rs:900`, `crates/ploke-eval/src/cli/prototype1_process.rs:946`), which matches the new narrow implementation claim. But successor startup still validates only the handoff invocation, active root, parent identity, and mutable scheduler continuation (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3050`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3100`, `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3110`; `crates/ploke-eval/src/cli/prototype1_process.rs:383`, `crates/ploke-eval/src/cli/prototype1_process.rs:394`). The invocation is created from the retired parent after append, but it does not include the stored block hash or require the successor to verify it (`crates/ploke-eval/src/cli/prototype1_process.rs:911`).

Impact: this is mostly documented as not implemented, but it means the new sealed block is not yet part of successor admission. A successor can acknowledge handoff without binding its startup to the sealed head the predecessor just wrote.

Recommended fix: add the stored `BlockHead` or expected `BlockHash` to the successor invocation, then make `acknowledge_prototype1_state_handoff` verify the configured store head, block hash, selected successor identity, active artifact/tree key, and parent identity before returning `Parent<Ready>`.

### Low: module docs still contain stale persistence-gap language

`history.rs` and the Crown section of `mod.rs` were updated to say live handoff seals/appends before successor launch (`crates/ploke-eval/src/cli/prototype1_state/history.rs:7`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:13`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:194`). However, `history.rs` still lists "live append of the handoff block through `BlockStore`" among current non-enforced items (`crates/ploke-eval/src/cli/prototype1_state/history.rs:170`). `mod.rs` also says the live implementation still lacks "live sealing and persistence of a `Crown<Locked>` History block" (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:662`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:668`).

Impact: reviewers and future agents will get conflicting status claims from adjacent docs.

Recommended fix: narrow those bullets to the remaining gaps: successor sealed-head admission, authenticated/CAS head transition, and typed derivation of block contents from parent/successor carriers.

### Low: structural naming still exposes flattened legacy events at the live boundary

The docs explicitly warn that `ChildArtifactCommittedEntry`, `ActiveCheckoutAdvancedEntry`, and `SuccessorHandoffEntry` should remain legacy evidence, not History entry kinds (`crates/ploke-eval/src/cli/prototype1_state/history.rs:281`). The live path still emits `SuccessorHandoffEntry` after ready acknowledgement (`crates/ploke-eval/src/cli/prototype1_process.rs:970`) and uses helper names such as `handoff_block_fields` (`crates/ploke-eval/src/cli/prototype1_process.rs:1029`). This is acceptable as a legacy journal seam, but the new History construction is close enough that the naming now hides the missing structural carrier.

Impact: low immediate risk, but it encourages expanding process helpers instead of introducing the missing typed object: a selected successor handoff authorized by a parent under policy.

Recommended fix: when the next slice changes this area, introduce a local carrier for the handoff transition and keep legacy journal names behind an evidence/import boundary. Prefer names shaped around carriers and state, such as `Successor<Selected>` or `Handoff<Sealed>`, over adding more `prototype1_successor_*` helpers.

## Positive observations

The commit does move the live parent across a move-only transition before successor spawn: `Parent<Selectable>::seal_block` consumes the parent and returns `Parent<Retired>` with a sealed block (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:183`), and spawn receives the retired parent (`crates/ploke-eval/src/cli/prototype1_process.rs:946`). `Block::open`, `Block::seal`, and `Block::admit` remain private to `history.rs`, while crate-visible construction is routed through `Crown<Ruling>` and `Crown<Locked>` methods (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1743`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1833`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1973`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2077`). Those are the right local type barriers for the narrow step.

## Bottom line

The latest commit truthfully implements "seal and append a minimal block before successor process launch." It does not yet implement "durable History blocks are projections of allowed typed transitions" in the stronger Crown/History sense, because the head transition is unchecked and the block authority fields are still caller-supplied. The docs should keep the new narrow claim, but they should explicitly name the remaining weak points as storage-head validation, successor sealed-head startup verification, and private typed construction of handoff block contents.
