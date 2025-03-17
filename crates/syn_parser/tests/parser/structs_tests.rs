use crate::common::*;
use syn_parser::parser::types::*;

#[test]
fn test_regular_struct_parsing() {
    let graph = parse_fixture("structs.rs");

    let sample_struct = find_struct_by_name(&graph, "SampleStruct")
        .expect("SampleStruct not found");

    assert_eq!(sample_struct.name, "SampleStruct");
    assert_eq!(sample_struct.visibility, VisibilityKind::Public);
    assert_eq!(sample_struct.fields.len(), 1);
    assert_eq!(sample_struct.fields[0].name, Some("field".to_string()));
}

#[test]
fn test_tuple_struct_parsing() {
    let graph = parse_fixture("structs.rs");

    let tuple_struct = find_struct_by_name(&graph, "TupleStruct")
        .expect("TupleStruct not found");

    assert_eq!(tuple_struct.fields.len(), 2);
    assert_eq!(tuple_struct.fields[0].visibility, VisibilityKind::Public);
}

#[test]
fn test_unit_struct_parsing() {
    let graph = parse_fixture("structs.rs");

    let unit_struct = find_struct_by_name(&graph, "UnitStruct")
        .expect("UnitStruct not found");

    assert_eq!(unit_struct.fields.len(), 0);
}

#[test]
fn test_struct_with_generics() {
    let graph = parse_fixture("structs.rs");

    let generic_struct = find_struct_by_name(&graph, "GenericStruct")
        .expect("GenericStruct not found");

    assert_eq!(generic_struct.generic_params.len(), 1);
    if let GenericParamKind::Type { name, .. } = &generic_struct.generic_params[0].kind {
        assert_eq!(name, "T");
    } else {
        panic!("Expected Type generic parameter");
    }
}

#[test]
fn test_struct_with_attributes() {
    let graph = parse_fixture("structs.rs");

    let attributed_struct = find_struct_by_name(&graph, "AttributedStruct")
        .expect("AttributedStruct not found");

    assert!(attributed_struct.attributes.iter().any(|attr| attr.name == "derive"));
}

#[test]
fn test_struct_with_docstring() {
    let graph = parse_fixture("structs.rs");

    let documented_struct = find_struct_by_name(&graph, "DocumentedStruct")
        .expect("DocumentedStruct not found");

    assert!(documented_struct.docstring.is_some());
    assert!(documented_struct.docstring.as_ref().unwrap().contains("documented struct"));
}
