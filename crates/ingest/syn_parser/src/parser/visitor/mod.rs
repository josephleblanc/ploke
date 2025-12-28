use colored::Colorize;
use ploke_core::byte_hasher::ByteHasher;
use ploke_core::ItemKind;
use quote::ToTokens;
use std::{collections::HashMap, hash::Hasher};
use syn::visit::Visit;
mod attribute_processing;
mod cfg_evaluator;
mod code_visitor;
mod state;
mod type_processing;

pub use code_visitor::CodeVisitor;
pub use state::VisitorState;

use crate::{
    error::SynParserError,
    parser::{
        nodes::{ModuleNodeInfo, PrimaryNodeId},
        relations::SyntacticRelation,
    },
    utils::{LogStyle, LogStyleDebug, LOG_TARGET_RELS},
};

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
        log::debug!(
            "Warning: Could not strip prefix '{}' from '{}'. Falling back to ['crate'].",
            crate_src_dir.display(),
            file_path.display()
        );
    }

    logical_path
}

use super::ParsedCodeGraph;

use {
    super::nodes::ModuleNode,          // Moved ModuleNode import here
    crate::discovery::DiscoveryOutput, // Import DiscoveryOutput
    ploke_core::NodeId,
    rayon::prelude::*, // Import rayon traits
    uuid::Uuid,
};

