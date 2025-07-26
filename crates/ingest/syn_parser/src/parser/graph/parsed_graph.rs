use crate::{
    discovery::DependencyMap as _,
    resolve::{ModuleTreeError, TreeRelation, UnlinkedModuleInfo},
};
use anyhow::Result;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use crate::utils::logging::LOG_TARGET_MOD_TREE_BUILD;

use super::*;
use thiserror::Error; // Add thiserror

#[derive(Error, Debug, Clone, PartialEq)]
pub enum ParsedGraphError {
    #[error("Crate context is missing, cannot determine root path.")]
    MissingCrateContext,
    #[error("Internal error: Expected exactly one crate context, found multiple.")]
    MultipleCrateContexts, // Should not happen with Option, but good practice
    #[error("Root file not found at file path '{0}' in graph.")]
    RootFileNotFound(PathBuf),
    #[error("Duplicate root module file path '{0}' found in graph.")]
    DuplicateRootFile(PathBuf),
}

#[derive(Debug, Deserialize, Clone)]
pub struct ParsedCodeGraph {
    /// The absolute path of the file that was parsed.
    pub file_path: PathBuf,
    /// The UUID namespace of the crate this file belongs to.
    pub crate_namespace: Uuid,
    /// The resulting code graph from parsing the file.
    pub graph: CodeGraph,
    /// Crate Context for target crate, such as name, dependencies, etc.
    pub crate_context: Option<CrateContext>,
}

impl ParsedCodeGraph {
    pub fn new(file_path: PathBuf, crate_namespace: Uuid, graph: CodeGraph) -> Self {
        Self {
            file_path,
            crate_namespace,
            graph,
            crate_context: None,
        }
    }

    /// Returns a set of dependency names declared in the crate's Cargo.toml.
    ///
    /// Returns an empty set if the crate context (including dependency information)
    /// is not available.
    pub fn dependency_names(&self) -> HashSet<String> {
        self.crate_context
            .as_ref()
            .map(|ctx| {
                // Using the DependencyMap trait's names() method:
                ctx.dependencies()
                    .names()
                    .cloned()
                    .collect::<HashSet<String>>()
                // Alternatively, if accessing the inner map directly:
                // ctx.dependencies.0.keys().cloned().collect::<HashSet<String>>()
            })
            .unwrap_or_default() // Return empty HashSet if crate_context is None
    }

