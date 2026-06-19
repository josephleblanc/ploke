# Formal Style And Proof-Spine Context Survey

Date: 2026-06-19
Status: survey artifact for drafting the next symbolic invariant / proposition outline
Related thread: `docs/active/agents/2026-06-19_detached-process-crown-proof-spine/`

## Purpose

This note maps the current evalnomicon/formal style and the active detached-process + Crown authority proof spine so a later worker can draft a symbolic invariant/proposition outline without redoing the repository survey.

The strongest current distinction to preserve is:

```text
formal target / proof obligation / checker vocabulary
  !=
current implementation already proves the target
```

Several documents define proof targets and active proof scaffolding, but the current implementation is still a partial proof spine with explicit blockers.

## Files inspected

Formal and evalnomicon style:

- `docs/workflow/evalnomicon/drafts/formal/README.md`
  - Index for the current formal drafts.
- `docs/workflow/evalnomicon/drafts/formal/procedure-notation.md`
  - Sections: `1. Universes And Basic Sorts`, `2. Procedure, Executor, And State`, `4. Target Metrics And Evidential Outputs`, `5. Recording And Forwarding`, `7. Fork And Merge`, `10. Reliability Properties`, `16. Bounded Inquiry Procedures`.
- `docs/workflow/evalnomicon/drafts/formal/edit-surface.md`
  - Sections: initial artifact-bound graph/grant model, `Observation And Admission`.
- `docs/workflow/evalnomicon/drafts/formal/detached-process-callgraph-proof-target.md`
  - Sections: `Terms`, `Primary theorem target`, `Surface-digest induction`, `Successor handoff semantics`, `What the call graph must become`, `Negative proof and proof blockers`, `Proof artifact requirements`, `Limitations the target must explicitly address`, `Non-goals for this draft`.
- `docs/workflow/evalnomicon/drafts/formal/callgraph-implementation-design-for-detached-process-proof.md`
  - Sections: `Core design principles`, `Top-level architecture`, `BuildDomain`, `Effect graph`, `Lifetime graph`, `Authority graph`, `Durable evidence graph`, `Proof artifact schema target`, `Proof engine boundary`, `External summaries`, `Macro and build-script policy`, `Admission integration`, `Termination and resource-bounds boundary`, `Non-goals`.
- `docs/workflow/evalnomicon/drafts/formal/macro-buildrs-callgraph-sequencing-survey.md`
  - Sections: `Recommendation`, `Survey facts from the current repository`, `Option C`, `Concrete next steps`, `Decision rule for implementation slices`, `Bottom line`.
- `docs/workflow/evalnomicon/drafts/formal/rustc-macro-expansion-backend-plan.md`
  - Sections: `Thesis`, `Source-of-truth notes`, `Fact schema sketch`, `Macro/proc-macro/build-script safety policy`, `Implementation sequence`, `Near-term recommendation`.
- `docs/workflow/evalnomicon/drafts/formal/trait-first-reification.md`
  - Sections: `Core Position`, `Why`, `Implication For This Work`, `Working Rule Of Thumb`.
- `docs/workflow/evalnomicon/drafts/formal/module-tree-and-trait-algebra.md`
  - Sections: `Reduction To Refuse`, `Architectural Readout`, `Minimal Trait Algebra`, `Implementation Order`, `Immediate Design Constraint`.
- `docs/workflow/evalnomicon/drafts/formal/typestate-sketches.md`
  - Headings inspected for legacy/mixed examples and the explicit `Old/bad example` marker.

Prototype 1 / proof-spine context:

- `docs/active/agents/2026-06-19_detached-process-crown-proof-spine/README.md`
  - States this directory is an active implementation spine, not proof by itself.
- `docs/active/agents/2026-06-19_detached-process-crown-proof-spine/traceability.md`
  - Sections: `Target-first invariant set`, `Existing extension points and vocabulary to reuse`, `Traceability matrix`, `Fact-family consumer plan`, `Next implementation slice chosen`.
- `docs/workflow/evalnomicon/src/prototype1/invariant-ledger.md`
  - Sections: `Status labels`, ledger entries 1-14, `Audit rule`.
