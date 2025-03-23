pub mod parser;
pub mod serialization;
// pub mod analysis; // For future code analysis features

// Re-export key items for easier access
pub use parser::{analyze_code, analyze_files_parallel, create_parser_channel, CodeGraph, ParserMessage};
pub use parser::visitor::start_parser_worker;
pub use serialization::ron::{save_to_ron, save_to_ron_threadsafe};
