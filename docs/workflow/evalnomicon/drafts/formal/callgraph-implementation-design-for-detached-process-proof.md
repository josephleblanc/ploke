# Call Graph Implementation Design For Detached-Process Proof

Status: draft implementation target, not a next-slice plan  
Date: 2026-06-18  
Depends on: `detached-process-callgraph-proof-target.md`

## Purpose

This document designs the call-graph implementation target needed to support the strong Ploke formal-verification claim:

```text
For admitted descendants of the genesis Parent, no operating-system process may outlive the runtime that created it except through an admitted successor/lineage handoff, and Crown semantics prove at most one permissioned Ruler-state Parent per lineage.
```

This is deliberately not a task breakdown. The point is to define the representation and verification architecture that would make the claim defensible to a security audit, enterprise compliance review, and red-team review. Implementation slices should be derived from this target later; they should not force this target to be weakened.

## Relationship to existing Ploke code

The design should reuse existing Ploke extension points rather than introduce an unrelated graph stack.

Current relevant components:

- `crates/ingest/syn_parser`
  - already discovers Cargo roots and builds a structural Rust code graph;
  - has typed node identifiers, module-tree construction, `cfg` filtering, type-use/type-relation work, and tests around imports, modules, and type relations;
  - currently stores function and method bodies mostly as token strings rather than a resolved expression/call/effect graph, so it is not sufficient by itself for the detached-process proof.
- `crates/ingest/ploke-transform`
  - transforms parsed nodes and typed relations into Cozo relations;
  - already has `syntax_edge`, `type_relation`, `type_use`, and related relation schema patterns;
  - should be extended by adding proof-oriented relations rather than replacing it.
- `crates/ploke-db`
  - provides query helpers and relation-aware lookup over stored graph facts;
  - useful for GraphRAG, inspection, and proof artifact assembly;
  - should not be the trusted authority by itself unless the extracted facts are independently content-addressed and checked.
- `crates/ploke-tree` and `crates/ploke-records`
  - already model passive evidence and runtime playback surfaces;
  - should ingest proof artifacts as evidence, not silently promote them into History authority.
- `crates/ploke-eval/src/cli/prototype1_state`
  - owns the current History/Crown/surface-admission semantics that the graph must eventually prove and preserve.

The call graph target is therefore an extension of the current parser/transform/database/evidence architecture, plus a stricter proof kernel over the extracted facts.

## Core design principles

### 1. Compiler-grade source of truth

For proof claims, a `syn`-only AST pass is not enough. The final source of truth for call edges and effect classification must be build-aware and expansion-aware.

The extractor must be able to bind facts to:

- a Cargo metadata graph;
- a Cargo lockfile;
- exact package, target, feature, profile, target triple, and `cfg` set;
- macro-expanded source or equivalent compiler/HIR facts;
- build-script and procedural-macro execution policy;
- resolved trait, inherent method, generic, closure, async, and function-pointer call candidates.

`syn_parser` remains valuable for stable source identity, structural graph facts, source spans, type-bearing identifiers, and existing ingestion paths. But the proof-grade call/effect layer must be compiler-aligned.

### 2. Fail closed

The verifier must never treat missing information as safe. A dangerous unresolved edge is a proof blocker.

Resolution states are first-class data:

```text
Resolved
CandidateSet
Ambiguous
Unresolved
ExternallySummarized
Blocked
```

Any process-affecting path in `CandidateSet`, `Ambiguous`, `Unresolved`, or unaudited `ExternallySummarized` state blocks the detached-process proof unless a separate conservative proof shows the whole candidate set is safe.

### 3. Separate extraction, normalization, and proof

The system should have three layers:

1. **Extractor layer**: gathers raw compiler/parser/build facts.
2. **Normalizer layer**: converts raw facts into a small proof-oriented intermediate representation.
3. **Proof layer**: checks theorem obligations over the normalized facts.

Cozo and GraphRAG can query the normalized facts, but the formal claim should be discharged by a small proof kernel or generated proof artifact, not by ad hoc human interpretation of a large database.

### 4. Immutable policy as induction base

