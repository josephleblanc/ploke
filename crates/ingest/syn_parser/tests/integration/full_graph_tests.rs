#[cfg(test)]
#[cfg(not(feature = "uuid_ids"))]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::Arc;
    use std::thread;
    use syn_parser::parser::visitor::analyze_code;
    use syn_parser::serialization::ron::save_to_ron_threadsafe;

    #[test]
    fn test_full_graph_generation_and_serialization() {
        // Parse a file with diverse Rust constructs
        let fixture_path = Path::new("tests/data/sample.rs");
        let code_graph = analyze_code(fixture_path).expect("Failed to parse file");

        // Verify the graph contains expected elements
        assert!(!code_graph.functions.is_empty(), "No functions found");
        assert!(
            !code_graph.defined_types.is_empty(),
            "No defined types found"
        );
        assert!(!code_graph.type_graph.is_empty(), "No types found");
        assert!(!code_graph.impls.is_empty(), "No impls found");
        assert!(!code_graph.traits.is_empty(), "No traits found");

        // Save to a temporary file
        let output_path = PathBuf::from("tests/data/full_graph_test.ron");
        let arc_graph = Arc::new(code_graph);
        save_to_ron_threadsafe(arc_graph, &output_path).expect("Failed to save graph");

        // Verify the file exists
        assert!(output_path.exists(), "Output file was not created");

        // Clean up
        std::fs::remove_file(output_path).expect("Failed to remove test file");
    }

    #[test]
    fn test_concurrent_dashmap_access() {
        // This test verifies that the DashMap in VisitorState handles concurrent access correctly

        // Parse a complex file that will populate the type_map
        let fixture_path = Path::new("tests/data/sample.rs");
        let code_graph = analyze_code(fixture_path).expect("Failed to parse file");

        // Wrap in Arc for thread-safety
        let arc_graph = Arc::new(code_graph);

        // Spawn multiple threads that will all try to read from the graph concurrently
        const NUM_THREADS: usize = 10;
        let mut handles = Vec::with_capacity(NUM_THREADS);

        for i in 0..NUM_THREADS {
            let graph_clone = Arc::clone(&arc_graph);

            let handle = thread::spawn(move || {
                // Access different parts of the graph concurrently
                let _functions = &graph_clone.functions;
                let _types = &graph_clone.type_graph;
                let _impls = &graph_clone.impls;

                // Just to ensure the compiler doesn't optimize away our reads
                assert!(
                    !graph_clone.functions.is_empty(),
                    "Thread {} found empty functions",
                    i
                );
                assert!(
                    !graph_clone.type_graph.is_empty(),
                    "Thread {} found empty types",
                    i
                );
            });

            handles.push(handle);
        }

        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
    }

    // TODO: Implement stress test for thread safety
    // Steps to set up a proper stress test:
    // 1. Create a large corpus of Rust files (50+ files of varying complexity)
    //    - Could use real-world crates or generate synthetic test files
    //    - Ensure files have diverse Rust constructs (traits, impls, macros, etc.)
    //
    // 2. Set up a test harness that:
    //    - Spawns many worker threads (20+)
    //    - Processes files in parallel with high concurrency
    //    - Has threads randomly accessing shared data structures
    //    - Introduces random delays to increase chance of race conditions
    //
    // 3. Verification steps:
    //    - Compare results with single-threaded parsing to ensure consistency
    //    - Run with thread sanitizer enabled to detect data races
    //    - Verify no deadlocks occur under heavy load
    //    - Check memory usage doesn't grow unexpectedly
    //
    // 4. Consider using tools like loom for systematic concurrency testing
    //    - https://github.com/tokio-rs/loom
}
