# Overview: Phase 1 -> Phase 2 -> return values: Analyzing TypeId conflation for `T`

## **1. Phase 1: File Discovery (`discovery.rs`)**

*   **Goal:** Identify all relevant Rust source files within the target crates and gather basic crate metadata.
*   **Process:**
    *   `run_discovery_phase` takes target crate root paths.
    *   For each crate:
        *   It finds and parses `Cargo.toml` to get the crate `name` and `version`.
        *   It calls `derive_crate_namespace` which generates a stable UUIDv5 namespace for this specific crate name and version, using `PROJECT_NAMESPACE_UUID` as the base. This ensures the *same crate version* gets the same namespace across runs.
        *   It uses `walkdir` to find *all* `.rs` files within the crate's `src` directory. This is purely based on file extension and location; it does **not** evaluate `#![cfg]` attributes at this stage.
        *   It performs a basic scan (`scan_for_mods`) of `lib.rs`/`main.rs` for `mod name;` declarations to build an `initial_module_map`. (This map isn't heavily used yet).
    *   **Output (`DiscoveryOutput`):** Contains a map of `CrateContext` structs (keyed by crate root path). Each `CrateContext` holds the crate name, version, derived namespace, root path, and the list of *all* discovered `.rs` file paths.

## **2. Phase 2 Orchestration & Worker Initialization (`visitor/mod.rs`)**

*   **Goal:** Parse each discovered file in parallel, generating a partial `CodeGraph` for each.
*   **Process (`analyze_files_parallel`):**
    *   Takes the `DiscoveryOutput`.
    *   Iterates through each `CrateContext`.
    *   For each `CrateContext`, it iterates through its list of `.rs` file paths (`crate_context.files`).
    *   It uses `rayon::par_iter` to distribute the parsing work for each file across available threads.
    *   For *each file path*:
        *   It calls `derive_logical_path` to guess the initial module path based *solely* on the file's path relative to `src` (e.g., `src/foo/bar.rs` becomes `["crate", "foo", "bar"]`). This ignores `mod.rs` conventions and `#[path]` attributes for this initial guess.
        *   It calls the worker function `analyze_file_phase2`, passing the specific `file_path`, the `crate_namespace` (from the `CrateContext`), and the `logical_module_path` derived above.
*   **Process (`analyze_file_phase2` - The Worker):**
    *   Takes `file_path`, `crate_namespace`, `logical_module_path`.
    *   Reads the file content.
    *   Uses `syn::parse_file` to get the raw Abstract Syntax Tree (AST) for the file. `syn` *does* handle basic `#![cfg]` at the file level, potentially resulting in an empty item list if the file is excluded by features, but the file is still *processed* up to this point.
    *   Creates a `VisitorState` instance, initializing it with the `crate_namespace`, `file_path`, and setting `state.current_module_path` to the input `logical_module_path`. The `state.current_definition_scope` stack starts empty.
    *   Generates a `root_module_id` (`NodeId::Synthetic`) for the file itself. This uses the `crate_namespace`, `file_path`, the *parent* of the `logical_module_path`, the module name (last segment of `logical_module_path`), `ItemKind::Module`, and `parent_scope_id: None`.
    *   Creates the root `ModuleNode` representing this file, using the `root_module_id` and `logical_module_path`. Adds it to `state.code_graph.modules`.
    *   **Crucially, it pushes this `root_module_id` onto the `state.current_definition_scope` stack.** This establishes the initial parent context for any top-level items defined directly within the file.
    *   Creates a `CodeVisitor` instance, passing it the mutable `state`.
    *   Calls `visitor.visit_file(&file)` to start traversing the AST.

## **3. Phase 2 Visiting (`code_visitor.rs`)**

*   **Goal:** Traverse the `syn` AST, identify code items, generate IDs, extract metadata, and build the partial `CodeGraph`.
*   **Note (2025-12-29):** This section claims `TypeId::generate_synthetic` does **not** use `parent_scope_id`. That is outdated; `TypeId::generate_synthetic` currently hashes `parent_scope_id` (see `ploke-core/src/lib.rs`), so `Self`/generic usages are already scoped by the current definition stack.
*   **General Item Processing (`visit_item_*` methods):**
    *   When the visitor encounters an item like `ItemFn`, `ItemStruct`, `ItemMod`, etc.:
        *   It extracts the item's `name`.
        *   It determines the `ItemKind`.
        *   It calls `self.add_contains_rel(name, kind)`:
            *   This helper calls `self.state.generate_synthetic_node_id(name, kind)`.
            *   `generate_synthetic_node_id` reads `self.state.current_definition_scope.last().copied()` to get the `parent_scope_id`.
            *   `NodeId::generate_synthetic` creates a UUIDv5 hash using: `crate_namespace`, `file_path`, `current_module_path`, `parent_scope_id`, `item_kind`, and `item_name`.
            *   `add_contains_rel` finds the `ModuleNode` matching the `current_module_path`, adds the new `NodeId` to its `items` list, and adds a `Contains` relation (`ModuleNode` -> new `NodeId`).
        *   It extracts other details (visibility, attributes, span, etc.).
        *   It processes nested types (fields, parameters, return types) by calling `get_or_create_type`.
        *   It processes generic parameter *definitions* using `state.process_generics`.
        *   It creates the corresponding `*Node` (e.g., `FunctionNode`) and adds it to the `state.code_graph`.
        *   **It pushes the new item's `NodeId` onto `state.current_definition_scope` before visiting children.**
        *   It calls `visit::visit_item_*` to let `syn` handle recursion into the item's body/contents.
        *   **It pops the item's `NodeId` from `state.current_definition_scope` after visiting children.** This restores the parent scope context for subsequent sibling items.
*   **Type Processing (`get_or_create_type`, `process_type`):**
    *   `get_or_create_type` is the entry point whenever a `TypeId` is needed for a `syn::Type`.
    *   It first calls `process_type(state, ty)`:
        *   `process_type` analyzes the structure of the `syn::Type` (e.g., `Path`, `Reference`, `Tuple`).
        *   It determines the corresponding `TypeKind` variant.
        *   If the type has nested types (e.g., `Vec<T>`, `&'a T`, `(i32, T)`), it *recursively calls `get_or_create_type`* for those inner types (`T`, `i32`).
        *   It collects the `TypeId`s returned by these recursive calls into a `related_types` vector.
        *   It returns the `(TypeKind, Vec<TypeId>)`.
    *   `get_or_create_type` then calls `TypeId::generate_synthetic(namespace, file_path, &type_kind, &related_types)`.
    *   `TypeId::generate_synthetic` creates a UUIDv5 hash based *only* on: `crate_namespace`, `file_path`, the `type_kind` structure (hashed via `#[derive(Hash)]`), and the `related_types` (hashed). **It does NOT currently receive or use the `parent_scope_id` from `VisitorState.current_definition_scope`.**
    *   `get_or_create_type` adds a `TypeNode` to the `state.code_graph.type_graph` if one with the generated ID doesn't already exist (handling recursive types).
    *   It returns the generated `TypeId`.
*   **Generic/Self Handling:**
    *   **Definition:** When `visit_item_fn` (or struct, etc.) calls `state.process_generics`, and it encounters `<T: Bound>`, `process_generics` calls `state.generate_synthetic_node_id("generic_type_T", ItemKind::GenericParam)`. This correctly uses the `parent_scope_id` from the stack (e.g., the `NodeId` of the function defining `T`), so the `GenericParamNode`'s `NodeId` is properly scoped.
    *   **Usage:** When `get_or_create_type` is called for a usage of `T` or `Self` (which are represented as `syn::Type::Path`), `process_type` returns `TypeKind::Named { path: ["T"], .. }` or `TypeKind::Named { path: ["Self"], .. }`. `get_or_create_type` then calls `TypeId::generate_synthetic` with this `TypeKind`. Because `TypeId::generate_synthetic` ignores the `parent_scope_id` available in `VisitorState.current_definition_scope`, the resulting `TypeId` is solely based on the file path, namespace, and the fact that it's a `TypeKind::Named` with the path `"T"` (or `"Self"`).
*   **Why State is Insufficient (for TypeId):** The `VisitorState.current_definition_scope` stack correctly tracks the parent `NodeId` for generating *NodeIds*. However, this crucial contextual information **is not passed to `TypeId::generate_synthetic`**. Therefore, a `T` used in `fn foo<T>` and a `T` used in `struct Bar<T>` (in the same file) will both result in calls to `TypeId::generate_synthetic` with identical inputs (namespace, file path, `TypeKind::Named { path: ["T"], .. }`, empty related types), leading to the same `TypeId::Synthetic`. This is the root cause of the conflation we aim to fix.

## **4. Phase 2 Aggregation & Return**

*   `analyze_files_parallel` collects the `Result<ParsedCodeGraph, syn::Error>` from each `analyze_file_phase2` worker into a `Vec`.
*   **Output:** The final result is this `Vec<Result<ParsedCodeGraph, syn::Error>>`. Each `ParsedCodeGraph` contains the graph built from *one specific file*, along with the file path and crate namespace used during its parsing. There is no merging or cross-file resolution at this stage.

## **5. Connecting Back to Test Failures**

This detailed walkthrough clarifies that while `NodeId` generation leverages the `current_definition_scope` stack for contextual disambiguation, `TypeId` generation currently does not, leading directly to the conflation issues observed with generics and `Self` types.
