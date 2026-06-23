# Typestate

Prototype 1 uses typestate to make the parent-turn order reviewable. Read this
page as an orientation; it intentionally quotes only the carrier shapes that
clarify the core model. For source-backed invariant details, use
[Typestate Invariants](typestate-invariants.md).

Short thesis:

> Prototype 1 is a typed local succession protocol over runtime/artifact
> coordinates. It is not a flat rerun script and not global consensus.

## Core model

The outer carrier is a product of independently meaningful axes, not one large
phase enum.

```rust,ignore
{{#include ../../src/cli/prototype1_state/typestate/runtime.rs:prototype1_runtime_product}}
```

Concrete authority islands use role carriers such as `Parent<S>` rather than
stringly phase names.

```rust,ignore
{{#include ../../src/cli/prototype1_state/parent.rs:prototype1_parent_struct}}
```

R-state aliases name useful points in that product space. The aliases make
later edge signatures demand the expected shape, while the edge bodies still
have to check files, provider outputs, profile policy, History heads, and child
messages.

## Strong local carriers

The strongest local guarantees are carried by values that cannot normally be
minted from raw paths or strings:

- `Parent<Unchecked> -> Parent<Checked> -> Parent<Ready>` for parent startup;
- `Open<M> -> Locked<M> -> Received<M>` for message boxes;
- `Block<Open> -> Block<Sealed>` plus `FsBlockStore::append` for local History;
- C1-C5 child-attempt states for one planned child.

For the concrete child-plan message box, see
[Zero Admission Flow](child-plan-authority/zero-admission-flow.md).

## What typestate does not prove

Typestate does not by itself prove:

- model judgment correctness;
- global process uniqueness or distributed consensus;
- that optional facts in transitional context have been semantically checked;
- that projection files are authority-bearing;
- that an operator used the right checkout binary.

Those facts are enforced, where currently implemented, by edge bodies, admitted
profile checks, sealed History, child channel evidence, and operator discipline.

## Practical reading order

| Need | Read |
| --- | --- |
| Operator command/phase view | [Operator Map](operator-map.md), [Phase Map](phase-map.md) |
| Enforcement and gaps | [Typestate Invariants](typestate-invariants.md) |
| Child-plan message-box mechanics | [Zero Admission Flow](child-plan-authority/zero-admission-flow.md) |
| Anchor index | [Development Source Map](../development/source-map.md) |
| Long-form working notes | [Typestate Notebook](../appendices/prototype1-typestate-notebook.md) |

## Status labels

Use the book-wide labels from [Status Labels](../orientation/status-labels.md):
implemented, partially implemented, intended, and not claimed.

In particular, avoid stronger claims than the current code makes. History is
local, lineage-scoped, and tamper-evident; it is not a global authority system.