/// Analyze a single file for Phase 2 (UUID Path) - The Worker Function
/// Receives context from analyze_files_parallel.
#[cfg(feature = "cfg_eval")]
pub fn analyze_file_phase2(
    file_path: PathBuf,
    crate_namespace: Uuid,            // Context passed from caller
    logical_module_path: Vec<String>, // NEW: The derived logical path for this file
    crate_context: &crate::discovery::CrateContext,
) -> Result<ParsedCodeGraph, syn::Error> {
    // Consider a more specific Phase2Error later

    use super::nodes::ModuleKind;
    use attribute_processing::{
        extract_cfg_strings, // NEW: Import raw string extractor
        extract_file_level_attributes,
        extract_file_level_docstring,
        // Removed parse_and_combine_cfgs_from_attrs import
    };

    let file_content = std::fs::read_to_string(&file_path).map_err(|e| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            format!("Failed to read file {}: {}", file_path.display(), e),
        )
    })?;
    // .expect("This is the primary problem? line 118 of visitor/mod.rs");
    // TODO: Add real error handling here.
    // let msg = format!("This is the primary problem? line 121 of visitor/mod.rs parsing: {}", file_path.display());
    let file = syn::parse_file(&file_content)?;
    // .inspect_err(|e| tracing::trace!("Getting closer to the source: {e}"))?;
    // .expect(&msg);

    // 1. Create VisitorState with the provided context
    let mut state =
        state::VisitorState::new(crate_namespace, file_path.to_path_buf(), crate_context);
    // Set the correct initial module path for the visitor
    state.current_module_path = logical_module_path.clone();

    // Extract raw file-level CFG strings (#![cfg(...)])
    let file_cfgs = extract_cfg_strings(&file.attrs);
    // Set the initial scope CFGs for the visitor state
    state.current_scope_cfgs = file_cfgs.clone();
    // Hash the file-level CFG strings for the root module ID
    let root_cfg_bytes = calculate_cfg_hash_bytes(&file_cfgs);

    // 2. Generate root module ID using the derived logical path context AND CFG context
    let root_module_name = logical_module_path
        .last()
        .cloned()
        .unwrap_or_else(|| "crate".to_string()); // Use last segment as name, fallback to "crate"
    let root_module_parent_path: Vec<String> = logical_module_path
        .iter()
        .take(logical_module_path.len().saturating_sub(1)) // Get parent path segments
        .cloned()
        .collect();

    let root_module_node_id = NodeId::generate_synthetic(
        crate_namespace,
        &file_path,
        &root_module_parent_path, // Use parent path for ID generation context
        &root_module_name,
        ItemKind::Module,          // Pass correct ItemKind
        None,                      // Root module has no parent scope ID within the file context
        root_cfg_bytes.as_deref(), // Pass hashed file-level CFG bytes
    );
    // #[cfg(test)]
    debug_file_module_id_gen(
        crate_namespace,
        &file_path,
        &root_module_parent_path,
        &root_module_name,
        ItemKind::Module,
        None,
        root_cfg_bytes.as_deref(),
    );

    // 3. Create the root module node using the derived path and name
    // Determine visibility: Public only for crate root (main.rs/lib.rs), Inherited otherwise
    let root_visibility = if logical_module_path == ["crate"] {
        crate::parser::types::VisibilityKind::Public
    } else {
        crate::parser::types::VisibilityKind::Inherited
    };

    let root_module_info = ModuleNodeInfo {
        id: root_module_node_id,
        name: root_module_name,      // Use derived name
        visibility: root_visibility, // Use determined visibility
        attributes: Vec::new(),
        docstring: None,
        imports: Vec::new(),
        exports: Vec::new(),
        path: logical_module_path.clone(), // Use derived path
        span: (0, file_content.len()),
        tracking_hash: Some(state.generate_tracking_hash(&file.to_token_stream())),
        module_def: ModuleKind::FileBased {
            items: Vec::new(),
            file_path: file_path.clone(),
            file_attrs: extract_file_level_attributes(&file.attrs), // Non-CFG attributes
            file_docs: extract_file_level_docstring(&file.attrs),
            // cfgs removed from here, belongs on ModuleNode
        },
        cfgs: file_cfgs, // Store raw file-level CFGs on the ModuleNode
    };

    state
        .code_graph
        .modules
        .push(ModuleNode::new(root_module_info));

    let root_module_pid: PrimaryNodeId = state.code_graph.modules[0].id.into();

    // Default parent scope for top-level items visited next.
    state.current_primary_defn_scope.push(root_module_pid);

    // 4. Create and run the visitor
    let mut visitor = code_visitor::CodeVisitor::new(&mut state);
    visitor.visit_file(&file);

    #[cfg(feature = "temp_target")]
    debug_relationships(&visitor);

    log::trace!(target: "parse_target", "parsing target: {}
validate_unique_rels = {}", file_path.display(), &visitor.validate_unique_rels());
    #[cfg(feature = "validate")]
    assert!(&visitor.validate_unique_rels());

    // let module_ids: Vec<NodeId> = state.code_graph.modules.iter().map(|m| m.id).collect();
    // for module_id in module_ids {
    //     if module_id != root_module_id {
    //         state.code_graph.relations.push(Relation {
    //             source: root_module_id,
    //             target: module_id,
    //             kind: crate::parser::relations::RelationKind::Contains,
    //         });
    //     }
    // }

    Ok(ParsedCodeGraph::new(
        file_path,
        crate_namespace,
        state.code_graph,
    ))
}

