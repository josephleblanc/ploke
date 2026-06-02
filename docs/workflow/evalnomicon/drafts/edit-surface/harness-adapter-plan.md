# Bounded Edit Harness Adapter Plan

Status: working implementation plan.

This note turns the edit-surface model into an implementation brief for
Prototype 1 candidate creation through a harness such as `ploke-tui`.

## Current Implementation Snapshot, 2026-05-13

Commit `bd00056b Add broad harness request fanout` moved the broad path from a
single pending request into a live complete-mode fanout prototype:

- admitted profiles may use `source = "broad-harness-request"`;
- the parent publishes one broad request slot per child budget slot;
- each broad slot carries a singleton request budget, `min = 1, max = 1`;
- complete mode invokes the headless `ploke-tui` adapter for each slot when no
  submitted result already exists;
- `SubmittedBroadHarnessResult` is request-bound and checked against the live
  `EditSurfaceAdmission` before a child transaction is admitted;
- the backend admits the actual candidate workspace diff, so a single broad
  child may contain multiple changed files when all changed paths remain inside
  the broad policy;
- the batch is sealed as a normal child plan only after at least the configured
  minimum number of admitted broad transactions exists.

This is sufficient to run a broad 5-generation, 3-child Prototype 1 campaign
and exercise multi-file child artifacts through `ploke-tui`. It is not the full
long-term adapter boundary described below. In particular:

- the current broad path is an eval-owned headless-TUI continuation path, not
  yet the production `Harness` trait implementation;
- path-policy admission is live, but the richer formal `SurfaceGrant` /
  read-write-touch proof spine is not yet the only broad admission path;
- `tool_seconds` is present in the request contract and prompt, but only
  `max_attempts` and the overall turn timeout are enforced by the adapter;
- the current 3-child campaign is strict: if any required slot fails to produce
  an admitted transaction, that parent fanout fails rather than proceeding with
  fewer children.

The central framing is:

```text
The adapter encodes the bounded edit operation interface.
`ploke-tui` is one harness implementation behind that interface.
```

The goal is not to build a special-purpose `ploke-tui` coupling layer. The goal
is to give `ploke-eval` a stable operation boundary for creating checked
candidate Artifacts while allowing different harnesses to propose and
mechanically apply edits.

Related proof index:

- [`proof-index.md`](proof-index.md)
  Maps implementation tests to the formal surface/edit judgments and records
  formal gaps exposed by the tests.

## Target End State

Prototype 1 should be able to inspect its own recent History, identify a
concrete failure mode, choose a bounded code-graph surface that is causally
upstream of that failure, ask `ploke-tui` or another harness to produce checked
candidate edits inside that surface, run the resulting children, and use
History-backed selection to decide whether any child improved the loop.

In operational terms:

```text
History evidence
  -> Diagnosis
  -> SurfaceChoice
  -> EditObjective
  -> SurfaceGrant over artifact-bound Γ_a
  -> checked candidate Artifacts
  -> child runtime evaluation
  -> History-backed selection
  -> compact operator explanation
```

The target is not merely "the adapter compiles." The target is a loop that can
answer:

- what failed or limited improvement;
- why this surface was selected;
- what the harness was asked to change;
- what the proposed edits read and wrote;
- why each candidate was admitted or rejected;
- whether child evaluation improved the selected metrics;
- why the successor was selected, rejected, or stopped.

## Current Direction: Broad Ruling Parent With Protected Core

The current preferred first pass leans toward the HyperAgents/DGM-H approach:
give the ruling Parent broad room to decide what improvement to attempt, while
mechanically protecting the parts of the system that make the experiment valid
and safe enough to run unattended.

The default surface is therefore not a narrow hand-selected tool file. It is:

```text
EditableSurface(a) = Γ_a \ ProtectedCore(a)
```

where `ProtectedCore` is the part of the artifact-bound code graph that the
Parent may observe but ordinary child-producing edits must not modify.

At minimum, `ProtectedCore` should include:

```text
Crown and History admission/sealing authority
successor spawn / trampoline / process-lifecycle authority
oracle and ground-truth evaluation authority
future consensus/blockchain invariants
dependency/build escape hatches
OS/process/filesystem authority expansion paths
```

Natural-language instructions such as "do not edit Crown, History, successor
spawn, oracle, dependency, or process authority code" are useful context for the
LLM, but they are not the guarantee. The guarantee must come from the checked
surface:

```text
g ⊢ q iff Q_r ⊆ R ∧ Q_w ⊆ W ∧ Q_w ∩ F = ∅
```

where `F` contains the protected core and high-risk capability-expansion
surfaces.

Under this direction, `Diagnosis` is context, not write authority. The Parent
receives progressive disclosure of History/evaluation/protocol/tool evidence
so it can decide what intervention might improve long-horizon outcomes. The
spawn path admits only checked child artifacts whose writes respect the
protected core and whose proposal/apply evidence can be recorded.

The more focused route-table approaches remain useful alternatives. They can be
compared later against the broad-ruling-parent approach on separate branches,
parents, archive paths, or consensus/tree segments.

### Form-Preserving Capability Growth

The broad-parent experiment must not accidentally freeze the child below the
Parent's capability level. That would defeat self-improvement. The admissibility
rule is not:

```text
capabilities(child) ⊆ capabilities(parent)
```

The intended rule is:

```text
the child may gain object-level capabilities,
but admitted transitions must preserve the framework form.
```

Separate the mutable object-level implementation from the protected framework
contract:

```text
Artifact a =
  Φ(a)  framework form / protected contract
  Ω(a)  mutable object-level implementation and capabilities
```

Ordinary self-improvement may change `Ω`:

```text
Ω(a') may add better tools, better summaries, better retrieval, better
protocol analysis, better planning, or other useful capabilities.
```

But it must preserve `Φ`:

```text
Φ(a') = Φ(a)
```

or pass through an explicitly admitted framework-migration path that is outside
ordinary child generation.

`Φ` is the continuity of form that makes a descendant still part of this
framework rather than "some other program that scored well by escaping." It
includes the Crown/History authority model, admitted child-spawn path,
oracle/evaluation boundary, protected runtime lifecycle, artifact/provenance
rules, sandbox/OS authority limits, and later consensus/blockchain invariants.

The useful design region is therefore:

```text
Useful Transformations
  ∩ Statically Checkable Properties
  ∩ Sandbox-Enforceable Properties
  ∩ Form-Preserving Transitions
```

The center is not "safe capability subset." It is:

```text
transformations that improve object-level capability
while preserving the framework form.
```

This framing lets a child become better than the parent without letting it
rewrite what "child", "authority", "evaluation", "History", or "successor"
mean.

### Protected Execution Invariant Across Descendants

The important safety distinction is:

```text
the LLM can edit the child Artifact;
the LLM cannot change the capabilities of its already-running Parent Runtime.
```

Because the Parent is a compiled binary editing a future artifact, it cannot
expand its own runtime capabilities by changing files in the child worktree.
It can only attempt to create a child that would have different capabilities.
Therefore child spawn admission is the critical gate.

For a protected execution subgraph `P_exec`, child spawn should require:

```text
digest_a(P_exec) = digest_child(P_exec)
```

and the parent binary that performs the check should itself have been built
from an artifact with the same protected execution digest. Informally:

```text
if the Parent can only spawn a child through path P_exec,
and spawning requires the child artifact to preserve P_exec,
then every admitted descendant preserves P_exec by induction.
```

This is the desired invariant:

```text
∀ admitted edge a_i -> a_{i+1}.
  digest_{a_i}(P_exec) = digest_{a_{i+1}}(P_exec)
```

So long as all successor creation flows through this admission path, the LLM
can improve child capabilities outside the protected execution subgraph, but it
cannot expand the capabilities that define child-spawn authority, History/Crown
authority, or other protected experimental invariants.

This guarantee is only as strong as the definition of `P_exec`, the material
digest check, and the claim that there is no alternate child-spawn path outside
`P_exec`.

The `P_exec` digest check is one concrete mechanism for preserving part of
`Φ`. It does not prove all of `Φ` by itself. Other parts of the framework form
may require graph checks, static capability scans, sandbox policy, provenance
checks, and explicit History admission rules.

### Guarantees And Non-Guarantees

What the broad protected-core approach can guarantee, if implemented correctly:

- proposed writes do not touch the forbidden file/span/symbol surface;
- child spawn is refused if protected execution digests differ;
- ordinary candidates cannot edit Crown/History admission, successor spawn,
  oracle, consensus, or other protected authority surfaces;
- ordinary candidates cannot add obvious process-spawn or OS-authority paths
  when those paths are included in `F` or a high-risk gate;
- every admitted child can be tied to the parent artifact, runtime, objective,
  disclosed evidence, proposal, checked delta, and evaluation outcome.

What it does not guarantee:

- that the LLM cannot make a harmful change in an allowed surface;
- that coarse keyword/symbol scans catch every semantic way to gain OS
  authority;
- that natural-language constraints are followed;
- that external runtime configuration files are safe unless their identities,
  digests, and admission rules are part of the protected model;
- that dynamic behavior through existing allowed code paths is harmless;
- that the selected child is actually better except as measured by the
  evaluation/oracle.

External permission/config files are especially risky. If a runtime reads a
mutable file to decide its own authority, that file becomes part of the
capability surface. It must either be immutable, digested and admitted as part
of `P_exec`, or excluded from runtime authority decisions. Otherwise the LLM
may be unable to edit its current binary but still influence a future runtime by
modifying the file that future runtime reads.