    // TODO: Turn this test back on once we complete the migration to using typed ids.
    /// Returns an iterator over the dependency names declared in the crate's Cargo.toml.
    ///
    /// Returns an empty iterator if the crate context (including dependency information)
    /// is not available. This avoids cloning the names or collecting into a new structure.
    ///
    /// # Example
    /// ```ignore
    /// # use syn_parser::parser::ParsedCodeGraph; // Adjust path
    /// # use std::collections::HashMap;
    /// # let graph: ParsedCodeGraph = /* ... initialize ... */;
    /// for dep_name in graph.iter_dependency_names() {
    ///     println!("Dependency: {}", dep_name);
    /// }
    /// ```
    pub fn iter_dependency_names(&self) -> impl Iterator<Item = &str> + '_ {
        self.crate_context
            .as_ref()
            .map(|ctx| ctx.dependencies().names().map(|s| s.as_str())) // Map &String -> &str
            .into_iter() // Convert Option<impl Iterator> -> impl Iterator<Item = impl Iterator>
            .flatten() // Flatten the outer iterator
    }

    pub fn merge_new(mut graphs: Vec<Self>) -> Result<Self, SynParserError> {
        for graph in graphs.iter() {
            log::debug!(target: "buggy_c", "Buggy First Context: {:#?}", graph.crate_context);
        }
        let mut new_graph = graphs.pop().ok_or(SynParserError::MergeRequiresInput)?;

        // Preserve crate context from any graph
        let mut found_context = new_graph.crate_context.take();
        log::trace!(target: "buggy", "First Context: {:#?}", new_graph.crate_context);
        for mut graph in graphs {
            if found_context.is_none() {
                log::trace!(target: "buggy", "Merging Context: {:#?}", graph.crate_context);
                found_context = graph.crate_context.take();
            }
            new_graph.append_all(graph)?;
        }

        log::trace!(target: "buggy", "Penult Context: {:#?}", new_graph.crate_context);
        new_graph.crate_context = found_context;
        log::trace!(target: "buggy", "Last Context: {:#?}", new_graph.crate_context);

        #[cfg(feature = "validate")]
        {
            ParsedCodeGraph::debug_relationships(&new_graph);
            log::debug!(target: "validate", 
                "{} <- {}",
                "Validating".log_step(),
                new_graph.root_file().unwrap().display(),
            );
            assert!(new_graph.validate_unique_rels());
        }

        Ok(new_graph)
    }

    pub fn append_all(&mut self, mut other: Self) -> Result<(), SynParserError> {
        self.graph.functions.append(&mut other.graph.functions);
        self.graph
            .defined_types
            .append(&mut other.graph.defined_types);
        self.graph.type_graph.append(&mut other.graph.type_graph);
        self.graph.impls.append(&mut other.graph.impls);
        self.graph.traits.append(&mut other.graph.traits);
        self.graph.relations.append(&mut other.graph.relations);
        self.graph.modules.append(&mut other.graph.modules);
        self.graph.consts.append(&mut other.graph.consts); // Use consts
        self.graph.statics.append(&mut other.graph.statics); // Use statics
                                                             // Removed values append
        self.graph.macros.append(&mut other.graph.macros);
        self.graph
            .use_statements
            .append(&mut other.graph.use_statements);

        #[cfg(feature = "validate")]
        {
            ParsedCodeGraph::debug_relationships(self);
            log::debug!(target: "validate", 
                "{} <- {}",
                "Validating".log_step(),
                self.file_path.as_os_str().to_string_lossy()
            );
            assert!(self.validate_unique_rels());
        }

        Ok(())
    }
    //  We already have the following `Relation`s from parsing that will be useful here:
    //
    // ModuleNode definition---Contains--------------> all primary nodes (NodeId)
    // ModuleNode -------------ModuleImports---------> ImportNode (NodeId)
    // NOTE: all `use` and `pub use` included in ModuleImports, not distinguished by relation
    //
    // The following are necessary to define during module tree construction:
    // (Must be constructed in this order)
    //
    // ModuleNode Delc---------ResolvesToDefinition--> ModuleNode definition
    // ModuleNode decl --------CustomPath------------> module defn for `#[path]` attribute
    // ImportNode--------------ReExport--------------> NodeId of reexported item
    //  Some Notes:
    //  ModuleImports - Note: Some duplication of concerns here, ModuleNode also has field for
    //  `imports` with all the nodes it imports - not just the ids, the full node. I think we were
    //  experimenting with trying to use nested data structures insted of parsing relations.
    //      - Note: Includes both `pub use` and `use` reexports/imports
    //
    //  - The NodeId of the ReExported item might be another re-export.
    // We need a new Relation to represent that connection, but it will be in a different set of
    // logical relations, whereas all of these relations are meant to be syntactically accurate.
    // Changed back to &self as graph is immutable again.
    pub fn build_module_tree(&self) -> Result<ModuleTree, SynParserError> {
        #[cfg(feature = "validate")]
        assert!(self.validate_unique_rels());
        let root_module = self.get_root_module_checked()?;
        let mut tree = ModuleTree::new_from_root(root_module)?;
        // 1: Register all modules with their containment info
        for module in self.modules() {
            log_build_tree_processing_module(module);
            // Populates:
            //  - imports/reexports.
            //  - module declaration index
            //  - path_index for reverse lookup
            //  - checks for duplicate paths/ids, causes early return on error.
            tree.add_module(module.clone())?;
        }

        // 2: Copies all relations, stores them as TreeRelation for type safety
        //      - Notably, includes `Contains` relations between parent definition module and all
        //      child elements, e.g. other module declarations. Includes file--contains-->items.
        //      - Does not include inter-file links, due to parallel parsing with no cross-channel
        //      communication.
        //      TODO: Add validation step for relations before adding them.
        tree.extend_relations(self.relations().iter().copied().map_into::<TreeRelation>());

        // 3: Build syntactic links
        //      - Creates `Relation::ResolvesToDefinition` link from
        //          module declaration --ResolvesToDefinition--> file-based module
        //      - Does not process `#[path = "..."]` attributes (see 4 below)
        if let Err(module_tree_error) = tree.link_mods_syntactic(self.modules()) {
            match module_tree_error {
                // Warn on this specific error, but it is safe to continue.
                // Indicates file-level module is not linked to the module tree through a parent.
                // The unlinked file-level module will either be processed by `#[path]` processing
                // below, or we return an error that the graph is inconsistend due to orphaned
                // module definitions.
                ModuleTreeError::FoundUnlinkedModules(unlinked_infos) => {
                    self.handle_unlinked_modules(unlinked_infos);
                }
                // All other erros fatal, meaning abort resolution but do not panic.
                _ => return Err(SynParserError::from(module_tree_error)),
            }
        }

        // 4: Process `#[path]` attributes, form `CustomPath` links
        //  - module declaration (with `#[path]`) --CustomPath--> file-based module
        //  - must run in this order:
        //      - resolve_pending_path_attrs
        //      - process_path_attributes
        //  - NOTE: consider moving these into a single method to remove the possibility of running
        //      them in the incorrect order.
        tree.resolve_pending_path_attrs()?;
        tree.process_path_attributes()?;

        // 5: Update tree.path_index using `CustomPath` relations to determine the canonical path
        //    of file-based modules with module declarations that have the `#[path]` attribute.
        //    NOTE: Decide on a best way to store and propogate the original mappings from
        //    file-system derived NodePath to canonical NodePath for use in processing incremental
        //    updates later. See method comments for more info.
        tree.update_path_index_for_custom_paths()?;

        // WARNING: This logic has moved. We are now creating ReExports after the ModuleTree is
        // built and we have resolved Ids to Cannonical Ids. Delete this code once we have
        // implementation of reexport in a new place.
        //
        // Old Code for reference:
        // 6: Process re-export relationships beween `pub use` statements and the **modules** they
        //    are re-exporting (does not cover other items like structs, functions, etc)
        //    - should be reexport --ReExports--> item definition
        //    All errors here indicate we should abort, handle these in caller:
        //      ModuleTreeError::NodePathValidation(Box::new(e))
        //      ModuleTreeError::ConflictingReExportPath
        // tree.process_export_rels(self)?; // Re-exports processed after ID resolution

        // 6. Prune unlinked file modules from the ModuleTree state
        let pruning_result = tree.prune_unlinked_file_modules()?; // Call prune, graph is not modified
        if !pruning_result.pruned_module_ids.is_empty() {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "Pruned {} unlinked modules, {} associated items, and _fill me in_ relations from ModuleTree.",
                pruning_result.pruned_module_ids.len(),
                pruning_result.pruned_item_ids.len(),
                // pruning_result.pruned_relations.len()
            );
            // TODO: Decide if/how to use pruning_result later (e.g., for diagnostics, incremental updates)
        }
        // By the time we are finished, we should have all the necessary relations to form the path
        // of all defined items by ModuleTree's shortest_public_path method.
        //  - Contains: Module --> contained items
        //  - Imports:
        Ok(tree)
    }

    #[allow(clippy::boxed_local, clippy::box_collection)]
    fn handle_unlinked_modules(&self, unlinked_infos: Vec<UnlinkedModuleInfo>) {
        if !unlinked_infos.is_empty() {
            debug!(
                "Warning: Found {} unlinked module file(s) (no corresponding 'mod' declaration):",
                unlinked_infos.len()
            );
            for info in unlinked_infos.iter() {
                // Iterate over the Boxed Vec
                debug!("  - Path: {}, ID: {}", info.definition_path, info.module_id);
                // Optionally include the absolute file path
                if let Some(module_node) = self.get_module(info.module_id) {
                    if let Some(file_path) = module_node.file_path() {
                        debug!("    File: {}", file_path.display());
                    }
                }
            }
        }
    }

    pub fn crate_context(&self) -> Option<&CrateContext> {
        self.crate_context.as_ref()
    }

    pub fn root_file(&self) -> Result<&Path> {
        let context = self
            .crate_context
            .as_ref() // Borrow the context
            .ok_or(ParsedGraphError::MissingCrateContext)?;
        let root_path = context
            .root_file()
            .ok_or_else(|| ParsedGraphError::RootFileNotFound(context.root_path.clone()))?;
        Ok(root_path)
    }

    pub fn get_root_module_checked(&self) -> Result<&ModuleNode, SynParserError> {
        // Ensure crate_context exists
        // eprintln!("crate_context: {:#?}", self.crate_context);
        // NOTE: Crate context not available for individual nodes.
        let context = self
            .crate_context
            .as_ref() // Borrow the context
            .ok_or(ParsedGraphError::MissingCrateContext)?;

        // Get the root path from the context
        let root_path = context
            .root_file()
            .ok_or_else(|| ParsedGraphError::RootFileNotFound(context.root_path.clone()))?;

        // Find the module by its file path.
        // find_module_by_file_path_checked already returns Result<&ModuleNode, SynParserError>
        self.find_module_by_file_path_checked(root_path)
    }
}

