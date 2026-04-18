# ploke-eval Procedure Model

- date: 2026-04-18
- task title: ploke-eval procedure model
- task description: eval-specific instantiation of the shared formal procedure language for the `ploke-eval` rewrite, defining the larger run procedure, canonical state families, admissible transitions, and the boundary between semantic outputs, evidence, and emitted projections

## Purpose

This note is the conceptual source of truth for the `crates/ploke-eval/src/inner`
rewrite.

It does not try to inherit the implicit structure of the current
`ploke-eval` crate. It defines the larger semantic object the rewrite should
implement, so that local modules and type-state transitions are derived from an
explicit procedure model rather than from whichever branch of `runner.rs`
happens to be in view.

This note is eval-specific. It uses the shared formal language from
`formal-procedure-notation.md`, but it is not a protocol architecture note and
should not import `ploke-protocol`'s packet-review abstractions as the native
carrier for eval execution.

## Relation To Shared Procedure Language

The shared procedure notation remains the metalanguage for:

- typed admissible states
- typed transitions and composed procedures
- fork and merge structure where real
- recording vs forwarding
- bounded inquiry procedures

For `ploke-eval`, those ideas are instantiated over benchmark-run execution
rather than over adjudication procedures.

Reusable from the shared notation:

- `Exec(e, x, s) = s'`
- step-local typed input/output boundaries
- runtime procedure graphs larger than any single compile-time type
- bounded inquiry as the right model for agent execution
- `Rec(s)` vs `Fwd(s)` as separate concerns

Not adopted as the primary organizing vocabulary for eval:

- packet-centric subject modeling
- target/supporting/evidential metric vocabulary as the top-level frame
- protocol-local judgment abstractions

`ploke-eval` is execution-first. It may produce evidence later consumed by
`ploke-protocol`, but it does not itself become a protocol crate.

## Larger Semantic Object

The larger object is a bounded benchmark-run execution procedure with an
optional bounded inquiry subprocedure.

At the coarsest level:

```text
RunIntent
  -> FrozenRunSpec
  -> RunRegistration
  -> CheckedOutWorkspace
  -> RuntimeReadyWorkspace
  -> PreparedWorkspace
  -> PatchCandidate?
  -> ValidationResult?
  -> PackagingResult?
  -> RunResult
```

The patch-generation slice is therefore not the whole crate. It is one
subprocedure inside a larger run procedure:

```text
PreparedWorkspace
  -> InquiryExecution
  -> PatchCandidate
```

The agent turn itself is not best modeled as a one-shot transform. It is a
bounded inquiry:

```text
s_setup -> q_0 -> q_1 -> ... -> q_t -> s_patch
```

where:

- `s_setup` is a prepared workspace state
- `q_i` are bounded inquiry states during agent execution
- `q_t` is a terminal inquiry state
- `s_patch` is the produced patch-candidate state

## Scope Boundary

This procedure model covers:

- run intake and preparation
- run-policy resolution
- checkout and runtime boot
- workspace seeding or indexing
- bounded inquiry execution for patch generation
- patch extraction
- optional validation
- optional packaging into benchmark-specific submission form
- emission of run-local artifacts

This procedure model does not treat these as part of the native execution core:

- replay and inspection
- closure accounting
- campaign export
- protocol adjudication and aggregation

Those are downstream consumers, operators, or adapters over emitted eval state.

## Canonical State Families

The rewrite should distinguish at least three classes of objects:

1. semantic state
2. evidence state
3. emitted projections

### Semantic State

Semantic states are the states whose values are needed to continue the
procedure.

Recommended eval-specific state families:

- `RunIntent`
  The initial subject for a run. Carries benchmark identity or ad hoc task
  identity plus requested policy inputs and overrides.
- `FrozenRunSpec`
  The frozen executable specification after all admissible resolution has
  completed:
  dataset provenance, repo path, output root, model/provider selection, budget,
  run mode, and packaging context.
- `RunRegistration`
  The first-class persisted authority record for a run, keyed by stable run id
  and carrying the frozen executable specification, schema/version identity,
  lifecycle status, artifact manifest roots, and cache-relevant fingerprints.
