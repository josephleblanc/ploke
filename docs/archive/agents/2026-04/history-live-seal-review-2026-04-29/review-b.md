# Review B: live History seal during successor handoff

Commit reviewed: `3acf04db536acd044d73e4ba93b6d1641822d9e8` (`history: append sealed block during successor handoff`)

Scope: implementation and directly relevant docs in `crates/ploke-eval/src/cli/prototype1_process.rs`, `crates/ploke-eval/src/cli/prototype1_state/history.rs`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs`, and adjacent parent/invocation wiring.

Verification run: `cargo test -p ploke-eval prototype1_state::history -- --nocapture` passed: 22 tests, 0 failures.

## Findings

### High: live successor blocks trust rebuildable head projections as predecessor authority

The new live handoff opens non-genesis blocks from `FsBlockStore::head()` in `handoff_block_fields` and converts that result directly into `OpeningAuthority::Predecessor(PredecessorAuthority::new(predecessor))` at `crates/ploke-eval/src/cli/prototype1_process.rs:1036` through `crates/ploke-eval/src/cli/prototype1_process.rs:1047`.

That head is only a projection. `FsBlockStore::head()` reads `heads.json`, then `stored_by_hash()` scans `by-hash.jsonl` and returns `BlockHead` from the index record at `crates/ploke-eval/src/cli/prototype1_state/history.rs:527` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:611`. It does not load the referenced sealed block from `blocks/segment-000000.jsonl`, does not verify the referenced block hash, and does not prove that the predecessor is the current sealed head. `append()` only verifies the new block hash at `crates/ploke-eval/src/cli/prototype1_state/history.rs:573` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:604`.

This means a corrupted or stale `heads.json`/`by-hash.jsonl` pair can authorize a new block that points at an unverified or nonexistent predecessor. It also means a missing `heads.json` with an existing block segment is treated as absence and can cause a second genesis block, because `read_heads()` returns an empty map for `NotFound` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:527` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:533`, and the live path then takes the genesis branch at `crates/ploke-eval/src/cli/prototype1_process.rs:1049` through `crates/ploke-eval/src/cli/prototype1_process.rs:1067`.

This conflicts with the docs' own store-scoped genesis rule: if the configured store is "unreadable, ambiguous, or inconsistent", startup must reject rather than bootstrap (`crates/ploke-eval/src/cli/prototype1_state/history.rs:87` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:91`). The current implementation does not distinguish "no History exists" from "the head projection is absent while block data may exist."

Recommended fix: make the live opening path consume a verified store head, not a projection. `BlockStore::head` should return a `Head<Verified>` or equivalent carrier built by loading the sealed block, verifying its hash, and checking index consistency. `append` should reject predecessor/head mismatches instead of only appending the new block.

### High: sealed live block material is still caller-assembled, not a projection of a typed handoff transition

The commit moves sealing into `Parent<Selectable>::seal_block(...)` at `crates/ploke-eval/src/cli/prototype1_state/inner.rs:183` through `crates/ploke-eval/src/cli/prototype1_state/inner.rs:193`, and that is a real improvement over exposing `Crown<Ruling>` construction. However, the authoritative material still comes from caller-assembled `OpenBlock` and `SealBlock` values.

The live process builds `OpenBlock` as a struct literal at `crates/ploke-eval/src/cli/prototype1_process.rs:1070` through `crates/ploke-eval/src/cli/prototype1_process.rs:1080`, and builds `SealBlock` using `SealBlock::from_handoff(...)` at `crates/ploke-eval/src/cli/prototype1_process.rs:890` through `crates/ploke-eval/src/cli/prototype1_process.rs:895`. `SealBlock::from_handoff` fills `claims` with `block::Claims::empty_unchecked()` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1518` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:1530`, and `empty_unchecked` produces no policy/surface/manifest claims at `crates/ploke-eval/src/cli/prototype1_state/history.rs:1582` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:1589`.

The type boundary proves only that a `Parent<Selectable>` was consumed, the Crown lineage matches the block lineage, and the open block can be sealed. It does not prove that the selected successor, active artifact, policy reference, or evidence were derived from a typed selection/install transition. Those fields remain data supplied by the caller. That is weaker than the stated rule that durable History blocks should be projections of allowed typed transitions rather than caller-assembled status blobs (`crates/ploke-eval/src/cli/prototype1_state/history.rs:141` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:146`).

Recommended fix: replace `(OpenBlock, SealBlock)` arguments on `LockCrown::seal_block` with a typed handoff carrier, for example a `Handoff<Selected>` or `Parent<Selectable>::seal_selected_successor(...)` method that consumes the selected child/artifact capability, the installed active checkout proof, and verified/admitted claims. Make `OpenBlock` and `SealBlock` fields private outside `history` or their construction module once that carrier exists.

