# Runtime Authority

Prototype 1 authority is role-scoped, state-scoped, and lineage-scoped. A runtime does not gain parent authority merely because a `ploke-eval` binary is executing.

## Authority shape

The draft authority model names the boundary as:

```text
ExecutionPath<Role, State> + Surface<Role> + Transport
```

A runtime is admitted to a role/state-shaped execution path. Ordinary code should then use typed carriers and role-shaped surfaces rather than raw paths or ambient role strings.

## Role-bounded authority

For an admitted runtime/path pair:

```text
A(Runtime) ⊆ A(role(path))
```

A child may evaluate its assigned node and write child-shaped evidence. It may not stage children, select a successor, mutate parent identity, or make continuation decisions.

## State-bounded authority

For an admitted runtime/path pair:

```text
A(Runtime) ⊆ A(role(path), state(path)) ⊆ A(role(path))
```

The role gives an outer authority boundary; the state narrows it. A `Successor` before validation is not yet a ruling parent.

## Mutable surfaces

Mutable child surfaces should be scoped per child/runtime. Shared campaign state may be readable by many children, but child outputs, runner results, telemetry streams, and message buffers must not silently collapse into one shared mutable surface.

## Transition records

A durable record should be a projection of an allowed transition:

```text
Role<S> -> Role<S'> emits record(S -> S')
```

The record is evidence of a typed transition, not an arbitrary status write.

## Canonical sources

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- Draft source: `docs/workflow/evalnomicon/drafts/runtime/authority.md`