- `CheckedOutWorkspace`
  The repository state after checkout/reset has succeeded and the workspace root
  is ready for runtime attachment.
- `RuntimeReadyWorkspace`
  The checked-out workspace plus booted runtime dependencies such as DB, config
  sandbox, embedder activation, and app/runtime handles.
- `PreparedWorkspace`
  The runtime-ready workspace after seeding or indexing has completed and the
  procedure can either stop at setup or proceed into inquiry execution.
- `InquiryState`
  Internal bounded-inquiry execution state for the agent path. This should not
  necessarily be one public type; it names the semantic role.
- `PatchCandidate`
  The patch-generation result before downstream validation or packaging. It
  should carry the produced diff or equivalent patch payload plus provenance on
  how it was obtained.
- `ValidationResult`
  The result of applying build/test/benchmark checks to a patch candidate.
- `PackagingResult`
  The benchmark-specific packaging of a patch candidate or validated result,
  such as Multi-SWE-bench submission payloads.
- `RunResult`
  The terminal semantic result of the run procedure, including the highest-level
  resolved outcome and links to the semantic products produced along the way.

### Configuration Authority And Discovery

One ambiguity the rewrite should remove early is where execution-relevant
configuration becomes authoritative for downstream systems.

The intended answer is:

- `RunIntent` is the operator or caller request
- `FrozenRunSpec` is the canonical immutable executable configuration
- `RunRegistration` is the persisted shared authority surface for discovery,
  caching, lifecycle tracking, and downstream consistency

`RunRegistration` should be the thing other systems consult when they need to
know:

- which runs exist
- which runs completed or failed
- which exact configuration each run used
- which schema/version governed the run
- where the canonical artifacts for that run live
- which cache keys or fingerprints are associated with the run

This means downstream systems should not infer run configuration by crawling a
directory of loosely related files or by reinterpreting evidence artifacts whose
meaning belongs to earlier phases.

One useful system-level carrier is a persistent `RunRegistry` or equivalent
authority store whose unit record is `RunRegistration`.

The important design property is not the exact storage mechanism. It is that
there is one discoverable shared source of truth for run identity and frozen
configuration.

The registration surface may be implemented as:

- immutable versioned records
- an append-only lifecycle ledger
- a canonical record plus append-only status updates

Any of those is acceptable so long as historical run meaning remains stable and
later lifecycle updates do not rewrite the configuration identity of an earlier
run.

### Evidence State

Evidence state is recorded because it explains what happened, supports later
inspection, or is useful to downstream consumers, but it is not the semantic
carrier of the run procedure itself.

Examples in the current domain:

- repo state after checkout
- indexing status and parse failures
- setup summary and checkpoint metadata
- turn events and turn summaries
- prompt/response provenance
- proposal snapshots and expected-file change summaries
- selected model/provider/endpoint provenance
- validation logs and failure records

### Emitted Projections

Emitted projections are filesystem or export materializations of semantic state
plus evidence. They are not the canonical outputs of the core procedure.

Current legacy emitted surfaces include:

- `run.json`
- `record.json.gz`
- `execution-log.json`
- `repo-state.json`
- `indexing-status.json`
- `snapshot-status.json`
- `agent-turn-trace.json`
- `agent-turn-summary.json`
- `llm-full-responses.jsonl`
- `multi-swe-bench-submission.jsonl`
- checkpoint and snapshot database files

The rewrite should therefore treat emission as a projection layer over semantic
products and evidence, not as the thing the pipeline fundamentally is.

These current surfaces are not presumptively retained. They are inputs to an
artifact audit, not commitments the rewrite must preserve.

The current reduced artifact decision for this rewrite is recorded in:

- [2026-04-18_ploke-eval-canonical-artifact-set.md](./2026-04-18_ploke-eval-canonical-artifact-set.md)

### Artifact Audit Policy

Before implementing the rewritten pipeline emitters, the emitted surface should
be reduced to a minimal canonical set.

