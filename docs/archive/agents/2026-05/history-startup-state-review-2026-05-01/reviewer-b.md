# Prototype 1 History/Crown Startup State Review - Reviewer B

Reviewed commit: `fd42a4bc` (`prototype1: validate startup against history state`)
Date: 2026-05-01

## 1. Executive Summary

Verdict: directionally correct, but not yet strong enough for the full Crown startup contract.

The commit materially improves live startup admission: `Parent<Checked>` now requires a `Startup<Validated>` before it can become `Parent<Ready>`, genesis startup rejects an existing configured History head, and predecessor startup verifies the sealed head's artifact tree key and surface commitment before entering the parent path.

The main correctness gap is that predecessor startup verifies "this checkout matches the sealed head's artifact claim", but does not verify that the sealed head actually selected this successor runtime/node/identity. That leaves the cross-runtime Crown contract weaker than the docs describe. A second important gap is that `FsBlockStore::append` is not atomic across its stale-state check and projection writes, so the local state-root check is not a concurrency-safe single-ruler guarantee.

## 2. Findings

### High: Predecessor startup does not verify the sealed head's selected successor

Evidence:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:213`-`217` says successor validation should verify the previous block hash, selected Artifact, selected successor identity, policy-bearing surface digest, and evidence references.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2644`-`2650` stores `selected_successor` and `active_artifact` in `SealedBlockHeader`.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:504`-`520` loads the sealed head and verifies only the current artifact tree and current surface.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:553`-`605` validates the startup kind, lineage, parent node id, and generation, but still does not inspect `selected_successor`.
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3081`-`3119` checks invocation campaign/node/root against the local identity and mutable invocation data, then constructs `Startup<Predecessor>`, but does not compare the invocation or identity to the sealed block's selected successor.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2466`-`2477` allows `SealBlock::from_handoff` callers to supply `selected_successor` and `active_artifact` independently of the artifact claim later injected in `inner.rs:214`-`218`.

Why it matters:

The implemented check proves that the current checkout matches an artifact claim in the current sealed head. It does not prove that this runtime was the successor selected by that head. A malformed or future buggy handoff can seal a block whose artifact claim matches the checkout but whose `selected_successor` points elsewhere, and predecessor startup will still admit the runtime. That is weaker than the documented Crown handoff contract.

### Medium: History append state-root checks are not atomic

Evidence:

- `crates/ploke-eval/src/cli/prototype1_state/history.rs:821`-`836` re-reads current lineage state and compares it to `expected`.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:837`-`863` then appends the block, appends indexes, and finally rewrites `heads.json`, with no file lock, compare-and-swap, transaction, or atomic rename protocol covering the whole operation.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:590`-`596` says `append` must consume expected lineage state so a block can advance only from the observed predecessor state.

Why it matters:

The stale-state check catches sequential stale appends, but two local processes can both observe the same absent or present head before either writes. Both can then pass the check and write conflicting segment/index/head data. The docs correctly avoid claiming distributed consensus, but the implementation also lacks a local filesystem mutual exclusion/transaction boundary for the single-ruler append path.

### Medium: Genesis startup is still absence-only, not full artifact admission

Evidence:

- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:442`-`470` implements `Startup<Genesis>::from_history` as lineage-state read, lineage match, generation-zero check, and `StoreHead::Absent`.
- `crates/ploke-eval/src/cli/prototype1_state/mod.rs:93`-`99` describes genesis admission as absence of a valid associated head for the lineage/artifact and predecessor admission as a sealed head naming the current clean artifact tree.
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:103`-`107` says unreadable, ambiguous, or checkout-inconsistent configured stores must reject rather than silently bootstrap.
- `crates/ploke-eval/src/cli/prototype1_process.rs:1145`-`1165` shows the first live handoff creates the genesis block if no head exists; gen0 startup itself does not open or append a genesis block.

Why it matters:

This is mostly documented as a limitation, but some wording still suggests a lineage/artifact absence claim. The current genesis gate is lineage absence in the configured local store plus `Parent<Checked>` checkout validation. It does not bind a startup `ProducedBy(SelfRuntime, CurrentArtifact)` or an artifact-local manifest into History before gen0 becomes `Parent<Ready>`.

### Low: Naming in the live process seam still flattens structure

Evidence:

- `crates/ploke-eval/src/cli/prototype1_process.rs:384` defines `validate_prototype1_successor_continuation`.
- `crates/ploke-eval/src/cli/prototype1_process.rs:404` defines `validate_prototype1_successor_node_continuation`.
- `crates/ploke-eval/src/cli/prototype1_process.rs:481` defines `prepare_prototype1_active_successor_runtime`.
- `crates/ploke-eval/src/cli/prototype1_process.rs:496` defines `install_prototype1_successor_artifact`.
- `crates/ploke-eval/src/cli/prototype1_process.rs:930` defines `spawn_and_handoff_prototype1_successor`.