/// Analyze a single file for Phase 2 (UUID Path) - The Worker Function
/// Receives context from analyze_files_parallel.
#[cfg(not(feature = "cfg_eval"))]
pub fn analyze_file_phase2(
    file_path: PathBuf,
    crate_namespace: Uuid,            // Context passed from caller
    logical_module_path: Vec<String>, // NEW: The derived logical path for this file
) -> Result<ParsedCodeGraph, syn::Error> {
    // Consider a more specific Phase2Error later

    use super::nodes::ModuleKind;
    use attribute_processing::{
        extract_cfg_strings, // NEW: Import raw string extractor
        extract_file_level_attributes,
        extract_file_level_docstring,
        // Removed parse_and_combine_cfgs_from_attrs import
    };

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

    // Extract raw file-level CFG strings (#![cfg(...)])
    let file_cfgs = extract_cfg_strings(&file.attrs);
    // Set the initial scope CFGs for the visitor state
    state.current_scope_cfgs = file_cfgs.clone();
    // Hash the file-level CFG strings for the root module ID
    let root_cfg_bytes = calculate_cfg_hash_bytes(&file_cfgs);

    // 2. Generate root module ID using the derived logical path context AND CFG context
    let root_module_name = logical_module_path
        .last()
        .cloned()
        .unwrap_or_else(|| "crate".to_string()); // Use last segment as name, fallback to "crate"
    let root_module_parent_path: Vec<String> = logical_module_path
        .iter()
        .take(logical_module_path.len().saturating_sub(1)) // Get parent path segments
        .cloned()
        .collect();

    let root_module_node_id = NodeId::generate_synthetic(
        crate_namespace,
        &file_path,
        &root_module_parent_path, // Use parent path for ID generation context
        &root_module_name,
        ItemKind::Module,          // Pass correct ItemKind
        None,                      // Root module has no parent scope ID within the file context
        root_cfg_bytes.as_deref(), // Pass hashed file-level CFG bytes
    );
    // #[cfg(test)]
    debug_file_module_id_gen(
        crate_namespace,
        &file_path,
        &root_module_parent_path,
        &root_module_name,
        ItemKind::Module,
        None,
        root_cfg_bytes.as_deref(),
    );

    // 3. Create the root module node using the derived path and name
    // Determine visibility: Public only for crate root (main.rs/lib.rs), Inherited otherwise
    let root_visibility = if logical_module_path == ["crate"] {
        crate::parser::types::VisibilityKind::Public
    } else {
        crate::parser::types::VisibilityKind::Inherited
    };

    let root_module_info = ModuleNodeInfo {
        id: root_module_node_id,
        name: root_module_name,      // Use derived name
        visibility: root_visibility, // Use determined visibility
        attributes: Vec::new(),
        docstring: None,
        imports: Vec::new(),
        exports: Vec::new(),
        path: logical_module_path.clone(), // Use derived path
        span: (0, file_content.len()),
        tracking_hash: Some(state.generate_tracking_hash(&file.to_token_stream())),
        module_def: ModuleKind::FileBased {
            items: Vec::new(),
            file_path: file_path.clone(),
            file_attrs: extract_file_level_attributes(&file.attrs), // Non-CFG attributes
            file_docs: extract_file_level_docstring(&file.attrs),
            // cfgs removed from here, belongs on ModuleNode
        },
        cfgs: file_cfgs, // Store raw file-level CFGs on the ModuleNode
    };

    state
        .code_graph
        .modules
        .push(ModuleNode::new(root_module_info));

    let root_module_pid: PrimaryNodeId = state.code_graph.modules[0].id.into();

    // Default parent scope for top-level items visited next.
    state.current_primary_defn_scope.push(root_module_pid);

    // 4. Create and run the visitor
    let mut visitor = code_visitor::CodeVisitor::new(&mut state);
    visitor.visit_file(&file);

    #[cfg(feature = "temp_target")]
    debug_relationships(&visitor);

    #[cfg(feature = "validate")]
    assert!(&visitor.validate_unique_rels());

    // let module_ids: Vec<NodeId> = state.code_graph.modules.iter().map(|m| m.id).collect();
    // for module_id in module_ids {
    //     if module_id != root_module_id {
    //         state.code_graph.relations.push(Relation {
    //             source: root_module_id,
    //             target: module_id,
    //             kind: crate::parser::relations::RelationKind::Contains,
    //         });
    //     }
    // }

    Ok(ParsedCodeGraph::new(
        file_path,
        crate_namespace,
        state.code_graph,
    ))
}

