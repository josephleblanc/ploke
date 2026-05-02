Verdict: The commit is a useful local step: the live handoff now computes the active checkout tree key after installing the selected successor and commits an artifact-shaped claim into the sealed block hash. It does not yet encode the stronger admission invariant claimed by the History/Crown model. The new artifact claim is currently a typed wrapper over a caller-supplied `TreeKeyHash`; it is not recoverable through a backend, is not structurally tied to `selected_successor`/`active_artifact`, and successor startup still does not verify the sealed head before entering the parent path. Treat this as "handoff seal includes a tree-key claim under Crown routing", not as implemented artifact eligibility.

Findings
========

1. High: `ArtifactLocator` is tautological and cannot verify the current runtime artifact against sealed History evidence.

   References: `crates/ploke-eval/src/cli/prototype1_state/history.rs:967`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:978`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:987`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:992`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:996`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1047`.

   The comments say `Artifact` is "recoverable through the configured tree/backend boundary" and that a `TreeKeyHash` can be "rechecked as the same tree-key commitment". The implementation does not carry a backend or current checkout context. `locate` ignores the key and returns `Artifact { _private: Private }`; `digest` deterministically hashes the stored key. Consequently, `Verifiable<Artifact, ArtifactLocator>::verify_with(&ArtifactLocator)` can only prove that the stored digest matches the stored key under the same hash function. It cannot answer the important question: "does this runtime's current clean tree equal the successor artifact committed by the sealed History head?"

   This matters because the core invariant in the prompt and docs is artifact eligibility, not just persistence of a hash-shaped field. The module docs explicitly say the target gate is current clean Artifact tree matching the Artifact committed by the sealed head (`history.rs:125`-`134`) and that admitted Artifacts should include backend tree key plus artifact-local manifest digest/reference (`history.rs:167`-`172`). This commit records a tree key, but the locator API does not make the successor compare its own current tree key against the sealed key.

2. Medium: the new artifact claim is not structurally tied to `selected_successor` or `active_artifact`, so the sealed block can contain inconsistent artifact identities.

   References: `crates/ploke-eval/src/cli/prototype1_process.rs:883`, `crates/ploke-eval/src/cli/prototype1_process.rs:892`, `crates/ploke-eval/src/cli/prototype1_process.rs:904`, `crates/ploke-eval/src/cli/prototype1_process.rs:913`, `crates/ploke-eval/src/cli/prototype1_process.rs:1046`, `crates/ploke-eval/src/cli/prototype1_process.rs:1074`, `crates/ploke-eval/src/cli/prototype1_process.rs:1110`.

   The live handoff records the successor as an `ArtifactRef` string derived from node metadata or branch fallback, while the admitted artifact claim uses a `TreeKeyHash` computed from `active_parent_root`. Those are both useful facts, but there is no type or check establishing that the `ArtifactRef` in `SuccessorRef`/`SealBlock.active_artifact` names the same object as the `TreeKeyHash` in `Claims.artifact`. `OperationalEnvironment::artifact(successor_artifact.clone())` records the string in the witness, but the claim's verifiable object is still only the tree key.

   The implementation probably records the intended pair in the common path because `prepare_prototype1_active_successor_runtime` installs and validates the selected branch before `handoff_block_fields` computes the tree key. That is caller-sequencing discipline, not a durable invariant encoded by the block type. A stale `derived_artifact_id`, branch fallback, or future alternate caller can produce a block that hashes cleanly but says "successor artifact B" while admitting tree key A.

3. Medium: artifact claim admission is optional at the transition boundary that seals handoff.

   References: `crates/ploke-eval/src/cli/prototype1_state/history.rs:1847`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1864`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:167`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:195`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:203`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:215`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:218`.

   `SealBlock::from_handoff` still creates `Claims::empty_unchecked()`. The original `seal_block` API remains available and seals exactly those claims. The new `seal_block_with` API lets the caller supply a closure that can return unchanged or otherwise caller-selected `Claims`. This keeps `Crown<Ruling>` in the path, which is good, but the transition does not require an admitted artifact claim before producing `Block<Sealed>`.

   This weakens the "durable records are projections of allowed typed transitions" claim. The live call site happens to add `claims.with_artifact(artifact_claim)`, but the type boundary still allows the same handoff transition to seal without the successor artifact claim. If the intended invariant is now "handoff seal admits current successor artifact", the artifact claim should be required by the handoff/seal carrier rather than injected through a generic closure.

4. Low: docs are mostly honest about the remaining startup gap, but the new comments around `ArtifactLocator` overstate recovery/verification.

   References: `crates/ploke-eval/src/cli/prototype1_state/history.rs:13`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:17`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:60`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:125`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:85`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:183`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:51`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:126`.

   The higher-level docs correctly say successor sealed-head verification is not yet live. The misleading part is local to the new artifact-claim code: comments such as "recoverable through the configured tree/backend boundary" and "rechecked as the same tree-key commitment" read stronger than the implementation. A reviewer or future caller could easily mistake `verify_with(&ArtifactLocator)` for backend artifact verification.

