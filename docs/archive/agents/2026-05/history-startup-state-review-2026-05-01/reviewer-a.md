# Prototype 1 History Startup State Review - Reviewer A

Reviewed commit: `fd42a4bc prototype1: validate startup against history state`

## 1. Executive Summary

Verdict: **mostly sound local startup hardening, with one material Crown-contract gap and some documentation drift.**

The commit meaningfully strengthens the live startup path: `Parent<Checked>` can no longer become `Parent<Ready>` without a `Startup<Validated>` token, gen0 startup rejects an existing configured History head, successor startup rejects absent heads, and successor startup recomputes both the current clean tree key and current surface before admission.

The remaining correctness gap is that predecessor startup verifies "this checkout matches the sealed head's admitted artifact tree and surface", but it does **not** verify that the sealed head's `selected_successor` names this incoming invocation/runtime/node/identity. That makes the enforced invariant weaker than the cross-runtime Crown narrative in the docs.

Targeted test run: `cargo test -p ploke-eval prototype1_state -- --nocapture` passed: 87 tests.

## 2. Findings

### Medium: successor startup does not bind the sealed head's selected successor to the incoming runtime

Evidence:
- `crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3081` checks invocation campaign, `:3090` checks invocation node against parent identity, `:3108` checks active root, and `:3118` checks scheduler continuation before loading predecessor startup.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:504` loads the sealed head block, `:507` derives current clean tree key, `:512` verifies the sealed artifact claim, `:515` recomputes current surface, and `:518` verifies the sealed surface.
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:553` then validates only the startup kind/head and the lineage/node/generation copied from the same parent identity (`:580`, `:589`, `:597`).
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:2100` defines `SuccessorRef { runtime, artifact }`, and `:2644` stores it in `SealedBlockHeader`, but there is no startup-side verifier/accessor that checks it against the successor invocation or parent identity.

Why it matters:
The docs claim a cross-runtime typed contract: the predecessor locks handoff material, and a later successor verifies that sealed material before becoming Parent (`history.rs:109`). The current enforced check is narrower: the current checkout must match the head's admitted artifact claim and surface. It does not prove that the incoming invocation/runtime is the selected successor named by the sealed Crown lock. If block construction or future in-crate code writes inconsistent `selected_successor` material, startup admission will not catch it.

### Low: sparse-Merkle implementation and docs are internally inconsistent

Evidence:
- `crates/ploke-eval/src/cli/prototype1_state/history.rs:590` says `LineageState` now includes a sparse-Merkle proof.
- `history.rs:1013` implements `state_map::Map` with `SparseMerkleTree`; `:1041` builds a proof; `:1071` verifies a compiled proof; `:1142` documents `LineageState` as carrying that proof; `:1177` verifies it during append.
- Older docs still say this is "not yet implemented": `history.rs:229` says the store only carries a root digest and `:231` says an authenticated lineage-head map is intended but not implemented; `:235` says `heads.json` is not such a proof.
- `history.rs:2395` also documents `OpenBlock` and still lists "Merkle/authenticated lineage-head map proofs" as not implemented at `:2401`.

Why it matters:
The code now has a local sparse-Merkle proof over the projected `heads.json` map. It is still not distributed consensus and still depends on the local projection, but saying both "implemented" and "not implemented" in the authoritative module docs weakens future audits.

### Low: the `Startup<Validated> -> Parent<Ready>` claim is enforced, but the docs still describe a stronger `Parent<Ruling>` model

Evidence:
- `crates/ploke-eval/src/cli/prototype1_state/parent.rs:432` requires `Startup<Validated>` to produce `Parent<Ready>`.
- `history.rs:83` documents the intended sequence as `Startup<Validated> -> Parent<Ruling>`.
- `history.rs:254` explicitly says live `Parent<Ruling>` as the only writer is not yet enforced.
- `history.rs:3013` exposes Crown operations on `Crown<Ruling>`, but the docs at `:3019` and `:3084` still note that actor identity is supplied as data rather than structurally by `Parent<Ruling>`.

Why it matters:
This is acceptable if treated as a staging model, but the current implementation proves readiness for the existing parent path, not a fully typed ruling-parent admission model. Reports and docs should avoid implying that `Startup<Validated>` currently realizes the whole `Parent<Ruling>` contract.

### Low: naming is improved in the core startup model, but the live process seam still flattens successor state

Evidence:
- The new core names preserve structure well: `Parent<Checked>`, `Startup<Genesis>`, `Startup<Predecessor>`, `Startup<Validated>`, and `Parent<Ready>` in `parent.rs:427`, `:442`, `:481`, and `:553`.
- The surrounding process layer still uses flattened seam names such as `record_prototype1_successor_ready` at `prototype1_process.rs:321`, `validate_prototype1_successor_continuation` at `prototype1_process.rs:384`, and `spawn_and_handoff_prototype1_successor` at `prototype1_process.rs:930`.

Why it matters:
The flattened process names are tolerable as legacy transport helpers, but they should not become the authority model. The missing selected-successor verifier should be modeled as a structured state transition or verifier on the sealed head, not another `*_successor_ready` helper.

## 3. Claimed Invariants Actually Enforced

- `Parent<Checked> -> Parent<Ready>` requires a `Startup<Validated>` value: `parent.rs:432`.
- Gen0 startup is store-scoped: it reads the configured `FsBlockStore`, rejects generation != 0, admits only `StoreHead::Absent`, and rejects an existing head: `parent.rs:447`, `:459`, `:460`, `:468`.
- Successor startup requires a present configured head, loads and verifies the sealed block by expected hash, verifies current clean tree key against the sealed artifact claim, and verifies current surface against the sealed surface: `parent.rs:492`, `:504`, `:507`, `:512`, `:515`, `:518`; `history.rs:793`, `:812`, `:2932`, `:2946`, `:2980`.
- The local state map has a sparse-Merkle proof shape and append verifies the proof, opened state root, lineage, height, and current-head parent relation: `history.rs:1013`, `:1041`, `:1071`, `:1177`, `:1224`.
- `Block<Open>::seal` remains private, and live sealing goes through `Parent<Selectable>::seal_block_with_artifact`, which moves the parent to retired, opens under a private `Crown<Ruling>`, injects the admitted artifact claim, locks the Crown, and seals: `inner.rs:134`, `:158`, `:197`, `:214`, `:217`, `:218`.
- `Parent` and `Startup` data fields are private, so sibling modules cannot directly fabricate `Parent<Ready>` or `Startup<Validated>` by struct literal: `parent.rs:64`, `:80`.

## 4. Weaker Or Aspirational Invariants

- The Crown handoff is not yet fully checked as "this invocation/runtime is the selected successor named by the sealed block"; only tree/surface continuity is checked.
- `ProducedBy(SelfRuntime, CurrentArtifact)` and full `AdmittedBy(CurrentArtifact, Lineage, Policy, History)` are still partial. The current predecessor check proves clean tree and surface against the sealed head; gen0 proves local absence plus checkout identity, not a uniform admission carrier.
- `Parent<Ruling>` is still not the live typed parent state; `Parent<Ready>` is the current path carrier.
- The sparse-Merkle proof is local to the filesystem projection rebuilt from `heads.json`; it is not a global fork-choice rule, process uniqueness proof, or consensus proof.
- Actor/ruler identity in History admission is still supplied as data in parts of the Crown boundary rather than derived from a structural `Parent<Ruling>` carrier.

## 5. Next Patch Recommendations

1. Add a sealed-head startup verifier that binds all predecessor startup facts at once, e.g. `sealed.verify_successor_startup(identity, invocation, current_tree, current_surface, expected_artifact_ref)`. It should verify current artifact claim, current surface, `selected_successor.runtime`, `selected_successor.artifact`, and active artifact consistency.
2. Add accessors or a typed comparison method for `SealedBlockHeader`/`SuccessorRef` only through that verifier; avoid exposing raw header fields as a general API.
3. Add regression tests that build a sealed head with a correct tree/surface but wrong selected successor runtime/artifact and assert predecessor startup rejects it.
4. Update `history.rs` docs at the older 2026-04-30 caveats to say the sparse proof is implemented locally over `heads.json`, while authenticated/global store semantics remain future work.
5. Keep process helpers as transport-only. Put the authority check in `history` or `parent` as a typed startup transition, not in another flattened `prototype1_successor_*` helper.
