use crate::{
    discovery::DependencyMap as _,
    resolve::{ModuleTreeError, PruningResult, TreeRelation, UnlinkedModuleInfo},
};
use anyhow::Result;
use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};
use tracing::{info, info_span, instrument};

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

    #[instrument(skip_all, fields(graph_count = graphs.len()))]
    pub fn merge_new(mut graphs: Vec<Self>) -> Result<Self, SynParserError> {
        let mut new_graph = {
            graphs.pop().ok_or(SynParserError::MergeRequiresInput)?
        };

        // Preserve crate context from any graph
        let mut found_context = new_graph.crate_context.take();
            for mut graph in graphs {
                if found_context.is_none() {
                    found_context = graph.crate_context.take();
        if let Some(ref ctx) = found_context {
            tracing::info!("name: {}, root_file: {:?}, files: {:#?}", ctx.name, ctx.root_file(), ctx.files);
        } else {
            tracing::info!("no crate_context found");
        }
                }
                new_graph.append_all(graph)?;
            }

        new_graph.crate_context = found_context;

        #[cfg(feature = "validate")]
        {
            ParsedCodeGraph::debug_relationships(&new_graph);
            tracing::debug!(target: "validate",
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
    #[instrument(skip_all, fields(self.file_path = ?self.file_path.as_path()))]
    pub fn build_module_tree(&self) -> Result<(ModuleTree, PruningResult), SynParserError> {
        #[cfg(feature = "validate")]
        assert!(self.validate_unique_rels());
        let root_module = self.get_root_module_checked()?;
        info!(module_name = root_module.name);
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
        let pruned_items = tree.prune_unlinked_file_modules()?; // Call prune, graph is not modified
        if !pruned_items.pruned_module_ids.is_empty() {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "Pruned {} unlinked modules, {} associated items, and {} relations from ModuleTree.",
                pruned_items.pruned_module_ids.len(),
                pruned_items.pruned_item_ids.len(),
                pruned_items.pruned_relations.len()
            );
            // TODO: Decide if/how to use pruned_items later (e.g., for diagnostics, incremental updates)
        }
        // 7. Link definitions to the import sites that bring them into scope (internal only).
        tree.link_definition_imports(self)?;
        // By the time we are finished, we should have all the necessary relations to form the path
        // of all defined items by ModuleTree's shortest_public_path method.
        //  - Contains: Module --> contained items
        //  - Imports:
        Ok((tree, pruned_items))
    }

    /// Removes every node and relation that belongs to a module that was pruned
    /// from the `ModuleTree`.
    ///
    /// After `ModuleTree::prune_unlinked_file_modules` is executed it returns a
    /// `PruningResult` that lists the module IDs, item IDs and relation IDs that
    /// are no longer part of the tree.
    /// This method uses that list to delete the corresponding data from the
    /// concrete `ParsedCodeGraph` so that the graph and the tree stay in sync.
    ///
    /// **Note:** The current implementation is intentionally simple and may be
    /// optimized later.
    /// **Limitations:**
    /// - Type nodes that are only referenced by the pruned items
    ///   are *not* removed, so the graph may still contain “orphaned” types after
    ///   pruning.
    /// - Currently does not explictly verify that secondary node types Variant, Field, Param, and
    ///   GenericParam are removed, though implicitly these are removed since they are fields of
    ///   other primary nodes such as Struct.
    ///
    /// - reviewed by JL Jul 27, 2025
    /// - edited by JL Jul 28, 2025 (added limitation re: secondary nodes)
    #[instrument(skip_all, fields(self.file_path = ?self.file_path.as_path()))]
    fn prune(&mut self, pruned_items: PruningResult) {
        // WARN: We are pruning all the unused items from the unlinked files, but that does not
        // include the unused types currently, meaning we could be ending up with unlinked types?
        // - Look into handling this problem within the way ModuleTree is handling types. It might
        // cause an issue later when we try to handle Canonical type ids, or earlier if there is
        // some behavior that relies on all items in the graph having a relation, and then there is
        // no relation found with type nodes (which I don't think we currently guarentee?).
        // NOTE: There are smarter ways of doing the below, this is just the easiest to write. We
        // could also be going through the children of the modules being removed, but I'd rather be
        // inefficient and hamfisted for now. Improve this later.
        //
        // -- handle pruning items
        let mut prune_counts = PruneCounts::default();
        tracing::info!(unresolved_count = %self.unresolved_nodes().len(), unresolved_pruned = %pruned_items.unresolved_nodes.len());

        // WARN: We are not currently tracking the Field, Variant, GenericParam, or ExternCrate at
        // this granularity, removing them from the number of items being counted before and after
        // removal, since they are subfields of struct, enum, union, and TypeAlias(?)
        prune_counts.pruned_items_initial = pruned_items.pruned_item_ids.len();
        // Exclude secondary IDs and `Method`: methods live on `ImplNode` / `TraitNode` and are
        // removed when the parent is dropped; `retain` does not consult method IDs. Counting
        // `Method` in `len()` would inflate the expected removal count (see
        // `diagnose_prune_counts_serde_github_clone`).
        let pruned_item_ids = pruned_items
            .pruned_item_ids
            .iter()
            .copied()
            .filter(|id| {
                !matches!(id, AnyNodeId::Variant(_))
                    && !matches!(id, AnyNodeId::Field(_))
                    && !matches!(id, AnyNodeId::Param(_))
                    && !matches!(id, AnyNodeId::GenericParam(_))
                    && !matches!(id, AnyNodeId::Method(_))
            })
            .collect_vec();
        prune_counts.removed_secondary_ids = prune_counts.pruned_items_initial- pruned_item_ids.len();

        // Methods live under `ImplNode` / `TraitNode`; module-tree BFS does not put their IDs in
        // `pruned_item_ids`, but removing an impl/trait removes all nested methods. Those removals
        // are counted in `prune_counts.methods` and must be added to the expected total.
        // See `diagnose_prune_counts_serde_github_clone` in tests/full/github_clones.rs.
        let pruned_id_set: HashSet<AnyNodeId> = pruned_item_ids.iter().copied().collect();
        let orphan_method_count: usize = self
            .impls()
            .iter()
            .filter(|n| pruned_id_set.contains(&n.id.as_any()))
            .flat_map(|imp| imp.methods.iter())
            .chain(
                self.traits()
                    .iter()
                    .filter(|n| pruned_id_set.contains(&n.id.as_any()))
                    .flat_map(|tr| tr.methods.iter()),
            )
            .filter(|m| !pruned_id_set.contains(&m.id.as_any()))
            .count();

        let func_count_pre = self.functions().len();
        self.functions_mut()
            .retain(|m| !pruned_item_ids.contains(&m.id.as_any()));
        prune_counts.functions = func_count_pre - self.functions().len();

        let defined_types_count_pre = self.defined_types().len();
        self.defined_types_mut()
            .retain(|defty| !pruned_item_ids.contains(&defty.any_id()));
        prune_counts.defined_types = defined_types_count_pre - self.defined_types().len();

        let consts_count_pre = self.consts().len();
        self.consts_mut()
            .retain(|m| !pruned_item_ids.contains(&m.id.as_any()));
        prune_counts.consts = consts_count_pre - self.consts().len();

        let statics_count_pre = self.statics().len();
        self.statics_mut()
            .retain(|m| !pruned_item_ids.contains(&m.id.as_any()));
        prune_counts.statics = statics_count_pre - self.statics().len();

        let macros_count_pre = self.macros().len();
        self.macros_mut()
            .retain(|m| !pruned_item_ids.contains(&m.id.as_any()));
        prune_counts.macros += macros_count_pre - self.macros().len();

        let use_statements_count_pre = self.use_statements().len();
        self.use_statements_mut()
            .retain(|m| !pruned_item_ids.contains(&m.id.as_any()));
        prune_counts.use_statements = use_statements_count_pre - self.use_statements().len();

        let methods_count_pre = self
            .impls()
            .iter()
            .flat_map(|imp| imp.methods.iter())
            .chain(self.traits().iter().flat_map(|tr| tr.methods.iter()))
            .count();
        let impls_count_pre = self.impls().len();
        self.impls_mut()
            .retain(|m| !pruned_item_ids.contains(&m.id.as_any()));
        prune_counts.impls = impls_count_pre - self.impls().len();

        let traits_count_pre = self.traits().len();
        self.traits_mut()
            .retain(|m| !pruned_item_ids.contains(&m.id.as_any()));
        prune_counts.traits = traits_count_pre - self.traits().len();

        let methods_count_post = self
            .impls()
            .iter()
            .flat_map(|imp| imp.methods.iter())
            .chain(self.traits().iter().flat_map(|tr| tr.methods.iter()))
            .count();
        prune_counts.methods = methods_count_pre - methods_count_post;

        // -- handle pruning module ids
        // file-based modules
        let modules_files_pre = self.modules().len();
        self.modules_mut()
            .retain(|m| !pruned_items.pruned_module_ids.contains(&m.id));
        prune_counts.file_modules = modules_files_pre - self.modules().len();

        // non-file-based modules (inline and declaration)
        let nonfile_modules_count_pre = self.modules().len();
        self.modules_mut()
            .retain(|m| !pruned_item_ids.contains(&m.id.as_any()));
        prune_counts.non_file_modules = nonfile_modules_count_pre - self.modules().len();
        assert_eq!(
            prune_counts.file_modules,
            pruned_items.pruned_module_ids.len(),
            "Count of expected pruned modules vs. pruned modules does not match."
        );

        prune_counts.type_graph = self.type_graph().len();

        let expected_resolved_removals = pruned_item_ids.len() + orphan_method_count;
        let mut removed_items = Vec::new();
        if prune_counts.count_resolved() != expected_resolved_removals {
            for item in pruned_item_ids.iter() {
                if self.find_node_unique(*item).is_ok() {
                    tracing::error!("Node not removed: {}", item);
                }
            }
            for item in pruned_item_ids.iter() {
                if self.find_node_unique(*item).is_err() {
                    removed_items.push(item);
                    tracing::trace!(target: "debug_dup", "\nNode removed: {}", item);
                }
            }
            for item in pruned_item_ids
                .iter()
                .filter(|i| !removed_items.contains(i))
            {
                tracing::error!("\nNode contained by a removed node not found or removed: {}", item);
            }
            tracing::trace!(target: "debug_dup", "Number of removed items: {}", removed_items.len());
        }
        let all_graph_ids = self.graph.functions.iter().map(|n| n.any_id())
            .chain(self.graph.defined_types.iter().map(|tydef| tydef.any_id()))
            .chain(self.graph.impls.iter().map(|n| n.any_id()))
            .chain(self.graph.traits.iter().map(|n| n.any_id()))
            .chain(self.graph.modules.iter().map(|n| n.any_id()))
            .chain(self.graph.consts.iter().map(|n| n.any_id()))
            .chain(self.graph.statics.iter().map(|n| n.any_id()))
            .chain(self.graph.macros.iter().map(|n| n.any_id()))
            .chain(self.graph.use_statements.iter().map(|n| n.any_id()))
            .chain(self.graph.unresolved_nodes.iter().map(|n| n.any_id()));
        let mut any_found = false;
        for id in all_graph_ids {
            if pruned_item_ids.contains(&id) {
                tracing::info!("Found overlap: {id}");
                any_found = true;
            }
        }
        if !any_found {
            tracing::info!("No intersection between pruned_item_ids and all_graph_ids");
        }
        let unique_pruned_item_ids = pruned_item_ids.iter().unique().collect_vec();
        if unique_pruned_item_ids.len() != pruned_item_ids.len() {
            tracing::info!("pruned_item_ids: unique: {}, total: {}", unique_pruned_item_ids.len(), pruned_item_ids.len());
        }
        let _unresolved_pruned_result_count = pruned_items.unresolved_nodes.len();
        if prune_counts.count_resolved() != expected_resolved_removals {
            tracing::error!("{prune_counts:#?}");
            tracing::error!(
                diff_resolved = %prune_counts.diff_initial_resolved(),
                diff_all = %prune_counts.diff_initial_all(),
                unresolved_removed_count = %prune_counts.unresolved_nodes,
                unresolved_prune_result_count = %prune_counts.unresolved_nodes,
                pruned_count = %pruned_item_ids.len(),
                orphan_method_count = %orphan_method_count,
                expected_resolved_removals = %expected_resolved_removals,
                removed_count = %removed_items.len()
            );
        } else {
            tracing::info!(diff_resolved = %prune_counts.diff_initial_resolved(),
                pruned_count = %pruned_item_ids.len(),
                orphan_method_count = %orphan_method_count,
                removed_count = %removed_items.len());
        }
        assert_eq!(
            prune_counts.count_resolved(),
            expected_resolved_removals,
            "Count of resolved pruned items (incl. nested methods of removed impls/traits) should equal \
             pruned_item_ids.len() + orphan_method_count; see diagnose_prune_counts_serde_github_clone."
        );

        // -- handle pruning relations
        let relations_count_pre = self.relations().len();

        // Build a set of all graph relations for O(1) membership checks.
        // `pruned_relations` may contain tree-construction relations (e.g. `ResolvesToDefinition`,
        // `CustomPath`) that were added to `ModuleTree::tree_relations` during tree building but
        // were never inserted into `ParsedCodeGraph::graph.relations`.  We must only attempt to
        // remove relations that actually live in the graph; tree-only relations have nothing to
        // remove and must not be counted in the assertion.
        let graph_rel_set: HashSet<_> = self.relations().iter().copied().collect();

        // Verify the assumption underlying graph_pruned_count: if the graph contains duplicate
        // SyntacticRelations the HashSet will be smaller than the Vec, which would cause
        // graph_pruned_count to undercount and the final assert_eq to pass spuriously.
        #[cfg(feature = "validate")]
        assert_eq!(
            graph_rel_set.len(),
            relations_count_pre,
            "Duplicate SyntacticRelations detected in graph.relations before pruning: \
             HashSet size ({}) != Vec size ({}). graph_pruned_count would undercount.",
            graph_rel_set.len(),
            relations_count_pre,
        );
        let graph_pruned_count = pruned_items
            .pruned_relations
            .iter()
            .filter(|tr| graph_rel_set.contains(tr.rel()))
            .count();

        self.relations_mut().retain(|r| {
            !pruned_items
                .pruned_relations
                .contains(&TreeRelation::new(*r))
        });

        assert_eq!(
            relations_count_pre - self.relations().len(),
            graph_pruned_count,
            "Count of expected pruned graph relations vs. actually pruned relations does not match."
        );
    }

    /// Builds the complete module-tree for this crate and prunes all items that
    /// belong to file-based modules that could not be linked to the tree.
    ///
    /// This is the primary public-facing API for turning the flat set of parsed
    /// files into a coherent crate-level structure:
    ///
    /// 1. Constructs a `ModuleTree` from the root module.
    /// 2. Registers every `ModuleNode` (definitions and declarations).
    /// 3. Copies all syntactic relations into the tree.
    /// 4. Establishes inter-module links (`ResolvesToDefinition`,
    ///    `CustomPath`, …).
    /// 5. Prunes file-based modules that remain unlinked after the above steps.
    /// 6. Removes every node, module and relation listed in the `PruningResult`
    ///    from the current `ParsedCodeGraph`, keeping the graph and the tree
    ///    consistent.
    ///
    /// After a successful call the `ParsedCodeGraph` contains only items that are
    /// reachable through the resulting `ModuleTree`.
    ///
    /// # Errors
    /// Returns any error encountered during tree construction, path resolution,
    /// or pruning (in creating the items to prune within build_module_tree).
    /// - JL Reviewed, Jul 28, 2025
    #[instrument(skip(self), fields(pruned_relations, pruned_modules, pruned_items))]
    pub fn build_tree_and_prune(&mut self) -> Result<ModuleTree, ploke_error::Error> {
        let (tree, pruned_items) = {
            let _span = info_span!("build_module_tree").entered();
            self.build_module_tree()?
        };
        {
            let _span = info_span!(
                "prune_graph_from_tree",
                pruned_modules = pruned_items.pruned_module_ids.len(),
                pruned_items = pruned_items.pruned_item_ids.len(),
                pruned_relations = pruned_items.pruned_relations.len()
            )
            .entered();
            self.prune(pruned_items);
        }
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

    // TODO: This is kind of a hack job, be more thorough.
    pub fn retain_all(&mut self, set: HashSet<AnyNodeId>) {
        self.graph
            .use_statements
            .retain(|n| set.contains(&n.any_id()));
        self.graph.impls.retain(|n| set.contains(&n.any_id()));
        self.graph.traits.retain(|n| set.contains(&n.any_id()));
        self.graph
            .defined_types
            .retain(|n| set.contains(&n.any_id()));
        self.graph.functions.retain(|n| set.contains(&n.any_id()));
        self.graph.consts.retain(|n| set.contains(&n.any_id()));
        self.graph.modules.retain(|n| set.contains(&n.any_id()));
        self.graph.statics.retain(|n| set.contains(&n.any_id()));
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

    fn unresolved_nodes(&self) -> &[UnresolvedNode] {
        &self.graph.unresolved_nodes
    }

    fn unresolved_nodes_mut(&mut self) -> &mut Vec<UnresolvedNode> {
        &mut self.graph.unresolved_nodes
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

#[derive(Debug, Default)]
struct PruneCounts {
    pruned_items_initial: usize,
    removed_secondary_ids: usize,
    functions: usize,
    defined_types: usize,
    type_graph: usize,
    impls: usize,
    traits: usize,
    #[allow(dead_code)]
    relations: usize,
    file_modules: usize,
    non_file_modules: usize,
    consts: usize,
    statics: usize,
    macros: usize,
    use_statements: usize,
    unresolved_nodes: usize,
    methods: usize,
}

impl PruneCounts {
    fn count_resolved(&self) -> usize {
        self.functions 
        + self.defined_types 
        + self.consts 
        + self.statics 
        + self.macros 
        + self.use_statements 
        + self.impls 
        + self.traits 
        + self.methods 
        + self.non_file_modules
        // self.removed_secondary_ids
        // + self.file_modules 
    }
    
    fn count_unresolved(&self) -> usize {
        self.unresolved_nodes
    }

    fn count_all_removed(&self) -> usize {
        self.count_resolved() + self.count_unresolved()
    }

    fn diff_initial_resolved(&self) -> isize {
        self.pruned_items_initial as isize - self.count_resolved() as isize
    }

    fn diff_initial_all(&self) -> isize {
        self.pruned_items_initial as isize - self.count_all_removed() as isize
    }
}
