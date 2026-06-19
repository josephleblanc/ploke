# Detached Process And Crown Symbolic Invariant Outline

Status: draft symbolic proposition outline, not yet discharged proof
Date: 2026-06-19
Depends on:
- `detached-process-callgraph-proof-target.md`
- `callgraph-implementation-design-for-detached-process-proof.md`
- `../../../src/prototype1/invariant-ledger.md`
- `../../../../active/agents/2026-06-19_detached-process-crown-proof-spine/traceability.md`

## Purpose

This note names the symbolic invariant track that a future checker must discharge for the Ploke loop. It is written in a Euclid-like style: define the objects, state the allowed constructions, name the assumptions, and only then state the propositions.

The document is not a claim that the current implementation already proves the theorem. Current code and tests provide passive DTO vocabulary, fail-closed fixture behavior, and early checker surfaces for pieces of the invariant. The construction-backed claim becomes available only when the required artifacts are generated for a named build domain, admitted by the decision-domain rules, and checked without blockers.

## Claim classes

Use the following labels when mapping prose, records, tests, or checker results to claims:

```text
Construction fact CF:
  A fact emitted by an admitted construction step over a named input state.

Admissibility assumption AA:
  A rule or trusted boundary that lets a record count as evidence for a decision domain.

Empirical/evaluative claim EV:
  A test result, observed run result, model judgment, survey count, or human review.
```

Interpretation:

- `CF` can support a symbolic invariant only when its construction step, input state, output record, schema version, and build domain are named.
- `AA` is allowed in the outline but must remain visible. It is not a construction fact.
- `EV` can motivate or regression-test the track, but it does not discharge a construction-backed invariant unless an admission rule converts the record into evidence for that decision domain.

The negative rule is:

```text
record(rec) does not imply admissible_D(rec)
projection(rec) ∨ log(rec) ∨ ui_state(rec) ∨ child_self_report(rec)
  does not imply admissible_D(rec)
```

## Universes and notation

Let:

```text
A   = Artifacts
B   = BuildDomains
Γ   = normalized source/effect/lifetime/authority graphs
H   = sealed lineage-local History
L   = lineage identifiers
R   = Runtime instances
P   = operating-system process families
K   = Crown authority states
E   = evidence records
O   = proof obligations
X   = proof blockers
D   = decision domains
```

A decision-domain admission function is:

```text
admit_D : record -> Option(Evidence_D)
```

A proof checker over normalized facts is:

```text
check_π : (B, Γ, H, E, O) -> ProofResult

ProofResult = Pass(artifact_id)
            | Blocked(X)
            | Fail(finding)
```

For this outline, `Pass` means only that the checker accepted the normalized model for the named `B` under policy `π`. It does not mean the whole repository, all feature sets, all targets, or future protocol versions are proved.

## Loop state

A Ploke-loop symbolic state is:

```text
S_n = (
  a_n,        // active Artifact
  b_n,        // named BuildDomain admitted for this transition
  Γ_n,        // normalized proof graph for b_n
  h_n,        // sealed History head for lineage L
  k_n,        // Crown state for lineage L
  r_n,        // active Runtime set
  p_n,        // process-family ownership/lifetime facts
  e_n,        // admitted evidence set
  o_n,        // open proof obligations
  x_n         // visible proof blockers
)
```

Interpretation:

- `a_n` is the material surface that can hydrate a Runtime.
- `b_n` is the build/proof boundary. A claim without `b_n` is not meaningful.
- `Γ_n` is not merely a call graph. It is the normalized graph containing source, expansion, call, effect, lifetime, authority, and durable-evidence facts needed by the proof kernel.
- `h_n` and `k_n` are the authority surfaces. Scheduler rows, reports, registries, logs, GraphRAG indexes, and UI state are projections unless admitted by an explicit `D`.
- `x_n` is part of the state. Unknown dangerous facts are not absent; they are blockers.

## Artifacts and observations

The relevant artifact carriers are:

```text
Artifact a = (Immutable, Mutated, Ambient, identity, surface_digest)
BuildDomain b = (
  cargo_metadata_hash,
  cargo_lock_hash,
  package_target,
  features,
  cfg_set,
  target_triple,
  host_triple,
  profile,
  rustc_identity,
  environment_policy,
  proof_policy_version,
  immutable_surface_digest,
  build_script_set,
  proc_macro_set,
  dependency_summary_ids
)
```

The relevant observation chain is:

```text
event q
observer o
observation obs = observe(o, q)
record rec = record(writer, obs)
evidence ev = admit_D(rec)
decision input d ∈ inputs(D) iff admit_D(rec) = Some(ev)
```

For the detached-process and Crown track, the principal decision domains are:

```text
BuildAdmission
ProofArtifactAdmission
HistoryAdmission
SuccessorAdmission
CrownTransition
OperatorProjection
```

Only the first five can feed the symbolic invariant. `OperatorProjection` can explain or inspect state, but it cannot by itself grant authority or discharge a proof obligation.

## Transition relation

The admitted loop transition relation is:

```text
S_n --τ--> S_{n+1}
```

where `τ` is one of the following ordinary transition classes:

```text
ObserveAndRecord
ExtractNormalizeAndBlock
CheckProofArtifact
AdmitProofEvidence
RunChildEvaluation
AdmitChildEvidence
SelectSuccessor
SealHistoryBlock
RetirePredecessorAuthority
AdmitSuccessorRuntime
RejectOrBlock
```

A transition is ordinary only if:

```text
Ordinary(τ, S_n, S_{n+1}) iff
  NamedBuildDomain(S_n.b_n)
  ∧ ImmutablePolicyPreserved(S_n.a_n, S_{n+1}.a_{n+1})
  ∧ TransitionRecordAdmissible(τ, S_n, S_{n+1})
  ∧ BlockersFailClosed(S_{n+1}.x_{n+1})
```

Protocol upgrades, forks, new-lineage creation, manual operator repair, and emergency state rewrites are not ordinary transitions in this outline. They require separate admission rules and separate invariant statements.

## Construction steps

The construction track is the sequence of artifact-producing steps. These are the only steps allowed to create `CF` claims.

```text
C1. name_build_domain(a, policy) -> b
C2. extract_source_and_expansion(a, b) -> Γ_source
C3. resolve_calls(Γ_source, b) -> Γ_call ∪ blockers
C4. classify_effects(Γ_call, b) -> Γ_effect ∪ blockers
C5. derive_lifetimes(Γ_effect, b) -> Γ_life ∪ blockers
C6. derive_authority_graph(Γ_effect, h, k) -> Γ_auth ∪ blockers
C7. bind_durable_evidence(Γ_life, Γ_auth, h, e) -> Γ_ev ∪ blockers
C8. assemble_obligations(Γ_ev, policy) -> O
C9. check_π(b, Γ_ev, h, e, O) -> ProofResult
C10. record_proof_artifact(ProofResult) -> rec
```

Construction facts produced by these steps include:

```text
CF(BuildDomainNamed(b))
CF(ExpansionBoundaryRecorded(site, state, reason?))
CF(CallResolutionState(site, state))
CF(EffectClassified(site, class))
CF(ProcessLifetimeClassified(site, lifetime))
CF(AuthorityTransitionClassified(edge, class))
CF(EvidenceUseClassified(record, use))
CF(ProofResultRecorded(result, b, policy))
```

A construction step that encounters an unknown proof-critical condition must produce a blocker, not omit the site:

```text
proof_critical_unknown(u) -> X := Blocker(u)
```

## Evidence-producing steps

Evidence-producing steps create records that may become inputs to a decision domain only through `admit_D`.

```text
E1. write_build_domain_record(b) -> rec_b
E2. write_expansion_boundary_record(site, state, reason) -> rec_exp
E3. write_effect_fact(site, class, evidence_use) -> rec_eff
E4. write_authority_fact(edge, term, status) -> rec_auth
E5. write_process_lifetime_fact(site, lifetime, handoff?) -> rec_life
E6. write_proof_blocker(reason, scope) -> rec_blocker
E7. write_handoff_evidence(successor, predecessor, exactly_one) -> rec_handoff
E8. write_history_block(h, transition, evidence_refs) -> rec_history
E9. write_checker_report(result, findings, blockers) -> rec_report
```