// TODO: Figure out how to get the test cfg working correctly
// #[cfg(test)]
fn debug_file_module_id_gen(
    crate_namespace: uuid::Uuid,
    file_path: &std::path::Path,
    relative_path: &[String],
    item_name: &str,
    item_kind: ItemKind, // Use ItemKind from this crate
    parent_scope_id: Option<NodeId>,
    cfg_bytes: Option<&[u8]>,
) {
    use log::debug;

    use crate::utils::logging::LOG_TEST_ID_REGEN;

    if let Ok(debug_target_item) = std::env::var("ID_REGEN_TARGET") {
        if log::log_enabled!(target: LOG_TEST_ID_REGEN, log::Level::Debug)
            && debug_target_item == item_name
        // allow for filtering by command env variable
        {
            // Check if specific log is enabled
            debug!(target: LOG_TEST_ID_REGEN, "{:=^60}", " FileBased Id Generation ".log_header());
            debug!(target: LOG_TEST_ID_REGEN,
                "  Inputs for '{}' ({}):\n    crate_namespace: {}\n    file_path: {}\n    relative_path: {}\n    item_name: {}\n    item_kind: {}\n    parent_scope_id: {}\n    cfg_bytes: {}\n",
                item_name.log_name(), // item name being processed by visitor
                item_kind.log_comment_debug(),
                crate_namespace,
                file_path.as_os_str().log_comment_debug(),
                relative_path.log_path_debug(), // This is the 'relative_path' for the item's ID context
                item_name.log_name(),
                item_kind.log_comment_debug(),
                parent_scope_id.log_id_debug(), // The actual parent_scope_id used by visitor
                cfg_bytes.log_comment_debug() // The actual cfg_bytes used by visitor
            );
        }
    }
}

#[allow(dead_code, reason = "Useful for debugging")]
fn debug_relationships(visitor: &CodeVisitor<'_>) {
    let unique_rels = visitor.relations().iter().fold(Vec::new(), |mut acc, r| {
        if !acc.contains(r) {
            acc.push(*r)
        }
        acc
    });
    let has_duplicate = unique_rels.len() == visitor.relations().len();
    log::debug!(target: "temp",
        "{} {} {}: {} | {}: {} | {}: {}",
        "Relations are unique?".log_header(),
        if has_duplicate {
            "Yes!".log_spring_green().bold()
        } else {
            "NOOOO".log_error()
        },
        "Unique".log_step(),
        unique_rels.len().to_string().log_magenta_debug(),
        "Total".log_step(),
        visitor.relations().len().to_string().log_magenta_debug(),
        "Difference".log_step(),
        (visitor.relations().len() - unique_rels.len() ).to_string().log_magenta_debug(),
    );
    // Update HashMap key type to SyntacticRelation
    let rel_map: HashMap<SyntacticRelation, usize> =
        visitor
            .relations()
            .iter()
            .copied()
            .fold(HashMap::new(), |mut hmap, r| {
                match hmap.entry(r) {
                    std::collections::hash_map::Entry::Occupied(mut occupied_entry) => {
                        let existing_count = occupied_entry.get();
                        occupied_entry.insert(existing_count + 1);
                    }
                    std::collections::hash_map::Entry::Vacant(vacant_entry) => {
                        vacant_entry.insert(1);
                    }
                };
                hmap
            });
    for (rel, count) in rel_map {
        if count > 1 {
            // Use the helper methods to get base NodeIds for logging
            log::debug!(target: LOG_TARGET_RELS,
                "{} | {}: {} | {}", // Log the full relation variant for kind info
                "Duplicate!".log_header(),
                "Count".log_step(),
                count.to_string().log_error(),
                rel, // Log the whole enum variant
            );
        }
    }
}

