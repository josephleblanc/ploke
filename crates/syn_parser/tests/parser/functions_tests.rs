use crate::common::*;
use syn_parser::parser::types::GenericParamKind;
use syn_parser::parser::types::VisibilityKind;
use syn_parser::parser::types::TypeKind;

#[test]
fn test_regular_function_parsing() {
    let graph = parse_fixture("functions.rs");

    let function =
        find_function_by_name(&graph, "regular_function").expect("regular_function not found");

    assert_eq!(function.name, "regular_function");
    assert_eq!(function.visibility, VisibilityKind::Public);
    assert!(function.parameters.is_empty());
    assert_eq!(function.return_type, None);
    assert!(function.generic_params.is_empty());
    assert!(function.attributes.is_empty());
    assert_eq!(function.docstring, None);
}

#[test]
fn test_function_with_params_parsing() {
    let graph = parse_fixture("functions.rs");

    let function = find_function_by_name(&graph, "function_with_params")
        .expect("function_with_params not found");

    assert_eq!(function.name, "function_with_params");
    assert_eq!(function.visibility, VisibilityKind::Public);
    assert_eq!(function.parameters.len(), 2);
    assert_eq!(function.parameters[0].name, Some("x".to_string()));
    assert_eq!(function.parameters[1].name, Some("y".to_string()));
    assert!(function.return_type.is_some());
    assert!(function.generic_params.is_empty());
}

#[test]
fn test_generic_function_parsing() {
    let graph = parse_fixture("functions.rs");

    let function =
        find_function_by_name(&graph, "generic_function").expect("generic_function not found");

    assert_eq!(function.name, "generic_function");
    assert_eq!(function.visibility, VisibilityKind::Public);
    assert_eq!(function.parameters.len(), 1);
    assert_eq!(function.parameters[0].name, Some("arg".to_string()));
    assert!(function.return_type.is_some());
    assert_eq!(function.generic_params.len(), 1);

    if let GenericParamKind::Type { name, .. } = &function.generic_params[0].kind {
        assert_eq!(name, "T");
    } else {
        panic!("Expected Type generic parameter");
    }
}

#[test]
fn test_attributed_function_parsing() {
    let graph = parse_fixture("functions.rs");

    let function = find_function_by_name(&graph, "attributed_function")
        .expect("attributed_function not found");

    assert_eq!(function.name, "attributed_function");
    assert_eq!(function.visibility, VisibilityKind::Public);
    assert!(function.parameters.is_empty());
    assert_eq!(function.return_type, None);
    assert!(function.generic_params.is_empty());
    assert_eq!(function.attributes.len(), 1);
    assert_eq!(function.attributes[0].name, "cfg");
    assert_eq!(function.attributes[0].args, vec!["test".to_string()]);
}

#[test]
fn test_documented_function_parsing() {
    let graph = parse_fixture("functions.rs");

    let function = find_function_by_name(&graph, "documented_function")
        .expect("documented_function not found");

    assert_eq!(function.name, "documented_function");
    assert_eq!(function.visibility, VisibilityKind::Public);
    assert!(function.parameters.is_empty());
    assert_eq!(function.return_type, None);
    assert!(function.generic_params.is_empty());
    assert!(function.docstring.is_some());
    assert!(function
        .docstring
        .as_ref()
        .unwrap()
        .contains("documented function"));
}

#[test]
fn test_unsafe_function_parsing() {
    let graph = parse_fixture("functions.rs");

    let function =
        find_function_by_name(&graph, "unsafe_function").expect("unsafe_function not found");

    assert_eq!(function.name, "unsafe_function");
    assert_eq!(function.visibility, VisibilityKind::Public);
    assert!(function.parameters.is_empty());
    assert_eq!(function.return_type, None);
    assert!(function.generic_params.is_empty());
    // You might want to add an is_unsafe field to your FunctionNode struct
    // and test it here
}