Useful artifact classes are:

1. canonical durable records
2. durable attachments
3. derived operator views

Proposed discipline:

- canonical durable records are the authoritative stored records other systems
  should reference
- durable attachments are stored only when the payload is too large, too raw, or
  too operationally useful to inline into a canonical record
- derived operator views are rendered for convenience and should not be stored
  if they are lossless subsets or reformatting of canonical records

Elimination rules:

- if one stored artifact is a strict subset of another canonical stored record,
  the subset should not exist
- if an artifact is a lossless reformat of another canonical stored record, it
  should not exist as a separate stored file
- if a view can be rendered deterministically from canonical records plus
  durable attachments, prefer on-demand rendering over duplicate persistence
- large raw payloads may survive as attachments if inlining them would make the
  canonical record impractical

Likely target shape for the rewrite:

- one canonical frozen-spec or registration surface
- one canonical run/evidence surface
- one packaging payload only where benchmark export actually requires it
- a small attachment set for things like snapshots or raw provider traces when
  they are not practical to inline

The point of the audit is not merely to rename files. It is to collapse
duplicated stored meaning down to one reference point per kind of information.

## Recording And Forwarding

Using the shared notation:

- externally meaningful semantic states are forwarded and recorded by default
- evidence states are usually recorded and sometimes forwarded
- emitted projections are recorded artifacts, not forwarded semantic states

Useful discipline for the eval rewrite:

```text
RunIntent           : Fwd, Rec
FrozenRunSpec       : Fwd, Rec
RunRegistration     : Fwd, Rec
CheckedOutWorkspace : Fwd, Rec
RuntimeReadyWorkspace : Fwd, Rec
PreparedWorkspace   : Fwd, Rec
InquiryState        : internal Fwd, selectively Rec
PatchCandidate      : Fwd, Rec
ValidationResult    : Fwd, Rec if attempted
PackagingResult     : Fwd, Rec if attempted
RunResult           : terminal Rec
```

Important consequence:

- `record.json.gz` is a durable projection over run state and evidence
- it is not itself the semantic state machine
- `CheckedOutWorkspace` and `RuntimeReadyWorkspace` should be recorded by
  attestation or summary, not by attempting to serialize live runtime handles
- `InquiryState` does not require every in-memory microstate to be persisted;
  selective event or evidence recording is sufficient

## Major Procedure Graph

The larger `ploke-eval` procedure should be treated as a runtime graph with
clear step-local type boundaries.

One useful coarse graph is:

```text
intake
  -> freeze_spec
  -> register
  -> checkout
  -> boot_runtime
  -> prepare_workspace
  -> branch
       -> setup_terminal
       -> execute_inquiry
            -> extract_patch
            -> validate?
            -> package?
  -> finalize_run
  -> emit
```

This graph is not purely linear because:

- registration is established early and finalized later with outcome and
  artifact references
- setup-only is a real terminal branch from `PreparedWorkspace`
- inquiry execution is an internal bounded procedure, not a single edge
- packaging is benchmark-specific and may be absent for some run sources
- validation is conceptually downstream of patch generation, not part of setup

## Admissible Transition Boundaries

The rewrite should make these boundaries explicit:

### 1. `RunIntent -> FrozenRunSpec`

This transition resolves all admissible ambient or persisted configuration into
a frozen executable specification.

By the end of this transition:

- repo and output roots are concrete
- source provenance is concrete
- budget is concrete
- model/provider/endpoint policy is concrete enough to execute
- packaging context is concrete

This is where ambient config should stop leaking into the rest of the system.

### 2. `FrozenRunSpec -> RunRegistration`

This transition publishes the frozen executable specification into the shared
authority surface.

By the end of this transition:

- the run has a stable run id
- the frozen executable configuration is durably recorded
- schema and version identity are explicit
- canonical storage roots and artifact-manifest expectations are explicit enough
  for downstream discovery
- cache-relevant fingerprints are explicit enough for reuse and consistency

This is the point after which downstream systems should be able to discover the
run without reconstructing its meaning from unrelated artifacts.