### Progressive Disclosure And Observation Trace

Broad authority should not mean dropping the Parent into an opaque codebase. The
Parent should receive progressive disclosure:

```text
summary -> indexed evidence -> selected records -> exact snippets/spans
```

The system should record what was disclosed:

```text
ObservationTrace {
  graph_snapshot_id,
  history/evaluation/protocol summaries disclosed,
  tool queries issued,
  nodes/spans/snippets disclosed,
  tool outputs disclosed,
  context packet digests,
}
```

This does not prove what the model "understood" or psychologically considered.
It does prove which records, snippets, and tool outputs were made available to
the proposal-generating conversation. That trace is necessary for later credit
assignment and for comparing broad free-roaming children against more focused
route-table children.

## Placeholder Milestones

These milestones are placeholders for implementation planning. Fill them in as
the design becomes executable, but keep the stage boundaries visible so setup
work stays connected to the end-to-end loop.

## Stable Core Vocabulary

Vertical slices must implement against a small stable vocabulary. They should
not invent slice-local nouns that later harden into the architecture.

The current core carriers are:

```text
Diagnosis
SurfaceChoice
EditObjective
SurfaceGrant
ProjectionIdentity
ResolvedTouches
SurfaceCheck
CheckedProposal
ArtifactDelta
surface_attempt::Evidence
```

These names are placeholders for the semantic objects, not a command to add
exactly these Rust structs immediately. Existing code may already provide some
of them under nearby names. When it does, prefer tightening that existing
carrier over adding a sibling type with a longer name.

Each carrier has a limited job:

- `Diagnosis`: what failed or limited improvement, with evidence refs.
- `SurfaceChoice`: which surface family and construction strategy follows from
  the diagnosis.
- `EditObjective`: the concrete repair task handed to the harness.
- `SurfaceGrant`: the read/write/forbidden authority over an artifact-bound
  graph surface.
- `ProjectionIdentity`: the binding between a graph projection `Γ_a` and the
  target Artifact.
- `ResolvedTouches`: the proposal's read/write touch sets `(Q_r, Q_w)`.
- `SurfaceCheck`: the result of containment and material validity checks.
- `CheckedProposal`: the proof-like authorization that unlocks checked apply.
- `ArtifactDelta`: the material transition from one Artifact to another.
- `surface_attempt::Evidence`: durable evidence for successful or rejected
  edit-surface attempts, under a namespace that keeps `Surface`, `Attempt`,
  and `Outcome` structurally related instead of flattened into one noun.

## Slice Discipline

The implementation should progress through vertical slices, but the slices must
not create their own vocabulary.

Each slice may do one or more of these:

- define a missing core carrier;
- strengthen an existing carrier so it matches the core semantics;
- add one variant or field justified by the formal model;
- connect two existing carriers;
- add a splice test across existing carriers;
- delete or merge a provisional duplicate.

Each slice must avoid:

- `SliceN`-specific types;
- objective-specific type families before the common carrier fails;
- one-off `Report`, `View`, `Info`, `Status`, or `Output` carriers that are not
  projections of an already-named object;
- encoding phase, subsystem, role, and mechanism into one long identifier;
- making CLI output, logs, TUI state, or mutable reports into source truth.

For example, do not let `EditObjective` split into a family like:

```text
EditObjectiveTool
EditObjectiveToolDefinition
PathEditObjectiveToolDefinition
ExecutionPathEditObjectiveToolDefinition
```

unless there is a real algebraic distinction that cannot be represented inside
the stable carrier.

Prefer one structural carrier with controlled dimensions:

```text
EditObjective {
  diagnosis_ref,
  surface_choice_ref,
  target_metric,
  evidence_refs,
  writable_intent,
  read_context,
  constraints,
  success_criteria,
  candidate_count,
}
```

Later specialization should normally happen through fields or variants such as:

```text
WritableIntent::SemanticNodes(...)
WritableIntent::Files(...)
WritableIntent::ToolDescriptions(...)
```

not through new top-level objective types.

Apply the same rule to `SurfaceChoice`:

```text
SurfaceChoice {
  family,
  anchor_policy,
  relation_basis,
  grant_constructor,
  evidence_refs,
}
```

The family can be an enum or registry key. It should not become a new type per
slice.

## Naming Pressure Check

Every implementation slice must answer this before the patch is accepted:

```text
Did this patch add a local noun that should instead be:
  - a field on a stable carrier,
  - a variant of an existing enum,
  - a method inside a narrower module,
  - a projection over an existing semantic object,
  - or a deletion/merge of a duplicate?
```

If a new name needs to mention subsystem, phase, role, mechanism, and local
context to be understandable, the slice should stop and find the missing
structure before continuing.

## Execution Slice Queue

The milestones below are broad stages. The actual implementation should be a
queue of vertical slices, each with:

```text
goal
core carriers touched
files expected to change
upstream fixture or replay A'
new implementation B*
downstream consumer contract C'
tests
verification command
done condition
naming pressure check
```

Early slices should be concrete. Later slices may remain placeholders until the
earlier splice tests reveal the exact record shapes.

### Milestone 1: Evidence Visibility

Goal:

```text
edit-surface attempts, successes, and failures are visible as typed evidence
that a later Parent can read from History.
```

The important distinction is that failed candidate-generation attempts must not
disappear as local harness state, logs, or absence of children. They need a
durable typed projection such as `SurfaceEvidence` or
`surface_attempt::Evidence`.

Done when:

- successful checked/applied attempts are recorded with grant, proposal, check,
  apply, and derived Artifact evidence;
- rejected attempts record the failure kind and evidence refs;
- parent-time History queries can see both success and failure evidence.

### Milestone 2: Mechanistic Diagnosis

Goal:

```text
History evidence -> Diagnosis
```

The first useful route should classify invalid candidate generation caused by
semantic edit resolution failures. Low score alone is not enough; the diagnosis
must cite typed evidence.

Done when:

- a small classifier maps recorded edit-surface failure kinds to
  `Diagnosis`;
- unknown or unsupported evidence returns `unknown` instead of inventing a
  cause;
- diagnosis records preserve evidence refs and confidence.

### Milestone 3: Bounded Surface Choice

Goal:

```text
Diagnosis -> SurfaceChoice -> SurfaceGrant(R, W, F)
```

The first surface choice should target the `ploke-tui` semantic edit resolver
path. Current-branch construction should use exact anchors, same-file/module
relations, explicit companions, and read-only search hits. Call graph and
type-reference expansion remain future work.

#### Considered Heuristic: Failure Mechanism To Surface Class

One possible route policy is to classify a tool failure by the likely failure
mechanism before choosing a surface. This is attractive because "tool failed"
does not imply "edit the tool directory." The same externally visible failure
can have different repair surfaces:

```text
model calls tool incorrectly
  -> tool schema / parameter shape / error message / description
  -> likely ploke-tui tool definition or tool metadata surface

model calls tool successfully but uses it poorly with other tools
  -> tool guidance / examples / tool_text / protocol scaffolding
  -> likely prompt/tool-text/harness-guidance surface

tool output is too large
  -> output shaping / truncation / summarization / retrieval limits
  -> likely tool implementation or RAG query surface

retrieval result is inaccurate
  -> query construction / ranking / embedding / database retrieval
  -> likely ploke-tui RAG, ploke-db, or ploke-embed surface

parser cannot see the requested code item
  -> parse / IR / transform / database ingestion / re-embedding pipeline
  -> not normally solvable inside the ploke-tui tools directory

tool description is stale or misleading
  -> tool description / schema / tool_text
  -> likely bounded tool metadata or prompt surface

parameter shape is poorly chosen
  -> schema/API boundary
  -> likely ploke-tui tool definition surface
```

This heuristic is under consideration, not yet the selected routing policy. If
implemented, it should preserve an explicit intermediate distinction:

```text
Diagnosis
  -> failure mechanism hypothesis
  -> repairability class
  -> SurfaceChoice
  -> EditObjective
```

The route must also be allowed to return:

```text
insufficient_surface_evidence
```

when typed diagnosis evidence shows that something failed, but the available
records do not justify a bounded edit surface. This preserves the distinction
between "we know a failure occurred" and "we know where a code edit is likely
causally upstream."

#### Considered Framing: Surface Choice As Experiment Design

Another possible framing is that parent-time routing is not primarily root
cause diagnosis. It is bounded experiment design. The parent does not need to
prove why a tool episode failed before acting. It needs to choose the next
intervention that is cheap enough, safe enough, informative enough, and likely
enough to improve either child quality or future credit assignment.

Under this framing, the route is not:

```text
failure -> true cause -> edit that cause
```

It is:

```text
History evidence
  -> bounded intervention objective
  -> surface construction plan
  -> one or more candidate children
  -> observed outcome
  -> later policy/credit update
```

The intervention can target different immediate effects:

```text
repair capability
  e.g. improve request_code_context output shaping or argument validation

improve affordance
  e.g. clarify tool description, schema, examples, or error messages

improve observability
  e.g. preserve tool episode ids, output-size summaries, parser coverage
       misses, or protocol-record links

compare alternatives
  e.g. spawn bounded children against multiple plausible surfaces and let
       evaluation/selection judge the result

decline to edit
  e.g. record insufficient evidence and choose a read-only probe or no-op
       when write authority would be speculative
```

