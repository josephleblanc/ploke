use ploke_common::{fixtures_crates_dir, workspace_root};
use ploke_core::{NodeId, TypeId};
use syn_parser::discovery::run_discovery_phase;
use syn_parser::parser::graph::CodeGraph;
use syn_parser::parser::relations::RelationKind;
use syn_parser::parser::types::TypeNode;
use syn_parser::parser::visitor::calculate_cfg_hash_bytes;
// Removed `use syn_parser::parser::visitor::ParsedCodeGraph;` - import directly in tests
use syn_parser::parser::{analyze_files_parallel, nodes::*, visitor::ParsedCodeGraph}; // Import ParsedCodeGraph here if needed internally

/// Helper function to run Phase 1 & 2 and collect results
pub fn run_phases_and_collect(fixture_name: &str) -> Vec<ParsedCodeGraph> {
    let crate_path = fixtures_crates_dir().join(fixture_name);
    let project_root = workspace_root(); // Use workspace root for context
    let discovery_output = run_discovery_phase(&project_root, &[crate_path.clone()])
        .unwrap_or_else(|e| panic!("Phase 1 Discovery failed for {}: {:?}", fixture_name, e));

    let results_with_errors: Vec<Result<ParsedCodeGraph, syn::Error>> =
        analyze_files_parallel(&discovery_output, 0); // num_workers ignored by rayon bridge

    // Collect successful results, panicking if any file failed to parse in Phase 2
    results_with_errors
        .into_iter()
        .map(|res| {
            res.unwrap_or_else(|e| {
                panic!(
                    "Phase 2 parsing failed for a file in fixture {}: {:?}",
                    fixture_name, e
                )
            })
        })
        .collect()
}

