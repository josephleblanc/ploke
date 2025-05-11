# ADR-014: Strict Encapsulation for Typed Node Identifiers

## Status
ACCEPTED 2025-05-02

## Context
ADR-013 introduced typed node identifiers (e.g., `ModuleNodeId`, `FunctionNodeId`) using newtypes around the base `NodeId` to improve compile-time safety. The initial implementation proposed using `pub(crate)` visibility for escape hatches (`.into_inner()`, `.as_inner()`) to allow necessary internal access to the base `NodeId` (for hashing, indexing, error reporting).

However, `pub(crate)` still allows *any* code within the `syn_parser` crate to potentially bypass the type safety by converting a typed ID back to a raw `NodeId`. This doesn't fully align with the project's goal of leveraging the type system to make invalid states unrepresentable ("program as proof") and prevent accidental misuse even *within* the crate's internal implementation. A stricter approach is desired to maximize compile-time guarantees.

## Decision
We will implement typed node identifiers with strict encapsulation using a private module structure:

1.  **Private Module:** All typed ID newtype structs (e.g., `StructNodeId(NodeId)`, `FieldNodeId(NodeId)`) will be defined within a dedicated private module (e.g., `crate::parser::nodes::ids::internal`). The inner `NodeId` field of these structs will be private to this module.
2.  **Restricted Constructors:** Constructors for these typed IDs will be defined within the private module and scoped appropriately (e.g., `pub(in crate::parser::nodes)`) to ensure only designated code (like node constructors) can create them. There will be no public `::new(NodeId)` constructor.
3.  **No Public Escape Hatches:** The private module will **not** expose public or `pub(crate)` `.into_inner()` or `.as_inner()` methods. Access to the base `NodeId` will be strictly controlled within the private module.
4.  **Internal Trait Implementations:** Essential traits requiring access to the inner `NodeId` (e.g., `Hash`, `Eq`, `Ord`, potentially an internal `HasBaseId` trait) will be implemented *inside* the private module. Standard traits like `Debug`, `Clone`, `Copy` can still be derived.
5.  **Public Re-exports:** Only the typed ID types themselves (e.g., `StructNodeId`) and necessary public traits (like marker traits or `TypedNodeIdGet`) will be re-exported (`pub use`) from the containing module (e.g., `crate::parser::nodes::ids`).
6.  **Heterogeneous Graph Key:** To handle graph storage requiring a common key type (e.g., `HashMap<Key, NodeWrapper>`) without exposing the base `NodeId`, a public enum `AnyNodeId { Struct(StructNodeId), Function(FunctionNodeId), ... }` will be defined. This enum will derive `Hash`, `Eq`, etc., and serve as the key type for heterogeneous collections, decoupling the graph storage implementation from the private ID module.
7.  **Trait Implementations (Markers, `TypedNodeIdGet`):** Public marker traits (`PrimaryNodeIdTrait`, etc.) and the `TypedNodeIdGet` trait will be defined publicly (or `pub(crate)`). Their implementations for the specific ID types will reside within the private module.
8.  **Macro Support:** A procedural or declarative macro will likely be used within the private module to manage the boilerplate of defining the ID structs and implementing the necessary internal traits.

## Consequences
- **Positive:**
    - **Maximum Compile-Time Safety:** Prevents *any* code outside the designated private module from converting a typed ID back to a raw `NodeId`, eliminating a class of potential internal errors. Makes invalid ID usage unrepresentable.
    - **Clear Encapsulation:** Establishes a very strong boundary around ID implementation details.
    - **Improved Maintainability (Safety):** Reduces the risk of introducing bugs during refactoring by enforcing strict type constraints; the compiler catches more errors.
    - **Conceptual Clarity (Set Theory Alignment):** The use of distinct typed IDs, marker traits (`PrimaryNodeIdTrait`, `SecondaryNodeIdTrait`, etc.), and category enums (`AnyNodeId`) provides a natural way to model and reason about different *sets* of nodes within the code structure, aligning well with set-theoretic concepts and improving the conceptual integrity of the design.
    - **Foundation for Abstraction Layers:** Provides a type-safe foundation for building higher-level abstractions. The categorized typed IDs allow defining transformations or mappings (e.g., from the syntactic graph layer to a logical/semantic layer) that are enforced by the type system, facilitating the creation of a multi-layered graph architecture with strong guarantees.
    - **Alignment with Goals:** Better reflects the "program as proof" philosophy.
- **Negative:**
    - **Increased Structural Complexity:** Requires careful management of the private module, its internal traits, restricted constructors, and the `AnyNodeId` enum.
    *   **`AnyNodeId` Maintenance:** The `AnyNodeId` enum needs to be defined and updated whenever new primary node types are added.
    *   **Potential Boilerplate:** Requires careful macro design to manage the implementation details within the private module.
    *   **Slight Runtime Overhead:** Using `AnyNodeId` as a hash key involves an extra level of indirection compared to using the raw `NodeId` directly (though likely negligible).
- **Neutral:**
    *   Requires a shift in how graph storage and access are implemented (using `AnyNodeId` keys).

## Compliance
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items:
- Section 5: Component Details (enforces stricter data hiding within components)
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:
- C-NEWTYPE: Newtypes provide static distinctions.
- C-NEWTYPE-HIDE: Newtypes encapsulate implementation details (taken to a stricter level).
- C-STRUCT-PRIVATE: Structs have private fields (applied rigorously to the newtype).
- Type safety: Leverages the type system effectively.
- Dependability: Crate is unlikely to do the wrong thing (by preventing internal misuse).
[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A
[WORKING_AGREEMENT.md](ai_workflow/AI_Always_Instructions/WORKING_AGREEMENT.md): Aligns with the core goal of idiomatic Rust and leveraging the type system for static guarantees.
