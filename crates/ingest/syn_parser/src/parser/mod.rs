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
// #[cfg(not(feature = "uuid_ids"))]
// pub use self::types::TypeId;

#[cfg(feature = "uuid_ids")]
pub use self::visitor::analyze_files_parallel;

pub use self::utils::ExtractSpan;

#[cfg(not(feature = "uuid_ids"))]
pub use crate::parser::nodes::NodeId; // Re-export the `usize` type alias
#[cfg(not(feature = "uuid_ids"))]
pub use crate::parser::types::TypeId; // Re-export the `usize` type alias
                                      // #[cfg(not(feature = "uuid"))]
                                      // pub use self::visitor::analyze_code;
