# Agent 14: History Evidence Access Operator Census

Date: 2026-05-06

Scope:
- Primary: `crates/ploke-eval/src/cli/prototype1_state/history.rs`, `history_preview.rs`, `inner.rs`, `backend.rs`, and `parent.rs`.
- Adjacent direct surfaces: transition journal paths and scheduler/branch/node path helpers used by those modules.
- Source code was not modified.

## Executive Finding

Prototype 1 already has most of the vocabulary that a HyperAgents evidence bundle should reuse. There are two deliberately separate store concepts:

- `history::BlockStore` / `history::FsBlockStore`: authority-bearing sealed History storage. This is where lineage head advancement belongs.
- `history_preview::EvidenceStore` / `history_preview::FsEvidenceStore`: read-only degraded evidence import. This is useful for discovery and preview bundles, but it must not become Crown/History authority.

The closest implemented equivalent to an `EvidenceLocator` is not `EvidencePointer`; it is `history::Locator<T>` plus `Verifiable<T, L>`, `Witnessed<W, X>`, and `claim::Admitted<Admission, X>`. That chain preserves the authority path:

```text
Locator<T> -> Verifiable<T, L> -> Witnessed<RulerWitness, _> -> claim::Admitted<Admission, _> -> sealed Block
```

For child selection/evaluation bundles, extend the existing pieces by role:

- Use `history_preview::EvidenceStore`, `Document`, `Stored<T>`, `EvidencePointer`, and `EvidenceClass` for read-only discovery of files and degraded/pre-History evidence.
- Use `history::Locator<T>` and `block::Claims` for evidence that must become admitted sealed authority.
- Use `inner::File`, `At<F>`, `Message`, `Locked<M>`, and `Received<M>` for typed cross-runtime unpacking operators.
- Use `backend::WorkspaceBackend`, `GitTreeKey`, `TreeKeyCommitment`, and `SurfaceCommitment` for artifact/tree/surface reconstruction.

Do not create a new generic `EvidenceStore` for authority; that name is already occupied by the read-only preview importer and its semantics are intentionally weaker.

## Census Table

