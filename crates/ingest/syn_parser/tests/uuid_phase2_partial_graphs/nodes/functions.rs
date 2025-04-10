#![cfg(feature = "uuid_ids")]
#![cfg(test)]

use crate::common::uuid_ids_utils::*;
use ploke_core::{NodeId, TypeId}; // Remove VisibilityKind from here
use syn_parser::parser::types::VisibilityKind; // Import VisibilityKind from its correct location
use syn_parser::parser::{
    nodes::Visible,
    types::TypeKind, // Use the correct existing struct
};

// Import helper to construct fixture paths

// --- Helper Functions ---

// --- Test Cases ---

#[test]
fn test_function_node_process_tuple() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed")) // Collect successful parses
        .collect();

    let func_name = "process_tuple";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results // Need graph for type lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
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
        "Expected TypeKind::Named for alias 'Point', found {:?}",
        param_type_node.kind
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
        "Expected TypeKind::Named for 'i32', found {:?}",
        return_type_node.kind
    );
}

#[test]
fn test_function_node_process_slice() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_slice";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
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
    // Current state: Correctly identifies the reference part.
    assert!(
        matches!(&param_type_node.kind, TypeKind::Reference { is_mutable, .. } if !*is_mutable),
        "Expected TypeKind::Reference for '&[u8]', found {:?}",
        param_type_node.kind
    );
    // Check the referenced type ([u8])
    assert_eq!(
        param_type_node.related_types.len(),
        1,
        "Reference should have one related type ([u8])"
    );
    let slice_type_id = param_type_node.related_types[0];
    let slice_type_node = find_type_node(graph, slice_type_id);
    // The underlying slice type [u8] currently falls back to Unknown because TypeKind::Slice is not implemented
    assert!(
        matches!(&slice_type_node.kind, TypeKind::Unknown { type_str } if type_str == "[u8]"),
        "Expected underlying type '[u8]' to be TypeKind::Unknown currently, found {:?}",
        slice_type_node.kind
    );

    // #[ignore = "TypeKind::Slice not yet handled in type_processing.rs"]
    {
        // Target state assertion for the underlying slice type (will fail until implemented)
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
        "Expected TypeKind::Named for 'usize', found {:?}",
        return_type_node.kind
    );
}

#[test]
fn test_function_node_process_array() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_array";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
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
        "Expected TypeKind::Named for alias 'Buffer', found {:?}",
        param_type_node.kind
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
        "Expected TypeKind::Named for 'u8', found {:?}",
        return_type_node.kind
    );
}

#[test]
fn test_function_node_process_ref() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_ref";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
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
        "Expected TypeKind::Reference (immutable), found {:?}",
        param_type_node.kind
    );
    assert_eq!(
        param_type_node.related_types.len(),
        1,
        "Reference should have one related type (String)"
    );
    let referenced_type_id = param_type_node.related_types[0];
    let referenced_type_node = find_type_node(graph, referenced_type_id);
    assert!(
        matches!(&referenced_type_node.kind, TypeKind::Named { path, .. } if path == &["String"]),
        "Expected referenced type to be 'String', found {:?}",
        referenced_type_node.kind
    );

    // Return Type (usize)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(
        matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["usize"]),
        "Expected TypeKind::Named for 'usize', found {:?}",
        return_type_node.kind
    );
}

#[test]
fn test_function_node_process_mut_ref() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_mut_ref";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
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
        "Expected TypeKind::Reference (mutable), found {:?}",
        param_type_node.kind
    );
    assert_eq!(
        param_type_node.related_types.len(),
        1,
        "Reference should have one related type (String)"
    );
    let referenced_type_id = param_type_node.related_types[0];
    let referenced_type_node = find_type_node(graph, referenced_type_id);
    assert!(
        matches!(&referenced_type_node.kind, TypeKind::Named { path, .. } if path == &["String"]),
        "Expected referenced type to be 'String', found {:?}",
        referenced_type_node.kind
    );

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none()); // Implicit unit
}

// --- Tests for functions inside `duplicate_names` module ---