impl GraphAccess for ParsedCodeGraph {
    fn functions(&self) -> &[FunctionNode] {
        &self.graph.functions
    }

    fn defined_types(&self) -> &[TypeDefNode] {
        &self.graph.defined_types
    }

    fn type_graph(&self) -> &[TypeNode] {
        &self.graph.type_graph
    }

    fn impls(&self) -> &[ImplNode] {
        &self.graph.impls
    }

    fn traits(&self) -> &[TraitNode] {
        &self.graph.traits
    }

    fn relations(&self) -> &[SyntacticRelation] {
        // Updated type
        &self.graph.relations
    }

    fn modules(&self) -> &[ModuleNode] {
        &self.graph.modules
    }

    // Removed values()
    fn consts(&self) -> &[ConstNode] {
        // Added
        &self.graph.consts
    }

    fn statics(&self) -> &[StaticNode] {
        // Added
        &self.graph.statics
    }

    fn macros(&self) -> &[MacroNode] {
        &self.graph.macros
    }

    fn use_statements(&self) -> &[ImportNode] {
        &self.graph.use_statements
    }

    fn functions_mut(&mut self) -> &mut Vec<FunctionNode> {
        &mut self.graph.functions
    }

    fn defined_types_mut(&mut self) -> &mut Vec<TypeDefNode> {
        &mut self.graph.defined_types
    }

