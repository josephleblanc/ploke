# Surface Roots And Artifact Tree Audit

Date: 2026-04-30

Scope: Prototype 1 surface-root computation, its relation to Artifact/Runtime/Crown invariants, and whether the current implementation should block the full loop.

## Summary

Surface roots today are not committed Git tree-object digests. They are deterministic SHA-256 commitments over selected working-tree file bytes, with Git used only for part of the path enumeration. The committed clean tree object is computed separately by `clean_tree_key`.

The current implementation is therefore a combination:

- Git tree-object digest for Artifact admission: `git status --porcelain --untracked-files=all` must be clean, then `git rev-parse HEAD^{tree}` is parsed as `GitTreeKey` (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1185`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1194`).
- Surface roots for policy-surface continuity: `crates/ploke-eval` paths are enumerated with `git ls-files`, but bytes are read from the filesystem (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1222`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1229`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1315`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1346`).
- Mutated surface roots are not Git-enumerated. They come from `ToolName::ALL` and are read from the filesystem (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1245`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1339`).
- Ambient roots are the hash of an empty declared surface (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1248`).

This mostly matches the latest concrete hardcoded surface docs, but the phrase "computed from Artifacts" is ambiguous unless read as "computed from checked-out Artifact worktrees" rather than "computed from committed tree objects."

## Documented Implemented Invariants

The module-level Artifact/Runtime model says every checkout is an Artifact, a worktree path is only a handle, and every Artifact is a dehydrated Runtime (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:40`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:46`). It also says a predecessor handoff uses a sealed History head naming the current clean artifact tree and, as of 2026-04-30, computes a partitioned surface commitment before successor execution and validates it for child/successor paths (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:95`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:101`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:106`).

The concrete partition is implemented as documented: immutable is `crates/ploke-eval`, mutated is all tool-description files, ambient is empty, and immutable changes reject ordinary succession (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1217`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1238`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1245`). The docs state the same hardcoded partition and hashing scheme (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:221`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:224`; `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:91`).

The sealed block commits to the surface because `OpenBlock.surface` is copied into `BlockCommon`, and `BlockCommon` is part of `SealedBlockPreimage` used for `block_hash` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2168`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2195`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2578`). There is an explicit regression test that changing the surface changes the block hash (`crates/ploke-eval/src/cli/prototype1_state/history.rs:3819`).

The successor startup path checks both layers: it derives the current clean tree key, verifies the sealed artifact claim, recomputes the current surface from the active checkout, and compares it against the sealed surface (`crates/ploke-eval/src/cli/prototype1_process.rs:419`, `crates/ploke-eval/src/cli/prototype1_process.rs:424`, `crates/ploke-eval/src/cli/prototype1_process.rs:427`, `crates/ploke-eval/src/cli/prototype1_process.rs:430`). The History-side comparison checks immutable, mutated-after, and ambient-after roots (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1247`).

Child paths also validate the same partition before build/hydration and after artifact persistence in the inspected live paths (`crates/ploke-eval/src/cli/prototype1_state/cli_facing.rs:3301`, `crates/ploke-eval/src/cli/prototype1_process.rs:798`, `crates/ploke-eval/src/cli/prototype1_process.rs:1839`).

## Documented Aspirational Or Deferred Invariants

The uniform startup admission carrier is still deferred. The History docs say startup should establish `ProducedBy(SelfRuntime, CurrentArtifact)` and `AdmittedBy(CurrentArtifact, Lineage, Policy, History)` before entering `Parent<Ruling>`, but mark this as intended rather than complete (`crates/ploke-eval/src/cli/prototype1_state/history.rs:65`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:71`). The v2 docs repeat that bootstrap/non-handoff startup still lacks the same admission shape (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:85`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:98`).

Artifact identity is also still partial. The History docs say admitted Artifacts should eventually commit to backend tree key plus artifact-local manifest digest (`crates/ploke-eval/src/cli/prototype1_state/history.rs:226`). The older handoff note says the same and explicitly records that `FsBlockStore` lacks an authenticated lineage-head map (`docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:123`, `docs/workflow/evalnomicon/drafts/prototype1-history-handoff-2026-04-29.md:159`).

