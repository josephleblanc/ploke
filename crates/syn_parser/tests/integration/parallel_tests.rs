#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use syn_parser::parser::analyze_files_parallel;

    #[test]
    fn test_parallel_parsing() {
        // Create a list of files to parse
        let file_paths = vec![
            PathBuf::from("tests/fixtures/functions.rs"),
            PathBuf::from("tests/fixtures/structs.rs"),
            PathBuf::from("tests/fixtures/enums.rs"),
            PathBuf::from("tests/fixtures/traits.rs"),
        ];

        // Parse files in parallel
        let results = analyze_files_parallel(file_paths, 4);

        // Check that all files were parsed successfully
        for result in results {
            let code_graph = result.expect("Failed to parse file");
            assert!(
                !code_graph.type_graph.is_empty(),
                "No types found in the parsed file"
            );
        }
    }
}