Admission is explicit:

```text
ev_b       = admit_BuildAdmission(rec_b)
ev_report  = admit_ProofArtifactAdmission(rec_report)
ev_history = admit_HistoryAdmission(rec_history)
ev_succ    = admit_SuccessorAdmission(rec_handoff)
ev_crown   = admit_CrownTransition(rec_auth)
```

A report visible to GraphRAG or an operator is not enough:

```text
EvidenceUse(rec) = NavigationOnly -> rec ∉ inputs(ProofArtifactAdmission)
ObligationStatus(rec) ∈ {Blocked, Rejected} -> rec does not satisfy proof
ProcessLifetime(site) = Unknown -> DetachedProcessSafety blocked
```

## Invariant predicates

The core predicates are:

```text
NamedBuildDomain(S) iff S.b names the exact Cargo/build/proof boundary.

ImmutableSurfaceDigestPreserved(S, S') iff
  digest(policy_surface(S.a)) = digest(policy_surface(S'.a)).

ProofArtifactMatchesSurface(S) iff
  S.b.immutable_surface_digest = digest(policy_surface(S.a)).

NoNavigationEvidenceForProof(S) iff
  ∀rec ∈ S.e. EvidenceUse(rec) = NavigationOnly -> rec ∉ inputs(ProofArtifactAdmission).

BlockersFailClosed(S) iff
  S.x = ∅ or check_π(S.b, S.Γ, S.h, S.e, S.o) = Blocked(S.x).

AllProcessSitesAccountedFor(S) iff
  every process-create/replace effect in the admitted build domain has a recorded effect fact
  or a recorded proof blocker.

NoUnresolvedDangerousProcessEdges(S) iff
  every process-affecting CandidateSet, Ambiguous, Unresolved, Blocked, or unaudited
  ExternallySummarized edge appears in S.x.

EveryNonSuccessorProcessRuntimeBounded(S) iff
  ∀site. OperatingSystemProcessCreate(site)
    ∧ ¬AdmittedSuccessorHandoff(site)
    -> RuntimeBounded(site).

SuccessorAdmissionDominatedByHistorySealAndSurfaceDigest(S, S') iff
  AdmittedSuccessor(S, S')
    -> SealedHistoryHeadValidated(S.h)
       ∧ ProofArtifactMatchesSurface(S)
       ∧ PredecessorAuthorityRetiredOrLocked(S, S')
       ∧ ExactlyOneAdmittedSuccessor(S, S').

AtMostOneRulerParentPerLineage(S) iff
  ∀L. |{ r ∈ S.r | ParentRuler(r, L) ∧ LivePermissioned(r) }| ≤ 1.
```

Interpretation:

- `EveryNonSuccessorProcessRuntimeBounded` is a lifetime property. It is not equivalent to searching for `.spawn()`.
- `AtMostOneRulerParentPerLineage` is an authority property. It is not equivalent to exactly one operating-system process.
- `BlockersFailClosed` makes blockers monotonic: adding a proof-critical unknown can only preserve `Blocked` or move to `Fail`, never silently become `Pass`.

## Proposition 1: immutable surface digest induction

```text
ImmutableSurfaceDigestInduction:
  Given S_0 with admitted genesis Artifact a_0 and digest D,
  if every ordinary transition S_i --τ_i--> S_{i+1} preserves the policy-bearing
  immutable surface digest,
  then every admitted descendant S_n satisfies
    digest(policy_surface(S_n.a_n)) = D.
```

Construction facts needed:

- `BuildDomainNamed(b_i)` for each transition.
- `ProofArtifactMatchesSurface(S_i)` for each checker artifact used in admission.
- `TransitionRecordAdmissible(τ_i, S_i, S_{i+1})` for each ordinary transition.

Admissibility assumptions:

