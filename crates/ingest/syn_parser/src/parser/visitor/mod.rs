use syn::visit::Visit;
mod attribute_processing;
mod code_visitor;
mod state;
mod type_processing;

pub use code_visitor::CodeVisitor;
pub use state::VisitorState;

use crate::parser::{channel::ParserMessage, graph::CodeGraph};
use flume::{Receiver, Sender};
use std::path::{Path, PathBuf};
use std::thread;

use super::nodes::ModuleNode;

/// Analyze a single file and return the code graph
pub fn analyze_code(file_path: &Path) -> Result<CodeGraph, syn::Error> {
    let file = syn::parse_file(&std::fs::read_to_string(file_path).unwrap())?;
    let mut visitor_state = state::VisitorState::new();

    // Create the root module first
    let root_module_id = visitor_state.next_node_id();

    visitor_state.current_module_path = vec!["crate".to_string()];

    visitor_state.code_graph.modules.push(ModuleNode {
        id: root_module_id,
        name: "root".to_string(),
        visibility: crate::parser::types::VisibilityKind::Inherited,
        attributes: Vec::new(),
        docstring: None,
        submodules: Vec::new(),
        items: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        path: vec!["crate".to_string()],
    });

    let mut visitor = code_visitor::CodeVisitor::new(&mut visitor_state);
    visitor.visit_file(&file);

    // Add relations between root module and top-level items
    for module in &visitor_state.code_graph.modules {
        if module.id != root_module_id {
            visitor_state
                .code_graph
                .relations
                .push(crate::parser::relations::Relation {
                    source: root_module_id,
                    target: module.id,
                    kind: crate::parser::relations::RelationKind::Contains,
                });
        }
    }
    #[cfg(feature = "verbose_debug")]
    visitor_state.code_graph.debug_print_all_visible();

    Ok(visitor_state.code_graph)
}

/// Start a background parser worker that processes files from a channel
pub fn start_parser_worker(
    receiver: Receiver<ParserMessage>,
    sender: Sender<ParserMessage>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for message in receiver.iter() {
            match message {
                ParserMessage::ParseFile(path) => {
                    let result = analyze_code(&path);
                    if sender.send(ParserMessage::ParseResult(result)).is_err() {
                        // Channel closed, exit the worker
                        break;
                    }
                }
                ParserMessage::Shutdown => {
                    // Received shutdown signal, exit the worker
                    break;
                }
                _ => {
                    // Ignore other message types
                }
            }
        }
    })
}

/// Process multiple files in parallel using rayon
pub fn analyze_files_parallel(
    file_paths: Vec<PathBuf>,
    _num_workers: usize, // Renamed as it might not be directly used in uuid_ids path yet
) -> Vec<Result<CodeGraph, syn::Error>> {
    // TODO: Return type might need adjustment for UUID path
    #[cfg(not(feature = "uuid_ids"))]
    {
        // --- Existing usize-based parallel implementation ---
        use rayon::prelude::*;

        // Create a thread pool with the specified number of workers
        let pool = rayon::ThreadPoolBuilder::new()
            .num_threads(_num_workers)
            .build()
            .unwrap();

        // Use the thread pool to process files in parallel
        pool.install(|| {
            file_paths
                .par_iter()
                .map(|path| analyze_code(path))
                .collect()
        })
    }
    #[cfg(feature = "uuid_ids")]
    {
        // --- New UUID-based implementation (Phase 1 + Phase 2 stub) ---
        use crate::discovery::{run_discovery_phase, DiscoveryError}; // Import discovery items

        // TODO: Determine project_root and target_crates properly.
        // This likely requires changes to how analyze_files_parallel is called,
        // or more sophisticated logic here to infer context from file_paths.
        // For now, using placeholders.
        let project_root = PathBuf::from("."); // Placeholder
                                               // Infer target crates from file paths (simplistic: assumes files are in crate roots/src)
        let target_crates_result: Result<Vec<PathBuf>, DiscoveryError> = file_paths
            .iter()
            .map(|p| {
                p.parent() // Get directory containing the file
                    .and_then(|dir| {
                        if dir.ends_with("src") {
                            dir.parent() // If in src, go up one level
                        } else {
                            Some(dir) // Assume it's already the crate root (simplistic)
                        }
                    })
                    .map(|p| p.to_path_buf())
                    .ok_or_else(|| DiscoveryError::CratePathNotFound { path: p.clone() })
                // Error if no parent
            })
            .collect::<Result<Vec<_>, _>>() // Collect potential crate roots
            .map(|mut paths| {
                paths.sort(); // Sort for deduplication
                paths.dedup(); // Remove duplicates
                paths
            });

        let target_crates = match target_crates_result {
            Ok(paths) => paths,
            Err(e) => {
                // Handle error determining target crates (e.g., return an error)
                // For now, just print and return empty results
                eprintln!("Error determining target crates: {:?}", e);
                return vec![]; // Or return a proper error Result
            }
        };

        // --- Phase 1: Discovery ---
        println!("Running Discovery Phase..."); // Temporary print
        let (discovery_output, discovery_errors) =
            run_discovery_phase(&project_root, &target_crates);

        // Log any errors encountered during discovery
        if !discovery_errors.is_empty() {
            eprintln!(
                "Discovery phase completed with errors: {:?}",
                discovery_errors
            );
            // Decide if we should proceed even with errors? For now, let's proceed
            // with the partial results. If discovery_output is empty and errors
            // occurred, we might want to return early.
            if discovery_output.crate_contexts.is_empty() {
                 eprintln!("No crates discovered successfully, aborting parse.");
                 return vec![]; // Or return a proper error
            }
        }

        println!(
            "Discovery successful. Found {} crates.",
            discovery_output.crate_contexts.len()
        );

        // --- Phase 2: Parallel Parse (Stub) ---
        // TODO: Implement the actual parallel parsing using discovery_output
        // This will involve:
        // 1. Distributing files from discovery_output.crate_contexts[crate].files
        //    to rayon workers.
        // 2. Passing the correct CrateContext (esp. namespace) to each worker.
        // 3. Modifying analyze_code or creating a new function for Phase 2 parsing
        //    that accepts CrateContext and generates synthetic UUIDs.
        // 4. Collecting partial CodeGraphs from workers.
        println!("TODO: Implement Phase 2 Parallel Parse using discovery output.");
        // For now, return empty results as Phase 2 is not implemented
        vec![]
    }
}
