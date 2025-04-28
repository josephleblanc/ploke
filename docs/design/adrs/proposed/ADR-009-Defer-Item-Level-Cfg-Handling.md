# ADR-009: Defer Handling of Item-Level `cfg` Attributes in Phase 2 ID Generation

## Status
PROPOSED

## Context
During Phase 2 parallel parsing, the `CodeVisitor` processes all Rust code, including items gated by `#[cfg(...)]` attributes, regardless of whether those conditions would be met in a specific build configuration.

The current `NodeId::generate_synthetic` function generates unique identifiers based on crate namespace, file path, relative module path, item name, item kind, and parent scope ID. However, it **does not** incorporate the item's own `#[cfg(...)]` attributes into the hash input.

As confirmed by tests (`test_cfg_struct_node_id_conflation`, `test_cfg_function_node_id_conflation`), this leads to **NodeId conflation**: identically named items within the same scope that differ only by mutually exclusive `cfg` attributes (e.g., `#[cfg(feature = "a")] struct Foo;` and `#[cfg(not(feature = "a"))] struct Foo;`) are assigned the **same `NodeId`**.

While the visitor creates distinct `StructNode` or `FunctionNode` instances for each `cfg` branch encountered, these distinct instances share the same identifier in the resulting `CodeGraph`.

## Decision
We will **defer** the implementation of incorporating item-level `#[cfg(...)]` attributes into the `NodeId::generate_synthetic` function for Phase 2.

The primary reasons for deferral are:
1.  **Complexity:** Reliably parsing, canonicalizing, and hashing the potentially complex expressions within `cfg` attributes (`all()`, `any()`, `not()`, specific target features, etc.) adds significant complexity to the visitor and ID generation logic.
2.  **Phase 2 Scope:** Phase 2 aims to capture the *potential* structure of the code. Since it parses all branches, generating distinct IDs for mutually exclusive items might provide limited immediate benefit, as the graph inherently contains nodes that wouldn't co-exist in a final compiled artifact. The conflation reflects this "all possibilities" nature of the Phase 2 graph.
3.  **Downstream Handling:** Resolving `cfg` conditions and filtering the graph based on a specific configuration is likely better suited for a later phase (e.g., Phase 3 or a dedicated analysis step) or handled by downstream query logic.

## Consequences
- **Positive:**
    - Avoids introducing significant complexity into the Phase 2 visitor and ID generation logic at this time.
    - Keeps Phase 2 focused on capturing the basic code structure and relationships.
    - Simplifies the current implementation and testing surface.
- **Negative:**
    - `NodeId`s do not uniquely identify items based on their `cfg` context. Two items differing only by `cfg` will share the same ID.
    - The Phase 2 `CodeGraph` contains duplicate node instances (with the same ID) for `cfg`-gated items.
    - Downstream consumers (Phase 3, queries, analysis tools) must be aware that a single `NodeId` might correspond to multiple `cfg`-gated definitions and may need to inspect node attributes or perform `cfg` evaluation themselves.
- **Neutral:**
    - Tests have been added (`test_cfg_*_node_id_conflation`) to explicitly verify and document the current conflation behavior, acting as a baseline.

## Compliance
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items:
- Phase 2: Acknowledges limitation in ID uniqueness regarding `cfg`.
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:
- C-VALIDATE: Arguably, not fully validating the `cfg` context during ID generation could be seen as a violation, but deferred for complexity reasons.
- C-NEWTYPE-HIDE: The `NodeId` currently hides the `cfg` distinction.
[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A