The verifier, process-effect policy, Cargo/build policy, History/Crown code, surface partition code, and admission rules are part of the immutable policy-bearing surface. Ordinary descendants cannot edit these files or `Cargo.toml`/build surfaces without a protocol-upgrade or fork transition.

The call graph proof is only meaningful if the proof checker itself is inside the immutable digest or otherwise has an admitted version commitment.

## Top-level architecture

The target implementation should produce a content-addressed `CallGraphProofArtifact` from one admitted build domain.

```text
Cargo metadata + lockfile + source tree + proof policy
  -> BuildDomain
  -> StructuralGraph          (current syn_parser/ploke-transform territory)
  -> ExpansionGraph           (macro/proc-macro/build-script provenance)
  -> ResolvedCallGraph        (call sites, candidate/resolved callees)
  -> EffectGraph              (process, async, authority, durable evidence effects)
  -> LifetimeGraph            (ownership, cleanup, containment, shutdown)
  -> AuthorityGraph           (Crown/Parent/History typestate transitions)
  -> ProofObligations
  -> CallGraphProofArtifact
  -> History/admission evidence
```

Each layer should preserve provenance back to source span, expansion context, package target, and feature/cfg domain.

## BuildDomain

The `BuildDomain` is the proof boundary. A proof with an unnamed build domain is not meaningful.

Required fields:

- workspace root;
- source tree or Artifact identity;
- Cargo metadata JSON hash;
- Cargo lockfile hash;
- package id;
- target kind: library, binary, test, example, benchmark, build script, proc macro;
- target triple and host triple;
- profile;
- enabled features;
- active `cfg` set;
- rustc version and toolchain identity;
- environment variables allowed to influence build or runtime;
- proof policy version;
- immutable surface digest;
- list of build scripts and proc macros in the build closure;
- dependency summary identities.

For the long-horizon autonomous claim, ordinary runtime admission should name which `BuildDomain` is admitted. A proof for `ploke-eval` under one feature set does not automatically prove `ploke-tui`, tests, another target triple, or another feature set.

## Source and expansion graph

The current structural graph should be extended with expansion provenance.

Required node classes:

- crate/package target;
- module;
- function;
- method;
- associated function;
- trait item;
- implementation block;
- closure;
- async block/future body;
- macro definition;
- macro invocation;
- expanded item/expression;
- build script;
- procedural macro crate/function;
- external dependency summary;
- unsafe/foreign-function block and foreign function item.

Required provenance fields:

- source file;
- byte span;
- original token span where available;
- expansion stack;
- hygiene context or equivalent macro expansion identifier;
- `cfg` condition;
- package target;
- tracking hash.

The important property is that an effect introduced by a macro expansion can be traced both to the expanded effect site and to the macro invocation/definition responsible for it.

## Resolved call graph

The resolved call graph should model all executable edges, not only named function calls.

Required call-site classes:

- direct function call;
- associated function call;
- inherent method call;
- trait method call;
- fully-qualified syntax call;
- closure call;
- function pointer call;
- trait-object dynamic dispatch;
- async poll/resume edge;
- `Drop` edge;
- panic/unwind cleanup edge where relevant;
- macro-introduced call;
- build-script/proc-macro execution edge;
- foreign-function call.

Required call-resolution fields:

```text
call_site_id
caller_body_id
source_span
expansion_context
callee_resolution_state
resolved_callee_id?              // for Resolved
candidate_callee_ids[]           // for CandidateSet or Ambiguous
external_summary_id?             // for ExternallySummarized
reason                           // why unresolved/ambiguous/blocked
cfg_condition
build_domain_id
```

Trait-object and function-pointer calls must remain candidate sets unless the proof domain can bound them more tightly. A candidate set is acceptable only if every candidate is safe under the same effect/lifetime proof.

## Effect graph

The effect graph is the first proof-specific layer. It classifies calls by semantic effect.

Required effect classes:

- `OperatingSystemProcessCreate`
- `OperatingSystemProcessReplace` (`exec`-like)
- `OperatingSystemProcessConfigure`
- `OperatingSystemProcessWait`
- `OperatingSystemProcessKill`
- `OperatingSystemProcessReap`
- `AsyncTaskSpawn`
- `AsyncTaskJoin`
- `AsyncTaskAbort`
- `AuthorityMint`
- `AuthorityRetire`
- `AuthorityLock`
- `AuthorityUnlock`
- `HistoryOpenBlock`
- `HistorySealBlock`
- `HistoryAppendBlock`
- `SurfaceMeasure`
- `SurfaceDigestCompare`
- `DurableEvidenceWrite`
- `DurableEvidenceRead`
- `ExternalSummaryBoundary`

