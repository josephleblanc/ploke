# ADR-010: Apply File-Level `cfg` Attributes to Contained Items

## Status
PROPOSED

## Context
Rust allows applying configuration attributes at the file level using inner attributes (`#![cfg(...)]`). The `CodeVisitor` currently processes these attributes during Phase 2 parsing (`analyze_file_phase2`) and stores them correctly on the corresponding `ModuleNode` within the `ModuleDef::FileBased::file_attrs` field.

Tests (`test_file_level_cfg_struct_node_id_disambiguation`) confirm that:
1.  File-level attributes *are* captured on the `ModuleNode`.
2.  Identically named items defined in separate files gated by mutually exclusive file-level `cfg` attributes (e.g., `FileGatedStruct` in `cfg_file_a.rs` vs. `cfg_file_not_a.rs`) currently receive **distinct `NodeId`s**. This distinction arises *indirectly* because `NodeId::generate_synthetic` includes the file path in its hash input, and the files have different paths.

However, the `cfg` context derived from the file-level attributes is **not** currently propagated or associated with the individual item nodes (e.g., `StructNode`, `FunctionNode`) defined *within* that file. To understand the full `cfg` context of an item defined in a file with `#![cfg(...)]`, a consumer must currently:
1.  Find the item's node.
2.  Find the `Contains` relation pointing to the item.
3.  Identify the source `ModuleNode`.
4.  Check if the `ModuleNode` is `FileBased` and access its `file_attrs`.

This requires extra traversal and makes it harder to reason about an item's configuration context directly from its node data.

## Decision
Propose that the `cfg` context established by file-level attributes (`#![cfg(...)]`) **should be associated directly with the item nodes** (e.g., `StructNode`, `FunctionNode`, `EnumNode`, etc.) defined within that file.

This ADR **does not specify the exact mechanism** for achieving this association. Potential approaches include:
- Modifying the `CodeVisitor` (Phase 2) to pass down the file-level `cfg` context and merge it into the `attributes` list of each visited item node.
- Performing this association in a later graph enhancement phase (e.g., Phase 3 or a dedicated step) by traversing the graph and propagating attributes from file-level `ModuleNode`s to their contained items.

The choice of mechanism requires further investigation into complexity, performance implications, and potential interactions with item-level `cfg` attributes (see ADR-009) and should be detailed before this ADR is accepted.

## Consequences
- **Positive:**
    - Item nodes would eventually contain their complete `cfg` context (combining file-level and item-level attributes), simplifying downstream analysis and queries.
    - Improves the semantic accuracy and self-contained nature of item nodes in the `CodeGraph`.
- **Negative:**
    - Increases complexity, either in the Phase 2 visitor or in a subsequent processing phase.
    - Requires defining clear rules for how file-level and item-level `cfg` attributes should be combined or represented on the item node.
- **Neutral:**
    - Current tests confirm file-level attributes are captured on `ModuleNode`.
    - Current tests confirm item `NodeId`s are distinct due to file path differences, which remains unchanged by this proposal (as it focuses on attribute propagation, not ID generation).

## Compliance
[PROPOSED_ARCH_V3.md](/PROPOSED_ARCH_V3.md) Items:
- Phase 2/3: Proposes enhancing the information associated with nodes.
[IDIOMATIC_RUST.md](ai_workflow/AI_Always_Instructions/IDIOMATIC_RUST.md) Sections:
- C-CONTEXT (Implicit): Aims to provide better contextual information directly on nodes.
- C-FLEXIBILITY: Supports understanding code under diverse configurations.
[CONVENTIONS.md](ai_workflow/AI_Always_Instructions/CONVENTIONS.md) Items: N/A