/// Process multiple files in parallel using rayon (UUID Path) - The Orchestrator
/// Takes DiscoveryOutput and distributes work to analyze_file_phase2.
pub fn analyze_files_parallel(
    discovery_output: &DiscoveryOutput, // Takes output from Phase 1
    _num_workers: usize, // May not be directly used if relying on rayon's default pool size
) -> Vec<Result<ParsedCodeGraph, SynParserError>> {
    // Adjust error type if needed

    log::debug!(target: "crate_context",
        // Temporary debug print
        "Starting Phase 2 Parallel Parse for {} crates...",
        discovery_output.crate_contexts.len()
    );

    let parsed_results: Vec<Result<ParsedCodeGraph, SynParserError>> = discovery_output
        .crate_contexts
        .values() // Iterate over CrateContext values
        .par_bridge() // Bridge into a parallel iterator (efficient for HashMap values)
        .flat_map(|crate_context| {
            // Process each crate in parallel
            // For each crate, parallelize over its files
            // Assume CrateContext has a `root_dir` field or similar
            // If not, we might need to adjust how src_dir is found
            let crate_root_dir = crate_context.root_path.clone(); // Assuming CrateContext has root_dir
            let src_dir = crate_root_dir.join("src");

            crate_context.files.par_iter().map(move |file_path| {
                // Derive the logical path for this file
                let logical_path = derive_logical_path(&src_dir, file_path);

                // Call the single-file worker function with its specific context + logical path
                #[cfg(not(feature = "cfg_eval"))]
                let parsed = analyze_file_phase2(
                    file_path.to_owned(),
                    crate_context.namespace,
                    logical_path, // Pass the derived path

                )
                .map(|pg| set_root_context(crate_context, pg)) // Give root module's graph the crate context  
                .map_err(|e| {
                    SynParserError::Syn(format!("{} (file: {})", e, file_path.display()))
                })
                .inspect(|pg| { log::debug!(target: "crate_context", "{}", info_crate_context(&src_dir, pg)) });

                log::debug!(target: "debug_dup", "file path in par_iter: {}", file_path.display());
                #[cfg(feature = "cfg_eval")]
                let parsed = analyze_file_phase2(
                    file_path.to_owned(),
                    crate_context.namespace,
                    logical_path, // Pass the derived path
                    crate_context
                )
                .map(|pg| set_root_context(crate_context, pg)) // Give root module's graph the crate context  
                .map_err(|e| {
                    tracing::trace!("Error found: {} (file: {})", e, file_path.display());
                    SynParserError::Syn(format!("{} (file: {})", e, file_path.display()))
                })
                .inspect(|pg| { log::debug!(target: "crate_context", "{}", info_crate_context(&src_dir, pg)) });
                parsed
            })
        })
        .collect(); // Collect all results (Result<ParsedCodeGraph, Error>) into a Vec

    let crate_count = parsed_results
        .iter()
        .filter_map(|pr| pr.as_ref().ok())
        .filter_map(|pr| pr.crate_context.as_ref())
        .inspect(|pr| {
            log::trace!(target: "crate_context", "root graph contains files: {:#?}", pr);
        })
        .count();
    if crate_count != 1 {
        log::trace!(target: "crate_context", "total crate count of graphs with crate_context: {}", crate_count);
    }
    // NOTE:2025-12-26
    // Commenting out the below so this function will not panic on finding a crate_context, as in
    // the case of an error in the syntax of the `lib.rs` for the target crate.
    // .find(|pr| pr.crate_context.is_some());
    // .expect("At least one crate must carry the context");
    // log::trace!(target: "crate_context", "root graph contains files: {:#?}", root_graph.crate_context);

    parsed_results
}

fn set_root_context(
    crate_context: &crate::discovery::CrateContext,
    mut pg: ParsedCodeGraph,
) -> ParsedCodeGraph {
    if pg
        .file_path
        .file_name()
        .is_some_and(|f| f == "lib.rs" || f == "main.rs")
    {
        pg.crate_context = Some(crate_context.clone());
    }
    pg
}

fn info_crate_context(src_dir: &PathBuf, pg: &ParsedCodeGraph) -> String {
    format!(
        "parsed_graph file_path: {}, crate_context: {:#?}",
        pg.file_path.strip_prefix(src_dir).as_ref().log_path_debug(),
        pg.crate_context
    )
}

/// Calculates a hash for a list of raw CFG strings.
/// Sorts the strings before joining and hashing to ensure deterministic output.
/// Returns None if the input slice is empty.
pub fn calculate_cfg_hash_bytes(cfgs: &[String]) -> Option<Vec<u8>> {
    if cfgs.is_empty() {
        return None;
    }

    // Clone and sort for determinism
    let mut sorted_cfgs = cfgs.to_vec();
    sorted_cfgs.sort_unstable();

    // Join with a separator (important if a cfg string could contain the separator)
    let joined_cfgs = sorted_cfgs.join("::CFG::");

    // Hash the joined string
    let mut hasher = ByteHasher::default();
    hasher.write(joined_cfgs.as_bytes());
    Some(hasher.finish_bytes())
}
