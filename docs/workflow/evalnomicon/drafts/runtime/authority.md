# Prototype 1 Runtime Authority

Status: working draft.

This note names the authority model we want the runtime code to preserve. It is
not a replacement for Rust typestate, filesystem permissions, or journal
validation. It is a compact specification for the claims those mechanisms should
make true.

## Purpose

Prototype 1 runs multiple runtimes that communicate through external surfaces:
files today, possibly sockets or another transport later. Those surfaces are
outside one Rust process, so ordinary ownership cannot prove enough by itself.

The goal is to make the boundary explicit:

```text
ExecutionPath<Role, State> + Surface<Role> + Transport
```

and then require implementation code to preserve the authority represented by
that structure.

## Vocabulary

Let:

- `R` be a runtime process.
- `Ψ` be an admitted execution path.
- `R ⊳ Ψ` mean runtime `R` has been admitted to execute along path `Ψ`.
- `role(Ψ)` be the role associated with that execution path: `Parent`, `Child`,
  or `Successor`.
- `state(Ψ)` be the typestate currently held by that execution path, such as
  `Child<Ready>` or `Parent<Retired>`.
- `A(R)` be the authority set of runtime `R`: the reads, writes, transitions,
  and process actions available to it.
- `S(role)` be the external surface exposed to a role.
- `T` be a transport backend, such as file buffers or sockets.
- `W(R)` be the set of external locations runtime `R` may write mutably.
- `M(R)` be the messages runtime `R` may send.

The intended construction is:

```text
admit(R, environment evidence) -> R ⊳ ExecutionPath<Role, Starting> + Surface<Role>
```

After admission, ordinary code should use typed carriers rather than raw paths
or role strings.

## Core Invariants

### Role Bounded Authority

For every admitted runtime/path pair `R ⊳ Ψ`:

```text
A(R) ⊆ A(role(Ψ))
```

A runtime may do only what the role associated with its admitted execution path
allows. A runtime executing along a `Child` path may evaluate its assigned node
and write child-shaped evidence. It may not stage children, select a successor,
mutate parent identity, or make continuation decisions.

### State Bounded Authority

For every admitted runtime/path pair `R ⊳ Ψ`:

```text
A(R) ⊆ A(role(Ψ), state(Ψ)) ⊆ A(role(Ψ))
```

State narrows role authority. The runtime is constrained by the state currently
held along its admitted path, not by a runtime-global role label. For example,
`Child<Starting>` may acknowledge startup, while `Child<Evaluating>` may write
evaluation progress and eventually cross to `Child<ResultWritten>`.

### Disjoint Mutable Surfaces

For any two distinct child runtimes `C_i` and `C_j` in the same parent wave:

```text
i ≠ j => W(C_i) ∩ W(C_j) = ∅
```

No two children should have mutable authority over the same file, socket stream,
result path, or channel endpoint. Shared campaign state may be readable by many
children, but mutable child outputs must be per-child or per-runtime.

### Shared Read, Scoped Write

Children may share read authority:

```text
Read(C_i) ∩ Read(C_j) may be nonempty
```

But shared reads must not imply shared writes:

```text
x ∈ Read(C_i) ∩ Read(C_j) does not imply x ∈ W(C_i) or x ∈ W(C_j)
```

Examples of shared read surfaces:

- campaign manifest
- scheduler/node records
- parent broadcast buffer
- immutable benchmark inputs

Examples of scoped write surfaces:

- child-to-parent buffer for one child/runtime
- runner result for one node/runtime
- attempt-scoped evaluation artifacts
- child telemetry stream

### Transition Records Are Projections

A durable record should be a projection of an allowed transition:

```text
Role<S> -> Role<S'> emits record(S -> S')
```

The record should not be an arbitrary status write. This is why
`Child<Ready>`, `Child<Evaluating>`, and `Child<ResultWritten>` matter: the
record is evidence of a typed transition, not just monitoring text.

### Transport Substitution

For a transport backend `T`:

```text
Surface<Role, T_file> ≈ Surface<Role, T_socket>
```

when both expose the same role-bounded operations and preserve the same
authority invariants. Code that evaluates a child should depend on the
role-shaped surface, not on the incidental fact that today's transport is a
file.

## Role Surfaces

### `Surface<Child>`

Allowed capabilities:

- read assigned campaign/node context
- read assigned runner request or equivalent child input
- read parent broadcast messages
- write child-to-parent messages for this child/runtime
- write this child/runtime's runner result
- write attempt-scoped evaluation artifacts
- emit child evaluation telemetry

Forbidden capabilities:

- mutate scheduler policy
- stage children
- select successor
- write parent continuation decisions
- write parent identity
- spawn recursive `prototype1-state`

### `Surface<Parent>`

Allowed capabilities include:

- read and validate active parent identity
- read and mutate scheduler state through parent transitions
- stage child nodes
- construct child admission environments
- observe child evidence
- select successor according to policy
- cross into `Parent<Retired>` during handoff

Parent authority is not campaign-global ambient authority. It belongs to a
specific admitted parent execution path in a specific state.

### `Surface<Successor>`

Allowed capabilities are narrower than parent authority until handoff
validation succeeds:

- validate predecessor handoff evidence
- acknowledge successor readiness
- enter the normal parent path only after the handoff boundary admits it

## Execution Path Admission

A runtime process does not have a role merely because it is a `ploke-eval`
binary. It gains access to a role-shaped surface only by entering an admitted
execution path.

Examples:

- first parent path: active checkout, artifact-carried parent identity, and
  scheduler node agree
- child path: child admission evidence binds the process to one campaign, node,
  runtime id, and child-scoped communication surface
- successor path: predecessor handoff evidence validates before the successor
  enters the normal parent path

After admission, the execution path should carry the role/state proof:

```text
ExecutionPath<Child, Starting> -> Child<Starting>
ExecutionPath<Parent, Ready> -> Parent<Ready>
```

The `Surface<Role>` exposed to that path is the only way ordinary runtime code
should gain read/write access to external campaign state, channel endpoints, or
result locations.

## Design Obligations

When adding a new parent/child communication path, check:

1. Which role owns mutable write authority?
2. Is the writable location unique per child/runtime when fanout is possible?
3. Is shared data read-only from child code?
4. Is the durable record a projection of a typed transition?
5. Can the same capability be expressed over another transport without changing
   role authority?
6. Does any helper take raw paths where it should take `Surface<Role>` or
   `Channel<Role<State>, T>`?

## Telemetry Consequence

Provider attempts should inherit their runtime context from the admitted
surface:

```text
Surface<Role> -> tracing span -> provider_attempt event
```

The provider attempt itself should not become a new authority record. It is
telemetry projected from a runtime already admitted through the authority
boundary.

For parent telemetry, use the parent identity and scheduler/node context already
held by the parent path. For child telemetry, use the admitted child surface.
The watcher should project enriched events; it should not recover authority by
parsing rendered logs.

## Current Implementation Mapping

Current code already has several pieces of this model:

- `Child<State>` in `crates/ploke-eval/src/cli/prototype1_state/child.rs`
- `Parent<State>` in `crates/ploke-eval/src/cli/prototype1_state/parent.rs`
- transport/channel sketches in
  `crates/ploke-eval/src/cli/prototype1_state/channel.rs`
- parent identity in `.ploke/prototype1/parent_identity.json`
- child/successor invocation records as today's serialized admission surface
- shared campaign state under `~/.ploke-eval/campaigns/<campaign>/prototype1/`

The main gap is that many execution paths still pass raw paths or loosely
populated environment variables instead of an admitted `Surface<Role>`.
