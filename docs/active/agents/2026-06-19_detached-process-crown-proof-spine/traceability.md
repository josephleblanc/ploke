# Detached Process And Crown Authority Proof Spine Traceability

Date: 2026-06-19
Status: Slice 0 preflight artifact
Repo state observed: branch `prototype1-parent-mwv-live-r16-dangling-symlink-surface-fix-20260615t003337z-gen0`, HEAD `55062d4c [verified] feat(ploke-records): add proof fact DTOs`, with unrelated untracked `.hermes/plans/2026-06-15_225532-ploke-call-graph-typed-plan.md` left untouched.

## Target-first invariant set

The implementation target is the proof-useful spine described in:

- `docs/workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md`
- `docs/workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md`

Primary obligations:

1. No operating-system process may outlive its creating runtime except through an admitted successor or lineage handoff.
2. A successor handoff produces exactly one detached successor `Parent` process.
3. At most one running process in a lineage has `Crown<Ruling>` authority.
4. Missing macro, build-script, cfg, type-resolution, external-dependency, or authority evidence must block proof explicitly rather than disappear into generic unresolved edges.

## Existing extension points and vocabulary to reuse

Do not introduce a disconnected graph stack. The current code already has these extension points:

- Passive proof records: `crates/ploke-records/src/proof_facts.rs`
  - Existing DTOs: `BuildDomainFact`, `ExpansionBoundaryFact`, `CallResolutionFact`, `EffectSeedFact`, `ProofFactRecord`.
  - Existing blocker vocabulary: `ProofBlockerReason::{MacroExpansionNotAvailable, ProcMacroSummaryMissing, BuildScriptSummaryMissing, ExternalDependencySummaryMissing, CfgDomainNotMaterialized, TypeResolutionMissing, ...}`.
  - Existing safety boundary: `crates/ploke-records/src/lib.rs` says deserialization only parses record shapes and does not validate authority, advance runtime state, admit History, or prove Crown/Block transitions.
- Structural graph extraction and relation storage: `crates/ingest/syn_parser`, `crates/ingest/ploke-transform`.
  - `syn_parser` already has typed node identifiers, parsed relations, module/path/type-resolution machinery, and fixture infrastructure.
  - `ploke-transform` already stores graph relations in Cozo; `crates/ingest/ploke-transform/src/schema/edges.rs` defines `syntax_edge`, `type_relation`, and `type_use` patterns to extend rather than replace.
- Database/query layer: `crates/ploke-db`.
  - Existing query helpers (`helpers::graph_resolve_edges`, `resolve_edges_for_all_primary`, `count_edges_by_kind`) already query stored graph relations for GraphRAG/inspection-style use.
  - Proof graph storage should add relation-aware proof queries here rather than leave records as loose JSONL.
- Prototype 1 authority and process semantics: `crates/ploke-eval/src/cli/prototype1_state` and `crates/ploke-eval/src/cli/prototype1_process.rs`.
  - `inner.rs` defines `Crown<crown::Ruling>`, `Crown<crown::Locked>`, `LineageKey`, and the `LockCrown` transition from `Parent<Selectable>` to `Parent<Retired>` while sealing History.
  - `prototype1_process.rs` documents and owns the current child/successor process seam, including the safety target that successor spawn crosses the predecessor into `Parent<Retired>` before executable successor authority.
  - `evidence_inventory.rs` already distinguishes `AuthorityTreatment` and `HistoryCommitmentLane`, which is the right vocabulary for evidence surfaces that must not become authority by accident.

## Traceability matrix

