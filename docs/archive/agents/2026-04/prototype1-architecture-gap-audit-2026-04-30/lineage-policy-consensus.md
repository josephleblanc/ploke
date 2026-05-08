# Prototype 1 Lineage, Policy, And Consensus Gap Audit

Recorded: 2026-04-30

## Scope

Audit focus: multi-lineage/authenticated head-map claims, distributed consensus/process uniqueness/signatures/global fork choice, and `PolicyRef` versus runtime/code-surface policy.

Primary sources reviewed:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md`
- `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md`
- Adjacent live wiring in `crates/ploke-eval/src/cli/prototype1_process.rs`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs`, and `crates/ploke-eval/src/cli/prototype1_state/backend.rs`.

## Implemented Invariants

The current implementation supports a local, lineage-scoped, tamper-evident History core. The main module explicitly says the current claim is "tamper-evident, transition-checked local History" and not distributed consensus or proof of LLM correctness (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:77`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:81`). The History module narrows the current claim the same way: it defines and partially enforces tamper-evident, lineage-scoped, transition-checked History, and does not upgrade legacy JSON records into authority (`crates/ploke-eval/src/cli/prototype1_state/history.rs:250`).

Concrete barriers are present for the narrow local claim:

- `Block<Open>::seal` is private to the module and sealing is exposed through `Crown<Locked>::seal`, with lineage matching before sealing (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2570`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2872`).
- Open-block construction is private and crate-visible opening is routed through `Crown<Ruling>::open_block`, which checks the Crown lineage against the requested block lineage (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2477`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2768`).
- Entry admission is routed through `Crown<Ruling>::admit_entry`, not a free-standing public status write, and checks ingress import lineage/block/height plus duplicate entry ids (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2528`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2809`).
- `Crown<Ruling>` construction is private; `Parent<Selectable>` is the production transition source that can retire the parent, lock the Crown, admit the artifact claim, and seal the block (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:134`, `crates/ploke-eval/src/cli/prototype1_state/inner.rs:188`).
- Parent role states are typed as `Parent<Unchecked>`, `Parent<Checked>`, `Parent<Ready>`, `Parent<Planned>`, `Parent<Selectable>`, and `Parent<Retired>` rather than flattened status strings (`crates/ploke-eval/src/cli/prototype1_state/parent.rs:24`, `crates/ploke-eval/src/cli/prototype1_state/parent.rs:48`).
- The live handoff seals and appends a History block before successor runtime spawn (`crates/ploke-eval/src/cli/prototype1_process.rs:1010`, `crates/ploke-eval/src/cli/prototype1_process.rs:1033`).
- The successor admission gate rejects startup without a sealed head, loads the sealed head, verifies the current clean tree key against the artifact claim, then verifies the current surface commitment against the sealed head (`crates/ploke-eval/src/cli/prototype1_process.rs:397`, `crates/ploke-eval/src/cli/prototype1_process.rs:416`, `crates/ploke-eval/src/cli/prototype1_process.rs:424`, `crates/ploke-eval/src/cli/prototype1_process.rs:430`).
- Sealed block verification recomputes entry count, entries root, and block hash from deterministic preimage material (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2647`).

The filesystem store implements only a local head projection, but it does enforce useful local append checks. `BlockStore::append` consumes an expected `StoreHead`, re-reads the current head, rejects stale heads, verifies append legality, appends the sealed block, and then updates rebuildable indexes (`crates/ploke-eval/src/cli/prototype1_state/history.rs:793`). `StoreHead::verify_append` rejects non-genesis appends without a head, duplicate genesis, nonconsecutive height, and blocks that do not cite the current head (`crates/ploke-eval/src/cli/prototype1_state/history.rs:985`).

