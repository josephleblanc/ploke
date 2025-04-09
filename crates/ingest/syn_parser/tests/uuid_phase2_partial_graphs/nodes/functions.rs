#![cfg(feature = "uuid_ids")]
#![cfg(test)]

use crate::common::uuid_ids_utils::*;
use ploke_core::{NodeId, TypeId}; // Remove VisibilityKind from here
use syn_parser::{
    parser::{
        graph::CodeGraph,
        nodes::VisibilityKind, // Import VisibilityKind from its correct location
        nodes::{FunctionNode, ParamData, TypeDefNode, Visible},
        types::{TypeKind, TypeNode},
        visitor::FileParseOutput, // Use the new return type (ensure this is pub in visitor/mod.rs)
    },
    // PROJECT_NAMESPACE_UUID is not needed here and not public
};
use uuid::Uuid;

// --- Helper Functions ---

/// Runs Phase 1 & 2 and extracts the single FileParseOutput for a single-file fixture.
/// Panics if parsing fails or the fixture generates more than one output.
fn get_single_file_parse_output(fixture_name: &str) -> FileParseOutput {
    let results = run_phase1_phase2(fixture_name);
    assert_eq!(
        results.len(),
        1,
        "Expected exactly one FileParseOutput for fixture '{}'",
        fixture_name
    );
    results
        .into_iter()
        .next()
        .expect("Failed to get first result")
        .expect("Parsing failed") // Panic if Err
}

/// Finds a FunctionNode, performs paranoid checks, and returns a reference.
/// Panics if the node is not found or if uniqueness checks fail.
fn find_function_node_paranoid<'a>(
    graph: &'a CodeGraph,
    crate_namespace: Uuid,
    file_path: &std::path::Path,
    expected_module_path: &[String],
    func_name: &str,
    // expected_span: (usize, usize), // Use span from node itself
) -> &'a FunctionNode {
    // 1. Filter candidates by name first
    let name_candidates: Vec<&FunctionNode> = graph
        .functions
        .iter()
        .filter(|f| f.name() == func_name)
        .collect();

    assert!(
        !name_candidates.is_empty(),
        "No FunctionNode found with name '{}'",
        func_name
    );

    // 2. Filter further by module association (workaround for direct ID regen)
    let module_node = graph
        .modules
        .iter()
        .find(|m| m.path == expected_module_path)
        .unwrap_or_else(|| {
            panic!(
                "ModuleNode not found for path: {:?}",
                expected_module_path
            )
        });

    let module_candidates: Vec<&FunctionNode> = name_candidates
        .into_iter()
        .filter(|f| module_node.items.contains(&f.id()))
        .collect();

    // 3. PARANOID CHECK: Assert exactly ONE candidate remains after filtering by module
    assert_eq!(
        module_candidates.len(),
        1,
        "Expected exactly one FunctionNode named '{}' associated with module path {:?}, found {}",
        func_name,
        expected_module_path,
        module_candidates.len()
    );

    let func_node = module_candidates[0];
    let func_id = func_node.id();
    let actual_span = func_node.span; // Get span from the found node

    // 4. PARANOID CHECK: Regenerate expected ID using node's actual span and context
    let regenerated_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path,
        expected_module_path,
        func_name,
        actual_span, // Use the span from the node itself
    );

    assert_eq!(
        func_id, regenerated_id,
        "Mismatch between node's actual ID ({}) and regenerated ID ({}) for function '{}' with span {:?}",
        func_id, regenerated_id, func_name, actual_span
    );

    // 5. Return the validated node
    func_node
}

/// Helper to find a TypeNode by its ID. Panics if not found.
fn find_type_node<'a>(graph: &'a CodeGraph, type_id: TypeId) -> &'a TypeNode {
    graph
        .type_graph
        .iter()
        .find(|tn| tn.id == type_id)
        .unwrap_or_else(|| panic!("TypeNode not found for TypeId: {}", type_id))
}

// --- Test Cases ---

