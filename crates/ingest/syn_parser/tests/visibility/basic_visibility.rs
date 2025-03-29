use crate::common::{find_function_by_name, find_struct_by_name, parse_fixture};
use syn_parser::parser::nodes::{OutOfScopeReason, VisibilityResult};

mod public_items {
    use super::*;

    #[test]
    fn public_struct_same_module() {
        let graph = parse_fixture("visibility.rs").expect("Fixture failed to parse");
        let public_struct = find_struct_by_name(&graph, "PublicStruct").unwrap();
        
        let result = graph.resolve_visibility(public_struct.id, &["crate"]);
        assert!(
            matches!(result, VisibilityResult::Direct),
            "Public struct should be directly visible"
        );
    }

    #[test]
    fn public_function_cross_module() {
        let graph = parse_fixture("modules.rs").expect("Fixture failed to parse");
        let outer_func = find_function_by_name(&graph, "outer_function").unwrap();
        
        let result = graph.resolve_visibility(outer_func.id, &["crate", "unrelated"]);
        assert!(
            matches!(result, VisibilityResult::Direct),
            "Public function should be visible across modules"
        );
    }
}

mod restricted_visibility {
    use super::*;

    #[test]
    fn pub_crate_visibility() {
        let graph = parse_fixture("visibility.rs").expect("Fixture failed to parse");
        let crate_struct = find_struct_by_name(&graph, "CrateVisibleStruct").unwrap();
        
        // Should be visible anywhere in crate
        let result = graph.resolve_visibility(crate_struct.id, &["crate", "any_module"]);
        assert!(
            matches!(result, VisibilityResult::Direct),
            "pub(crate) item should be visible within crate"
        );
    }

    #[test]
    fn pub_in_path_restriction() {
        let graph = parse_fixture("restricted_visibility.rs").expect("Fixture failed to parse");
        let restricted_fn = find_function_by_name(&graph, "restricted_fn").unwrap();
        
        // Allowed path
        let allowed_result = graph.resolve_visibility(restricted_fn.id, &["crate", "outer"]);
        assert!(
            matches!(allowed_result, VisibilityResult::Direct),
            "Should be visible in specified path"
        );
        
        // Denied path
        let denied_result = graph.resolve_visibility(restricted_fn.id, &["crate", "unrelated"]);
        assert!(
            matches!(
                denied_result,
                VisibilityResult::OutOfScope {
                    reason: OutOfScopeReason::SuperRestricted,
                    ..
                }
            ),
            "Should be blocked outside specified path"
        );
    }
}