| Existing piece | Acts like | Accesses | Source strength / authority | Bundle expansion |
| --- | --- | --- | --- | --- |
| `history_preview::EvidenceStore` (`history_preview.rs:36`) | `EvidenceStore` | `transition_journal()` and `documents()` | Read-only preview importer, no Crown authority | Extend only for degraded evidence discovery. Do not use for sealed admission. |
| `history_preview::FsEvidenceStore` (`history_preview.rs:46`) | Filesystem evidence store adapter | Current campaign layout from `manifest_path` and `prototype_root` | Filesystem evidence/projection reader | Good place to add attempt-scoped child/eval document discovery. Keep authority status explicit. |
| `history_preview::EvidencePointer` (`history_preview.rs:435`) | `EvidenceLocator`/source pointer | `class`, `ref_id`, `path`, optional `line`, content `hash` | Read-only source citation | Extend with richer source coordinates if needed. It should not become a recovery capability. |
| `history_preview::Document` (`history_preview.rs:474`) | `EvidenceDocument` | Raw JSON-ish file plus pointer and parsed `serde_json::Value` | Degraded document evidence | Extend for selection bundles that need raw payload hash plus parsed fields. Not authority by itself. |
| `history_preview::Stored<T>` (`history_preview.rs:458`) | Typed loaded document | Parsed item plus pointer | Degraded typed evidence | Good for transition journal rows and typed preview imports. |
| `history_preview::EvidenceClass` (`history_preview.rs:575`) | Evidence kind classifier | Transition journal, evaluation, invocation, attempt result, successor files, scheduler, branches, node, runner request/result | Classification only; `treatment()` marks projection/degraded status | Extend when adding child bundle file classes. Preserve projection vs degraded vs ingress distinctions. |
| `history_preview::EvidenceIndex` (`history_preview.rs:521`) | Local join/index operator | Node generation, branch generation, branch-to-node from node documents | Preview-only inference | Useful for bundle joins, but any inferred generation should remain weaker than sealed History. |
| `history::BlockStore` (`history.rs:608`) | Authority store port | `append(expected, block)` and `lineage_state(lineage)` | Authority-bearing sealed block store interface | Extend for authority queries, block range reads, entry reads. Do not merge with preview `EvidenceStore`. |
| `history::FsBlockStore` (`history.rs:622`) | Filesystem authority store adapter | `prototype1/history/blocks` and `prototype1/history/index` | Local single-ruler History authority, with rebuildable projections | Extend to load sealed entries and query lineage blocks before archive selection. |
| `history::StoredBlock`, `BlockLocation`, `BlockHead` (`history.rs:913`, `history.rs:920`, `history.rs:978`) | Artifact/History cursors | Block hash, lineage, height, segment line | Store-derived projections of sealed append | Good `ArtifactRef`-like cursors for archive selection. Add accessors before creating another ref type. |
| `history::LineageState`, `HistoryStateRoot`, `StoreHead` (`history.rs:1003`, `history.rs:1163`, `history.rs:1207`) | Head locator/proof | Sparse-Merkle root, proof, absent/present head | Local filesystem predecessor proof, not global consensus | Must be part of authority bundle citations. Do not replace with branch/generation. |
| `history::EvidenceRef` (`history.rs:1345`) | Stable source reference | String reference to payload/evidence | Reference only, not locator | Keep as citation value. Pair with `EvidencePointer` or `Locator<T>` when recovery is required. |
| `history::ArtifactRef` (`history.rs:1359`) | Artifact reference | String identity at block boundaries | Reference only, weaker than tree-key verification | Do not extend into a locator. Use `TreeKeyHash`/`ArtifactLocator` for verification. |
| `history::SurfaceRoot`, `Surface<P>`, `SurfaceDelta<P>`, `SurfaceCommitment` (`history.rs:1397`, `1414`, `1440`, `1463`) | Surface locators/commitments | Immutable, mutated, ambient roots and deltas | Sealed block commitment when included in History; backend-derived before sealing | Extend for richer surface partitions only here or in backend `SurfaceRoots`. |
| `history::Locator<T>` (`history.rs:1573`) | Real `EvidenceLocator`/recovery capability | Locate `T` and compute digest from a key | Capability, not stored data | Extend this for manifest, bounded surface, evaluation bundle, or artifact-local evidence recovery. |
| `history::ArtifactLocator` (`history.rs:1601`) | Artifact locator | `TreeKeyHash -> Artifact` plus digest | Current tree-key-backed artifact verifier | Extend or mirror this pattern for typed manifest/evaluation locators. |
| `history::Verifiable<T, L>` (`history.rs:1626`) | Key + digest evidence object | Locator key and digest | Innermost checked claim, not admission by itself | Reuse for bundle items that need digest-checked recovery. |
| `history::Witnessed`, `RulerWitness`, `Admission`, `claim::Admitted` (`history.rs:1708`, `1772`, `1790`) | Provenance/admission envelope | Witness actor/environment/time and admitting authority/policy/time | Authority only after block claim/entry is sealed | This is the right expansion point for admissible evaluation bundle facts. |
| `history::block::Claims` (`history.rs:2501`) | Typed claim storage | Optional policy, surface, manifest, artifact claims | Sealed header material when block is sealed | Extend claim slots before inventing sibling claim objects. Current live path fills artifact only. |
| `history::Entry<S>` and states `Draft`, `Observed`, `Proposed`, `Admitted` (`history.rs:2328`) | Typed evidence fact | Subject, executor, input/output refs, observation, proposal, admission, payload hash | Authority only in `Entry<Admitted>` inside sealed block | Correct target for child evaluation facts once import/admission policy exists. |
| `history::Ingress<S>` (`history.rs:3183`) | Late evidence import operator | Observation after prior block plus import policy into later block | Typed but not fully live-wired | Use for child/successor files that arrive after Crown lock. |
| `inner::File` and `inner::At<F>` (`inner.rs:19`, `inner.rs:258`) | Filesystem locator schema | Static file type plus runtime params -> path | Addressing/citation, not content authority | Extend for typed files in child/eval bundles instead of ad hoc path strings. |
| `inner::Message`, `Open<M>`, `Locked<M>`, `Received<M>` (`inner.rs:318`, `354`, `447`, `545`) | Typed pack/unpack operator | Cross-runtime message body in a static message box | Typestate transport capability | Reuse for bundle handoff/unpacking where receipt must be proven. |
| `parent::ChildPlanFiles`, `ChildFiles`, `ChildPlanFile`, `SchedulerFile`, `BranchesFile`, `NodeFile`, `RunnerRequestFile` (`parent.rs:102`, `194`, `201`, `253`, `266`, `279`, `292`) | Existing child bundle locator set | Child-plan message, scheduler, branch registry, node records, runner requests | Typed transport/projection, not History authority | Extend this family for child selection/evaluation bundle manifests before creating a generic bundle path map. |
| `backend::WorkspaceBackend` (`backend.rs:321`) | Artifact/tree/surface store adapter | Workspaces, branches, clean tree keys, surface commitments | Backend-derived operational authority, admitted only through History | Extend associated operations for artifact/evidence reconstruction. |
| `backend::GitTreeKey` and `history::TreeKeyCommitment` (`backend.rs:135`, `history.rs:2031`) | Tree locator/key commitment | Clean git tree key -> `TreeKeyHash` | Strong artifact identity input when derived by backend | Prefer this over branch name, path, or caller-authored string. |
| `backend::SurfaceRoots` (`backend.rs:37`) | Filesystem surface store adapter output | Immutable/mutated/ambient hashes | Backend-only constructor prevents fabrication | Extend here when surface hashing changes. |
| `intervention::RecordStore` and `journal::PrototypeJournal` (`intervention/algebra/mod.rs:92`, `journal.rs:655`) | Append-only evidence writer | Transition journal JSONL | Durable evidence stream, not sealed History | Good producer of degraded evidence. Do not treat append as admission. |
| `intervention::Prototype1NodeRecord`, `Prototype1RunnerRequest`, path helpers (`scheduler.rs:117`, `174`, `240`) | Projection documents and locators | Node mirror, runner request, scheduler/node paths | Scheduler projection / execution input | Useful for bundle assembly, but selection should cite stronger attempt/evaluation refs and History status. |