This suggests a possible intermediate carrier:

```text
Diagnosis
  -> InterventionObjective
  -> SurfaceConstructionPlan
  -> SurfaceChoice | ProbeChoice | InstrumentationChoice | NoAction
```

This is also only a considered solution. Its value is that it fits the
multi-child loop: a child can be useful because it improves score, because it
improves future visibility, or because it lets selection compare bounded
interventions empirically. Its risk is that "experiment" can become too broad
unless grants and objective records remain tight.

#### Considered Alternatives From The HyperAgents Frame

The HyperAgents/DGM-H pattern suggests not over-specifying the self-improvement
route too early. It keeps an archive of evaluated variants, branches from
selected parents, and lets useful process changes emerge as stepping stones.
In that frame, parent-time routing may be better treated as constrained
generation of useful variants rather than as a one-step causal proof.

Possible solution families:

```text
Intervention portfolio
  Same evidence produces several bounded children against different plausible
  intervention types. Selection and later History decide which one was useful.

Open-ended bounded mutation operators
  Define broad operators such as ImproveAffordance, ImproveObservability,
  ReduceCost, ImproveReliability, ImproveEvaluationAnalysis, or
  ImproveGeneratorStrategy. Each operator constructs a bounded surface for the
  current artifact instead of hard-coding one failure-to-file route.

Probe or instrumentation children
  Treat visibility improvements as first-class children. A child may add tool
  episode records, preserve failed args/output/error ids, summarize large
  outputs, record parser coverage misses, or link protocol artifacts to
  History evidence, even if it does not immediately raise benchmark score.

Archive-level credit assignment
  Record intervention type, surface family, objective, and descendant outcomes
  so later selection or analysis can learn which interventions tend to create
  useful descendants. This avoids pretending that one local failure explains
  one local surface.

Budget-aware strategy
  Vary intervention selection by remaining run budget: earlier generations can
  explore/instrument/process-improve, middle generations can compare competing
  hypotheses, and late generations can prefer conservative high-confidence
  fixes.
```

These alternatives are qualitatively different from a static route table. They
preserve room for stepping-stone variants and meta-level improvements while
still allowing `ploke-eval` to enforce bounded grants and provenance. Their
main risk is loss of focus: without tight objective records, evaluation hooks,
and surface grants, "open-ended" can become broad, expensive, and hard to
credit.

Done when:

- the semantic-resolution diagnosis maps to a named surface family;
- the grant records readable `R`, writable `W`, and forbidden `F` subsets or
  their honest current equivalent;
- policy-bearing `crates/ploke-eval` remains forbidden for ordinary candidate
  edits.

### Milestone 4: Concrete Edit Objective

Goal:

```text
SurfaceChoice -> EditObjective
```

The harness should receive a specific repair objective, not a vague "improve
the score" prompt.

Done when:

- the objective names the diagnosis, evidence refs, target metric, read
  context, writable intent, constraints, success criteria, and requested
  candidate count;
- the objective is recorded with proposal/candidate evidence;
- later History can compare which objectives and surfaces produced useful
  children.

### Milestone 5: Checked Candidate Generation

Goal:

```text
EditObjective + SurfaceGrant + harness -> CheckedProposal -> ArtifactDelta
```

`ploke-tui` may propose, resolve, stage, preview, and mechanically apply. It
must not mint authority. `ploke-eval` checks projection identity, containment,
forbidden writes, and material hashes before producing the checked
authorization.

Done when:

- proposals resolve to read/write touches `(Q_r, Q_w)`;
- stale graph/index, out-of-surface writes, forbidden writes, stale hashes, and
  partial apply fail before child build;
- `CheckedProposal` or its equivalent is the only normal checked-apply path.

### Milestone 6: Child Evaluation And Selection

Goal:

```text
derived Artifact -> child runtime evaluation -> History-backed selection
```

The created candidates must enter the same child evaluation and successor
selection path as other candidates, with surface evidence attached.

Done when:

- checked ArtifactDeltas hydrate child runtimes;
- children self-evaluate from rebuilt binaries;
- selection can see candidate score evidence and surface evidence;
- selection can compare current children with admissible previous History
  candidates where the policy allows it.

### Milestone 7: Operator Inspection

Goal:

```text
compact projection over the same typed evidence
```

The operator should be able to answer "what is happening and why?" without
reading raw logs, huge JSON blobs, or TUI session internals.

Done when a compact projection can show:

- diagnosis;
- selected surface and objective;
- candidate count and child outcomes;
- rejected edit-surface attempts with failure kinds;
- selected/rejected successor reason;
- paths or evidence refs for deeper inspection.

### Out Of Scope For This Slice

Do not make these load-bearing in the first implementation:

- ordinary mutation of `crates/ploke-eval`;
- call-graph or type-reference-based admission checks;
- cross-ruler consensus or global finality;
- stochastic reputation or full posterior scoring;
- partial apply as a success mode;
- treating TUI proposal state or CLI projection output as authority.

## Validation Strategy

The implementation should use tight validation loops before relying on a long
live run. Passing isolated unit tests is not enough for this system because the
most common failures happen at stage boundaries: a producer writes a record the
consumer does not read, a consumer falls back to a mutable projection, or a live
path reads a surface the plan never named.

Use four validation speeds.

### 1. Unit Loop

Fast deterministic tests for the authority algebra:

```text
Grant accepts Q_w ⊆ W
Grant rejects Q_w ∩ F
stale ProjectionIdentity is rejected
hash mismatch is rejected
CheckedProposal is required before apply
```

These should run without provider calls, child runtimes, or live `ploke-tui`
actors.

### 2. Harness Fixture Loop

Use a mock or deterministic harness over a tiny fixture Artifact:

```text
History evidence -> Diagnosis
Diagnosis -> SurfaceChoice
SurfaceChoice -> EditObjective
EditObjective + mock harness -> checked ArtifactDelta
rejected semantic resolution -> durable attempt evidence
```

This validates the operation shape while keeping runtime cost low.

### 3. Trace-Contract Loop

For each milestone, run the smallest end-to-end path and assert that the record
written by one step is exactly the record read by the next step:

```text
producer writes X
consumer reads X
consumer rejects when X is missing, stale, or mismatched
consumer does not recover authority from projection/log/session state
```

Risky boundaries that need trace-contract coverage:

```text
edit_surface attempt -> History evidence
History evidence -> Diagnosis
Diagnosis -> SurfaceChoice
SurfaceChoice -> SurfaceGrant
SurfaceGrant -> harness proposal
proposal/check/apply -> CandidateArtifact
CandidateArtifact -> child plan
child result -> selection input
selection decision -> successor handoff
```

Include negative probes:

```text
delete or omit surface evidence -> diagnosis cannot claim semantic_resolution
tamper candidate surface digest -> projection or selection rejects it
write only TUI local proposal state -> parent cannot see it
write old projection identity -> checked apply rejects it
write mutable report but no History evidence -> live path ignores it
```

This is the guard against "the unit pieces passed, but the live run used a
different record path."

### 4. Pipeline Splice Loop

Every milestone should include at least one splice test:

```text
recorded/mock upstream output A'
  -> new implementation B*
  -> downstream consumer contract C'
```

The point is to validate the new stage by how it transforms realistic upstream
records into exactly the shape expected by the next real consumer.

Examples:

```text
History fixture A'
  -> Diagnosis policy B*
  -> SurfaceChoice consumer C'

Diagnosis fixture A'
  -> SurfaceChoice policy B*
  -> SurfaceGrant consumer C'

SurfaceGrant + EditObjective fixture A'
  -> harness proposal/check B*
  -> CandidateArtifact consumer C'

CandidateArtifact fixture A'
  -> child plan/materialization B*
  -> child runtime/eval consumer C'
```

`A'` should be replay-shaped whenever possible: real records from an old run,
or minimal typed fixtures copied from real records. `C'` should be the actual
downstream parser, validator, or projection function, or a strict contract
equivalent.

The splice loop catches errors such as:

- `B*` writes a field the downstream consumer never reads;
- `B*` writes the right meaning under the wrong record variant;
- `B*` uses a path, digest, or identity shape downstream rejects;
- `B*` works only with hand-built unit data, not replay-shaped data;
- `B*` silently depends on mutable state outside the stated boundary.

### 4.1. Transition Proof Planning

Do not scope a slice as "test the route" when the route contains several
semantic transitions. Split the route into named transition proofs, and mark a
slice complete only when the transitions it claims to cover have tests at the
right boundary.

For the first broad protected-core route, the route is:

```text
History/context evidence
  -> EditObjective
  -> EditableSurface = Γ_a \ ProtectedCore
  -> checked proposal
  -> candidate Artifact evidence
  -> child evaluation
  -> History-backed selection
```

The transition proof map is:

```text
T1 EvidenceAdmitted:
  A': replay-shaped History/context evidence
  B*: parent evidence/context admission
  C': admitted context refs usable by EditObjective construction

T2 ObjectiveConstructed:
  A': admitted context refs + broad ruling policy
  B*: EditObjective constructor/route
  C': EditObjective records intent, evidence refs, constraints, and success
      criteria without turning Diagnosis into write authority

T3 SurfaceBounded:
  A': EditObjective + graph projection Γ_a + ProtectedCore policy F
  B*: EditableSurface::broad / Grant construction
  C': Grant has W = Γ_a \ F and preserves F for admission checks

T4 ProposalChecked:
  A': SurfaceGrant + harness proposal touches Q_r/Q_w
  B*: SurfaceCheck / checked proposal admission
  C': ordinary writes are accepted, protected-core writes are rejected, and the
      harness cannot apply without checked authority

T5 CandidateProduced:
  A': checked proposal + target Artifact
  B*: checked apply / ArtifactDelta construction
  C': candidate Artifact evidence is accepted by child-plan/materialization

T6 OutcomeSelectable:
  A': evaluated child candidate with surface/proposal/candidate evidence
  B*: parent selection evidence fold
  C': History-backed selection can compare the candidate and preserve
      provenance into successor handoff
```

Current status after 7.3:

```text
T2: covered at the local primitive boundary using synthetic refs.
T3: covered at the local primitive boundary using explicit Γ_a and explicit F.
T4: covered at the local primitive boundary for Grant::check on hand-built Draft
    touches.

T1 is not covered.
T2/T3 are not covered from replay-shaped parent inputs.
T5/T6 are later slices.
```

This means the current local broad-surface tests are not wrong; they are just
not enough to close the full route. They prove local admission behavior after
the parent-side inputs already exist. The next slice, 7.4, owns the upstream
splice:

```text
replay-shaped parent evidence + graph projection + protected-core policy
  -> EditObjective + EditableSurface
```

### 5. Mini Runtime Loop

Only after the lower loops pass, run a tiny live Prototype 1 path:

```text
1 parent
1-2 candidate children
semantic-resolution route enabled
children self-evaluate
History-backed selection sees surface evidence
compact projection explains the run
```

The mini runtime loop should verify the trampoline behavior, not discover the
basic record-contract errors that cheaper loops should catch first.

### First Splice Test For Semantic Resolution

The first route should start with this positive splice:

```text
A': replay-shaped edit-surface failure evidence
B*: parent diagnosis classifier
C': SurfaceChoice/EditObjective contract
```

Assertions:

```text
Diagnosis.limiter = invalid_candidate_generation
Diagnosis.failure_kind = semantic_edit_resolution
SurfaceChoice.surface_family = ploke-tui semantic edit resolver
EditObjective carries evidence refs, target metric, constraints, and success
criteria
```

And this negative splice:

```text
A': same failure exists only as TUI-local/projection/log-shaped data
B*: parent diagnosis classifier
C': SurfaceChoice/EditObjective contract
```

Assertion:

```text
no semantic_resolution diagnosis is produced
```

That proves the parent is reading the intended authority/evidence path, not
ambient UI or projection state.

## Current Blocker Status

The long-term surface-selection plan is graph-slice based:

```text
History diagnosis
  -> failing procedure/tool
  -> graph anchor nodes
  -> relation-expanded read/write subgraphs
  -> bounded SurfaceGrant
  -> EditObjective for the multi-turn harness
```

For the current branch, the available graph relations are more limited than the
long-term shape. The first implementation must be honest about this instead of
pretending a full causal call graph exists.

Available or usable now:

- exact semantic item lookup by file/module/canonical item/node kind;
- parsed Rust item granularity: functions, methods, impls, traits, modules,
  files, and related primary/associated nodes where the parser/index exposes
  them;
- file path, module path, canonical path, namespace, node kind, byte span, and
  tracking/file hash evidence;
- containment-style structure such as same file, same module, containing impl
  or trait, and explicit companion nodes;
- BM25/RAG/text search as weak read-context discovery;
- manually seeded anchor lists for known tools/procedures such as
  `apply_code_edit`.

Not available on this branch:

- call graph relations such as `callers(anchor)` and `callees(anchor)`;
- type-reference graph expansion, though most of this work exists on another
  branch and should be integrated later.

Blocked by call graph:

- mechanistic expansion from a failing function to its direct callees/callers;
- distinguishing local helper callees from unrelated same-file functions;
- routing failures through dynamic tool execution paths without manual anchor
  lists;
- using call-neighborhood evidence as a strong causal reason for writable
  inclusion.

Blocked by type-reference graph:

- mechanistic expansion from a failing function/method to the concrete types,
  traits, enum variants, DTOs, and error types it uses;
- selecting writable type/trait/helper definitions because they are referenced
  by the failing path;
- separating "read this type for context" from "edit this type because it is
  part of the failure" using graph evidence rather than text search.

Available first-slice surface expansion:

```text
A_f = exact anchor nodes from known failure kind / tool name

R_f =
  A_f
  ∪ same_file(A_f)
  ∪ same_module(A_f)
  ∪ containing_impl_or_trait(A_f)
  ∪ explicit_read_companions(f)
  ∪ text_search_hits(anchor names, read_only)

W_f =
  A_f
  ∪ explicit_write_companions(f)
  ∪ selected tests

F_f =
  policy-bearing ploke-eval nodes
  ∪ unrelated crates
  ∪ lower-level substrates unless explicitly selected
```

Future expansion after call/type-reference graphs land:

```text
R_f += callees(A_f, depth = 1)
R_f += callers(A_f, depth = 1)
R_f += type_ref_neighbors(A_f)

W_f += selected local callees(A_f)
W_f += selected referenced helper/type nodes
```

Those future relation kinds should remain in the plan, but v1 tests and
admission rules should only rely on relation kinds present on this branch.

## Surface Request

Use the existing `ploke-tui` semantic edit machinery to generate candidate
patches for Prototype 1, while keeping candidate admission, approval,
ArtifactDelta evidence, and History authority in `ploke-eval`.

## Causal Chain

```text
Parent<Ruling>
  -> read History evidence
  -> diagnose limiting failure
  -> choose bounded surface
  -> construct EditObjective
  -> SurfaceGrant over target Artifact
  -> harness proposal
  -> SurfaceCheck by ploke-eval
  -> checked apply by harness/backend
  -> ArtifactDelta / derived Artifact evidence
  -> child Runtime self-evaluation
  -> History-backed selection
  -> successor handoff
```

The child does not edit the repository. The parent creates candidate Artifacts.
The child hydrates from one candidate Artifact and evaluates itself.

## Concern

Without an adapter boundary, `ploke-eval` will either:

- import `ploke-tui` session internals as if they were authority; or
- fall back to raw file/path patching that loses semantic target evidence.

The adapter keeps the loop organized around the operation we need:

```text
propose bounded edit -> expose touches -> check surface -> apply checked edit
```

But the adapter is only the actuator. The self-improvement loop also needs the
parent-side transformation:

```text
History -> Diagnosis -> SurfaceChoice -> EditObjective -> SurfaceGrant
```

That transformation is where the loop decides what kind of work is worth
trying. A perfectly checked edit can still be useless if the surface was chosen
randomly or the harness was given a vague objective.

## Evidence Surface

A candidate produced through this path must leave enough structured evidence to
diagnose, replay, score, and select it later:

- generator runtime id;
- target artifact id or tree identity;
- coordinate / operation id;
- surface policy id or digest;
- read and write grant identifiers or digests;
- graph or index identity used for resolution;
- proposal id and harness-run id;
- canonical targets, node types, files, spans, and expected hashes;
- preview or preview digest;
- `SurfaceCheck` result;
- apply result with before/after hashes;
- `ArtifactDelta` or derived artifact identity;
- later child runtime id and evaluation evidence.

Rendered previews, TUI proposal status, chat events, and user-facing command
output may be recorded as telemetry, but they are not the authority source.

### Observation And Admission

Do not treat logging, monitoring, or UI state as special material outside the
model. In a self-modifying evaluator, the loop may eventually edit its own
observers and projection code, so the model must distinguish record existence
from decision authority.

Use this authority chain:

```text
Event
  -> Observation
  -> Record
  -> AdmissibleEvidence<D>
  -> DecisionInput<D>
```

where `D` is a decision domain such as:

```text
Diagnosis
Selection
SurfaceCheck
HistoryAdmission
OperatorProjection
```

A record can exist without being admissible for a decision:

```text
record(rec) does not imply admissible_D(rec)
```

and:

```text
projection(rec) ∨ log(rec) ∨ ui_state(rec)
  does not imply admissible_D(rec)
```

unless an explicit admission rule for `D` says otherwise.

For the first route, the admissible diagnosis input is the typed
`surface_attempt::Evidence` carried by `EvaluationPayload.surface_attempt`.
Projection text, raw logs, monitor views, TUI-local proposal state, and absence
of a child Artifact are records or symptoms, but they are not diagnosis inputs
unless admitted through the typed evidence path.

### Generator Surface Provenance

Some edited surfaces are part of the machinery that generates future children.
When those surfaces change, later generation records must cite which version of
that machinery they actually used.

Track surface roles explicitly:

```text
SurfaceRole =
    ObjectSurface
  | GeneratorSurface
  | ObserverSurface
  | PolicySurface
```

Meaning:

- `ObjectSurface`: code being improved or evaluated.
- `GeneratorSurface`: tools, prompts, configs, or code used to create future
  candidate proposals.
- `ObserverSurface`: tools, prompts, configs, or code used to observe, score,
  record, diagnose, or project state.
- `PolicySurface`: code or configs used to admit, authorize, select, or change
  rules.

A graph node or file can have multiple roles. For example, `ploke-tui`
semantic edit machinery is a `GeneratorSurface`; parts of it may also be an
`ObserverSurface`. `ploke-eval` admission/selection code is a `PolicySurface`.