    fn type_graph_mut(&mut self) -> &mut Vec<TypeNode> {
        &mut self.graph.type_graph
    }

    fn impls_mut(&mut self) -> &mut Vec<ImplNode> {
        &mut self.graph.impls
    }

    fn traits_mut(&mut self) -> &mut Vec<TraitNode> {
        &mut self.graph.traits
    }

    fn relations_mut(&mut self) -> &mut Vec<SyntacticRelation> {
        // Updated type
        &mut self.graph.relations
    }

    fn modules_mut(&mut self) -> &mut Vec<ModuleNode> {
        &mut self.graph.modules
    }

    // Removed values_mut()
    fn consts_mut(&mut self) -> &mut Vec<ConstNode> {
        // Added
        &mut self.graph.consts
    }

    fn statics_mut(&mut self) -> &mut Vec<StaticNode> {
        // Added
        &mut self.graph.statics
    }

    fn macros_mut(&mut self) -> &mut Vec<MacroNode> {
        &mut self.graph.macros
    }

    fn use_statements_mut(&mut self) -> &mut Vec<ImportNode> {
        &mut self.graph.use_statements
    }

    // Removed prune_items method to keep ParsedCodeGraph immutable for now.
}

/// Logs the start of processing a module during module tree building.
fn log_build_tree_processing_module(module: &ModuleNode) {
    debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} {} ({}) | Visibility: {}",
        "Processing module for tree:".log_header(),
        module.name.log_name(),
        module.id.to_string().magenta(),
        format!("{:?}", module.visibility).cyan()
    );
}

