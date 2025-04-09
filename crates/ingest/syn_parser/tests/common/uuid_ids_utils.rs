#![cfg(feature = "uuid_ids")]

use ploke_common::{fixtures_crates_dir, workspace_root};
use ploke_core::{NodeId, TypeId};
use syn_parser::discovery::run_discovery_phase;
use syn_parser::parser::graph::CodeGraph;
use syn_parser::parser::relations::{GraphId, RelationKind};
use syn_parser::parser::types::{TypeKind, TypeNode};
use syn_parser::parser::visitor::ParsedCodeGraph;
use syn_parser::parser::{analyze_files_parallel, nodes::*};

/// Finds a node ID by its module path and name within a Phase 2 CodeGraph.
/// Assumes ModuleNode.items is populated during Phase 2 parsing for nodes defined in that file.
pub fn find_node_id_by_path_and_name(
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
pub fn find_import_longname_by_id(graph: &CodeGraph, node_id: NodeId) -> Option<String> {
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
                .map(|f| format!("Return type of fn name: {}", f.name))
        });
    // I think complete?
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
            "{}\n",
            format!(
                "{} -> {}",
                find_name_by_graph_id(graph, rel.source).unwrap_or("Not Found".to_string()),
                find_name_by_graph_id(graph, rel.target).unwrap_or("Not Found".to_string())
            )
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
        .filter(|f| f.name() == func_name)
        .collect();

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
        .filter(|f| module_node.items.contains(&f.id()))
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
    let actual_span = func_node.span; // Get span from the found node

    // 6. PARANOID CHECK: Regenerate expected ID using node's actual span and context
    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        expected_module_path,
        func_name,
        actual_span, // Use the span from the node itself
    );

    assert_eq!(
        func_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for function '{}' in file '{}' with span {:?}",
        func_id, regenerated_id, func_name, file_path.display(), actual_span
    );

    // 7. Return the validated node
    func_node
}

/// Helper to find a TypeNode by its ID. Panics if not found.
pub fn find_type_node<'a>(graph: &'a CodeGraph, type_id: TypeId) -> &'a TypeNode {
    graph
        .type_graph
        .iter()
        .find(|tn| tn.id == type_id)
        .unwrap_or_else(|| panic!("TypeNode not found for TypeId: {}", type_id))
}

/// Finds the specific ParsedCodeGraph for the target file, then finds the StructNode
/// within that graph, performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_struct_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/structs.rs"
    expected_module_path: &[String],      // Module path within the target file
    struct_name: &str,
) -> &'a StructNode {
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

    // 3. Filter candidates by name and type within the target graph
    let name_candidates: Vec<&StructNode> = graph
        .defined_types
        .iter()
        .filter_map(|td| match td {
            TypeDefNode::Struct(s) if s.name() == struct_name => Some(s),
            _ => None,
        })
        .collect();

    assert!(
        !name_candidates.is_empty(),
        "No StructNode found with name '{}' in file '{}'",
        struct_name,
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

    let module_candidates: Vec<&StructNode> = name_candidates
        .into_iter()
        .filter(|s| module_node.items.contains(&s.id()))
        .collect();

    // 5. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one StructNode named '{}' associated with module path {:?} in file '{}', found {}",
        struct_name,
        expected_module_path,
        file_path.display(),
        module_candidates.len()
    );

    let struct_node = module_candidates[0];
    let struct_id = struct_node.id();
    let actual_span = struct_node.span; // Get span from the found node

    // 6. PARANOID CHECK: Regenerate expected ID using node's actual span and context
    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        expected_module_path,
        struct_name,
        actual_span, // Use the span from the node itself
    );

    assert_eq!(
        struct_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for struct '{}' in file '{}' with span {:?}",
        struct_id, regenerated_id, struct_name, file_path.display(), actual_span
    );

    // 7. Return the validated node
    struct_node
}