- `docs/workflow/evalnomicon/src/prototype1/runtime-loop.md`
  - Sections: `Current loop shape`, `Runtime roles`, `Why this is not a flat eval loop`.
- `docs/workflow/evalnomicon/src/prototype1/runtime-authority.md`
  - Sections: `Authority shape`, `Role-bounded authority`, `State-bounded authority`, `Transition records`.
- `docs/workflow/evalnomicon/src/prototype1/edit-surface.md`
  - Sections: `Surface partition`, `Policy-bearing surface`, `Admission role`.
- `docs/workflow/evalnomicon/drafts/edit-surface/proof-index.md`
  - Sections: `Current Broad Fanout Coverage`, `Formal Reference`, `Proof Flow`, `Test Index`, `Current Formal Additions Needed`.
- `crates/ploke-eval/docs/prototype1-proof-ladder.md`
  - Sections: `Rule`, `Current Rungs`, `Latest Live Child Success Attempt`, `Remaining Rungs`.

Current code/test proof vocabulary:

- `crates/ploke-records/src/proof_facts.rs`
  - Passive proof-fact DTOs; key enums/records: `ExpansionState`, `ProofBlockerReason`, `EvidenceUse`, `ObligationStatus`, `HandoffRole`, `HandoffEvidence`, `ProcessLifetime`, `AuthorityTerm`, `ResolutionState`, `EffectClass`, `AuthorityFact`, `ProofBlockerFact`, `ProofValidationReport`, `ProofFactRecord`.
- `crates/ploke-records/src/proof_effects.rs`
  - Conservative source extractor for call/effect seed facts; records proof-critical unknowns and macro boundaries as blockers.
- `crates/ploke-records/src/proof_authority.rs`
  - Conservative authority/typestate seed extractor; exact source-site authority boundaries are opt-in and structural similarity is blocked.
- `crates/ploke-db/src/proof_graph.rs`
  - Proof graph storage/query surface and first active checker result types: `ProofInvariantStatus::{Pass, Fail, Blocked}` and `ProofInvariantFinding`.
- `crates/ploke-records/tests/proof_build_evidence.rs`
  - Build evidence snapshot and typed build-script/cfg blockers.
- `crates/ploke-records/tests/proof_effect_extractor.rs`
  - Process/async/durable-write/macro-boundary extraction expectations.
- `crates/ploke-records/tests/proof_authority_extractor.rs`
  - Admitted authority terms, blocked structural similarity, and call-site scoping.
- `crates/ploke-records/tests/proof_validation.rs`
  - Proof fact import/validation invariants and inert authority record checks.
- `crates/ploke-db/tests/proof_graph_store.rs`
  - GraphRAG/checker query behavior that retains blockers and distinguishes `navigation_only` from proof evidence.
- `crates/ploke-db/tests/proof_invariant_checker.rs`
  - Current active invariant checker fixtures for detached process and Crown uniqueness.

## House style and formatting constraints

Use the formal-draft style, not a generic RFC style:

- Top-level title is a plain `# Title`; many formal drafts include `Status: ...`, `Date: ...`, and sometimes `Depends on: ...` immediately below.
- Prefer short sections with precise English paragraphs followed by `text` fences for symbolic shapes.
- Use symbolic definitions sparingly but directly. Existing notation favors:
  - set and function notation: `M`, `X`, `S`, `V`, `P(A)`, `I_x`, `O_x`, `val : S -> P(V)`;
  - Greek/code symbols for graph objects: `Γₐ = (Vₐ, Eₐ, μₐ)`, `ρₐ(q)`, `H' = H ⋅ ...`;
  - typed Rust/typestate tokens when they are semantically meaningful: `Parent<Retired>`, `Crown<Ruling>`, `Startup<Validated>`, `Block<Sealed>`;
  - compact implication/containment forms: `g ⊢ q iff ...`, `A(Runtime) ⊆ A(role(path), state(path))`.
