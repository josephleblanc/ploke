# Exposed Usage

This document tracks every location in our code that relies on escape hatches and why.

The stated goal of our program is to strive towards the "program as proof" concept of correctness articulated by the Curry-Howard correspondence. However, as we are a rapidly growing project we may not have the time to design or refactor all aspects of our code base with as much validity as desired. Therefore we will use some "escape hatches" as necessary to enable ergonomic functionality, only on low-level functionality wherever possible. We will document all such occurrences for review and future refactoring.

## Type-bearing Ids

### Assurances Offered

Type-bearing IDs (e.g., `ModuleNodeId`, `FunctionNodeId`, `StructNodeId`) are newtype wrappers around the base `NodeId`. Their primary purpose is to leverage Rust's type system to provide **compile-time guarantees** about the semantic kind of node an ID refers to. By using distinct types in function signatures, struct fields, and relation definitions, we prevent accidental misuse of IDs, such as attempting to treat a function's ID as a module's ID. This makes invalid states or relationships unrepresentable at the type level, catching errors during compilation rather than at runtime.

### Example Use Case

Consider a function designed to process module relationships:

```rust
// Without typed IDs (error prone)
fn process_module_relation(source_id: NodeId, target_id: NodeId) { /* ... */ }
let func_id: NodeId = get_some_function_id();
let mod_id: NodeId = get_some_module_id();
process_module_relation(func_id, mod_id); // Compiles, but logically incorrect!

// With typed IDs (compile-time safety)
fn process_module_relation(source_id: ModuleNodeId, target_id: ModuleNodeId) { /* ... */ }
let func_id: FunctionNodeId = get_some_function_id(); // Assume function returns typed ID
let mod_id: ModuleNodeId = get_some_module_id();
// process_module_relation(func_id, mod_id); // Compile-time error! Type mismatch.
process_module_relation(mod_id, mod_id); // Correct usage enforced.
```

### Necessity of Escape Hatches (`.into_inner()`, `.as_inner()`)

While typed IDs provide significant safety benefits, accessing the underlying base `NodeId` is sometimes unavoidable. The `.into_inner()` (consuming) and `.as_inner()` (borrowing) methods serve as necessary "escape hatches" for these situations:

1.  **Interoperability with Base `NodeId` Systems:**
    *   **Generic Collections:** Using `NodeId` as keys in `HashMap` or elements in `HashSet` when the collection needs to store IDs of *any* type (e.g., main node storage, relation indices).
    *   **Core ID Generation:** Functions in `ploke-core` for generating IDs often require base `NodeId`s as context (e.g., parent scope ID).
2.  **Trait Implementations:** Traits requiring a common return type, like `GraphNode::id()`, must return the base `NodeId` if the trait is implemented by nodes with different specific ID types.
3.  **Generic Accessors:** Methods designed explicitly to retrieve the base ID from wrappers (e.g., `PrimaryNodeId::base_id()`, `SyntacticRelation::source()`).
4.  **Error Reporting:** Including a `NodeId` in generic error variants for context when the specific type might not be known or easily representable.
5.  **Debugging and Logging:** Obtaining the underlying UUID for logging or debugging purposes.

The goal is not to eliminate these methods entirely, but to minimize their use and ensure they are only employed where the type-level guarantee is intentionally and necessarily bypassed.

### Enumeration of Current Usages

Based on `rg` analysis (2025-04-30), the following locations use `.into_inner()` or `.as_inner()`:

**Category 1: Trait Implementation Requirement (`GraphNode::id`)**
*   `parser/nodes/import.rs:212` (`impl GraphNode for ImportNode`)
*   `parser/nodes/traits.rs:48` (`impl GraphNode for TraitNode`)
*   `parser/nodes/module.rs:244` (`impl GraphNode for ModuleNode`)
*   `parser/nodes/macros.rs:35` (`impl GraphNode for MacroNode`)
*   `parser/nodes/impls.rs:46` (`impl GraphNode for ImplNode`)
*   `parser/nodes/union.rs:36` (`impl GraphNode for UnionNode`)
*   `parser/nodes/function.rs:39` (`impl GraphNode for MethodNode`)
*   `parser/nodes/function.rs:105` (`impl GraphNode for FunctionNode`)
*   `parser/nodes/value.rs:35` (`impl GraphNode for ConstNode`)
*   `parser/nodes/value.rs:87` (`impl GraphNode for StaticNode`)
*   `parser/nodes/type_alias.rs:36` (`impl GraphNode for TypeAliasNode`)
*   `parser/nodes/enums.rs:70` (`impl GraphNode for EnumNode`)
*   `parser/nodes/structs.rs:62` (`impl GraphNode for StructNode`)

