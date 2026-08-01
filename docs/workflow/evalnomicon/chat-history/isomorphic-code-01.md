Yes. What you want is not “good naming.” You want a **representation relation** between the formal system and the code, so that the structural distinctions in the formalism reappear as enforced distinctions in Rust.

That is a real thing.

The closest precise statement is not usually “the code is isomorphic to the math” in the strict sense. It is more like:

```text
the code is a faithful refinement of the formal model
```

or:

```text
there is an abstraction map from code states to formal states
that preserves the important structure and transitions
```

The reason I would not say strict isomorphism is that code almost always contains extra representational detail that the formal layer does not care about:

```text
temp dirs
file paths
logs
timestamps
backend handles
serialization details
retry counters
```

Those are not part of the core ontology.

So the right target is:

## 1. A commuting abstraction, not a vague resemblance

You want an abstraction map:

```text
⟦ · ⟧ : Code -> Formal
```

such that the important transitions commute.

If the formal system says:

```text
i_proposed --materialize--> i_planned
```

then the code should satisfy:

```text
⟦ materialize_code(ci) ⟧ = materialize_formal(⟦ ci ⟧)
```

Same for:

```text
stage
apply
validate
commit
```

So the real goal is:

```text
code transition, then abstract
=
abstract, then formal transition
```

That is the structure-preservation property you are reaching for.

## 2. The right decomposition

There are really three levels here.

### A. Formal ontology

What kinds of things exist?

```text
surface S
proposal
plan
staged edit
applied result
validated result
candidate state
event / edge
```

### B. Formal transitions

What moves one kind of thing to another?

```text
materialize : Proposal[S] -> Plan[S]
stage       : (C_g, Plan[S]) -> Staged[S]
apply       : Staged[S] -> Applied[S]
validate    : Applied[S] -> Validated[S]
commit      : (C_g, Validated[S]) -> Event[S] + C_{g+1}
```

### C. Code reification

How do those distinctions become compiler-visible?

```text
generic families
phase indices
private constructors
trait bounds
smart constructors
runtime graph ids
```

That is the bridge.

## 3. The key question: what should be one type family?

If you say:

> these are essentially the same object but in different states

then yes, that is exactly where a **typestate-indexed family** is the right move.

So instead of:

```rust
Proposal
Validated
Applied
```

as unrelated nouns, you can make one family:

```rust
Intervention<S, P>
```

where:

* `S` is the bounded surface
* `P` is the phase/state in the lifecycle

That is much closer to the formal picture.

## 4. The formal meaning of that family

This says:

```text
Intervention[S, P]
```

is the code representation of:

```text
an intervention over surface S at formal phase P
```

Then your formal phases become actual Rust phase indices:

```rust
pub struct Proposed;
pub struct Planned<T>(pub T);
pub struct Staged<T>(pub T);
pub struct Applied<T>(pub T);
pub struct Validated<T>(pub T);
```

or equivalently with more explicit names.

Then:

```rust
pub struct Intervention<S: Surface, P> {
    core: Core<S>,
    phase: P,
}
```

where `core` is the phase-stable identity of the intervention:

```rust
pub struct Core<S: Surface> {
    target: S::Target,
    evidence: S::Evidence,
    policy: S::Policy,
    intent: Intent,
}
```

Now the conceptual claim:

```text
same intervention, different phase
```

is not just a comment. It is literally represented as:

```text
same Core<S>, different P
```

That is exactly the kind of structural preservation you are asking for.

## 5. This is the code analogue of the formal transition system

Formally, we had something like:

```text
Proposal[S] -> Plan[S] -> Staged[S] -> Applied[S] -> Validated[S]
```

In Rust, that becomes:

```rust
fn materialize<S: Surface>(
    x: Intervention<S, Proposed>,
) -> Result<Intervention<S, Planned<S::Plan>>, Error>;

fn stage<S: Surface>(
    from: &CandidateState,
    x: Intervention<S, Planned<S::Plan>>,
) -> Result<Intervention<S, Staged<S::Staged>>, Error>;

fn apply<S: Surface>(
    x: Intervention<S, Staged<S::Staged>>,
) -> Result<Intervention<S, Applied<S::Applied>>, Error>;

fn validate<S: Surface>(
    x: Intervention<S, Applied<S::Applied>>,
) -> Result<Intervention<S, Validated<S::Report>>, Error>;
```

