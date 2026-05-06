# Status

Prototype 1 now has a real local History/Crown authority spine, but it is still narrow. The implemented authority is a lineage-scoped, single-ruler, tamper-evident chain of sealed blocks stored through `FsBlockStore`; it is not yet a distributed archive, global fork-choice rule, or complete startup/admission system.

The strongest implemented path is the live successor handoff:

- parent prepares a successor artifact and computes a `SurfaceCommitment`;
- parent constructs an `OpenBlock` from the observed `LineageState`;
- `Parent<Selectable>` consumes itself through `seal_block_with_artifact`, which creates a lineage-matching `Crown<Ruling>`, opens the block, admits the artifact claim, locks the Crown, and seals the block;
- the filesystem store appends the sealed block only if the observed store state still matches and the block extends the current local lineage head;
- successor startup reloads the sealed head and verifies current clean tree key plus current surface before it may become `Parent<Ready>`.

This is enough for a HyperAgents-like archive design to say: archive admission must cite sealed History material, not scheduler projections. It is not enough yet for archive-aware parent selection to treat all historical records as authority, because many useful records are still degraded evidence or projections.

# Existing Pieces

## History/Crown authority implemented today

The module-level contract is explicit: History is the durable authority surface for admitted lineage facts, while scheduler snapshots, branch registries, CLI reports, metrics, preview aggregates, and database side tables are projections, caches, or evidence sources. The semantic model is documented as `History = authenticated store over sealed lineage-local blocks`, `Block = one authority epoch`, `Entry = provenance-bearing fact`, `Ingress = late/backchannel observation`, and `Projection = disposable view`. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:52` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:65`.

Implemented block storage is isolated behind `BlockStore`. Its `append` method is the only semantic operation that may advance the lineage head; `heads.json` and the other filesystem indexes are rebuildable projections, not independent authority. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:587` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:617`.

`FsBlockStore::append` verifies the sealed block hash, checks the current `LineageState` against the caller's expected state, verifies append validity, appends the block, writes by-hash and by-lineage-height projections, then updates `heads.json`. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:831` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:875`.

`HistoryStateRoot` is a sparse-Merkle root over the local lineage-head projection. `LineageState` carries that root, proof, and `StoreHead`; append verifies the proof and rejects blocks opened from a different state root. This is a local proof, not distributed consensus. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:994` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:1161` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:1187` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:1196`.

`StoreHead` enforces local chain shape: absent head can only accept genesis height 0 with no parents; present head rejects duplicate genesis, non-consecutive heights, and blocks that do not cite the current head hash. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:1199` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:1285`.

`Crown` is structurally stateful. `Crown<crown::Ruling>` and `Crown<crown::Locked>` have private state markers, the locked state carries `SealBlock`, and `Crown<Ruling>::lock` is private to `inner.rs`. See `crates/ploke-eval/src/cli/prototype1_state/inner.rs:48` through `crates/ploke-eval/src/cli/prototype1_state/inner.rs:155`.

The live authority transition is routed through `LockCrown for Parent<Selectable>`. `seal_block_with_artifact` consumes `Parent<Selectable>`, creates the ruling Crown from the parent's lineage, opens the block, inserts the admitted artifact claim into `SealBlock`, locks the Crown, and seals the block. See `crates/ploke-eval/src/cli/prototype1_state/inner.rs:158` through `crates/ploke-eval/src/cli/prototype1_state/inner.rs:220`.

History construction boundaries are crate-visible but gated: `Crown<Ruling>::open_block`, `open_successor`, `admit_entry`, and `admit_claim` all check Crown lineage before mutating/opening; `Crown<Locked>::seal` is the crate boundary for `Block<Open> -> Block<Sealed>`. The code still records that actor identity is passed as data rather than supplied structurally by `Parent<Ruling>`. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:3023` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:3145`.

Sealed blocks commit their common header, Crown lock transition, selected successor, active artifact, block claims, sealed time, entry count, and entries root into `BlockHash`. Verification recomputes entry hashes and the sealed preimage. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:2652` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:2676` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:2824` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:2939`.

The live handoff writes this authority path. `prepare_prototype1_successor_handoff` creates `SealBlock`, calls `parent.seal_block_with_artifact`, admits the artifact claim with `ArtifactLocator`, then appends the sealed block via `FsBlockStore::append`. See `crates/ploke-eval/src/cli/prototype1_process.rs:1160` through `crates/ploke-eval/src/cli/prototype1_process.rs:1250`.

`handoff_block_fields` chooses genesis vs predecessor opening authority from current `LineageState`, commits the observed `HistoryStateRoot`, computes the clean tree key hash from the active checkout, and embeds the supplied `SurfaceCommitment` into `OpenBlock`. See `crates/ploke-eval/src/cli/prototype1_process.rs:1477` through `crates/ploke-eval/src/cli/prototype1_process.rs:1537`.