#[test]
fn test_function_node_process_tuple() {
    let fixture = "fixture_types";
    let parsed_output = get_single_file_parse_output(fixture);
    let graph = &parsed_output.graph;
    let crate_namespace = parsed_output.crate_namespace;
    let file_path = &parsed_output.file_path;

    let func_name = "process_tuple";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        graph,
        crate_namespace,
        file_path,
        &module_path,
        func_name,
        // Span determined by find_function_node_paranoid
    );

    // Assertions
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(func_node.tracking_hash.is_some());
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.docstring.is_none());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.generic_params.is_empty());
    // assert!(func_node.body_str.is_some()); // FunctionNode doesn't expose body_str

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("p")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (Point -> (i32, i32))
    let param_type_node = find_type_node(graph, param.type_id);
    // Current state: Falls back to Unknown because TypeKind::Path processing doesn't fully resolve aliases yet
    assert!(
        matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Point"]),
        "Expected TypeKind::Named for alias 'Point', found {:?}", param_type_node.kind
    );
    // TODO: Once alias resolution is deeper, this should check the underlying tuple type.

    // Return Type
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));

    // Check return TypeNode (i32)
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(
        matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]),
       "Expected TypeKind::Named for 'i32', found {:?}", return_type_node.kind
    );
}

#[test]
fn test_function_node_process_slice() {
    let fixture = "fixture_types";
    let parsed_output = get_single_file_parse_output(fixture);
    let graph = &parsed_output.graph;
    let crate_namespace = parsed_output.crate_namespace;
    let file_path = &parsed_output.file_path;

    let func_name = "process_slice";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        graph,
        crate_namespace,
        file_path,
        &module_path,
        func_name,
    );

    // Assertions
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(func_node.tracking_hash.is_some());
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.docstring.is_none());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.generic_params.is_empty());
    // assert!(func_node.body_str.is_some()); // FunctionNode doesn't expose body_str

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("s")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (&[u8])
    let param_type_node = find_type_node(graph, param.type_id);
    // Current state: Falls back to Unknown because TypeKind::Slice not implemented
     assert!(
         matches!(param_type_node.kind, TypeKind::Unknown { type_str } if type_str == "& [u8]"),
         "Expected TypeKind::Unknown for '&[u8]' currently, found {:?}", param_type_node.kind
     );
    #[ignore = "TypeKind::Slice not yet handled in type_processing.rs"]
    {
        // Target state assertion (will fail until implemented)
        // assert!(matches!(param_type_node.kind, TypeKind::Slice { .. }));
        // assert_eq!(param_type_node.related_types.len(), 1); // Should relate to u8
    }


    // Return Type
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));

    // Check return TypeNode (usize)
    let return_type_node = find_type_node(graph, return_type_id);
     assert!(
         matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["usize"]),
        "Expected TypeKind::Named for 'usize', found {:?}", return_type_node.kind
     );
}

#[test]
fn test_function_node_process_array() {
    let fixture = "fixture_types";
    let parsed_output = get_single_file_parse_output(fixture);
    let graph = &parsed_output.graph;
    let crate_namespace = parsed_output.crate_namespace;
    let file_path = &parsed_output.file_path;

    let func_name = "process_array";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        graph,
        crate_namespace,
        file_path,
        &module_path,
        func_name,
    );

    // Assertions
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(func_node.tracking_hash.is_some());
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("a")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (Buffer -> [u8; 1024])
    let param_type_node = find_type_node(graph, param.type_id);
     assert!(
         matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Buffer"]),
         "Expected TypeKind::Named for alias 'Buffer', found {:?}", param_type_node.kind
     );
    // TODO: Deeper check for underlying array type once alias resolution is better

    // Return Type
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));

    // Check return TypeNode (u8)
    let return_type_node = find_type_node(graph, return_type_id);
     assert!(
         matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["u8"]),
        "Expected TypeKind::Named for 'u8', found {:?}", return_type_node.kind
     );
}

