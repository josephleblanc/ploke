#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};
    use std::sync::{Arc, Barrier};
    use std::thread;
    use syn_parser::parser::visitor::analyze_code;
    use syn_parser::serialization::ron::save_to_ron_threadsafe;

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
    
    #[test]
    fn test_concurrent_access_to_shared_graph() {
        // Parse a file
        let fixture_path = Path::new("tests/fixtures/functions.rs");
        let code_graph = analyze_code(fixture_path).expect("Failed to parse file");
        
        // Wrap in Arc for thread-safety
        let arc_graph = Arc::new(code_graph);
        
        // Number of concurrent threads to test with
        const NUM_THREADS: usize = 5;
        
        // Create a barrier to synchronize thread start
        let barrier = Arc::new(Barrier::new(NUM_THREADS));
        
        // Create temporary output paths for each thread
        let mut output_paths = Vec::with_capacity(NUM_THREADS);
        for i in 0..NUM_THREADS {
            output_paths.push(PathBuf::from(format!("tests/data/concurrent_test_{}.ron", i)));
        }
        
        // Spawn threads that all access the same graph concurrently
        let mut handles = Vec::with_capacity(NUM_THREADS);
        
        for (i, path) in output_paths.iter().enumerate() {
            let graph_clone = Arc::clone(&arc_graph);
            let path_clone = path.clone();
            let barrier_clone = Arc::clone(&barrier);
            
            let handle = thread::spawn(move || {
                // Wait for all threads to be ready
                barrier_clone.wait();
                
                // All threads will try to access the graph and serialize it simultaneously
                save_to_ron_threadsafe(graph_clone, &path_clone)
                    .expect(&format!("Thread {} failed to save graph", i));
                
                // Verify the file exists
                assert!(path_clone.exists(), "Output file was not created by thread {}", i);
            });
            
            handles.push(handle);
        }
        
        // Wait for all threads to complete
        for handle in handles {
            handle.join().unwrap();
        }
        
        // Clean up
        for path in output_paths {
            if path.exists() {
                std::fs::remove_file(path).expect("Failed to remove test file");
            }
        }
    }
}