## Surface commitments and policy-bearing digest enforcement

`SurfaceCommitment` is a typed partition: `Immutable`, `Mutated`, and `Ambient`. `Immutable` is stored as one root; `Mutated` and `Ambient` are before/after deltas. The type-level markers avoid flattened names and keep partition role structural. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:1371` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:1512`.

The intended policy-bearing surface is the `ploke-eval` code surface containing parent creation, child/successor execution, Crown transitions, History admission, and handoff rules. Ordinary self-improvement must preserve that digest; changing it is a protocol-upgrade/fork candidate, not an ordinary successor transition. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:151` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:185`.

The concrete backend computes `SurfaceRoots` as five hashes: immutable, mutated before/after, ambient before/after. See `crates/ploke-eval/src/cli/prototype1_state/backend.rs:37` through `crates/ploke-eval/src/cli/prototype1_state/backend.rs:80`.

For the git backend, `surface_commitment(before_root, after_root)` hashes tracked files under `crates/ploke-eval` for immutable before/after and rejects if they differ. It hashes the tool-description files as the mutated surface and currently uses an empty ambient declared surface. See `crates/ploke-eval/src/cli/prototype1_state/backend.rs:446` through `crates/ploke-eval/src/cli/prototype1_state/backend.rs:456` and `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1220` through `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1258`.

Child/successor execution uses this backend check. `validate_child_surface` calls `GitWorktreeBackend.surface_commitment`; the controller calls it before child build and after child artifact persistence, and successor installation computes the same commitment before the active checkout handoff. See `crates/ploke-eval/src/cli/prototype1_process.rs:433` through `crates/ploke-eval/src/cli/prototype1_process.rs:444`, `crates/ploke-eval/src/cli/prototype1_process.rs:900` through `crates/ploke-eval/src/cli/prototype1_process.rs:948`, `crates/ploke-eval/src/cli/prototype1_process.rs:2294` through `crates/ploke-eval/src/cli/prototype1_process.rs:2326`, and `crates/ploke-eval/src/cli/prototype1_process.rs:523` through `crates/ploke-eval/src/cli/prototype1_process.rs:616`.

Successor startup recomputes current artifact and surface from the active checkout. It loads the sealed head, verifies the current clean tree key against the sealed artifact claim, computes `surface_commitment(active_parent_root, active_parent_root)`, then verifies it against the sealed block's surface. See `crates/ploke-eval/src/cli/prototype1_state/parent.rs:500` through `crates/ploke-eval/src/cli/prototype1_state/parent.rs:589`.