### Medium: append is not a lineage-head compare-and-swap, so live sealing can fork or overwrite the head

`handoff_block_fields` reads the current head before sealing at `crates/ploke-eval/src/cli/prototype1_process.rs:1036` through `crates/ploke-eval/src/cli/prototype1_process.rs:1047`, then `spawn_and_handoff_prototype1_successor` appends later at `crates/ploke-eval/src/cli/prototype1_process.rs:896` through `crates/ploke-eval/src/cli/prototype1_process.rs:902`. `FsBlockStore::append` then unconditionally inserts the new block hash into `heads.json` at `crates/ploke-eval/src/cli/prototype1_state/history.rs:600` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:602`.

The docs correctly warn that `append` does not yet validate a compare-and-swap lineage-head transition (`crates/ploke-eval/src/cli/prototype1_state/history.rs:424` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:430`), but the new live code now relies on this method as the handoff authority boundary. Two parent-capable processes, retries, or a stale local process can both read the same head and append different blocks at the same successor height. The type system does not prevent this across process boundaries.

Recommended fix: make `append` accept the expected predecessor/head state and reject stale writes. At minimum, append should check that the current persisted head still matches the predecessor used to open the block, and reject duplicate `(lineage_id, block_height)` unless explicitly admitted as a merge by policy.

### Medium: successor startup still does not consume the sealed head, so the cross-runtime contract is not closed

The block is appended before spawning at `crates/ploke-eval/src/cli/prototype1_process.rs:896` through `crates/ploke-eval/src/cli/prototype1_process.rs:910`, but the successor invocation created immediately afterward does not carry the sealed block hash or a verified History-head target (`crates/ploke-eval/src/cli/prototype1_process.rs:911` through `crates/ploke-eval/src/cli/prototype1_process.rs:921`). Readiness polling only loads the ready record at `crates/ploke-eval/src/cli/prototype1_process.rs:840` through `crates/ploke-eval/src/cli/prototype1_process.rs:844`.

The docs are mostly honest that successor verification is not implemented (`crates/ploke-eval/src/cli/prototype1_state/history.rs:15` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:18`, and `crates/ploke-eval/src/cli/prototype1_state/mod.rs:194` through `crates/ploke-eval/src/cli/prototype1_state/mod.rs:199`). The remaining correctness risk is that the new append can look like a live authority gate, while startup still uses invocation/ready files instead of the sealed head.

Recommended fix: include the expected sealed block hash and lineage in the successor invocation, then require startup to verify the active checkout/tree key and parent identity against that sealed head before entering `Parent<Ready>` or any later parent state.

### Low: docs still contain a stale "does not yet enforce live append" claim

`history.rs` now opens the file by saying the live handoff seals and appends a minimal block before successor launch (`crates/ploke-eval/src/cli/prototype1_state/history.rs:6` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:18`). Later, the "current implementation does not yet enforce" list still includes "live append of the handoff block through `BlockStore`" at `crates/ploke-eval/src/cli/prototype1_state/history.rs:170` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:177`.

Recommended fix: move live append to the implemented list, but qualify it: live append exists, successor verification and authenticated/CAS head admission do not.

### Low: structural naming still points at missing carriers

`LockCrown` now exposes `seal_block` (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:161` through `crates/ploke-eval/src/cli/prototype1_state/inner.rs:171`), so the trait name no longer matches the transition surface. The transition is not just "lock Crown"; it consumes `Parent<Selectable>` and produces `(Parent<Retired>, Block<Sealed>)`. Similarly, `handoff_block_fields` (`crates/ploke-eval/src/cli/prototype1_process.rs:1029` through `crates/ploke-eval/src/cli/prototype1_process.rs:1081`) is a long helper name carrying missing structure: it gathers store head, parent identity, tree key, policy refs, and artifact refs that should be a typed handoff context or transition object.

Recommended fix: introduce a structural carrier for the handoff, then name methods locally from that boundary. For example, `Parent<Selectable>::seal(handoff)` or `Handoff<Selected>::seal(parent)` is clearer than a `LockCrown` trait with generic `OpenBlock`/`SealBlock` arguments.

## Overall assessment

The commit improves the live sequence: the predecessor now seals and appends a `Block<Sealed>` before attempting successor launch. The local typestate checks around `Block<Open>`, `Block<Sealed>`, `Crown<Ruling>`, and `Crown<Locked>` are still useful, and the tests cover several of those local invariants.

The main gap is that the new live block is not yet anchored to verified predecessor authority or a first-class handoff transition. It is a sealed, deterministic record, but the most important handoff facts are still assembled in `prototype1_process.rs` and the store head used for continuity is a mutable projection. Treat this as a live append milestone, not as completed Crown/History authority.
