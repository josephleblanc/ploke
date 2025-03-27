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
    visitor_state.code_graph.modules.push(ModuleNode {
        id: root_module_id,
        name: "root".to_string(),
        // TODO: Consider whether implementing this even makes sense.
        // span: ???
        visibility: crate::parser::types::VisibilityKind::Inherited,
        attributes: Vec::new(),
        docstring: None,
        submodules: Vec::new(),
        items: Vec::new(),
        imports: Vec::new(),
        exports: Vec::new(),
        path: todo!(),
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
    num_workers: usize,
) -> Vec<Result<CodeGraph, syn::Error>> {
    use rayon::prelude::*;

    // Create a thread pool with the specified number of workers
    let pool = rayon::ThreadPoolBuilder::new()
        .num_threads(num_workers)
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
