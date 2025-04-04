use crate::parser::graph::CodeGraph; // Update the import path
use ron::ser::{to_string_pretty, PrettyConfig};
use std::fs::File;
use std::io::Write;
use std::path::Path;
use std::sync::Arc;
// RON format serialization

/// Save a code graph to a RON file
pub fn save_to_ron(code_graph: &CodeGraph, output_path: &Path) -> std::io::Result<()> {
    let pretty_config = PrettyConfig::default();
    let ron_string = to_string_pretty(code_graph, pretty_config).expect("Serialization failed");

    let mut output_file = File::create(output_path)?;
    output_file.write_all(ron_string.as_bytes())?;
    Ok(())
}

/// Thread-safe version that accepts an Arc<CodeGraph>
pub fn save_to_ron_threadsafe(code_graph: Arc<CodeGraph>, output_path: &Path) -> std::io::Result<()> {
    let pretty_config = PrettyConfig::default();
    let ron_string = to_string_pretty(&*code_graph, pretty_config).expect("Serialization failed");

    let mut output_file = File::create(output_path)?;
    output_file.write_all(ron_string.as_bytes())?;
    Ok(())
}