**Category 2: Generic Accessor Methods (`base_id`, `source`, `target`)**
*   `parser/nodes/mod.rs:106-117` (`PrimaryNodeId::base_id`)
*   `parser/nodes/mod.rs:153-155` (`AssociatedItemId::base_id`)
*   `parser/relations.rs:136-147` (`SyntacticRelation::source`)
*   `parser/relations.rs:159-167` (`SyntacticRelation::target`)

**Category 3: Error Reporting / Handling**
*   `parser/graph/mod.rs:366` (`get_struct_checked`)
*   `parser/graph/mod.rs:397` (`get_enum_checked`)
*   `parser/graph/mod.rs:428` (`get_type_alias_checked`)
*   `parser/graph/mod.rs:459` (`get_union_checked`)
*   `parser/graph/mod.rs:829` (`get_function_checked`)
*   `parser/graph/mod.rs:847` (`get_impl_checked`)
*   `parser/graph/mod.rs:866` (`get_trait_checked`)
*   `parser/graph/mod.rs:884` (`get_module_checked`)
*   `parser/graph/mod.rs:903` (`get_const_checked`)
*   `parser/graph/mod.rs:921` (`get_static_checked`)
*   `parser/graph/mod.rs:940` (`get_macro_checked`)
*   `parser/graph/mod.rs:959` (`get_import_checked`)
*   `resolve/module_tree.rs:872` (`get_root_module`) - Error case
*   `resolve/module_tree.rs:932` (`find_file_based_definition`) - Error case
*   `resolve/module_tree.rs:1973` (`find_child_module_by_name`) - Error case
*   `resolve/module_tree.rs:2221`, `2226` (`find_declaration_directory`) - Error cases
*   `resolve/module_tree.rs:2274` (`resolve_path_attribute`) - Error case

**Category 4: Heterogeneous Collections / Indexing**
*   `resolve/module_tree.rs:2468` (`update_path_index`) - Inserting into `path_index: HashMap<NodePath, NodeId>`.
*   `resolve/module_tree.rs:2505` (`find_definition_for_decl`) - Getting from `relations_by_source: HashMap<NodeId, Vec<usize>>`.
*   `resolve/module_tree.rs:2588` (`prune_unlinked_file_modules`) - Collecting `ModuleNodeId`s into `HashSet<NodeId>`.

**Category 5: Interfacing with Functions/Methods Expecting `NodeId`**
*   `resolve/module_tree.rs:927` (`find_file_based_definition`) - Calling `get_relations_to(NodeId, ...)`
*   `resolve/module_tree.rs:1094` (`build_module_tree`) - Comparing `m.id() != *root_id.as_inner()`
*   `resolve/module_tree.rs:1177` (`find_public_path_bfs`) - Comparing `module_node.id() == *self.root.as_inner()`
*   `resolve/module_tree.rs:1346` (`find_public_path_bfs`) - Calling `get_relations_to(NodeId, ...)`
*   `resolve/module_tree.rs:1374` (`find_public_path_bfs`) - Calling `get_relations_to(NodeId, ...)`
*   `resolve/module_tree.rs:1428` (`find_public_path_bfs`) - Calling `get_relations_to(NodeId, ...)`
*   `resolve/module_tree.rs:1436` (`find_public_path_bfs`) - Calling `graph.find_node_unique(NodeId)`
*   `resolve/module_tree.rs:1611` (`add_module`) - Calling `graph.get_item_module_path(NodeId)`
*   `resolve/module_tree.rs:1842` (`resolve_path_segment_in_module`) - Calling `get_relations_from(NodeId, ...)`
*   `resolve/module_tree.rs:2295` (`resolve_path_attribute`) - Constructing `Relation { source: NodeId, ... }` (Likely outdated usage).
*   `resolve/module_tree.rs:2564` (`prune_unlinked_file_modules`) - Calling `get_relations_to(NodeId, ...)`
*   `utils/logging.rs:507, 511, 516, 524, 557, 561, 581, 610` - Calling logging functions expecting `NodeId`.