| Proof obligation / evidence family | Current code evidence | Current gap | First consumer needed |
| --- | --- | --- | --- |
| Process spawns | `prototype1_process.rs` imports `std::process::Command as ProcessCommand`; `c2.rs` runs `cargo check` / `cargo build` via `.output()`; repo-wide search finds additional `Command`, `tokio::spawn`, and `spawn_blocking` sites in `ploke-io`, `ploke-tui`, `ploke-llm`, and tests. | No normalized proof graph yet distinguishes runtime-bounded `.output()`/`.status()` from detached `.spawn()`/successor launch or from async task escapes. | Proof checker must consume process/effect facts and return `Pass`, `Fail`, or `Blocked`; GraphRAG may view the same facts but cannot drop blockers. |
| Detached/background behavior | `prototype1_process.rs` documents `SuccessorHandoffMode::Detached` and the handoff flow; `ploke-io/src/builder.rs` has a comment mentioning intentionally detached behavior; async background task sites exist across `ploke-io`, `ploke-tui`, `ploke-llm`, and fixtures/tests. | Current DTOs have `EffectClass::{OperatingSystemProcessCreate, AsyncTaskSpawn, ...}` but no active checker classifying runtime-bounded vs detached vs navigation-only unknown. | Detached-process invariant checker and adversarial fixtures for illegal detached spawn, legal handoff, async escape, and navigation-only unknown. |
| Async task boundaries | Search found `tokio::spawn` / `tokio::task::spawn_blocking` sites in `ploke-io`, `ploke-tui`, `ploke-llm`, and ingest tests. | Async boundaries are not normalized into lifetime/cleanup facts; proof cannot yet know whether a task can outlive a Parent authority epoch. | Effect extractor must mark async spawn/join/abort facts; checker must block ambiguous authority-bearing async paths. |
| `Parent` lineage constructors | `prototype1_state/parent` module is referenced by `channel.rs`, `inner.rs`, and `prototype1_process.rs`; `inner.rs` implements `LockCrown for Parent<Selectable>` and converts to `Parent<Retired>`. | No machine-readable authority facts identify which constructors/transitions are authoritative versus structurally similar DTOs. | Authority extractor and negative tests must prove ordinary records/deserialization cannot look like admitted `Parent` authority. |
| `Crown<Ruling>` transitions | `inner.rs` has private `Crown<crown::Ruling>::for_lineage`, `Crown<crown::Ruling>::lock`, `Crown<crown::Locked>::into_seal_fields`, and private `crown::Ruling::new`. | No proof graph records authority mint/lock/retire effects; no checker enforces at-most-one `Crown<Ruling>` per lineage. | Authority facts plus Crown invariant checker. |
| Successor handoff admission | `prototype1_process.rs` documents selected Artifact install, parent retirement, successor invocation, ready acknowledgement, and bounded trampoline. `invocation.rs`, `channel.rs`, `journal.rs`, and `successor` records participate in handoff evidence. | The evidence path is operationally documented but not normalized into handoff obligations with exact-one successor and predecessor-retired gates. | Handoff checker consumes authority/evidence/process facts and blocks if selected artifact, digest, retirement, or exactly-one successor evidence is absent. |
| Predecessor retirement | `inner.rs` `LockCrown` returns `Parent<Retired>` and a locked/sealed transition; `prototype1_process.rs` says predecessor must not remain ruling-capable after successor is executable. | Active checker does not yet assert retirement dominates successor Ruler admission. | Authority/lifetime graph with explicit `predecessor_retired` obligation. |
| Immutable-surface digest checks | `prototype1_process.rs` and target docs describe selected Artifact and immutable surface digest admission; `history` and `edit_surface` code hold surface/evidence types. | No current proof fact links build domain, selected artifact, immutable digest, and successor admission. | Build-domain facts plus authority facts; checker blocks if digest evidence is missing or mismatched. |
| Durable evidence reads/writes | `evidence_inventory.rs` lists transition journal, scheduler, branch registry, run artifacts, successor ready/completion, and authority treatments; `prototype1_process.rs` writes journal entries and successor records. | Durable reads/writes are not yet effect facts with provenance and authority treatment in the proof graph. | Evidence-effect facts and report path; consumer must distinguish projection-only from admitted preview or sealed History. |
| Macro expansion boundaries | `ploke-records::proof_facts` already has `ExpansionBoundaryFact`, `ExpansionBoundaryKind`, `ExpansionState`, and blocker reasons. | There is no build-domain/expansion importer or checker scaffold; missing expansion may still be invisible to proof consumers. | Build-domain/expansion checker must turn missing macro/proc-macro/build-script facts into typed `Blocked` results. |
| Build scripts | `ExpansionBoundaryKind::BuildScript` and `ProofBlockerReason::{BuildScriptSummaryMissing, BuildScriptExecutionBlocked}` exist. | Cargo metadata/lock/build-script evidence is not yet emitted or consumed. | Build-domain lane must record metadata/hash/toolchain and generate build-script blocker facts consumed by checker/report. |
| Feature and `cfg` domains | `BuildDomainFact` includes `features_hash` and `active_cfg_hash`; `syn_parser` and transform schema retain `cfgs`/compilation-unit concepts. | No active cfg-domain materialization path yet; unresolved cfg domain must block. | Build-domain lane and proof checker blocks `CfgDomainNotMaterialized`. |
| External dependency calls | `ExternalSummaryId`, `ExpansionBoundaryKind::ExternalSummary`, and `ProofBlockerReason::ExternalDependencySummaryMissing` exist. | No external summary catalog or proof consumer. | Resolver/effect extractor emits external-summary or typed blocker; checker refuses proof success when dangerous external summaries are missing. |

## Fact-family consumer plan

Included now because each has a direct proof consumer in this milestone:

- Build domains -> checker/report ensures each proof claim names Cargo metadata, lockfile, target, features, cfg, toolchain, and proof policy.
- Expansion boundaries -> checker/report blocks macro/proc-macro/build-script gaps with typed reasons.
- Call sites / call edges -> GraphRAG, symbol lookup, and proof traversal consume the same records.
- Effect facts -> detached-process and durable-evidence checkers consume process, async, authority, History, surface, and evidence effects.
- Authority facts -> Crown uniqueness and successor handoff checkers consume authority mint/retire/lock/admission evidence.
- Blocker statuses -> blocker inspection/reporting and proof checker consume them; navigation-only consumers may display them but must not erase them.
- Source/provenance/canonical identity -> import/canonical identity validator and mismatch adversarial fixtures consume these facts.

Deferred until a later scoped extension, not silently included:

- Full compiler/HIR macro expansion backend: target schema should allow it, but this slice will not hand-roll full macro expansion.
- Full monomorphized trait/dynamic dispatch proof: unresolved/candidate states block proof until bounded by a resolver or external summary.
- Operating-system process-family containment proof for opaque commands: initially represented as missing lifetime/containment evidence and therefore `Blocked`, not `Pass`.

## Next implementation slice chosen

Next slice: proof-obligation vocabulary and checker-facing semantics in `ploke-records`, tested before code changes.

Reason: existing DTOs are useful but passive. The proof spine needs a small, stable vocabulary for `resolved`, `candidate`, `unresolved`, `blocked`, `admitted`, `rejected`, `runtime-bounded`, `detached`, `successor`, `predecessor-retired`, `Crown<Ruling>`, authority-token constructors, and proof-only versus navigation-only evidence before extractor/database work can avoid schema churn. This advances the end-to-end proof spine because the same vocabulary will be consumed by record validation, database import, GraphRAG views, and the active invariant checker rather than becoming a JSONL-only milestone.
