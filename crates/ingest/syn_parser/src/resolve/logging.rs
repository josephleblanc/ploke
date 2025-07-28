use super::*;

pub(super) trait LogTree
where
    Self: RelationIndexer,
{
    fn modules(&self) -> &HashMap<ModuleNodeId, ModuleNode>;
    fn pending_imports(&self) -> &Vec<PendingImport>;
    fn pending_exports(&self) -> Option<&Vec<PendingExport>>;
    fn log_access_restricted_check_ancestor(
        &self,
        ancestor_id: ModuleNodeId,
        restriction_module_id: ModuleNodeId,
    ) {
        debug!(target: LOG_TARGET_VIS, "  {} Checking ancestor: {} ({}) against restriction: {} ({})",
            "->".dimmed(), // Indentation marker
            self.modules().get(&ancestor_id).map(|m| m.name.as_str()).unwrap_or("?").yellow(), // Ancestor name yellow
            ancestor_id.to_string().magenta(), // Ancestor ID magenta
            self.modules().get(&restriction_module_id).map(|m| m.name.as_str()).unwrap_or("?").blue(), // Restriction name blue
            restriction_module_id.to_string().magenta() // Restriction ID magenta
        );
    }

    fn log_update_path_index_processing(&self, decl_mod_id: ModuleNodeId) {
        let decl_mod_name = self
            .modules()
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} {} ({})",
            "Processing:".log_step(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_target_error(
        &self,
        decl_mod_id: ModuleNodeId,
        error: &ModuleTreeError,
    ) {
        let decl_mod_name = self
            .modules()
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Failed to find CustomPath target for {} ({}): {:?}. Skipping index update.",
            "Error:".log_error(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id(),
            error
        );
    }

    fn log_update_path_index_found_target(
        &self,
        decl_mod_id: ModuleNodeId,
        def_mod_id: ModuleNodeId,
    ) {
        let decl_mod_name = self
            .modules()
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        let def_mod_name = self
            .modules()
            .get(&def_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Found target: {} ({}) for decl {} ({})",
            "->".log_comment(),
            def_mod_name.log_name(),
            def_mod_id.to_string().log_id(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_paths(
        &self,
        decl_mod_id: ModuleNodeId,
        def_mod_id: ModuleNodeId,
        canonical_path: &NodePath,
        original_path: &NodePath,
    ) {
        // Lookup names using IDs
        let decl_mod_name = self
            .modules()
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        let def_mod_name = self
            .modules()
            .get(&def_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    {} Decl Mod: {} ({}) -> Canonical Path: {}",
            "->".log_comment(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id(), // Use ID directly
            canonical_path.to_string().log_path()
        );
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    {} Def Mod:  {} ({}) -> Original Path:  {}",
             "->".log_comment(),
            def_mod_name.log_name(),
            def_mod_id.to_string().log_id(), // Use ID directly
            original_path.to_string().log_path()
        );
    }

    fn log_update_path_index_skip_external(&self, decl_mod_id: ModuleNodeId) {
        let decl_mod_name = self
            .modules()
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "  {} Skipping index update for external path declaration {} ({}). Target not found as expected.",
            "Warning:".log_yellow(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id()
        );
    }

    fn log_update_path_index_abort_inconsistent(&self, decl_mod_id: ModuleNodeId) {
        let decl_mod_name = self
            .modules()
            .get(&decl_mod_id)
            .map(|m| m.name.as_str())
            .unwrap_or("?");
        log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Aborting path index update: Inconsistent state. CustomPath target not found for internal declaration {} ({}), but it wasn't marked as external.",
            "Error:".log_error(),
            decl_mod_name.log_name(),
            decl_mod_id.to_string().log_id()
        );
    }

    /// Logs detailed information about a relation for debugging purposes.
    /// This function is intended for verbose debugging and may perform lookups.
    fn log_relation_verbose(&self, rel: TreeRelation) {
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Relation Details:", "Verbose Log:".log_header());
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Kind: {}", rel.rel().to_string().log_name()); // Use rel()

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Source:");
        self.log_node_id_verbose(rel.rel().source()); // Use rel()

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Target:");
        self.log_node_id_verbose(rel.rel().target()); // Use rel()
    }

    /// Logs detailed information about a relation for debugging purposes.
    /// This function is intended for verbose debugging and may perform lookups.
    fn log_relation_verbose_target(&self, rel: TreeRelation, target: &str) {
        debug!(target: target, "{} Relation Details:", "Verbose Log:".log_header());
        debug!(target: target, "  Kind: {}", rel.rel().to_string().log_name()); // Use rel()

        debug!(target: target, "  Source:");
        self.log_node_id_verbose(rel.rel().source()); // Use rel()

        debug!(target: target, "  Target:");
        self.log_node_id_verbose(rel.rel().target()); // Use rel()
    }

    /// Logs detailed information about a NodeId for debugging purposes.
    /// This function is intended for verbose debugging and may perform lookups within the ModuleTree.
    fn log_node_id_verbose(&self, node_id: AnyNodeId) {
        // Changed: Parameter is AnyNodeId
        // Try to convert AnyNodeId to ModuleNodeId for module lookup
        let mod_id_result: Result<ModuleNodeId, _> = node_id.try_into();

        if let Ok(mod_id) = mod_id_result {
            if let Some(module) = self.modules().get(&mod_id) {
                // Changed: Use typed ID key
                // Log ModuleNode details
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    ID: {} ({})", node_id.to_string().log_id(), "Module".log_spring_green());
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Name: {}", module.name.log_name());
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Path: {}", module.path.join("::").log_path());
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Visibility: {}", format!("{:?}", module.visibility).log_vis());
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Kind: {}", crate::utils::logging::get_module_def_kind_str(module).log_orange());
                if let Some(fp) = module.file_path() {
                    debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      File Path: {}", fp.display().to_string().log_path());
                }
                if let Some(span) = module.inline_span() {
                    debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Inline Span: {:?}", span);
                }
                if let Some(span) = module.declaration_span() {
                    debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      Decl Span: {:?}", span);
                }
                if !module.cfgs.is_empty() {
                    debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      CFGs: {}", module.cfgs.join(", ").log_magenta());
                }
            } else {
                // Node ID was convertible to ModuleNodeId but not found in map
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    ID: {} ({})", node_id.to_string().log_id(), "Module (Not Found in Map)".log_comment());
            }
        } else {
            // Node is not a module found in self.modules()
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    ID: {} ({})", node_id.to_string().log_id(), "Node (Non-Module)".log_comment());
        }

        // Check pending imports/exports using AnyNodeId
        let is_in_pending_import = self
            .pending_imports()
            .iter()
            .any(|p| p.import_node().id.as_any() == node_id); // Compare AnyNodeId
        if is_in_pending_import {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Status: {} in pending_imports", "Found".log_yellow());
        }
        if let Some(exports) = self.pending_exports() {
            let is_in_pending_export = exports
                .iter()
                .any(|p| p.export_node().id.as_any() == node_id); // Compare AnyNodeId
            if is_in_pending_export {
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Status: {} in pending_exports", "Found".log_yellow());
            }
        }

        // Log relations FROM this node using AnyNodeId
        if let Some(relations_from) = self.get_all_relations_from(&node_id) {
            // Changed: Use AnyNodeId
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations From ({}):", relations_from.len());
            for rel_ref in relations_from {
                let target_id: AnyNodeId = rel_ref.rel().target(); // Target is AnyNodeId
                let target_id_str = target_id.to_string().log_id();
                // Try to get target name if it's a module
                let target_name = ModuleNodeId::try_from(target_id) // TryFrom AnyNodeId
                    .ok()
                    .and_then(|mid| self.modules().get(&mid))
                    .map(|m| m.name.as_str());
                let target_display = target_name
                    .map(|n| n.log_name().to_string())
                    .unwrap_or_else(|| target_id_str.to_string());

                // Format the relation variant directly
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      -> {:<18} {}", format!("{:?}", rel_ref.rel()).log_name(), target_display);
            }
        } else {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations From: {}", "None".log_error());
        }

        // Log relations TO this node using AnyNodeId
        if let Some(relations_to) = self.get_all_relations_to(&node_id) {
            // Changed: Use AnyNodeId
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations To ({}):", relations_to.len());
            for rel_ref in relations_to {
                let source_id: AnyNodeId = rel_ref.rel().source(); // Source is AnyNodeId
                let source_id_str = source_id.to_string().log_id();
                // Try to get source name if it's a module
                let source_name = ModuleNodeId::try_from(source_id) // TryFrom AnyNodeId
                    .ok()
                    .and_then(|mid| self.modules().get(&mid))
                    .map(|m| m.name.as_str());
                let source_display = source_name
                    .map(|n| n.log_name().to_string())
                    .unwrap_or_else(|| source_id_str.to_string());

                // Format the relation variant directly
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      <- {:<18} {}", format!("{:?}", rel_ref.rel()).log_name(), source_display);
            }
        } else {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations To: {}", "None".log_error());
        }
        // Removed duplicated logging block that referred to non-existent `module` variable

        // Check pending imports/exports using AnyNodeId
        let is_in_pending_import = self
            .pending_imports()
            .iter()
            .any(|p| p.import_node().id.as_any() == node_id); // Compare AnyNodeId
        if is_in_pending_import {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Status: {} in pending_imports", "Found".log_yellow());
        }
        if let Some(exports) = self.pending_exports() {
            let is_in_pending_export = exports
                .iter()
                .any(|p| p.export_node().id.as_any() == node_id); // Compare AnyNodeId
            if is_in_pending_export {
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Status: {} in pending_exports", "Found".log_yellow());
            }
        }

        // Log relations FROM this node using AnyNodeId
        if let Some(relations_from) = self.get_all_relations_from(&node_id) {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations From ({}):", relations_from.len());
            for rel_ref in relations_from {
                let target_id: AnyNodeId = rel_ref.rel().target(); // Target is AnyNodeId
                let target_id_str = target_id.to_string().log_id();
                // Try to get target name if it's a module
                let target_name = ModuleNodeId::try_from(target_id) // TryFrom AnyNodeId
                    .ok()
                    .and_then(|mid| self.modules().get(&mid))
                    .map(|m| m.name.as_str());
                let target_display = target_name
                    .map(|n| n.log_name().to_string())
                    .unwrap_or_else(|| target_id_str.to_string());

                // Format the relation variant directly
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      -> {:<18} {}", format!("{:?}", rel_ref.rel()).log_name(), target_display);
            }
        } else {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations From: {}", "None".log_error());
        }

        // Log relations TO this node using AnyNodeId
        if let Some(relations_to) = self.get_all_relations_to(&node_id) {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations To ({}):", relations_to.len());
            for rel_ref in relations_to {
                let source_id: AnyNodeId = rel_ref.rel().source(); // Source is AnyNodeId
                let source_id_str = source_id.to_string().log_id();
                // Try to get source name if it's a module
                let source_name = ModuleNodeId::try_from(source_id) // TryFrom AnyNodeId
                    .ok()
                    .and_then(|mid| self.modules().get(&mid))
                    .map(|m| m.name.as_str());
                let source_display = source_name
                    .map(|n| n.log_name().to_string())
                    .unwrap_or_else(|| source_id_str.to_string());

                // Format the relation variant directly
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "      <- {:<18} {}", format!("{:?}", rel_ref.rel()).log_name(), source_display);
            }
        } else {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "    Relations To: {}", "None".log_error());
        }
    }
}
// Extension trait for Path normalization
// trait PathNormalize {
//     fn normalize(&self) -> PathBuf;
// }

// impl PathNormalize for std::path::Path {
//     fn normalize(&self) -> PathBuf {
//         let mut components = Vec::new();
//
//         for component in self.components() {
//             match component {
//                 std::path::Component::ParentDir => {
//                     if components
//                         .last()
//                         .map(|c| c != &std::path::Component::RootDir)
//                         .unwrap_or(false)
//                     {
//                         components.pop();
//                     }
//                 }
//                 std::path::Component::CurDir => continue,
//                 _ => components.push(component),
//             }
//         }
//
//         components.iter().collect()
//     }
// }
