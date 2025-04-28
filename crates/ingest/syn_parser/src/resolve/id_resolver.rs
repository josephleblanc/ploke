use ploke_core::{CanonId, IdInfo, NodeId, ResolvedId};
use uuid::Uuid;

use crate::{
    error::SynParserError,
    parser::{
        graph::{GraphAccess, ParsedCodeGraph},
        nodes::{GraphNode, ModuleNodeId, NodePath},
    },
    resolve::module_tree::ModuleTree,
};

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
    ) -> std::result::Result<ploke_core::CanonId, ploke_core::IdConversionError> {
        CanonId::generate_resolved(
            self.namespace(),
            IdInfo::new(
                &self.graph.file_path,   // file_path,
                node_path.as_segments(), // logical_item_path,
                graph_node.cfgs(),       // cfgs,
                graph_node.kind(),       // item_kind
            ),
        )
    }

    /// Resolves all synthetic IDs in the graph to `CanonId`s.
    ///
    /// This method iterates through the nodes in the `ParsedCodeGraph`, determines
    /// their canonical paths using the `ModuleTree`, and generates the corresponding
    /// `CanonId`.
    ///
    /// # Returns
    /// A `Result` containing a `HashMap` mapping the original synthetic `NodeId`
    /// to its resolved `CanonId`, or an `IdConversionError` if resolution fails
    /// for any node (e.g., path determination error, I/O error).
    ///
    /// TODO: Implement the actual iteration and resolution logic.
    pub fn resolve_all(
        &self,
    ) -> impl Iterator<Item = Result<(NodeId, CanonId), SynParserError>> + '_ {
        // path_index does not contain declarations, so we know all node_ids here are for only
        // `ModuleNode`s that are either inline or file-based
        self.module_tree
            .path_index()
            .iter()
            .filter_map(|(np, mod_id)| {
                let module = self
                    .module_tree
                    .modules()
                    .get(&ModuleNodeId::new(*mod_id))?;
                Some((np, module))
            })
            .filter_map(|(np, module)| module.items().map(|items| (np, module, items)))
            .flat_map(move |(np, _m, items)| {
                items
                    .iter()
                    .map(move |&item_id| self.graph.find_node_unique(item_id).map(|n| (np, n)))
                // self.graph.find_node_unique()
            })
            // At this point, items are Result<(NodePath, &dyn GraphNode), SynParserError>
            .map(|find_result| {
                // find_result is Result<(&NodePath, &dyn GraphNode), SynParserError>
                match find_result {
                    Ok((np, node)) => {
                        // If find succeeded, try to resolve the node.
                        // self.resolve_single_node returns Result<CanonId, IdConversionError>
                        self.resolve_single_node(np, node)
                            .map(|canon_id| (node.id(), canon_id))
                            // Convert IdConversionError to SynParserError if resolve fails
                            .map_err(SynParserError::from)
                    }
                    Err(syn_err) => {
                        // If find_node_unique failed, propagate the SynParserError directly.
                        Err(syn_err)
                    }
                }
            })
        // .map(|result| {
        //     result.map(|(np, module, items)| items.iter().map(|item| (np, module, item)))
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
