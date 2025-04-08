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
            .find_map(|res| {
                res.as_ref().ok().filter(|g| {
                    // Check functions, structs, modules etc. for the name
                    find_node_id_by_name(g, item_name).is_some()
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
                graph.defined_types.iter().find_map(|td| match td {
                    TypeDefNode::Struct(s) if s.name == name => Some(s.id),
                    TypeDefNode::Enum(e) if e.name == name => Some(e.id),
                    TypeDefNode::TypeAlias(t) if t.name == name => Some(t.id),
                    TypeDefNode::Union(u) if u.name == name => Some(u.id),
                    _ => None,
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

        assert_relation_exists(
            crate_graph, // Check in the parent's graph
            GraphId::Node(parent_mod_id),
            GraphId::Node(mod_two_id_in_crate_graph),
            RelationKind::Contains,
            "Crate module should contain 'module_two' submodule",
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
    fn test_struct_field_relations() {
        let results = run_phase1_phase2("example_crate");
        let lib_graph = find_graph_containing_item(&results, "MyStruct");

        let struct_id =
            find_node_id_by_name(lib_graph, "MyStruct").expect("Failed to find 'MyStruct' struct");

        // 1. Struct -> Field Node
        // We need the NodeId generated specifically for the field itself.
        let field_id = find_field_node_id(lib_graph, struct_id, "my_field")
            .expect("Failed to find NodeId for 'my_field'");

        // This relation might not exist if fields aren't treated as first-class nodes
        // with their own Contains relation. Let's assume it *should* exist for now.
        // If FieldNode gets its ID via generate_synthetic, this relation should be added.
        assert_relation_exists(
            lib_graph,
            GraphId::Node(struct_id),
            GraphId::Node(field_id),
            RelationKind::StructField, // Or maybe Contains? Let's use StructField for semantics.
            "Relation missing for 'MyStruct' -> 'my_field' Node",
        );

        // 2. Field Node -> Field Type
        let field_type_id =
            find_field_type_id(lib_graph, field_id).expect("Failed to find type ID for 'my_field'");

        // This relation is less common. Usually, the type info is directly on FieldNode.
        // However, if we model types relationally, it might exist.
        // Let's assume for now the primary check is FieldNode.type_id.
        // If a relation *is* created, it might be References or ValueType?
        // Let's skip asserting this specific relation for now unless the design requires it.
        // assert_relation_exists(
        //     lib_graph,
        //     GraphId::Node(field_id),
        //     GraphId::Type(field_type_id),
        //     RelationKind::ValueType, // Or References?
        //     "Relation missing for 'my_field' Node -> field type",
        // );

        // Verify the type_id stored on the FieldNode directly
        let field_node = lib_graph
            .find_node(field_id)
            .expect("Cannot find field node by ID");
        // We need to downcast or access the field directly. Let's assume find_node is enhanced or we search manually.
        let found_field_type_id = lib_graph
            .defined_types
            .iter()
            .find_map(|td| match td {
                TypeDefNode::Struct(s) if s.id == struct_id => s
                    .fields
                    .iter()
                    .find(|f| f.id == field_id)
                    .map(|f| f.type_id),
                _ => None,
            })
            .expect("Could not re-find field node to check type_id");

        assert_eq!(
            found_field_type_id, field_type_id,
            "FieldNode.type_id mismatch"
        );
        assert!(
            matches!(field_type_id, TypeId::Synthetic(_)),
            "Field type ID should be synthetic"
        );
    }

    #[test]
    fn test_impl_relations() {
        let results = run_phase1_phase2("example_crate");
        let lib_graph = find_graph_containing_item(&results, "MyStruct"); // Impl is in lib.rs

        // Find the impl block (tricky without a direct name)
        // Let's find it by the type it implements for (MyStruct)
        let my_struct_type_id = find_field_type_id(
            lib_graph,
            find_field_node_id(
                lib_graph,
                find_node_id_by_name(lib_graph, "MyStruct").unwrap(),
                "my_field", // Use a known field to get the TypeId for MyStruct
            )
            .unwrap(),
        )
        .unwrap();

        let impl_node = lib_graph
            .impls
            .iter()
            .find(|imp| imp.self_type == my_struct_type_id)
            .expect("Failed to find impl block for MyStruct");
        let impl_id = impl_node.id;

        // 1. Impl -> Self Type (ImplementsFor)
        let self_type_id = find_impl_self_type_id(lib_graph, impl_id)
            .expect("Failed to find self_type ID for impl");
        assert_eq!(self_type_id, my_struct_type_id, "Impl self_type mismatch"); // Sanity check
        assert_relation_exists(
            lib_graph,
            GraphId::Node(impl_id),
            GraphId::Type(self_type_id),
            RelationKind::ImplementsFor,
            "Relation missing for Impl -> Self Type",
        );

        // 2. Impl -> Trait Type (ImplementsTrait) - If applicable
        if let Some(trait_type_id) = find_impl_trait_type_id(lib_graph, impl_id) {
            assert_relation_exists(
                lib_graph,
                GraphId::Node(impl_id),
                GraphId::Type(trait_type_id),
                RelationKind::ImplementsTrait,
                "Relation missing for Impl -> Trait Type",
            );
            // We would also need to find the TraitNode and verify its TypeId matches
            let trait_id =
                find_node_id_by_name(lib_graph, "MyTrait").expect("Failed to find MyTrait");
            // How do we get the TypeId for a TraitNode? Assume it's derivable or stored.
            // For now, just check the relation exists.
        } else {
            // If the impl in the fixture doesn't implement a trait, this is expected.
            // Check the example_crate fixture to confirm.
            // example_crate's impl block *does* implement MyTrait.
            panic!("Expected to find a trait implementation in example_crate's impl block");
        }

        // 3. Impl -> Method (Method relation, or maybe Contains?)
        let method_id =
            find_node_id_by_name(lib_graph, "impl_method").expect("Failed to find 'impl_method'");
        assert_relation_exists(
            lib_graph,
            GraphId::Node(impl_id),
            GraphId::Node(method_id),
            RelationKind::Method, // Using 'Method' kind
            "Relation missing for Impl -> Method",
        );
    }

    // TODO: Add tests for other relation kinds:
    // - Method (Trait -> Function)
    // - EnumVariant (Enum -> VariantNode)
    // - Inherits (Trait -> Trait TypeId)
    // - References (Could be used for various things, e.g., field -> type?)
    // - Uses (ImportNode -> TypeId/NodeId of imported item)
    // - ValueType (ValueNode -> TypeId)
    // - MacroUse (?)
    // - ModuleImports (ModuleNode -> ImportNode)
}