## Read-Only Projection vs Authority

`history_preview.rs` states its role directly: it reads current persistence surfaces, preserves source refs and payload hashes, and emits a History-shaped preview without writing sealed blocks. Its `EvidenceStore` trait is intentionally consumer-shaped and names only the reads needed by the preview importer (`history_preview.rs:32` to `history_preview.rs:42`). `FsEvidenceStore::documents()` walks scheduler, branch registry, evaluations, invocations, attempt results, successor-ready/completion files, node records, runner requests, and latest runner results (`history_preview.rs:70` to `history_preview.rs:123`).

That is exactly the right shape for a child selection/evaluation bundle collector if the bundle is labeled `degraded_evidence` or `projection`. It is the wrong shape for archive admission or Crown authority.

The authority path is `history::BlockStore::append()` and the Crown-gated constructors. `FsBlockStore::append()` verifies the sealed block hash, checks the caller's expected `LineageState`, verifies append against `StoreHead`, writes the block and indexes, and updates heads (`history.rs:831` to `history.rs:875`). `LineageState::verify_append()` verifies the sparse proof and rejects a block opened from a different `HistoryStateRoot` (`history.rs:1187` to `history.rs:1195`). `StoreHead::verify_append()` enforces genesis and predecessor shape (`history.rs:1234` to `history.rs:1285`).

For archive-aware child evaluation, source strength should be encoded like:

- `sealed`: `Block<Sealed>` verified through `BlockStore`, with relevant `Entry<Admitted>` or `block::Claims`.
- `sealed_ingress`: `Ingress<Imported>` whose proposed entry was admitted into a sealed block.
- `backend_verified`: `WorkspaceBackend::clean_tree_key`, `surface_commitment`, or `verify_artifact_target`, cited by a sealed claim or entry.
- `degraded_evidence`: `EvidencePointer`, `Document`, `Stored<T>`, transition journal row, attempt result, evaluation report.
- `projection`: scheduler, branch registry, node mirror, latest runner result, preview block, report output.

## Existing Typed Unpacking Operators

The code already has typed unpacking machinery in `inner.rs`; adding a new "bundle unpacker" would duplicate it.