- The policy-bearing surface partition is itself inside the immutable/protocol-upgrade boundary.
- Digest computation and artifact identity are collision-resistant enough for the admitted policy.
- Manual edits outside ordinary transition rules are either impossible for the admitted runtime or recorded as non-ordinary repair/fork events.

Current implementation correspondence:

- Prototype 1 documents define the current surface partition and ordinary successor-digest intent.
- The current checker and DTO fixtures do not by themselves prove the full induction for all descendants.

## Proposition 2: detached process safety except admitted successor handoff

```text
DetachedProcessSuccessorException:
  For every admitted ordinary state S and every operating-system process family p
  created by a Runtime r in S:

  if p may outlive r,
  then p is part of an admitted successor/lineage handoff transition;
  otherwise p is runtime-bounded by ownership, containment, wait/reap, kill/reap,
  or a stricter admitted external-command summary.
```

Equivalent checker obligations:

```text
AllProcessSitesAccountedFor
NoUnresolvedDangerousProcessEdges
EveryNonSuccessorProcessRuntimeBounded
EveryOpaqueCommandContainedOrSummarized
```

Construction facts needed:

- Every process-create/replace site in `b` has an effect fact or blocker.
- Every non-successor process site has lifetime/containment facts showing runtime-boundedness.
- Every external command family has an admitted summary or named containment policy when needed.
- Every successor exception has handoff evidence admitted by `SuccessorAdmission`.

Admissibility assumptions:

- The extractor/normalizer sees the build-domain-relevant source after macro expansion or has admitted summaries for boundaries.
- The operating-system containment primitive named by the evidence actually enforces process-family cleanup on the target platform.
- External command summaries are scoped to artifact identity, version, environment policy, and containment policy.

Empirical/evaluative claims allowed:

- Fixture tests may show that a checker returns `Pass`, `Fail`, or `Blocked` for example fact sets.
- Survey counts may show current process/async search results for one repository state.
- These do not prove the proposition outside the admitted build domain.

## Proposition 3: Crown/Ruler uniqueness per lineage

```text
CrownRulingLineageUniqueness:
  For every admitted ordinary state S and lineage L,
    AtMostOneRulerParentPerLineage(S).
```

The successor transition obligation is:

```text
AdmitSuccessorRuntime(S, S') is defined only if
  SealedHistoryHeadValidated(S.h)
  ∧ ActiveArtifactIdentityValidated(S.a)
  ∧ ProofArtifactMatchesSurface(S)
  ∧ PredecessorAuthorityRetiredOrLocked(S, S')
  ∧ ExactlyOneAdmittedSuccessor(S, S')
```

Construction facts needed:

- Authority-token constructors, private fields, move-only transitions, and typestate states are represented in `Γ_auth`.
- `Crown<Ruling>` can only be obtained through admitted transitions.
- `Crown<Locked>` or `Parent<Retired>` is recorded before or at the logical boundary where the successor can become `Parent<Ruling>`.
- Structural similarity to authority construction is blocked unless explicitly admitted.

Admissibility assumptions:

- Rust privacy, typestate markers, and move semantics are represented accurately enough in the authority graph for the named build domain.
- Durable History/Crown records are append-only or otherwise tamper-evident under the admitted storage policy.
- Clock time is not the authority source. The invariant uses logical transition order recorded in History.

## Combined theorem target

```text
AdmittedLineageProcessAndCrownSafety:
  For every ordinary admitted descendant S_n of an admitted genesis state S_0,

  ImmutableSurfaceDigestInduction(S_0, S_n)
  ∧ DetachedProcessSuccessorException(S_n)
  ∧ CrownRulingLineageUniqueness(S_n)
  ∧ NoNavigationEvidenceForProof(S_n)
  ∧ BlockersFailClosed(S_n)
```

This is the normalized proof obligation the checker must eventually discharge. It is not a statement that the current Ploke repository already satisfies the theorem.

## Assumptions and scope limits

Construction-backed claims require these assumptions to be named, versioned, and where possible replaced by construction facts later:

1. Compiler/provenance soundness: source spans, macro expansion, cfg selection, build scripts, proc macros, and dependency summaries are represented for the admitted build domain.
2. Build-domain closure: no package/target/feature/cfg/profile/environment input outside `b` can affect admitted runtime behavior without becoming a blocker.
3. Policy-surface immutability: checker code, proof policy, History/Crown transition code, surface partition code, Cargo/build policy, and process containment wrappers are not ordinary mutable-surface edits.
4. History integrity: sealed History blocks and Crown transition records are append-only or tamper-evident under the admitted storage model.
5. Typestate fidelity: authority terms such as `Parent<Retired>`, `Crown<Ruling>`, `Crown<Locked>`, and `Startup<Validated>` cannot be forged outside admitted constructors for the named build domain.
6. OS containment fidelity: process-family cleanup semantics used by the lifetime graph are valid for the target operating system and runtime environment.
7. External summary validity: external dependency, command, build-script, and proc-macro summaries are scoped, reviewable, and invalidated when artifact identity or policy changes.
8. Logical transition ordering: authority handoff is ordered by admitted History/Crown transition records, not by wall-clock timestamps or process ids.

Out of scope for this outline:

- Full loop termination, bounded token/cost growth, workspace garbage collection, and recovery after power loss.
- Protocol upgrades, forks, and deliberate new-lineage creation.
- Claims over all targets, tests, examples, benches, feature sets, target triples, or future dependency versions unless each is named by a `BuildDomain`.
- Claims that GraphRAG retrieval, logs, child self-eval, or UI projections are authority by themselves.

## Current implementation correspondence

Current repository pieces that correspond to this outline include:

- `crates/ploke-records/src/proof_facts.rs`: passive DTO vocabulary for build domains, proof blockers, evidence use, obligations, handoff evidence, process lifetimes, authority terms, and validation reports.
- `crates/ploke-records/src/proof_effects.rs`: conservative source extraction for call/effect seed facts and blocker emission around proof-critical unknowns.
- `crates/ploke-records/src/proof_authority.rs`: admitted authority term extraction and blocked structural-similarity behavior.
- `crates/ploke-db/src/proof_graph.rs`: proof graph storage/query surface with pass/fail/blocked checker result vocabulary.
- `crates/ploke-db/tests/proof_invariant_checker.rs`: fixture-level behavior for `detached_process_successor_handoff` and `crown_ruling_lineage_uniqueness`.

Status interpretation:

```text
DTO vocabulary                         = construction support, not proof
Fixture-level checker tests            = empirical/evaluative regression evidence
Fail-closed blocker treatment in code   = local construction fact when tied to exact code/build domain
Full combined theorem                   = not yet proved
```

## Blocker semantics

A checker must prefer `Blocked` over speculative success:

```text
MacroExpansionNotAvailable
ProcMacroSummaryMissing
BuildScriptSummaryMissing
ExternalDependencySummaryMissing
CfgDomainNotMaterialized
TypeResolutionMissing
DynamicDispatchUnbounded
ExternalCommandSummaryMissing
AuthorityEvidenceMissing
ProcessLifetimeEvidenceMissing
CanonicalIdentityMismatch
SchemaVersionMismatch
RustcInvocationEvidenceMissing
```

These blockers are proof-relevant evidence. They should remain visible in GraphRAG and reports, but visibility is not admission. A blocked proof artifact can be useful diagnosis evidence while still being unable to satisfy `ProofArtifactAdmission`.

## Non-goals and not-yet-proved cases

This draft does not prescribe the exact Rust module layout, database schema, verifier backend, proof-assistant target, or implementation sequence. It only fixes the symbolic shape that those choices must support.

The current proof spine should not claim:

- every Ploke process effect is currently accounted for;
- every external command is currently process-family contained;
- every macro/proc-macro/build-script effect is currently expanded or summarized;
- every authority constructor is currently represented in a proof-grade authority graph;
- every descendant runtime already admits a checker artifact before succession.

The safe wording is: this invariant outline names the construction-backed claim Ploke is trying to make, distinguishes what must be constructed from what must be assumed, and keeps empirical tests and reports in their supporting but non-authoritative role.
