#![cfg(feature = "module_path_tracking")]
use crate::common::*;
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
    let graph = parse_fixture("mixed_sample.rs");

    // Should have root + private_module + public_module
    assert_eq!(graph.modules.len(), 3);

    let root = graph.modules.iter().find(|m| m.name == "root").unwrap();
    assert_eq!(root.path, vec!["crate"]);

    // Verify non-module items were still parsed
    assert!(!graph.functions.is_empty(), "No functions found");
    assert!(!graph.defined_types.is_empty(), "No types found");
    assert!(!graph.traits.is_empty(), "No traits found");
}

#[test]
fn test_private_module_handling() {
    let graph = parse_fixture("mixed_sample.rs");

    let private_mod = graph.modules.iter().find(|m| m.name == "private_module");
    assert!(
        private_mod.is_some(),
        "private_module not found. Found modules: {:?}",
        graph.modules.iter().map(|m| &m.name).collect::<Vec<_>>()
    );

    let private_mod = private_mod.unwrap();
    assert_eq!(
        private_mod.visibility,
        VisibilityKind::Restricted(vec!["super".to_string()]),
        "private_module has wrong visibility"
    );

    #[cfg(feature = "module_path_tracking")]
    assert_eq!(
        private_mod.path,
        vec!["crate", "private_module"],
        "private_module has wrong path"
    );
}

#[test]
fn test_public_module_handling() {
    let graph = parse_fixture("mixed_sample.rs");

    let public_mod = graph.modules.iter().find(|m| m.name == "public_module");
    assert!(
        public_mod.is_some(),
        "public_module not found. Found modules: {:?}",
        graph.modules.iter().map(|m| &m.name).collect::<Vec<_>>()
    );

    let public_mod = public_mod.unwrap();
    assert_eq!(
        public_mod.visibility,
        VisibilityKind::Public,
        "public_module should be public"
    );

    #[cfg(feature = "module_path_tracking")]
    assert_eq!(
        public_mod.path,
        vec!["crate", "public_module"],
        "public_module has wrong path"
    );
}