Protocol upgrades that mutate `crates/ploke-eval` are deferred. Current docs require ordinary succession to preserve the policy-bearing `ploke-eval` digest until an explicit upgrade/fork transition exists (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:123`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:174`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:195`).

Distributed consensus, cryptographic signatures, and OS-process uniqueness are explicitly outside the current claim (`crates/ploke-eval/src/cli/prototype1_state/history.rs:242`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:111`).

## Ambiguous Or Stale Claims

`history.rs` has a stale local statement: it says current live code computes the commitment before successor execution but child execution still relies on the older bounded-target validation path (`crates/ploke-eval/src/cli/prototype1_state/history.rs:215`). The live code now performs child surface validation before build and after persistence, and newer docs say the same (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:226`). This should be narrowed or updated.

"Computed from Artifacts" is easy to overread. The backend trait says `surface_commitment` computes from checked-out Artifacts (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:446`), while the implementation actually reads current filesystem bytes after enumerating only the immutable pathspec via Git. That is a checked-out worktree snapshot of declared paths, not a Git object-level surface root.

"All tool-description text files" is implemented as the current `ToolName::ALL` relpath list, not as a Git-tracked/pathspec enumeration (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1339`). If a listed tool-description file is missing, the surface root fails; if there are additional undeclared tool-description-like files, they are not included.

## Code Barriers And Data Sources

The tree-key barrier is relatively strong for the current Git backend. `GitTreeKey` is backend-owned, derived from a clean checkout, and converted into `TreeKeyHash` through the private `TreeKeyHash::from_serialized_key` path exposed by `TreeKeyCommitment` only for backend key types (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:136`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1766`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1777`). Successor verification compares the current key with the sealed artifact claim (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2702`).

The surface-root construction barrier is improved but not a full proof of backend derivation. `SurfaceRoot::new`, `Surface::new`, and `SurfaceDelta::new` are private, and normal crate-visible construction goes through `SurfaceCommitment::from_backend_roots` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1152`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1171`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1196`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:1233`). `SurfaceRoots` has private fields and a private constructor in the backend module (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:37`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:45`).

However, `SurfaceCommitment` derives `Deserialize` and is crate-visible, because sealed blocks must be loaded and verified (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1213`). That means the type itself should be read as "surface roots with block verification semantics," not as an unforgeable capability proving fresh backend derivation. The live path uses the backend correctly; the type boundary does not fully encode that provenance.

The child surface gate is also not yet a typed role/state carrier. `validate_child_surface` returns a raw `SurfaceCommitment` (`crates/ploke-eval/src/cli/prototype1_process.rs:435`), and `history.rs` explicitly lists structural type-state representation of the child/successor surface gate as not yet enforced (`crates/ploke-eval/src/cli/prototype1_state/history.rs:242`).

## Full Loop Expectation

I expect the current full loop to complete with respect to surface-root computation when the ordinary Prototype 1 assumptions hold: the candidate only mutates declared tool-description files, `crates/ploke-eval` is unchanged, listed tool-description files exist, and the active checkout is clean when tree-key admission runs. The tests encode the core intended behavior: tool text mutation is allowed, `ploke-eval` mutation is rejected, and missing immutable surface rejects (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1685`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1696`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1708`).

Surface-root computation should stop the loop if a candidate mutates `crates/ploke-eval`, if declared files are missing, or if the successor starts from an active checkout whose recomputed roots do not match the sealed head. The separate tree-key path should stop the loop when the active checkout is dirty or does not match the sealed artifact claim.

The implementation is therefore adequate for the current hardcoded ordinary-succession loop, but it does not yet satisfy the stronger Artifact/Runtime invariant as a single structural object. The current proof is assembled from sequencing: commit/install a successor Artifact, derive a clean tree key, seal a block with that tree key and a separately computed surface commitment, then require successor startup to recompute both. It is not yet one backend-derived `ArtifactSurface` or `Artifact<Admitted>` carrier that ties tree key, manifest, and surface roots together.
