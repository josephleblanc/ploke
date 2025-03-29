#![cfg(feature = "visibility_resolution")]
#[cfg(feature = "module_path_tracking")]
use crate::common::{find_function_by_name, find_module_by_path, find_struct_by_name, parse_fixture};
use syn_parser::parser::nodes::{VisibilityResult, OutOfScopeReason};

fn test_module_path(segments: &[&str]) -> Vec<String> {
    segments.iter().map(|s| s.to_string()).collect()
}

#[test]
fn test_pub_in_path_restricted_function() {
    let graph = parse_fixture("restricted_visibility.rs").expect("Fixture failed to parse");

    let restricted_fn = find_function_by_name(&graph, "restricted_fn")
        .expect("restricted_fn not found in fixture");
    
    // Test allowed access from within the specified path
    let allowed_result = graph.resolve_visibility(
        restricted_fn.id,
        &test_module_path(&["crate", "outer"])
    );
    assert!(
        matches!(allowed_result, VisibilityResult::Direct),
        "\nrestricted_fn should be visible in outer module\nGot: {:?}",
        allowed_result
    );

    // Test denied access from unrelated module
    let denied_result = graph.resolve_visibility(
        restricted_fn.id,
        &test_module_path(&["crate", "unrelated"])
    );
    assert!(
        matches!(
            denied_result,
            VisibilityResult::OutOfScope {
                reason: OutOfScopeReason::SuperRestricted,
                ..
            }
        ),
        "\nrestricted_fn should be blocked outside specified path\nGot: {:?}",
        denied_result
    );
}

#[test]
fn test_pub_in_path_restricted_struct() {
    let graph = parse_fixture("restricted_visibility.rs").expect("Fixture failed to parse");

    #[cfg(feature = "verbose_debug")]
    {
        println!("\n=== Debugging restricted_visibility_tests ===");
        println!("Defined types in graph:");
        for ty in &graph.defined_types {
            match ty {
                TypeDefNode::Struct(s) => println!("Struct: {} (vis: {:?})", s.name, s.visibility),
                TypeDefNode::Enum(e) => println!("Enum: {} (vis: {:?})", e.name, e.visibility),
                _ => {}
            }
        }
        
        println!("\nModule paths in graph:");
        for m in &graph.modules {
            #[cfg(feature = "module_path_tracking")]
            println!("- {} (path: {:?})", m.name, m.path);
            #[cfg(not(feature = "module_path_tracking"))]
            println!("- {}", m.name);
        }
    }

    let restricted_struct = find_struct_by_name(&graph, "RestrictedStruct")
        .unwrap_or_else(|| {
            #[cfg(feature = "verbose_debug")]
            {
                eprintln!("\nAll structs in graph:");
                for ty in &graph.defined_types {
                    if let TypeDefNode::Struct(s) = ty {
                        eprintln!("- {} (vis: {:?})", s.name, s.visibility);
                    }
                }
            }
            panic!("RestrictedStruct not found in fixture");
        });
    
    // Test allowed access
    let allowed_result = graph.resolve_visibility(
        restricted_struct.id,
        &test_module_path(&["crate", "outer"])
    );
    assert!(
        matches!(allowed_result, VisibilityResult::Direct),
        "\nRestrictedStruct should be visible in outer module\nGot: {:?}",
        allowed_result
    );

    // Test denied access
    let denied_result = graph.resolve_visibility(
        restricted_struct.id,
        &test_module_path(&["crate", "unrelated"])
    );
    assert!(
        matches!(
            denied_result,
            VisibilityResult::OutOfScope {
                reason: OutOfScopeReason::SuperRestricted,
                allowed_scopes: Some(_)
            }
        ),
        "\nRestrictedStruct should be blocked outside specified path\nGot: {:?}",
        denied_result
    );
    
    // Verify restricted path is included in denied message
    if let VisibilityResult::OutOfScope { allowed_scopes: Some(scopes), .. } = denied_result {
        assert!(
            scopes.iter().any(|s| s.contains("outer")),
            "\nError message should include allowed scope 'outer'\nGot: {:?}",
            scopes
        );
    }
}
