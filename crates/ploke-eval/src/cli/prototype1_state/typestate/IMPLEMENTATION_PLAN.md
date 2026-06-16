# Prototype 1 Runtime Typestate Implementation Plan

This document records the implementation path for turning the live Prototype 1
parent controller into a structural typestate pipeline.

The goal is not to invent a second controller. The goal is to migrate the
existing live loop, edge by edge, into a form where the compiler can prove that
later phases only run after their required earlier facts exist.

The current live entrypoint is:

```rust
run_prototype1_state_turn(command: Prototype1StateCommand) -> Result<(), PrepareError>
```

The target shape is a pipeline of typed state transitions:

```rust
Transition<From, To, F>
```

composed with:

```rust
Step::then(...)
```

A transition is a typed arrow:

```text
From -> Result<To, Error>
```

The transition itself has no authority. Authority remains in the consumed state
value. Applying the transition consumes the old state and produces the next
state.

## Current state

The scaffold currently contains:

- `Runtime<Phase, Role, Context, Plan, Children, History, Evidence, Continuation, Report>`.
- Axis carriers for role, context, child plan, child set, History, evidence,
  continuation, and report facts.
- R-phase aliases for the observed Prototype 1 parent loop.
- A generic transition layer:
  - `Transition<From, To, F, Error>`
  - `Step<From>`
  - `Chain<First, Second, Mid>`
  - `transition(...)`
- The first live controller edge:
  - `R0 -> R1<Prototype1StateRunShape, ResolvedCampaignConfig>`

That first edge collects the command-derived setup inputs and then temporarily
unpacks `R1` back into the existing locals. That unpacking is an intentional
migration seam, not the final design.

## Invariants

The implementation should preserve these constraints:

- Do not weaken correctness, schema, import, or History authority semantics.
- Do not add permissive fallback behavior to make transitions easier to wire.
- Do not duplicate existing typestate carriers.
- Reuse the existing carriers directly where possible:
  - `Parent<S>`
  - C1-C5 child attempt aliases
  - `Block<S>`
  - `Crown<S>`
  - `Startup<S>`
  - `Received<T>`
  - `LineageState`
- Do not flatten several facts into long state names.
- Represent compound facts structurally through axes and type parameters.
- Use branch sum types for forks rather than `Option` fields when the branch is
  authoritative control flow.
- Keep escape hatches narrow and temporary.

## Phase 1: prove the transition layer

Add focused tests for the generic transition mechanism before moving more live
code.

Test the following behavior:

- A single `Transition` consumes a source state and produces the target state.
- `Step::then(...)` composes adjacent transitions.
- A composed chain returns the final target state.
- Errors short-circuit and later transitions are not run.
- The transition error type remains generic and is not hard-coded to
  `PrepareError`.

Normal unit tests cannot directly assert that invalid compositions fail to
compile. If compile-fail coverage becomes important, add `trybuild` or doctest
compile-fail tests later.

## Phase 2: split the module before adding more edges

The current `typestate/mod.rs` is intentionally dense while the map is being
sketched, but it should be split before the next implementation wave.

Target layout:

```text
typestate/
  mod.rs
  runtime.rs
  transition.rs
  context.rs
  aliases.rs
  axes/
    mod.rs
    phase.rs
    role.rs
    plan.rs
    children.rs
    history.rs
    evidence.rs
    continuation.rs
    report.rs
  tests.rs
```

Responsibilities:

- `mod.rs`
  - module-level explanation;
  - public-in-crate facade;
  - re-exports used by `cli_facing.rs`.

- `runtime.rs`
  - `Runtime<...>`;
  - `RuntimeRole<...>`;
  - generic axis carriers such as `Context<State>`, `Plan<A, S>`,
    `Children<Set, Attempt>`, `History<Startup, Head, Epoch>`,
    `Evidence<...>`, `Continuation<...>`, and `Report<State>`.

- `transition.rs`
  - `Transition<From, To, F, Error>`;
  - `Step<From>`;
  - `Chain<First, Second, Mid>`;
  - `transition(...)`.

- `context.rs`
  - payload context states such as `Command<T>` and `Collected<...>`;
  - temporary migration extraction types such as `CollectedParts<...>`.

- `aliases.rs`
  - R0 through R14 aliases;
  - constructor and extraction impls for R aliases where needed.

- `axes/*`
  - marker modules currently nested inside `mod.rs`.

This split should not change behavior. It is a mechanical organization step.
Run `cargo fmt --all` and `cargo check -p ploke-eval --all-targets` after the
split.

## Phase 3: isolate the first live edge for direct tests

The live `R0 -> R1` transition currently exists inline in
`run_prototype1_state_turn`.

Move the transition-producing code into a small helper while keeping the edge
structural, not semantically named as its own transition type.

Acceptable shape:

```rust
fn r0_to_r1() -> impl Step<
    R0,
    To = R1<Prototype1StateRunShape, ResolvedCampaignConfig>,
    Error = PrepareError,
> {
    transition(|r0: R0| { ... })
}
```

This helper may live near the controller at first if it needs private live-loop
types. The important point is that tests can exercise the edge without running
the whole parent loop.

Do not introduce a named transition struct for this edge.

## Phase 4: migrate the parent-identity branch

Next typed fork:

```text
R1 -> R2a
R1 -> R3
```

`R2a` is the `--init-parent-identity` terminal branch.

