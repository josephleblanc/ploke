# ADR-024: Skip Executable-Path Traversal During Code Graph Construction

## Status
ACCEPTED

Git tracking: feature/debug-corpus 17443f68, a9effd02

## Context
Recent corpus triage exposed a recurring class of duplicate synthetic node ID
failures caused by local items defined inside executable paths:

- local `const` items in sibling `if` / `else` branches
- local `const` items in different `match` arms
- local `fn` items in sibling inner blocks
- similar method-local items nested inside executable control flow

These are valid Rust, because each branch or block introduces a distinct lexical
scope. However, the current `syn_parser` code graph builder does not model
`syn::Expr`-level control-flow scopes as first-class graph context. When local
items inside executable paths are traversed and assigned synthetic node IDs,
multiple branch-local items with the same name can collapse to the same ID and
fail graph validation.

We considered two broad approaches:

1. Introduce finer-grained executable-scope identity into node generation.
2. Stop creating graph nodes from executable-path local items for now.

The current product goal is to build a stable graph of top-level items,
associated items, relations, and retrievable code snippets. That goal depends
more heavily on modules, types, impls, traits, methods, and associated items
than on method-local executable-path items.

## Decision
Do not traverse inside executable method bodies during code graph construction.

Current policy:

- traverse modules, impl blocks, trait blocks, and associated item signatures
- create graph nodes for top-level and associated items
- do not descend into method bodies or trait default method bodies
- do not create graph nodes for local items defined inside executable paths

This is an intentional coarse-grained limitation. We are choosing graph
stability and invariant preservation over partial support for local executable
scopes.

If and when we decide to model `syn::Expr` and executable control-flow scopes as
part of the graph, we may revisit this decision and introduce explicit
branch/block scope handling.

## Consequences
- Positive:
  - eliminates a class of duplicate node ID failures caused by executable-path
    local items
  - keeps graph validation strict without weakening uniqueness invariants
  - preserves coverage for higher-value graph entities such as impls, traits,
    methods, and associated item signatures
  - keeps current parser behavior simpler until expression-level modeling is
    intentionally designed

- Negative:
  - local items inside executable paths are not represented in the code graph
  - downstream consumers cannot query or embed those local executable-path items
    as graph nodes
  - some previously added repros now document intentional non-coverage rather
    than a bug to be fixed structurally

- Neutral:
  - free-function executable-body traversal was already absent; this decision
    makes method behavior consistent with that limitation
  - revisiting this later will likely require explicit `syn::Expr`-scope design
    rather than another incremental node ID tweak

## Compliance
- Graph validity: preserves strict duplicate-node and relation uniqueness
  invariants
- Parser scope policy: executable-path local items are currently out of scope
  for graph construction
- Future extension: expression-level traversal should only be added alongside a
  deliberate scope model for branch/block identity