#[test]
fn test_function_node_process_tuple_in_duplicate_names() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_tuple";
    let relative_file_path = "src/lib.rs"; // Function is defined in lib.rs
                                           // Module path *within lib.rs* where the function is defined
    let module_path = vec!["crate".to_string(), "duplicate_names".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Basic Assertions (should be similar to the top-level one)
    let graph = &results // Need graph for type lookups
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
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
        "Expected TypeKind::Named for alias 'Point' (in duplicate_names), found {:?}",
        param_type_node.kind
    );

    // Return Type
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(
        matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]),
        "Expected TypeKind::Named for 'i32', found {:?}",
        return_type_node.kind
    );

    // Add more assertions for other functions in duplicate_names...
    // test_function_node_process_slice_in_duplicate_names
    // test_function_node_process_array_in_duplicate_names
    // etc.
}

#[test]
fn test_function_node_apply_op() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "apply_op";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);

    // Parameters (a: i32, b: i32, op: MathOperation)
    assert_eq!(func_node.parameters.len(), 3);
    let param_a = &func_node.parameters[0];
    let param_b = &func_node.parameters[1];
    let param_op = &func_node.parameters[2];

    assert_eq!(param_a.name.as_deref(), Some("a"));
    assert_eq!(param_b.name.as_deref(), Some("b"));
    assert_eq!(param_op.name.as_deref(), Some("op"));

    let type_a = find_type_node(graph, param_a.type_id);
    let type_b = find_type_node(graph, param_b.type_id);
    let type_op = find_type_node(graph, param_op.type_id);

    assert!(matches!(&type_a.kind, TypeKind::Named { path, .. } if path == &["i32"]));
    assert!(matches!(&type_b.kind, TypeKind::Named { path, .. } if path == &["i32"]));
    assert!(matches!(&type_op.kind, TypeKind::Named { path, .. } if path == &["MathOperation"]));
    // TODO: Check underlying fn pointer type for MathOperation once alias resolution is better

    // Return Type (i32)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]));
}

#[test]
fn test_function_node_process_const_ptr() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_const_ptr";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()]; // Private function at top level

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    // Private functions default to Inherited visibility in the parser
    assert_eq!(func_node.visibility(), VisibilityKind::Inherited);

    // Parameters (p: *const i32)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("p"));
    let param_type_node = find_type_node(graph, param.type_id);
    // Current state: Falls back to Unknown because TypeKind::Ptr not implemented
    assert!(
        matches!(&param_type_node.kind, TypeKind::Unknown { type_str } if type_str == "* const i32"),
        "Expected TypeKind::Unknown for '*const i32' currently, found {:?}",
        param_type_node.kind
    );
    // #[ignore = "TypeKind::Ptr not yet handled in type_processing.rs"]
    {
        // Target state assertion
        // assert!(matches!(param_type_node.kind, TypeKind::Pointer { is_mutable: false, .. }));
        // assert_eq!(param_type_node.related_types.len(), 1); // Should relate to i32
    }

    // Return Type (i32)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]));
}

#[test]
fn test_function_node_consumes_point_in_func_mod() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "consumes_point";
    // Function is defined in src/func/return_types.rs
    let relative_file_path = "src/func/return_types.rs";
    // Module path *within return_types.rs*
    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
    ];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    // pub(crate) is parsed as Restricted(["crate"]) currently
    assert_eq!(
        func_node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()])
    );

    // Parameters (point: Point)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("point"));
    let param_type_node = find_type_node(graph, param.type_id);
    // Should resolve to the top-level Point alias
    assert!(matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Point"]));

    // Return Type (bool)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["bool"]));
}

#[test]
fn test_function_node_draw_object() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "draw_object";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters (obj: &dyn Drawable)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("obj"));
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (&dyn Drawable)
    let param_type_node = find_type_node(graph, param.type_id);
    // Should be an immutable reference
    assert!(
        matches!(&param_type_node.kind, TypeKind::Reference { is_mutable, .. } if !*is_mutable),
        "Expected TypeKind::Reference (immutable) for '&dyn Drawable', found {:?}",
        param_type_node.kind
    );
    // Check the referenced type (dyn Drawable)
    assert_eq!(
        param_type_node.related_types.len(),
        1,
        "Reference should have one related type (dyn Drawable)"
    );
    let trait_object_type_id = param_type_node.related_types[0];
    let trait_object_type_node = find_type_node(graph, trait_object_type_id);
    // The underlying trait object type currently falls back to Unknown because TypeKind::TraitObject is not implemented
    assert!(
        matches!(&trait_object_type_node.kind, TypeKind::Unknown { type_str } if type_str == "dyn Drawable"),
        "Expected underlying type 'dyn Drawable' to be TypeKind::Unknown currently, found {:?}",
        trait_object_type_node.kind
    );

    // #[ignore = "TypeKind::TraitObject not yet handled in type_processing.rs"]
    {
        // Target state assertion for the underlying trait object type
        // assert!(matches!(trait_object_type_node.kind, TypeKind::TraitObject { .. }));
        // assert_eq!(trait_object_type_node.related_types.len(), 1); // Should relate to Drawable trait TypeId
    }

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none());
}

