use std::path::Path;

use log::{debug, trace}; // Import debug macro
use ploke_core::{CanonId, IdInfo, NodeId, ResolvedId};
use uuid::Uuid;

use crate::{
    error::SynParserError,
    parser::{
        graph::{GraphAccess, ParsedCodeGraph},
        nodes::{GraphNode, ModuleNodeId, NodePath},
    }, // Import GraphNode trait for logging
    resolve::module_tree::{ModuleTree, ModuleTreeError},
    utils::{LogStyle, LogStyleDebug}, // Import logging traits
};

// Define a logging target for this file
const LOG_TARGET: &str = "id_resolver";

/// Responsible for resolving synthetic `NodeId`s and `TypeId`s generated during
/// Phase 2 parsing into stable, canonical `CanonId`s based on the fully resolved
/// module structure and paths determined in Phase 3.
///
/// It takes references to the completed `ModuleTree` and the merged `ParsedCodeGraph`
/// containing all nodes from the crate.
pub struct CanonIdResolver<'a, 'b> {
    /// Project namespace of the defining crate, used as namespace for Id generation using v5 hash.
    namespace: Uuid,
    /// Reference to the fully resolved module tree.
    module_tree: &'a ModuleTree,
    /// Reference to the merged code graph containing all parsed nodes.
    graph: &'b ParsedCodeGraph,
}

impl<'a, 'b> CanonIdResolver<'a, 'b> {
    /// Creates a new `CanonIdResolver`.
    ///
    /// # Arguments
    /// * `namespace` - The UUID namespace of the crate being processed.
    /// * `module_tree` - A reference to the constructed `ModuleTree`.
    /// * `graph` - A reference to the `ParsedCodeGraph`.
    pub fn new(namespace: Uuid, module_tree: &'a ModuleTree, graph: &'b ParsedCodeGraph) -> Self {
        Self {
            namespace,
            module_tree,
            graph,
        }
    }

    /// Returns the crate namespace used by this resolver.
    pub fn namespace(&self) -> Uuid {
        self.namespace
    }

    fn resolve_single_node(
        &self,
        node_path: &NodePath,
        graph_node: &dyn GraphNode,
        file_path: &Path,
    ) -> std::result::Result<ploke_core::CanonId, ploke_core::IdConversionError> {
        // Log the attempt to resolve this specific node
        debug!(target: LOG_TARGET, "  {} Resolving node: {} ({}) at path: {}",
            "->".log_comment(),
            graph_node.name().log_name(),
            graph_node.id().to_string().log_id(),
            node_path.to_string().log_path()
        );

        //
        let result = CanonId::generate_resolved(
            self.namespace(),
            IdInfo::new(
                file_path,
                node_path.as_segments(), // logical_item_path,
                graph_node.cfgs(),       // cfgs,
                graph_node.kind(),       // item_kind
            ),
        ); // Added semicolon
        result // Explicitly return the result
    }

    /// Resolves all synthetic IDs in the graph to `CanonId`s.
    ///
    /// This method iterates through the nodes in the `ParsedCodeGraph`, determines
    /// their canonical paths using the `ModuleTree`, and generates the corresponding
    /// `CanonId`.
    ///
    /// # Returns
    /// A `Result` containing a `HashMap` mapping the original synthetic `NodeId`
    /// to its resolved `CanonId`, or an `SynParserError` if resolution fails
    /// for any node (e.g., path determination error, node not found, I/O error).
    pub fn resolve_all(
        &self,
    ) -> impl Iterator<Item = Result<(NodeId, CanonId), SynParserError>> + '_ {
        trace!(target: LOG_TARGET, "{} Starting CanonId resolution...", "Begin".log_header());
        let path_index_len = self.module_tree.path_index().len();
        trace!(target: LOG_TARGET, "  Processing {} modules from path_index.", path_index_len.to_string().log_id());