The runtime/code-surface policy is partly implemented. `SurfaceCommitment` structurally carries immutable, mutated, and ambient partitions (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1206`). Backend construction hardcodes `crates/ploke-eval` as immutable, all tool-description text files as mutated, and empty ambient; it rejects immutable-surface changes before creating the commitment (`crates/ploke-eval/src/cli/prototype1_state/backend.rs:1217`, `crates/ploke-eval/src/cli/prototype1_state/backend.rs:1237`). Startup verification checks immutable/current mutated-after/current ambient-after roots against the sealed head (`crates/ploke-eval/src/cli/prototype1_state/history.rs:1247`).

## Aspirational Or Deferred Invariants

The authenticated multi-lineage store is explicitly deferred. The History module says the store should eventually maintain an authenticated lineage-head map, likely Merkle-Patricia or equivalent, with present and absent proofs; the current `heads.json` projection is not such a proof (`crates/ploke-eval/src/cli/prototype1_state/history.rs:219`). The main module repeats that an authenticated lineage-head map is missing and current code only has local parent links and rebuildable head projections (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:700`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:713`).

Distributed consensus, signatures, and process uniqueness are deferred. The History docs list cryptographic signatures and distributed consensus as not enforced (`crates/ploke-eval/src/cli/prototype1_state/history.rs:242`). The older Crown draft says today's model does not provide distributed consensus, compromised-local-file protection, global authority across unrelated lineages, or OS-process uniqueness (`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:111`). It also says "signed" currently means content-addressed and sealed by the Crown-lock authority transition; cryptographic signing is a later extension (`docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:126`, `docs/workflow/evalnomicon/drafts/history-blocks-and-crown-authority.md:151`).

Global fork choice/finality is also deferred. The v2 note distinguishes local History from complete History and says future consensus may admit blocks from multiple local rulers; current local Crown blocks should not be described as global finality (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:354`). The code reserves multi-parent ancestry by storing `parent_block_hashes` as a list, but states that branch merges and consensus extensions are forward-facing, not implemented finality (`crates/ploke-eval/src/cli/prototype1_state/history.rs:368`).

Uniform startup admission remains incomplete. The intended startup shape is `ProducedBy(SelfRuntime, CurrentArtifact)` plus `AdmittedBy(CurrentArtifact, Lineage, Policy, History)` (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:85`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:83`). Current successor handoff has a live gate, but bootstrap/non-handoff startup still lacks the same admission carrier (`crates/ploke-eval/src/cli/prototype1_state/history.rs:130`, `docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:95`).

Artifact-local provenance manifests are deferred. The main module says `.ploke/prototype1/parent_identity.json` is only an artifact-carried parent identity witness, not a full provenance manifest (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:673`). It then says admitted Artifacts should eventually carry or reference a provenance manifest committed by both the Artifact tree and admitting History block (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:678`).

## Ambiguous Or Overstatement Risks

The main documentation now mostly avoids overclaiming. It explicitly warns that History is a global authenticated substrate over lineage-local authority chains as an intended model, not a single global chain (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:32`), and says `Crown<Locked>` does not make local execution globally trustworthy (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:77`). The v2 conceptual document similarly says Prototype 1 lacks distributed consensus, authenticated head-map proofs, full policy/finality semantics, and uniform startup admission (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:39`).

The remaining overstatement risk is reader confusion around words like "authenticated substrate", "blockchain", and "policy". In v2, a blockchain is defined as an authenticated, append-only evidence/authority structure with head-state proofs and policy/finality rules (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:20`). That is a model definition, not the current filesystem implementation. The current code's `StoreHead` comment correctly narrows `Absent` to local checked-store absence, not global absence (`crates/ploke-eval/src/cli/prototype1_state/history.rs:950`). Any status document that quotes only the blockchain definition without the current implementation note would overstate current guarantees.

`policy_ref` is particularly easy to overread. `OpenBlock` still has a `policy_ref: ProcedureRef`, but the code comment states that it is a procedure/policy-material label, not an independently authoritative `PolicyRef` (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2160`). The v2 doc says the policy that matters for cross-runtime continuity is embodied by the admitted runtime and its policy-bearing code surface, especially `ploke-eval`, not an external policy reference (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:152`). The current implementation reflects this direction with `SurfaceCommitment`, but still accepts procedure and actor labels as caller-provided data in some boundaries.