/// Finds a node ID by its module path and name within a Phase 2 CodeGraph.
/// Assumes ModuleNode.items is populated during Phase 2 parsing for nodes defined in that file.
// NOTE: Cannot find import id reliably. Use `find_import_id` instead.
pub fn find_node_id_by_path_and_name(
    graph: &CodeGraph,
    module_path: &[String], // e.g., ["crate", "outer", "inner"]
    name: &str,
) -> Option<NodeId> {
    // 1. Find the module node corresponding to the path in *this* graph
    let parent_module = graph.modules.iter().find(|m| {
        #[cfg(feature = "verbose_debug")]
        println!(
            "searching for: {:?}\nm.defn_path() = {:?}\nm.path = {:?}
m.name = {}\nm.id = {}\nm.is_file_based() = {}
m.items() = {:#?}",
            module_path,
            m.defn_path(),
            m.path,
            m.name,
            m.id,
            m.is_file_based(),
            m.items()
        );
        (m.defn_path() == module_path && m.is_inline())
            || (m.defn_path() == module_path && m.is_file_based())
    })?;

    // Convert items Vec<NodeId> to a HashSet for faster lookups if needed,
    // though for typical module sizes, linear scan might be fine.
    // NO, DO NOT DO THIS WITHOUT FIRST CHECKING ONLY ONE MATCH
    // let module_item_ids: std::collections::HashSet<_> = parent_module.items.iter().collect();

    // 2. Search functions
    let func_id = graph
        .functions
        .iter()
        .find(|f| f.name() == name && parent_module.items().is_some_and(|m| m.contains(&f.id())))
        .map(|f| f.id());

    if func_id.is_some() {
        return func_id;
    }

    // 3. Search defined types (Struct, Enum, Union, TypeAlias)
    let type_def_id = graph.defined_types.iter().find_map(|td| {
        // Use the GraphNode trait implemented by node types
        if td.name() == name && parent_module.items().is_some_and(|m| m.contains(&td.id())) {
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
        .find(|t| t.name() == name && parent_module.items().is_some_and(|m| m.contains(&t.id())))
        .map(|t| t.id());

    if trait_id.is_some() {
        return trait_id;
    }

    let module_id = graph
        .modules
        .iter()
        .find(|target_m| {
            target_m.name() == name
                && parent_module
                    .items()
                    .is_some_and(|m| m.contains(&target_m.id()))
        })
        .map(|target_m| target_m.id());

    if module_id.is_some() {
        return module_id;
    }

    // ... add searches for other relevant node types that implement GraphNode and belong in ModuleNode.items

    None
}

/// Find an import node in a graph using the graph's modules.items method.
/// Requires an extra field over `find_node_id_by_path_and_name` due to the primary name only being
/// the final token in the `use` statement.
pub fn find_import_id(
    graph: &CodeGraph,
    module_path: &[String], // e.g., ["crate", "outer", "inner"]
    visible_name: &str,
    import_path: &[&str],
) -> Option<NodeId> {
    let parent_module = graph.modules.iter().find(|m| {
        // #[cfg(feature = "verbose_debug")]
        (m.defn_path() == module_path && m.is_inline())
            || (m.defn_path() == module_path && m.is_file_based())
    })?;
    let import_id = graph
        .use_statements
        .iter()
        .find(|import| {
            import.source_path == import_path
                && import
                    .original_name
                    .clone()
                    .or_else(|| {
                        eprintln!("2. ORIGINAL_NAME : {:?}", import.original_name);
                        Some(import.visible_name.clone())
                    })
                    .map(|import_name| import_name == visible_name)
                    .is_some()
                && parent_module
                    .items()
                    .is_some_and(|items| items.contains(&import.id))
        })
        .map(|imp| imp.id);
    import_id
}

pub fn find_node_id_container_mod_paranoid(graph: &CodeGraph, node_id: NodeId) -> Option<NodeId> {
    let count = graph
        .relations
        .iter()
        .filter(|m| m.target == GraphId::Node(node_id))
        .map(|r| match r.source {
            GraphId::Node(node_id) => node_id,
            GraphId::Type(_type_id) => panic!("Should never have type containing node"),
        })
        .count();
    if count != 1 {
        panic!("More than one containing module");
    }
    graph
        .relations
        .iter()
        .find(|m| m.target == GraphId::Node(node_id))
        .map(|r| match r.source {
            GraphId::Node(node_id) => node_id,
            GraphId::Type(_type_id) => panic!("Should never have type containing node"),
        })
}

pub fn find_import_longname_by_id(graph: &CodeGraph, node_id: NodeId) -> Option<String> {
    graph
        .use_statements
        .iter()
        .find(|imp| imp.id == node_id)
        .map(|imp| {
            format!(
                "{}::{}{}",
                imp.source_path.join("::"),
                imp.visible_name,
                if let Some(original_name) = &imp.original_name {
                    format!(" as {}", original_name)
                } else {
                    "".to_string()
                }
            )
        })
}
pub fn find_node_id_name(graph: &CodeGraph, node_id: NodeId) -> Option<String> {
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
    // incomplete, can add more
}
pub fn find_type_id_name(graph: &CodeGraph, ty_id: TypeId) -> Option<String> {
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
                .map(|f| format!("Return type of fn name: {:?}", f.return_type))
        });
    // .or_else(|| graph.impls.iter().find(|imp| imp.self_type == ty_id)
    //     .map(|imp| imp.methods.iter().find(|f| f.))
    // ); // TODO: Build this out more.
    // I think complete? Nope, not complete
    found_name
}
pub fn find_name_by_graph_id(graph: &CodeGraph, graph_id: GraphId) -> Option<String> {
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
pub fn print_all_relations(graph: &CodeGraph) {
    for rel in &graph.relations {
        println!("{:?}: {} -> {}", rel.kind, rel.source, rel.target);
        println!(
            "{} -> {}",
            find_name_by_graph_id(graph, rel.source).unwrap_or("Not Found".to_string()),
            find_name_by_graph_id(graph, rel.target).unwrap_or("Not Found".to_string())
        );
    }
}

// Helper function to run Phase 1 & 2 for a single fixture
pub fn run_phase1_phase2(fixture_name: &str) -> Vec<Result<ParsedCodeGraph, syn::Error>> {
    let crate_path = fixtures_crates_dir().join(fixture_name);
    // Use workspace root as project root for discovery context
    let project_root = workspace_root();
    let discovery_output = run_discovery_phase(&project_root, &[crate_path]).unwrap_or_else(|e| {
        panic!(
            "Phase 1 Discovery failed for fixture '{}': {:?}",
            fixture_name, e
        )
    });
    analyze_files_parallel(&discovery_output, 0)
}

// Helper to find the TypeId of a function's parameter by index
pub fn find_param_type_id(
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
pub fn find_return_type_id(graph: &CodeGraph, func_id: NodeId) -> Option<TypeId> {
    graph
        .functions
        .iter()
        .find(|f| f.id == func_id)
        .and_then(|f| f.return_type)
}

// Helper to find the TypeId of a struct field
pub fn find_field_type_id(graph: &CodeGraph, field_id: NodeId) -> Option<TypeId> {
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
pub fn find_impl_self_type_id(graph: &CodeGraph, impl_id: NodeId) -> Option<TypeId> {
    graph
        .impls
        .iter()
        .find(|i| i.id == impl_id)
        .map(|i| i.self_type)
}

// Helper to find the TypeId of an impl's trait_type
pub fn find_impl_trait_type_id(graph: &CodeGraph, impl_id: NodeId) -> Option<TypeId> {
    graph
        .impls
        .iter()
        .find(|i| i.id == impl_id)
        .and_then(|i| i.trait_type)
}

// Core assertion helper
pub fn assert_relation_exists(
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
pub fn assert_relation_does_not_exist(
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

pub fn find_inline_module_by_path<'a>(
    graph: &'a CodeGraph,
    module_path: &[String],
) -> Option<&'a ModuleNode> {
    let mut modules = graph
        .modules
        .iter()
        .filter(|m| m.defn_path() == module_path);
    let found = modules.next();
    let mut errs = Vec::new();
    for unexpected_module in modules {
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

pub fn find_mod_decl_by_path_and_name<'a>(
    graph: &'a CodeGraph,
    module_path: &[String],
    name: &str,
) -> Option<&'a ModuleNode> {
    let mut modules = graph
        .modules
        .iter()
        .filter(|m| m.is_declaration() && m.name() == name && m.path == module_path);
    let found = modules.next();
    let mut errs = Vec::new();
    for unexpected_module in modules {
        errs.push(unexpected_module);
    }
    if !errs.is_empty() {
        panic!(
            "Mutiple module declarations found with same path.
  First module found: {:?}
  Other modules found: {:?}",
            found, errs
        );
    }
    found
}

/// Finds the specific ParsedCodeGraph for the target file, then finds the FunctionNode
/// within that graph, performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_function_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/func/return_types.rs"
    expected_module_path: &[String],      // Module path within the target file
    func_name: &str,
) -> &'a FunctionNode {
    // 1. Construct the absolute expected file path
    let fixture_root = fixtures_crates_dir().join(fixture_name);
    let target_file_path = fixture_root.join(relative_file_path);

    // 2. Find the specific ParsedCodeGraph for the target file
    let target_data = parsed_graphs
        .iter()
        .find(|data| data.file_path == target_file_path)
        .unwrap_or_else(|| {
            panic!(
                "ParsedCodeGraph for '{}' not found in results",
                target_file_path.display()
            )
        });

    let graph = &target_data.graph;
    let crate_namespace = target_data.crate_namespace;
    let file_path = &target_data.file_path; // Use the path from the found graph data

    // 3. Filter candidates by name within the target graph
    let name_candidates: Vec<&FunctionNode> = graph
        .functions
        .iter()
        .chain(graph.impls.iter().flat_map(|imp| &imp.methods))
        .filter(|f| f.name() == func_name)
        .collect();
    // let method_candidates: Vec<&FunctionNode> = graph.impls.iter().map(|imp| imp.methods).flatten()

    assert!(
        !name_candidates.is_empty(),
        "No FunctionNode found with name '{}' in file '{}'",
        func_name,
        file_path.display()
    );

    // 4. Filter further by module association within the target graph
    let module_node = graph
        .modules
        .iter()
        .find(|m| m.path == expected_module_path)
        .unwrap_or_else(|| {
            panic!(
                "ModuleNode not found for path: {:?} in file '{}'",
                expected_module_path,
                file_path.display()
            )
        });

    let module_candidates: Vec<&FunctionNode> = name_candidates
        .into_iter()
        .filter(|f| module_node.items().is_some_and(|m| m.contains(&f.id())))
        .collect();

    // 5. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one FunctionNode named '{}' associated with module path {:?} in file '{}', found {}",
        func_name,
        expected_module_path,
        file_path.display(),
        module_candidates.len()
    );

    let func_node = module_candidates[0];
    let func_id = func_node.id();

    // 6. PARANOID CHECK: Regenerate expected ID using node's context and ItemKind
    //    Find the actual parent scope ID from the 'Contains' relation in the graph.
    let actual_parent_scope_id = graph
        .relations
        .iter()
        .find(|rel| {
            rel.target == GraphId::Node(func_id)
                && (rel.kind == RelationKind::Contains || rel.kind == RelationKind::Method)
        })
        .map(|rel| match rel.source {
            GraphId::Node(id) => Some(id),
            GraphId::Type(_) => {
                panic!("'Contains' relation source cannot be a TypeId for a Node target")
            }
        })
        .unwrap_or_else(|| {
            // If no Contains relation found, it might be the root module's content,
            // or something went wrong during parsing. For paranoia, assume None is correct only if it's top-level.
            // A more robust check might be needed depending on how root items are handled.
            // For now, let's assume top-level items in a file *should* have the file's module as parent.
            panic!(
                "Could not find 'Contains' relation for function '{}' ({}) in file '{}'",
                func_name,
                func_id,
                file_path.display()
            );
            // If we expect items directly under crate root with no module parent:
            // if expected_module_path == ["crate"] { None } else { panic!(...) }
        });

    // 7. PARANOID CHECK: Calculate expected CFG hash bytes
    let item_cfgs = &func_node.cfgs; // Get the function's own CFGs
    let scope_cfgs: Vec<String> = actual_parent_scope_id
        .and_then(|p_id| graph.find_node(p_id)) // Find the parent node
        .map(|p_node| p_node.cfgs().to_vec()) // Get the parent's CFGs using the GraphNode trait method
        .unwrap_or_default(); // Default to empty if no parent found (e.g., root module items)

    let mut provisional_effective_cfgs: Vec<String> = scope_cfgs // Combine parent and item cfgs
        .iter()
        .cloned()
        .chain(item_cfgs.iter().cloned())
        .collect();
    provisional_effective_cfgs.sort_unstable(); // Sort for deterministic hashing input

    let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs); // Hash the sorted, combined list

    // 8. Regenerate ID *with* calculated CFG bytes
    let regenerated_id = NodeId::generate_synthetic(
        // Renamed back to regenerated_id for consistency
        crate_namespace,
        file_path,            // Use the file_path from the target_data
        expected_module_path, // Still use the expected module path for context hashing
        func_name,
        ploke_core::ItemKind::Function, // Pass the correct ItemKind
        actual_parent_scope_id, // Pass the parent scope ID found via the Contains/Method relation
        cfg_bytes.as_deref(),   // Pass the calculated bytes
    );

    // 9. Assert Regenerated ID Matches Actual ID
    let possible_parent = actual_parent_scope_id.and_then(|id| graph.find_node(id));
    assert_eq!(
        func_id, regenerated_id, // Compare actual ID with the regenerated ID (which now includes CFG)
        "PARANOID CHECK FAILED: Mismatch between node's actual ID ({}) and regenerated ID ({}) for function '{}' in file '{}'.\nExpected Module Path: {:?}\nParent Scope ID: {:?}\nParent Node Name: {}\nScope CFGs (Parent): {:?}\nItem CFGs (Function): {:?}\nCombined & Sorted CFGs for Hash: {:?}\nFOUND FUNCTION NODE: {:#?}",
        func_id,
        regenerated_id,
        func_name,
        file_path.display(),
        expected_module_path,
        actual_parent_scope_id,
        possible_parent.map(|n| n.name()).unwrap_or("<None>"), // Safely get parent name
        scope_cfgs, // Include scope CFGs in assertion message
        item_cfgs, // Include item CFGs in assertion message
        provisional_effective_cfgs, // Include the actual list used for hashing
        func_node // Print the found node for debugging
    );

    // 10. Return the validated node
    func_node
}

/// Helper to find a TypeNode by its ID. Panics if not found.
pub fn find_type_node(graph: &CodeGraph, type_id: TypeId) -> &TypeNode {
    graph
        .type_graph
        .iter()
        .find(|tn| tn.id == type_id)
        .unwrap_or_else(|| panic!("TypeNode not found for TypeId: {}", type_id))
}

/// Finds a method (FunctionNode) within a specific impl or trait block, performs paranoid checks,
/// and returns a reference to it.
///
/// This helper leverages `find_impl_node_paranoid` or `find_trait_node_paranoid` to locate
/// the parent scope first, ensuring the parent context is correct before searching for the method.
///
/// Panics if the graph, parent node, or method is not found, or if uniqueness or ID checks fail.
pub fn find_method_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/impls.rs"
    expected_module_path: &[String],      // Module path containing the impl/trait block
    parent_context: MethodParentContext<'a>, // Specifies the parent impl or trait
    method_name: &str,                    // Name of the method to find
) -> &'a FunctionNode {
    // --- 1. Find the ParsedCodeGraph (Common logic) ---
    let fixture_root = fixtures_crates_dir().join(fixture_name);
    let target_file_path = fixture_root.join(relative_file_path);
    let target_data = parsed_graphs
        .iter()
        .find(|data| data.file_path == target_file_path)
        .unwrap_or_else(|| {
            panic!(
                "ParsedCodeGraph for '{}' not found in results",
                target_file_path.display()
            )
        });
    let crate_namespace = target_data.crate_namespace;
    let file_path = &target_data.file_path;

    // --- 2. Find Parent and Method, Ensuring Uniqueness ---
    let parent_node_id: NodeId; // Will be assigned within the match
    let method_node: &'a FunctionNode; // Will be assigned within the match

    match parent_context {
        MethodParentContext::Impl {
            self_type_str,
            trait_type_str,
        } => {
            // Use the existing paranoid helper to find the specific impl block
            let impl_node = super::paranoid::impl_helpers::find_impl_node_paranoid(
                parsed_graphs,
                fixture_name,
                relative_file_path,
                expected_module_path,
                self_type_str,
                trait_type_str,
            );
            parent_node_id = impl_node.id(); // Assign parent ID

            // Find method candidates by name within this impl block
            let method_candidates: Vec<&FunctionNode> = impl_node
                .methods
                .iter()
                .filter(|m| m.name() == method_name)
                .collect();

            // PARANOID CHECK: Assert exactly ONE method matches the name within this impl
            assert_eq!(
                method_candidates.len(),
                1,
                "Expected exactly one method named '{}' within parent impl scope {:?} (context: {:?}) in file '{}', found {}",
                method_name,
                parent_node_id,
                parent_context,
                file_path.display(),
                method_candidates.len()
            );
            method_node = method_candidates[0]; // Assign the unique method node
        }
        MethodParentContext::Trait { trait_name } => {
            // Use the existing paranoid helper to find the specific trait block
            let trait_node = super::paranoid::trait_helpers::find_trait_node_paranoid(
                parsed_graphs,
                fixture_name,
                relative_file_path,
                expected_module_path,
                trait_name,
            );
            parent_node_id = trait_node.id(); // Assign parent ID

            // Find method candidates by name within this trait block
            let method_candidates: Vec<&FunctionNode> = trait_node
                .methods
                .iter()
                .filter(|m| m.name() == method_name)
                .collect();

            // PARANOID CHECK: Assert exactly ONE method matches the name within this trait
            assert_eq!(
                method_candidates.len(),
                1,
                "Expected exactly one method named '{}' within parent trait scope {:?} (context: {:?}) in file '{}', found {}",
                method_name,
                parent_node_id,
                parent_context,
                file_path.display(),
                method_candidates.len()
            );
            method_node = method_candidates[0]; // Assign the unique method node
        }
    };

    // --- 3. PARANOID CHECK: Regenerate Method's NodeId ---
    // Now that we have the unique method_node and its parent_node_id
    let method_id = method_node.id();

    // --- 4. Calculate expected CFG hash bytes ---
    let item_cfgs = &method_node.cfgs; // Get the method's own CFGs
                                       // Explicitly use target_data.graph to avoid potential shadowing issues
    let scope_cfgs: Vec<String> = target_data.graph
        .find_node(parent_node_id) // Find the parent impl/trait node
        .map(|p_node| p_node.cfgs().to_vec()) // Get the parent's CFGs
        .unwrap_or_else(|| {
            eprintln!("Warning: Could not find parent node {:?} to get scope CFGs for method '{}'. Defaulting to empty.", parent_node_id, method_name);
            Vec::new() // Default to empty if parent not found
        });

    let mut provisional_effective_cfgs: Vec<String> = scope_cfgs // Renamed to avoid shadowing
        .iter()
        .cloned()
        .chain(item_cfgs.iter().cloned())
        .collect();
    provisional_effective_cfgs.sort_unstable(); // Sort before hashing
    let cfg_bytes = calculate_cfg_hash_bytes(&provisional_effective_cfgs);

    // --- 5. Regenerate ID *with* calculated CFG bytes ---
    // The context for the method's ID includes the module path containing the parent impl/trait,
    // the method's name, its kind, the parent impl/trait's ID, and the combined CFG hash.
    let regenerated_id_with_cfg = NodeId::generate_synthetic(
        crate_namespace,
        file_path,
        expected_module_path, // Module path containing the parent impl/trait
        method_name,
        ploke_core::ItemKind::Function, // Methods are functions
        Some(parent_node_id),           // Parent scope is the impl/trait ID found above
        cfg_bytes.as_deref(),           // Pass the calculated bytes
    );

    // --- 6. Assert Regenerated ID Matches Actual ID ---
    assert_eq!(
        method_id, regenerated_id_with_cfg, // Compare with the ID generated *with* CFG context
        "Mismatch between method's actual ID ({}) and regenerated ID ({}) for method '{}' in parent {:?} (context: {:?}) file '{}'.\nScopeCFGs: {:?}\nItemCFGs: {:?}\nCombinedCFGs: {:?}",
        method_id, regenerated_id_with_cfg, method_name, parent_node_id, parent_context, file_path.display(), scope_cfgs, item_cfgs, provisional_effective_cfgs // Use renamed variable
    );

    // --- 7. Return the validated method node ---
    method_node
}

/// Specifies the context (impl or trait) containing the method being searched for.
#[derive(Debug, Clone, Copy)] // Added derive for Debug, Clone, Copy
pub enum MethodParentContext<'a> {
    Impl {
        /// The string representation of the `self` type of the impl block.
        self_type_str: &'a str,
        /// Optional string representation of the trait type, if it's a trait impl.
        trait_type_str: Option<&'a str>,
    },
    Trait {
        /// The name of the trait containing the method.
        trait_name: &'a str,
    },
}
