#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use syn_parser::{analyze_code, save_to_ron_threadsafe};

    #[test]
    fn test_threadsafe_serialization() {
        // Parse a file
        let fixture_path = Path::new("tests/fixtures/functions.rs");
        let code_graph = analyze_code(fixture_path).expect("Failed to parse file");
        
        // Wrap in Arc for thread-safety
        let arc_graph = Arc::new(code_graph);
        
        // Save to a temporary file
        let output_path = PathBuf::from("tests/data/threadsafe_test.ron");
        save_to_ron_threadsafe(arc_graph, &output_path).expect("Failed to save graph");
        
        // Verify the file exists
        assert!(output_path.exists(), "Output file was not created");
        
        // Clean up
        std::fs::remove_file(output_path).expect("Failed to remove test file");
    }
}