        // path_index does not contain declarations, so we know all node_ids here are for only
        // `ModuleNode`s that are either inline or file-based
        self.module_tree
            .path_index()
            .iter() // Iterate over (NodePath, NodeId) from path_index
            .filter_map(|(np, mod_id)| {

                trace!(target: LOG_TARGET, "path_index filter_map: <name unknown> ({}) | NodePath: {}",
                    mod_id.to_string().log_id(),
                    np.to_string().log_path()
                );
                // Get the ModuleNode for the ID
                self.module_tree
                    .modules()
                    .get(&ModuleNodeId::new(*mod_id))
                    .inspect(|opt| {
                trace!(target: LOG_TARGET, "  getting: {} ({}) | Option is_some(), name: {:#?}",
                    mod_id.to_string().log_id(),
                    np.to_string().log_path(),
                    opt.name(),
                );
                    })
                    .map(|module| (np, module)).inspect(| (np, module)| {
                trace!(target: LOG_TARGET, "Filtering empty modules: {} ({}) | NodePath: {}",
                    module.name().log_name(),
                    module.id().to_string().log_id(),
                    np.to_string().log_path()
                );
                    }
                    ) // Keep NodePath and ModuleNode
            })
            .flat_map(move |(np, module)| {
                // Log which module we are processing items for
                trace!(target: LOG_TARGET, "Processing items in module: {} ({})",
                    module.name().log_name(),
                    module.id().to_string().log_id()
                );
                // Get items contained in the module, or an empty slice if none
                let items = module.items().unwrap_or(&[]);
                // Create an iterator that pairs the NodePath with each item ID
                items.iter().map(move |&item_id| (np, item_id))
            })
            .map(move |(np, item_id)| {
                // Find the actual GraphNode for the item ID
                trace!(target: LOG_TARGET, "  Attempting find_node_unique for item_id: {}", item_id.to_string().log_id());
                // Chain the fallible operations using and_then and map_err
                self.graph.find_node_unique(item_id) // -> Result<&dyn GraphNode, SynParserError>
                    .and_then(|node| { // If find_node_unique is Ok, proceed
                        trace!(target: LOG_TARGET, "    Found node: {}", node.name().log_name());
                        self.module_tree.find_defining_file_path_ref_seq(item_id) // -> Result<&Path, ModuleTreeError>
                            .map_err(SynParserError::from) // Convert ModuleTreeError to SynParserError if Err
                            .map(|fp| { // If find_defining_file_path_ref is Ok
                                trace!(target: LOG_TARGET, "    Found defining path: {}", fp.display().to_string().log_path());
                                // Combine np (cloned), node, and fp into the final Ok tuple
                                (np, node, fp)
                            })
                    }) // Result of the chain is Result<(NodePath, &dyn GraphNode, &Path),  SynParserError>
            })
            // At this point, items are Result<(&NodePath, &dyn GraphNode), SynParserError>
            .map(|find_result| {
                // find_result is Result<(&NodePath, &dyn GraphNode), SynParserError>
                match find_result {
                    Ok((np, node, fp)) => {
                        // If find succeeded, try to resolve the node.
                        let resolve_result = self.resolve_single_node(np, node, fp);
                        match resolve_result {
                            Ok(canon_id) => {
                                trace!(target: LOG_TARGET, "    {} Resolved {} -> {}",
                                    "✓".log_green(),
                                    node.id().to_string().log_id(),
                                    canon_id.to_string().log_id_debug() // Use debug log style for CanonId
                                );
                                Ok((node.id(), canon_id))
                            }
                            Err(id_conv_err) => {
                                // Log the IdConversionError before converting
                                log::error!(target: LOG_TARGET, "    {} Failed IdConversion for {}: {}",
                                    "✗".log_error(),
                                    node.id().to_string().log_id(),
                                    id_conv_err.to_string().log_error()
                                );
                                // Convert IdConversionError to SynParserError
                                Err(SynParserError::from(id_conv_err))
                            }
                        }
                    }
                    Err(syn_err) => {
                        // Log the SynParserError from find_node_unique
                        log::error!(target: LOG_TARGET, "  {} Failed find_node_unique: {}",
                            "✗".log_error(),
                            syn_err.to_string().log_error()
                        );
                        // Propagate the SynParserError directly.
                        Err(syn_err)
                    }
                }
            })
        // .map(|result| { // Original commented-out code
        //     result.map(|(np, module, items)| items.iter().map(|item| (np, module, item))) // Original commented-out code
        // });
        // self.graph
        //     .functions()
        //     .iter()
        //     .map(|f| (f.name(), f.id()))
        //     .chain(self.graph.impls().iter().map(|imp| (imp.name(), imp.id())))
        //     .map(|(name, id)| {
        //         let result = self
        //             .module_tree
        //             .get_containing_mod_checked(&GraphId::Node(id), RelationKind::Contains);
        //         result.map(|tr| (name, id, tr))
        //     }).map(| result | result.map(|(name, id, tr)| {
        //         let containing_module = tr.relation().source;
        //         let fileself.module_tree.get_module_checked(containing_module)
        //     }
        //
        //     ) );
        //
        // todo!();