/// Finds the specific ParsedCodeGraph for the target file, then finds the EnumNode
/// within that graph, performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_enum_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/enums.rs"
    expected_module_path: &[String],      // Module path within the target file
    enum_name: &str,
) -> &'a EnumNode {
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

    // 3. Filter candidates by name and type within the target graph
    let name_candidates: Vec<&EnumNode> = graph
        .defined_types
        .iter()
        .filter_map(|td| match td {
            TypeDefNode::Enum(e) if e.name() == enum_name => Some(e),
            _ => None,
        })
        .collect();

    assert!(
        !name_candidates.is_empty(),
        "No EnumNode found with name '{}' in file '{}'",
        enum_name,
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

    let module_candidates: Vec<&EnumNode> = name_candidates
        .into_iter()
        .filter(|e| module_node.items.contains(&e.id()))
        .collect();

    // 5. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one EnumNode named '{}' associated with module path {:?} in file '{}', found {}",
        enum_name,
        expected_module_path,
        file_path.display(),
        module_candidates.len()
    );

    let enum_node = module_candidates[0];
    let enum_id = enum_node.id();
    let actual_span = enum_node.span; // Get span from the found node

    // 6. PARANOID CHECK: Regenerate expected ID using node's actual span and context
    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        expected_module_path,
        enum_name,
        actual_span, // Use the span from the node itself
    );

    assert_eq!(
        enum_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for enum '{}' in file '{}' with span {:?}",
        enum_id, regenerated_id, enum_name, file_path.display(), actual_span
    );

    // 7. Return the validated node
    enum_node
}

/// Finds the specific ParsedCodeGraph for the target file, then finds the TypeAliasNode
/// within that graph, performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_type_alias_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/type_alias.rs"
    expected_module_path: &[String],      // Module path within the target file
    alias_name: &str,
) -> &'a TypeAliasNode {
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

    // 3. Filter candidates by name and type within the target graph
    let name_candidates: Vec<&TypeAliasNode> = graph
        .defined_types
        .iter()
        .filter_map(|td| match td {
            TypeDefNode::TypeAlias(ta) if ta.name() == alias_name => Some(ta),
            _ => None,
        })
        .collect();

    assert!(
        !name_candidates.is_empty(),
        "No TypeAliasNode found with name '{}' in file '{}'",
        alias_name,
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

    let module_candidates: Vec<&TypeAliasNode> = name_candidates
        .into_iter()
        .filter(|ta| module_node.items.contains(&ta.id()))
        .collect();

    // 5. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one TypeAliasNode named '{}' associated with module path {:?} in file '{}', found {}",
        alias_name,
        expected_module_path,
        file_path.display(),
        module_candidates.len()
    );

    let type_alias_node = module_candidates[0];
    let alias_id = type_alias_node.id();
    let actual_span = type_alias_node.span; // Get span from the found node

    // 6. PARANOID CHECK: Regenerate expected ID using node's actual span and context
    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        expected_module_path,
        alias_name,
        actual_span, // Use the span from the node itself
    );

    assert_eq!(
        alias_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for type alias '{}' in file '{}' with span {:?}",
        alias_id, regenerated_id, alias_name, file_path.display(), actual_span
    );

    // 7. Return the validated node
    type_alias_node
}

/// Finds the specific ParsedCodeGraph for the target file, then finds the UnionNode
/// within that graph, performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_union_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/unions.rs"
    expected_module_path: &[String],      // Module path within the target file
    union_name: &str,
) -> &'a UnionNode {
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

    // 3. Filter candidates by name and type within the target graph
    let name_candidates: Vec<&UnionNode> = graph
        .defined_types
        .iter()
        .filter_map(|td| match td {
            TypeDefNode::Union(u) if u.name() == union_name => Some(u),
            _ => None,
        })
        .collect();

    assert!(
        !name_candidates.is_empty(),
        "No UnionNode found with name '{}' in file '{}'",
        union_name,
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

    let module_candidates: Vec<&UnionNode> = name_candidates
        .into_iter()
        .filter(|u| module_node.items.contains(&u.id()))
        .collect();

    // 5. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one UnionNode named '{}' associated with module path {:?} in file '{}', found {}",
        union_name,
        expected_module_path,
        file_path.display(),
        module_candidates.len()
    );

    let union_node = module_candidates[0];
    let union_id = union_node.id();
    let actual_span = union_node.span; // Get span from the found node

    // 6. PARANOID CHECK: Regenerate expected ID using node's actual span and context
    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        expected_module_path,
        union_name,
        actual_span, // Use the span from the node itself
    );

    assert_eq!(
        union_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for union '{}' in file '{}' with span {:?}",
        union_id, regenerated_id, union_name, file_path.display(), actual_span
    );

    // 7. Return the validated node
    union_node
}