That is not just “inspired by” the formalism.

It **is** the formalism, reified into type signatures.

## 6. Where `Surface` fits

Earlier, in the formal framing, we kept talking about a bounded target/surface:

```text
T = (artifact_ref, allowed_region, contract, validation_policy)
```

In code, the family parameter `S` is how that bounded target class becomes structural.

For example:

```rust
pub trait Surface {
    type Target;
    type Evidence;
    type Policy;
    type Plan;
    type Staged;
    type Applied;
    type Report;
}
```

Then:

```rust
pub enum ToolText {}
pub enum PolicyConfig {}
```

or zero-sized marker structs, whichever you prefer.

And:

```rust
impl Surface for ToolText {
    type Target = ToolName;
    type Evidence = ToolTextEvidence;
    type Policy = ToolTextPolicy;
    type Plan = TextPatchPlan;
    type Staged = StagedTextEdit;
    type Applied = AppliedTextEdit;
    type Report = ToolTextValidationReport;
}
```

Now a `ToolText` intervention and a `PolicyConfig` intervention are not merely two enum variants hidden inside one blob. They are different indexed families.

That preserves the formal distinction between bounded intervention surfaces.

## 7. When PhantomData is right

If a phase carries no runtime payload, use `PhantomData`.

If a phase has a real witness, make it an actual field.

So for `Proposed`, this is fine:

```rust
pub struct Proposed;
```

or:

```rust
pub struct Proposed(PhantomData<()>);
```

For `Validated`, if validation actually produces something meaningful, prefer:

```rust
pub struct Validated<R> {
    report: R,
}
```

Likewise for staged/applied phases:

```rust
pub struct Planned<P> {
    plan: P,
}
```

So the pattern is:

* marker-only when the phase is purely logical
* witness-carrying when the phase introduces new evidence/guarantee

That also mirrors the formal system better, because some transitions produce real objects, not just state labels.

## 8. What the code should preserve

There are five structural things worth preserving from formalism into code.

### 1. Sorts

Different formal kinds should become different Rust kinds.

```text
candidate state ≠ intervention ≠ validation report ≠ event
```

Do not collapse these into one “manager” object.

### 2. Indices

If the formalism distinguishes families by surface or phase, encode that as type parameters.

```text
surface S  -> generic S
phase P    -> generic P
```

### 3. Constructors

Only legal formal objects should be constructible.

That means:

* private fields
* smart constructors
* phase transitions through functions, not public mutation

### 4. Judgments/invariants

If the formalism says “this object is validated,” that should be a type-level fact or an explicit witness, not a bool.

Bad:

```rust
struct Intervention {
    validated: bool,
}
```

Better:

```rust
Intervention<S, Validated<R>>
```

### 5. Transitions

Formal inference rules or transition relations should become typed functions whose signatures enforce the domain/codomain.

That is the biggest one.

## 9. What should stay at runtime

Not everything belongs in the type system.

The whole branch graph should usually stay runtime:

```text
state ids
parent ids
generation
branch refs
status
evaluation records
```

That is because the graph shape is open-ended and data-dependent.

So the disciplined split is:

### Type system for local step structure

```text
surface
phase
witnesses
legal transitions
```

### Runtime graph for global search structure

```text
candidate nodes
event edges
branch/merge/fork history
selected-best node
```

That matches your Prototype 1 split very well: Step 4 is the typed local intervention machinery, and Step 5 is the candidate-state ledger/runtime graph. 

## 10. The most consistent mapping

If we make it precise, the bridge looks like this.

### Formal

