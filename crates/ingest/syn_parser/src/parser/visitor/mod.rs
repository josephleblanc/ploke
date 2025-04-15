use ploke_core::ItemKind;
use serde::{Deserialize, Serialize};
use syn::visit::Visit;
mod attribute_processing;
mod code_visitor;
mod state;
mod type_processing;

pub use code_visitor::CodeVisitor;
pub use state::VisitorState;

use crate::parser::graph::CodeGraph;

use std::path::{Component, Path, PathBuf}; // Add Path and Component

/// Helper function to derive the logical module path from a file path relative to src
///
/// Examples:
///  `some/user/dir/crate_name/src/main.rs` -> ["crate"]
///  `some/user/dir/crate_name/src/lib.rs` -> ["crate"]
///  `some/user/dir/crate_name/src/mod_one.rs` -> ["crate", "mod_one"]
///  `some/user/dir/crate_name/src/mod_two/mod.rs` -> ["crate", "mod_two"]
///  `some/user/dir/crate_name/src/mod_two/some_mod.rs` -> ["crate", "mod_two", "some_mod"]
///  `some/user/dir/crate_name/src/mod_two/mod_three/mod.rs` -> ["crate", "mod_two", "mod_three"]
///  .. etc
///  
// Goes through the file path provided by Phase 1's DiscoveryOutput, and processes the string into
// the module path for the given file. Note that this is a helpful step in later resolution
// handled in Phase 3, but is not sufficient to develop a fully reliable module path due to the
// possibility of the #[path = ..] attribute.
// DO NOT USE FOR NodeId::Resolved CREATION OR TypeId::Resolved, defer resolution to Phase 3
fn derive_logical_path(crate_src_dir: &Path, file_path: &Path) -> Vec<String> {
    let mut logical_path = vec!["crate".to_string()];

    // Get the path relative to the src directory
    if let Ok(relative_path) = file_path.strip_prefix(crate_src_dir) {
        let mut components: Vec<String> = relative_path
            .components()
            .filter_map(|comp| match comp {
                Component::Normal(name) => name.to_str().map(|s| s.to_string()),
                _ => None,
            })
            .collect();

        // Check if the last component is a filename like "mod.rs" or "lib.rs" or "main.rs"
        if let Some(last) = components.last() {
            if last == "mod.rs" || last == "lib.rs" || last == "main.rs" {
                components.pop(); // Remove "mod.rs", "lib.rs", or "main.rs"
            } else if let Some(stem) = Path::new(&last.clone())
                .file_stem()
                .and_then(|s| s.to_str())
            {
                // Replace the filename with its stem
                if let Some(last_mut) = components.last_mut() {
                    *last_mut = stem.to_string();
                }
            }
        }
        logical_path.extend(components);
    } else {
        // Fallback or error handling if strip_prefix fails
        // For now, just return ["crate"] as a basic fallback
        eprintln!(
            "Warning: Could not strip prefix '{}' from '{}'. Falling back to ['crate'].",
            crate_src_dir.display(),
            file_path.display()
        );
    }

    logical_path
}

use {
    super::nodes::ModuleNode,          // Moved ModuleNode import here
    crate::discovery::DiscoveryOutput, // Import DiscoveryOutput
    crate::parser::relations::{GraphId, Relation}, // Assuming Relation is in parser::relations
    ploke_core::NodeId,
    rayon::prelude::*, // Import rayon traits
    uuid::Uuid,
};

#[derive(Debug, Serialize, Deserialize, Clone)]
pub struct ParsedCodeGraph {
    /// The absolute path of the file that was parsed.                                
    pub file_path: PathBuf,
    /// The UUID namespace of the crate this file belongs to.                         
    pub crate_namespace: Uuid,
    /// The resulting code graph from parsing the file.                               
    pub graph: CodeGraph,
    // Potentially add other relevant context if needed in the future
}