Generator-artifact coupling invariant:

```text
GeneratedProposal(q) =>
  ∃ parent_runtime, parent_artifact, generator_surface_version.
    hydrated_from(parent_runtime, parent_artifact)
    ∧ used_generator_surface(q, generator_surface_version)
    ∧ generator_surface_version ∈ parent_artifact
```

If an `ArtifactDelta` modifies a `GeneratorSurface`, later proposal records
must cite the exact modified generator-surface version from the parent Runtime
Artifact that performed the generation. This lets later analysis trace paths
such as:

```text
δ_tool
  -> modified GeneratorSurface T'
  -> T' used by later parent Runtime
  -> Proposal q_next
  -> ArtifactDelta δ_next
  -> descendant evaluation score
```

The current implementation path only needs to preserve this provenance. Full
descendant-fitness analysis of generator-surface changes is deferred and tracked
in the task stack as `bounded-edit-surface-generator-fitness-analysis`.

Future-only Rust scaffolding for this section must not be silent dead code. If
a type, enum variant, or helper exists only to preserve this planned provenance
shape before it is used, add a nearby comment citing the task-stack id and use:

```rust
#[allow(dead_code, reason = "task-stack:bounded-edit-surface-generator-fitness-analysis preserves planned generator-fitness analysis")]
```

Do this only for code whose future use is explicitly tracked in the task stack;
otherwise delete or defer the code.

## Existing Algebra

The current conceptual carriers are:

- `Artifact`: checkout/tree material that can hydrate a runtime.
- `Runtime`: process hydrated from an Artifact.
- `Parent<Ruling>`: the lineage authority operator for candidate creation.
- `SurfaceGrant`: operational grant describing readable/writable surfaces.
- `SurfaceCommitment`: admission-side commitment about immutable, mutated, and
  ambient surfaces.
- `EditProposal`: staged edit candidate, possibly backed by `ploke-tui`.
- `SurfaceCheck`: `ploke-eval` validation that proposal touches are within the
  grant and hashes/identity match.
- `ArtifactDelta`: material patch from one Artifact to another.
- `History`: durable candidate/evaluation/selection evidence substrate.
- `AdmissibleEvidence<D>`: a record admitted for a specific decision domain,
  not merely something that was observed or logged.

This workstream also needs parent-side planning carriers:

- `Diagnosis`: compact explanation of what limited improvement or blocked
  candidate creation, derived from History evidence.
- `SurfaceChoice`: selected surface family plus reason, confidence, and evidence
  references.
- `EditObjective`: concrete repair task handed to the harness, including target
  metric, constraints, read context, writable intent, success criteria, and
  requested candidate count.

## Missing Structure

The missing implementation object is the trait adapter that preserves this
operation interface without exposing TUI internals to the loop:

```text
CodeGraphView: project/search/resolve/delta over an Artifact-bound graph view.
EditHarness: propose/touches/apply_checked over a SurfaceGrant and proposal.
```

The graph side and edit side should stay separate. `ploke-tui` may implement
both, but future harnesses may only need one side.

## Formal Surface Algebra

The bounded edit surface should be implemented as operations over an
artifact-bound code graph, not as a path allowlist.

Let:

```text
Γ_a = (V_a, E_a, μ_a)
```

where:

```text
a      = target Artifact
Γ_a    = code graph projection bound to Artifact a
V_a    = semantic code nodes in the projection
E_a    = semantic/code relations in the projection
μ_a    = partial materialization map from graph nodes to Artifact spans
Span_a = material byte spans in Artifact a
```

The materialization map is partial:

```text
μ_a : V_a ⇀ Span_a
```

Only nodes in `dom(μ_a)` can be directly edited through the semantic edit path.
Other nodes may be readable context, graph structure, or attribution evidence,
but they cannot be written unless another write mode explicitly admits them.

The graph projection must be artifact-bound. A stale DB/index is not a valid
`Γ_a`; it is a projection failure. Implementations may represent this as an
artifact id, tree hash, source root digest, index identity, manifest path, or a
stronger binding proof, but the adapter boundary must make the binding visible
enough for `ploke-eval` to reject stale projections before apply.

### Grants As Subsets

A grant over `Γ_a` is:

```text
g = (R, W, F)
```

with:

```text
R, W, F ⊆ V_a
W ⊆ R
W ∩ F = ∅
W ⊆ dom(μ_a)
```

Meaning:

- `R`: readable/context subgraph;
- `W`: writable subgraph;
- `F`: forbidden subgraph;
- `W ⊆ dom(μ_a)`: every writable node has a concrete material span.

Path globs, canonical prefixes, node kinds, crate names, relation closures, and
manual allowlists are ways to construct `R`, `W`, and `F`. They are not the
grant itself.

For example, the named surface `ploke-tui-tools` should lower to something like:

```text
S_tools(Γ_a) =
  { v ∈ V_a |
      relpath(v) starts_with "crates/ploke-tui/src/tools/"
      ∧ kind(v) ∈ {function, method}
  }

R = context_closure(S_tools(Γ_a))
W = S_tools(Γ_a)
F = { v ∈ V_a | relpath(v) starts_with "crates/ploke-eval/" }
```

The implementation does not need to compute a sophisticated context closure in
the first slice. It may start with `R = W` plus a small explicit read-only
context set. The important point is that the grant records which construction
was used and checks proposals against the resulting sets.

### Proposal Resolution

A proposal `q` resolves against the artifact-bound graph:

```text
ρ_a(q) = (Q_r, Q_w)
```

where:

```text
Q_r, Q_w ⊆ V_a
Q_w ⊆ dom(μ_a)
```

Meaning:

- `Q_r`: graph nodes read or used as context/evidence by the proposal;
- `Q_w`: graph nodes the proposal intends to modify.

For v1, semantic writes should resolve exactly. Ambiguous, relaxed, or fallback
resolution should be represented explicitly and rejected unless the grant policy
admits it.

### Containment Judgment

Containment is the judgment:

```text
g ⊢ q
```

defined by:

```text
g ⊢ q  iff  Q_r ⊆ R ∧ Q_w ⊆ W ∧ Q_w ∩ F = ∅
```

This should become the meaning of `SurfaceCheck` containment. It is not enough
to check that a path string starts with an allowed prefix.

### Material Validity

Material validity is separate from graph containment:

```text
valid_a(q) iff ∀v ∈ Q_w.
  hash_a(file(μ_a(v))) = expected_hash_q(v)
```

The exact hash granularity may be file-level in the first slice. Span-level or
node-level hashes can be added later. The key requirement is that the expected
hash is checked against Artifact `a`, not against ambient workspace state.

### Checked Proposal

Only containment plus material validity should produce an apply authorization:

```text
ρ_a(q) = (Q_r, Q_w)
g ⊢ q
valid_a(q)
-------------------------
CheckedProposal(a, Γ_a, g, q, Q_r, Q_w)
```

`CheckedProposal` is the proof-like value that unlocks `apply_checked`. It is
created by `ploke-eval`, not by the harness. The harness may report candidate
touches and perform local preflight checks, but it must not mint the checked
authorization.

### Application

Application is partial:

```text
apply_a(q) is defined iff CheckedProposal(a, Γ_a, g, q, Q_r, Q_w) exists
```

When defined:

```text
apply_a(q) = (a', δ)
δ : a → a'
```

The first implementation may delegate the mechanical write to `ploke-tui` /
`ploke-io`, but `ploke-eval` must verify or receive enough evidence to name the
derived Artifact and `ArtifactDelta`.

### History Admission

History admission should append the authority chain:

```text
H' = H ⋅ (a, Γ_a, g, q, ρ_a(q), CheckedProposal, δ, a')
```

The current implementation may store this inside existing candidate payloads or
History evidence records, but the data must not exist only as a mutable report,
TUI proposal state, or CLI rendering.

Rejected attempts are not successful Artifact transitions, but they are still
surface-attempt events that can be admitted for diagnosis:

```text
AttemptOutcome_a(q) =
    Applied(a', δ)
  | Rejected(reason)

H' = H ⋅ Attempt(a, Γ_a, g, q, ρ_a(q), AttemptOutcome_a(q))
```

The success-only admission above is the `Applied(a', δ)` case. The rejected
case produces no `a'` and no `δ`, but it may still produce
`AdmissibleEvidence<Diagnosis>` when the record came through the authorized
evaluation/payload path.

### Current Code Mapping

Recent code inspection found that `ploke-eval` already has much of the skeleton:

- `graph::View`: `project`, `bounds`, `search`, `resolve`, `delta`;
- `harness::Harness`: `graph`, `propose`, `apply_checked`;
- `graph::Projection`, `graph::Span`, `graph::Delta`;
- `surface::Grant`, `surface::Touch`, `surface::Check`;
- `harness::ArtifactDelta`;
- adjacent candidate handoff carriers in `history.rs` and `parent.rs`.

The formalization exposes the missing or underspecified carriers:

- explicit `Γ_a = (V_a, E_a, μ_a)` projection identity/binding;
- explicit grant components `R`, `W`, and `F`;
- explicit resolved touches `(Q_r, Q_w)`;
- a dedicated `CheckedProposal`;
- material validity evidence for `valid_a(q)`;
- History admission evidence that records the full chain;
- explicit admission rules from records to `AdmissibleEvidence<D>` for
  diagnosis, selection, surface checking, and operator projection.

These are mostly sharper return types and proof carriers, not a demand for a
large new API surface.