- Avoid overclaiming. The current docs repeatedly label target docs as `draft target note`, `implementation target`, or `not a next-slice plan` rather than claiming implementation success.
- Separate interpretation from definition. `procedure-notation.md` uses code fences for definitions and `Interpretation:` bullets immediately after.
- Keep claims monotonic and fail-closed. The call-graph design says missing dangerous facts become proof blockers, not safe defaults.
- Preserve the local vocabulary of evidence authority. The style distinguishes `record`, `projection`, `log`, `evidence`, `admit_D(rec)`, and decision input rather than calling all persisted data proof.
- When mentioning current implementation, cite code/test paths and status labels. The `invariant-ledger.md` style uses `Implemented`, `Partially implemented`, `Intended`, and `Not claimed`.
- Do not write a proof assistant transcript unless asked. The expected next artifact is a symbolic invariant/proposition outline in house notation, with proof obligations and blockers named, not a formalized Coq/Lean/Verus proof.

## Reusable terminology

Core runtime/authority terms:

- `Runtime`: executing process hydrated from an `Artifact`.
- `Parent`: Runtime with lineage authority under the current protocol.
- `Child`: Runtime that evaluates an assigned candidate and writes child-shaped evidence; it cannot promote itself.
- `Successor`: fresh Runtime from the selected artifact; becomes next Parent only after handoff/admission validation.
- `Crown`: lineage-scoped mutable authority; not a pid, branch, path, machine, or global singleton.
- `Crown<Ruling>`: permissioned active authority.
- `Crown<Locked>` / `Parent<Retired>`: predecessor authority-retirement / lock vocabulary.
- `LineageKey` / lineage id: coordinate for one authority lineage.
- `History`: durable authority surface over sealed lineage-local blocks.
- Projection/report/registry/log/UI state: evidence or operator surfaces, not History authority unless admitted by a decision-domain rule.

Surface/artifact terms:

- `Artifact`: source/checkouts/surfaces that can hydrate a Runtime.
- `ArtifactSurface = Immutable + Mutated + Ambient`.
- Current concrete Prototype 1 partition: `Immutable = crates/ploke-eval`, `Mutated = tool-description text files`, `Ambient = empty declared surface`.
- `Γₐ = (Vₐ, Eₐ, μₐ)`: artifact-bound code graph.
- `g = (R, W, F)`: surface grant with read/write/forbidden sets.
- `g ⊢ q`: grant containment for proposal `q`.
- `validₐ(q)`: material hash/spans still match expected proposal preconditions.
- `AttemptOutcomeₐ(q) = Applied(a', δ) | Rejected(reason)`.

Proof graph / checker terms:

- `BuildDomain`: named proof boundary over Cargo metadata, lockfile, package/target, feature/cfg/profile/toolchain, environment policy, proof-policy version, and immutable-surface digest.
- `ExpansionBoundary`: macro/build/proc-macro/include/external-summary boundary with expansion state and blocker reason.
- `ResolutionState`: `Resolved`, `CandidateSet`, `Ambiguous`, `Unresolved`, `ExternallySummarized`, `Blocked`.
- `EvidenceUse`: `ProofOnly`, `NavigationOnly`, `ProofAndNavigation`.
- `ObligationStatus`: `Admitted`, `Rejected`, `Blocked`.
- `ProofBlockerReason`: includes `MacroExpansionNotAvailable`, `ProcMacroSummaryMissing`, `BuildScriptSummaryMissing`, `ExternalDependencySummaryMissing`, `CfgDomainNotMaterialized`, `TypeResolutionMissing`, `DynamicDispatchUnbounded`, `ExternalCommandSummaryMissing`, `AuthorityEvidenceMissing`, `ProcessLifetimeEvidenceMissing`, `CanonicalIdentityMismatch`, `SchemaVersionMismatch`, `RustcInvocationEvidenceMissing`.
- `EffectClass`: includes process, async, authority, History, surface, durable-evidence, and external-summary effects.
- `ProcessLifetime`: `RuntimeBounded`, `Detached`, `Unknown`.
- `HandoffEvidence`: successor admitted, predecessor retired, exactly one successor.
- Current active checker invariant names in tests: `detached_process_successor_handoff` and `crown_ruling_lineage_uniqueness`.

## Existing definitions and proposition seeds

These are the strongest reusable formal seeds:

1. Procedure execution and state composition

