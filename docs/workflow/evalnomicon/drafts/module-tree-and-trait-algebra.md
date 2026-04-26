# Module Tree And Trait Algebra

Restart-safe note on the next structural step for `ploke-eval`'s intervention
work: define the semantic carriers first, then implement concrete runtime
objects against them.

This is the step between:

- informal framework and prototype notes
- and a safe Rust implementation that can support bounded staged propagation

without collapsing back into a giant `cli.rs` or another ad hoc scheduler blob.

## Reduction To Refuse

The tempting reduction is:

```text
move the current Prototype 1 code out of cli.rs
into a few new files
and call that architecture
```

That is not enough.

The real object here is not “a subcommand with helper functions.” It is a
composed semantic substrate with at least these distinct carriers:

- a bounded mutable `Surface`
- a phase-indexed `Intervention`
- a runtime branch/search `Graph`
- admissible `History` views over that graph
- a `Policy` that consumes history and branch outcomes
- a dangerous process seam that stages/builds/spawns one candidate generation
- a controller that coordinates one generation at a time

Those distinctions should exist in code structurally before we shuffle more
implementation around.

## Architectural Readout

The larger object is:

```text
a staged, branch-aware intervention/search framework over bounded surfaces
```

Prototype 1 is the first concrete controller over that substrate. It is not the
substrate itself.

The minimum structural split we want is:

- `intervention/`
  Formal kinds, trait-level contracts, phase-indexed families, and concrete
  runtime graph records used by the intervention/search substrate.
- `prototype1/`
  The first concrete controller/process/policy instantiation over that
  substrate.
- `cli.rs`
  Command surface and dispatch only.

## Relation To The Existing Drafts

This note sits on top of:

- [formal-procedure-notation.md](formal-procedure-notation.md)
- [type-state.md](type-state.md)
- [prototype-1-intervention-loop.md](prototype-1-intervention-loop.md)
- [trait-first-reification.md](trait-first-reification.md)
- [framework-ext-01.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-01.md)
- [framework-ext-02.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-02.md)
- [framework-ext-03.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-03.md)
- [framework-ext-04.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-04.md)
- [isomorphic-code-01.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/isomorphic-code-01.md)

The practical conclusion from those notes is:

```text
trait/algebra first
concrete runtime instance second
```

The CatColab `catlog` codebase is the main local precedent for this style:

- [`one/category.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/one/category.rs)
- [`one/path.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/one/path.rs)
- [`dbl/category.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/dbl/category.rs)
- [`dbl/tree.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/dbl/tree.rs)
- [`dbl/theory.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/dbl/theory.rs)
- [`dbl/model.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/dbl/model.rs)

The lesson we want from those files is not “type-level everything.” It is:

- encode the formal sorts/contracts as traits and associated types
- let concrete structs implement those contracts
- keep open-ended runtime graph instances as data
- use typed local paths/trees/views where they carry real semantic weight

## Target Module Tree

The intended next module tree is:

```text
crates/ploke-eval/src/
  cli.rs
  intervention/
    mod.rs
    surface.rs
    phases.rs
    intervention.rs
    graph.rs
    history.rs
    policy.rs
    event.rs
    ...
  prototype1/
    mod.rs
    command.rs
    controller.rs
    process.rs
    continuation.rs
    policy.rs
    report.rs
```

### Ownership

`intervention/` owns:

- trait-level formal kinds
- phase markers and transition boundaries
- graph/history/event/policy contracts
- concrete persisted graph/node/event records for the generic substrate
- bounded-surface-neutral machinery

`prototype1/` owns:

- the first concrete control policy
- one-generation orchestration for the currently active Parent runtime
- the staged build/spawn/wait leaf seam
- report rendering
- continuation/handoff behavior for this concrete prototype

"One-generation orchestration" is a bounded controller slice, not a semantic
ban on fan-out. The substrate should support a frontier of child nodes, bounded
concurrent child execution, durable child observations, and a policy that
selects exactly one successor from the generation/history view before handoff.

`cli.rs` owns:

- clap types
- command dispatch
- no semantic process or controller logic

## Minimal Trait Algebra

This is the first pass, not the final universe.

### Surface

`Surface` is the bounded writable region plus its relevant associated sorts.

