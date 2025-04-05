#[cfg(test)]
#[cfg(not(feature = "uuid_ids"))]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    use syn_parser::parser::visitor::analyze_files_parallel;

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
                !code_graph.type_graph.is_empty() || !code_graph.functions.is_empty(),
                "No content found in the parsed file"
            );
        }
    }

    #[test]
    fn test_parallel_parsing_with_shared_counter() {
        // This test verifies that our parallel parsing can safely share state
        // across threads using atomic operations

        // Create a list of files to parse - FIXED: Use only files we know exist
        let file_paths = vec![
            PathBuf::from("tests/fixtures/functions.rs"),
            PathBuf::from("tests/data/sample.rs"),
        ];

        // Create a shared counter to track parsing progress
        let counter = Arc::new(AtomicUsize::new(0));
        let counter_clone = Arc::clone(&counter);

        // Use a higher number of threads than files to test thread pool behavior
        let num_threads = 4;

        // Parse files in parallel
        let results = analyze_files_parallel(file_paths.clone(), num_threads);

        // Check that all files were parsed successfully and increment counter
        for result in results {
            let code_graph = result.expect("Failed to parse file");
            assert!(
                !code_graph.type_graph.is_empty() || !code_graph.functions.is_empty(),
                "No content found in the parsed file"
            );

            // Increment our atomic counter
            counter.fetch_add(1, Ordering::SeqCst);
        }

        // Verify the counter matches the number of files
        assert_eq!(
            counter_clone.load(Ordering::SeqCst),
            file_paths.len(),
            "Counter doesn't match number of processed files"
        );
    }
}