Invariant Coverage
==================

Implemented:

- The live handoff computes a clean tree key from the active checkout after successor installation and before spawning the successor: `prototype1_process.rs:876`-`891`, `prototype1_process.rs:1074`-`1079`.
- The handoff route consumes `Parent<Selectable>` into `Parent<Retired>` and constructs a lineage-bound `Crown<Ruling>`/`Crown<Locked>` path before sealing: `inner.rs:203`-`221`.
- The artifact claim is admitted through `Crown<Ruling>::admit_claim`, flattened into block claims, and committed into the sealed block hash: `history.rs:2418`-`2442`, `history.rs:1989`-`2007`, `history.rs:2192`-`2223`.
- `FsBlockStore::append` verifies the sealed block hash and advances the expected lineage head, preserving local append ordering for the configured store: `history.rs:427`-`438`, `history.rs:630`.

Claimed or documented:

- Crown is local mutable History authority, not pid/branch/path/global consensus: `prototype1_state/mod.rs:151`-`181`, `history.rs:136`-`147`.
- The Crown object does not cross runtimes; the invariant is a shared typed contract plus sealed durable evidence: `history.rs:94`-`107`.
- Startup admission should require current Artifact plus sealed History admission before entering `Parent<Ruling>`: `history.rs:60`-`64`, `prototype1_state/mod.rs:85`-`97`.
- A sealed block should be a projection of typed authority transitions, not a caller-assembled status blob: `history.rs:142`-`147`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:141`-`152`.

Missing or still caller-discipline:

- Successor startup does not verify the sealed head or compare its current clean tree key against the sealed artifact claim.
- The admitted artifact is only a tree-key hash, not tree key plus artifact-local manifest digest/reference.
- `ArtifactRef`, `TreeKeyHash`, selected successor identity, active checkout identity, and parent identity are not one structural object.
- The handoff seal transition can still produce a sealed block with empty claims.
- Ruler identity for `admit_claim` is caller-provided data, as already acknowledged in `history.rs:2413`-`2417`.

Structural Naming Notes
=======================

- `ArtifactLocator` is structurally suspicious because it sounds like a backend recovery capability but is actually a pure hash wrapper over `TreeKeyHash`. If it remains, its name should be narrowed to the actual object, or it should gain the backend/current-checkout context needed to deserve the locator name.
- `seal_block_with` is a weak boundary name. The important semantic operation is not "with a closure"; it is "seal a handoff with admitted claims". The closure hides the domain transition instead of naming it.
- `successor_artifact_ref` and `artifact_key` expose the split artifact model in text: one string ref and one tree key are carried separately through the handoff. A small structural carrier such as a successor artifact commitment would preserve the relationship better than parallel parameters.
- I would not flag `claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<T, L>>>` itself as a flattening problem; despite the generic terms, the nested shape is doing real work and is explicitly documented as the block-claim boundary.

Recommended next patch
======================

Make the handoff artifact commitment a required structural input to sealing. Concretely: introduce a typed carrier for the selected successor artifact that contains the active checkout tree key, artifact ref, selected successor runtime/ref, and the backend/manifest evidence needed to validate it; construct it only after installing and validating the active checkout; require it in the parent handoff seal transition; and have the transition itself emit the admitted artifact claim. Then add the successor-side startup check that loads the sealed head, extracts the artifact claim, recomputes the current clean tree key, and refuses `Parent<Ruling>` if it differs.

Targeted test run:

- `cargo test -p ploke-eval block_claims -- --nocapture`
- Result: passed, 3 tests. The run emitted existing dead-code/unused warnings; no test failure.