```text
Exec(e, x, s) = s'   where s ∈ I_x and s' ∈ O_x
```

Use this if the new proposition needs to describe checker execution over an input proof-fact state. `procedure-notation.md` treats `x` as a specification, `e` as executor, and `s`/`s'` as typed states.

2. Recording versus forwarding

```text
Rec(s) ∧ ¬Fwd(s)   = record only
¬Rec(s) ∧ Fwd(s)   = forward only
Rec(s) ∧ Fwd(s)    = record and forward
```

Useful for separating proof artifact persistence from downstream admission.

3. Grant containment and material validity

```text
g ⊢ q  iff  Qr ⊆ R ∧ Qw ⊆ W ∧ Qw ∩ F = ∅
validₐ(q) iff ∀v ∈ Qw. hashₐ(file(μₐ(v))) = expected_hash_q(v)
applyₐ(q) is defined iff g ⊢ q ∧ validₐ(q)
```

Use as style precedent if the new invariant has a checker-side admission predicate.

4. Observation/admission distinction

```text
event e
observer o
observation obs = observe(o, e)
record rec = record(writer, obs)
evidence ev = admit_D(rec)
decision input d ∈ inputs(D) iff admit_D(rec) = Some(ev)
```

And the negative case:

```text
record(rec) does not imply admissible_D(rec)
projection(rec) ∨ log(rec) ∨ ui_state(rec)
  does not imply admissible_D(rec)
```

This is important for any proposition involving GraphRAG-visible proof facts, run records, or child self-eval output.

5. Primary theorem target for detached process + Crown authority

From `detached-process-callgraph-proof-target.md`, the desired shape is:

```text
For every admitted descendant Runtime produced from the genesis Parent under the ordinary Ploke transition system:

1. the policy-bearing immutable surface digest is preserved;
2. no operating-system process created by that Runtime can outlive that Runtime, except through an admitted successor-handoff transition;
3. any admitted successor can become Parent<Ruling> only after validating the predecessor sealed History head, active Artifact identity, and policy-bearing surface commitment;
4. for each lineage, at most one permissioned Ruler-state Parent exists at any time.
```

This is a target, not an implementation claim.

6. Surface-digest induction

```text
Base:
  Genesis Parent is admitted with immutable surface digest D.

Step:
  A Parent admitted under digest D may execute or admit a child/successor only after proving the candidate Artifact also carries digest D for the policy-bearing surface.

Conclusion:
  Every admitted descendant produced by ordinary succession preserves the same authority and process-safety rules.
```

This is likely the right induction frame for the new symbolic outline.

7. Successor handoff authority proposition

```text
The predecessor loses permissioned Ruler authority before or at the same logical boundary where the successor can obtain permissioned Ruler authority, and the sealed History/Crown evidence makes any overlap in Ruler authority invalid.
```

The proof outline should avoid reducing this to same-host process replacement.

8. Central lifetime obligation

From the implementation-target design:

```text
For each OperatingSystemProcessCreate site E:
  if E is not an admitted successor/lineage handoff,
  then every path from E to runtime termination is dominated by a proof of wait/reap
  or kill/reap of the process family.
```

This is still a target obligation. Current code has seed extraction and checker fixtures, not full CFG cleanup dominance.

9. Central authority theorem

```text
For every lineage L and time/transition interval T:
  the graph admits at most one live permissioned Ruler-state Parent for L.
```

Current tests check simplified fixtures and build-domain scoping; they do not yet prove the full runtime graph theorem.

10. Candidate proof-kernel obligations

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

These names are already in `callgraph-implementation-design-for-detached-process-proof.md` and should be reused unless there is a strong reason to rename.

## Current proof obligations and gaps

Existing target obligations:

- Name the `BuildDomain` for any proof claim: Cargo metadata, lockfile, package/target, features/cfg/profile/toolchain, environment policy, proof-policy version, immutable-surface digest.
- Preserve source and expansion provenance so macro-introduced effects point back to invocation/definition and build domain.
- Treat dangerous `CandidateSet`, `Ambiguous`, `Unresolved`, unaudited `ExternallySummarized`, and `Blocked` states as proof blockers.
- Inventory all operating-system process effects: `std::process::Command`, `tokio::process::Command`, `exec`/`pre_exec`, shells, FFI process creation, build-time processes, proc macros, dependency wrappers, test-only effects if in domain.
- Record command-builder dataflow: program, args, cwd, env, stdio, process group/session, containment, timeout/cancel policy, handle owner, journal/evidence records.
- Prove process-family containment for opaque commands; handle-only evidence is not enough for high-bar claims involving git/cargo/shells/editors/daemons.
- Model async task scopes because async tasks can own process or authority effects even though they are not OS processes themselves.
- Model authority-token constructors, private fields, move-only transitions, module visibility, `Parent<...>`, `Crown<Ruling>`, `Crown<Locked>`, `Parent<Retired>`, `Startup<Validated>`, `Block<Open>`, `Block<Sealed>`.
- Bind static proof artifacts to durable evidence: sealed History blocks, transition journals, successor invocation/ready/completion records, surface commitments, child evaluation records, walk lifecycle records.
- Fail closed on macro/build-script/proc-macro/cfg/external-summary gaps.

Current implemented/scaffolded pieces:

- `crates/ploke-records/src/proof_facts.rs` now has stable passive DTO vocabulary for build domains, cfg domains, rustc invocations, expansion boundaries, call sites/edges/resolutions, effect seeds, authority facts, and proof blockers.
- `proof_effects.rs` emits conservative proof facts for direct calls, method calls, process/async/durable evidence seeds, and macro boundaries. It marks proof-critical unknowns as proof evidence and navigation-only unknowns as non-proof.
- `proof_authority.rs` emits authority facts only for exact source sites admitted by configuration; structurally similar authority-like calls become blocked evidence.
- `ploke-db` proof graph storage keeps blockers visible for GraphRAG/checker queries and prevalidates schema/required fields before storage.
- `proof_invariant_checker.rs` exercises fixture-level `Pass`/`Fail`/`Blocked` outcomes for legal successor handoff, illegal detached spawn, two Crown rulings, macro/cfg/external gaps, navigation-only evidence, call-site/build-domain mismatch, unscoped blockers, and per-site independence.

Important gaps to keep explicit:

- Current checker fixtures are not a complete codebase proof. They operate over provided proof facts.
- Full compiler-grade macro expansion/HIR-backed call resolution is not implemented.
- Full control-flow cleanup dominance is not implemented.
- External dependency summaries and external command containment are not complete.
- Build scripts/proc macros are represented as boundaries and blockers, not fully semantically analyzed.
- `BuildDomain + ExpansionBoundary + proof-blocking unresolved statuses` is the recommended next narrow lane before broad call/effect extraction, per `macro-buildrs-callgraph-sequencing-survey.md`.

## Claims already framed as construction vs empirical evidence

Use these distinctions carefully:

Proved-by-construction / architecture-local claims:

- `proof_facts.rs` explicitly states passive DTOs do not validate proof, admit History, grant Crown authority, or execute build/proc-macro code. Authority records are inert by construction because `AuthorityFact::record_deserialization_grants_authority()` returns false and `AuthorityTerm::record_deserialization_grants_authority()` returns false.
- `EvidenceUse::can_satisfy_proof()` makes `NavigationOnly` unable to satisfy proof obligations by construction.
- `ObligationStatus::satisfies_proof()` returns true only for `Admitted`; `Blocked` and `Rejected` do not satisfy proof.
- `ProcessLifetime::Unknown` blocks detached-process proof by construction; `Detached` only passes with complete handoff evidence.
- `HandoffEvidence::satisfies_detached_successor()` requires successor admitted, predecessor retired, and exactly-one successor all admitted.
- The current checker is fail-closed at the fact level for unknown evidence-use values, schema mismatches, missing required fields, unscoped proof blockers, process effects without call sites, and navigation-only unresolved process calls.

Current architecture claims / partially implemented targets:

- `History is authority; scheduler/report/registry are projections` is `partially implemented / current local claim`.
- `Crown is lineage authority, not pid/path/branch` is a current architecture claim.
- `At most one valid Crown<Ruling> per lineage` is `partially implemented / target invariant`.
- `Successor handoff is a cross-runtime typed contract` is partially implemented.
- `Startup admission is not just binary ran` is intended / partially implemented for successor handoff.
- `Policy-bearing surface is protected` is current Prototype 1 policy, not a universal proof.
- `Child self-report is evidence, not promotion` is a current architecture claim.

