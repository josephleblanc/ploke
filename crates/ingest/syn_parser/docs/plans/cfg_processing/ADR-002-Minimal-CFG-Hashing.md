# ADR-002: Minimal CFG Hashing for Synthetic ID Uniqueness

## Status
ACCEPTED

## Context
The current `syn_parser` implementation (pre-ADR-001) does not incorporate `#[cfg(...)]` attribute information into the generation of `NodeId::Synthetic` or `TypeId::Synthetic`. This leads to duplicate IDs for code items or type usages that differ only by their conditional compilation flags, violating the requirement for unique identifiers within a parsed graph.

While ADR-001 outlines a comprehensive strategy for full CFG processing, including database storage and RAG filtering, there's a need for an immediate, minimal change to guarantee synthetic ID uniqueness as a foundational step. This avoids breaking assumptions in downstream processing or testing that rely on unique IDs.

## Decision
Implement the core Phase 2 logic outlined in ADR-001 immediately to ensure unique synthetic IDs, deferring Phase 3 calculations, database storage, and RAG filtering logic. This involves:

1.  **Dependency:** Add `cfg-expr = { version = "0.15", features = ["serde", "hash"] }` to `syn_parser/Cargo.toml`.
2.  **Core ID Generation:** Modify `ploke_core::NodeId::generate_synthetic` and `ploke_core::TypeId::generate_synthetic` to accept an optional byte slice (`cfg_bytes: Option<&[u8]>`). If provided, these bytes will be appended to the data hashed for the UUIDv5 generation.
3.  **CFG Helpers:** Implement helper functions in `syn_parser::parser::visitor`:
    *   `attribute_processing::parse_and_combine_cfgs_from_attrs`: Parses `#[cfg]` attributes into `Option<cfg_expr::Expression>`, combining multiples deterministically.
    *   `code_visitor::combine_cfgs`: Combines inherited scope CFG with item CFG into a new `Option<Expression>`.
    *   `code_visitor::hash_expression`: Hashes an `Option<&Expression>` into `Option<Vec<u8>>` using `ploke_core::byte_hasher::ByteHasher`.
4.  **Visitor State:** Add `current_scope_cfg: Option<Expression>` and `cfg_stack: Vec<Option<Expression>>` to `VisitorState` to track the inherited CFG context.
5.  **Visitor Logic:** Modify the visitor (`code_visitor`, `mod`, `type_processing`):
    *   Initialize `current_scope_cfg` based on file-level attributes.
    *   For each visited item (function, struct, enum, etc.) or type usage:
        *   Calculate its *provisional effective CFG* by combining the `current_scope_cfg` with the item's own parsed `cfg` attributes using `combine_cfgs`.
        *   Hash this `provisional_effective_cfg` using `hash_expression` to get `cfg_bytes`.
        *   Pass `cfg_bytes.as_deref()` to the appropriate `generate_synthetic` function when creating the `NodeId` or `TypeId`.
        *   Manage the `current_scope_cfg` and `cfg_stack` correctly when entering/leaving scopes that define CFG context (modules, structs, enums, impls).

## Consequences
- Positive:
    - **Guarantees unique `NodeId::Synthetic` and `TypeId::Synthetic`** for items/types differing only by `cfg` attributes.
    - Directly implements the necessary Phase 2 foundation for the full CFG processing plan (ADR-001).
    - Minimizes immediate code churn compared to the full plan by focusing only on ID generation.
    - Allows tests for ID uniqueness (like `test_cfg_*_conflation`) to pass correctly.
- Negative:
    - Introduces the `cfg-expr` dependency.
    - Adds complexity to visitor state management and ID generation calls.
    - Does not provide the full benefits of CFG processing (filtering, resolved ID uniqueness) yet.
- Neutral:
    - Requires careful implementation of CFG context tracking using the stack.
    - The calculated `provisional_effective_cfg` is used for hashing but not explicitly stored on the nodes in this minimal step.

## Compliance
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:
- C-GOOD-ERR: Error handling during `cfg_expr::Expression::parse` should be robust.
- C-DEBUG: `cfg_expr::Expression` derives `Debug`.
- C-SERDE: Relies on `cfg-expr` deriving `Serialize`, `Deserialize`.
- C-HASH: Relies on `cfg-expr` deriving `Hash`.

[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items: N/A
