# Prototype 1 Edit Surface Model

Date: 2026-05-07

Task title: Surface-bounded edit generation for Prototype 1

Task description: Define the terms and procedure shape for connecting the
`ploke-tui` edit proposal machinery to the Prototype 1
`Runtime -> Surface(Artifact) -> derived Artifact` create transition without
making UI proposal storage, raw patch text, or projection files into authority.

Related planning files:

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-tui/src/tools/code_edit.rs`
- `crates/ploke-tui/src/rag/tools.rs`
- `crates/ploke-tui/src/rag/editing.rs`
- `crates/ploke-tui/src/parser.rs`
- `crates/ploke-db/src/helpers.rs`
- `crates/ploke-core/src/io_types.rs`
- `docs/workflow/evalnomicon/drafts/formal-procedure-notation.md`
- `docs/workflow/evalnomicon/chat-history/framework-ext-01.md`
- `docs/workflow/evalnomicon/chat-history/framework-ext-02.md`
- `docs/workflow/evalnomicon/chat-history/framework-ext-03.md`
- `docs/workflow/evalnomicon/chat-history/prototype-1-probability.md`
- `docs/active/todo/2026-05-05_long-horizon.md`

## Purpose

The current Prototype 1 docs already name the intended create shape:

```text
Runtime -> Surface(Artifact) -> PatchAttempt
PatchAttempt + base Artifact -> derived Artifact
derived Artifact -> hydrated Runtime
```

For the `ploke-tui` bridge, `PatchAttempt` is too broad as an implementation
term. The existing tool path produces staged `EditProposal` values. A proposal
may contain canonical semantic edits, non-semantic patch edits, or file creation
requests. It is an intermediate procedure state, not an admitted Artifact
transition and not History authority.

This note defines the terms for the first real edit-surface design:

```text
Runtime -> SurfaceGrant(Artifact) -> EditProposal
EditProposal + boundary check + apply -> derived Artifact
derived Artifact -> child Runtime self-evaluation
```

## Basic Sorts

```text
A      = set of Artifacts
R      = set of Runtimes
C      = set of operation coordinates
G      = set of surface grants
Q      = set of edit proposals
T      = set of artifact transitions
H      = set of History records / blocks
X      = set of procedure specifications
E      = set of executors
S      = set of typed procedure states
```

Current concrete anchors:

```text
Artifact              ~= checkout state / tree state
Runtime               ~= process hydrated from Artifact
SurfaceCommitment     ~= Immutable + Mutated + Ambient
Coordinate            ~= loop_graph::Coordinate
OperationTarget       ~= loop_graph::OperationTarget
EditProposal          ~= ploke-tui staged proposal
CanonicalEdit         ~= file + canon + node_type + replacement code
WriteSnippetData      ~= resolved byte-span edit with expected file hash
NsWriteSnippetData    ~= non-semantic diff edit with expected file hash
```

## Core Relations

```text
Hydrates(a, r)        := Artifact a can hydrate Runtime r
Target(c)             := target Artifact of operation coordinate c
Generator(c)          := Runtime generating the operation at c

Grant(g, c)           := surface grant g applies to coordinate c
Read(g)               := readable surface allowed by g
Write(g)              := writable surface allowed by g
Touches(q)            := files/spans/symbols touched by edit proposal q

