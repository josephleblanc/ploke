# Trait-First Reification Note

Short restart-safe note on how to carry formal structure into Rust when the
underlying object is too important to leave as naming conventions inside large
implementation files.

## Core Position

When the conceptual framework distinguishes a universe of kinds or sorts, those
distinctions should usually appear in code first as **traits/algebras**, and
only second as concrete structs implementing those traits.

In other words:

```text
formal kind / algebra / contract
  -> trait family

concrete realized instance
  -> struct implementing that trait family
```

This is stronger than “use good names” and stronger than “use typestate in a
few places.” The goal is that the codebase itself encodes what counts as a
`Graph`, `Tree`, `Intervention`, `History`, or `Policy`, before any particular
implementation is even exercised.

## Why

This style gives us:

- compiler-visible contracts for the major semantic objects
- cleaner separation between ontology and backend detail
- the ability to have multiple concrete implementations of the same formal
  object
- better cold-restart recoverability, because the code shape carries the
  framework distinctions directly

The intended shape is:

```text
trait level:
  kinds of thing, admissible operations, associated sorts

struct level:
  one concrete realization of the trait-level object
```

## Implication For This Work

For the intervention / branching substrate, the default posture should be:

- `Surface`, `Intervention`, `Graph`, `Tree`, `History`, `Policy`,
  `Configuration`, `Event`, etc. are candidates for trait-level formal objects
- concrete runtime realizations like `Prototype1Graph` or
  `ToolTextIntervention<...>` should be structs implementing those contracts
- important family distinctions should be encoded with generics / associated
  types where possible
- erased/runtime-only data should be reserved for:
  - ids
  - persisted paths
  - timestamps
  - open-ended graph instances
  - backend-specific operational detail

This means the right split is **not**:

```text
one blob struct per subsystem
plus conventions and comments
```

It is closer to:

```text
formal algebra as traits
local lifecycle as indexed families
runtime graph instance as concrete struct
```

## Relation To The Existing Formal Drafts

This note sits on top of the existing formal work:

- [formal-procedure-notation.md](formal-procedure-notation.md)
- [type-state.md](type-state.md)
- [prototype-1-intervention-loop.md](prototype-1-intervention-loop.md)
- [framework-ext-01.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-01.md)
- [framework-ext-02.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-02.md)
- [framework-ext-03.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-03.md)
- [framework-ext-04.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/framework-ext-04.md)
- [isomorphic-code-01.md](/home/brasides/code/ploke/docs/workflow/evalnomicon/chat-history/isomorphic-code-01.md)

The main implementation lesson from those documents is:

```text
code should be a faithful refinement of the formal model
```

not merely “inspired by it.”

## CatColab Reference Point

The `CatColab` / `catlog` codebase is a useful concrete precedent for this
style. Relevant files:

- [`one/category.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/one/category.rs)
- [`one/functor.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/one/functor.rs)
- [`one/path.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/one/path.rs)
- [`dbl/category.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/dbl/category.rs)
- [`dbl/tree.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/dbl/tree.rs)
- [`dbl/theory.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/dbl/theory.rs)
- [`dbl/model.rs`](../../../../.symlinks/CatColab-symlink/packages/catlog/src/dbl/model.rs)

What is useful about that codebase for our purposes:

- trait families and associated types carry the formal sorts
- generic parameters encode meaningful family distinctions
- wrapper/path/tree objects encode local compositional structure
- macros reduce repetition once the trait algebra is fixed
- concrete graph/model/theory instances remain structs implementing those
  algebras

Important caution:

```text
type-level algebra != “push the whole runtime universe into the type system”
```

The lesson is to encode the **formal contract** at the trait level, while still
allowing concrete runtime graph instances to exist as data structures.

## Working Rule Of Thumb

When introducing a new major type, ask:

```text
Is this a formal kind of thing?
Or is it one concrete instance of a formal kind?
```

If it is a formal kind of thing, prefer:

- a trait
- associated sorts
- generic indices / markers where the distinction matters semantically

If it is a concrete realized thing, prefer:

- a struct implementing the trait-level contract

This is the default approach we want to favor for complex semantic structures
going forward, especially when cold-start recoverability and safety matter.