### 3. `RunRegistration -> CheckedOutWorkspace`

This transition establishes repo state.

By the end:

- the repository root exists
- the requested checkout target is materialized
- repo-state evidence is available

### 4. `CheckedOutWorkspace -> RuntimeReadyWorkspace`

This transition boots the runtime surface.

By the end:

- the DB or checkpoint restore is ready
- config sandbox state is assigned
- runtime/app services are booted for the chosen run mode
- model/provider selection is injected into runtime state

### 5. `RuntimeReadyWorkspace -> PreparedWorkspace`

This transition prepares the workspace for actual execution.

By the end:

- workspace seeding or indexing has completed
- setup evidence is available
- the workspace is admissible either for setup-only termination or for inquiry
  execution

This is the most important seam in the current rewrite.

### 6. `PreparedWorkspace -> PatchCandidate`

This is the patch-generation subprocedure.

For the setup-only branch, this transition is not taken.

For the agent branch, this transition is realized as a bounded inquiry
procedure:

- prompt construction
- agent execution
- tool interaction
- terminal turn outcome
- patch extraction

By the end:

- the patch payload is concrete
- patch provenance is concrete
- proposal and expected-file evidence is available

### 7. `PatchCandidate -> ValidationResult`

This transition is conceptually distinct from patch generation itself.

Validation must not be allowed to redefine what the patch candidate was. It may
only assess it.

### 8. `PatchCandidate | ValidationResult -> PackagingResult`

Packaging is an adapter step from generic run output into benchmark-specific
submission form.

This step must not own patch semantics. It consumes them.

### 9. `... -> RunResult`

The terminal run result summarizes the procedure outcome without collapsing away
the produced semantic objects and evidence.

Publishing the terminal result should also finalize the registration surface
with outcome and canonical artifact references, whether that is implemented as
an updated canonical record or as append-only lifecycle material.

## Core Invariants

The rewrite should preserve these invariants structurally:

### Frozen Plan Invariant

After `FrozenRunSpec`, execution-relevant configuration is frozen. Later stages
may read it but should not keep re-resolving ambient state.

### Registration Authority Invariant

`RunRegistration` is the shared discovery and consistency surface for run
identity, frozen configuration, lifecycle status, and canonical artifact
references. Downstream systems should consult it rather than reverse-engineer
configuration from incidental evidence files.

### Prepared Workspace Invariant

`PreparedWorkspace` is the shared reusable product of the setup spine. It is
the stop point for setup-only and the sole admissible input to patch inquiry.

### Patch Semantics Invariant

`PatchCandidate` is the canonical patch-generation product. Packaging and
emission are downstream adapters over it, not alternate sources of truth.

### Evidence Separation Invariant

Evidence explains the run and supports downstream consumers, but it does not
replace semantic state as the carrier of control flow.

### Emission Separation Invariant

Filesystem artifacts are projections. Emitting or not emitting a particular
artifact must not change the semantic interpretation of the run result.

### Artifact Minimality Invariant

Stored artifacts should have one canonical durable reference point per kind of
information. Subset records and lossless duplicate views should be eliminated.

### Recorded-State Invariant

Externally meaningful semantic states are recorded by default. Internal
microstates may be selectively recorded, but the system should bias toward
durable accounting rather than implicit transient meaning.

### Consumer Boundary Invariant

Replay, inspection, campaign export, closure, and protocol work consume emitted
state and evidence. They do not belong inside the native patch-generation
execution core.

## Relation To ploke-protocol

`ploke-eval` and `ploke-protocol` should share formal language where that
language is genuinely general:

- typed procedures
- typed transitions
- bounded inquiry
- recording vs forwarding

But they should keep different native domain carriers:

- `ploke-eval` owns run execution, patch generation, validation, packaging, and
  emitted run evidence
- `ploke-protocol` owns analysis/adjudication procedures over projected eval
  evidence and other subjects

The eval rewrite should therefore expose clearer adapter surfaces for protocol
consumption, but it should not distort its internal model to look like a
protocol packet system.

## Immediate Implementation Consequences