/// Analyze a single file for Phase 2 (UUID Path) - The Worker Function
/// Receives context from analyze_files_parallel.
pub fn analyze_file_phase2(
    file_path: PathBuf,
    crate_namespace: Uuid,            // Context passed from caller
    logical_module_path: Vec<String>, // NEW: The derived logical path for this file
) -> Result<ParsedCodeGraph, syn::Error> {
    // Consider a more specific Phase2Error later

    use attribute_processing::{extract_file_level_attributes, extract_file_level_docstring};

    use super::nodes::ModuleDef;
    let file_content = std::fs::read_to_string(&file_path).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Failed to read file {}: {}", file_path.display(), e),
        )
    })?;
    let file = syn::parse_file(&file_content)?;

    // 1. Create VisitorState with the provided context
    let mut state = state::VisitorState::new(crate_namespace, file_path.to_path_buf());
    // Set the correct initial module path for the visitor
    state.current_module_path = logical_module_path.clone();

    // 2. Generate root module ID using the derived logical path context
    let root_module_name = logical_module_path
        .last()
        .cloned()
        .unwrap_or_else(|| "crate".to_string()); // Use last segment as name, fallback to "crate"
    let root_module_parent_path: Vec<String> = logical_module_path
        .iter()
        .take(logical_module_path.len().saturating_sub(1)) // Get parent path segments
        .cloned()
        .collect();

    let root_module_id = NodeId::generate_synthetic(
        crate_namespace,
        &file_path,
        &root_module_parent_path, // Use parent path for ID generation context
        &root_module_name,
        ItemKind::Module, // Pass correct ItemKind
        None,             // Root module has no parent scope ID within the file context
    );

    #[cfg(feature = "verbose_debug")]
    eprintln!(
        "root_module_id: {}\ncreated by:\n\tcrate_namespace: {}
    \tfile_path: {:?}\n\troot_module_parent_path: {:?}\n\troot_module_name: {}\n",
        root_module_id,
        crate_namespace,
        file_path.as_os_str(),
        root_module_parent_path,
        root_module_name
    );

    // 3. Create the root module node using the derived path and name
    state.code_graph.modules.push(ModuleNode {
        id: root_module_id,
        name: root_module_name, // Use derived name
        visibility: crate::parser::types::VisibilityKind::Public, // Assume public for now, Phase 3 resolves
        attributes: Vec::new(),
        docstring: None,
        imports: Vec::new(),
        exports: Vec::new(),
        path: logical_module_path.clone(), // Use derived path
        span: (0, 0), // NOTE: Not generally good practice, we may wish to make this the start/end of the file's bytes.
        tracking_hash: None, // Root module conceptual, no specific content hash
        module_def: ModuleDef::FileBased {
            items: Vec::new(),
            file_path: file_path.clone(),
            file_attrs: extract_file_level_attributes(&file.attrs),
            file_docs: extract_file_level_docstring(&file.attrs),
        },
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
    // visitor.state.code_graph.debug_print_all_visible();

    Ok(ParsedCodeGraph {
        graph: state.code_graph,
        file_path,
        crate_namespace,
    })
}

/// Process multiple files in parallel using rayon (UUID Path) - The Orchestrator
/// Takes DiscoveryOutput and distributes work to analyze_file_phase2.
pub fn analyze_files_parallel(
    discovery_output: &DiscoveryOutput, // Takes output from Phase 1
    _num_workers: usize, // May not be directly used if relying on rayon's default pool size
) -> Vec<Result<ParsedCodeGraph, syn::Error>> {
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
            #[cfg(feature = "verbose_debug")]
            println!(
                // Temporary debug print
                "  Processing crate '{}' with {} files...",
                crate_context.name,
                crate_context.files.len()
            );
            // For each crate, parallelize over its files
            // Assume CrateContext has a `root_dir` field or similar
            // If not, we might need to adjust how src_dir is found
            let crate_root_dir = crate_context.root_path.clone(); // Assuming CrateContext has root_dir
            let src_dir = crate_root_dir.join("src");

            crate_context.files.par_iter().map(move |file_path| {
                // Derive the logical path for this file
                let logical_path = derive_logical_path(&src_dir, file_path);

                // Call the single-file worker function with its specific context + logical path
                analyze_file_phase2(
                    file_path.to_owned(),
                    crate_context.namespace,
                    logical_path, // Pass the derived path
                )
            })
        })
        .collect() // Collect all results (Result<ParsedCodeGraph, Error>) into a Vec
}