        // .graph.functions().iter().map(|f| f);

        // IdInfo::new(
        //     todo!(), // file_path,
        //     todo!(), // logical_item_path,
        //     todo!(), // cfgs,
        //     todo!(), // item_kind

        // --- Placeholder Logic ---
        // The actual implementation will involve chaining iterators over different node types.
        // Example structure:
        // self.graph.functions().iter().map(|func_node| {
        //     let synthetic_id = func_node.id;
        //     self.resolve_single_node(synthetic_id, func_node) // Helper returns Result<(NodeId, CanonId), Error>
        // })
        // .chain(self.graph.defined_types().iter().map(|type_def_node| {
        //     let synthetic_id = type_def_node.id();
        //     self.resolve_single_node(synthetic_id, type_def_node)
        // }))
        // .chain(...) // for other node types (modules, impls, traits, values, macros, imports)

        // For now, return an empty iterator that satisfies the type signature.
        // std::iter::empty()

        // --- Original Placeholder Logic (for reference during implementation) ---
        // let mut resolved_ids = HashMap::new();
        // Iterate through self.graph.functions(), self.graph.defined_types(), etc.
        // For each node:
        // 1. Get its synthetic NodeId.
        // 2. Determine its canonical path using self.module_tree.
        //    - This involves finding the containing module and walking up the tree.
        //    - Need helper functions in ModuleTree or here to get the canonical path Vec<String>.
        // 3. Get other necessary info (file_path, item_kind, cfgs).
        // 4. Create IdInfo struct.
        // 5. Call CanonId::generate_resolved(self.namespace, id_info).
        // 6. Insert into resolved_ids map.

        // Example (Conceptual - Needs Real Implementation):
        // for func_node in self.graph.functions() {
        //     let synthetic_id = func_node.id; // Assuming this is the synthetic NodeId
        //     let canonical_path = self.determine_canonical_path(synthetic_id)?; // Placeholder
        //     let id_info = IdInfo { /* ... populate ... */ };
        //     let canon_id = CanonId::generate_resolved(self.namespace, id_info)?;
        //     resolved_ids.insert(synthetic_id, canon_id);
        // }
        // ... repeat for other node types ...
        // --- End Placeholder ---
    }

    // Placeholder for a helper function to resolve a single node
    // fn resolve_single_node(
    //     &self,
    //     synthetic_id: NodeId,
    //     node: &dyn GraphNode, // Need access to the node itself
    // ) -> Result<(NodeId, CanonId), IdConversionError> {
    //     let canonical_path = self.determine_canonical_path(synthetic_id)?; // Placeholder
    //     let id_info = IdInfo { /* ... populate ... */ };
    //     let canon_id = CanonId::generate_resolved(self.namespace, id_info)?;
    //     Ok((synthetic_id, canon_id))
    // }

    // Placeholder for a helper function to determine canonical path (might live in ModuleTree or here)
    // fn determine_canonical_path(&self, node_id: NodeId) -> Result<Vec<String>, IdConversionError> {
    //     // ... logic to find containing module and build path ...
    //     Ok(vec!["crate".to_string(), "some_mod".to_string(), "item_name".to_string()]) // Dummy path
    // }
}
