#![cfg(feature = "uuid_ids")] // Gate the whole module

#[cfg(test)]
mod phase2_relation_tests {
    use ploke_common::{fixtures_crates_dir, workspace_root};
    use ploke_core::{NodeId, TypeId};
    use std::{
        collections::HashMap,
        path::{Path, PathBuf},
    };
    use syn_parser::{
        discovery::{run_discovery_phase, DiscoveryOutput},
        parser::{analyze_files_parallel, nodes::ImportNode},
        parser::{
            graph::CodeGraph,
            nodes::{
                FieldNode, FunctionNode, ImplNode, ModuleNode, StructNode, TraitNode, TypeDefNode,
                ValueNode, Visible,
            },
            relations::{GraphId, Relation, RelationKind},
            types::TypeNode,
        },
    };
    use uuid::Uuid;

    // --- Test Setup Helpers ---

    // Helper function to run Phase 1 & 2 for a single fixture
    fn run_phase1_phase2(fixture_name: &str) -> Vec<Result<CodeGraph, syn::Error>> {
        let crate_path = fixtures_crates_dir().join(fixture_name);
        // Use workspace root as project root for discovery context
        let project_root = workspace_root();
        let discovery_output =
            run_discovery_phase(&project_root, &[crate_path]).unwrap_or_else(|e| {
                panic!(
                    "Phase 1 Discovery failed for fixture '{}': {:?}",
                    fixture_name, e
                )
            });
        analyze_files_parallel(&discovery_output, 0)
    }