## Parent Planning Layer

The bounded edit adapter answers:

```text
can this proposed edit be checked and applied safely?
```

It does not answer:

```text
what failure should we try to repair?
which surface is causally upstream of that failure?
what objective should the harness pursue?
```

Those questions belong to the parent planning layer. The parent already has
access to the needed evidence because the child-selection and scoring machinery
persists mechanical and LLM-adjudicated results into blocks/History.

### Inputs

The parent planning layer should read a bounded History window containing:

- mechanical evaluation stats;
- protocol/adjudicated judgments;
- score decomposition and child-selection records;
- build/test/oracle outcomes;
- timing/cost/provider failure evidence;
- invalid proposal or failed materialization evidence;
- prior surface choices and proposal/check/apply evidence when available.

The implementation should not start by scraping rendered CLI output. It should
consume the same typed records that successor selection already uses, or add a
small typed projection over those records if the selection view is too narrow.

### Diagnosis

`Diagnosis` identifies what hurt the score or blocked improvement.

Examples:

```text
Diagnosis {
  limiter: invalid_candidate_generation,
  failure_kind: semantic_edit_resolution,
  evidence_refs: [...],
  confidence: high,
}
```

```text
Diagnosis {
  limiter: child_evaluation_regression,
  failure_kind: patch_semantically_wrong,
  evidence_refs: [...],
  confidence: medium,
}
```

The useful first taxonomy is operational, not philosophical:

```text
generation failure
surface violation
stale graph/index
hash mismatch
build failure
test/oracle failure
protocol reasoning failure
retrieval/context failure
selection/scoring failure
runtime cost/failure
```

The taxonomy can be refined later. The key requirement is that "score bad" must
be decomposed into a failure kind that can route to a surface family.

### Surface Choice

`SurfaceChoice` maps diagnosis to a surface family and a bounded grant
construction strategy.

In the long-term design, this should be a graph-slice constructor:

```text
failure_kind
  -> anchors A_f ⊆ V_a
  -> readable subgraph R_f
  -> writable subgraph W_f
  -> forbidden subgraph F_f
  -> grant g_f = (R_f, W_f, F_f)
```

For the current branch, anchors and expansion should use exact lookup,
containment relations, same-file/same-module structure, explicit companion
lists, and read-only search hits. Do not claim call-graph or type-reference
causality until those relations exist on this branch.

Examples:

```text
semantic edit target ambiguous
  -> surface family: ploke-tui semantic resolver
  -> writable: selected functions in crates/ploke-tui/src/rag/tools.rs
  -> read: code_edit tool, ploke-db exact graph helpers, span/hash carriers
```

```text
invalid patch application or hash mismatch
  -> surface family: ploke-tui edit application / ploke-io bridge
  -> writable: narrow apply/preview/hash-check functions
```

```text
poor retrieved code context
  -> surface family: ploke-rag or ploke-tui request_code_context path
  -> writable: retrieval/context assembly functions
```

```text
tool description causes bad model behavior
  -> surface family: tool text artifacts
  -> writable: specific tool description files
```

```text
selection appears wrong
  -> surface family: ploke-eval selection/projection
  -> default action: do not ordinary-edit; require a higher-trust
     protocol-upgrade/fork surface before mutating policy-bearing code
```

The first implementation can use a mechanistic routing table:

```text
failure_kind -> surface_family -> grant_constructor -> objective_template
```

The `grant_constructor` should record which relation kinds it used:

```text
relation_basis = [
  exact_anchor,
  same_file,
  same_module,
  containing_impl,
  explicit_companion,
  text_search_read_only,
]
```

Later constructors may add:

```text
type_ref_neighbor
caller
callee
```

but those should be absent or marked unavailable in current-branch evidence.

A bounded LLM-adjudicated protocol step may be added before or after this table,
but it should produce structured fields and evidence references. It should not
replace the grant/check machinery.

### Edit Objective

`EditObjective` is the repair task handed to the harness.

It should include:

- diagnosis and evidence references;
- selected surface family;
- target metric or failure rate to improve;
- read context;
- intended writable target family;
- constraints and forbidden areas;
- success criteria;
- requested number of candidate proposals;
- whether tests or cargo checks should be run by the harness if available.

Example:

```text
EditObjective {
  diagnosis: semantic_edit_resolution ambiguity,
  target_metric: reduce invalid edit-surface candidates,
  read_context: [code_edit tool, graph_resolve_exact, resolver tests],
  writable_intent: exact resolver functions in ploke-tui rag/tools,
  constraints: exact resolution only; do not mutate ploke-eval,
  success: existing edit_surface tests pass and ambiguous-canon test improves,
  candidates: 6,
}
```

The `EditObjective` becomes part of proposal evidence. Later History can compare
which diagnoses, surfaces, and objective templates actually led to improving
children.

### Mechanistic Policy First

For now, prefer the option of a mechanistic parent planning policy:

```text
History stats -> failure classifier -> routing table -> surface choice
```

Reasons:

- it is easier to test deterministically;
- it avoids making surface choice an opaque model judgment too early;
- it lets us accumulate data about whether the routing table is sensible;
- it keeps LLM adjudication available as a bounded helper rather than the only
  source of planning authority.

LLM-adjudicated decision procedures can still be used for narrow tasks:

```text
classify this failure among known labels;
rank these two candidate diagnoses;
summarize why the last child regressed;
propose an objective inside this already-selected surface.
```

The parent should record whether a diagnosis or surface choice was produced by
mechanistic policy, LLM adjudication, or a combination.

## First Worked Route: Semantic Edit Resolution

The first route should be concrete enough to implement and test without a call
graph or type-reference graph.

```text
limiter: invalid_candidate_generation
failure_kind: semantic_edit_resolution
surface_family: ploke-tui semantic edit resolver
target_metric: reduce invalid edit-surface candidates
```

This route is the best first slice because it connects directly to the current
`tui-edit-surface` candidate generator and the `ploke-tui-tools` surface, while
remaining outside the policy-bearing `ploke-eval` runtime surface.

### Evidence Available Now

Current code already has parent-readable evidence for child outcomes and
selection:

- child/runtime/branch/evaluation evidence;
- baseline/treatment operational metrics;
- protocol/adjudicated metrics and diagnostics;
- selection inputs, candidate-set commitments, and selection decisions;
- positive edit-surface evidence when it has been projected into
  `CandidateArtifact.surface` or sealed evaluation evidence.

This is enough to say how evaluated children performed and why selection chose
or rejected them.

It is not yet enough to diagnose failed semantic edit resolution unless the
edit-surface failure is explicitly recorded. Local adapter states such as
proposal, check, write, or apply attempts do not become parent-time diagnosis
evidence merely because they existed during candidate generation.

### Immediate Recording Gap

The current durable shape records successful checked/applied surface evidence
better than rejected edit-surface attempts.

For this route, History needs a typed record or sealed projection for failed
candidate-generation attempts such as:

- stale graph or projection identity;
- ambiguous or missing semantic target resolution;
- proposed read or write outside the grant;
- write intersection with the forbidden set;
- expected hash mismatch;
- partial apply or auto-apply refusal;
- harness-local failure before a candidate Artifact exists.

Without this, the parent can observe that no useful child improved, but it
cannot mechanistically conclude that the semantic resolver surface is the
causally upstream place to edit.

### Diagnosis Rule

The first mechanistic classifier can be intentionally simple:

```text
if recent candidate-generation attempts include edit_surface_failure
  and failure.kind in {
    stale_projection,
    semantic_target_missing,
    semantic_target_ambiguous,
    outside_read_grant,
    outside_write_grant,
    forbidden_write,
    expected_hash_mismatch,
    partial_apply,
  }
then
  Diagnosis {
    limiter: invalid_candidate_generation,
    failure_kind: semantic_edit_resolution,
    evidence_refs: failed attempt refs,
    confidence: high when failure count crosses threshold,
  }
```

If this evidence is absent, the classifier must not infer semantic resolver
failure from low scores alone. It may return `unknown` or route to a broader
analysis objective.

### Surface Construction

For the current branch, construct the graph subsets from exact anchors and
manual companions:

```text
A_f =
  exact anchors for the semantic edit tool, canonical target parser,
  graph resolution path, staging path, and hash-checked write bridge

R_f =
  A_f
  ∪ same_file(A_f)
  ∪ same_module(A_f)
  ∪ containing_impl_or_trait(A_f)
  ∪ explicit read companions for ploke-db graph helpers,
    ploke-core write-span carriers, and ploke-io hash checks
  ∪ text_search_hits(anchor names, read_only)

W_f =
  selected resolver/staging/tool functions in the ploke-tui edit path
  ∪ selected local tests for that path

F_f =
  crates/ploke-eval/**
  ∪ unrelated crates
  ∪ lower-level substrates such as ploke-db/ploke-io unless this route
    explicitly admits them as writable
```

Callers, callees, and type-reference neighbors remain future expansions. They
must not be part of the v1 admission check until those relations exist on this
branch.

### Edit Objective Template

The parent should hand the harness a concrete objective, not a vague score
improvement request:

```text
EditObjective {
  diagnosis: invalid_candidate_generation / semantic_edit_resolution,
  evidence_refs: [...],
  surface_family: ploke-tui semantic edit resolver,
  target_metric: reduce rejected edit-surface candidate attempts,
  read_context: resolver path, staging path, graph helper refs, prior failures,
  writable_intent: exact selected resolver/staging/tool functions and tests,
  constraints: exact resolution only; no ploke-eval mutation; no broad
    non-semantic patch unless granted as whole-file write,
  success: existing edit_surface tests pass and the failure-kind regression
    test changes from rejected to checked/applied or to a clearer typed
    rejection,
  candidates: configured child fanout,
}
```

The objective should be recorded with the proposal evidence so later History
queries can compare which diagnoses, surfaces, and templates actually led to
improving children.

### Minimal Implementation Consequence

Before a useful parent planning policy can target this route, the code needs a
durable bridge from edit-surface attempt state into History evidence:

```text
edit_surface attempt/check/apply result
  -> typed SurfaceEvidence or surface_attempt::Evidence
  -> CandidateArtifact / EvaluationPayload evidence
  -> parent-time History query
  -> Diagnosis
```

Positive checked/applied evidence should continue to use the existing
`SurfaceEvidence` path where possible. Rejected attempts need an equivalent
typed evidence shape; otherwise they are invisible to the next parent except as
absence or low score.

## Operation Interface

The operation-level contract should support these capabilities.

### 1. Bind To The Target Artifact

The harness must operate against the artifact named by `ploke-eval`, not an
ambient user TUI session.

For a `ploke-tui` implementation this likely means constructing a harness
runtime over the candidate parent worktree or an artifact-local checkout and
loading/indexing that workspace explicitly.

### 2. Project Or Report Graph Identity

If semantic resolution uses a code graph or database index, the harness must
report which artifact/tree/index the projection came from.

`ploke-eval` must be able to reject stale projections before apply.

In formal terms, this is the construction of `Γ_a`. The projection returned by
the graph adapter must carry an identity or binding to Artifact `a`; otherwise
the later subset and hash judgments have no stable domain.

### 3. Search Within A Read Surface

The harness may use exact lookup, graph edges, RAG context, or other discovery
tools, but discovery is bounded by the readable grant. Search output is
candidate evidence, not write authority.

Search should be understood as a read over `R`. It may help construct a
proposal, but it does not enlarge `W`.

The harness receives an `EditObjective`, not just a grant. The objective tells
the harness what failure to repair and which evidence to consider. The grant
tells the harness what it may read and write.

### 4. Resolve Semantic Targets

The harness must resolve semantic edit requests such as:

```text
file + canon + node_type + replacement code
```

into material write data:

```text
relpath / absolute path
node id or canon id when available
start byte
end byte
expected file hash
replacement
namespace or crate identity
```

Resolution should be exact for v1. Relaxed fallback can be added later only if
it is explicitly represented in the proposal and surface policy.

Resolution should produce, or make derivable, `ρ_a(q) = (Q_r, Q_w)`. A resolver
that only returns byte spans is insufficient unless those spans retain enough
node/canon/file/hash evidence to reconstruct the written graph nodes.

### 5. Stage A Proposal

The harness returns a proposal and harness-run evidence. A proposal should
include:

- proposal id;
- request/call/run ids where available;
- canonical target data;
- resolved material spans and expected hashes;
- preview or preview digest;
- reported touched files/spans;
- harness telemetry references.

For `ploke-tui`, existing staging and preview generation are reusable here.
They remain executor evidence, not authority.

### 6. Return Effective Request Policy Receipts

For every Router/model call that materially shapes a proposal, the harness must
return an effective request-policy receipt. Replay does not need to reproduce
the same model output, but it must reconstruct the same client-side request
policy that the parent runtime used.

The receipt should include:

- Router trait/backend identity and policy id;
- router/config source paths or ids plus digests;
- which values were explicit and which came from defaults;
- resolved model/provider request fields such as model, endpoint class,
  temperature, top_p, top_k, max tokens, seed, stop sequences, response format,
  tool choice, reasoning effort, timeout, retry policy, and fallback policy;
- prompt, schema, and tool definition digests;
- material context selection policy and digest;
- request payload digest and response payload digest;
- external execution receipt fields when available: provider request id,
  response id, provider selected, finish reason, usage, routing metadata, and
  system fingerprint;
- explicit `unknown` values for external metadata the provider does not expose.

The receipt must distinguish:

```text
client_policy_hash
request_payload_hash
response_payload_hash
external_execution_receipt_hash
```

This is a provenance requirement, not a determinism claim. If OpenRouter or an
upstream provider routes nondeterministically, uses an unknown backend, or
returns no quantization/fingerprint metadata, that nondeterminism should be
recorded as unknown external execution detail. It must not be confused with a
different client-side policy.

Admission invariant:

```text
AdmitProposal(q) =>
  ∀ call ∈ material_model_calls(q).
    ∃ receipt(call).
      complete_effective_policy(receipt)
```

`material_model_calls(q)` are model calls that shape the target, patch,
proposal, validation argument, or other proposal-authoring evidence. UI chatter
and non-authority summaries may remain telemetry, but they must not be cited as
proposal provenance.

This receipt is especially important at the `ploke-eval` / `ploke-tui` /
`ploke-llm::Router` boundary. The adapter must not rely on ambient `ploke-tui`
or provider config that is not represented in the receipt.

### 7. Expose Touches Before Apply

`ploke-eval` needs structured touches before any write happens:

```text
files
byte spans
canonical paths
node kinds
expected hashes
created/deleted paths if supported
```

This is what makes a real `SurfaceCheck` possible.

The touch carrier should distinguish read/context touches from write touches:

```text
Touches = {
  read:  Q_r,
  write: Q_w,
}
```

The first implementation may store these as vectors of resolved touch records,
but the distinction must not be lost.

### 8. Apply Only With Checked Authorization

The harness must not self-authorize writes. It applies only after receiving a
`ploke-eval` check/authorization value derived from the proposal and grant.

For v1, the authorization should only exist for a passed check:

```text
SurfaceCheck::Passed -> CheckedProposal -> apply_checked
```

### 9. Return Apply Evidence

The apply result must include:

- every file touched;
- before hashes;
- after hashes;
- applied spans or ranges;
- failed edits if any;
- whether the write was all-or-rejected;
- enough data for `ploke-eval` to compute or verify the derived Artifact.

### 10. Use All-Or-Rejected Semantics For V1

Partial application should not silently count as success. Either all planned
edits apply, or the operation returns a rejected/failed outcome with evidence.

Later partial-edit behavior can be modeled explicitly, but it should not be the
default for candidate creation.

### 11. Hide Harness Internals

The adapter may use `ploke-tui::AppState`, proposal maps, commands, actors,
event bus, or `TestRuntime` internally. The trait surface should not expose
those as protocol objects.

`ploke-eval` should see operation evidence, not TUI session structure.

## Sketch

The exact Rust names should follow the surrounding module structure, but the
semantic split should look like this:

```rust
trait CodeGraphView {
    type Artifact;
    type Projection;
    type Bounds;
    type Query;
    type Target;
    type Hit;
    type Span;
    type Delta;
    type Error;

    fn project(&self, artifact: &Self::Artifact) -> Result<Self::Projection, Self::Error>;

    fn projection_identity(
        &self,
        projection: &Self::Projection,
    ) -> Result<ProjectionIdentity, Self::Error>;

    fn bounds(
        &self,
        projection: &Self::Projection,
        grant: &SurfaceGrant,
    ) -> Result<Self::Bounds, Self::Error>;

    fn search(
        &self,
        projection: &Self::Projection,
        bounds: &Self::Bounds,
        query: &Self::Query,
    ) -> Result<Vec<Self::Hit>, Self::Error>;

    fn resolve(
        &self,
        projection: &Self::Projection,
        bounds: &Self::Bounds,
        target: &Self::Target,
    ) -> Result<ResolvedTarget<Self::Span>, Self::Error>;

    fn delta(
        &self,
        before: &Self::Projection,
        after: &Self::Projection,
        patch: &ArtifactDelta,
    ) -> Result<Self::Delta, Self::Error>;
}
```

`ProjectionIdentity` is the implementation carrier for the binding `Γ_a ↝ a`.
It should include whatever the current backend can honestly prove: artifact id,
tree hash, workspace root digest, DB snapshot id, parser/index identity, or a
manifest reference. Weak early identities are acceptable if they are named as
weak and rejected when they cannot support a safe write.

`ResolvedTarget` should retain graph identity, not just a byte span:

```rust
struct ResolvedTarget<Span> {
    node: GraphNodeRef,
    canon: Option<String>,
    node_kind: NodeKindRef,
    span: Span,
    expected_file_hash: String,
    resolution: ResolutionMode,
}
```

The exact field types should follow existing code. The semantic requirement is
that this object can populate `Q_w` and material validity checks.

```rust
trait EditHarness {
    type Graph: CodeGraphView;
    type Proposal;
    type Run;
    type Touches;
    type Applied;
    type Error;

    fn graph(&self) -> &Self::Graph;

    fn propose(
        &self,
        input: EditInput<'_>,
    ) -> Result<(Self::Proposal, Self::Run), Self::Error>;

    fn touches(
        &self,
        proposal: &Self::Proposal,
    ) -> Result<Self::Touches, Self::Error>;

    fn apply_checked(
        &self,
        proposal: Self::Proposal,
        authorization: CheckedProposal,
    ) -> Result<Self::Applied, Self::Error>;
}
```

`Self::Touches` should be structurally equivalent to:

```rust
struct ResolvedTouches<Touch> {
    read: Vec<Touch>,
    write: Vec<Touch>,
}
```

The harness may do local preflight validation, but authority validation remains
in `ploke-eval`. Avoid adding a harness method named `validate` if it would
blur that line; prefer `preflight` for harness-local checks and reserve
`SurfaceCheck` / `CheckedProposal` for `ploke-eval`.

`CheckedProposal` is produced by `ploke-eval`, not by the harness:

```text
SurfaceGrant + ResolvedTouches + artifact/index/hash evidence
  -> SurfaceCheck
  -> CheckedProposal only if passed
```

The check should embody:

```text
ρ_a(q) = (Q_r, Q_w)
g = (R, W, F)
Q_r ⊆ R
Q_w ⊆ W
Q_w ∩ F = ∅
valid_a(q)
```

## Ploke-TUI Adapter

The first `ploke-tui` adapter can reuse these pieces:

- canonical edit request shape from `apply_code_edit`;
- canonical target parsing;
- exact DB resolution to material spans;
- `WriteSnippetData`;
- preview/diff generation;
- overlap/range conflict detection;
- `ploke-io` hash-checked writes;
- `TestRuntime` or a similar actor harness to stand up the needed TUI services
  without the terminal UI.

It must not expose these as authority:

- `AppState.proposals`;
- `EditProposalStatus`;
- user config proposal persistence;
- TUI `edit approve`;
- chat/tool UI events;
- `RetrievalScope::LoadedWorkspace` as if it were an edit grant.

Those may be internal mechanics or telemetry references. The admitted operation
is the checked Artifact transition recorded by `ploke-eval`.

## First Useful Surface

The near-term writable surface should target the `ploke-tui` tool/edit area:

```text
crates/ploke-tui/src/tools/**
crates/ploke-tui/src/rag/tools.rs
crates/ploke-tui/src/rag/editing.rs
```

The first semantic target set should be narrower than those globs. Prefer one
or a few explicit functions/methods resolved exactly through the code graph,
with `crates/ploke-eval` kept immutable.

After this works, admit similarly bounded `ploke-db` method targets through
explicit semantic grants.

## Implementation Plan

### Phase A: Current-State Audit

Use sub-agents to compare this plan against the current code. Report:

- where the current edit-surface module lives;
- whether `SurfaceGrant`, `SurfaceCheck`, checked apply evidence, and
  child-materialization evidence already exist;
- whether `tui-edit-surface + ploke-tui-tools` is live, deterministic-only, or
  fail-closed;
- whether surface evidence reaches candidate/History selection paths;
- what History/scoring records are available for parent-side diagnosis and
  surface selection;
- the smallest tests that currently prove each claim.

No broad implementation should start until this audit is current, because older
handoff docs mention dirty Phase 4 work and the CLI surface was recently culled.

### Phase B: Authority-Side Adapter Boundary

Define or repair the `ploke-eval` boundary module around:

- graph projection/resolution traits, including projection identity for `Γ_a`;
- edit harness trait;
- `SurfaceGrant` as explicit or derivable `(R, W, F)`;
- `SurfaceCheck`;
- `CheckedProposal`;
- resolved touches `(Q_r, Q_w)`;
- material validity evidence for `valid_a(q)`;
- apply evidence.

Use mocks or deterministic fixtures first. Do not import TUI session concepts
into the authority-side types.

Minimum authority-side tests:

- a grant with `W ⊆ R` accepts a proposal whose `Q_w ⊆ W` and `Q_r ⊆ R`;
- a proposal with `Q_w ∩ F ≠ ∅` is rejected;
- a proposal with stale projection identity is rejected;
- a proposal with mismatched expected hash is rejected;
- `apply_checked` cannot be called without a `CheckedProposal` in normal code;
- the mock harness can produce all-or-rejected apply evidence from a checked
  proposal.

Verification:

```bash
cargo fmt --all
cargo check -p ploke-eval 2>&1 | tail -n 80
cargo test -p ploke-eval edit_surface --lib 2>&1 | tail -n 40
```

### Phase C: Parent Planning Policy

Add or repair the parent-side planning projection:

```text
History -> Diagnosis -> SurfaceChoice -> EditObjective
```

Minimum behavior:

- read the same typed History/scoring evidence used by child selection;
- classify one or a few concrete failure kinds;
- route those failure kinds to named surface families;
- produce an `EditObjective` with evidence refs and success criteria;
- record the planning decision as candidate-generation evidence.

Start with a mechanistic routing table. Add bounded LLM adjudication only as a
structured helper that outputs known labels or ranked options.

Minimum tests:

- invalid candidate generation routes to edit-surface resolver/objective;
- build failure routes away from resolver work and toward build/test surface or
  stop/defer if no safe surface exists;
- selection/scoring failure does not ordinary-edit `ploke-eval` without an
  explicit higher-trust surface;
- the produced `EditObjective` carries evidence references from History.

### Phase D: Ploke-TUI Harness Adapter

Implement a concrete adapter that uses `ploke-tui` machinery behind the trait.

Minimum behavior:

- bind to an explicit target artifact/worktree;
- return effective request-policy receipts for proposal-producing
  `ploke-llm::Router` calls;
- load or project an index tied to that artifact;
- resolve one exact semantic target inside the granted surface;
- stage proposal evidence;
- expose resolved read/write touches before apply;
- apply only after `CheckedProposal`;
- return all-or-rejected apply evidence.

Use `TestRuntime` or an equivalent harness if it is the least invasive way to
stand up TUI services without terminal UI coupling.

The adapter may use TUI proposal staging internally. It must return operation
evidence across the trait boundary, not `AppState.proposals` as authority.

Before wiring the real TUI adapter, prove the request-policy receipt behavior
with a fake Router or deterministic harness:

```text
A': parent artifact + router config + EditObjective
B*: harness builds proposal-producing Router request
C': complete effective request-policy receipt with stable client_policy_hash
```

Negative tests should reject proposal admission when a material model call has
no receipt or an incomplete effective policy.

### Phase E: Candidate Creation Integration

Wire the adapter into Prototype 1 candidate creation:

```text
Parent<Ruling>
  -> diagnose from History
  -> choose surface
  -> construct EditObjective and grant
  -> ask harness for N proposals
  -> check each proposal
  -> apply checked proposals into N derived Artifacts
  -> publish child plan with surface evidence
```

Failures must occur before child materialization/build when:

- no grant exists;
- projection/index identity is stale;
- `Q_r` or `Q_w` escape the grant;
- `Q_w` intersects the forbidden subgraph;
- hashes mismatch;
- apply is partial or failed.

### Phase F: History Evidence And Selection

Persist enough evidence so later selection/replay can answer:

- what diagnosis was made?
- why was this surface chosen?
- what objective was given to the harness?
- what surface was granted?
- what proposal was made?
- what exactly did it touch?
- why was it admitted?
- what ArtifactDelta was produced?
- how did the child evaluate?
- why was this candidate selected or rejected?

The evidence may initially live inside existing candidate payload/history
records, but it must not be only a mutable report or UI projection.

### Phase G: Short Live Run

Before any long run, set up a short run:

```text
generations = 3
candidate-generator = tui-edit-surface
edit-surface = ploke-tui-tools
successor-selection = history-score-child-prop
successor-selection-metrics = operational-and-protocol
```

Confirm:

- child fanout still runs concurrently;
- candidate Artifacts were derived from checked edit evidence;
- children self-evaluate from rebuilt runtimes;
- History-backed selection sees the candidates;
- successor handoff works;
- the run can be inspected without relying on mutable TUI/session state.

## Sub-Agent Brief

When delegating implementation, point agents here and give them a narrow phase.

Required invariant for every phase:

```text
The harness may propose and mechanically apply.
`ploke-eval` grants, checks, admits, records, hydrates, and selects.
```

Common reductions to reject:

- treating TUI proposal status as authority;
- using path globs as the whole edit surface;
- collapsing `(R, W, F)` into a single allowlist;
- collapsing `(Q_r, Q_w)` into a single undifferentiated touch list;
- choosing a surface randomly or only from a CLI flag when History evidence can
  diagnose a likely failure kind;
- giving the harness a vague "improve the score" task instead of a concrete
  `EditObjective`;
- allowing child runtimes to create candidate Artifacts;
- accepting partial apply as success;
- hiding surface evidence in CLI reports only;
- importing `ploke-tui` session types across broad areas of `ploke-eval`;
- mutating `crates/ploke-eval` as an ordinary candidate surface.

## Done Criteria

This workstream is ready for longer multi-generation runs when:

- the trait adapter exists and has tests with a mock/deterministic harness;
- parent planning can produce a `Diagnosis`, `SurfaceChoice`, and
  `EditObjective` from History evidence for at least one common failure kind;
- the graph projection has an artifact identity/binding sufficient for the
  admitted write mode;
- grants represent or derive `R`, `W`, and `F`;
- proposal touches preserve read/write distinction;
- `CheckedProposal` is the only normal path into checked apply;
- a `ploke-tui` adapter can produce and apply at least one checked semantic
  edit inside the admitted surface;
- out-of-surface, stale-index, stale-hash, and partial-apply cases fail before
  child build;
- candidate/History evidence records grant, proposal, check, apply, and derived
  Artifact identity;
- selection/replay can inspect the evidence used to choose a successor;
- a short run over `ploke-tui` tool surface reaches successor handoff.
