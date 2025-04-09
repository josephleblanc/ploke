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
        parser::{
            analyze_files_parallel,
            graph::CodeGraph,
            nodes::{
                FieldNode, FunctionNode, ImplNode, ImportNode, ModuleNode, StructNode, TraitNode,
                TypeDefNode, ValueNode, Visible,
            },
            relations::{GraphId, Relation, RelationKind},
            types::{GenericParamKind, TypeNode},
            visitor::ParsedCodeGraph,
        },
    };
    use uuid::Uuid;

    // --- Test Setup Helpers ---

    // Helper function to run Phase 1 & 2 for a single fixture
    fn run_phase1_phase2(fixture_name: &str) -> Vec<Result<ParsedCodeGraph, syn::Error>> {
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
    fn find_inline_module_by_path<'a>(
        graph: &'a CodeGraph,
        module_path: &[String],
    ) -> Option<&'a ModuleNode> {
        let mut modules = graph.modules.iter().filter(|m| m.path == module_path);
        let found = modules.next();
        let mut errs = Vec::new();
        while let Some(unexpected_module) = modules.next() {
            errs.push(unexpected_module);
        }
        if !errs.is_empty() {
            panic!(
                "Mutiple modules found with same path.
  First module found: {:?}
  Other modules found: {:?}",
                found, errs
            );
        }
        found
    }

    /// Finds a node ID by its module path and name within a Phase 2 CodeGraph.
    /// Assumes ModuleNode.items is populated during Phase 2 parsing for nodes defined in that file.
    fn find_node_id_by_path_and_name(
        graph: &CodeGraph,
        module_path: &[String], // e.g., ["crate", "outer", "inner"]
        name: &str,
    ) -> Option<NodeId> {
        // 1. Find the module node corresponding to the path in *this* graph
        let target_module = graph.modules.iter().find(|m| m.path == module_path)?;

        // Convert items Vec<NodeId> to a HashSet for faster lookups if needed,
        // though for typical module sizes, linear scan might be fine.
        // let module_item_ids: std::collections::HashSet<_> = target_module.items.iter().collect();

        // 2. Search functions
        let func_id = graph
            .functions
            .iter()
            .find(|f| {
                f.name() == name && target_module.items.contains(&f.id()) // Check name and module membership
            })
            .map(|f| f.id());

        if func_id.is_some() {
            return func_id;
        }

        // 3. Search defined types (Struct, Enum, Union, TypeAlias)
        let type_def_id = graph.defined_types.iter().find_map(|td| {
            // Use the Visible trait implemented by node types
            if td.name() == name && target_module.items.contains(&td.id()) {
                Some(td.id())
            } else {
                None
            }
        });

        if type_def_id.is_some() {
            return type_def_id;
        }

        // 4. Search other top-level items if needed (Traits, Impls - though Impls might not have names/paths like this)
        let trait_id = graph
            .traits
            .iter()
            .find(|t| t.name() == name && target_module.items.contains(&t.id()))
            .map(|t| t.id());

        if trait_id.is_some() {
            return trait_id;
        }

        // ... add searches for other relevant node types that implement Visible and belong in ModuleNode.items

        None
    }
    fn find_import_longname_by_id(graph: &CodeGraph, node_id: NodeId) -> Option<String> {
        graph
            .use_statements
            .iter()
            .find(|imp| imp.id == node_id)
            .map(|imp| {
                format!(
                    "{}::{}{}",
                    imp.path.join("::"),
                    imp.visible_name,
                    if let Some(original_name) = &imp.original_name {
                        format!(" as {}", original_name)
                    } else {
                        "".to_string()
                    }
                )
            })
    }
    fn find_node_id_name(graph: &CodeGraph, node_id: NodeId) -> Option<String> {
        graph
            .find_node(node_id)
            .map(|n| n.name().to_string())
            .or_else(|| find_import_longname_by_id(graph, node_id))
            .or_else(|| {
                graph
                    .defined_types
                    .iter()
                    .find_map(|def_type| match def_type {
                        TypeDefNode::Struct(struct_node) => struct_node
                            .fields
                            .iter()
                            .find(|field| field.id == node_id)
                            .map(|field| field.name.clone()),
                        TypeDefNode::Enum(_enum_node) => None, // fill out as needed
                        TypeDefNode::TypeAlias(_type_alias_node) => None, // fill out as needed
                        TypeDefNode::Union(_union_node) => None, // fill out as needed
                    })
                    .unwrap_or(None)
            })
    }
    fn find_type_id_name(graph: &CodeGraph, ty_id: TypeId) -> Option<String> {
        let found_name: Option<String> = graph
            .defined_types
            .iter()
            .filter_map(|td| match td {
                TypeDefNode::Struct(struct_node) => struct_node
                    .generic_params
                    .iter()
                    .find_map(|param| {
                        param
                            .name_if_type_id(ty_id)
                            .map(|param_name| param_name.to_string())
                    })
                    .or_else(|| {
                        struct_node
                            .fields
                            .iter()
                            .find(|field| field.type_id == ty_id)
                            .map(|field| {
                                field
                                    .clone()
                                    .name
                                    .unwrap_or(format!("Unnamed_field of {}", struct_node.name))
                            })
                    }),
                TypeDefNode::Enum(enum_node) => enum_node
                    .variants
                    .iter()
                    .find_map(|v| {
                        // Check each variant's fields
                        v.fields
                            .iter()
                            .find(|field| field.type_id == ty_id)
                            .map(|field| {
                                field
                                    .clone()
                                    .name
                                    .unwrap_or(format!("Unnamed_field of {}", enum_node.name))
                            })
                    })
                    .or_else(|| {
                        // Check generic params
                        enum_node.generic_params.iter().find_map(|param| {
                            param
                                .name_if_type_id(ty_id)
                                .map(|param_name| param_name.to_string())
                        })
                    }),
                TypeDefNode::TypeAlias(type_alias_node) => type_alias_node
                    .generic_params // Chech generic params
                    .iter()
                    .find_map(|param| {
                        param
                            .name_if_type_id(ty_id)
                            .map(|param_name| param_name.to_string())
                    }),
                TypeDefNode::Union(union_node) => union_node
                    .generic_params
                    .iter()
                    .find_map(|param| {
                        param
                            .name_if_type_id(ty_id)
                            .map(|param_name| param_name.to_string())
                    })
                    .or_else(|| {
                        union_node
                            .fields
                            .iter()
                            .find(|field| field.type_id == ty_id)
                            .map(|field| field.name.clone())
                            .unwrap_or(Some(format!("Unnamed_field of {}", union_node.name)))
                    }),
            })
            .next()
            .or_else(|| {
                graph
                    .functions
                    .iter()
                    .find(|f| f.return_type.is_some_and(|ret| ret == ty_id))
                    .map(|f| format!("Return type of fn name: {}", f.name))
            });
        found_name
    }
    fn find_name_by_graph_id(graph: &CodeGraph, graph_id: GraphId) -> Option<String> {
        match graph_id {
            GraphId::Node(node_id) => {
                print!("NodeId ");
                find_node_id_name(graph, node_id).map(|n_name| n_name.to_string())
            }
            GraphId::Type(type_id) => {
                print!("TypeId ");
                find_type_id_name(graph, type_id)
            }
        }
        // graph.functions.iter().find(|f| f.id == )
    }
    fn print_all_relations(graph: &CodeGraph) {
        for rel in &graph.relations {
            println!("{:?}: {} -> {}", rel.kind, rel.source, rel.target);
            println!(
                "{}\n",
                format!(
                    "{} -> {}",
                    find_name_by_graph_id(graph, rel.source).unwrap_or("Not Found".to_string()),
                    find_name_by_graph_id(graph, rel.target).unwrap_or("Not Found".to_string())
                )
            );
        }
    }

    // --- Relation Tests ---

    #[test]
    fn test_contains_relation() {
        let crate_name = "example_crate";
        let crate_version = "0.1.0";
        let crate_path = fixtures_crates_dir().join(crate_name);
        // Use workspace root as project root for discovery context
        let project_root = workspace_root();
        let discovery_output: DiscoveryOutput =
            run_discovery_phase(&project_root, &[crate_path.clone()]).unwrap_or_else(|e| {
                panic!(
                    "Phase 1 Discovery failed for fixture '{}': {:?}",
                    crate_name, e
                )
            });
        let code_graphs: Vec<ParsedCodeGraph> = analyze_files_parallel(&discovery_output, 0)
            .iter_mut()
            .map(|res| res.to_owned().unwrap())
            .collect();

        println!("{:-^60}", "");
        println!("{:-^60}", "All Relations");
        println!("{:-^60}", "");
        for code_graph in &code_graphs {
            print_all_relations(&code_graph.graph);
        }
        println!("{:-^60}", "");

        let expect_crate_namespace =
            syn_parser::discovery::derive_crate_namespace(crate_name, crate_version);

        // 1. Module -> Function

        // This expect_mod_two_id is generated from what we expect from the crate context.
        //
        // from visitor/mod.rs
        // let root_module_id = NodeId::generate_synthetic(
        //     crate_namespace, // defined with derive_crate_namespace in `discovery.rs`
        //     file_path, // absolute file path to file
        //     &[], // Empty relative path for crate root
        //     "crate", // somewhat arbitrarily chosen, possibly misleading and should perhaps more
        //     accurately be "root", as "crate" more accurately refers to the "lib.rs" or "main.rs"
        //     file (I think).
        //     (0, 0), // Span, there should be no other items with a (0, 0) span and this makes sense for
        //             // root crate (almost, probably would make more sense as (0, <file byte length>))
        // );
        let expect_mod_two_path = crate_path
            .clone()
            .join("src")
            .join("module_two")
            .join("mod.rs");
        let is_this_equal = crate_path.join("src/module_two/mod.rs");
        assert_eq!(
            expect_mod_two_path, is_this_equal,
            "No you can't use file paths like you want"
        );
        let expect_mod_two_id = NodeId::generate_synthetic(
            expect_crate_namespace,
            &expect_mod_two_path,
            &[],
            "crate",
            (0, 0),
        );
        let code_graph_with_mod_two = code_graphs
            .iter()
            .find(|cg| find_node_id_name(&cg.graph, expect_mod_two_id).is_some());
        assert!(code_graph_with_mod_two.is_some(), "module_two's expected NodeId not found in any CodeGraph:\n\tmodule_two_expected_id: {}",
                expect_mod_two_id
                );
        print!("Looking for module_two in code_graphs...");
    }
}
