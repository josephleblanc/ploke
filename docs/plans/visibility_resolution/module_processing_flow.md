# Module Processing Flow in CodeVisitor

## Overview

This document details the current module processing flow within the `CodeVisitor` in `syn_parser`. It outlines how modules are identified, visited, and represented in the `CodeGraph`. This documentation serves as a baseline for understanding the impact of the module path tracking refactor.

## Current Flow

1.  **`visit_item_mod` Entry Point:** The `visit_item_mod` method in `CodeVisitor` is called when the visitor encounters a `syn::ItemMod` (a module definition) in the AST.

2.  **Module Node Creation:** Inside `visit_item_mod`, a `ModuleNode` is created. This node represents the module in the `CodeGraph`.

3.  **Recursive Traversal:** The `visit_item_mod` method recursively calls `visit` on the items within the module (functions, structs, enums, etc.). This ensures that all code within the module is processed.

4.  **Item Processing:** As the visitor traverses the module's items, the appropriate `visit_item_*` methods are called (e.g., `visit_item_fn` for functions, `visit_item_struct` for structs). These methods create nodes for each item and add them to the `CodeGraph`.

5.  **Relationship Creation:** Within the `visit_item_*` methods, relationships are created between the items and their parent module using the `Relation` struct and `RelationKind::Contains`.

6.  **Module Hierarchy:** The module hierarchy is implicitly represented by the parent-child relationships between `ModuleNode`s. However, the current implementation *does not* explicitly track the full path to each module.

## Key Observations

*   **No Explicit Path Tracking:** The current implementation does not store the full module path (e.g., `crate::module1::module2`) for each `ModuleNode`. This limits the ability to accurately determine the scope and visibility of code elements.
*   **Recursive Traversal:** The recursive nature of the `visit_item_mod` method is crucial for processing nested modules.
*   **Relationship-Based Hierarchy:** The module hierarchy is maintained through the `Relation::Contains` relationships.

## Impact of Module Path Tracking Refactor

The refactor will introduce the following changes:

1.  **`current_module_path` in `VisitorState`:** A `Vec<String>` will be added to the `VisitorState` to track the current module path during traversal.
2.  **Path Updates in `visit_item_mod`:** The `visit_item_mod` method will be modified to push the current module's name onto the `current_module_path` before recursively visiting its items and pop the name after the recursive call.
3.  **`path` Field in `ModuleNode`:** The `ModuleNode` struct will be updated to include a `path: Vec<String>` field to store the full module path.

## Future Considerations

*   **Path Normalization:** Consider normalizing module paths to handle cases with relative imports or aliases.
*   **Error Handling:** Implement robust error handling to gracefully handle malformed module structures.
*   **Performance:** Monitor the performance impact of adding the module path tracking and optimize as needed.