    // Helper to find the CodeGraph for a specific file within the results
    // Note: This relies on the file path being stored or derivable.
    // For now, we might need to use heuristics or find nodes within the graph.
    // Let's assume we can find the graph containing a specific top-level item for now.
    fn find_graph_containing_item<'a>(
        results: &'a [Result<CodeGraph, syn::Error>],
        item_name: &str,
    ) -> &'a CodeGraph {
        results
            .iter()
            .enumerate() // Add enumeration for index
            .find_map(|(index, res)| {
                #[cfg(feature = "verbose_debug")]
                println!(
                    "REL_TEST_DEBUG: [find_graph_containing_item] Checking graph index: {}",
                    index
                );
                res.as_ref().ok().and_then(|g| {
                    // Check functions, structs, modules etc. for the name
                    let found_id = find_node_id_by_name(g, item_name);
                    #[cfg(feature = "verbose_debug")]
                    println!(
                        "REL_TEST_DEBUG: [find_graph_containing_item] Checking for '{}', found_id: {:?}",
                        item_name, found_id
                    );
                    if found_id.is_some() {
                        Some(g)
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_else(|| panic!("Could not find graph containing item '{}'", item_name))
    }

    // Helper to find a NodeId by name (searches common node types)
    fn find_node_id_by_name(graph: &CodeGraph, name: &str) -> Option<NodeId> {
        graph
            .functions
            .iter()
            .find(|n| n.name == name)
            .map(|n| n.id)
            .or_else(|| {
                graph.defined_types.iter().find_map(|td| {
                    #[cfg(feature = "verbose_debug")]
                    {
                        // Print details about the TypeDefNode being checked
                        let type_name = match td {
                            TypeDefNode::Struct(s) => &s.name,
                            TypeDefNode::Enum(e) => &e.name,
                            TypeDefNode::TypeAlias(t) => &t.name,
                            TypeDefNode::Union(u) => &u.name,
                        };
                        println!(
                            "REL_TEST_DEBUG: [find_node_id_by_name] Checking defined_type: {}, looking for: {}",
                            type_name, name
                        );
                    }
                    match td {
                        TypeDefNode::Struct(s) if s.name == name => Some(s.id),
                        TypeDefNode::Enum(e) if e.name == name => Some(e.id),
                        TypeDefNode::TypeAlias(t) if t.name == name => Some(t.id),
                        TypeDefNode::Union(u) if u.name == name => Some(u.id),
                        _ => None,
                    }
                })
            })
            .or_else(|| {
                graph
                    .traits
                    .iter()
                    .chain(&graph.private_traits)
                    .find(|n| n.name == name)
                    .map(|n| n.id)
            })
            .or_else(|| graph.modules.iter().find(|n| n.name == name).map(|n| n.id))
            .or_else(|| graph.values.iter().find(|n| n.name == name).map(|n| n.id))
            .or_else(|| graph.macros.iter().find(|n| n.name == name).map(|n| n.id))
            .or_else(|| {
                graph
                    .use_statements
                    .iter()
                    .find(|n| n.visible_name == name) // Check visible name for imports
                    .map(|n| n.id)
            })
            .or_else(|| {
                // Search within impl methods (less efficient)
                graph
                    .impls
                    .iter()
                    .find_map(|imp| imp.methods.iter().find(|m| m.name == name).map(|m| m.id))
            })
        // Note: Does not find FieldNode IDs by name directly, need parent context.
    }

    // Helper to find a FieldNode ID within a struct/enum variant
    fn find_field_node_id(
        graph: &CodeGraph,
        parent_id: NodeId,
        field_name: &str,
    ) -> Option<NodeId> {
        graph.defined_types.iter().find_map(|td| match td {
            TypeDefNode::Struct(s) if s.id == parent_id => s
                .fields
                .iter()
                .find(|f| f.name.as_deref() == Some(field_name))
                .map(|f| f.id),
            TypeDefNode::Enum(e) if e.id == parent_id => {
                // Need to know variant name too, or search all variants
                e.variants.iter().find_map(|v| {
                    v.fields
                        .iter()
                        .find(|f| f.name.as_deref() == Some(field_name))
                        .map(|f| f.id)
                })
            }
            TypeDefNode::Union(u) if u.id == parent_id => u
                .fields
                .iter()
                .find(|f| f.name.as_deref() == Some(field_name))
                .map(|f| f.id),
            _ => None,
        })
    }

    // Helper to find the TypeId of a function's parameter by index
    fn find_param_type_id(
        graph: &CodeGraph,
        func_id: NodeId,
        param_index: usize,
    ) -> Option<TypeId> {
        graph
            .functions
            .iter()
            .find(|f| f.id == func_id)
            .and_then(|f| f.parameters.get(param_index))
            .map(|p| p.type_id)
    }

    // Helper to find the TypeId of a function's return type
    fn find_return_type_id(graph: &CodeGraph, func_id: NodeId) -> Option<TypeId> {
        graph
            .functions
            .iter()
            .find(|f| f.id == func_id)
            .and_then(|f| f.return_type)
    }

    // Helper to find the TypeId of a struct field
    fn find_field_type_id(graph: &CodeGraph, field_id: NodeId) -> Option<TypeId> {
        // Need to iterate through all fields in all structs/enums/unions
        graph.defined_types.iter().find_map(|td| match td {
            TypeDefNode::Struct(s) => s
                .fields
                .iter()
                .find(|f| f.id == field_id)
                .map(|f| f.type_id),
            TypeDefNode::Enum(e) => e.variants.iter().find_map(|v| {
                v.fields
                    .iter()
                    .find(|f| f.id == field_id)
                    .map(|f| f.type_id)
            }),
            TypeDefNode::Union(u) => u
                .fields
                .iter()
                .find(|f| f.id == field_id)
                .map(|f| f.type_id),
            _ => None,
        })
    }

    // Helper to find the TypeId of an impl's self_type
    fn find_impl_self_type_id(graph: &CodeGraph, impl_id: NodeId) -> Option<TypeId> {
        graph
            .impls
            .iter()
            .find(|i| i.id == impl_id)
            .map(|i| i.self_type)
    }

    // Helper to find the TypeId of an impl's trait_type
    fn find_impl_trait_type_id(graph: &CodeGraph, impl_id: NodeId) -> Option<TypeId> {
        graph
            .impls
            .iter()
            .find(|i| i.id == impl_id)
            .and_then(|i| i.trait_type)
    }

    // Core assertion helper
    fn assert_relation_exists(
        graph: &CodeGraph,
        source: GraphId,
        target: GraphId,
        kind: RelationKind,
        message: &str,
    ) {
        let found = graph
            .relations
            .iter()
            .any(|r| r.source == source && r.target == target && r.kind == kind);
        assert!(found, "{}", message);
    }

    // Core assertion helper to check if a specific relation DOES NOT exist
    fn assert_relation_does_not_exist(
        graph: &CodeGraph,
        source: GraphId,
        target: GraphId,
        kind: RelationKind,
        message: &str,
    ) {
        let found = graph
            .relations
            .iter()
            .any(|r| r.source == source && r.target == target && r.kind == kind);
        assert!(!found, "{}", message);
    }

    // Helper to find the TypeId of a specific field within a struct
    // Returns the TypeId stored *on the FieldNode*, not from a relation
    fn find_field_type_id_on_node(
        graph: &CodeGraph,
        struct_id: NodeId,
        field_name: &str,
    ) -> Option<TypeId> {
        graph.defined_types.iter().find_map(|td| match td {
            TypeDefNode::Struct(s) if s.id == struct_id => s
                .fields
                .iter()
                .find(|f| f.name.as_deref() == Some(field_name))
                .map(|f| f.type_id), // Get TypeId directly from FieldNode
            // Add cases for Enum variants / Unions if needed
            _ => None,
        })
    }

    // --- Relation Tests ---

    #[test]
    fn test_contains_relation() {
        let results = run_phase1_phase2("example_crate");
        let lib_graph = find_graph_containing_item(&results, "add"); // Find graph for lib.rs

        // 1. Module -> Function
        let crate_mod_id =
            find_node_id_by_name(lib_graph, "crate").expect("Failed to find crate root module");
        let add_func_id =
            find_node_id_by_name(lib_graph, "add").expect("Failed to find 'add' function");
        assert_relation_exists(
            lib_graph,
            GraphId::Node(crate_mod_id),
            GraphId::Node(add_func_id),
            RelationKind::Contains,
            "Crate module should contain 'add' function",
        );

        // 2. Module -> Struct
        let my_struct_id =
            find_node_id_by_name(lib_graph, "MyStruct").expect("Failed to find 'MyStruct' struct");
        assert_relation_exists(
            lib_graph,
            GraphId::Node(crate_mod_id),
            GraphId::Node(my_struct_id),
            RelationKind::Contains,
            "Crate module should contain 'MyStruct' struct",
        );

        // 3. Module -> Sub-Module
        let mod_two_graph = find_graph_containing_item(&results, "mod_two_func"); // Find graph for module_two/mod.rs
        let mod_two_id = find_node_id_by_name(mod_two_graph, "module_two")
            .expect("Failed to find 'module_two' module");
        // We need the ID of the parent module ('crate' in this case) from the *correct graph*
        let parent_mod_id = find_node_id_by_name(mod_two_graph, "crate")
            .expect("Failed to find crate root module in mod_two's graph");

        // This assertion might be tricky if the relation is stored only in the parent graph.
        // Let's assume for now the relation might appear in either graph's perspective,
        // or ideally, Phase 3 merges this. For Phase 2 testing, we might need to check
        // the graph where the parent module is defined.
        let crate_graph = find_graph_containing_item(&results, "add"); // Graph for lib.rs
        let mod_two_id_in_crate_graph = find_node_id_by_name(crate_graph, "module_two")
            .expect("Failed to find 'module_two' module in crate graph");

        // Note: The structure in example_crate is a bit odd. module_one.rs also declares a module_two.
        // We are testing the top-level `mod module_two;` declared in lib.rs here.
        #[cfg(feature = "verbose_debug")]
        {
            println!("REL_TEST_DEBUG: [test_contains_relation] Checking Module->SubModule:");
            println!("  REL_TEST_DEBUG: Parent Module ('crate') ID: {:?}", parent_mod_id);
            println!("  REL_TEST_DEBUG: Sub Module ('module_two') ID: {:?}", mod_two_id_in_crate_graph);
            println!("  REL_TEST_DEBUG: Parent Graph Relations Count: {}", crate_graph.relations.len());
            // Optionally print all relations if the list isn't too long
            // println!("  REL_TEST_DEBUG: Parent Graph Relations: {:?}", crate_graph.relations);
        }

        assert_relation_exists(
            crate_graph, // Check in the parent's graph
            GraphId::Node(parent_mod_id),
            GraphId::Node(mod_two_id_in_crate_graph), // Use ID found in the parent graph context
            RelationKind::Contains,
            "Crate module should contain 'module_two' submodule",
        );

        // Debugging for the failing Module -> Function case
        #[cfg(feature = "verbose_debug")]
        {
            println!("REL_TEST_DEBUG: [test_contains_relation] Checking Module->Function:");
            println!("  REL_TEST_DEBUG: Module ('crate') ID: {:?}", crate_mod_id);
            println!("  REL_TEST_DEBUG: Function ('add') ID: {:?}", add_func_id);
            println!("  REL_TEST_DEBUG: Graph Relations Count: {}", lib_graph.relations.len());
             println!("  REL_TEST_DEBUG: Graph Relations: {:?}", lib_graph.relations); // Print all relations
        }
        // Re-assert the failing one after printing debug info
        // This assertion is expected to FAIL until the implementation bug is fixed.
         assert_relation_exists(
            lib_graph,
            GraphId::Node(crate_mod_id),
            GraphId::Node(add_func_id),
            RelationKind::Contains,
            "Crate module should contain 'add' function (re-assert after debug)",
        );

    }

    #[test]
    fn test_function_type_relations() {
        let results = run_phase1_phase2("example_crate");
        let lib_graph = find_graph_containing_item(&results, "add");

        // 1. Function -> Parameter Type
        let add_func_id =
            find_node_id_by_name(lib_graph, "add").expect("Failed to find 'add' function");
        let left_param_type_id = find_param_type_id(lib_graph, add_func_id, 0)
            .expect("Failed to find type ID for 'left' param");
        let right_param_type_id = find_param_type_id(lib_graph, add_func_id, 1)
            .expect("Failed to find type ID for 'right' param");

        assert_relation_exists(
            lib_graph,
            GraphId::Node(add_func_id),
            GraphId::Type(left_param_type_id),
            RelationKind::FunctionParameter,
            "Relation missing for 'add' function -> 'left' parameter type",
        );
        assert_relation_exists(
            lib_graph,
            GraphId::Node(add_func_id),
            GraphId::Type(right_param_type_id),
            RelationKind::FunctionParameter,
            "Relation missing for 'add' function -> 'right' parameter type",
        );
        // Check they are the same TypeId (for u64)
        assert_eq!(
            left_param_type_id, right_param_type_id,
            "Expected params to have same TypeId for u64"
        );

        // 2. Function -> Return Type
        let return_type_id = find_return_type_id(lib_graph, add_func_id)
            .expect("Failed to find return type ID for 'add' function");
        assert_relation_exists(
            lib_graph,
            GraphId::Node(add_func_id),
            GraphId::Type(return_type_id),
            RelationKind::FunctionReturn,
            "Relation missing for 'add' function -> return type",
        );
        // Check return type is also same TypeId (for u64)
        assert_eq!(
            left_param_type_id, return_type_id,
            "Expected return type to have same TypeId for u64"
        );
    }

    #[test]
    fn test_struct_field_logic() {
        // This test verifies the FieldNode data for SomeStruct in module_one.rs
        let results = run_phase1_phase2("example_crate");
        #[cfg(feature = "verbose_debug")]
        {
            println!("REL_TEST_DEBUG: [test_struct_field_logic] Phase 1&2 Results Count: {}", results.len());
            for (i, res) in results.iter().enumerate() {
                 match res {
                    Ok(g) => println!("  REL_TEST_DEBUG: Graph {}: {} functions, {} types, {} relations", i, g.functions.len(), g.defined_types.len(), g.relations.len()),
                    Err(e) => println!("  REL_TEST_DEBUG: Graph {}: Error - {}", i, e),
                 }
            }
        }
        // SomeStruct is defined in module_one.rs
        let mod_one_graph = find_graph_containing_item(&results, "SomeStruct");

        let struct_id = find_node_id_by_name(mod_one_graph, "SomeStruct")
            .expect("Failed to find 'SomeStruct' struct");

        // 1. Verify FieldNode data exists and has correct TypeId
        // We use the helper that reads the TypeId directly from the FieldNode
        let field_type_id = find_field_type_id_on_node(mod_one_graph, struct_id, "some_field")
            .expect("Failed to find type ID for 'some_field' directly on FieldNode");

        // Assert the type ID is synthetic (as it's likely i32)
        assert!(
            matches!(field_type_id, TypeId::Synthetic(_)),
            "Field type ID should be synthetic"
        );

        // 2. Assert that a StructField relation DOES NOT exist (as FieldNode has no ID)
        // We cannot construct a target GraphId::Node(field_id).
        // We also assume a Struct -> Type relation isn't created for fields.
        // Therefore, we check that no relation points *from* the struct *to* the field's type
        // with the kind StructField.
        assert_relation_does_not_exist(
            mod_one_graph,
            GraphId::Node(struct_id),
            GraphId::Type(field_type_id),
            RelationKind::StructField,
            "Relation Struct -> FieldType should NOT exist for StructField kind",
        );

        println!("REL_TEST_DEBUG: test_struct_field_logic passed checks for SomeStruct.");
    }

    // #[test] // TODO: Re-enable this test when a fixture with an impl block is available or created.
    // fn test_impl_relations() {
    //     let results = run_phase1_phase2("example_crate");
    //      #[cfg(feature = "verbose_debug")]
    //     {
    //         println!("REL_TEST_DEBUG: [test_impl_relations] Phase 1&2 Results Count: {}", results.len());
    //         for (i, res) in results.iter().enumerate() {
    //              match res {
    //                 Ok(g) => println!("  REL_TEST_DEBUG: Graph {}: {} functions, {} types, {} impls, {} relations", i, g.functions.len(), g.defined_types.len(), g.impls.len(), g.relations.len()),
    //                 Err(e) => println!("  REL_TEST_DEBUG: Graph {}: Error - {}", i, e),
    //              }
    //         }
    //     }
    //     // This test requires a struct/enum/trait that actually has an impl block in the fixture.
    //     // 'example_crate' currently does not have one.
    //     // Find an item that *is* implemented in the fixture.
    //     let item_name_with_impl = "ItemWithImpl"; // Replace with actual item name
    //     let graph_with_impl = find_graph_containing_item(&results, item_name_with_impl);
    //
    //     let item_id = find_node_id_by_name(graph_with_impl, item_name_with_impl).unwrap();
    //     // Need a way to get the TypeId for the item being implemented
    //     let item_type_id = /* ... get TypeId for item_id ... */ unimplemented!();
    //
    //     let impl_node = graph_with_impl
    //         .impls
    //         .iter()
    //         .find(|imp| imp.self_type == item_type_id)
    //         .expect("Failed to find impl block for item");
    //     let impl_id = impl_node.id;
    //
    //     // 1. Impl -> Self Type (ImplementsFor)
    //     let self_type_id = find_impl_self_type_id(graph_with_impl, impl_id)
    //         .expect("Failed to find self_type ID for impl");
    //     assert_eq!(self_type_id, item_type_id, "Impl self_type mismatch"); // Sanity check
    //     assert_relation_exists(
    //         graph_with_impl,
    //         GraphId::Node(impl_id),
    //         GraphId::Type(self_type_id),
    //         RelationKind::ImplementsFor,
    //         "Relation missing for Impl -> Self Type",
    //     );
    //
    //     // 2. Impl -> Trait Type (ImplementsTrait) - If applicable
    //     if let Some(trait_type_id) = find_impl_trait_type_id(graph_with_impl, impl_id) {
    //         assert_relation_exists(
    //             graph_with_impl,
    //             GraphId::Node(impl_id),
    //             GraphId::Type(trait_type_id),
    //             RelationKind::ImplementsTrait,
    //             "Relation missing for Impl -> Trait Type",
    //         );
    //         // Find the TraitNode and verify its TypeId matches trait_type_id
    //         let trait_name = "SomeTrait"; // Replace with actual trait name
    //         let trait_id = find_node_id_by_name(graph_with_impl, trait_name).expect("Failed to find Trait");
    //         // ... verify TypeId match ...
    //     }
    //
    //     // 3. Impl -> Method (Method relation)
    //     let method_name = "impl_method_name"; // Replace with actual method name
    //     let method_id = find_node_id_by_name(graph_with_impl, method_name)
    //         .expect("Failed to find method in impl");
    //     assert_relation_exists(
    //         graph_with_impl,
    //         GraphId::Node(impl_id),
    //         GraphId::Node(method_id),
    //         RelationKind::Method, // Using 'Method' kind
    //         "Relation missing for Impl -> Method",
    //     );
    // }

    // TODO: Add tests for other relation kinds based on current implementation:
    // - Method (Trait -> Function)
    // - EnumVariant (Enum -> VariantNode)
    // - Inherits (Trait -> Trait TypeId)
    // - References (Could be used for various things, e.g., field -> type?)
    // - Uses (ImportNode -> TypeId/NodeId of imported item)
    // - ValueType (ValueNode -> TypeId)
    // - Method (Trait -> Function) - Check TraitNode.methods and assert relation TraitNodeId -> FunctionNodeId
    // - EnumVariant (Enum -> VariantNode) - Similar to StructField, VariantNode has no ID. Check data on EnumNode.
    // - Inherits (Trait -> Trait TypeId) - Check TraitNode.super_traits and assert relation TraitNodeId -> SuperTraitTypeId
    // - Uses (ImportNode -> ???) - What ID does an import point to in Phase 2? Likely unresolved TypeId::Synthetic.
    // - ValueType (ValueNode -> TypeId) - Check ValueNode.type_id and assert relation ValueNodeId -> TypeId
    // - ModuleImports (ModuleNode -> ImportNode) - Check ModuleNode.imports and assert relation ModuleNodeId -> ImportNodeId
}
