# Typed ID Refactoring (ADR-014) Tracking

This document tracks the progress of refactoring the codebase to use strictly encapsulated typed IDs, aiming for static dispatch and eliminating raw `NodeId` usage outside the `ids::internal` module.

## Immediate Objectives

1.  [X] Define strictly private typed IDs (`ids::internal`).
2.  [X] Define category enums (`PrimaryNodeId`, `AssociatedItemId`, `AnyNodeId`) using macros.
3.  [X] Fix `base_id` visibility on category enums to `pub(super)`.
4.  [X] Enhance `define_internal_node_id!` macro to implement marker traits.
5.  [ ] Refactor `GraphNode` trait and implementations to use `any_id() -> AnyNodeId`.
6.  [ ] Refactor `GraphAccess` trait and implementations to use typed IDs / `AnyNodeId` and static dispatch.
7.  [ ] Refactor `SynParserError` to use `AnyNodeId`.
8.  [ ] Refactor remaining code (e.g., `resolve/`, `utils/logging.rs`) to eliminate escape hatch usage where possible.
9.  [ ] Document necessary escape hatches in `docs/design/escape_hatches/escape_hatches.md`.

## Condensed History

*   **ADR-013:** Introduced typed node IDs using newtypes. Initial implementation used `pub(crate)` escape hatches.
*   **ADR-014:** Decided on strict encapsulation using a private `ids::internal` module and `AnyNodeId` for heterogeneous keys, eliminating public/crate escape hatches.
*   Defined `define_internal_node_id!` macro for basic ID struct generation.
*   Defined `define_category_enum!` macro for `PrimaryNodeId`, `AssociatedItemId`.
*   Manually defined `AnyNodeId` and `From` impls.
*   Fixed `base_id` visibility in `define_category_enum!` macro. (This step)
*   Enhanced `define_internal_node_id!` to implement marker traits. (This step)

## Detailed TODO List

*   **`GraphNode` Refactor:**
    *   [ ] Change `GraphNode::id()` signature to `fn any_id(&self) -> AnyNodeId;` in `parser/graph/mod.rs`.
    *   [ ] Update all `impl GraphNode for ...` blocks in `parser/nodes/` files to match the new signature and return the correct `AnyNodeId` variant.
*   **`GraphAccess` Refactor:**
    *   [ ] Remove `find_node`, `find_node_checked`, `find_node_unique`.
    *   [ ] Update signatures of methods like `get_child_modules`, `get_item_module`, `find_containing_mod_id`, etc., to use typed IDs or `AnyNodeId`.
    *   [ ] Update internal logic of `GraphAccess` methods to use `match AnyNodeId` and typed comparisons, removing `.base_id()` calls.
    *   [ ] Implement `get_*_checked` methods using a helper function and `AnyNodeId` for errors.
*   **Error Handling:**
    *   [ ] Change `SynParserError::NotFound`, `DuplicateNode` to store `AnyNodeId`.
    *   [ ] Update error creation sites.
*   **Relation Accessors:**
    *   [ ] Evaluate necessity of `SyntacticRelation::source/target` returning `NodeId`.
    *   [ ] If needed, change visibility to `pub(crate)` and document in `escape_hatches.md`.
    *   [ ] If not needed, remove them and refactor callers.
*   **Remaining Escape Hatches:**
    *   [ ] Use `rg` to find remaining `.base_id()` calls outside `internal.rs`.
    *   [ ] Refactor call sites in `resolve/`, `utils/logging.rs`, etc., to use typed IDs or `AnyNodeId`.
*   **Documentation:**
    *   [ ] Update `escape_hatches.md` with justifications for any remaining necessary uses of `base_id()`.
    *   [ ] Add doc comments explaining the strict ID system.


Okay, let's dive into refactoring `module_tree.rs`.

**Context Recap & Style Reminder**

Before we triage, let's quickly reiterate our core style goals relevant to this task:

1.  **Typed IDs:** Replace raw `NodeId` with specific types like `ModuleNodeId`, `FunctionNodeId`, or category enums like `PrimaryNodeId`, `AnyNodeId` wherever appropriate. This is the primary driver of this refactor.
2.  **Type Safety:** Leverage the compiler to prevent errors by using the correct ID types in function signatures, struct fields, and relation definitions (`SyntacticRelation`). Use `TryFrom`/`try_into()` for safe downcasting between ID categories or from `AnyNodeId`.
3.  **Clear APIs:** Function signatures should clearly indicate the *kind* of ID expected.
4.  **Iterator Idioms:** Continue using iterator chains (`filter_map`, `find_map`, etc.) for processing collections like relations or modules, especially now that relations are more type-safe.
5.  **Error Handling:** Use `Result` with specific `ModuleTreeError` variants (or `SynParserError` where appropriate) for recoverable errors.
6.  **Static Dispatch:** The typed IDs facilitate static dispatch, which we prefer. Avoid `dyn GraphNode` where simpler alternatives using `AnyNodeId` or specific types exist.

**Refactoring Triage for `module_tree.rs`**