`R3` is the path where parent identity has been observed and the runtime can
continue toward a parent role.

Use an explicit branch sum type. Do not encode this as one state with optional
fields.

A generic branch carrier may be enough:

```rust
enum Choice<A, B> {
    A(A),
    B(B),
}
```

If the branch semantics need documentation at the type level, add a small domain
enum with clear variants.

The existing implementation details to preserve are:

- explicit `--init-parent-identity` handling;
- successor invocation validation;
- child invocation rejection;
- optional parent identity loading;
- the current error semantics when no parent identity exists.

## Phase 5: migrate parent startup role transitions

Next linear and branching edges:

```text
R3 -> R4a
R4a -> R4bGenesisChecked
R4bGenesisChecked -> R4cGenesisReady
R4a -> R4cPredecessorReady
```

This is where the global runtime state should start carrying the existing parent
role carriers directly:

```rust
Parent<Unchecked>
Parent<Checked>
Parent<Ready>
```

Do not introduce a parallel parent-state marker if the existing `Parent<S>` type
can carry the fact.

This phase should make the startup design drift visible without changing it:

the current live path reaches `Parent<Ready>`, while the intended History model
describes a stronger path through `Startup<Validated> -> Parent<Ruling>`.

Do not silently strengthen or weaken that model during this phase. Preserve live
behavior first.

## Phase 6: migrate readiness and plan acquisition

Next edges:

```text
R4c -> R5
R5 -> R6
R6 -> R7
R7 -> R8
```

These phases should move loose locals into structural axes:

- parent-start journal evidence;
- complete baseline readiness;
- search policy and child budget readiness;
- received child plan authority.

The likely carriers are already present:

- `ParentStartedEntry`
- `CompleteBaseline`
- `Prototype1SearchPolicy`
- `Prototype1ChildBudget`
- `Received<ChildPlan>`

This phase should reduce positional-argument plumbing. It should not change the
child-plan authority rules.

## Phase 7: migrate schedule shaping and child fanout

Next edges:

```text
R8 -> R9
R9 -> R10
R10 -> R11aRejectedOnly
R10 -> R11FanoutComplete
```

This phase should connect the global runtime state to the existing child attempt
island:

```text
C1 -> C2 -> C3 -> C4 -> C5
```

Do not duplicate the C1-C5 states. The global runtime should carry child-set and
attempt completion facts at the parent orchestration level, while per-child work
continues to use the existing child attempt chain.

The rejected-only branch should remain explicit. Do not hide it behind an empty
successful child set.

## Phase 8: migrate report projection and continuation decision

Next edges:

```text
R11* -> R12
R12 -> R13aStopped
R12 -> R13bHandoffCommitted
```

Use branch sum types for the continuation decision.

Keep these facts separate:

- selected successor candidate;
- continuation decision;
- handoff record;
- final report facts.

Do not collapse them into one terminal mega-state name.

## Phase 9: migrate History handoff authority

The successor handoff path should use the existing History authority carriers:

```text
LineageState -> Block<Open> -> Crown<Ruling> -> Crown<Locked> -> Block<Sealed>
```

The post-commit state should carry the fact that the sealed block was appended
and that the parent role is retired:

```text
Parent<Selectable> -> Parent<Retired>
```

This phase should preserve current History behavior. Parent identity/genesis
replacement remains a separate ADR-backed future change, not part of this slice.

## Phase 10: migrate terminal report emission

Final edges:

```text
R13aStopped -> R14aFinalStopped
R13bHandoffCommitted -> R14bFinalHandoff
```

The terminal state should represent report emission and final outcome without
claiming new authority facts that were not produced earlier.

## Phase 11: remove temporary migration seams

After adjacent phases are typed, shrink or remove:

- `R0::into_command()` if no longer needed outside the first edge;
- `R1::into_collected()`;
- `CollectedParts<...>`;
- any equivalent extraction helpers added during migration.

The final loop should mostly consume and produce typed runtime values rather than
extracting raw locals and continuing manually.

## Final target shape

The controller should eventually read like this:

```rust
let r0 = R0::new(command);
let r1 = r0_to_r1().apply(r0)?;

match r1_branch().apply(r1)? {
    Choice::A(r2a) => finish_parent_identity_init(r2a),
    Choice::B(r3) => {
        let r4a = r3_to_r4a().apply(r3)?;
        let r4c = startup_branch_pipeline(r4a)?;
        let r12 = parent_work_pipeline().apply(r4c)?;
        let r13 = continuation_branch().apply(r12)?;
        let r14 = final_report_pipeline().apply(r13)?;
        finish(r14)
    }
}
```

Linear subsequences can use `Step::then(...)`:

```rust
let pipeline = transition(|r4c| { ... })
    .then(transition(|r5| { ... }))
    .then(transition(|r6| { ... }));
```

The intended invariant is:

```text
If a function receives R9, the compiler knows every required R0-R8 fact exists.
```

That is the payoff of the migration.

## Verification rhythm

For each phase:

1. Run focused unit tests for the transition layer or migrated edge.
2. Run the relevant Prototype 1 state tests.
3. Run `cargo check -p ploke-eval --all-targets`.
4. Run `gitnexus detect-changes` and inspect `git status`.
5. Commit only the phase that just passed verification.

If a phase requires relaxing an invariant or accepting previously invalid state,
stop and ask before implementing that change.
