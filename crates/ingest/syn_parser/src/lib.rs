pub mod parser;
pub mod serialization;
// pub mod analysis; // For future code analysis features

// Add the new discovery module, gated by the feature flag
#[cfg(feature = "uuid_ids")]
pub mod discovery;

// Re-export key items for easier access
pub use parser::visitor::start_parser_worker;
pub use parser::{create_parser_channel, CodeGraph, ParserMessage};

#[cfg(feature = "uuid_ids")]
pub use parser::visitor::analyze_file_phase2;
#[cfg(not(feature = "uuid_ids"))]
pub use parser::visitor::{analyze_code, analyze_files_parallel};

#[cfg(feature = "uuid_ids")]
pub use ploke_core::NodeId; // Re-export the enum from ploke-core
#[cfg(feature = "uuid_ids")]
pub use ploke_core::TypeId;
pub use serialization::ron::{save_to_ron, save_to_ron_threadsafe}; // Re-export the enum/struct from ploke-core

#[cfg(not(feature = "uuid_ids"))]
pub use crate::parser::nodes::NodeId; // Re-export the `usize` type alias
#[cfg(not(feature = "uuid_ids"))]
pub use crate::parser::types::TypeId; // Re-export the `usize` type alias
