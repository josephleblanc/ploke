pub mod discovery;
pub mod error;
pub mod parser;

// Re-export key items for easier access
pub use parser::visitor::analyze_file_phase2;
pub use parser::{create_parser_channel, CodeGraph, ParserMessage};
pub use ploke_core::NodeId; // Re-export the enum from ploke-core
pub use ploke_core::TypeId; // Re-export the enum/struct from ploke-core

#[cfg(test)]
pub mod test_utils {
    pub use ploke_core::NodeId;

    pub use crate::parser::graph::CodeGraph;
    pub use crate::{
        error::SynParserError,
        parser::module_tree::{ModuleTree, ModuleTreeError},
    };
    pub fn test_build_module_tree(graph: &CodeGraph) -> Result<ModuleTree, SynParserError> {
        graph.build_module_tree()
    }

    pub fn test_shortest_public_path(
        module_tree: &ModuleTree,
        item_id: NodeId,
        graph: &CodeGraph,
    ) -> Result<Vec<String>, ModuleTreeError> {
        module_tree.test_shortest_public_path(item_id, graph)
    }
}