`File` gives each protocol file a static address schema. `At<F>` resolves runtime params into a concrete path and serializes as that path. `MessageBox` ties a file to one lock transition and one unlock transition (`inner.rs:19` to `inner.rs:45`, `inner.rs:258` to `inner.rs:312`).

`Message` defines a body, valid sender/receiver transitions, and receiver validation. `Open<M>::lock()` consumes the sender role/state and writes the body. `Locked<M>::from_box()` reads a body from the typed box. `Locked<M>::unlock()` validates the exact receiver and returns both the post-read state and `Received<M>`. `Received<M>` is already a capability proving the cross-runtime message was consumed (`inner.rs:318` to `inner.rs:350`, `inner.rs:371` to `inner.rs:432`, `inner.rs:454` to `inner.rs:503`, `inner.rs:543` to `inner.rs:559`).

`parent::ChildPlan` is the concrete example. `ChildPlanFiles` is a small bundle: message path, scheduler path, branch registry path, parent node, child generation, and per-child `ChildFiles` with node and runner request paths (`parent.rs:102` to `parent.rs:169`). It validates that the receiver is the same parent and that the direct-child generation rule still holds (`parent.rs:171` to `parent.rs:189`). The file schemas `ChildPlanFile`, `SchedulerFile`, `BranchesFile`, `NodeFile`, and `RunnerRequestFile` already provide typed path locators (`parent.rs:219` to `parent.rs:300`).

If a future child-selection/evaluation bundle needs a durable transfer file, it should extend this `File`/`Message`/`Received` pattern. If it only needs a read-only import of existing files, it should extend `history_preview::FsEvidenceStore`.

## Tree, Artifact, and Surface Access

`WorkspaceBackend` is already the filesystem/backend store adapter for artifact realization and verification. Its associated `TreeKey` is the backend-owned key for the exact clean Artifact tree hosted by a checkout root (`backend.rs:321` to `backend.rs:330`). `clean_tree_key()` is explicitly the operation History admission should rely on, not caller-authored strings (`backend.rs:438` to `backend.rs:444`).

For git, `GitWorktreeBackend::clean_tree_key()` rejects dirty checkouts, runs `git rev-parse HEAD^{tree}`, parses the object id into `GitTreeKey`, and then `TreeKeyCommitment for GitTreeKey` produces `TreeKeyHash` (`backend.rs:1185` to `backend.rs:1215`, `history.rs:2031` to `history.rs:2039`). `ArtifactLocator` then verifies the admitted artifact claim from that `TreeKeyHash` (`history.rs:1601` to `history.rs:1618`).

Surface access is also already typed. Backend-only `SurfaceRoots` has private fields and constructor, so sibling modules cannot fabricate roots (`backend.rs:30` to `backend.rs:80`). `GitWorktreeBackend::surface_commitment()` hashes tracked `crates/ploke-eval` files as immutable, rejects immutable changes, hashes tool-description files as mutated, and currently hashes an empty ambient surface (`backend.rs:1217` to `backend.rs:1258`). `SurfaceCommitment::verify_current()` compares immutable, mutated-after, and ambient-after roots (`history.rs:1469` to `history.rs:1512`).

Startup already consumes these operators. `Startup<Predecessor>::from_history()` loads the sealed head, derives the current clean tree key, verifies the artifact claim with `ArtifactLocator`, recomputes current surface, and verifies it against the sealed head (`parent.rs:500` to `parent.rs:589`). This is the current strongest example of a typed unpacking/evidence access path across runtime boundaries.

## What Should Be Extended

Extend `history::Locator<T>` when the object must be recoverable and digest-checked under History admission. Candidate targets:

- `Manifest`: artifact-local provenance manifest. The marker already exists, and `block::Claims` already has a `manifest` slot.
- `surface::Bounded`: bounded surface material. `block::Claims` already has a `surface` slot, while `SurfaceCommitment` covers partition roots.
- Evaluation bundle document/root: if child selection/evaluation evidence becomes admissible, model it as a typed `T` with `Locator<T>`, not as a stringly `EvidenceRef`.

Extend `history::block::Claims` before adding a sibling "archive claim" object. It already has policy, surface, manifest, and artifact accessors that reconstruct `claim::Admitted<Admission, Witnessed<RulerWitness, Verifiable<T, L>>>` (`history.rs:2501` to `history.rs:2635`). The live path only fills artifact today, so the gap is population and policy, not absence of a claim abstraction.