#[test]
fn test_function_node_process_impl_trait_arg() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_impl_trait_arg";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.generic_params.is_empty()); // `impl Trait` is not a generic param here
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters (arg: impl Debug)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("arg"));
    assert!(matches!(param.type_id, TypeId::Synthetic(_)));

    // Check parameter TypeNode (impl Debug)
    let param_type_node = find_type_node(graph, param.type_id);
    // The impl trait type currently falls back to Unknown because TypeKind::ImplTrait is not implemented
    assert!(
        matches!(&param_type_node.kind, TypeKind::Unknown { type_str } if type_str == "impl Debug"),
        "Expected type 'impl Debug' to be TypeKind::Unknown currently, found {:?}",
        param_type_node.kind
    );

    // #[ignore = "TypeKind::ImplTrait not yet handled in type_processing.rs"]
    {
        // Target state assertion for the impl trait type
        // assert!(matches!(param_type_node.kind, TypeKind::ImplTrait { .. }));
        // assert_eq!(param_type_node.related_types.len(), 1); // Should relate to Debug trait TypeId
    }

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none());
}

#[test]
fn test_function_node_create_impl_trait_return() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "create_impl_trait_return";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters ()
    assert!(func_node.parameters.is_empty());

    // Return Type (impl Debug)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    assert!(matches!(return_type_id, TypeId::Synthetic(_)));

    // Check return TypeNode (impl Debug)
    let return_type_node = find_type_node(graph, return_type_id);
    // The impl trait type currently falls back to Unknown because TypeKind::ImplTrait is not implemented
    assert!(
        matches!(&return_type_node.kind, TypeKind::Unknown { type_str } if type_str == "impl Debug"),
        "Expected type 'impl Debug' to be TypeKind::Unknown currently, found {:?}",
        return_type_node.kind
    );

    // #[ignore = "TypeKind::ImplTrait not yet handled in type_processing.rs"]
    {
        // Target state assertion for the impl trait type
        // assert!(matches!(return_type_node.kind, TypeKind::ImplTrait { .. }));
        // assert_eq!(return_type_node.related_types.len(), 1); // Should relate to Debug trait TypeId
    }
}

#[test]
fn test_function_node_process_mut_ptr() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_mut_ptr";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()]; // Private function at top level

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    // Private functions default to Inherited visibility in the parser
    assert_eq!(func_node.visibility(), VisibilityKind::Inherited);
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters (p: *mut i32)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("p"));
    let param_type_node = find_type_node(graph, param.type_id);
    // Current state: Falls back to Unknown because TypeKind::Ptr not implemented
    assert!(
        matches!(&param_type_node.kind, TypeKind::Unknown { type_str } if type_str == "* mut i32"),
        "Expected TypeKind::Unknown for '*mut i32' currently, found {:?}",
        param_type_node.kind
    );
    // #[ignore = "TypeKind::Ptr not yet handled in type_processing.rs"]
    {
        // Target state assertion
        // assert!(matches!(param_type_node.kind, TypeKind::Pointer { is_mutable: true, .. }));
        // assert_eq!(param_type_node.related_types.len(), 1); // Should relate to i32
    }

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none());
}

#[test]
fn test_function_node_inferred_type_example() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "inferred_type_example";
    let relative_file_path = "src/lib.rs";
    let module_path = vec!["crate".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    // let graph = &results // Graph not needed for this test's assertions
    //     .iter()
    //     .find(|data| data.file_path.ends_with(relative_file_path))
    //     .unwrap()
    //     .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public);
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters () - None
    assert!(func_node.parameters.is_empty());

    // Return Type (implicit unit `()`)
    assert!(func_node.return_type.is_none());

    // Note: We don't currently parse function bodies deeply enough to create
    // nodes or types for inferred types within `let` bindings like `let x = 5;`
    // or `let _y: _ = ...;`. So, there are no specific type assertions to make here
    // regarding the `_` type itself based on the FunctionNode.
}

