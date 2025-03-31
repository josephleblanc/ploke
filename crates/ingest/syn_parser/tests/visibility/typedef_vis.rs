#![cfg(feature = "visibility_resolution")]
//!    TODO: Add more test documentation

use crate::common::{
    find_function_by_name, find_struct_by_name, get_visibility_info, parse_fixture,
};
use syn_parser::{
    parser::{
        nodes::{NodeId, TypeDefNode, VisibilityResult},
        types::VisibilityKind,
    },
    CodeGraph,
};

// Helper function needs explicit lifetime

#[test]
fn test_typedefnode_visibility_resolution() {
    let code_graph = parse_fixture("sample.rs").expect(
        "Failed to parse simple_pub.rs - file missing or inval 
 syntax",
    );

    // ===== PRIVATE ITEMS TEST =====
    let expected_private_types = &[
        "PrivateStruct",
        "PrivateStruct2",
        "PrivateEnum",
        "PrivateTypeAlias",
        "PrivateUnion",
        "RestrictedStruct",
        "PrivateTypeAlias",
        "ConditionalVisibilityStruct",
        "ConditionalPrivateStruct",
    ];

    // Updated test code
    let private_items = code_graph
        .defined_types
        .iter()
        .filter(|t| {
            let (id, _) = get_visibility_info(t, &code_graph);
            !matches!(
                code_graph.resolve_visibility(id, &["crate".to_string()]),
                VisibilityResult::Direct
            )
        })
        .map(|t| get_visibility_info(t, &code_graph).1)
        .collect::<Vec<_>>();

    #[cfg(feature = "verbose_debug")]
    {
        println!("All private type_def items found:");
        for private_item in &private_items {
            println!("   private type_def: {}", private_item);
        }
        println!(
            "     Total private type_def items found: {}",
            private_items.len()
        )
    }
    // Check we found exactly the expected private types
    //   All private type_def items expected:
    //     1.  PrivateStruct
    //     2.  PrivateStruct2
    //     3.  PrivateEnum
    //     4.  PrivateTypeAlias
    //     5.  PrivateUnion
    //     6.  RestrictedStruct
    //      - defined inside multiple `pub mod` declarations:
    //          - In target file: pub(in crate::outer) struct RestrictedStruct;
    //      - Correctly identified as !VisibiltyResult::Direct
    //          - This test could be more granular by defining whether we are searching for a user
    //          TypeDefItem or a user's dependency TypeDefItem. In the case of a user's code we
    //          would want to show this as VisibilityResult::
    //     7.  PrivateTypeAlias
    //     8.  ConditionalVisibilityStruct
    //      - Behind cfg flag "#[cfg_attr(feature = "public", pub)]"
    //      - Known failure: Cargo.toml not parsed for cfg flags, this is a goal down the road.
    //     9.  ConditionalPrivateStruct
    //      - Behind cfg flag "#[cfg_attr(feature = "never_enabled", pub)]"
    //      - Known failure: Cargo.toml not parsed for cfg flags, this is a goal down the road.
    assert_eq!(
        private_items.len(),
        expected_private_types.len(),
        "Mismatch in number of private types for the case of user dependency visibility"
    );

    for expected_type in expected_private_types {
        assert!(
            private_items.contains(expected_type),
            "Expected private type '{}' not found",
            expected_type
        );
    }

    assert_eq!(
        private_items.len(),
        9,
        "Expected 9 PRIVATE defined types: {:#?}. Found: {}: {:#?}",
        expected_private_types,
        private_items.len(),
        code_graph
            .defined_types
            .iter()
            .map(|t| get_visibility_info(t, &code_graph).1)
            .collect::<Vec<_>>()
    );

    // ===== TOTAL ITEMS TEST =====
    let total_defined_types = code_graph
        .defined_types
        .iter()
        .map(|t| get_visibility_info(t, &code_graph).1)
        .collect::<Vec<_>>();

    #[cfg(feature = "verbose_debug")]
    {
        println!("All defined types found:");
        for defined_type in &total_defined_types {
            println!("   type_def: {}", defined_type);
        }
        println!(
            "     Total defined types found: {}",
            total_defined_types.len()
        )
    }
    assert_eq!(
        code_graph.defined_types.len(),
        25,
        "Expected 15 TOTAL defined types (10 public + 5 private). Found: {}: {:?}",
        code_graph.defined_types.len(),
        code_graph
            .defined_types
            .iter()
            .map(|t| get_visibility_info(t, &code_graph).1)
            .collect::<Vec<_>>()
    );

    // ===== VISIBILITY CHECKS =====
    let private_struct = code_graph
        .defined_types
        .iter()
        .find(|t| {
            let (_, name) = get_visibility_info(t, &code_graph);
            name == "PrivateStruct"
        })
        .unwrap();

    let (private_struct_id, _) = get_visibility_info(private_struct, &code_graph);
    assert!(
        !matches!(
            code_graph.resolve_visibility(private_struct_id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "PrivateStruct should not be directly visible from crate root"
    );

    // ===== PUBLIC ITEMS TEST =====
    let public_type_definitions = code_graph
        .defined_types
        .iter()
        .filter(|t| {
            let (id, _) = get_visibility_info(t, &code_graph);
            matches!(
                code_graph.resolve_visibility(id, &["crate".to_string()]),
                VisibilityResult::Direct
            )
        })
        .map(|t| get_visibility_info(t, &code_graph).1)
        .collect::<Vec<_>>();

    #[cfg(feature = "verbose_debug")]
    {
        println!("All public type_def items found:");
        for public_type_definition in &public_type_definitions {
            println!("   public type_def: {}", public_type_definition);
        }
        println!(
            "     Total public type_def items found: {}",
            public_type_definitions.len()
        )
    }

    assert_eq!(
        public_type_definitions.len(),
        16,
        "Expected 16 PUBLIC defined types when checking visibility"
    );

    // ===== FUNCTION VISIBILITY TEST =====
    let private_fn = code_graph
        .functions
        .iter()
        .find(|f| f.name == "private_function")
        .unwrap();

    assert!(
        !matches!(
            code_graph.resolve_visibility(private_fn.id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "private_function should not be directly visible"
    );
}

#[test]
fn test_function_visibility() {
    let code_graph = parse_fixture("sample.rs").expect(
        "Failed to parse simple_pub.rs - file missing or inval 
 syntax",
    );

    let private_fn = code_graph
        .functions
        .iter()
        .find(|f| f.name == "private_function")
        .unwrap();

    assert!(
        !matches!(
            code_graph.resolve_visibility(private_fn.id, &["crate".to_string()]),
            VisibilityResult::Direct
        ),
        "private_function should not be directly visible"
    );
}
