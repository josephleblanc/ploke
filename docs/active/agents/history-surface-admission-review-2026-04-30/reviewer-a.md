# Reviewer A: history surface admission

Reviewed commit: `2ba2f54e history: enforce surface admission boundaries`.

## Findings

### High: sealed-head/surface admission is not the universal `Parent` startup gate

Invariant claim: the docs state that ordinary descendants preserve the `ploke-eval` policy-surface digest and that incompatible code is outside the admitted transition system and may not enter the History/Crown mutation path (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:174`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:190`; also `crates/ploke-eval/src/cli/prototype1_state/mod.rs:111`). That is the right conceptual model for the cross-runtime Crown contract.

What the code enforces: the live successor handoff path does call `validate_prototype1_successor_history_admission` before recording readiness (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3118`), and that validation checks both the sealed artifact tree and surface (`crates/ploke-eval/src/cli/prototype1_process.rs:419`, `crates/ploke-eval/src/cli/prototype1_process.rs:427`). But if no handoff invocation is supplied, startup returns `Parent<Ready>` directly (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3065`). Initial setup similarly validates only the parent checkout/identity, not a History admission carrier or surface root (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:211`). So the claim is true for the explicit handoff path, not for every path that can become a ready parent.

Patch to close: introduce a `Startup<Validated>` or equivalent admission carrier and make `Parent<Checked> -> Parent<Ready>` consume it. The carrier should have explicit variants for bootstrap absence proof and predecessor sealed-head proof, both including surface validation. Until then, narrow the cross-runtime claim to "live successor handoff descendants that enter through handoff invocation."

### Medium: `SurfaceCommitment` is a caller-supplied fact, not a sealed validation product

Invariant claim: the immutable/mutated/ambient commitment should be a static validation witness computed from Artifacts before candidate execution (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:213`). Ordinary succession must disallow `ploke-eval` mutation, and the block should commit to the resulting surface (`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:86`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:91`).

What the code enforces: the backend computes and rejects immutable-surface changes (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1164`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1185`). However, the History boundary accepts a plain `SurfaceCommitment` in crate-visible `OpenBlock`/`OpenSuccessorBlock` fields (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2152`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2879`), while `SurfaceRoot::new`, `Surface::new`, `SurfaceDelta::new`, and `SurfaceCommitment::new` are all `pub(crate)` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1152`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1171`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1196`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1220`). `Crown<Ruling>::open_block` checks lineage only (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2762`), so crate-local callers can mint arbitrary surface commitments and still open/seal a block.

Patch to close: make the constructors module-private or seal them behind a backend-produced `Surface<Validated>`/`ArtifactTransition<Validated>` carrier. Prefer an `OpenBlock::from_validated_transition(...)` boundary that accepts tree key, artifact claim material, and surface commitment computed from the same artifact roots, rather than a public field bag.

### Medium: surface roots are filesystem snapshots, not committed Artifact-tree roots

Invariant claim: admitted Runtimes are `ProducedBy(Artifact)`, and startup/static validation should check recoverable Artifact commitments (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:75`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:215`). The implementation comments also describe surface commitments as computed from Artifacts (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2426`).

What the code enforces: `surface_commitment` uses `git ls-files` only to enumerate the immutable pathspec, then reads current filesystem bytes (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1260`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1291`). Mutated paths are hardcoded from `ToolName::ALL` and are read from the filesystem without a tracked-tree check (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1284`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1303`). There is no cleanliness or tree-object check inside `surface_commitment` itself. That leaves the commitment partly dependent on working-tree state and caller sequencing rather than the committed Artifact tree.

Patch to close: compute surface roots from the git tree/commit that is the Artifact identity, or require and verify clean worktrees plus tracked membership for every declared partition before hashing. The commitment should carry or be paired with the tree key it was derived from.

### Low: local docs still contain a stale surface-admission caveat

`OpenBlock`'s doc comment still lists "live surface-commitment admission" as not implemented (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2148`), while the module and workflow docs say the live successor handoff now computes, commits, and checks the surface (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:101`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:226`). If the intended distinction is "block opening does not validate surface itself," say that explicitly; otherwise remove the stale caveat.

## Accurately encoded claims

- The Crown constructor barrier is materially stronger than before: `Crown<S>` fields are private (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:85`), `Crown<crown::Ruling>::for_lineage` is private (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:134`), and production locking/sealing is routed through `Parent<Selectable>` via `LockCrown::seal_block_with_artifact` (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:197`).
- `Block<Open>::seal` includes `BlockCommon`, so the new `surface` field is committed into the sealed block hash (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2179`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2562`).
- Successor startup recomputes current tree/surface and compares them to the sealed head on the handoff path (`crates/ploke-eval/src/cli/prototype1_process.rs:419`, `crates/ploke-eval/src/cli/prototype1_process.rs:427`; `crates/ploke-eval/src/cli/prototype1_state/history.rs:2686`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2720`).
- The partition naming mostly preserves structure: `Surface<surface::Immutable>`, `SurfaceDelta<surface::Mutated>`, and `SurfaceDelta<surface::Ambient>` avoid flattened names like `immutable_surface_root_before` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1130`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1214`).

## Deferred gaps that are documented

- Uniform bootstrap/predecessor admission is still explicitly deferred (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:85`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:700`).
- Authenticated lineage-head maps, process uniqueness, signatures, and consensus are still out of scope and documented as such (`crates/ploke-eval/src/cli/prototype1_state/history.rs:242`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:39`).
- `Parent<Ruling>`/actor identity is still caller data rather than a structural source for `opened_by`, `ruling_authority`, and claim admission; this is documented in the History methods (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2757`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2823`).

## Naming and structure audit

- `validate_prototype1_successor_history_admission` names the desired result but remains a process helper; the missing structural object is `Startup<Validated>` or an equivalent admission carrier.
- `SurfaceCommitment` currently implies more authority than the type enforces. Until the constructors are sealed, a name like `SurfaceRoots` or a validated-state parameter would better reflect the distinction.
- The legacy `policy_ref` field is still present but correctly documented as non-authoritative (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2148`). Avoid building new names around `PolicyRef` as if it were the authority source.

Validation: static review only; I did not run tests.