Here's a breakdown of the changes needed, categorized as requested:

**Category 1: Simple Updates (Direct Replacements & Type Adjustments)**

These functions/areas primarily need straightforward type substitutions (`NodeId` -> specific typed ID or `AnyNodeId`), adjustments to use methods on typed IDs (like `.into()` or `.as_inner()` *internally* if absolutely necessary, though prefer higher-level operations), or updates to match the already-typed `SyntacticRelation` enum.

## Go ahead with these changes:
*   **`ModuleTree` Struct Fields:**
    *   `root`: `NodeId` -> `ModuleNodeId`.
    *   `modules`: `HashMap<NodeId, ModuleNode>` -> `HashMap<ModuleNodeId, ModuleNode>`.
    *   `pending_imports`/`pending_exports`: `containing_mod_id: NodeId` -> `ModuleNodeId`. (The `ImportNode` itself is likely already updated).
    *   `path_index`: `HashMap<NodePath, NodeId>` -> `HashMap<NodePath, AnyNodeId>` (Since any item type can have a path).
    *   `external_path_attrs`: `HashMap<NodeId, PathBuf>` -> `HashMap<ModuleNodeId, PathBuf>`.
    *   `decl_index`: `HashMap<NodePath, NodeId>` -> `HashMap<NodePath, ModuleNodeId>` (Specifically indexes module declarations).
    *   `found_path_attrs`: `HashMap<NodeId, PathBuf>` -> `HashMap<ModuleNodeId, PathBuf>`.
    *   `pending_path_attrs`: `Option<Vec<NodeId>>` -> `Option<Vec<ModuleNodeId>>`.
    *   `relations_by_source`/`relations_by_target`: `HashMap<NodeId, Vec<usize>>` -> `HashMap<AnyNodeId, Vec<usize>>` (Relations connect any node type).
*   **`ResolvedItemInfo` Struct Fields:**
    *   `resolved_id`: `NodeId` -> `AnyNodeId` (Can resolve to a definition or an `ImportNode`).
    *   `definition_id` (in `InternalDefinition`): `NodeId` -> `AnyNodeId` (or potentially `PrimaryNodeId` if strictly enforced, but `AnyNodeId` is safer for now).
*   **`PruningResult` Struct Fields:**
    *   `pruned_module_ids`: `HashSet<NodeId>` -> `HashSet<ModuleNodeId>`.
    *   `pruned_item_ids`: `HashSet<NodeId>` -> `HashSet<AnyNodeId>` (Items contained within pruned modules can be of any type).
*   **Function Signatures & Simple Internal Logic:**
    *   `new_from_root`: Takes `&ModuleNode`, sets `root: ModuleNodeId`.
    *   `root()`: Returns `ModuleNodeId`.
    *   `modules()`: Returns `&HashMap<ModuleNodeId, ModuleNode>`.
    *   `get_relations_from/to`, `get_iter_relations_from/to`, `get_all_relations_from/to`: Take `&NodeId` -> `&AnyNodeId`. Internal map keys are `AnyNodeId`.
    *   `path_index()`: Returns `&HashMap<NodePath, AnyNodeId>`.
    *   `pending_imports/exports()`: Return types are fine (contain typed IDs).
    *   `get_root_module`: Uses `self.root` (`ModuleNodeId`).
    *   `get_module_checked`: Takes `&ModuleNodeId`.
    *   `resolve_pending_path_attrs`: Uses `ModuleNodeId`.
    *   `find_declaring_file_dir`: Takes `ModuleNodeId`.
    *   `resolve_path_for_module`: Takes `ModuleNodeId`.
    *   `process_path_attributes`: Uses `ModuleNodeId`. Relation creation needs `ModuleNodeId`.
    *   **Logging Functions** (`log_relation_verbose`, `log_node_id_verbose`, etc.): Update to accept appropriate typed IDs or `AnyNodeId` and format them (using their `Display` impls).

## Do not change these yet:
* Do not change yet, needs care:
    *   `is_accessible`: Takes `ModuleNodeId`. Internal comparisons need care.
    *   `get_parent_module_id`: Takes/returns `ModuleNodeId`. Internal comparisons need care.
    *   `get_effective_visibility`: Takes `ModuleNodeId`. Internal comparisons need care.
    *   `find_custom_path_target`: Takes/returns `ModuleNodeId`. Internal relation access is typed.
    *   `add_reexport_checked`: Takes `target_node_id: NodeId` -> `AnyNodeId`. `reexport_index` value is `AnyNodeId`.
    *   `link_mods_syntactic`: Relation creation needs `ModuleNodeId`. Comparison `module.id() != *root_id.as_inner()` needs care (compare `ModuleNodeId` or base `NodeId`).
    *   `add_relation/checked`, `extend_relations`: Take `TreeRelation` (already typed). Internal map keys are `AnyNodeId`.

**Category 2: Fundamental Improvements (Leveraging Typed IDs/Categories)**

These functions can likely be refactored to be safer, clearer, or more efficient by using `AnyNodeId`, category enums (`PrimaryNodeId`), marker traits, or `TryFrom`.