The process effect classes are the immediate target. Authority and evidence effects are needed to prove the successor exception and Crown uniqueness.

## Process builder dataflow

For every operating-system process create/replace effect, the graph must retain the complete builder chain.

Required facts:

```text
process_effect_id
builder_object_id
program_expr
program_resolution_state
arguments[]
current_dir
stdin_policy
stdout_policy
stderr_policy
environment_added[]
environment_removed[]
environment_inherited_policy
process_group_policy
session_policy
parent_death_policy
uid_gid_policy
namespace_or_container_policy
timeout_policy
cancellation_policy
handle_owner
terminal_operation             // status, output, spawn, exec, etc.
```

This needs expression-level dataflow. The verifier must know that `let mut command = Command::new(x); command.arg(y); command.spawn()` is one builder, not three unrelated operations.

For external commands, the proof should not rely only on command name. It should classify command families:

- hermetic wrapper;
- known interactive external command;
- VCS command such as git;
- package manager/build tool such as cargo;
- shell interpreter;
- editor;
- server/daemon;
- unknown executable.

Shell interpreters and unknown executables are proof blockers unless a containment policy or external summary proves boundedness.

## Lifetime graph

The lifetime graph proves that non-successor processes cannot outlive the creating runtime.

Required nodes:

- runtime scope;
- function body scope;
- async task scope;
- process handle;
- process family;
- cleanup guard;
- timeout guard;
- cancellation token;
- shutdown signal;
- successor handoff boundary;
- Parent authority epoch;
- walk-service ownership epoch.

Required edges:

- `OwnsProcessHandle`
- `TransfersOwnership`
- `DropsHandle`
- `WaitsFor`
- `Reaps`
- `Kills`
- `KillsProcessFamily`
- `CancelsTask`
- `JoinsTask`
- `AbortTask`
- `DominatesNormalExit`
- `DominatesErrorExit`
- `DominatesTimeoutExit`
- `DominatesCancellationExit`
- `BoundedByRuntime`
- `MayOutliveRuntime`
- `SuccessorExceptionCandidate`

The central proof obligation for a non-successor process is:

```text
For each OperatingSystemProcessCreate site E:
  if E is not an admitted successor/lineage handoff,
  then every path from E to runtime termination is dominated by a proof of wait/reap
  or kill/reap of the process family.
```

That requires control-flow graph information, not only call edges. The implementation should use a normalized control-flow representation with explicit normal, error, panic/unwind, timeout, and cancellation exits.

## Process-family containment

The immediate child process is not enough. Git, Cargo, shells, editors, and build tools can spawn further processes.

The graph must attach each external command to a containment policy:

```text
ContainmentPolicy:
  None
  ProcessHandleOnly
  ProcessGroupKill
  SessionKill
  CgroupKill
  JobObjectKill
  NamespaceSandbox
  ExternalCommandSummary
  Blocked
```

For the high-bar claim, `ProcessHandleOnly` is rarely sufficient for opaque commands. A command that can create surviving grandchildren needs process-family containment or a strict external-command summary.

`git` and `cargo` should not be treated as inherently harmless. Their safe status depends on noninteractive flags, environment policy, hook policy, network policy, credential-helper policy, process-family containment, and cleanup dominance.

## Async task graph

Async tasks are not operating-system processes, but they can hide process effects or authority effects.

The graph must model:

- `tokio::spawn` and `JoinSet::spawn`;
- ownership of task handles;
- whether a task is awaited, joined, aborted, or intentionally detached;
- cancellation behavior;
- whether cancellation runs destructors/cleanup;
- any process effects reachable from the task body;
- whether the task can continue after Parent authority is retired or locked.

A detached async task with no process effects may be outside the immediate operating-system-process theorem, but it is still relevant to Crown authority if it can mutate History, channels, files, or launch processes later.