Actor/ruler identity remains partly caller-discipline. `Crown<Ruling>::open_block` notes that the Crown proves lineage authority but does not yet carry ruling actor identity; `OpenBlock` still supplies `opened_by` and `ruling_authority` as data (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2769`). `Crown<Ruling>::admit_claim` similarly says the method proves Crown possession but still accepts the ruler actor identity as data until a future `Parent<Ruling>` carrier supplies it structurally (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2831`).

## Docs Versus Code Barriers

The documentation claim that advanced states should be hard to construct is mostly matched inside the History/Crown path. State marker fields and authoritative block state payloads are private or module-scoped (`crates/ploke-eval/src/cli/prototype1_state/inner.rs:47`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:2384`). `Claims` has no public constructor or `Default`; flat serialized fields must pass through nested admitted-claim accessors, and missing claims are explicitly not implicit verification (`crates/ploke-eval/src/cli/prototype1_state/history.rs:2262`).

The weaker barriers are at process and store boundaries:

- `FsBlockStore` is an append-only local prototype port, not an authenticated replicated store (`crates/ploke-eval/src/cli/prototype1_state/history.rs:550`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:563`).
- `StoreHead::Absent` is local projection absence, not authenticated global absence (`crates/ploke-eval/src/cli/prototype1_state/history.rs:950`).
- `sealed_head_block` currently refuses stored blocks with entries because verified entry loading is not implemented (`crates/ploke-eval/src/cli/prototype1_state/history.rs:759`, `crates/ploke-eval/src/cli/prototype1_state/history.rs:912`).
- Parent identity and selected successor continuity still use local scheduler/invocation/identity evidence around the History gate; the gate is stronger than those files but does not replace all surrounding transport state (`crates/ploke-eval/src/cli/prototype1_process.rs:384`, `crates/ploke-eval/src/cli/prototype1_state/mod.rs:700`).

## Loop Completion Expectation

I expect the current implementation to support the local single-ruler loop structure, assuming ordinary successors do not modify the immutable `crates/ploke-eval` policy-bearing surface and the filesystem store remains locally consistent. The intended single-successor loop is documented as parent creates child, evaluates, selects, updates active checkout, launches successor, hands off authority, and exits (`crates/ploke-eval/src/cli/prototype1_state/mod.rs:158`). The current code has concrete local gates for sealing/appending before spawn and verifying the sealed head before successor parent entry (`crates/ploke-eval/src/cli/prototype1_process.rs:976`, `crates/ploke-eval/src/cli/prototype1_process.rs:397`).

The future multi-ruler/consensus/policy concerns should not block this local loop from completing. They do, however, bound what completion means. A completed run is evidence of local single-ruler continuity under one configured store and one runtime-surface policy, not proof of global uniqueness, distributed finality, or canonical fork choice. The v2 doc is explicit that local History is what one runtime/store accepts under Crown authority, while complete History would require consensus/finality policy (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:363`). Process uniqueness is also explicitly outside the current type-state claim until a lease, lock, or consensus layer exists (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:123`).

The practical policy caveat is important: current ordinary succession treats `crates/ploke-eval` as immutable (`docs/workflow/evalnomicon/chat-history/history-blocks-v2.md:221`). Therefore a loop whose child patch changes `crates/ploke-eval` should fail ordinary admission/execution rather than become an admitted descendant. That is correct under the current policy, but it means "full loop completion" currently applies to the bounded mutable surface, not arbitrary self-modification of the runtime/Crown/History implementation.

## Conclusion

Current documentation mostly does not overstate the code. The implemented guarantee is narrow and local: sealed, deterministic, lineage-scoped History blocks with Crown-routed open/admit/seal transitions, local store-head append checks, and runtime-surface commitments for ordinary succession. The explicitly deferred guarantees are authenticated multi-lineage head proofs, signatures, distributed consensus, global fork choice/finality, process uniqueness, uniform startup admission, and protocol upgrades that mutate the policy-bearing `ploke-eval` surface.

The highest-risk ambiguity is not an incorrect code path; it is terminology drift. Any future summary should say "local single-ruler History with rebuildable head projections" unless and until the authenticated head map, consensus/finality layer, signatures, process uniqueness, and protocol-upgrade policy are actually implemented.