#[test]
fn test_function_node_process_slice_in_duplicate_names() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_slice";
    let relative_file_path = "src/lib.rs"; // Defined in lib.rs
    let module_path = vec!["crate".to_string(), "duplicate_names".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions (similar to top-level process_slice)
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public); // Public within module

    // Parameters (s: &[u8])
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("s"));
    let param_type_node = find_type_node(graph, param.type_id);
    assert!(
        matches!(&param_type_node.kind, TypeKind::Reference { is_mutable, .. } if !*is_mutable),
        "Expected TypeKind::Reference for '&[u8]', found {:?}",
        param_type_node.kind
    );
    assert_eq!(param_type_node.related_types.len(), 1);
    let slice_type_id = param_type_node.related_types[0];
    let slice_type_node = find_type_node(graph, slice_type_id);
    assert!(
        matches!(&slice_type_node.kind, TypeKind::Unknown { type_str } if type_str == "[u8]"),
        "Expected underlying type '[u8]' to be TypeKind::Unknown currently, found {:?}",
        slice_type_node.kind
    );

    // Return Type (usize)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["usize"]));
}

#[test]
fn test_function_node_process_array_in_duplicate_names() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "process_array";
    let relative_file_path = "src/lib.rs"; // Defined in lib.rs
    let module_path = vec!["crate".to_string(), "duplicate_names".to_string()];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions (similar to top-level process_array)
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Public); // Public within module

    // Parameters (a: Buffer)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("a"));
    let param_type_node = find_type_node(graph, param.type_id);
    // Should resolve to the Buffer alias defined *within* duplicate_names
    assert!(
        matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Buffer"]),
        "Expected TypeKind::Named for alias 'Buffer' (in duplicate_names), found {:?}",
        param_type_node.kind
    );
    // TODO: Deeper check for underlying array type once alias resolution is better

    // Return Type (u8)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["u8"]));
}

#[test]
fn test_function_node_generic_func_in_func_mod() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "generic_func";
    let relative_file_path = "src/func/return_types.rs";
    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
    ]; // Defined directly in the file

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(
        func_node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()]) // pub(crate)
    );
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Generics <T: Display + Clone, S: Send + Sync>
    assert_eq!(func_node.generic_params.len(), 2);
    // TODO: Add detailed checks for GenericParamNode kinds and bounds once implemented/needed

    // Parameters (first: T, unused_param: S)
    assert_eq!(func_node.parameters.len(), 2);
    let param_t = &func_node.parameters[0];
    let param_s = &func_node.parameters[1];

    assert_eq!(param_t.name.as_deref(), Some("first"));
    assert_eq!(param_s.name.as_deref(), Some("unused_param"));

    let type_t = find_type_node(graph, param_t.type_id);
    let type_s = find_type_node(graph, param_s.type_id);

    // Check parameter types refer to the generic names
    assert!(matches!(&type_t.kind, TypeKind::Named { path, .. } if path == &["T"]));
    assert!(matches!(&type_s.kind, TypeKind::Named { path, .. } if path == &["S"]));

    // Return Type (T)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    // Should be the same TypeId as the 'T' parameter
    assert_eq!(return_type_id, param_t.type_id);
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["T"]));
}

#[test]
fn test_function_node_math_operation_consumer_in_func_mod() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "math_operation_consumer";
    let relative_file_path = "src/func/return_types.rs";
    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
    ]; // private function

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Inherited); // Private function
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters (func_param: MathOperation, x: i32, y: i32)
    assert_eq!(func_node.parameters.len(), 3);
    let param_func = &func_node.parameters[0];
    let param_x = &func_node.parameters[1];
    let param_y = &func_node.parameters[2];

    assert_eq!(param_func.name.as_deref(), Some("func_param"));
    assert_eq!(param_x.name.as_deref(), Some("x"));
    assert_eq!(param_y.name.as_deref(), Some("y"));

    let type_func = find_type_node(graph, param_func.type_id);
    let type_x = find_type_node(graph, param_x.type_id);
    let type_y = find_type_node(graph, param_y.type_id);

    assert!(matches!(&type_func.kind, TypeKind::Named { path, .. } if path == &["MathOperation"]));
    assert!(matches!(&type_x.kind, TypeKind::Named { path, .. } if path == &["i32"]));
    assert!(matches!(&type_y.kind, TypeKind::Named { path, .. } if path == &["i32"]));

    // Return Type (i32)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["i32"]));
}