## Authority graph

The authority graph proves that the successor exception does not create overlapping permissioned Ruler authority.

Required authority-token nodes:

- `Parent<Unchecked>`;
- `Parent<Checked>`;
- `Parent<Ready>`;
- `Parent<Planned>`;
- `Parent<Selectable>`;
- `Parent<Retired>`;
- `Crown<Ruling>`;
- `Crown<Locked>`;
- `Startup<Validated>`;
- `Block<Open>`;
- `Block<Sealed>`;
- successor invocation/admission carrier;
- lineage identifier;
- protocol-upgrade or fork/new-lineage carrier, when it exists.

Required authority edges:

- constructor edge;
- move/consume edge;
- lock edge;
- seal edge;
- append edge;
- validate edge;
- unlock/admit edge;
- retire edge;
- fork/new-lineage edge;
- visibility/privacy proof edge.

The graph must know whether a token can be constructed only through private/module-scoped transitions. Rust privacy, field visibility, sealed state markers, and move-only ownership are proof facts.

The central authority theorem is:

```text
For every lineage L and time/transition interval T:
  the graph admits at most one live permissioned Ruler-state Parent for L.
```

This theorem is stronger than same-host process-tree reasoning. It can support future remote successor handoff because the authority state, not process parentage, is the invariant.

## Durable evidence graph

The static proof must bind to evidence that successors can validate.

Required evidence nodes:

- proof artifact;
- proof policy version;
- source tree/Artifact identity;
- immutable surface digest;
- Cargo metadata and lockfile identity;
- sealed History block;
- History head projection;
- transition journal entry;
- successor invocation;
- successor ready/completion record;
- surface commitment record;
- child evaluation record;
- walk lifecycle record;
- external-command summary record;
- dependency summary record.

Evidence must be content-addressed where possible. A successor should reject proof evidence when it is missing, stale, computed under a different build domain, or inconsistent with the current immutable surface digest.

## Proof artifact schema target

The proof artifact should be a stable record, likely in `ploke-records` once the design stabilizes.

Conceptual shape:

```text
CallGraphProofArtifact {
  schema_version,
  proof_artifact_id,
  generated_at,
  build_domain,
  immutable_surface_digest,
  extractor_version,
  normalizer_version,
  verifier_version,
  policy_version,
  source_graph_digest,
  expansion_graph_digest,
  call_graph_digest,
  effect_graph_digest,
  lifetime_graph_digest,
  authority_graph_digest,
  process_inventory_summary,
  unresolved_dangerous_edges,
  proof_obligations,
  proof_results,
  external_summaries,
  blockers,
}
```

This artifact should be treated as evidence that a verifier ran over a specific graph. It does not replace sealed History/Crown authority; it becomes one of the things that History/admission checks.

## Proof engine boundary

The proof engine should operate over a compact normalized model rather than over the entire parser database.

A practical high-bar design is:

1. Use compiler/parser infrastructure to extract facts.
2. Normalize facts into a small typed proof model.
3. Use Datalog/Cozo-style queries for reachability, candidate discovery, and artifact assembly.
4. Use a proof-oriented checker for theorem discharge over the normalized model.
5. Where feasible, use Verus-style specifications for the proof kernel and invariant-preserving transformations.

The important point is that Cozo queries can help discover and summarize, but the security claim should rest on a small auditable proof kernel with fail-closed semantics.

Candidate proof-kernel obligations:

```text
AllProcessSitesAccountedFor
NoUnresolvedDangerousProcessEdges
EveryNonSuccessorProcessRuntimeBounded
EveryOpaqueCommandContainedOrSummarized
NoAuthorityMintOutsideAllowedTransitions
SuccessorAdmissionDominatedByHistorySealAndSurfaceDigest
AtMostOneRulerParentPerLineage
ProofArtifactMatchesImmutableSurfaceDigest
```

Each obligation should have machine-readable inputs and outputs, not just prose diagnostics.

## External summaries

Dependencies, proc macros, build scripts, and external executables may not be analyzable with the same precision as local source. The implementation needs explicit summaries with trust levels.

Summary classes:

- `AnalyzedSource`: source was fully analyzed under the same model.
- `AuditedNoProcessEffects`: external unit is asserted and checked to have no process effects.
- `AuditedBoundedProcessEffects`: external unit may spawn but is proved bounded by a known policy.
- `OpaqueBlocked`: no acceptable summary; proof fails.
- `AllowedOnlyUnderContainment`: summary is valid only when paired with a named containment policy.

A summary must name:

- artifact hash;
- version;
- signer/reviewer or generation method;
- scope of validity;
- allowed effects;
- required containment;
- expiration or invalidation conditions.

No dependency or command should get implicit trust because it is common or familiar.

## Macro and build-script policy

Macro expansion and build scripts are part of the hard problem and must be first-class.

Target policy:

- Macro-expanded effects must point back to the macro invocation and definition.
- Procedural macro crates are executable build-time dependencies and require analysis or summary.
- `build.rs` is executable build-time code and requires analysis or summary.
- Changes to macro definitions, proc macro dependencies, `build.rs`, Cargo manifests, feature definitions, or dependency resolution belong to the immutable/protocol-upgrade surface, not ordinary mutable edit scope.
- A proof over unexpanded source is insufficient when macro expansion can introduce process effects.

## Verification fixtures the design must support

The design should eventually support adversarial fixtures that attempt to hide or leak processes through:

- direct `std::process::Command::spawn`;
- builder aliases and helper wrappers;
- `tokio::process::Command`;
- command builders passed across functions;
- `CommandExt::exec`;
- `pre_exec` hooks;
- process groups and sessions;
- shell wrappers;
- build scripts;
- procedural macros;
- declarative macros;
- trait method calls;
- dynamic dispatch;
- generic wrappers;
- function pointers;
- closures;
- async tasks that drop join handles;
- timeout branches that forget to reap;
- error paths using `?` before cleanup;
- panic/unwind paths;
- external tools that spawn grandchildren;
- Cargo hooks or git hooks;
- future walk service lifecycle handoff.

The expected result for each fixture should be either a proof of boundedness or a named proof blocker. There should be no fixture where the verifier silently omits the dangerous site.

## Admission integration

Eventually, admitted runtime startup should validate more than source digest alone.

A successor admission check should be able to require:

```text
current immutable surface digest == sealed digest
current Cargo/build domain == proof artifact build domain
proof artifact verifies under admitted proof policy
proof artifact has no blockers
History head names the proof artifact or an accepted digest of it
successor authority transition is admitted for this lineage
```

This makes the proof artifact part of the induction step. If an ordinary descendant preserves the immutable policy surface and proof/admission code, then it also preserves the rule that future descendants must satisfy the same process-lifetime and Crown-authority proof obligations.

## Walk service design target

The walk service should be represented as a runtime-owned service, not as an arbitrary detached server.

The graph must be able to model:

- Parent starts or attaches to walk service;
- service has an owning Parent authority epoch;
- successor starts replacement or takes over service after admission;
- predecessor can perform cleanup after authority retirement;
- temporary overlap is bounded and justified by a handoff protocol;
- service cannot remain active without either current Parent ownership or an explicit successor handoff record.

This allows graceful user-facing continuity without weakening the detached-process theorem.

## Termination and resource-bounds boundary

Detached-process safety is not the same as full loop termination. The graph should support both, but the claims should remain distinct.

Detached-process safety says no runtime leaves unowned operating-system processes behind except admitted successor/lineage handoff.

Long-horizon resource safety additionally needs:

- bounded child fanout;
- bounded generations or a proven termination condition;
- bounded workspace and artifact cleanup;
- bounded log/history growth or compaction with retained hashes;
- bounded external command timeouts;
- durable recovery after crash or power loss.

The call graph can support these later by sharing the same lifetime/effect/evidence model, but this design keeps the first theorem focused on process lifetime and Crown uniqueness.

## Non-goals

This document does not select a next implementation slice, commit to a particular crate layout, or claim the current code satisfies the theorem.

It does specify the implementation target: any call graph design that cannot represent build-domain identity, macro expansion, candidate resolution, process builder dataflow, process-family containment, cleanup dominance, authority-token transitions, and content-addressed proof artifacts is not sufficient for the formal guarantee Ploke is aiming to make.
