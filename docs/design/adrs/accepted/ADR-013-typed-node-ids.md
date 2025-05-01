# ADR-013: Using Newtypes for Typed Node Identifiers

## Status
ACCEPTED

## Context
The code graph contains various types of nodes (Modules, Functions, Structs, Fields, Imports, etc.), each identified by a `NodeId` (currently a wrapper around `Uuid`). Relations connect these nodes. Using a raw `NodeId` for all connections makes it possible to create semantically invalid relations at runtime (e.g., a `Field` relation originating from a `FunctionNodeId` instead of a `StructNodeId` or `VariantNodeId`). We need stronger type safety to prevent such invalid connections at compile time.

## Decision
We will use the newtype pattern to create distinct types for identifiers corresponding to different kinds of graph nodes. A macro (`define_node_id_wrapper!`) will be used to reduce boilerplate for creating these wrappers around the base `NodeId`. Examples include:
- `struct ModuleNodeId(NodeId);`
- `struct FunctionNodeId(NodeId);`
- `struct StructNodeId(NodeId);`
- `struct FieldNodeId(NodeId);`
- etc.

These typed IDs will be used in the fields of node structs (where appropriate) and crucially, within the definitions of relations (e.g., in a future `SyntacticRelation` enum or similar structure) to specify the expected types of the source and target nodes for each relation kind.

## Consequences
- **Positive:**
    - **Compile-Time Safety:** Prevents the creation of relations between incorrect types of nodes. Type errors occur if, for example, a `FunctionNodeId` is passed where a `ModuleNodeId` is expected in a relation constructor.
    *   **Improved Code Clarity:** Function signatures and struct definitions become more self-documenting, clearly indicating the specific type of ID expected.
    *   **Reduced Runtime Errors:** Eliminates a class of potential runtime logic errors related to mismatched node types in relations.
- **Negative:**
    - **Boilerplate:** Requires defining numerous newtype structs, although the macro mitigates this significantly.
    - **Conversion Overhead:** Requires explicit conversions (e.g., using `.into()`, `.as_inner()`, or `From`/`Into` implementations) when needing to access the underlying `NodeId` or convert between the wrapper and the base ID.
- **Neutral:**
    - Requires consistent use throughout the codebase where node IDs are handled.

## Compliance
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items:
- Section 5: Component Details (affects data structures like `CodeGraph`, `FunctionNode`, etc.)
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:
- C-NEWTYPE: Newtypes provide static distinctions.
- Type safety: Leverages the type system effectively.
[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A