/// Finds the specific ParsedCodeGraph for the target file, then finds the TraitNode
/// within that graph, performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_trait_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/traits.rs"
    expected_module_path: &[String],      // Module path within the target file
    trait_name: &str,
) -> &'a TraitNode {
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
    // Traits are stored directly in graph.traits (assuming public/crate) or graph.private_traits
    let name_candidates: Vec<&TraitNode> = graph
        .traits // Check public/crate traits first
        .iter()
        .chain(graph.private_traits.iter()) // Then check private traits
        .filter(|t| t.name() == trait_name)
        .collect();

    assert!(
        !name_candidates.is_empty(),
        "No TraitNode found with name '{}' in file '{}'",
        trait_name,
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

    let module_candidates: Vec<&TraitNode> = name_candidates
        .into_iter()
        .filter(|t| module_node.items.contains(&t.id()))
        .collect();

    // 5. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one TraitNode named '{}' associated with module path {:?} in file '{}', found {}",
        trait_name,
        expected_module_path,
        file_path.display(),
        module_candidates.len()
    );

    let trait_node = module_candidates[0];
    let trait_id = trait_node.id();
    let actual_span = trait_node.span; // Get span from the found node

    // 6. PARANOID CHECK: Regenerate expected ID using node's actual span and context
    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path, // Use the file_path from the target_data
        expected_module_path,
        trait_name,
        actual_span, // Use the span from the node itself
    );

    assert_eq!(
        trait_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for trait '{}' in file '{}' with span {:?}",
        trait_id, regenerated_id, trait_name, file_path.display(), actual_span
    );

    // 7. Return the validated node
    trait_node
}

/// Helper to find a TypeId based on its string representation in the type_map.
/// NOTE: This relies on the exact string generated by `type_to_string`.
fn find_type_id_by_string(graph: &CodeGraph, type_str: &str) -> Option<TypeId> {
    // We need access to the type_map used during parsing. Since it's not stored
    // on CodeGraph, we have to reverse lookup: find TypeNode by string, then get its ID.
    // This is inefficient but necessary for testing without modifying CodeGraph structure.
    graph.type_graph.iter().find_map(|tn| {
        // Reconstruct the string representation from TypeKind for comparison
        // This is complex and might not perfectly match the original type_to_string key.
        // A simpler approach for testing might be to find nodes *using* the type
        // and get the TypeId from there, if the type string itself isn't stored on TypeNode.

        // Let's try finding a TypeNode whose Unknown kind matches the string for now.
        // This will only work if the type wasn't successfully parsed into another Kind.
        if let TypeKind::Unknown {
            type_str: stored_str,
        } = &tn.kind
        {
            if stored_str == type_str {
                return Some(tn.id);
            }
        }
        // TODO: Add more robust lookup based on reconstructing string from other TypeKinds if needed.
        None
    })
    // A more reliable way might be needed if the above fails often, perhaps searching
    // function params/returns, struct fields etc. that use the type string.
}

