# Prototype 1 Runtime Loop

Prototype 1 is better described as a runtime succession loop than as a simple rerun of one patched checkout.

The key distinction is that changing an artifact does not change the semantics of an already-running parent binary. A descendant artifact must be built into a descendant runtime before that descendant can evaluate itself under the modified surface.

## Current loop shape

```text
active parent runtime
  -> synthesize descendant candidates
  -> realize candidate artifact state
  -> build descendant binary
  -> spawn descendant child runtime
  -> child evaluates itself and records evidence
  -> parent observes child evidence
  -> parent applies selection policy
  -> selected successor artifact is installed into the active checkout
  -> successor runtime validates handoff evidence
  -> successor becomes the next parent if admission succeeds
  -> old parent exits
```

## Runtime roles

- **Parent:** owns the current lineage authority for a bounded generation. It can synthesize candidates, realize child artifacts, observe child evidence, select a successor, and perform the handoff boundary.
- **Child:** evaluates one assigned candidate. It can write child-shaped evidence, but it cannot promote itself.
- **Successor:** a fresh runtime hydrated from the selected artifact. It becomes the next parent only after validating predecessor handoff material and admission checks.

## Why this is not a flat eval loop

Prototype 1 has at least two state axes:

- artifact state: files/checkouts/surfaces that can hydrate a runtime
- runtime state: an executing process with compiled semantics and role/state authority

A parent can operate over an artifact surface, but it cannot fully evaluate the semantics of a descendant binary from inside the old binary. The child runtime exists to produce evidence from the descendant operational environment.

## Canonical sources

- `crates/ploke-eval/src/cli/prototype1_state/mod.rs`
- `crates/ploke-eval/src/cli/prototype1_state/history.rs`
- Draft source: `docs/workflow/evalnomicon/drafts/runtime/loop.md`