Empirically evidenced / test-backed claims:

- `crates/ploke-eval/docs/prototype1-proof-ladder.md` uses `Proven` only for focused checks such as zero-admission persistence, parent patch-generation slot concurrency, child runner terminal failure evidence, and historical successful treatment evidence rebuild.
- The same proof ladder explicitly marks `Live child self-eval success` as blocked by registration/closure gap; it says the model/tool run can produce the intended patch but does not yet prove the successful child self-eval rung.
- `proof_invariant_checker.rs` empirically validates fixture-level checker outcomes, not global repository proof.
- `macro-buildrs-callgraph-sequencing-survey.md` includes repository survey facts such as zero workspace custom-build targets, many external dependency build scripts, workspace proc-macro crates, and process/async search counts. Treat those as observed survey facts for that repo state, not timeless invariants.

Do not phrase the next document as if Ploke already proves the long-horizon theorem. The correct wording is closer to: "This proposition names the normalized proof obligation the checker must discharge" or "This invariant is currently represented by DTO/checker fixtures and remains blocked until ...".

## Recommended placement and name for the new symbolic outline

Recommended placement:

```text
docs/workflow/evalnomicon/drafts/formal/detached-process-crown-symbolic-invariant-outline.md
```

Reasoning:

- The artifact will be a formal/symbolic proposition outline, so it belongs with the formal drafts rather than inside active agent handoffs.
- The active proof-spine directory should link to it as implementation context, but should not be the primary home for the formal statement.
- The name keeps it adjacent to `detached-process-callgraph-proof-target.md` and `callgraph-implementation-design-for-detached-process-proof.md` while making clear that it is an outline, not a proof artifact.

Suggested header:

```text
# Detached Process And Crown Symbolic Invariant Outline

Status: draft symbolic proposition outline, not yet discharged proof
Date: 2026-06-19
Depends on:
- `detached-process-callgraph-proof-target.md`
- `callgraph-implementation-design-for-detached-process-proof.md`
- `../../../src/prototype1/invariant-ledger.md`
- `../../../../active/agents/2026-06-19_detached-process-crown-proof-spine/traceability.md`
```

Suggested section skeleton:

```text
## Purpose
## Informal claim
## Domains and evidence carriers
## Admitted transition system
## Proposition 1: Immutable surface digest induction
## Proposition 2: Detached process safety except admitted successor handoff
## Proposition 3: Crown/Ruler uniqueness per lineage
## Proof obligations
## Blocker semantics
## Current implementation correspondence
## Non-goals and not-yet-proved cases
```

Suggested proposition naming:

- `ImmutableSurfaceDigestInduction`
- `DetachedProcessSuccessorException`
- `CrownRulingLineageUniqueness`
- combined theorem name: `AdmittedLineageProcessAndCrownSafety`

Use the existing checker/test names when mapping to code:

- `detached_process_successor_handoff`
- `crown_ruling_lineage_uniqueness`

## Drafting cautions for the next worker

- Do not use `proof` when the repo only has a target, passive DTO, checker fixture, or empirical run evidence. Say `target`, `obligation`, `fixture-backed checker behavior`, or `current architecture claim`.
- Do not collapse process lifetime into `.spawn()` syntax. The target defines detached process as a lifetime property.
- Do not collapse Crown authority into operating-system process uniqueness. Overlapping OS processes can exist during handoff; overlapping permissioned Ruler authority is the forbidden case.
- Do not let GraphRAG/navigation evidence satisfy proof obligations. Reuse `EvidenceUse` vocabulary.
- Do not silently pass unknown macro/build/cfg/external/dependency facts. Reuse typed blocker reasons.
- Do not treat child self-eval or run logs as promotion authority. They can be admitted evidence only through explicit decision-domain rules.
- If proposing code-facing types, follow the trait-first/local-typestate style: formal contract/algebra first, concrete runtime instance second.
