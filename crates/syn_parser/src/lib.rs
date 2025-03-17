pub mod parser;
pub mod serialization;
// pub mod analysis; // For future code analysis features

// Re-export key items for easier access
pub use parser::{analyze_code, CodeGraph};
pub use serialization::ron::save_to_ron;