#[test]
fn test_function_node_process_ref() {
    let fixture = "fixture_types";
    let parsed_output = get_single_file_parse_output(fixture);
    let graph = &parsed_output.graph;
    let crate_namespace = parsed_output.crate_namespace;
    let file_path = &parsed_output.file_path;

    let func_name = "process_ref";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        graph,
        crate_namespace,
        file_path,
        &module_path,
        func_name,
    );

    // Assertions
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(func_node.tracking_hash.is_some());
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("r")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (&String)
    let param_type_node = find_type_node(graph, param.type_id);
    assert!(
        matches!(&param_type_node.kind, TypeKind::Reference { is_mutable, .. } if !*is_mutable),
       "Expected TypeKind::Reference (immutable), found {:?}", param_type_node.kind
    );
    assert_eq!(param_type_node.related_types.len(), 1, "Reference should have one related type (String)");
    let referenced_type_id = param_type_node.related_types[0];
    let referenced_type_node = find_type_node(graph, referenced_type_id);
     assert!(
         matches!(&referenced_type_node.kind, TypeKind::Named { path, .. } if path == &["String"]),
        "Expected referenced type to be 'String', found {:?}", referenced_type_node.kind
     );


    // Return Type (usize)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
     assert!(
         matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["usize"]),
        "Expected TypeKind::Named for 'usize', found {:?}", return_type_node.kind
     );
}


#[test]
fn test_function_node_process_mut_ref() {
    let fixture = "fixture_types";
    let parsed_output = get_single_file_parse_output(fixture);
    let graph = &parsed_output.graph;
    let crate_namespace = parsed_output.crate_namespace;
    let file_path = &parsed_output.file_path;

    let func_name = "process_mut_ref";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        graph,
        crate_namespace,
        file_path,
        &module_path,
        func_name,
    );

    // Assertions
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(func_node.tracking_hash.is_some());
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("r")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (&mut String)
    let param_type_node = find_type_node(graph, param.type_id);
    assert!(
        matches!(&param_type_node.kind, TypeKind::Reference { is_mutable, .. } if *is_mutable),
       "Expected TypeKind::Reference (mutable), found {:?}", param_type_node.kind
    );
     assert_eq!(param_type_node.related_types.len(), 1, "Reference should have one related type (String)");
     let referenced_type_id = param_type_node.related_types[0];
     let referenced_type_node = find_type_node(graph, referenced_type_id);
      assert!(
          matches!(&referenced_type_node.kind, TypeKind::Named { path, .. } if path == &["String"]),
         "Expected referenced type to be 'String', found {:?}", referenced_type_node.kind
      );

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none()); // Implicit unit
}


// --- Tests for functions inside `duplicate_names` module ---

#[test]
fn test_function_node_process_tuple_in_duplicate_names() {
    let fixture = "fixture_types";
    let parsed_output = get_single_file_parse_output(fixture);
    let graph = &parsed_output.graph;
    let crate_namespace = parsed_output.crate_namespace;
    let file_path = &parsed_output.file_path;

    let func_name = "process_tuple";
    // Module path as recorded during Phase 2 parse of lib.rs
    let module_path = vec!["crate".to_string(), "duplicate_names".to_string()];

    let func_node = find_function_node_paranoid(
        graph,
        crate_namespace,
        file_path,
        &module_path,
        func_name,
    );

    // Basic Assertions (should be similar to the top-level one)
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(func_node.tracking_hash.is_some());
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public); // Check visibility within module

    // Parameters
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("p")); // Use as_deref() for Option<String>
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));
    let param_type_node = find_type_node(graph, param.type_id);
    // Check type kind - should reference the 'Point' defined *within* duplicate_names
    // This requires checking the TypeNode's path or related types carefully.
    // For now, assert it's Named "Point". Phase 3 resolves which "Point" it is.
     assert!(
         matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Point"]),
         "Expected TypeKind::Named for alias 'Point' (in duplicate_names), found {:?}", param_type_node.kind
     );


    // Return Type
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));
    let return_type_node = find_type_node(graph, return_type_id);
     assert!(
         matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]),
        "Expected TypeKind::Named for 'i32', found {:?}", return_type_node.kind
     );

    // Add more assertions for other functions in duplicate_names...
    // test_function_node_process_slice_in_duplicate_names
    // test_function_node_process_array_in_duplicate_names
    // etc.
}

// TODO: Add tests for apply_op, draw_object, process_impl_trait_arg, create_impl_trait_return, inferred_type_example
// TODO: Add tests for the private functions process_const_ptr, process_mut_ptr (check visibility is Inherited or Restricted)
// TODO: Add tests for the corresponding functions inside duplicate_names module.