#[test]
fn test_lifetime_function_parsing() {
    let graph = parse_fixture("functions.rs");
    let function = find_function_by_name(&graph, "lifetime_function").expect("lifetime_function not found");

    assert_eq!(function.name, "lifetime_function");
    assert_eq!(function.visibility, VisibilityKind::Public);
    assert_eq!(function.generic_params.len(), 1);

    if let GenericParamKind::Lifetime { ref name, ref bounds } = function.generic_params[0].kind {
        assert_eq!(name, "a");
        assert!(bounds.is_empty());
    } else {
        panic!("Expected Lifetime generic parameter");
    }

    assert_eq!(function.parameters.len(), 1);
    assert_eq!(function.parameters[0].name, Some("param".to_string()));
    assert_eq!(function.parameters[0].type_id, graph.type_graph.iter().find(|t| matches!(t.kind, TypeKind::Reference { lifetime: Some(ref lt), is_mutable: false } if lt == "a")).map(|t| t.id).expect("Reference type with lifetime 'a' not found"));
    assert_eq!(function.return_type, Some(graph.type_graph.iter().find(|t| matches!(t.kind, TypeKind::Reference { lifetime: Some(ref lt), is_mutable: false } if lt == "a")).map(|t| t.id).expect("Reference type with lifetime 'a' not found")));
}

#[test]
fn test_private_function_parsing() {
    let graph = parse_fixture("functions.rs");

    let function =
        find_function_by_name(&graph, "private_function").expect("private_function not found");

    assert_eq!(function.name, "private_function");
    assert_eq!(function.visibility, VisibilityKind::Restricted(vec!["super".to_string()]));
    assert!(function.parameters.is_empty());
    assert_eq!(function.return_type, None);
}

#[test]
fn test_multi_generic_function_parsing() {
    let graph = parse_fixture("functions.rs");

    let function = find_function_by_name(&graph, "multi_generic_function")
        .expect("multi_generic_function not found");

    assert_eq!(function.name, "multi_generic_function");
    assert_eq!(function.visibility, VisibilityKind::Public);
    assert_eq!(function.parameters.len(), 2);
    assert_eq!(function.generic_params.len(), 2);

    // Check that we have both T and U generic parameters
    let generic_names: Vec<String> = function
        .generic_params
        .iter()
        .filter_map(|param| {
            if let GenericParamKind::Type { name, .. } = &param.kind {
                Some(name.clone())
            } else {
                None
            }
        })
        .collect();

    assert!(generic_names.contains(&"T".to_string()));
    assert!(generic_names.contains(&"U".to_string()));
}

#[test]
fn test_where_clause_function_parsing() {
    let graph = parse_fixture("functions.rs");

    let function = find_function_by_name(&graph, "where_clause_function")
        .expect("where_clause_function not found");

    assert_eq!(function.name, "where_clause_function");
    assert_eq!(function.visibility, VisibilityKind::Public);
    assert_eq!(function.parameters.len(), 1);
    assert_eq!(function.generic_params.len(), 1);

    // Check for where clause constraints
    // This depends on how your parser stores where clause constraints
    // You might need to add a field to your GenericParamNode struct
}

#[test]
fn test_async_function_parsing() {
    let graph = parse_fixture("functions.rs");

    let function =
        find_function_by_name(&graph, "async_function").expect("async_function not found");

    assert_eq!(function.name, "async_function");
    assert_eq!(function.visibility, VisibilityKind::Public);
    // You might want to add an is_async field to your FunctionNode struct
    // and test it here
}

#[test]
fn test_default_params_function_parsing() {
    let graph = parse_fixture("functions.rs");

    let function =
        find_function_by_name(&graph, "default_params").expect("default_params not found");

    assert_eq!(function.name, "default_params");
    assert_eq!(function.visibility, VisibilityKind::Public);
    assert_eq!(function.parameters.len(), 2);
    assert_eq!(function.parameters[0].name, Some("required".to_string()));
    assert_eq!(function.parameters[1].name, Some("optional".to_string()));
    assert!(function.return_type.is_some());
}
