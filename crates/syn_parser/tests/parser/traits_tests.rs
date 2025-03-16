// Placeholder for trait parsing tests
#[test]
fn test_trait_parsing() {
    // Add trait parsing tests here
}

use crate::common::*;
use syn_parser::parser::types::*;

#[test]
fn test_regular_trait_parsing() {
    let graph = parse_fixture("traits.rs");

    let sample_trait = find_trait_by_name(&graph, "SampleTrait").expect("SampleTrait not found");

    assert_eq!(sample_trait.name, "SampleTrait");
    assert_eq!(sample_trait.visibility, VisibilityKind::Public);
    assert_eq!(sample_trait.methods.len(), 1);
    assert_eq!(sample_trait.methods[0].name, "sample_method");
}

#[test]
fn test_default_trait_parsing() {
    let graph = parse_fixture("traits.rs");

    let default_trait = find_trait_by_name(&graph, "DefaultTrait").expect("DefaultTrait not found");

    assert_eq!(default_trait.name, "DefaultTrait");
    assert_eq!(default_trait.visibility, VisibilityKind::Public);
    assert_eq!(default_trait.methods.len(), 1);
    assert_eq!(default_trait.methods[0].name, "default_method");
}

#[test]
fn test_generic_trait_parsing() {
    let graph = parse_fixture("traits.rs");

    let generic_trait = find_trait_by_name(&graph, "GenericTrait").expect("GenericTrait not found");

    assert_eq!(generic_trait.name, "GenericTrait");
    assert_eq!(generic_trait.visibility, VisibilityKind::Public);
    assert_eq!(generic_trait.methods.len(), 1);
    assert_eq!(generic_trait.methods[0].name, "generic_method");
    assert_eq!(generic_trait.generic_params.len(), 1);
    if let GenericParamKind::Type { name, .. } = &generic_trait.generic_params[0].kind {
        assert_eq!(name, "T");
    } else {
        panic!("Expected Type generic parameter");
    }
}

#[test]
fn test_assoc_type_trait_parsing() {
    let graph = parse_fixture("traits.rs");

    let assoc_type_trait =
        find_trait_by_name(&graph, "assoc_type_trait").expect("assoc_type_trait not found");

    assert_eq!(assoc_type_trait.name, "assoc_type_trait");
    assert_eq!(assoc_type_trait.visibility, VisibilityKind::Public);
    assert_eq!(assoc_type_trait.methods.len(), 1);
    assert_eq!(assoc_type_trait.methods[0].name, "method_with_assoc");
}

#[test]
fn test_private_trait_parsing() {
    let graph = parse_fixture("traits.rs");

    if let Some(private_trait) = find_trait_by_name(&graph, "PrivateTrait") {
        println!(
            "Visibility of private_trait is: {:?}",
            private_trait.visibility
        );
        assert!(matches!(
            private_trait.visibility,
            VisibilityKind::Restricted(_)
        ));
    } else {
        panic!("PrivateTrait not found in graph when using find_trait_by_name.")
    }
}