#[test]
fn test_function_node_math_operation_producer_in_func_mod() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "math_operation_producer";
    let relative_file_path = "src/func/return_types.rs";
    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
    ];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(func_node.visibility(), VisibilityKind::Inherited); // Private function
    assert!(func_node.generic_params.is_empty());
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Parameters ()
    assert!(func_node.parameters.is_empty());

    // Return Type (MathOperation)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(
        matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["MathOperation"])
    );
    // TODO: Check underlying fn pointer type once alias resolution is better
}

// --- Tests for functions inside src/func/return_types.rs/restricted_duplicate ---

#[test]
fn test_function_node_consumes_point_in_restricted_duplicate() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "consumes_point";
    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
    ];
    let relative_file_path = "src/func/return_types.rs";
    // Module path *within return_types.rs* for the nested module

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions (should be identical to consumes_point_in_func_mod)
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    // pub(crate) is parsed as Restricted(["crate"]) currently
    assert_eq!(
        func_node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()])
    );

    // Parameters (point: Point)
    assert_eq!(func_node.parameters.len(), 1);
    let param = &func_node.parameters[0];
    assert_eq!(param.name.as_deref(), Some("point"));
    let param_type_node = find_type_node(graph, param.type_id);
    // Should resolve to the top-level Point alias (defined outside this module)
    assert!(matches!(&param_type_node.kind, TypeKind::Named { path, .. } if path == &["Point"]));

    // Return Type (bool)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["bool"]));
}

#[test]
fn test_function_node_generic_func_in_restricted_duplicate() {
    let fixture_name = "fixture_types";
    let results: Vec<_> = run_phase1_phase2(fixture_name)
        .into_iter()
        .map(|res| res.expect("Parsing failed"))
        .collect();

    let func_name = "generic_func";
    let relative_file_path = "src/func/return_types.rs";

    let module_path = vec![
        "crate".to_string(),
        "func".to_string(),
        "return_types".to_string(),
        "restricted_duplicate".to_string(),
    ];

    let func_node = find_function_node_paranoid(
        &results,
        fixture_name,
        relative_file_path,
        &module_path,
        func_name,
    );

    // Assertions (should be identical to generic_func_in_func_mod)
    let graph = &results
        .iter()
        .find(|data| data.file_path.ends_with(relative_file_path))
        .unwrap()
        .graph;
    assert!(matches!(func_node.id(), NodeId::Synthetic(_)));
    assert!(
        func_node.tracking_hash.is_some(),
        "Tracking hash should be present"
    );
    assert_eq!(func_node.name(), func_name);
    assert_eq!(
        func_node.visibility(),
        VisibilityKind::Restricted(vec!["crate".to_string()]) // pub(crate)
    );
    assert!(func_node.attributes.is_empty());
    assert!(func_node.docstring.is_none());

    // Generics <T: Display + Clone, S: Send + Sync>
    assert_eq!(func_node.generic_params.len(), 2);
    // TODO: Add detailed checks for GenericParamNode kinds and bounds once implemented/needed

    // Parameters (first: T, unused_param: S)
    assert_eq!(func_node.parameters.len(), 2);
    let param_t = &func_node.parameters[0];
    let param_s = &func_node.parameters[1];

    assert_eq!(param_t.name.as_deref(), Some("first"));
    assert_eq!(param_s.name.as_deref(), Some("unused_param"));

    let type_t = find_type_node(graph, param_t.type_id);
    let type_s = find_type_node(graph, param_s.type_id);

    // Check parameter types refer to the generic names
    assert!(matches!(&type_t.kind, TypeKind::Named { path, .. } if path == &["T"]));
    assert!(matches!(&type_s.kind, TypeKind::Named { path, .. } if path == &["S"]));

    // Return Type (T)
    assert!(func_node.return_type.is_some());
    let return_type_id = func_node.return_type.unwrap();
    // Should be the same TypeId as the 'T' parameter
    assert_eq!(return_type_id, param_t.type_id);
    let return_type_node = find_type_node(graph, return_type_id);
    assert!(matches!(&return_type_node.kind, TypeKind::Named { path, .. } if path == &["T"]));
}

// TODO: Add tests for the corresponding functions inside duplicate_names module (process_ref, process_mut_ref, process_const_ptr, process_mut_ptr, apply_op, draw_object, process_impl_trait_arg, create_impl_trait_return, inferred_type_example).
// TODO: Add tests for functions inside src/func/return_types.rs/restricted_duplicate (math_operation_consumer, math_operation_producer)
