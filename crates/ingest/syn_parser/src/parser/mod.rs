pub mod channel;
pub mod graph; // Make these public
pub mod nodes;
pub mod relations;
pub mod types;
pub mod utils;
pub mod visibility;
pub mod visitor;

// Re-export key items
pub use self::channel::{create_parser_channel, ParserMessage};
pub use self::graph::CodeGraph;
pub use self::utils::ExtractSpan;
pub use self::visitor::analyze_files_parallel;