Extend `history::FsBlockStore` for authority reads before building archive selection. Today `sealed_head_block()` rejects non-empty stored blocks because entry loading lacks its own transition (`history.rs:797` to `history.rs:824`, `history.rs:950` to `history.rs:973`). Archive selection needs a verified read path for sealed entries and claims; it should be added to `BlockStore`/`FsBlockStore`, not to `history_preview::EvidenceStore`.

Extend `history_preview::FsEvidenceStore` only for non-authority discovery. If selection needs to gather `nodes/*/results/<runtime-id>.json`, evaluation reports, branch registry, runner request, invocation, streams, or future protocol artifacts, the preview importer is the existing filesystem adapter. Its output must carry `EvidenceClass` and source strength into selection.

Extend `parent::ChildPlanFiles` or define a sibling `Message` using the same `inner::Message` pattern for any child bundle that must cross runtime roles. Do not pass free `Vec<PathBuf>` bundles where `At<F>` can preserve file role and receiver validation.

## What Should Not Be Promoted

`EvidencePointer` should not become a universal locator. It is a source pointer and hash. A locator must be able to recover and digest-check a typed object through context; that is `Locator<T>`.

`ArtifactRef` should not become artifact authority. It is a recoverable identity string used at block boundaries. The authority path is backend `TreeKey`, `TreeKeyCommitment`, `TreeKeyHash`, `ArtifactLocator`, and an admitted sealed claim.

`history_preview::HistoryPreview` and preview entries should not feed archive selection as sealed facts. They are useful operator-facing projections and degraded evidence join outputs.

Scheduler, branch registry, node records, latest runner result, and child-plan paths should not be treated as admitted child success. They can locate evidence and support planning, but selection/evaluation bundles need attempt-scoped result/evaluation refs plus explicit source strength.

## Bundle Implications

A child selection/evaluation bundle can be assembled today as a layered object without new authority primitives:

1. Address layer: `At<F>` file roles from `parent.rs` plus scheduler path helpers in `intervention/scheduler.rs`.
2. Discovery layer: `history_preview::FsEvidenceStore::documents()` and `transition_journal()`.
3. Citation layer: `EvidencePointer`, `Stored<T>`, `Document`, `EvidenceRef`.
4. Backend verification layer: `WorkspaceBackend::clean_tree_key()`, `TreeKeyCommitment::tree_key_hash()`, `surface_commitment()`, `verify_artifact_target()`.
5. Authority layer: `Locator<T>`, `Verifiable<T, L>`, `Witnessed`, `Admission`, `block::Claims`, `Entry<Admitted>`, sealed `Block<Sealed>`, and `StoredBlock`.

The missing object is not a generic `EvidenceStore`; it is a typed selection/evaluation bundle record that carries the above layers without collapsing them. Its key field should be authority status, because the same file may be useful as degraded evidence before it is admitted and authoritative only after a specific sealed block or ingress import cites it.

## Recommended Next Slice

Add a verified History read/query path first:

- `BlockStore` should grow read operations for sealed blocks by `BlockHead`/`BlockHash` and eventually lineage ranges.
- `FsBlockStore` should get an entry-loading transition for `Block<Sealed>` rather than `Deserialize` on authority carriers.
- The query result should expose `StoredBlock`, `BlockHead`, claims, entries, and verification status.

Then add a child evidence bundle projection that does not claim authority:

- Source it from `history_preview::FsEvidenceStore`.
- Include `EvidencePointer` hashes and `EvidenceClass`.
- Include path roles using `At<F>` where available.
- Include authority status for each item.

Finally, admit the subset of bundle facts needed for archive selection through existing History machinery:

- Use `Locator<T>` implementations for manifest/evaluation bundle recovery.
- Use `Crown<Ruling>::admit_claim()` or `Crown<Ruling>::admit_entry()`.
- Store the result in `block::Claims`, `Entry<Admitted>`, or `Ingress<Imported>` depending on timing.

This keeps read-only evidence access, typed unpacking, backend verification, and Crown authority as separate layers instead of hiding them behind one new abstraction.