**Category 6: Direct Comparison with `NodeId`**
*   `parser/graph/mod.rs:184` (`get_child_modules`) - `source.as_inner() == &module_id`
*   `parser/graph/mod.rs:552` (`find_containing_mod_id`) - Returns `source.into_inner()`
*   `parser/graph/mod.rs:753` (`module_contains_node`) - `source.as_inner() == &module_id`
*   `parser/graph/mod.rs:773` (`check_use_statements`) - `source.as_inner() == &context_module_id`
*   `resolve/module_tree.rs:1602` (`add_module`) - `source: *source_mod_id.as_inner()` (in commented-out code).
*   `resolve/module_tree.rs:1984` (`find_module_declaration`) - `rel.source == *decl_id.as_inner()`
*   `resolve/module_tree.rs:1999` (`get_parent_module_id`) - `r.relation().target == *module_id.as_inner()`
*   `resolve/module_tree.rs:2011` (`get_parent_module_id`) - `r_decl.relation().target == *module_id.as_inner()`
*   `resolve/module_tree.rs:2453` (`update_path_index`) - `removed_id != *def_mod_id.as_inner()`

### Evaluation of Usages

*   **Categories 1, 2, 3, 4 (Trait Impl, Accessors, Errors, Indexing):** These usages appear largely **unavoidable** given the current architecture and the need for generic access, base ID retrieval for specific purposes (errors, indexing), and fulfilling trait contracts. Modifying these would require significant architectural changes (e.g., redesigning the `GraphNode` trait or error handling).
*   **Categories 5 & 6 (Interfacing Functions, Direct Comparison):** These categories are **potentially avoidable** and represent the primary targets for refactoring to reduce escape hatch usage. The necessity depends on:
    *   **Called Function Signatures:** Can functions like `get_relations_to`, `get_relations_from`, `get_item_module_path`, logging functions, etc., be modified to accept the specific typed ID (e.g., `ModuleNodeId`) instead of the generic `NodeId`? This is feasible if the function *only* ever operates on that specific ID type in that context.
    *   **Origin of `NodeId` Variables:** In comparison cases (Category 6), where does the base `NodeId` variable being compared against come from? Can it be replaced with a typed ID earlier in the code flow? For example, if `module_id` in `get_child_modules` is always a `ModuleNodeId`, the comparison could potentially be done without unwrapping `source`.
    *   **Clarifying Questions:**
        *   For `ModuleTree` methods like `get_relations_to`/`from`, what specific ID types are realistically passed as the `NodeId` argument? If it's always `ModuleNodeId`, change the signature. If not, can generics (`impl Borrow<NodeId>`) or specific methods per type be used?
        *   In comparison scenarios (e.g., `parser/graph/mod.rs:184`), what is the type context of the `NodeId` variable (`module_id`)? Can it be typed?

### Possible Solutions for Avoidable Cases

1.  **Refactor Function Signatures:** Change functions currently accepting `NodeId` to accept the specific typed ID(s) they actually operate on (e.g., `fn get_relations_from_module(id: ModuleNodeId, ...)`).
2.  **Use Category Enums:** Pass category enums (`PrimaryNodeId`, `AssociatedItemId`) where appropriate and use `match` within the function to handle different specific types safely.
3.  **Trace Variable Origins:** Identify where base `NodeId` variables used in comparisons originate and attempt to replace them with typed IDs earlier in the logic.
4.  **Generics with Traits (Limited Use):** For functions needing the base ID from multiple *different* typed IDs, consider a trait bound like `T: Borrow<NodeId>` or similar, although this still involves accessing the base ID.

---

## Other Potential Escape Hatches

*   **`unsafe` Blocks:** Track any usage of `unsafe` code, explaining the necessity and the invariants being manually upheld.
*   **Panics (`panic!`, `unwrap`, `expect`):** Document instances where panics are used instead of `Result` for error handling, especially in library code. Justify why a panic is acceptable (e.g., unrecoverable state, invariant violation) or mark for refactoring to `Result`.
*   **Thread Safety Primitives (if applicable):** Document usage of `Mutex`, `RwLock`, atomics, explaining why they are needed and how potential deadlocks or race conditions are mitigated.
*   **FFI (Foreign Function Interface):** Calls to C or other languages bypass Rust's safety guarantees. Document these interfaces and the assumptions made about the foreign code.