#[cfg(test)]
mod tests {
    use anyhow::{Ok, Result};

    use crate::utils::test_setup::run_phases_and_collect;

    use super::*;

    #[test]
    fn test_build_mod_tree() -> Result<()> {
        let _ = env_logger::builder()
            .format_file(true)
            .format_line_number(true)
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();
        let parsed_graphs = run_phases_and_collect("file_dir_detection");

        let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
        merged.build_module_tree()?;
        Ok(())
    }

    #[test]
    fn test_build_mod_tree_inners() -> Result<()> {
        let _ = env_logger::builder()
            .is_test(true)
            .format_timestamp(None) // Disable timestamps
            .try_init();
        let parsed_graphs = run_phases_and_collect("file_dir_detection");

        let merged = ParsedCodeGraph::merge_new(parsed_graphs)?;
        #[cfg(feature = "validate")]
        assert!(merged.validate_unique_rels());
        let root_module = merged.get_root_module_checked()?;
        let mut tree = ModuleTree::new_from_root(root_module)?;
        // 1: Register all modules with their containment info
        for module in merged.modules() {
            log_build_tree_processing_module(module);
            tree.add_module(module.clone())?;
        }
        assert_eq!(merged.modules().len(), tree.modules().len());
        for module in merged.modules() {
            // Sanity check: all modules make it into the tree's map
            assert!(tree.modules().get(&module.id).is_some());

            // Check all imports make it in as well:
            for import in &module.imports {
                let import_is_in_tree = tree
                    .pending_imports()
                    .iter()
                    .map(|pi| pi.import_node().import_id())
                    .find(|pending_import_id| *pending_import_id == import.id);

                let export_is_in_tree = tree
                    .pending_exports()
                    .iter()
                    .map(|pi| pi.export_node().import_id())
                    .find(|pending_export_id| *pending_export_id == import.id);
                // Note the use of the exclusive or `^` symbol
                assert!(import_is_in_tree.is_some() ^ export_is_in_tree.is_some(),
                "Expect imports to be sorted into either imports or exports in the tree, not both, not neither.");
            }
        }

        // 2: Copies all relations, stores them as TreeRelation for type safety
        tree.extend_relations(
            merged
                .relations()
                .iter()
                .copied()
                .map_into::<TreeRelation>(),
        );

        // 3: Build syntactic links
        if let Err(module_tree_error) = tree.link_mods_syntactic(merged.modules()) {
            match module_tree_error {
                // Warn on this specific error, but it is safe to continue.
                ModuleTreeError::FoundUnlinkedModules(unlinked_infos) => {
                    merged.handle_unlinked_modules(unlinked_infos);
                }
                // All other erros fatal, meaning abort resolution but do not panic.
                _ => return Err(SynParserError::from(module_tree_error).into()),
            }
        }

        // 4: Process `#[path]` attributes, form `CustomPath` links
        tree.resolve_pending_path_attrs()?;
        tree.process_path_attributes()?;

        // 5: Update tree.path_index using `CustomPath` relations to determine the canonical path
        //    of file-based modules with module declarations that have the `#[path]` attribute.
        tree.update_path_index_for_custom_paths()?;

        // 6. Prune unlinked file modules from the ModuleTree state
        let pruning_result = tree.prune_unlinked_file_modules()?; // Call prune, graph is not modified
        if !pruning_result.pruned_module_ids.is_empty() {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "Pruned {} unlinked modules, {} associated items, and _fill me in_ relations from ModuleTree.",
                pruning_result.pruned_module_ids.len(),
                pruning_result.pruned_item_ids.len(),
                // pruning_result.pruned_relations.len()
            );
        }
        let all_mod_ids_with_pruned: Vec<&ModuleNodeId> = tree
            .modules()
            .keys()
            .chain(pruning_result.pruned_module_ids.iter())
            .collect();
        assert_eq!(
            merged.modules().len(),
            all_mod_ids_with_pruned.len(),
            "Expect all modules to be accounted for post-pruning"
        );
        Ok(())
    }
}
