# ADR-002: Minimal CFG Hashing for Synthetic ID Uniqueness

## Status
ACCEPTED

## Context
The current `syn_parser` implementation (pre-ADR-001) does not incorporate `#[cfg(...)]` attribute information into the generation of `NodeId::Synthetic` or `TypeId::Synthetic`. This leads to duplicate IDs for code items or type usages that differ only by their conditional compilation flags, violating the requirement for unique identifiers within a parsed graph.

While ADR-001 outlines a comprehensive strategy for full CFG processing, including database storage and RAG filtering, there's a need for an immediate, minimal change to guarantee synthetic ID uniqueness as a foundational step. This avoids breaking assumptions in downstream processing or testing that rely on unique IDs.

## Decision
Implement a minimal change to ensure unique synthetic IDs by incorporating CFG information directly into the hash, deferring semantic parsing and evaluation using `cfg-expr` until Phase 3 or later. This involves:

1.  **Dependency:** Remove `cfg-expr` and `target-lexicon` dependencies from `syn_parser` for Phase 2.
2.  **Node Storage:** Add a `cfgs: Vec<String>` field to relevant `*Node` structs in `src/parser/nodes.rs`. This field will store the raw string content extracted directly from the `#[cfg(...)]` attributes attached *only* to that specific item.
3.  **Core ID Generation:** Modify `ploke_core::NodeId::generate_synthetic` to accept an optional byte slice (`cfg_bytes: Option<&[u8]>`). If provided, these bytes (representing the hashed combined CFG strings) will be appended to the data hashed for the UUIDv5 generation. `TypeId::generate_synthetic` remains unchanged as it relies on the parent `NodeId`'s context.
4.  **CFG Helpers:** Implement/modify helper functions in `syn_parser::parser::visitor`:
    *   `attribute_processing::extract_cfg_strings`: Extracts raw strings from `#[cfg(...)]` attributes.
    *   `attribute_processing::extract_attributes` / `extract_file_level_attributes`: Ensure these filter out `cfg` attributes.
    *   `code_visitor::calculate_cfg_hash_bytes`: Takes a slice of raw CFG strings (`&[String]`), sorts them alphabetically, joins them into a single delimited string, hashes the bytes of the joined string using `ploke_core::byte_hasher::ByteHasher`, and returns `Option<Vec<u8>>`.
    *   Remove previous helpers related to `cfg-expr` (`parse_and_combine_cfgs_from_attrs`, `combine_cfgs`, `hash_expression`).
5.  **Visitor State:** Add `current_scope_cfgs: Vec<String>` and `cfg_stack: Vec<Vec<String>>` to `VisitorState` to track the inherited raw CFG strings.
6.  **Visitor Logic:** Modify the visitor (`code_visitor`, `mod`):
    *   Initialize `current_scope_cfgs` based on file-level raw CFG strings.
    *   For each visited item:
        *   Extract its own raw `item_cfgs` using `extract_cfg_strings`.
        *   Store `item_cfgs` in the node's `cfgs` field.
        *   Combine inherited `scope_cfgs` and `item_cfgs` into a `provisional_effective_cfgs: Vec<String>`.
        *   Calculate `cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs)`.
        *   Pass `cfg_bytes.as_deref()` to `NodeId::generate_synthetic` when creating the `NodeId`.
        *   Manage the `current_scope_cfgs` and `cfg_stack` correctly when entering/leaving scopes.

## Consequences
- Positive:
    - **Guarantees unique `NodeId::Synthetic` and `TypeId::Synthetic`** for items/types differing only by `cfg` attributes.
    - Directly implements the necessary Phase 2 foundation for the full CFG processing plan (ADR-001).
    - Minimizes immediate code churn compared to the full plan by focusing only on ID generation.
    - Allows tests for ID uniqueness (like `test_cfg_*_conflation`) to pass correctly by differentiating based on CFG attributes.
    - Avoids adding `cfg-expr` dependency overhead to Phase 2 parsing.
- Negative:
    - Adds complexity to visitor state management (tracking string vectors).
    - Does not provide the full benefits of CFG processing (filtering, resolved ID uniqueness, semantic understanding) yet.
    - **Limitation:** Does not guarantee unique IDs for items under semantically equivalent but syntactically different CFG conditions (e.g., `#[cfg(all(A, B))]` vs `#[cfg(all(B, A))]`). Hashing relies on the sorted raw strings.
- Neutral:
    - Requires careful implementation of CFG context tracking using the stack.
    - Stores raw CFG strings on nodes, deferring parsing cost.

## Compliance
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections: N/A (No direct impact beyond standard data structures)

[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items: N/A
