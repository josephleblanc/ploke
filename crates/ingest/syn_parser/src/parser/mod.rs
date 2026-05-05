pub mod channel;
pub mod diagnostics;
pub mod graph; // Make these public
pub mod nodes;
pub mod relations;
#[cfg(feature = "typed_type_graph")]
pub mod type_nodes;
pub mod types;
pub mod utils;
pub mod visibility;
pub mod visitor;

// Re-export key items
pub use self::channel::{ParserMessage, create_parser_channel};
pub use self::graph::{CodeGraph, ParsedCodeGraph};
pub use self::utils::ExtractSpan;
pub use self::visitor::analyze_files_parallel;
