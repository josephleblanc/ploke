use crate::tests::common::parse_fixture;
use crate::parser::visitor::analyze_code;
use crate::parser::graph::CodeGraph;

#[test]
fn test_module_path_tracking_basic() {
    let graph = parse_fixture("../tests/fixtures/modules.rs");
    
    #[cfg(feature = "module_path_tracking")]
    {
        let outer = graph.modules.iter().find(|m| m.name == "outer").unwrap();
        assert_eq!(outer.path, vec!["crate", "outer"]);
        
        let inner = graph.modules.iter().find(|m| m.name == "inner").unwrap();
        assert_eq!(inner.path, vec!["crate", "outer", "inner"]);
    }
}

#[test]
fn test_module_path_serialization() {
    let graph = parse_fixture("modules.rs");
    let serialized = ron::to_string(&graph).unwrap();
    let deserialized: CodeGraph = ron::from_str(&serialized).unwrap();
    
    #[cfg(feature = "module_path_tracking")]
    {
        let outer = deserialized.modules.iter().find(|m| m.name == "outer").unwrap();
        assert_eq!(outer.path, vec!["crate", "outer"]);
    }
}

#[test]
fn test_root_module_path() {
    let graph = parse_fixture("modules.rs");
    let root = graph.modules.first().unwrap();
    
    #[cfg(feature = "module_path_tracking")]
    assert_eq!(root.path, vec!["crate"]);
}
