use serde::{Deserialize, Serialize};
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

#[cfg(feature = "uuid_ids")]
use {
    super::nodes::ModuleNode,          // Moved ModuleNode import here
    crate::discovery::DiscoveryOutput, // Import DiscoveryOutput
    crate::parser::relations::{GraphId, Relation}, // Assuming Relation is in parser::relations
    ploke_core::NodeId,
    rayon::prelude::*, // Import rayon traits
    uuid::Uuid,
};
#[cfg(not(feature = "uuid_ids"))]
use {
    super::nodes::ModuleNode, crate::parser::nodes::NodeId, crate::parser::relations::Relation,
    rayon::prelude::*,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct AnalyzedCodeGraph {
    graph: CodeGraph,
    file_path: PathBuf,
    crate_namespace: Uuid, // Context passed from caller
}

/// Analyze a single file and return the code graph
#[cfg(not(feature = "uuid_ids"))]
pub fn analyze_code(file_path: &Path) -> Result<CodeGraph, syn::Error> {
    let file = syn::parse_file(&std::fs::read_to_string(file_path).unwrap())?;
    let mut visitor_state = VisitorState::new();

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
        #[cfg(feature = "uuid_ids")]
        tracking_hash: None, // Placeholder
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

/// Analyze a single file for Phase 2 (UUID Path) - The Worker Function
/// Receives context from analyze_files_parallel.
#[cfg(feature = "uuid_ids")]
pub fn analyze_file_phase2(
    file_path: &Path,
    crate_namespace: Uuid, // Context passed from caller
) -> Result<CodeGraph, syn::Error> {
    // Consider a more specific Phase2Error later
    let file_content = std::fs::read_to_string(file_path).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Failed to read file {}: {}", file_path.display(), e),
        )
    })?;
    let file = syn::parse_file(&file_content)?;

    // 1. Create VisitorState with the provided context
    let mut state = state::VisitorState::new(crate_namespace, file_path.to_path_buf());

    // 2. Generate root module ID using context
    // Context: crate_namespace, file_path, empty relative path [], name "crate"
    let root_module_id = NodeId::generate_synthetic(
        crate_namespace,
        file_path,
        &[], // Empty relative path for crate root
        "crate",
        (0, 0), // Span, there should be no other items with a (0, 0) span and this makes sense for
                // root crate (almost, probably would make more sense as (0, <file byte length>))
    );
    state.current_module_path = vec!["crate".to_string()];

    // 3. Create the root module node
    state.code_graph.modules.push(ModuleNode {
        id: root_module_id,
        name: "crate".to_string(),
        visibility: crate::parser::types::VisibilityKind::Public,
        attributes: Vec::new(),
        docstring: None,
        submodules: Vec::new(),
        items: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        path: vec!["crate".to_string()],
        tracking_hash: None, // Root module conceptual, no specific content hash
    });

    // 4. Create and run the visitor
    let mut visitor = code_visitor::CodeVisitor::new(&mut state);
    visitor.visit_file(&file);

    // 5. Add relations using GraphId wrappers
    let module_ids: Vec<NodeId> = state.code_graph.modules.iter().map(|m| m.id).collect();
    for module_id in module_ids {
        if module_id != root_module_id {
            state.code_graph.relations.push(Relation {
                source: GraphId::Node(root_module_id),
                target: GraphId::Node(module_id),
                kind: crate::parser::relations::RelationKind::Contains,
            });
        }
    }

    // TODO: Add another debug_print_all_visible function under cfg "uuid_ids", since our recent
    // chagnes would break the normal version.
    // #[cfg(feature = "verbose_debug")]
    // visitor_state.code_graph.debug_print_all_visible();

    Ok(state.code_graph)
}

/// Process multiple files in parallel using rayon (UUID Path) - The Orchestrator
/// Takes DiscoveryOutput and distributes work to analyze_file_phase2.
#[cfg(feature = "uuid_ids")]
pub fn analyze_files_parallel(
    discovery_output: &DiscoveryOutput, // Takes output from Phase 1
    _num_workers: usize, // May not be directly used if relying on rayon's default pool size
) -> Vec<Result<CodeGraph, syn::Error>> {
    // Adjust error type if needed

    println!(
        // Temporary debug print
        "Starting Phase 2 Parallel Parse for {} crates...",
        discovery_output.crate_contexts.len()
    );

    discovery_output
        .crate_contexts
        .values() // Iterate over CrateContext values
        .par_bridge() // Bridge into a parallel iterator (efficient for HashMap values)
        .flat_map(|crate_context| {
            // Process each crate in parallel
            println!(
                // Temporary debug print
                "  Processing crate '{}' with {} files...",
                crate_context.name,
                crate_context.files.len()
            );
            // For each crate, parallelize over its files
            crate_context.files.par_iter().map(move |file_path| {
                // Move namespace into the closure
                // Call the single-file worker function with its specific context
                analyze_file_phase2(file_path, crate_context.namespace)
            })
        })
        .collect() // Collect all results (Result<CodeGraph, Error>) into a Vec
}

/// Process multiple files in parallel using rayon (Non-UUID Path)
#[cfg(not(feature = "uuid_ids"))]
pub fn analyze_files_parallel(
    file_paths: Vec<PathBuf>,
    num_workers: usize,
) -> Vec<Result<CodeGraph, syn::Error>> {
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_workers)
        .build()
        .unwrap();
    pool.install(|| {
        file_paths
            .par_iter()
            .map(|path| analyze_code(path)) // Call original analyze_code
            .collect()
    })
}

// start_parser_worker remains unchanged as it uses the non-UUID analyze_code
pub fn start_parser_worker(
    receiver: Receiver<ParserMessage>,
    sender: Sender<ParserMessage>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        for message in receiver.iter() {
            match message {
                ParserMessage::ParseFile(path) => {
                    #[cfg(not(feature = "uuid_ids"))]
                    // Only run this worker logic if not using UUIDs
                    {
                        let result = analyze_code(&path);
                        if sender.send(ParserMessage::ParseResult(result)).is_err() {
                            break; // Channel closed
                        }
                    }
                    #[cfg(feature = "uuid_ids")]
                    {
                        // This worker model doesn't fit the Phase 2 plan which uses
                        // analyze_files_parallel directly with DiscoveryOutput.
                        // Log an error or ignore if ParseFile is received under uuid_ids.
                        eprintln!("Warning: Received ParseFile message via channel, but running with uuid_ids feature. This path is not supported in Phase 2.");
                    }
                }
                ParserMessage::Shutdown => break,
                _ => {} // Ignore other message types
            }
        }
    })
}