Within(q, g)          := Touches(q) is contained by Write(g)
Applies(q, a) = a'    := proposal q applied to Artifact a yields Artifact a'
Admit(t)              := transition t is admitted into History
```

Important negative relations:

```text
EditProposal(q) does not imply ArtifactTransition(q)
EditProposal(q) does not imply Admit(q)
UIApproval(q) does not imply HistoryAuthority(q)
Preview(q) does not imply Evidence(q)
```

## Substrates And Operators

A `Surface` should be understood as a boundary relation:

```text
Surface(operator, substrate, mode, policy)
```

It is not a standalone path list. It relates an operator to a substrate under a
mode such as read, write, admit, observe, or communicate.

Primary substrates:

```text
Artifact    = source/tree material that can hydrate a Runtime
History     = sealed authority/evidence substrate
Channel     = typed cross-runtime message buffer
Tree        = artifact graph and backend handles
Projection  = operator-facing derived view or telemetry stream
```

The most important writable substrate for create is `Artifact`. The writable
surface over an `Artifact` exists only for `Parent<Ruling>` or for an executor
acting inside a `Parent<Ruling>`-authorized create transition.

`Runtime` needs more care. It is not a substrate in the same family as
`Artifact`, `History`, `Channel`, or `Tree`. A running Runtime contains
materialized information: policy compiled into code, constants/statics included
in the binary, in-memory state, process identity, and invocation context. That
information can be observed or recorded, and it can influence procedure
execution, but ordinary edit-surface containment is not over the Runtime
itself. We do not grant a writable `Surface(Runtime)` in the candidate-create
path. We grant a Runtime authority to operate over an Artifact surface.

Operators:

```text
Parent<Ruling>        = lineage authority operator
Child                 = evaluation operator
Successor             = startup/admission operator before becoming Parent<Ruling>
Generator Runtime     = executor that proposes edits under a create procedure
TUI/tool harness      = executor/projection surface that stages proposals
Artifact backend      = mechanized executor over Artifact/Tree material
History admission     = authority-gated executor over History
Operator/human        = observer or explicit human-in-the-loop executor
```

Allowed near-term relations:

```text
Parent<Ruling> -> Artifact : read/write within SurfaceGrant
Parent<Ruling> -> Tree     : materialize/install candidate Artifacts
Parent<Ruling> -> History  : admit/seal through Crown
Parent<Ruling> -> Channel  : write plans/handoffs, read child results

Child          -> Artifact : read/execute/evaluate only
Child          -> Channel  : read invocation, write self-evaluation/result
Child          -> History  : no direct admission authority

Successor      -> Artifact : read/validate before parent admission
Successor      -> History  : read sealed predecessor head
Successor      -> Channel  : read handoff/ack path
Successor      -> Parent<Ruling> only after startup admission

TUI/harness     -> Artifact : propose/apply only as executor under Parent grant
Backend         -> Artifact : mechanized realization/checking
Operator        -> Projection : inspect/control view, not authority
```

So a Child may have surfaces, but they are not candidate-generation writable
surfaces:

```text
Surface(Child, Artifact, read, evaluation_policy)
Surface(Child, Channel, write, result_policy)
```

The ordinary candidate-generation writable surface is:

```text
Surface(Parent<Ruling>, Artifact, write, create_policy)
```

## Code Graph As Derived View

`ploke-tui` is not only an edit harness. It is also the current access path to
the parsed/indexed code graph. That code graph should not be modeled as an
independent authority substrate. It is a derived semantic view over an Artifact.

```text
Parse(Artifact) = CodeGraph
Store(CodeGraph) = DatabaseIndex
Resolve(DatabaseIndex, CanonicalTarget) = MaterialSpan
```

The database is the materialized index for the view. It can mediate a surface,
but it does not own the surface authority.

```text
Artifact          = material source/tree substrate
CodeGraph         = semantic projection of Artifact
DatabaseIndex     = persisted/queryable representation of CodeGraph
MaterialSpan      = file path + byte range + expected source hash
```

The concrete current path for canonical edits is:

```text
CanonicalEdit {
  file,
  canon,
  node_type,
  code
}

Resolve via ploke_db helpers
  -> EmbeddingData {
       file_path,
       file_tracking_hash,
       start_byte,
       end_byte,
       namespace,
       ...
     }

EmbeddingData
  -> WriteSnippetData {
       file_path,
       expected_file_hash,
       start_byte,
       end_byte,
       replacement,
       namespace
     }
```

This means the code graph acts as a semantic mediator between:

```text
SurfaceGrant over Artifact
  and
concrete writable material spans
```

The containment check should therefore have two layers:

```text
Semantic containment:
  resolved code graph node/symbol is inside the granted semantic surface

Material containment:
  resolved file/span/hash is inside the granted writable Artifact surface