/// Finds the specific ParsedCodeGraph for the target file, then finds the ImplNode
/// within that graph based on type info, performs paranoid checks, and returns a reference.
/// Panics if the graph or node is not found, or if uniqueness checks fail.
pub fn find_impl_node_paranoid<'a>(
    parsed_graphs: &'a [ParsedCodeGraph], // Operate on the collection
    fixture_name: &str,                   // Needed to construct expected path
    relative_file_path: &str,             // e.g., "src/lib.rs" or "src/impls.rs"
    expected_module_path: &[String],      // Module path within the target file
    self_type_str: &str,                  // Expected string representation of the self type
    trait_type_str: Option<&str>,         // Expected string representation of the trait type
) -> &'a ImplNode {
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

    // 3. Find the TypeIds corresponding to the type strings
    //    NOTE: This uses the potentially fragile find_type_id_by_string helper.
    let self_type_id = find_type_id_by_string(graph, self_type_str).unwrap_or_else(|| {
        // Attempt fallback: Find a TypeNode with a matching Named path if Unknown fails
        graph
            .type_graph
            .iter()
            .find_map(|tn| {
                if let TypeKind::Named { path, .. } = &tn.kind {
                    // Simple comparison assuming single segment path for basic types
                    if path.len() == 1 && path[0] == self_type_str {
                        return Some(tn.id);
                    }
                    // TODO: Handle multi-segment paths if needed
                }
                None
            })
            .unwrap_or_else(|| panic!("TypeId not found for self_type_str: '{}'", self_type_str))
    });

    let trait_type_id: Option<TypeId> = trait_type_str.map(|tts| {
        find_type_id_by_string(graph, tts).unwrap_or_else(|| {
            graph
                .type_graph
                .iter()
                .find_map(|tn| {
                    if let TypeKind::Named { path, .. } = &tn.kind {
                        if path.len() == 1 && path[0] == tts {
                            return Some(tn.id);
                        }
                    }
                    None
                })
                .unwrap_or_else(|| panic!("TypeId not found for trait_type_str: '{}'", tts))
        })
    });

    // 4. Filter candidates by matching self_type and trait_type IDs
    let type_candidates: Vec<&ImplNode> = graph
        .impls
        .iter()
        .filter(|imp| imp.self_type == self_type_id && imp.trait_type == trait_type_id)
        .collect();

    assert!(
        !type_candidates.is_empty(),
        "No ImplNode found matching self_type '{}' ({:?}) and trait_type '{:?}' ({:?}) in file '{}'",
        self_type_str, self_type_id, trait_type_str, trait_type_id, file_path.display()
    );

    // 5. Filter further by module association
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

    let module_candidates: Vec<&ImplNode> = type_candidates
        .into_iter()
        .filter(|imp| module_node.items.contains(&imp.id()))
        .collect();

    // 6. PARANOID CHECK: Assert exactly ONE candidate remains
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one ImplNode matching types and associated with module path {:?} in file '{}', found {}",
        expected_module_path,
        file_path.display(),
        module_candidates.len()
    );

    let impl_node = module_candidates[0];
    let impl_id = impl_node.id();
    let actual_span = impl_node.span;

    // 7. PARANOID CHECK: Regenerate expected ID using node's actual span and context
    //    Need to generate the expected name based on type strings.
    let expected_name = match trait_type_str {
        Some(t) => format!("impl {} for {}", t, self_type_str),
        None => format!("impl {}", self_type_str),
    };
    // Note: This name generation might differ slightly from the visitor if to_string() representations vary.
    // It assumes simple type strings are sufficient.

    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path,
        expected_module_path,
        &expected_name, // Use the generated name
        actual_span,
    );

    // We compare the regenerated ID against the actual ID found on the node.
    // NOTE: This check might be brittle if the `expected_name` generation here
    // doesn't perfectly match the one used inside the visitor's `add_contains_rel` call,
    // especially for generic types where `to_string()` adds spaces.
    // Consider relaxing this specific check if it proves too fragile due to name generation differences.
    assert_eq!(
        impl_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for impl block '{}' in file '{}' with span {:?}. Name generation might be the cause.",
        impl_id, regenerated_id, expected_name, file_path.display(), actual_span
    );

    // 8. Return the validated node
    impl_node
}