```text
C_g                     candidate state at generation g
I[S, proposed]          proposed intervention on surface S
I[S, planned]           materialized intervention
I[S, staged]            staged intervention
I[S, applied]           applied intervention
I[S, validated]         validated intervention

materialize : I[S, proposed] -> I[S, planned]
stage       : C_g × I[S, planned] -> I[S, staged]
apply       : I[S, staged] -> I[S, applied]
validate    : I[S, applied] -> I[S, validated]
commit      : C_g × I[S, validated] -> (α_g^S, C_{g+1})
```

### Code

```rust
pub struct CandidateState { /* runtime node */ }

pub struct Intervention<S: Surface, P> {
    core: Core<S>,
    phase: P,
}

fn materialize<S: Surface>(
    x: Intervention<S, Proposed>,
) -> Result<Intervention<S, Planned<S::Plan>>, Error>;

fn stage<S: Surface>(
    from: &CandidateState,
    x: Intervention<S, Planned<S::Plan>>,
) -> Result<Intervention<S, Staged<S::Staged>>, Error>;

fn apply<S: Surface>(
    x: Intervention<S, Staged<S::Staged>>,
) -> Result<Intervention<S, Applied<S::Applied>>, Error>;

fn validate<S: Surface>(
    x: Intervention<S, Applied<S::Applied>>,
) -> Result<Intervention<S, Validated<S::Report>>, Error>;

fn commit<S: Surface>(
    parent: StateId,
    x: Intervention<S, Validated<S::Report>>,
) -> Result<Event<S>, Error>;
```

That is a near-direct encoding.

## 11. Then what is `Event`?

This is where consistency with the earlier formal story matters.

The intervention **family** is the evolving typed object.

The realized intervention **event** is the edge in the candidate-state graph.

So:

```rust
pub struct Event<S: Surface> {
    pub parent: StateId,
    pub child: StateId,
    pub committed: Intervention<S, Validated<S::Report>>,
    pub evaluation: Option<EvaluationRecord>,
    pub status: EventStatus,
}
```

That corresponds to the formal edge:

```text
α_g^S : C_g -> C_{g+1}
```

This keeps the two levels separate:

* `Intervention<S, P>` = local typed object moving through phases
* `Event<S>` = runtime ledger edge once committed

That is very clean.

## 12. The right word is not exactly “isomorphic”

I do understand what you mean, but I would sharpen it like this:

You want the code to be a **faithful reification** of the formal algebra.

That means:

* every meaningful formal distinction is represented
* illegal formal transitions are unrepresentable or rejected
* code transitions correspond to formal transitions
* extra implementation detail is clearly extra, not confused with ontology

If you want a concise mathematical phrase, I would say:

```text
the code should be a refinement whose abstraction map is structure-preserving
```

or even more concretely:

```text
the important diagrams should commute
```

That is the practical version of “don’t let the architecture drift away from the model.”

## 13. A simple design test

Here is the test I would use on every type you introduce:

For a code type `X`, ask:

```text
What formal sort does X represent?
What invariants does X guarantee?
What formal transitions have X as domain or codomain?
What information in X is ontological, and what is merely operational?
```

If you cannot answer those, the type is probably not aligned with the model.

Likewise for every field:

```text
Does this field correspond to a formal component,
or is it backend noise?
```

That will keep the representation honest.

## 14. The practical conclusion for your implementation

Given what you said, I think this is the right move:

* use an `intervention` module
* use one phase-indexed family `Intervention<S, P>`
* keep a stable `Core<S>`
* make phases witness-bearing where appropriate
* keep `CandidateState` and `Event<S>` as runtime graph objects
* make the transition functions the capability seam

That gives you exactly the compiler-enforced reflection of the conceptual structure.

And it fits your Prototype 1 plan closely: bounded entities, explicit capability seams, and a candidate-state ledger are three different layers, not one blob. 

The shortest formulation is:

> The formal model should not merely inspire the code; it should determine the code’s indexed families, legal constructors, and legal transitions. In your case, that means `Intervention<S, P>` for the local phase-typed object, `Event<S>` for the committed edge, and `CandidateState` for the runtime node.

If you want, next I can sketch the exact Rust shape for `Intervention<S, P>` with phase markers and show where to put erased dispatch when you need a heterogeneous collection.
