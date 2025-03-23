mod attribute_processing;
mod code_visitor;
mod state;
mod type_processing;

pub use self::analyze_code;
pub use code_visitor::CodeVisitor;
pub use state::VisitorState;

use crate::parser::graph::CodeGraph;
use std::path::Path;

pub fn analyze_code(file_path: &Path) -> Result<CodeGraph, syn::Error> {
    let file = syn::parse_file(&std::fs::read_to_string(file_path).unwrap())?;
    let mut visitor_state = state::VisitorState::new();

    // Create the root module first
    let root_module_id = visitor_state.next_node_id();
    visitor_state
        .code_graph
        .modules
        .push(crate::parser::nodes::ModuleNode {
            id: root_module_id,
            name: "root".to_string(),
            visibility: crate::parser::types::VisibilityKind::Inherited,
            attributes: Vec::new(),
            docstring: None,
            submodules: Vec::new(),
            items: Vec::new(),
            imports: Vec::new(),
            exports: Vec::new(),
        });

    let mut visitor = code_visitor::CodeVisitor::new(&mut visitor_state);
    visitor.visit_file(&file);

    // Add relations between root module and top-level items
    for module in &visitor_state.code_graph.modules {
        if module.id != root_module_id {
            visitor_state
                .code_graph
                .relations
                .push(crate::parser::relations::Relation {
                    source: root_module_id,
                    target: module.id,
                    kind: crate::parser::relations::RelationKind::Contains,
                });
        }
    }

    Ok(visitor_state.code_graph)
}
