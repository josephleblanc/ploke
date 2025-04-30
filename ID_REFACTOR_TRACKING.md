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
