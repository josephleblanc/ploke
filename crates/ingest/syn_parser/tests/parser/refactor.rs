#![cfg(feature = "module_path_tracking")]
use crate::common::parse_fixture;
use syn_parser::{parser::types::VisibilityKind, CodeGraph};

#[test]
fn test_module_path_tracking_basic() {
    let graph = parse_fixture("modules.rs");

    let outer = graph.modules.iter().find(|m| m.name == "outer").unwrap();
    assert_eq!(outer.path, vec!["crate", "outer"]);

    let inner = graph.modules.iter().find(|m| m.name == "inner").unwrap();
    assert_eq!(inner.path, vec!["crate", "outer", "inner"]);
}

#[test]
fn test_module_path_serialization() {
    let graph = parse_fixture("modules.rs");
    let serialized = ron::to_string(&graph).unwrap();
    let deserialized: CodeGraph = ron::from_str(&serialized).unwrap();

    let outer = deserialized
        .modules
        .iter()
        .find(|m| m.name == "outer")
        .unwrap();
    assert_eq!(outer.path, vec!["crate", "outer"]);
}

#[test]
#[cfg(not(feature = "module_path_tracking"))]
fn test_module_visibility() {
    let graph = parse_fixture("modules.rs");
    let outer = graph.modules.iter().find(|m| m.name == "outer").unwrap();
    assert_eq!(outer.visibility, VisibilityKind::Public);
}

#[test]
#[cfg(feature = "module_path_tracking")]
fn test_module_visibility() {
    let graph = parse_fixture("modules.rs");
    let outer = graph.modules.iter().find(|m| m.name == "outer").unwrap();
    assert_eq!(
        outer.visibility,
        VisibilityKind::Restricted(vec!["super".to_string()])
    );
}

#[test]
fn test_root_module_path() {
    let graph = parse_fixture("modules.rs");
    let root = graph.modules.first().unwrap();
    assert_eq!(root.path, vec!["crate"]);
}

#[test]
fn test_non_module_items_ignored() {
    let graph = parse_fixture("sample.rs");
    
    // Should only have the root module
    assert_eq!(graph.modules.len(), 1);
    let root = &graph.modules[0];
    assert_eq!(root.name, "root");
    assert_eq!(root.path, vec!["crate"]);
    
    // Verify non-module items were still parsed
    assert!(!graph.functions.is_empty());
    assert!(!graph.defined_types.is_empty());
    assert!(!graph.traits.is_empty());
}

#[test]
fn test_private_module_handling() {
    let graph = parse_fixture("sample.rs");
    
    // The private_module in sample.rs should be captured
    let private_mod = graph.modules.iter().find(|m| m.name == "private_module");
    assert!(private_mod.is_some(), "Private module should be captured");
    
    let private_mod = private_mod.unwrap();
    assert_eq!(
        private_mod.visibility,
        VisibilityKind::Restricted(vec!["super".to_string()])
    );
    
    #[cfg(feature = "module_path_tracking")]
    assert_eq!(private_mod.path, vec!["crate", "private_module"]);
}

#[test]
fn test_public_module_handling() {
    let graph = parse_fixture("sample.rs");
    
    // The public_module in sample.rs should be captured
    let public_mod = graph.modules.iter().find(|m| m.name == "public_module");
    assert!(public_mod.is_some(), "Public module should be captured");
    
    let public_mod = public_mod.unwrap();
    assert_eq!(public_mod.visibility, VisibilityKind::Public);
    
    #[cfg(feature = "module_path_tracking")]
    assert_eq!(public_mod.path, vec!["crate", "public_module"]);
}