Tests prove key pieces: sealed-head artifact verification accepts matching tree keys and rejects mismatches; surface commitments affect the block hash; startup surface verification rejects mismatches; block claims are committed to the sealed block hash. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:3993` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:4043`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:4131` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:4177`, and `crates/ploke-eval/src/cli/prototype1_state/history.rs:4240` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:4259`.

## Records and projections that are not sealed History authority

The module docs explicitly warn that mutable files such as `scheduler.json`, `branches.json`, node records, invocation files, ready/completion files, and monitor reports may be cited as evidence or projections, but do not become History authority until admitted into a sealed block or imported as ingress under explicit policy. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:203` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:208`.

The current persisted artifact map lists many useful records under `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`: scheduler, branch registry, evaluations, transition journal, child-plan messages, node summaries, runner request/result, invocations, attempt results, successor-ready/completion files, worktrees, build products, and `.ploke/prototype1/parent_identity.json`. The parent identity witness is committed into the Artifact tree and used by startup validation, but is not yet a full artifact-local provenance manifest. See `crates/ploke-eval/src/cli/prototype1_state/mod.rs:637` through `crates/ploke-eval/src/cli/prototype1_state/mod.rs:688`.

The transition journal preserves typed evidence but is not itself the ontology. Its comments state that child artifact commits, active checkout advancement, and successor handoff entries should be normalized into structural transition facts before History admission. See `crates/ploke-eval/src/cli/prototype1_state/journal.rs:273` through `crates/ploke-eval/src/cli/prototype1_state/journal.rs:353`.

Successor records are also projections: `successor.rs` states that successor is not yet a separate controller role or live `Successor<State>` authority carrier; its records project the handoff path into the append-only transition journal. See `crates/ploke-eval/src/cli/prototype1_state/successor.rs:1` through `crates/ploke-eval/src/cli/prototype1_state/successor.rs:6`.

`history_preview.rs` is read-only and does not write sealed History blocks. It emits History-shaped previews from existing persistence surfaces. See `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1` through `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:6`. Its generated entries are marked `degraded_pre_history`, with notes saying they predate sealed blocks and are preview projections, not admitted History entries. See `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:686` through `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:706`.

Preview imports classify raw documents, evaluations, invocations, runner results, successor-ready/completion files, and similar artifacts as degraded or projection-degraded evidence. See `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1137` through `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1369`. Preview blocks are explicitly `provisional_unsealed` and "not sealed by Crown<Locked>". See `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1578` through `crates/ploke-eval/src/cli/prototype1_state/history_preview.rs:1610`.

# Gaps

The direct archive-relevant gaps are:

- Startup admission is not uniform. Genesis startup is still a configured-store absence check, and predecessor startup validates a sealed head plus current checkout. The intended explicit carrier is still not complete. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:75` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:100`, and current genesis/predecessor code in `crates/ploke-eval/src/cli/prototype1_state/parent.rs:443` through `crates/ploke-eval/src/cli/prototype1_state/parent.rs:589`.
- Actor/ruler identity is still passed as data at key boundaries. `Crown<Ruling>` proves lineage authority, but `OpenBlock` and `admit_claim` still accept actor identity fields from the caller instead of deriving them structurally from `Parent<Ruling>`. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:3023` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:3032` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:3094` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:3098`.
- Artifact-local provenance manifests are still missing. The docs say History should commit the backend tree key plus manifest digest, with large self-evaluation/build/runtime evidence referenced by digest; the current live claim admits the clean tree key and parent identity witness, not a full manifest. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:248` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:253` and `crates/ploke-eval/src/cli/prototype1_state/mod.rs:690` through `crates/ploke-eval/src/cli/prototype1_state/mod.rs:695`.
- Block claims have slots for policy, bounded surface, manifest, and artifact, but the live handoff currently fills the artifact claim; empty claim slots are explicitly not implicit admission. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:2515` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:2539` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:2541` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:2635`.
- Ingress exists structurally, but live ingress capture/import while the Crown is locked is still listed as not enforced. The implemented `Ingress<Open>::import` binds late observation payload to a target block/lineage/height, and `Block<Open>::admit` rejects imports moved to another block, but the live handoff path is not yet routing late/backchannel records through this policy. See `crates/ploke-eval/src/cli/prototype1_state/history.rs:264` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:270` and `crates/ploke-eval/src/cli/prototype1_state/history.rs:3161` through `crates/ploke-eval/src/cli/prototype1_state/history.rs:3294`.
- There is no archive admission record yet. Existing archive-like evidence can be previewed, grouped, and hashed, but the preview module intentionally marks it degraded/provisional. An archive admission that reads `history_preview` output directly would bypass Crown/History.
- Head state is local and partial. `HistoryStateRoot` is a local sparse-Merkle projection over filesystem heads, not distributed consensus, process uniqueness, global canonical state, rollback/finality, or fork-choice. The current persistence gap list says exactly that. See `crates/ploke-eval/src/cli/prototype1_state/mod.rs:706` through `crates/ploke-eval/src/cli/prototype1_state/mod.rs:730`.

# Recommended Next Slice

Do not introduce a new authority system for archive admission. The next slice should make archive admission cite the existing sealed History/Crown spine.

Minimum citation shape for archive admission:

- lineage id and sealed head `BlockHash`;
- block height and predecessor/head relation from `StoreHead`;
- `HistoryStateRoot` observed when the admitted block was opened;
- sealed block hash verification result;
- admitted artifact claim: backend `TreeKeyHash`, `ArtifactLocator` digest, and `Admission`;
- sealed `SurfaceCommitment` roots, especially the immutable `crates/ploke-eval` policy-bearing digest;
- evidence refs and payload hashes for any evaluation/self-improvement material used by selection;
- if evidence arrives after the sealed epoch, the ingress import record and import policy that brought it into a later sealed block.

For archive-aware parent selection, the selection input should distinguish:

- sealed History facts: acceptable as authority citations;
- imported ingress in sealed blocks: acceptable with import policy citation;
- artifact-local manifest refs once implemented: acceptable as reconstructive evidence if the manifest digest is committed by History;
- transition journal, scheduler, branches, invocations, ready/completion files, runner-result latest copies, and preview blocks: evidence/projections only, never admission authority by themselves.

The most useful implementation slice before archive-aware parent selection is therefore:

1. Add a small archive-admission projection over sealed `Block<Sealed>` and imported ingress, not over preview output.
2. Populate or explicitly mark absent the block claim slots needed by archive policy: bounded surface, manifest, policy/procedure material, and artifact.
3. Give archive selection a typed evidence input that carries authority status (`sealed`, `sealed_ingress`, `degraded_evidence`, `projection`) so generation-local successor selection cannot silently rank mutable projections as if they were History facts.
4. Keep bootstrap/predecessor startup admission and artifact provenance manifest work on the critical path; both are directly relevant to whether an archive entry can prove that a parent candidate belongs to the admitted transition system rather than merely looking like a successful runtime in old JSON.
