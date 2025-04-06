pub mod channel;
pub mod graph; // Make these public
pub mod nodes;
pub mod relations;
pub mod types;
pub mod utils;
pub mod visitor;

// Re-export key items
pub use self::channel::{create_parser_channel, ParserMessage};
pub use self::graph::CodeGraph;
#[cfg(not(feature = "uuid"))]
pub use self::types::TypeId; // legacy version re-exports here, prefer direct imports on new
                             // version.
pub use self::utils::ExtractSpan;
pub use self::visitor::analyze_files_parallel;

#[cfg(not(feature = "uuid"))]
pub use self::visitor::analyze_code;