*   **`add_module`:** While partly Category 1, the logic for indexing paths needs to correctly handle inserting `AnyNodeId` into `path_index` and `ModuleNodeId` into `decl_index`.
*   **`find_defining_file_path_ref_seq`:** Takes `NodeId` -> `AnyNodeId`. The logic finding parent relations (`get_relations_to`) needs `AnyNodeId`. Comparisons between module IDs should use `ModuleNodeId`.
*   **`shortest_public_path`:**
    *   Input: Takes `NodeId` -> `PrimaryNodeId` (or maybe `AnyNodeId` if we need paths to non-primary items?). `PrimaryNodeId` seems most logical for finding *public* paths to defined items.
    *   Internal Lookups: Replace `graph.find_node_unique(node_id)` with potentially more direct lookups if the type is known, or use `graph.get_node(&AnyNodeId)` if using the `HashMap<AnyNodeId, GraphNodeWrapper>` approach in `CodeGraph`.
    *   Relation Traversal: Use typed relations and `TryFrom` where applicable.
    *   Return Value: `ResolvedItemInfo` needs `AnyNodeId` as discussed.
    *   Visibility Checks: Can potentially use `target_node.visibility()` more directly instead of complex lookups if the node type is known or retrieved safely.
*   **`explore_up_via_containment`:** Takes `ModuleNodeId`. Uses `get_relations_to` (needs `AnyNodeId`). Needs `is_accessible_from` helper.
*   **`explore_up_via_reexports`:** Takes `ModuleNodeId`. Uses `get_relations_to/from` (needs `AnyNodeId`). Needs `graph.get_import_checked` (takes `ImportNodeId`). Logic can use `TryFrom` to ensure the relation target is an `ImportNodeId`.
*   **`resolve_visibility`:** Takes `&T: GraphNode`. Can potentially take `AnyNodeId` and use `graph.get_node()` and then `match` on the `GraphNodeWrapper` to get the specific node and its visibility/parent info more safely.
*   **`process_export_rels` / `resolve_single_export`:** These deal with `ImportNode` and resolving paths. They need to use `ImportNodeId` and potentially `resolve_path_relative_to` needs to return `Result<AnyNodeId, ...>`. The creation of the `ReExports` relation needs the correct typed IDs (`ImportNodeId` -> `AnyNodeId`).
*   **`resolve_path_relative_to`:** Takes `ModuleNodeId`. Returns `NodeId` -> `Result<AnyNodeId, ModuleTreeError>`. Needs to handle lookups (`get_relations_from`, potentially `path_index.get`) using appropriate IDs (`AnyNodeId`, `ModuleNodeId`). Can use `TryFrom` when expecting a module ID from a lookup result.
*   **`update_path_index_for_custom_paths`:** Logic manipulating `path_index` needs to use `AnyNodeId` for values.
*   **`prune_unlinked_file_modules`:** The collection `all_prunable_item_ids` should be `HashSet<AnyNodeId>`. Logic checking relations and pruning indices needs to use `AnyNodeId`.

**Category 3: Obsolete/Removal**

Functions that might become redundant or significantly simpler.

*   **`is_part_of_reexport_chain`:** This logic might be implicitly handled or simplified within the refactored `shortest_public_path` or re-export processing, potentially making this specific helper unnecessary. (Needs verification after refactoring).
*   **Potentially some `graph.find_node_unique` calls:** If the calling context already knows the expected *type* of the node ID it has, it might be replaceable with a type-specific getter (like `graph.get_module_checked`) or a direct lookup in the proposed `nodes: HashMap<AnyNodeId, GraphNodeWrapper>` followed by a `TryFrom` or `match`. The need for a generic `find_node_unique` that searches *all* node types might decrease.

**Summary Plan:**

1.  **Apply Category 1 Changes:** Start with the straightforward type replacements in struct fields, function signatures, and simple variable usage. Update `HashMap` key/value types. Adjust basic comparisons. Update logging calls.
2.  **Refactor Category 2 Functions:** Tackle the more complex functions one by one, focusing on:
    *   Using `AnyNodeId` for generic ID parameters/variables.
    *   Using specific typed IDs (`ModuleNodeId`, `ImportNodeId`, etc.) where the type is known or expected.
    *   Employing `TryFrom`/`try_into()` for safe conversions between `AnyNodeId` and specific/category IDs.
    *   Leveraging the typed `SyntacticRelation` variants.
    *   Simplifying lookups if using the `HashMap<AnyNodeId, GraphNodeWrapper>` pattern.
    *   Ensuring `shortest_public_path` and path resolution logic correctly handle and return typed IDs/`AnyNodeId`.
3.  **Review and Remove Category 3:** After refactoring, identify and remove any code that has become truly obsolete.
4.  **Testing:** Thoroughly test the refactored `ModuleTree` logic, paying close attention to path resolution, visibility checks, and `shortest_public_path`.

This triage provides a roadmap for methodically updating `module_tree.rs` to align with the new typed ID system and our established coding style.