For the `inner` rewrite, this model suggests:

- `inner/core` should define the stable eval domain types and invariants
- `inner/core` should define the frozen-spec and registration authority types
- a first-class registry or equivalent authority surface should exist from the
  start of the rewrite rather than being bolted on later
- the concrete stored artifact shape should follow the canonical artifact note
  rather than the legacy run-directory surface
- setup should be rewritten first around the shared `PreparedWorkspace` product
- patch generation should consume only `PreparedWorkspace`
- validation and packaging should stay separate from patch extraction
- emitters should project semantic state plus evidence into a reduced canonical
  artifact set
- emitted artifacts should be audited before implementation so the rewrite does
  not recreate legacy duplication under new names
- CLI code should become translation into `RunIntent` or operator requests, not
  the place where execution semantics live

### Developer Guardrail Rationale

The rewrite should use a generic carrier such as:

```text
RunPipeline<S, M>
```

with explicit state markers for the coarse admissible transitions and an
explicit mode marker for the admissible execution branch.

This is not only a type-theory preference. It is a maintainability guardrail.

The project is expected to be edited by:

- core maintainers with full context
- collaborators with partial context
- LLM-assisted workflows that may optimize for the immediate local fix

Those conditions create strong pressure toward ad hoc convenience edges unless
the codebase makes the intended procedure shape obvious and mechanically
enforced.

So the design goal is:

- stored enums continue to describe recorded outcomes and evidence
- the live pipeline carrier constrains what transitions are admissible at
  compile time
- the live pipeline carrier also constrains which branch transitions are
  admissible for setup-only versus patch-generation runs
- incorrect local wiring should surface as a compile error rather than as silent
  semantic drift

Concretely:

- `S` should encode coarse irreversible stage
- `M` should encode the admissible branch, such as `setup_only` or
  `patch_generation`

The immediate payoff is at the most important seam:

```text
RunPipeline<Prepared, SetupOnly>
  -> setup_terminal

RunPipeline<Prepared, PatchGeneration>
  -> bounded_inquiry
```

That split prevents a future contributor from accidentally entering inquiry
execution on a setup-only run merely because both paths happen to have reached
the same prepared workspace evidence.

This does not require every inquiry microstep to become a typestate. The useful
guardrail is at the coarse irreversible boundaries:

- intake
- freeze spec
- registration
- checkout
- runtime boot
- workspace preparation
- patch generation
- validation
- packaging
- finalization

That gives the development team a strong default path toward preserving the
conceptual framework even when not every contributor holds the whole model in
their head.

One plausible first type-state slice is:

```text
RunIntent
  -> FrozenRunSpec
  -> RunRegistration
  -> CheckedOutWorkspace
  -> RuntimeReadyWorkspace
  -> PreparedWorkspace
```

followed by:

```text
RunPipeline<Prepared, SetupOnly>
  -> RunResult

RunPipeline<Prepared, PatchGeneration>
  -> PatchCandidate
  -> PackagingResult?
  -> RunResult
```

## Rewrite Sequence

Recommended order for the rewrite:

1. define the eval domain types and registry-authority contract in `inner/core`
2. decide the reduced canonical artifact set and eliminate redundant stored
   records on paper before implementing emitters
3. encode the setup spine up to `PreparedWorkspace`
4. encode the bounded inquiry patch subprocedure from `PreparedWorkspace`
5. define `PatchCandidate`, `ValidationResult`, `PackagingResult`, and
   `RunResult`
6. move artifact writing into explicit emitters over the reduced canonical
   record set
7. leave replay, inspection, closure, and protocol as downstream consumers over
   the new emitted surfaces

## Short Take

The rewrite should treat `ploke-eval` as a run-execution procedure with a
bounded inquiry patch subprocedure.

Its source of truth should be:

- semantic state families
- explicit admissible transitions
- explicit evidence products
- explicit configuration authority and discovery surfaces
- explicit emitted projections

not:

- CLI command shape
- legacy file layout
- current `runner.rs` branch structure
- protocol-local abstractions imported wholesale into eval
