//!    TODO: Add more test documentation
#[cfg(feature = "visibility_resolution")]
mod visibility_resolution_tests {

    use crate::common::{find_function_by_name, find_struct_by_name, parse_fixture};
    use syn_parser::{
        parser::nodes::{NodeId, TypeDefNode, VisibilityResult},
        CodeGraph,
    };

    // Helper function needs explicit lifetime
    fn get_visibility_info<'a>(def: &'a TypeDefNode, _graph: &CodeGraph) -> (NodeId, &'a str) {
        match def {
            TypeDefNode::Struct(s) => (s.id, s.name.as_str()),
            TypeDefNode::Enum(e) => (e.id, e.name.as_str()),
            TypeDefNode::TypeAlias(a) => (a.id, a.name.as_str()),
            TypeDefNode::Union(u) => (u.id, u.name.as_str()),
        }
    }

    #[test]
    fn test_analyzer_visibility_resolution() {
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

        // Check we found exactly the expected private types
        assert_eq!(
            private_items.len(),
            expected_private_types.len(),
            "Mismatch in number of private types"
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
            5,
            "Expected 5 PRIVATE defined types (PrivateStruct, PrivateStruct2, PrivateEnum, PrivateTypeAlias, PrivateUnion). Found: {}: {:?}",
            private_items.len(),
            code_graph
                .defined_types
                .iter()
                .map(|t| get_visibility_info(t, &code_graph).1)
                .collect::<Vec<_>>()
        );

        // ===== TOTAL ITEMS TEST =====
        assert_eq!(
            code_graph.defined_types.len(),
            15,
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
        let public_items = code_graph
            .defined_types
            .iter()
            .filter(|t| {
                let (id, _) = get_visibility_info(t, &code_graph);
                matches!(
                    code_graph.resolve_visibility(id, &["crate".to_string()]),
                    VisibilityResult::Direct
                )
            })
            .count();

        assert_eq!(
            public_items, 10,
            "Expected 10 PUBLIC defined types when checking visibility"
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
}