```

When only material surface information is available, material containment is the
required authority check. Semantic containment is the stronger future shape once
surface grants can name canonical symbols, modules, code graph regions, or
component identities.

The staleness invariant is critical:

```text
CodeGraph must be tied to the Artifact it projects.
```

A `DatabaseIndex` produced from one Artifact must not authorize edits against a
different Artifact unless the relevant file/node hashes still match and the
projection is explicitly accepted as valid for that target Artifact.

So the relation should be:

```text
Projects(CodeGraph, Artifact, projection_hashes)
```

and a canonical edit is admissible only if:

```text
Projects(code_graph, target_artifact, hashes)
Resolve(code_graph, canonical_target) = material_span
material_span.expected_hash matches target_artifact content
material_span is within Write(surface_grant)
```

This is where `ploke-tui` has special value for Prototype 1. It can provide the
adapter from material patch to semantic code-graph delta:

```text
Patch / ArtifactDelta
  -> CodeGraphDelta
  -> candidate RuntimeDelta
```

`ploke-eval` should consume that through a trait adapter. It should not import
`ploke-tui` database internals directly into History authority paths.

## Runtime And Role

A `Runtime` is the executing environment hydrated from an Artifact. A `Role` is
the protocol/authority position that Runtime occupies.

Prototype 1 should treat this as a prescriptive invariant:

```text
forall r in Runtime, exists exactly one Role<State> such that InRole(r, Role<State>)
```

In words:

```text
Every live Runtime inhabits exactly one Role<State>.
```

So the core relation is:

```text
Artifact hydrates Runtime
Runtime inhabits exactly one Role<State>
Role<State> determines admissible Surfaces and transitions
```

Consequences:

```text
Runtime<Child<_>> cannot hold a writable Artifact surface for create.
Runtime<Successor<_>> is not also Parent during handoff.
Runtime<Successor<Validated>> may transition into Runtime<Parent<Ruling>>.
Runtime<Parent<Ruling>> may grant writable Artifact surfaces under create policy.
```

Role transition is an authority transition, not a local status write. A process
cannot become `Parent<Ruling>` by setting a flag, choosing a branch name, or
writing a projection file. It becomes `Parent<Ruling>` only through the
admission transition for that role.

This invariant also leaves room for future multi-agent runtimes. A Runtime may
eventually contain multiple threads, processes, or agents, such as a code
editing agent, a code reviewing agent, and a planning agent. Those internal
agents are not separate top-level `Role<State>` occupants in this model. They
operate inside the authority envelope of the one Role held by the Runtime.

Future internal permission systems should therefore be derivative:

```text
Runtime<Parent<Ruling>>
  contains agent_i with delegated permissions derived from Parent<Ruling>
```

not:

```text
Runtime has Parent<Ruling> and Child<Evaluating> simultaneously
```

The derivative agent-permission framework is future work. The invariant needed
now is that the outer Runtime has one Role, and all surfaces, channel writes,
History admissions, and handoff transitions are indexed by that Role.

## Operation Coordinate

The intended coordinate from `prototype1_state::mod` is:

```text
OperationCoordinate = (generator Runtime, target Artifact)
```

The current code already has the first carrier:

```text
Coordinate {
  runtime_id,
  target: OperationTarget
}
```

For v1, the ordinary self-improvement create step should use:

```text
c = Coordinate {
  runtime_id: active_parent_runtime,
  target: OperationTarget::Artifact {
    artifact_id: active_parent_artifact
  }
}
```

Cross-lineage targets, ancestor targets, sibling targets, artifact sets, and
patch sets are native extensions of this coordinate model, but they are not
required for the first TUI-backed create path.

## Surface Grant

`SurfaceGrant` is the missing structural object. It should be authority-bearing
inside the create transition, not a UI filter and not a scheduler/report field.

Minimal shape:

```text
g = {
  coordinate: c,
  readable: SurfaceReadSet,
  writable: SurfaceWriteSet,
  immutable: SurfaceConstraint,
  ambient: SurfaceReadSet,
  grantor: Parent<Ruling>,
  policy: SurfacePolicyId
}
```

For v1:

```text
immutable = crates/ploke-eval
writable  = selected tool-text files or canonical semantic targets
ambient   = empty or read-only context handles
```

The current sealed `SurfaceCommitment` is still the admission-side commitment:

```text
SurfaceCommitment = Immutable + Mutated + Ambient
```

The grant is more operational than the commitment:

- `SurfaceGrant` says what the generator may inspect or edit.
- `SurfaceCommitment` records what changed or stayed fixed across the admitted
  Artifact transition.

Those two should be related but not collapsed.

## Proposal

`EditProposal` is a staged candidate edit produced by the TUI/tool harness.

Current concrete fields include:

```text
q = {
  proposal_id,
  request_id,
  parent_id,
  call_id,
  proposed_at_ms,
  edits,
  files,
  edits_ns,
  preview,
  status,
  is_semantic
}
```

Prototype 1 should interpret it as:

```text
Proposal(q, c, g)
```

not merely as a pending UI approval. The proposal id is useful evidence and a
join key, but it is not the semantic identity of the operation.

The preferred v1 proposal mode is canonical semantic edit:

```text
CanonicalEdit = {
  file,
  canon,
  node_type,
  code
}
```

The tool resolves that to:

```text
WriteSnippetData = {
  file_path,
  expected_file_hash,
  start_byte,
  end_byte,
  replacement,
  namespace
}
```

Non-semantic patch edits may remain available, but they should be treated as a
broader write mode because their touched range is effectively whole-file unless
a stronger typed range extraction is added.

## Create Procedure

Let `x_create` be the create procedure.

```text
s0 = Parent<Ruling> carrying runtime r and active artifact a

