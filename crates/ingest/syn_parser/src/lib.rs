pub mod discovery;
pub mod parser;

// Re-export key items for easier access
pub use parser::visitor::analyze_file_phase2;
pub use parser::{create_parser_channel, CodeGraph, ParserMessage};
pub use ploke_core::NodeId; // Re-export the enum from ploke-core
pub use ploke_core::TypeId; // Re-export the enum/struct from ploke-core