```rust
pub trait Surface {
    type Target;
    type Evidence;
    type Plan;
    type Applied;
    type Report;
}
```

Prototype 1's current tool-text rewrite surface should become one concrete
`Surface` implementation.

### Phase markers

The local intervention lifecycle should be typed.

```rust
pub struct Proposed;
pub struct Planned<P> {
    pub plan: P,
}
pub struct Staged<W> {
    pub witness: W,
}
pub struct Applied<A> {
    pub applied: A,
}
pub struct Validated<R> {
    pub report: R,
}
```

These are not yet the global scheduler state. They are the local admissible
states of one intervention.

### Intervention

`Intervention<S, P>` is the phase-indexed local intervention family.

```rust
pub struct Intervention<S: Surface, P> {
    pub core: InterventionCore<S>,
    pub phase: P,
}

pub struct InterventionCore<S: Surface> {
    pub target: S::Target,
    pub evidence: S::Evidence,
}
```

Important consequence:

- local transition legality belongs here
- this is where typestate buys us the most immediately
- this should not be confused with the global graph node

### Graph

The search/branching substrate should have a trait-level contract.

```rust
pub trait Graph {
    type StateId;
    type EventId;
    type State;
    type Event;

    fn state(&self, id: &Self::StateId) -> Option<&Self::State>;
    fn event(&self, id: &Self::EventId) -> Option<&Self::Event>;
}
```

This does **not** mean the whole search universe becomes a compile-time tree.
It means the graph as a formal kind has an algebra/contract, and concrete
runtime graph implementations satisfy it.

### Event

An event is the realized graph edge or transition carrier, not the local
intervention family itself.

```rust
pub trait Event {
    type StateId;
    type Intervention;

    fn parent(&self) -> &Self::StateId;
    fn child(&self) -> &Self::StateId;
    fn intervention(&self) -> &Self::Intervention;
}
```

Concrete persisted event records can later carry:

- branch lineage
- selected winner information
- evaluation refs
- timestamps
- status fields

### History

`History` should be its own contract rather than “some helper methods over the
scheduler JSON.”

```rust
pub trait History {
    type Graph: Graph;
    type StateId;
    type View;

    fn view(&self, state: &Self::StateId) -> Self::View;
}
```

The important distinction is:

- the graph is the persisted runtime universe
- history is one admissible view over that universe

### Policy

`Policy` consumes history plus local branch outcomes and decides what the
controller is allowed to do next.

```rust
pub trait Policy {
    type History;
    type Decision;

    fn decide(&self, history: &Self::History) -> Self::Decision;
}
```

Prototype 1 continuation and branch-selection rules are one concrete policy
implementation, not the definition of policy itself.

## What Stays Runtime

The following should remain ordinary runtime data even if the traits above are
generic and strongly typed:

- node/state ids
- event ids
- persisted artifact paths
- timestamps
- scheduler snapshots
- campaign ids
- actual open-ended graph instances

The point is a typed algebra over those objects, not pretending the whole
runtime exploration frontier is a compile-time constant.

## Typed Local Structures We Probably Do Want

The CatColab lesson is especially useful here.

We likely want some local typed carriers derived from the runtime graph, for
example:

- `AncestryPath<G>`
- `ContinuationTrace<G>`
- `MergeInput<G>`

Those are good candidates for strongly typed local path/tree objects once the
graph contract exists.

## Implementation Order

The next implementation passes should be:

1. create the `intervention/` trait-algebra files with mostly-empty carriers
2. create `prototype1/` only after those contracts exist
3. make Prototype 1 implement:
   - a concrete `Surface`
   - a concrete `Graph`
   - a concrete `History`
   - a concrete `Policy`
4. move the existing controller/process/report code behind those contracts
5. only then add explicit gen1 -> gen2 handoff

This avoids hardening the current `cli.rs` layout or the current scheduler
record shapes into the architecture.

## Immediate Design Constraint

Before any new continuation behavior is added, the dangerous process seam
currently in `cli/prototype1_process.rs` should remain:

- local
- terminal for the child
- structurally separate from policy and controller logic

That seam belongs under `prototype1/process.rs` later, but only after the new
trait/module carriers exist.