c  = Coordinate {
       generator = r,
       target = OperationTarget::Artifact(a)
     }

g  = SurfaceGrant for c

s1 = Exec(parent, choose_surface, s0)
     carries c, g

q  = Exec(generator_or_harness, generate_edit, s1)
     where q in Q

s2 = Exec(surface_checker, check_proposal, q)
     allowed iff Within(q, g)

t  = Exec(io_backend, apply_checked_edit, s2)
     where t = (a -> a')

s3 = Exec(history_admission, admit_candidate_artifact, t)
     carries derived artifact a'
```

Then child execution begins:

```text
r' = Hydrate(a')
child_result = Exec(r', self_evaluate, a')
```

So the model is not:

```text
Child edits repo
```

It is:

```text
Parent/Runtime creates checked EditProposal
EditProposal applies to target Artifact
Derived Artifact hydrates Child Runtime
Child self-evaluates
```

## Admissibility

A proposal can be applied or admitted only if the relevant checks pass.

```text
Within(q, g)
before_hashes_match(q, a)
immutable_surface_unchanged(a, a')
derived_artifact_identity_known(a')
```

Additional v1 checks:

- all edited files resolve under the target Artifact root;
- all writes are contained in the writable grant;
- semantic edits resolve through the DB to exactly one node;
- expected file hashes match before write;
- the derived Artifact is recoverable through the backend;
- the admitted History record names the coordinate, grant, proposal, and
  transition.

## History Recording

History should carry or reference enough structure to replay the authority
chain:

```text
Recorded(h, {
  coordinate: c,
  surface_grant: g,
  proposal: q,
  transition: a -> a',
  validation: passed | failed,
  child_evaluation: later
})
```

Recommended History-facing fields:

- generator runtime id;
- target artifact id;
- surface policy id;
- readable and writable grant identifiers or commitments;
- proposal id, request id, and call id;
- edit mode: canonical, splice, ns_patch, create;
- touched files and spans where known;
- before hashes and after hashes;
- derived artifact identity;
- surface validation result;
- later child runtime id and self-evaluation evidence.

Rendered previews, UI approval text, and operator projection files may be
recorded as telemetry, but they are not the authoritative transition.

## Patch Attribution And Scores

For this model:

```text
Patch = ArtifactDelta
```

A patch is the material delta between one Artifact and another:

```text
p : Artifact a -> Artifact a'
ArtifactDelta(a, a') = p
```

The uncertain part is the semantic and behavioral projection:

```text
Patch / ArtifactDelta
  -> CodeGraphDelta
  -> inferred RuntimeDelta
  -> observed child evaluation
```

`ploke-tui` is important because it has the code-graph machinery needed to hold
the middle mapping:

```text
Project(ploke_tui, ArtifactDelta) = CodeGraphDelta
Infer(ploke_tui, CodeGraphDelta) = candidate RuntimeDelta
```

This inference is provisional, but it is still useful. The system should score
descendant success along the whole provenance chain while preserving the context
needed to revise the attribution later.

Observed child success is direct evidence about the child runtime and derived
Artifact:

```text
Score(child_runtime)    = observed self/eval metrics
Score(derived_artifact) = score(child_runtime hydrated from it)
```

It is also provisional evidence about the patch and the generator:

```text
Score(patch) =
  attributed effect of ArtifactDelta(a, a')

Score(parent_runtime_as_generator) =
  credited effect of generated patch over target Artifact under SurfaceGrant
```

The patch score is contextual:

```text
PatchScore(p | base_artifact=a, generator=r, surface=g, eval=e)
```

Later, after repeated observations, we want stronger estimates:

```text
GeneralizedPatchScore(p)
CompositionScore(p_i, p_j)
Interaction(p_i, p_j) = constructive | destructive | neutral | unknown
```

The current implementation may begin with scalar scores and confidence labels,
but the intended model should allow distributions.

The older notes in `framework-ext-01.md`, `framework-ext-02.md`,
`framework-ext-03.md`, and `prototype-1-probability.md` should be read as
conceptual background rather than current implementation authority. The
framework has moved toward History, Crown, Artifact, Runtime, Role, and
Surface-boundary vocabulary, but those notes still capture the relevant
probabilistic-programming idea: mutation-capable procedures and interventions
are stochastic kernels whose realized outputs must be recorded with provenance
and uncertainty.

In that older notation, a mutation-capable procedure is better understood as:

```text
[[x]]_{Gamma_g} : I_x x View(Sigma_g) -> Delta(O_x x PatchSet x U_x)
```

where `Delta(...)` is a distribution over outputs, patches, and uncertainty or
risk information.

For the current edit-surface model, the same idea can be restated as:

```text
K_(executor, create_procedure) :
  InputState x SurfaceView(Artifact)
    -> Delta(EditProposal x PatchSet x Uncertainty)
```

and at the intervention level:

```text
T_intervention :
  Configuration_g x HistoryView_g
    -> Delta(Configuration_(g+1) x RealizedInterventionEvent)
```

The strict layer is still separate:

```text
ApplyValidateCommit :
  Artifact x Patch x SurfaceGrant x ValidationPolicy
    -> Artifact'
```

So the proposal/generation side may be probabilistic, while the commit,
surface-containment, artifact-identity, and History-admission side remains
auditable and fail-closed.

Later attribution should therefore be able to represent distributions over:

- patch effect;
- component effect;
- generator/runtime effectiveness;
- harness/sub-executor effectiveness;
- patch composition and interaction effects.

The realized intervention event should preserve enough state for later
posterior updates:

```text
RealizedInterventionEvent = {
  pre_artifact,
  post_artifact,
  generator_runtime,
  surface_grant,
  proposed_patches,
  selected_patch,
  validation_result,
  evaluation_samples,
  uncertainty_payload,
  provenance,
  decision
}
```

That keeps the door open for later rules such as:

```text
P(Y_candidate > Y_baseline + epsilon | eval data) >= threshold
```

or reputation-weighted validator samples, without requiring this first design
to implement the full probabilistic layer.

So the invariant is not "do not score until certain." It is:

```text
Score provisionally, with provenance and context.
Do not erase the base Artifact, generator Runtime, SurfaceGrant, co-applied
changes, evaluation policy, or uncertainty model needed to update the score.
```

## Executor Roles

Step-local executors should remain distinct:

```text
Parent<Ruling>        grants the surface and owns create authority
Generator Runtime     proposes edits over the granted Artifact surface
TUI/tool harness      resolves/stages proposals and can apply them
Surface checker       validates containment and hash expectations
Artifact backend      realizes derived Artifact identity
History admission     records the admitted transition/evidence
Child Runtime         evaluates the derived Artifact
```

This avoids collapsing the TUI into authority. The TUI is an executor and
projection surface within the procedure graph. It also avoids treating Runtime
as a normal writable substrate: Runtime is the materialized executor carrying
implicit policy and code state; the granted edit boundary is over the target
Artifact.

## Open Design Questions

1. Should `SurfaceGrant` live near `loop_graph`, `prototype1_state::history`,
   or a new edit/create module?
2. Should writable semantic targets be represented as file/span ranges,
   canonical symbols, or both?
3. How should read-only context be modeled when it is broader than writable
   context?
4. Should non-semantic patch mode be admitted for v1, or treated as a fallback
   requiring whole-file write grants?
5. What is the first durable Artifact identity for this path: git tree,
   artifact manifest, current text-file artifact id, or another backend key?
6. Does operator approval belong as telemetry only, or can it become an
   explicit executor step for human-in-the-loop create procedures?

## Implementation Direction

The first implementation slice should not start by adding a TUI filter.

It should start by adding the structural boundary:

```text
SurfaceGrant + Proposal(q, c, g) + Within(q, g)
```

Then adapt the existing TUI `EditProposal` path as one executor that can produce
`q`. After that, the existing Prototype 1 child materialization path can consume
the derived Artifact and continue with child self-evaluation and History-backed
selection.

## Trait Adapter Shape

`ploke-eval` should not import `ploke-tui` internals into active authority
paths. It should define a small adapter surface that can be implemented by a
`ploke-tui` bridge, a mock harness in tests, or a later non-TUI editor.

The code graph should be a separate adapter from the edit harness. The edit
harness may depend on it, but the graph projection is independently useful for
search, attribution, and surface containment checks.

Sketch:

```rust
trait CodeGraphView {
    type Artifact;
    type Projection;
    type Target;
    type SearchQuery;
    type SearchHit;
    type MaterialSpan;
    type Delta;
    type Error;

    fn project(&self, artifact: &Self::Artifact) -> Result<Self::Projection, Self::Error>;

    fn search(
        &self,
        projection: &Self::Projection,
        query: &Self::SearchQuery,
        read_surface: &SurfaceGrant,
    ) -> Result<Vec<Self::SearchHit>, Self::Error>;

    fn resolve(
        &self,
        projection: &Self::Projection,
        target: &Self::Target,
    ) -> Result<Self::MaterialSpan, Self::Error>;

    fn delta(
        &self,
        before: &Self::Projection,
        after: &Self::Projection,
        patch: &ArtifactDelta,
    ) -> Result<Self::Delta, Self::Error>;
}
```

Required semantics:

```text
project(Artifact) returns a graph projection tied to that Artifact.
search(...) is read-only and bounded by the readable surface.
resolve(...) maps semantic targets to material spans with expected hashes.
delta(...) maps Patch / ArtifactDelta to CodeGraphDelta for attribution.
```

The edit harness sits above that:

```rust
trait EditHarness {
    type Graph: CodeGraphView;
    type Proposal;
    type Approved;
    type Applied;
    type HarnessRun;
    type Error;

    fn graph(&self) -> &Self::Graph;

    fn propose(
        &self,
        input: EditInput<'_>,
    ) -> Result<(Self::Proposal, Self::HarnessRun), Self::Error>;

    fn approve(
        &self,
        proposal: Self::Proposal,
        approval: Approval,
    ) -> Result<Self::Approved, Self::Error>;

    fn apply_checked(
        &self,
        approved: Self::Approved,
        check: SurfaceCheck,
    ) -> Result<Self::Applied, Self::Error>;
}
```

Required semantics:

```text
propose(...) may be stochastic and may expose harness/sub-executor provenance.
approve(...) is an explicit executor step when human or policy approval matters.
apply_checked(...) must require a prior ploke-eval surface check; the harness
must not be trusted to self-authorize writes.
```

The `EditInput` passed by `ploke-eval` should carry the authority envelope:

```text
EditInput = {
  role: Parent<Ruling>,
  coordinate,
  target_artifact,
  surface_grant,
  graph_projection,
  procedure_policy,
  seed_or_run_id,
}
```

The `HarnessRun` returned by the adapter should preserve internal provenance
without making it authority:

```text
HarnessRun = {
  harness_id,
  harness_artifact_component,
  sub_executors,
  tool_calls,
  proposal_id,
  touched_surface_reported,
  uncertainty,
  telemetry_refs,
}
```

`ploke-eval` owns the authority checks:

```text
SurfaceCheck = {
  proposal_touches,
  semantic_containment,
  material_containment,
  before_hashes_match,
  result: pass | fail
}
```

and only after a passing `SurfaceCheck` may the candidate proceed to Artifact
application, History admission, child hydration, and later attribution.