Why it matters:

These names carry subsystem, role, phase, and action because `prototype1_process.rs` is still too broad. This is not a direct correctness bug in this commit, but it makes the authority boundary harder to audit. The newer `Parent<Checked> -> Startup<Validated> -> Parent<Ready>` naming is much stronger; the process seam should move toward that structure.

## 3. Claimed Invariants Actually Enforced

- `Parent<Ready>` in the live `prototype1-state` path now requires `Startup<Validated>`: `cli_facing.rs:3217`-`3235` loads/checks `Parent<Unchecked> -> Parent<Checked>` and calls `acknowledge_prototype1_state_handoff`; `parent.rs:432`-`439` consumes `Startup<Validated>` before returning `Parent<Ready>`.
- `Parent<Checked>` validates active checkout identity and scheduler node facts: `parent.rs:386`-`424` validates checkout, generation, branch, and selected instance.
- Genesis startup rejects non-gen0 and existing configured heads: `parent.rs:459`-`477`.
- Predecessor startup rejects absent heads, loads the sealed head by indexed hash, verifies the sealed block hash, verifies current clean tree key against the sealed artifact claim, and verifies current surface roots: `parent.rs:492`-`521`, `history.rs:787`-`815`, `history.rs:2940`-`2985`.
- The local state map has a sparse-Merkle proof object for present/absent lineage-head projection and verifies it before append: `history.rs:1013`-`1093`, `history.rs:1177`-`1186`.
- Append rejects sequential stale heads, wrong opening state roots, duplicate genesis, non-consecutive heights, and blocks that do not cite the current head: `history.rs:821`-`836`, `history.rs:1224`-`1275`.
- Surface commitment gates the immutable `crates/ploke-eval` surface in backend computation and compares current after-roots at startup: `backend.rs:1217`-`1243`, `history.rs:1486`-`1501`.

## 4. Claimed Or Ambiguous Invariants Still Weaker Than Stated

- The Crown handoff is not yet a complete cross-runtime typed contract because startup does not check `selected_successor` or successor runtime identity against the sealed head.
- `Startup<Genesis>` is a local configured-store absence observation, not a full genesis admission procedure with artifact-local provenance and a sealed genesis block before gen0 parent execution.
- The sparse Merkle state-map proof is local to the filesystem `heads.json` projection and has no external root anchor, fork-choice policy, or consensus semantics. The docs mostly state this correctly.
- `FsBlockStore::append` is stale-state checked but not transactionally serialized.
- `OpenBlock` still accepts actor identity, policy labels, surface commitment, and artifact refs as crate-visible data fields; `Crown<Ruling>::open_block` checks lineage only (`history.rs:3013`-`3031`). The docs acknowledge this gap at `history.rs:3016`-`3022`.
- Stored sealed block loading supports zero-entry blocks only: `history.rs:787`-`815`, `history.rs:940`-`963`. That is acceptable for the current handoff path but not for general History recovery.
- Continuation admission still consults mutable scheduler state before predecessor startup: `prototype1_process.rs:384`-`437`.

## 5. Specific Next Patch Recommendations

1. Add a sealed-head successor admission check, preferably as a method on `Block<block::Sealed>`, that verifies the invocation/runtime id, parent identity node id, selected successor artifact/ref, and active artifact against the sealed header before `Startup<Predecessor>` can become `Validated`.
2. Make `SealBlock::from_handoff` harder to misuse: either derive `selected_successor`, `active_artifact`, and the admitted artifact claim from one typed handoff carrier, or add a validation step that rejects inconsistent header material before sealing.
3. Add a filesystem append lock or atomic transaction boundary around `FsBlockStore::append`, including state read, segment/index append, and head update. Until then, narrow docs from "single-ruler local History" to "sequential stale-state checked local History".
4. Narrow genesis wording to "lineage absence in the configured local store plus checked parent checkout" until startup can bind current artifact tree, surface digest, and parent identity into a genesis admission carrier before `Parent<Ready>`.
5. Refactor the broad successor helpers in `prototype1_process.rs` into a small handoff/startup context type so names can become `validate`, `prepare_runtime`, `install_artifact`, and `spawn` under a structural module/type boundary.

Verification run:

- `cargo test -p ploke-eval prototype1_state -- --nocapture` passed: 87 tests passed, 0 failed. The run emitted existing dead-code/unused warnings in Prototype 1 and parser code.
