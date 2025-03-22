# Boundary Crossing: Parser Coordinator(async) to Parallel Parser(rayon)

This example demonstrates:

1. **Domain Boundary**: `flume` channels create clear boundaries between Tokio and Rayon domains, with `recv_async()` and `send_async()` methods handling the async side elegantly.

2. **Shared State**: `DashMap` provides thread-safe shared state for tracking parsed files and dependencies, accessible from both Tokio and Rayon threads.

3. **Parser Coordinator**: Orchestrates work by prioritizing files based on the dependency graph and tracking parsing status.

4. **Parallel Parsing**: Uses Rayon for CPU-intensive parsing work while maintaining isolation from the async components.

The key advantage of `flume` here is its seamless support for both async and sync operations. You can use `send_async()`/`recv_async()` in Tokio contexts and regular `send()`/`recv()` in thread contexts without adapters or wrappers.

This architecture gives you the flexibility to scale both I/O and CPU-intensive components independently while maintaining clean boundaries between them.

Here's an example showing how to use `flume` at the boundary between the Parser Coordinator and Parallel Parsing components, with `dashmap` for shared state:

```rust
use dashmap::DashMap;
use flume::{Receiver, Sender};
use rayon::ThreadPoolBuilder;
use std::path::{Path, PathBuf};
use std::sync::Arc;

// Types representing our domain model
struct ParsedFile {
    path: PathBuf,
    items: Vec<RustItem>,
    // Other parsed information
}

enum RustItem {
    Function { name: String, /* other fields */ },
    Struct { name: String, /* other fields */ },
    // Other Rust items
}

struct ParseJob {
    path: PathBuf,
    priority: usize,
    // Metadata about the parsing job
}

// Main Parser Coordinator
struct ParserCoordinator {
    // Channel for receiving file change events from Tokio
    file_events_rx: Receiver<PathBuf>,
    
    // Channel for sending parse jobs to worker threads
    parse_job_tx: Sender<ParseJob>,
    
    // Channel for receiving parsed results
    parsed_results_rx: Receiver<ParsedFile>,
    
    // Shared state tracking what's been parsed
    parsed_files: Arc<DashMap<PathBuf, bool>>,
    
    // Dependency graph of files
    dependencies: Arc<DashMap<PathBuf, Vec<PathBuf>>>,
}

impl ParserCoordinator {
    fn new(file_events_rx: Receiver<PathBuf>) -> (Self, Sender<ParsedFile>, Receiver<ParseJob>) {
        // Create channels for parsing pipeline
        let (parse_job_tx, parse_job_rx) = flume::bounded(100);
        let (parsed_results_tx, parsed_results_rx) = flume::bounded(100);
        
        let coordinator = ParserCoordinator {
            file_events_rx,
            parse_job_tx,
            parsed_results_rx,
            parsed_files: Arc::new(DashMap::new()),
            dependencies: Arc::new(DashMap::new()),
        };
        
        (coordinator, parsed_results_tx, parse_job_rx)
    }
    
    async fn run(&self) {
        // Process incoming file events from Tokio
        while let Ok(file_path) = self.file_events_rx.recv_async().await {
            // Check if this file needs parsing
            if self.should_parse(&file_path) {
                // Determine priority based on dependencies
                let priority = self.calculate_priority(&file_path);
                
                // Mark as queued for parsing
                self.parsed_files.insert(file_path.clone(), false);
                
                // Send to parsing workers
                let job = ParseJob {
                    path: file_path,
                    priority,
                };
                
                // This is where we cross from Tokio to Rayon's domain
                self.parse_job_tx.send_async(job).await.unwrap();
            }
        }
    }
    
    async fn process_results(&self) {
        // Process results coming back from Rayon workers
        while let Ok(parsed_file) = self.parsed_results_rx.recv_async().await {
            // Mark as successfully parsed
            self.parsed_files.insert(parsed_file.path.clone(), true);
            
            // Update dependencies based on imports found
            // (simplified for example)
            
            // Here we'd also send to the database writer component
            // which would be another async task in Tokio
        }
    }
    
    fn should_parse(&self, path: &Path) -> bool {
        // Check if we need to parse this file based on our current state
        !self.parsed_files.contains_key(path) || 
        self.dependencies_changed(path)
    }
    
    fn calculate_priority(&self, path: &Path) -> usize {
        // Calculate priority based on dependency graph
        // Files with many dependents get higher priority
        self.dependencies
            .iter()
            .filter(|entry| entry.value().contains(path))
            .count()
    }
    
    fn dependencies_changed(&self, path: &Path) -> bool {
        // Check if any dependencies have changed
        if let Some(deps) = self.dependencies.get(path) {
            for dep in deps.value() {
                if let Some(parsed) = self.parsed_files.get(dep) {
                    if !*parsed.value() {
                        return true;
                    }
                }
            }
        }
        false
    }
}

// Setup for the parallel parsing component
fn setup_parallel_parsing(parse_job_rx: Receiver<ParseJob>, parsed_results_tx: Sender<ParsedFile>) {
    // Create a dedicated thread pool for parsing
    let pool = ThreadPoolBuilder::new()
        .num_threads(num_cpus::get())
        .build()
        .unwrap();
    
    // Spawn a dedicated thread to manage the work distribution
    std::thread::spawn(move || {
        // This thread pulls from the channel and dispatches to Rayon
        while let Ok(job) = parse_job_rx.recv() {
            let results_tx = parsed_results_tx.clone();
            
            // Submit this job to the Rayon thread pool
            pool.spawn(move || {
                // Parse the file (using your actual parsing logic)
                let parsed_file = parse_file(&job.path);
                
                // Send results back across the boundary to Tokio
                // This is where we cross back from Rayon to Tokio
                results_tx.send(parsed_file).unwrap();
            });
        }
    });
}

// Simulate parsing a file
fn parse_file(path: &Path) -> ParsedFile {
    // Your actual parsing logic using syn would go here
    ParsedFile {
        path: path.to_owned(),
        items: vec![],
    }
}

// Main function showing how to wire everything together
async fn main() {
    // Create a channel for file events (from file watcher)
    let (file_events_tx, file_events_rx) = flume::bounded(100);
    
    // Setup file watcher (in Tokio) - simplified
    tokio::spawn(async move {
        // Simulate file events
        for path in ["src/main.rs", "src/lib.rs"].iter() {
            file_events_tx.send_async(PathBuf::from(path)).await.unwrap();
        }
    });
    
    // Create and setup the parser coordinator
    let (coordinator, parsed_results_tx, parse_job_rx) = ParserCoordinator::new(file_events_rx);
    
    // Setup the parallel parsing component
    setup_parallel_parsing(parse_job_rx, parsed_results_tx);
    
    // Run the coordinator
    let coordinator_handle = tokio::spawn(async move {
        let process_handle = tokio::spawn(coordinator.process_results());
        coordinator.run().await;
        process_handle.await.unwrap();
    });
    
    // Wait for everything to complete
    coordinator_handle.await.unwrap();
}
```

