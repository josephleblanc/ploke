use super::*;

impl LogDataStructure for ModuleTree {}
impl RelationIndexer for ModuleTree {
    fn relations_by_source(&self) -> &HashMap<AnyNodeId, Vec<usize>> {
        &self.relations_by_source
    }

    fn relations_by_source_mut(&mut self) -> &mut HashMap<AnyNodeId, Vec<usize>> {
        &mut self.relations_by_source
    }

    fn relations_by_target(&self) -> &HashMap<AnyNodeId, Vec<usize>> {
        &self.relations_by_target
    }

    fn relations_by_target_mut(&mut self) -> &mut HashMap<AnyNodeId, Vec<usize>> {
        &mut self.relations_by_target
    }

    fn tree_relations(&self) -> &Vec<TreeRelation> {
        &self.tree_relations
    }

    fn tree_relations_mut(&mut self) -> &mut Vec<TreeRelation> {
        &mut self.tree_relations
    }
}
impl LogTree for ModuleTree {
    fn modules(&self) -> &HashMap<ModuleNodeId, ModuleNode> {
        &self.modules
    }

    fn pending_imports(&self) -> &Vec<PendingImport> {
        &self.pending_imports
    }

    fn pending_exports(&self) -> Option<&Vec<PendingExport>> {
        self.pending_exports.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct ModuleTree {
    // ModuleNodeId of the root file-level module, e.g. `main.rs`, `lib.rs`, used to initialize the
    // ModuleTree.
    pub(super) root: ModuleNodeId,
    root_file: PathBuf,
    /// Index of all modules in the merged `CodeGraph`, in a HashMap for efficient lookup
    modules: HashMap<ModuleNodeId, ModuleNode>,
    /// Temporary storage for unresolved imports (e.g. `use` statements)
    pending_imports: Vec<PendingImport>,
    /// Temporary storage for unresolved exports (e.g. `pub use` statements).
    /// Wrapped in Option to allow taking ownership without cloning in `process_export_rels`.
    pending_exports: Option<Vec<PendingExport>>,
    /// Reverse path indexing to find NodeId on a given path
    /// HashMap appropriate for many -> few possible mapping
    /// Contains all `NodeId` items except module declarations due to
    /// path collision with defining module.
    pub(super) path_index: HashMap<NodePath, AnyNodeId>,
    /// Maps declaration module IDs with `#[path]` attributes pointing outside the crate's
    /// `src` directory to the resolved absolute external path. These paths do not have
    /// corresponding `ModuleNode` definitions within the analyzed crate context.
    external_path_attrs: HashMap<ModuleNodeId, PathBuf>,
    /// Separate HashMap for module declarations.
    /// Reverse lookup, but can't be in the same HashMap as the modules that define them, since
    /// they both have the same `path`. This should be the only case in which two items have the
    /// same path.
    pub(super) decl_index: HashMap<NodePath, ModuleNodeId>,
    tree_relations: Vec<TreeRelation>,
    /// re-export index for faster lookup during visibility resolution.
    reexport_index: HashMap<NodePath, ReexportNodeId>,
    /// Stores resolved absolute paths for modules declared with `#[path]` attributes
    /// that point to files *within* the crate's `src` directory.
    /// Key: ID of the declaration module (`mod foo;`).
    /// Value: Resolved absolute `PathBuf` of the target file.
    found_path_attrs: HashMap<ModuleNodeId, PathBuf>,
    /// Temporarily stores the IDs of module declarations that have a `#[path]` attribute.
    /// Used during the initial tree building phase before paths are fully resolved.
    /// Wrapped in `Option` to allow taking ownership via `take()` during processing.
    pending_path_attrs: Option<Vec<ModuleNodeId>>,

    /// Index mapping a source `NodeId` to a list of indices
    /// into the `tree_relations` vector where that ID appears as the source.
    /// Used for efficient lookup of outgoing relations.
    relations_by_source: HashMap<AnyNodeId, Vec<usize>>,
    /// Index mapping a target `NodeId` to a list of indices
    /// into the `tree_relations` vector where that ID appears as the target.
    /// Used for efficient lookup of incoming relations.
    relations_by_target: HashMap<AnyNodeId, Vec<usize>>,
}

impl ModuleTree {
    /// Adds a `ModuleNode` to the `ModuleTree`, updating internal state and indices.
    ///
    /// This function performs several key actions during the initial phase of building the module tree:
    ///
    /// 1. **Stores the Module:** Inserts the provided `ModuleNode` into the `modules` HashMap,
    ///    keyed by its `ModuleNodeId`.
    /// 2. **Indexes Paths:**
    ///    * Adds the module's definition path (`NodePath`) to the appropriate index
    ///      (`path_index` for definitions, `decl_index` for declarations).
    ///    * Checks for duplicate paths and returns `ModuleTreeError::DuplicatePath` if a
    ///      conflict is found.
    ///    * Note: The path->Id indexes for definitions and declarations must be kept separate
    ///      because a module's declaration, e.g. `mod module_a;` and the file-based module this
    ///      declaration points to, e.g. `project/src/module_a.rs` or directory, e.g.
    ///      `project/src/module_a/mod.rs`, have the same canonical path `crate::module_a`.
    /// 3. **Separates Imports/Exports:**
    ///    * Filters the module's `imports` (`use` statements).
    ///    * Adds private imports (`use some::item;` or `extern crate ...;`) identified by
    ///      `is_inherited_use()` to `pending_imports`.
    ///    * Adds all re-exports (`pub use`, `pub(crate) use`, `pub(in path) use`) identified by
    ///      `is_any_reexport()` to `pending_exports`.
    /// 4. **Tracks Path Attributes:** If the module has a `#[path]` attribute, its ID is added to
    ///    `pending_path_attrs` for later resolution.
    /// 5. **Checks for Duplicate IDs:** Returns `ModuleTreeError::DuplicateModuleId` if a module
    ///    with the same ID already exists in the `modules` map.
    ///
    /// # Arguments
    /// * `module`: The `ModuleNode` to add to the tree. The function takes ownership.
    ///
    /// # Returns
    /// * `Ok(())` on successful addition and indexing.
    /// * `Err(ModuleTreeError)` if:
    ///     * The module's path conflicts with an existing entry (`DuplicatePath`).
    ///     * A module with the same ID already exists (`DuplicateModuleId`).
    ///     * The module's path is invalid (`NodePathValidation`).
    ///     * An internal state error occurs (e.g., `InvalidStatePendingExportsMissing`).
    pub fn add_module(&mut self, module: ModuleNode) -> Result<(), ModuleTreeError> {
        let imports = module.imports.clone();
        // Add all private imports
        self.pending_imports.extend(
            // NOTE: We already have `Relation::ModuleImports` created at parsing time.
            imports
                .iter()
                .filter(|imp| imp.is_inherited_use())
                .map(|imp| PendingImport::from_import(imp.clone(), module.id)),
        );

        // Add all re-exports to the Vec inside the Option
        if let Some(exports) = self.pending_exports.as_mut() {
            exports.extend(
                imports
                    .iter()
                    .filter(|imp| imp.is_any_reexport()) // Updated method name
                    .map(|imp| PendingExport::from_export(imp.clone(), module.id)),
            );
        } else {
            // This state is invalid. pending_exports should only be None after process_export_rels
            // has been called and taken ownership. If we are adding a module, it means
            // process_export_rels hasn't run yet (or ran unexpectedly early).
            return Err(ModuleTreeError::InvalidStatePendingExportsMissing {
                module_id: module.id,
            });
        }

        // Use map_err for explicit conversion from SynParserError to ModuleTreeError
        let node_path = NodePath::try_from(module.path().clone())
            .map_err(|e| ModuleTreeError::NodePathValidation(Box::new(e)))?;
        let conflicting_id = module.id; // ID of the module we are trying to add

        // Separate declaration and definition path->Id indexes.
        // Indexes for declaration vs definition (inline or filebased) must be kept separate to
        // avoid collision, as module definitions and declarations have the same canonical path.
        if module.is_decl() {
            match self.decl_index.entry(node_path.clone()) {
                // Clone node_path for the error case
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // Path already exists
                    let existing_id = *entry.get();
                    return Err(ModuleTreeError::DuplicatePath {
                        path: node_path,                         // Use the cloned path
                        existing_id: existing_id.as_any(),       // This is ModuleNodeId, convert
                        conflicting_id: conflicting_id.as_any(), // This is ModuleNodeId, convert
                    });
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    // Path is free, insert it
                    entry.insert(conflicting_id);
                }
            }
        } else {
            match self.path_index.entry(node_path.clone()) {
                // Clone node_path for the error case
                std::collections::hash_map::Entry::Occupied(entry) => {
                    // Path already exists
                    let existing_id = *entry.get(); // This is AnyNodeId
                    return Err(ModuleTreeError::DuplicatePath {
                        path: node_path,                         // Use the cloned path
                        existing_id,                             // Keep as AnyNodeId
                        conflicting_id: conflicting_id.as_any(), // Convert ModuleNodeId to AnyNodeId
                    });
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    // Path is free, insert it
                    entry.insert(conflicting_id.as_any());
                }
            }
        }

        // Assign new Id wrapper for modules for better type-safety
        self.log_module_insert(&module);

        // Store path attribute if present
        // Index `#[path = "dir/to/file.rs"]` for later processing in `resolve_pending_path_attrs`
        // and `process_path_attributes`
        if module.has_path_attr() {
            self.log_add_pending_path(module.id, &module.name);
            self.pending_path_attrs
                .as_mut()
                .expect("Invariant: pending_path_attrs should always be Some before take()")
                .push(module.id); // clarity. This should be invariant, however.
        }

        // Finally, if no error have been encountered, we insert all modules of any kind to a
        // shared index of ModuleId->ModuleNode for lookup later.
        let dup_node = self.modules.insert(module.id, module);
        if let Some(dup) = dup_node {
            self.log_duplicate(&dup);
            return Err(ModuleTreeError::DuplicateModuleId(Box::new(dup)));
        }

        Ok(())
    }

    pub fn add_relations_batch(
        &mut self,
        relations: &[SyntacticRelation],
    ) -> Result<(), ModuleTreeError> {
        for rel in relations.iter() {
            self.add_rel(TreeRelation::from(*rel)); // Explicitly convert using From
        }
        Ok(())
    }

    pub fn root(&self) -> ModuleNodeId {
        self.root
    }

    pub fn modules(&self) -> &HashMap<ModuleNodeId, ModuleNode> {
        &self.modules
    }

    /// Returns a reference to the internal path index mapping canonical paths to NodeIds.
    pub fn path_index(&self) -> &HashMap<NodePath, AnyNodeId> {
        // Changed: Return map with AnyNodeId value
        &self.path_index
    }

    /// Returns a slice of the relations relevant to the module tree structure.
    pub fn tree_relations(&self) -> &[TreeRelation] {
        &self.tree_relations
    }

    /// Returns a slice of the pending private imports collected during tree construction.
    pub fn pending_imports(&self) -> &[PendingImport] {
        &self.pending_imports
    }

    /// Returns a slice of the pending public re-exports collected during tree construction.
    /// Returns an empty slice if the internal Option is None.
    pub fn pending_exports(&self) -> &[PendingExport] {
        self.pending_exports
            .as_deref() // Converts Option<Vec<T>> to Option<&[T]>
            .unwrap_or(&[]) // Returns &[] if None
    }

    pub fn new_from_root(root: &ModuleNode) -> Result<Self, ModuleTreeError> {
        let root_id = root.id;
        let root_file = root
            .file_path()
            .ok_or(ModuleTreeError::RootModuleNotFileBased(root_id))?;
        Ok(Self {
            root: root_id,
            root_file: root_file.to_path_buf(),
            modules: HashMap::new(),
            pending_imports: vec![],
            pending_exports: Some(vec![]), // Initialize with Some(empty_vec)
            path_index: HashMap::new(),
            decl_index: HashMap::new(),
            tree_relations: vec![],
            reexport_index: HashMap::new(),
            found_path_attrs: HashMap::new(),
            external_path_attrs: HashMap::new(), // Initialize the new field
            pending_path_attrs: Some(Vec::new()),
            relations_by_source: HashMap::new(),
            relations_by_target: HashMap::new(),
        })
        // Should never happen, but might want to handle this sometime
        // // Initialize path attributes from root if present
        // if let Some(path_val) = extract_attribute_value(&root.attributes, "path") {
        //     if let Some(root_file) = root.file_path() {
        //         tree.path_attributes.insert(
        //             root_id,
        //             root_file.parent().unwrap().join(path_val).normalize(),
        //         );
        //     }
        // }
    }

    /// Adds a relation *between two modules* to the tree, first checking if both the source
    /// and target module nodes exist in the `modules` map.
    ///
    /// This is intended for relations like `ResolvesToDefinition` or `CustomPath` where both
    /// ends are expected to be modules already registered in the tree.
    ///
    /// Returns `ModuleTreeError::ModuleNotFound` if either module is not found.
    /// Returns `ModuleTreeError::InternalState` if the source or target of the relation
    /// cannot be converted to a `ModuleNodeId`.
    pub fn add_new_mod_rel_checked(&mut self, tr: TreeRelation) -> Result<(), ModuleTreeError> {
        let relation = tr.rel();
        let source_any_id = relation.source(); // Returns AnyNodeId
        let target_any_id = relation.target(); // Returns AnyNodeId

        // Attempt to convert source and target to ModuleNodeId
        let source_mod_id = ModuleNodeId::try_from(source_any_id).map_err(|_| {
            ModuleTreeError::InternalState(format!(
                "Source ID {} of relation {:?} is not a ModuleNodeId, cannot use add_new_mod_rel_checked",
                source_any_id, relation
            ))
        })?;
        let target_mod_id = ModuleNodeId::try_from(target_any_id).map_err(|_| {
            ModuleTreeError::InternalState(format!(
                "Target ID {} of relation {:?} is not a ModuleNodeId, cannot use add_new_mod_rel_checked",
                target_any_id, relation
            ))
        })?;

        // Check if both modules exist in the map
        if !self.modules.contains_key(&source_mod_id) {
            return Err(ModuleTreeError::ModuleNotFound(source_mod_id));
        }
        if !self.modules.contains_key(&target_mod_id) {
            return Err(ModuleTreeError::ModuleNotFound(target_mod_id));
        }

        // Checks passed, add the relation using the logic from add_rel
        let new_index = self.tree_relations.len();
        self.tree_relations.push(tr); // Push the original TreeRelation

        // Update indices using the AnyNodeIds obtained earlier
        self.relations_by_source
            .entry(source_any_id)
            .or_default()
            .push(new_index);
        self.relations_by_target
            .entry(target_any_id)
            .or_default()
            .push(new_index);

        Ok(())
    }
    /// Extends the module tree with multiple relations efficiently.
    ///
    /// This method takes an iterator yielding `Relation` items and adds them to the
    /// tree's internal storage. It updates the `tree_relations` vector and both
    /// index HashMaps (`relations_by_source` and `relations_by_target`).
    ///
    /// This is generally more efficient than calling `add_relation` repeatedly in a loop,
    /// especially for large numbers of relations, as it reserves capacity for the
    /// `tree_relations` vector upfront if the iterator provides a size hint.
    ///
    /// Note: This method performs *unchecked* insertion, meaning it does not verify
    /// if the source or target nodes of the relations exist within the `modules` map.
    /// Use `add_rel_checked` if such checks are required for individual relations.
    ///
    /// # Arguments
    /// * `relations_iter`: An iterator that yields `Relation` items to be added.
    pub(crate) fn extend_relations<I>(&mut self, relations_iter: I)
    where
        I: IntoIterator<Item = TreeRelation>,
    {
        let relations_iter = relations_iter.into_iter(); // Ensure we have an iterator

        // Get the starting index for the new relations
        let mut current_index = self.tree_relations.len();

        // Reserve capacity in the main vector if the iterator provides a hint
        let (lower_bound, upper_bound) = relations_iter.size_hint();
        let reserve_amount = upper_bound.unwrap_or(lower_bound);
        if reserve_amount > 0 {
            self.tree_relations.reserve(reserve_amount);
            // Note: Reserving capacity for HashMaps is more complex as it depends on
            // the number of *new* keys, not just the total number of relations.
            // We'll let the HashMaps resize as needed for simplicity here.
        }

        // Iterate through the provided relations
        for tr in relations_iter {
            // Convert to TreeRelation (cheap wrapper)
            let source_id = tr.rel().source(); // Get AnyNodeId
            let target_id = tr.rel().target(); // Get AnyNodeId

            // Update the source index HashMap using AnyNodeId key
            // entry().or_default() gets the Vec<usize> for the source_id,
            // creating it if it doesn't exist, then pushes the current_index.
            self.relations_by_source
                .entry(source_id) // Use AnyNodeId directly
                .or_default()
                .push(current_index);

            // Update the target index HashMap similarly using AnyNodeId key
            self.relations_by_target
                .entry(target_id) // Use AnyNodeId directly
                .or_default()
                .push(current_index);

            // Add the relation to the main vector
            self.tree_relations.push(tr);

            // Increment the index for the next relation
            current_index += 1;
        }
    }

    fn add_reexport_checked(
        &mut self,
        public_reexport_path: NodePath,
        target_node_id: ReexportNodeId,
    ) -> Result<(), ModuleTreeError> {
        match self.reexport_index.entry(public_reexport_path.clone()) {
            // Clone path for error case
            std::collections::hash_map::Entry::Occupied(entry) => {
                let existing_target_id = *entry.get();
                if existing_target_id != target_node_id {
                    // Found a *different* item already re-exported at this exact public path.
                    return Err(ModuleTreeError::ConflictingReExportPath {
                        path: public_reexport_path, // Use the cloned path
                        existing_id: existing_target_id,
                        conflicting_id: target_node_id, // The item we just resolved
                    });
                }
                // If it's the same target ID, do nothing (idempotent)
            }
            std::collections::hash_map::Entry::Vacant(entry) => {
                // Path is free, insert the mapping: public_path -> actual_target_id
                entry.insert(target_node_id);
            }
        };
        Ok(())
    }

    pub fn get_root_module(&self) -> Result<&ModuleNode, ModuleTreeError> {
        self.modules
            .get(&self.root)
            .ok_or(ModuleTreeError::RootModuleNotFound(self.root))
    }

    /// Finds the absolute file path of the file-based module containing the given primary node ID.
    ///
    /// This traverses upwards from the node's immediate parent module. If the parent
    /// is inline, it continues searching upwards using the `get_parent_module_id` helper
    /// until a file-based module is found.
    // TODO: Handle Assoc/Secondary items in another function.
    // Write another function or method to handle the case of Assoc or Secondary items.
    // This approach will not directly work for Associated or Secondary items, whether or not we
    // include the PrimaryNodeIdTrait here. We may wish to implement a new method that will find
    // the direct parent for any given node (e.g. Assoc nodes like MethodNode) or Secondary Nodes
    // like struct fields, to handle these cases.
    pub fn find_defining_file_path_ref_seq<T: PrimaryNodeIdTrait>(
        &self,
        typed_pid: T,
    ) -> Result<&Path, ModuleTreeError> {
        // 1. Find the immediate parent module ID using the relation index.
        //    We still need this initial lookup as the input is AnyNodeId.
        let initial_parent_mod_id = self
            .get_iter_relations_to(&typed_pid.as_any()) // Use iterator version
            .ok_or(ModuleTreeError::NoRelationsFoundForId(typed_pid.as_any()))? // Use specific error
            .find_map(|tr| tr.rel().source_contains(typed_pid))
            .ok_or(ModuleTreeError::ContainingModuleNotFound(
                typed_pid.as_any(),
            ))?; // Error if no Contains relation found

        let mut current_mod_id = initial_parent_mod_id;
        let mut recursion_limit = 100; // Safety break

        // 2. Loop upwards using get_parent_module_id
        loop {
            recursion_limit -= 1;
            if recursion_limit <= 0 {
                return Err(ModuleTreeError::RecursionLimitExceeded {
                    start_node_id: typed_pid.as_any(), // Use the original input ID
                    limit: 100,
                });
            }

            // Get the current module node using the typed ID
            let module_node = self.get_module_checked(&current_mod_id)?; // Use checked version with ?

            // 3. Check if the current module is file-based
            if let Some(file_path) = module_node.file_path() {
                // Found the defining file
                return Ok(file_path);
            }

            // 4. Check if we've reached the root (and it wasn't file-based, handled above)
            if current_mod_id == self.root {
                // If root is reached and wasn't file-based, it's an error state.
                return Err(ModuleTreeError::RootModuleNotFileBased(self.root));
            }

            // 5. If inline or declaration, find its parent module ID using the helper
            current_mod_id = self.get_parent_module_id(current_mod_id).ok_or_else(|| {
                // If get_parent_module_id returns None (and not at root), the tree is inconsistent
                self.log_find_decl_dir_missing_parent(current_mod_id); // Log helper
                ModuleTreeError::InternalState(format!(
                    "Module tree inconsistent: Parent not found for non-root module {}",
                    current_mod_id
                ))
            })?;
        }
    }

    pub fn resolve_pending_path_attrs(&mut self) -> Result<(), ModuleTreeError> {
        self.log_resolve_entry_exit(true); // Log entry

        let module_ids: Vec<ModuleNodeId> = match self.pending_path_attrs.take() {
            // Changed: Type annotation
            Some(pending_ids) => {
                if pending_ids.is_empty() {
                    self.log_resolve_pending_status(None);
                    self.log_resolve_entry_exit(false); // Log exit
                    return Ok(());
                }
                self.log_resolve_pending_status(Some(pending_ids.len()));
                pending_ids
            }
            None => {
                self.log_resolve_pending_status(None);
                self.log_resolve_entry_exit(false); // Log exit
                return Ok(()); // TODO: This should return error. It means the invariant of the
                               // pending path attrs always being `Some` outside of this function is not being
                               // respected.
            }
        };

        for module_id in module_ids {
            // Log which ID we are starting to process
            self.log_resolve_step(module_id, "Processing ID", &module_id.to_string(), false);

            let base_dir = match self.get_file_declaring_dir(module_id) {
                Ok(dir) => {
                    self.log_resolve_step(
                        module_id,
                        "Find Base Dir",
                        &dir.display().to_string(),
                        false,
                    );
                    dir
                }
                Err(e) => {
                    self.log_resolve_step(module_id, "Find Base Dir", &format!("{:?}", e), true);
                    Self::log_path_attr_not_found(module_id);
                    // Continue processing other IDs, maybe collect errors later?
                    // For now, let's return the first error encountered to halt processing.
                    return Err(ModuleTreeError::UnresolvedPathAttr(Box::new(e)));
                }
            };

            let module = match self.get_module_checked(&module_id) {
                Ok(m) => m,
                Err(e) => {
                    self.log_resolve_step(module_id, "Get Module Node", &format!("{:?}", e), true);
                    continue; // Skip this ID if module node not found
                }
            };

            let path_val = match extract_path_attr_from_node(module) {
                Some(val) => {
                    self.log_resolve_step(module_id, "Extract Attr Value", val, false);
                    val
                }
                None => {
                    self.log_resolve_step(
                        module_id,
                        "Extract Attr Value",
                        "Attribute value not found",
                        true,
                    );
                    continue; // Skip if attribute value missing
                }
            };

            // Consider adding error handling for normalization if needed
            let resolved = base_dir.join(path_val).normalize();
            // *** NEW LOGGING CALL ***
            self.log_resolve_step(
                module_id,
                "Normalize Path",
                &resolved.display().to_string(),
                false,
            );

            match self.found_path_attrs.entry(module_id) {
                std::collections::hash_map::Entry::Occupied(entry) => {
                    let existing_path = entry.get().clone();
                    self.log_resolve_duplicate(module_id, &existing_path, &resolved);
                    return Err(ModuleTreeError::DuplicatePathAttribute {
                        module_id,
                        existing_path,
                        conflicting_path: resolved,
                    });
                }
                std::collections::hash_map::Entry::Vacant(entry) => {
                    Self::log_resolve_insert(module_id, &resolved);
                    entry.insert(resolved);
                }
            };
        }

        self.log_resolve_entry_exit(false); // Log exit
        Ok(())
    }

    pub fn get_module_checked(
        &self,
        module_id: &ModuleNodeId,
    ) -> Result<&ModuleNode, ModuleTreeError> {
        self.modules
            .get(module_id)
            .ok_or(ModuleTreeError::ModuleNotFound(*module_id))
    }

    /// Links modules (declaration/definition) syntactically by building relations between them and
    /// adding the relations to the module tree's `tree_relation` field.
    /// Builds 'ResolvesToDefinition' relations between module declarations and their file-based definitions.
    /// Assumes the `path_index` and `decl_index` have been populated correctly by `add_module`.
    /// Returns `Ok(())` on complete success.
    /// Returns `Err(ModuleTreeError::FoundUnlinkedModules)` if only unlinked modules are found.
    /// Returns other `Err(ModuleTreeError)` variants on fatal errors (e.g., path validation).
    pub(crate) fn link_mods_syntactic(
        &mut self,
        modules: &[ModuleNode],
    ) -> Result<(), ModuleTreeError> {
        // Return Ok(()) or Err(ModuleTreeError)
        let mut new_relations: Vec<TreeRelation> = Vec::new();
        let mut collected_unlinked: Vec<UnlinkedModuleInfo> = Vec::new(); // Store only unlinked info
        let root_id = self.root();

        for module in modules
            .iter()
            .filter(|m| m.is_file_based() && m.id != root_id)
        {
            // This is ["crate", "renamed_path", "actual_file"] for the file node
            let path = module.path();

            // Log the attempt to find a declaration matching the *file's* definition path
            self.log_path_resolution(module, path, "Checking", Some("decl_index..."));

            match self.decl_index.get(path.as_slice()) {
                Some(decl_id) => {
                    // Found declaration, create relation
                    let resolves_to_rel = SyntacticRelation::ResolvesToDefinition {
                        source: *decl_id,  // Declaration Node (NodeId)
                        target: module.id, // Definition Node (NodeId)
                    };
                    self.log_relation(resolves_to_rel, None);
                    new_relations.push(resolves_to_rel.into());
                }
                None => {
                    // No declaration found matching the file's definition path.
                    self.log_unlinked_module(module, path);
                    let node_path = NodePath::try_from(path.clone()) // Use the file's path
                        .map_err(|e| ModuleTreeError::NodePathValidation(Box::new(e)))?;

                    // If path conversion succeeded, collect the unlinked info.
                    collected_unlinked.push(UnlinkedModuleInfo {
                        module_id: module.id,
                        definition_path: node_path,
                    });
                }
            }
        }

        // Append relations regardless of whether unlinked modules were found.
        // We only skip appending if a fatal error occurred earlier (which would have returned Err).
        for relation in new_relations.into_iter() {
            self.add_rel(relation);
        }

        // Check if any unlinked modules were collected
        if collected_unlinked.is_empty() {
            Ok(()) // Complete success
        } else {
            // Only non-fatal "unlinked" issues occurred. Return the specific error variant.
            Err(ModuleTreeError::FoundUnlinkedModules(collected_unlinked))
        }
    }

    // Helper needed for visibility check upwards (simplified version of ModuleTree::is_accessible)
    // Checks if `target_id` (decl or inline mod) is accessible *from* `potential_parent_id`
    #[allow(unused_variables, dead_code)]
    pub(super) fn is_accessible_from(
        &self,
        potential_parent_id: ModuleNodeId,
        target_id: ModuleNodeId,
    ) -> bool {
        is_accessible_from(self, potential_parent_id, target_id)
    }

    /// Checks if an item (`target_item_id`) is reachable via a chain of `ReExports` relations
    /// starting from a specific `ImportNode` (`start_import_id`).
    /// Used to detect potential re-export cycles or verify paths.
    #[allow(
        dead_code,
        reason = "May be useful later for cycle detection or validation"
    )]
    fn is_part_of_reexport_chain(
        &self,
        start_import_id: ImportNodeId,
        target_item_id: AnyNodeId, // Target can be any node type
    ) -> Result<bool, ModuleTreeError> {
        is_part_of_reexport_chain(self, start_import_id, target_item_id)
    }

    // TODO: Make a parallellized version with rayon
    // fn process_export_rels(&self, graph: &ParsedCodeGraph) -> Result<Vec<TreeRelation>, ModuleTreeError> {
    //     todo!()
    // }
    //
    // or

    #[cfg(not(feature = "reexport"))]
    pub fn process_export_rels(&mut self, graph: &ParsedCodeGraph) -> Result<(), ModuleTreeError> {
        // Take ownership of the pending exports Vec, leaving None in its place.
        let pending = match self.pending_exports.take() {
            Some(p) => p,
            None => {
                log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "process_export_rels called when pending_exports was already None.");
                return Ok(()); // Nothing to process
            }
        };

        let mut new_relations: Vec<TreeRelation> = Vec::new();
        for export in pending {
            // Iterate over the owned Vec
            // get the target node:
            let source_mod_id = export.containing_mod_id(); // Use owned export
            let export_node = export.export_node();

            // Create relation
            // let relation = Relation { // Relation creation moved to resolve_single_export
            //     // WARNING:
            //     // Bug, currently forms relation with it's containing module, NOT the target that
            //     // the `ImportNode` is actually re-exporting.
            //     source: *source_mod_id.as_inner(), // Use NodeId directly
            //     target: export_node.id, // Use NodeId directly
            //     kind: RelationKind::ReExports,
            // };
            // self.log_rel(relation, None);

            new_relations.push(relation.into());

            // NOTE: `relation` is not defined here, this block seems broken.
            // Add to reexport_index
            // FIXME: This block is likely broken due to `as_inner` and `get_item_module_path` issues.
            //        Commenting out until `get_item_module_path` is refactored or this cfg block removed.
            /*
            if let Some(reexport_name) = export_node.source_path.last() {
                // ERROR: Uses as_inner() and potentially broken get_item_module_path
                let mut reexport_path = graph.get_item_module_path(*source_mod_id.as_inner());
                // Check for renamed export path, e.g. `a::b::Struct as RenamedStruct`
                if export_node.is_renamed() {
                    // if renamed, use visible_name for path extension
                    // TODO: Keep a list of renamed modules specifically to track possible
                    // collisions.
                    reexport_path.push(export_node.visible_name.clone());
                } else {
                    // otherwise, use standard name
                    reexport_path.push(reexport_name.clone());
                }

                let node_path = NodePath::try_from(reexport_path)
                    .map_err(|e| ModuleTreeError::NodePathValidation(Box::new(e)))?;

                let debug_node_path = node_path.clone();

                // Check for duplicate re-exports at the same path
                match self.reexport_index.entry(node_path) {
                    std::collections::hash_map::Entry::Occupied(entry) => {
                        // NOTE: Could filter for cfg here, to make graph cfg aware in a
                        // relatively easy way.
                        let existing_id = *entry.get();
                        if existing_id != export_node.id {
                            // Found a different NodeId already registered for this exact path.
                            return Err(ModuleTreeError::ConflictingReExportPath {
                                path: debug_node_path, // Use the cloned path
                                existing_id,
                                conflicting_id: export_node.id,
                            });
                        }
                        // If it's the same ID, do nothing (idempotent)
                    }
                    std::collections::hash_map::Entry::Vacant(entry) => {
                        // Path is free, insert the new re-export ID.
                        entry.insert(export_node.id); // ERROR: export_node.id is ImportNodeId, reexport_index expects ReexportNodeId
                    }
                }
            }
            */
        }
        for new_tr in new_relations {
            // NOTE: new_relations might be empty due to commented block
            self.add_rel(new_tr);
        }

        Ok(())
    }

    /// Iterates through pending re-exports (`pub use`), resolves their targets,
    /// updates the `reexport_index`, and adds the corresponding `ReExports`
    /// relations to the `tree_relations`.
    ///
    /// Relies on `path_index` and `decl_index` being populated correctly beforehand.
    /// Returns an error if a re-export target cannot be resolved or if conflicting
    /// re-exports are found at the same public path.
    #[cfg(feature = "reexport")]
    pub fn process_export_rels(
        &mut self,
        graph: &ParsedCodeGraph, // Keep graph for potential future use in logging or resolution
    ) -> Result<(), ModuleTreeError> {
        // Take ownership of the pending exports Vec, leaving None in its place.
        let pending = match self.pending_exports.take() {
            Some(p) => p,
            None => {
                log::warn!(target: LOG_TARGET_MOD_TREE_BUILD, "process_export_rels called when pending_exports was already None.");
                return Ok(()); // Nothing to process
            }
        };

        for export in pending {
            // Iterate over the owned Vec
            // Call the helper function to resolve this single export
            match self.resolve_single_export(&export, graph) {
                Ok((relation, public_reexport_path)) => {
                    // Log the correctly formed relation
                    // Note: The target_node_id is relation.target
                    // The target() method always returns AnyNodeId, so no match needed.
                    let target_node_id = relation.target();

                    self.log_relation(relation, Some("ReExport Target Resolved")); // Log before potential error

                    let reexport = target_node_id.try_into()?; // Convert AnyNodeId to ReexportNodeId

                    // Update the reexport_index: public_path -> target_node_id
                    self.add_reexport_checked(public_reexport_path, reexport)?;

                    // If index update succeeded, add relation using the unchecked method
                    // TODO: Revisit this, not sure this is sound.
                    self.add_rel(relation.into());
                }
                Err(e) => {
                    // Decide error handling: Propagate first error or collect all?
                    // Propagating first error for now.
                    log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "Failed to resolve pending export {:#?}: {}", export, e);
                    return Err(e);
                }
            } // End match resolve_single_export
        } // End loop through pending_exports

        Ok(())
    }

    /// Processes pending re-exports (`pub use`) to:
    /// 1. Create `RelationKind::ReExports` between the `ImportNode` and the actual item being  re-exported.
    /// 2. Populate the `reexport_index` mapping the new public path to the ID of the re-exported item.
    ///
    /// Relies on `path_index` and `decl_index` being populated correctly beforehand.
    /// Returns an error if a re-export target cannot be resolved or if conflicting re-exports are found.
    #[cfg(feature = "reexport")]
    fn resolve_single_export(
        &self,
        export: &PendingExport,
        graph: &ParsedCodeGraph, // Needed for graph lookups during relative resolution
    ) -> Result<(SyntacticRelation, NodePath), ModuleTreeError> {
        // Changed return type
        let source_mod_id = export.containing_mod_id();
        let export_node = export.export_node();
        let export_node_id = export_node.id; // Get ImportNodeId

        // Always use the original source_path to find the target item
        let target_path_segments = export_node.source_path();

        if target_path_segments.is_empty() {
            return Err(ModuleTreeError::NodePathValidation(Box::new(
                SynParserError::NodeValidation("Empty export path".into()),
            )));
        }

        let first_segment = &target_path_segments[0];

        // --- Delegate ALL path resolution to resolve_path_relative_to ---
        let (base_module_id, segments_to_resolve) = if first_segment == "crate" {
            (self.root(), &target_path_segments[1..]) // Start from root, skip "crate"
        } else {
            (source_mod_id, target_path_segments) // Start from containing mod, use full path
        };

        // Check for external crate re-exports *before* attempting local resolution
        if base_module_id == self.root()
            && !segments_to_resolve.is_empty()
            && graph
                .iter_dependency_names()
                .any(|dep_name| dep_name == segments_to_resolve[0])
        {
            self.log_resolve_single_export_external(segments_to_resolve);
            // Return specific error for external re-exports that SPP might handle later
            return Err(ModuleTreeError::ExternalItemNotResolved(
                export_node_id.as_any(),
            )); // Use AnyNodeId
        }

        // Resolve the path, expecting AnyNodeId
        let target_any_id: AnyNodeId = self // Changed: Expect AnyNodeId
            .resolve_path_relative_to(
                base_module_id,
                segments_to_resolve,
                graph, // Pass graph access
            )
            .map_err(|e| {
                self.wrap_resolution_error(e, export_node_id.as_any(), target_path_segments)
            })?;

        // --- If target_any_id was found ---

        // Try to convert the resolved AnyNodeId to PrimaryNodeId, as required by ReExports relation
        let target_primary_id = PrimaryNodeId::try_from(target_any_id).map_err(|_| {
            // If conversion fails, it means the resolved item is not a primary node type
            // (e.g., it resolved to a Field or Variant, which cannot be directly re-exported this way).
            log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "Re-export target {} resolved to a non-primary node type ({:?}), which is invalid for ReExports relation.", target_any_id, target_any_id);
            // Explicitly handle NodePath conversion error within the closure
            match NodePath::try_from(target_path_segments.to_vec()) {
                Ok(path_for_error) => ModuleTreeError::UnresolvedReExportTarget {
                    path: path_for_error, // Provide path context
                    import_node_id: Some(export_node_id.as_any()), // Provide import node context
                },
                Err(e) => {
                    // If NodePath conversion fails, return that specific error
                    ModuleTreeError::NodePathValidation(Box::new(e))
                }
            }
        })?;

        // Create the SyntacticRelation::ReExports
        let relation = SyntacticRelation::ReExports {
            source: export_node_id,    // Source is ImportNodeId
            target: target_primary_id, // Target must be PrimaryNodeId
        };
        self.log_relation(relation, Some("resolve_single_export created relation")); // Use new log helper

        // Construct the public path using the visible_name
        let containing_module = self.get_module_checked(&source_mod_id)?;
        let mut public_path_vec = containing_module.path().clone();
        public_path_vec.push(export_node.visible_name.clone()); // Use the name it's exported AS
        let public_reexport_path = NodePath::try_from(public_path_vec)?;

        Ok((relation, public_reexport_path))
    }

    /// Helper to resolve a path relative to a starting module.
    /// This is a complex function mimicking Rust's name resolution.
    fn resolve_path_relative_to(
        &self,
        base_module_id: ModuleNodeId,
        path_segments: &[String],
        graph: &ParsedCodeGraph, // Need graph access
    ) -> Result<AnyNodeId, ModuleTreeError> {
        resolve_path_relative_to(self, base_module_id, path_segments, graph)
    }

    pub fn resolve_custom_path(&self, module_id: ModuleNodeId) -> Option<&PathBuf> {
        self.found_path_attrs.get(&module_id)
    }

    #[allow(dead_code)]
    fn get_reexport_name(&self, module_id: ModuleNodeId, item_id: ImportNodeId) -> Option<String> {
        // Changed: item_id is ImportNodeId
        self.pending_exports
            .as_deref() // Get Option<&[PendingExport]>
            .and_then(|exports| {
                // Iterate over the slice if Some
                exports
                    .iter()
                    .find(|exp| {
                        exp.containing_mod_id() == module_id && exp.export_node().id == item_id
                    })
                    .and_then(|exp| exp.export_node().source_path.last().cloned())
            })
    }

    #[allow(dead_code)]
    fn get_contained_mod(&self, child_id: ModuleNodeId) -> Result<&ModuleNode, ModuleTreeError> {
        let child_module_node =
            self.modules
                .get(&child_id)
                .ok_or(ModuleTreeError::ContainingModuleNotFound(
                    child_id.as_any(), // Use AnyNodeId in error
                ))?;
        Ok(child_module_node)
    }

    /// Finds the NodeId of the definition module corresponding to a declaration module ID.
    #[allow(dead_code)]
    fn find_definition_for_declaration(&self, decl_id: ModuleNodeId) -> Option<ModuleNodeId> {
        self.tree_relations
            .iter()
            .filter_map(|tr| tr.rel().resolves_to_defn(decl_id))
            .next()
    }

    /// Finds the parent `ModuleNodeId` of a given `ModuleNodeId`.
    ///
    /// Handles both inline modules (direct parent via `Contains`) and file-based modules
    /// (finds the declaration via `ResolvesToDefinition`/`CustomPath`, then finds the declaration's parent).
    pub(super) fn get_parent_module_id(&self, module_id: ModuleNodeId) -> Option<ModuleNodeId> {
        // First, try finding a direct 'Contains' relation targeting this module_id.
        // This covers inline modules and declarations contained directly.
        let direct_parent = self
            .get_iter_relations_to(&module_id.as_any()) // Use as_any()
            .and_then(|mut iter| {
                iter.find_map(|tr| match tr.rel() {
                    SyntacticRelation::Contains { source, target }
                        if target.as_any() == module_id.as_any() =>
                    // Use as_any()
                    {
                        Some(*source) // Source is already ModuleNodeId
                    }
                    _ => None,
                })
            });

        if direct_parent.is_some() {
            return direct_parent;
        }

        // If no direct parent found via Contains, check if `module_id` is a file-based definition.
        // If so, find its declaration and then the declaration's parent.
        self.get_iter_relations_to(&module_id.as_any()) // Use as_any()
            .and_then(|mut iter| {
                iter.find_map(|tr| match tr.rel() {
                    // Find the relation linking a declaration *to* this definition
                    SyntacticRelation::ResolvesToDefinition {
                        source: decl_id,
                        target,
                    }
                    | SyntacticRelation::CustomPath {
                        source: decl_id,
                        target,
                    } if *target == module_id => {
                        // Found the declaration ID (`decl_id`). Now find *its* parent.
                        self.get_iter_relations_to(&decl_id.as_any()) // Use as_any()
                            .and_then(|mut decl_iter| {
                                decl_iter.find_map(|decl_tr| match decl_tr.rel() {
                                    SyntacticRelation::Contains {
                                        source: parent_id,
                                        target: contains_target,
                                    } if contains_target.as_any() == decl_id.as_any() =>
                                    // Compare AnyNodeId representations
                                    {
                                        Some(*parent_id) // Parent is already ModuleNodeId
                                    }
                                    _ => None,
                                })
                            })
                    }
                    _ => None,
                })
            })
    }

    /// Determines the effective visibility of a module definition for access checks.
    ///
    /// For inline modules or the crate root, it's the visibility stored on the `ModuleNode` itself.
    /// For file-based modules (that are not the root), it's the visibility of the corresponding
    /// `mod name;` declaration statement found via `ResolvesToDefinition` or `CustomPath` relations.
    /// If the declaration cannot be found (e.g., unlinked module file), it defaults to the
    /// visibility stored on the definition node itself (which is typically `Inherited`).
    pub(super) fn get_effective_visibility(
        &self,
        module_def_id: ModuleNodeId,
    ) -> Option<&VisibilityKind> {
        get_effective_visibility(self, module_def_id)
    }

    /// Checks if the `target` module is accessible from the `source` module based on visibility rules.
    pub fn is_accessible(&self, source: ModuleNodeId, target: ModuleNodeId) -> bool {
        is_accessible(self, source, target)
    }

    // Proposed new function signature and implementation
    fn get_file_declaring_dir(&self, module_id: ModuleNodeId) -> Result<PathBuf, ModuleTreeError> {
        let mut current_id = module_id;

        while let Some(current_node) = self.modules.get(&current_id) {
            // Get the current node, returning error if not found in the tree

            // Check if the current node is file-based
            if let Some(file_path) = current_node.file_path() {
                // Found a file-based ancestor. Get its parent directory.
                return file_path
                    .parent()
                    .map(|p| p.to_path_buf())
                    // Return error if the file path has no parent (e.g., it's just "/")
                    .ok_or_else(|| ModuleTreeError::FilePathMissingParent(file_path.clone()));
            }

            // Check if we have reached the root *without* finding a file-based node
            if current_id == self.root {
                // This indicates an invalid state - the root must be file-based.
                return Err(ModuleTreeError::RootModuleNotFileBased(self.root));
            }

            // If not file-based and not the root, move up to the parent.
            // Return error if the parent link is missing (inconsistent tree).
            current_id = self.get_parent_module_id(current_id).ok_or_else(|| {
                self.log_find_decl_dir_missing_parent(current_id);
                ModuleTreeError::ContainingModuleNotFound(current_id.as_any()) // Use AnyNodeId in error
                                                                               // Re-use existing error
            })?;
        }
        Err(ModuleTreeError::ContainingModuleNotFound(
            current_id.as_any(), // Use AnyNodeId in error
        ))
    }
    /// Resolves a path string (either relative or absolute) relative to a base directory.
    ///
    /// # Arguments
    /// * `base_dir` - The base directory to resolve relative paths from
    /// * `relative_or_absolute` - The path string to resolve (can be relative like "../foo" or absolute "/bar")
    ///
    /// # Returns
    /// The resolved absolute PathBuf
    pub fn resolve_relative_path(
        base_dir: &std::path::Path,
        relative_or_absolute: &str,
    ) -> PathBuf {
        let path = PathBuf::from(relative_or_absolute);

        if path.is_absolute() {
            // If absolute, return as-is (normalized)
            path
        } else {
            // If relative, join with base_dir and normalize
            base_dir.join(path).normalize()
        }
    }

    pub fn resolve_path_for_module(
        &self,
        module_id: ModuleNodeId,
        path: &str,
    ) -> Result<PathBuf, ModuleTreeError> {
        let base_dir = self.get_file_declaring_dir(module_id)?;
        Ok(Self::resolve_relative_path(&base_dir, path))
    }

    pub(crate) fn process_path_attributes(&mut self) -> Result<(), ModuleTreeError> {
        let mut internal_relations = Vec::new();
        let mut external_path_files: Vec<(ModuleNodeId, PathBuf)> = Vec::new();

        for (decl_module_id, resolved_path) in self.found_path_attrs.iter() {
            let ctx = PathProcessingContext {
                module_id: *decl_module_id,
                module_name: "?", // Temporary placeholder
                attr_value: None,
                resolved_path: Some(resolved_path),
            };
            let decl_module_node = self.modules.get(decl_module_id).ok_or_else(|| {
                self.log_path_processing(&ctx, "Error", Some("Module not found"));
                ModuleTreeError::ContainingModuleNotFound(decl_module_id.as_any())
                // Use as_any()
            })?;

            // Update context with module info
            let ctx = PathProcessingContext {
                module_name: &decl_module_node.name,
                attr_value: extract_path_attr_from_node(decl_module_node),
                ..ctx
            };
            self.log_path_processing(&ctx, "Processing", None);

            let mut targets_iter = self.modules.values().filter(|m| {
                m.is_file_based() && m.file_path().is_some_and(|fp| fp == resolved_path)
            });
            let target_defn = targets_iter.next();

            match target_defn {
                Some(target_defn_node) => {
                    // 2. Found the target file definition node. Create the relation.
                    let target_defn_id = target_defn_node.id; // Get ModuleNodeId
                    let relation = SyntacticRelation::CustomPath {
                        source: *decl_module_id, // Use ModuleNodeId directly
                        target: target_defn_id,  // Use ModuleNodeId directly
                    };
                    self.log_relation(relation, None);
                    // NOTE: Edge Case
                    // It is actually valid to have a case of duplicate definitions. We'll
                    // need to consider how to handle this case, since it is possible to have an
                    // inline module with the `#[path]` attribute that contains items which shadow
                    // the items in the linked file, in which case the shadowed items are ignored.
                    // For now, just throw error.
                    if let Some(dup) = targets_iter.next() {
                        // Use ModuleNodeId directly for display, as it implements Display
                        return Err(ModuleTreeError::DuplicateDefinition(format!(
                        "Duplicate module definition for path attribute target '{}' {}:\ndeclaration: {:#?}\nfirst: {:#?},\nsecond: {:#?}",
                            decl_module_node.id,
                        resolved_path.display(),
                            &decl_module_node,
                            &target_defn_node, // Use the found node
                            &dup
                    )));
                    }
                    internal_relations.push(relation); // Push SyntacticRelation
                }
                None => {
                    // 3. Handle case where the target file node wasn't found.
                    // This indicates an inconsistency - the path resolved, but thecorresponding
                    // module node isn't in the map.
                    // Either the file is outside the target directory, and we return a warning,
                    // since we don't want to do a second parse, or if the path is inside the
                    // directory, we abort the process of resolving the tree because the parsed
                    // files are inconsistent.

                    // Determine the crate's src directory
                    let src_dir = match self.root_file.parent() {
                        Some(dir) => dir,
                        None => {
                            // Should be rare, but handle if root file has no parent
                            self.log_module_error(
                                *decl_module_id,
                                &format!(
                                    "Could not determine src directory from root file: {}",
                                    self.root_file.display()
                                ),
                            );
                            return Err(ModuleTreeError::FilePathMissingParent(
                                self.root_file.clone(),
                            ));
                        }
                    };

                    // Check if the resolved path is outside the src directory
                    if !resolved_path.starts_with(src_dir) {
                        // External path target not found - Log a warning.
                        // This is not an error that will result in an invalid state, only a
                        // slightly pruned one. Unavoidable without having complete access to the
                        // user's filesystem, which we don't want for security reasons.
                        self.log_path_attr_external_not_found(*decl_module_id, resolved_path);
                        external_path_files.push((*decl_module_id, resolved_path.clone()))
                    } else {
                        // Path is inside src, but module node not found - this is an error
                        self.log_module_error(
                            *decl_module_id,
                            &format!(
                                "Path attribute target file not found in modules map: {}",
                                resolved_path.display(),
                            ),
                        );
                        return Err(ModuleTreeError::ModuleKindinitionNotFound(format!(
                            "Module definition for path attribute target '{}' not found for declaration {}:\n{:#?}",
                            resolved_path.display(),
                            decl_module_node.id,
                            &decl_module_node,
                        )));
                    }
                }
            }
        }
        self.external_path_attrs.extend(external_path_files);
        // Add relations one by one using add_rel which takes TreeRelation
        for rel in internal_relations {
            self.add_rel(rel.into());
        }
        Ok(())
    }

    /// Updates the `path_index` to use canonical paths for modules affected by `#[path]` attributes.
    ///
    /// This function iterates through modules that had a `#[path]` attribute (identified via
    /// `found_path_attrs`). For each such module declaration, it finds the corresponding
    /// definition module (linked by `RelationKind::CustomPath`). It then:
    /// 1. Removes the `path_index` entry based on the definition module's original file-system path.
    /// 2. Inserts a new `path_index` entry using the declaration module's canonical path as the key
    ///    and the definition module's ID as the value.
    ///
    /// This ensures that lookups in `path_index` using canonical paths will correctly resolve
    /// to the definition module, even when `#[path]` is used.
    ///
    /// This function assumes it's called after `process_path_attributes` has run and created
    /// the necessary `CustomPath` relations. It does *not* yet handle updating paths for items
    /// *contained within* the modules affected by `#[path]`.
    pub(crate) fn update_path_index_for_custom_paths(&mut self) -> Result<(), ModuleTreeError> {
        #[cfg(feature = "validate")]
        assert!(self.validate_unique_rels());
        self.log_update_path_index_entry_exit(true);
        // Collect keys to avoid borrowing issues while modifying the map inside the loop.
        // We iterate based on the declarations found to have path attributes.
        let decl_ids_with_path_attrs: Vec<ModuleNodeId> =
            self.found_path_attrs.keys().copied().collect();

        if decl_ids_with_path_attrs.is_empty() {
            self.log_update_path_index_status(None);
            self.log_update_path_index_entry_exit(false);
            return Ok(());
        }

        self.log_update_path_index_status(Some(decl_ids_with_path_attrs.len()));

        for decl_mod_id in decl_ids_with_path_attrs {
            self.log_update_path_index_processing(decl_mod_id);

            // 1. Find the definition module ID using the CustomPath relation
            let def_mod_id = match self.find_custom_path_target(decl_mod_id) {
                Ok(id) => id,
                Err(e) => {
                    self.log_update_path_index_target_error(decl_mod_id, &e);
                    if self.external_path_attrs.contains_key(&decl_mod_id) {
                        // Log a warning using the helper method, but continue processing.
                        self.log_update_path_index_skip_external(decl_mod_id);
                        continue; // Skip to the next decl_mod_id
                    } else {
                        // Log the fatal error using the helper method and abort.
                        self.log_update_path_index_abort_inconsistent(decl_mod_id);
                        return Err(e);
                    }
                }
            };
            self.log_update_path_index_found_target(decl_mod_id, def_mod_id);

            // 2. Get paths (convert Vec<String> to NodePath)
            // We need the nodes temporarily to get their paths, but avoid cloning them.
            let canonical_path = {
                let decl_mod = self.get_module_checked(&decl_mod_id)?;
                NodePath::try_from(decl_mod.path.clone())? // Clone path Vec, not whole node
            };
            let original_path = {
                let def_mod = self.get_module_checked(&def_mod_id)?;
                NodePath::try_from(def_mod.path.clone())? // Clone path Vec, not whole node
            };

            // 3. Log using IDs
            self.log_update_path_index_paths(
                decl_mod_id,
                def_mod_id,
                &canonical_path,
                &original_path,
            );

            // 4. Remove the old path index entry for the definition module
            // Use the original_path (derived from file system) as the key to remove.
            let def_mod_any_id = def_mod_id.as_any(); // Get AnyNodeId
            if let Some(removed_id) = self.path_index.remove(&original_path) {
                let rem_id = removed_id.try_into()?; // This is AnyNodeId
                if removed_id != def_mod_any_id {
                    // Compare AnyNodeId
                    self.log_update_path_index_remove_inconsistency(
                        rem_id, // This is AnyNodeId
                        &original_path,
                        def_mod_id,
                    );
                    return Err(ModuleTreeError::InternalState(format!("Path index inconsistency during removal for path {}: expected {}, found {}. This suggests the path_index was corrupted earlier.", original_path, def_mod_any_id, removed_id)));
                }
                self.log_update_path_index_remove(&original_path, def_mod_id);
            } else {
                self.log_update_path_index_remove_missing(&original_path, def_mod_id);
            }

            // 5. Insert the new path index entry using the canonical path
            // Use the canonical_path (from the declaration) as the key, mapping to the definition ID (as AnyNodeId).
            if let Some(existing_id) = self
                .path_index
                .insert(canonical_path.clone(), def_mod_any_id)
            // Insert AnyNodeId
            {
                if existing_id != def_mod_any_id {
                    // Compare AnyNodeId
                    let ex_id = existing_id.try_into()?;
                    self.log_update_path_index_insert_conflict(
                        &canonical_path,
                        def_mod_id,
                        ex_id, // This is AnyNodeId
                    );
                    return Err(ModuleTreeError::DuplicatePath {
                        path: canonical_path,
                        existing_id,                    // AnyNodeId
                        conflicting_id: def_mod_any_id, // AnyNodeId
                    });
                }
                self.log_update_path_index_reinsert(&canonical_path, def_mod_id);
            } else {
                self.log_update_path_index_insert(&canonical_path, def_mod_id);
            }
        }

        self.log_update_path_index_entry_exit(false);

        #[cfg(feature = "validate")]
        assert!(self.validate_unique_rels());
        Ok(())
    }

    // Helper function to find the target of a CustomPath relation
    // (Ensure this exists or add it if it doesn't)
    fn find_custom_path_target(
        &self,
        decl_mod_id: ModuleNodeId,
    ) -> Result<ModuleNodeId, ModuleTreeError> {
        self.relations_by_source
         .get(&decl_mod_id.into()) // Changed: Use .into() to get AnyNodeId for lookup
         .and_then(|indices| {
             indices.iter().find_map(|&index| {
                 // Use .get() for safe access and .rel() to get inner Relation
                 let relation = self.tree_relations.get(index)?.rel();
                 // Match on the specific variant to get the correctly typed target
                 match relation {
                     SyntacticRelation::CustomPath { target, .. } => Some(*target), // Target is already ModuleNodeId
                     _ => None,
                 }
             })
         })
         .ok_or_else(|| {
             log::error!(target: LOG_TARGET_MOD_TREE_BUILD, "CustomPath relation target not found for declaration module {}", decl_mod_id);
             // Use a more specific error if available, otherwise adapt ModuleKindinitionNotFound
             ModuleTreeError::ModuleKindinitionNotFound(format!(
                 "Definition module for declaration {} (via CustomPath relation) not found",
                 decl_mod_id
             ))
         })
    }

    /// Removes unlinked file-based modules and their contained items from the tree state.
    ///
    /// This should be called after linking (`link_mods_syntactic`) and path attribute
    /// processing (`process_path_attributes`, `update_path_index_for_custom_paths`)
    /// are complete.
    ///
    /// It identifies file-based modules (excluding the root) that are not targeted by
    /// either a `ResolvesToDefinition` or `CustomPath` relation. These modules, along
    /// with the items they contain, are removed from the `ModuleTree`'s state
    /// (`modules`, `path_index`, `tree_relations`). The relation indices are rebuilt after pruning.
    ///
    /// Note: This function currently does *not* modify the underlying `ParsedCodeGraph`.
    ///
    /// # Returns
    /// A `Result` containing `PruningResult` which lists the IDs of the pruned modules,
    /// all contained items, and the relations that were removed, or a `ModuleTreeError`
    /// if an issue occurs during processing.
    pub(crate) fn prune_unlinked_file_modules(&mut self) -> Result<PruningResult, ModuleTreeError> {
        #[cfg(feature = "validate")]
        let _rels_before = self.validate_unique_rels(); // Store result to avoid unused warning

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Starting pruning of unlinked file modules from ModuleTree...", "Begin".log_header());

        // --- Step 1: Initial Identification of Unlinked File Modules ---
        let mut initial_prunable_module_ids: HashSet<ModuleNodeId> = HashSet::new();
        let root_any_id = self.root.as_any(); // Get AnyNodeId for comparison

        // for debugging
        let root_module = self.get_module_checked(&self.root)?;
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Root module identified: {} ({}) path: {}", root_module.name.log_name(), root_module.id.to_string().log_id(), root_module.path().join("::").log_path());

        // 1. Identify prunable module IDs
        for (mod_id, module_node) in self.modules.iter() {
            // Skip root and non-file-based modules
            if module_node.id.as_any() == root_any_id || !module_node.is_file_based() {
                continue;
            }

            // Check for incoming ResolvesToDefinition or CustomPath relations using get_relations_to with AnyNodeId
            let is_linked = self
                .get_iter_relations_to(&mod_id.as_any()) // Use AnyNodeId
                .is_some_and(|mut iter| {
                    iter.any(|tr| {
                        matches!(
                            tr.rel(), // Use rel() to get SyntacticRelation
                            SyntacticRelation::ResolvesToDefinition { .. }
                                | SyntacticRelation::CustomPath { .. }
                        )
                    })
                });

            if !is_linked {
                debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Marking initially unlinked module for pruning: {} ({})", module_node.name.log_name(), mod_id.to_string().log_id());
                initial_prunable_module_ids.insert(*mod_id);
            }
        }

        if initial_prunable_module_ids.is_empty() {
            debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} No unlinked modules found to prune.", "Info".log_comment());
            return Ok(PruningResult::default()); // Return empty PruningResult
        }
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Identified {} initially unlinked modules.", initial_prunable_module_ids.len().to_string().log_id());

        // --- Step 2: Recursively Collect All Items Defined Within Unlinked Files ---
        // Changed: Use AnyNodeId for the set and queue
        let mut all_prunable_item_ids: HashSet<AnyNodeId> = initial_prunable_module_ids
            .iter()
            .map(|id| id.as_any()) // Convert ModuleNodeId to AnyNodeId
            .collect();
        // Queue for BFS traversal, starting with the initial unlinked module IDs as AnyNodeId
        let mut queue: VecDeque<AnyNodeId> = all_prunable_item_ids.iter().copied().collect();

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Recursively finding items contained within unlinked modules...");
        while let Some(current_any_id) = queue.pop_front() {
            // Find items directly contained by the current node using AnyNodeId
            if let Some(contained_relations) = self.get_iter_relations_from(&current_any_id) {
                for rel in contained_relations {
                    // Get target AnyNodeId using the helper method
                    let target_any_id = rel.rel().target();
                    // If this target item is newly discovered as prunable...
                    if all_prunable_item_ids.insert(target_any_id) {
                        // ...add it to the queue to explore its contents
                        // (Important if it's an inline module defined within the unlinked file)
                        queue.push_back(target_any_id);
                    }
                }
            }
        }
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Collected {} total item IDs (including modules) to prune via containment.", all_prunable_item_ids.len().to_string().log_id());

        // --- Step 3: Identify Relations to Prune ---
        let mut relations_to_prune_indices = HashSet::new();
        for (index, tr) in self.tree_relations.iter().enumerate() {
            // Get source and target AnyNodeIds using helper methods
            let source_any_id = tr.rel().source();
            let target_any_id = tr.rel().target();

            // Check if either source or target is in the set of prunable AnyNodeIds
            let source_is_pruned = all_prunable_item_ids.contains(&source_any_id);
            let target_is_pruned = all_prunable_item_ids.contains(&target_any_id);

            if source_is_pruned || target_is_pruned {
                relations_to_prune_indices.insert(index);
            }
        }
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Identified {} relations to prune.", relations_to_prune_indices.len());

        // --- Step 4: Prune ModuleTree state ---
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Pruning ModuleTree structures...");
        // Prune modules based on the initial set of unlinked module IDs
        let modules_before = self.modules.len();
        self.modules
            .retain(|id, _| !initial_prunable_module_ids.contains(id));
        let modules_after = self.modules.len();
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Pruned modules map: {} -> {} (removed {})", modules_before, modules_after, modules_before - modules_after);

        // Prune path_index: Remove entries whose VALUE (AnyNodeId) is in the full prunable item set
        let path_index_before = self.path_index.len();
        self.path_index
            .retain(|_path, any_node_id| !all_prunable_item_ids.contains(any_node_id)); // Use AnyNodeId directly
        let path_index_after = self.path_index.len();
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Pruned path_index: {} -> {} (removed {})", path_index_before, path_index_after, path_index_before - path_index_after);

        // Prune decl_index: Remove entries whose VALUE (ModuleNodeId) corresponds to an AnyNodeId in the prunable set
        let decl_index_before = self.decl_index.len();
        self.decl_index.retain(|_path, module_node_id| {
            !all_prunable_item_ids.contains(&module_node_id.as_any())
        }); // Convert to AnyNodeId for check
        let decl_index_after = self.decl_index.len();
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Pruned decl_index: {} -> {} (removed {})", decl_index_before, decl_index_after, decl_index_before - decl_index_after);

        // Collect the actual relations being pruned before modifying tree_relations
        let pruned_relations: Vec<TreeRelation> = relations_to_prune_indices
            .iter()
            .filter_map(|&index| self.tree_relations.get(index).copied()) // Copy the TreeRelation
            .collect();

        // Filter tree_relations: Keep only those whose index is NOT in relations_to_prune_indices
        let original_relation_count = self.tree_relations.len();
        let mut i = 0;
        self.tree_relations.retain(|_| {
            let keep = !relations_to_prune_indices.contains(&i);
            i += 1;
            keep
        });
        let removed_relation_count = original_relation_count - self.tree_relations.len();
        // Sanity check
        assert_eq!(
            removed_relation_count,
            pruned_relations.len(),
            "Mismatch between counted pruned relations and collected pruned relations"
        );
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Removed {} relations involving pruned items.", removed_relation_count);

        // --- Step 5: Rebuild relation indices ---
        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "  Rebuilding relation indices...");
        self.relations_by_source.clear();
        self.relations_by_target.clear();
        // Iterate over the *filtered* tree_relations
        for (index, tr) in self.tree_relations.iter().enumerate() {
            // Use AnyNodeId keys for indices by calling helper methods
            self.relations_by_source
                .entry(tr.rel().source()) // Use AnyNodeId directly
                .or_default()
                .push(index);
            self.relations_by_target
                .entry(tr.rel().target()) // Use AnyNodeId directly
                .or_default()
                .push(index);
        }

        // --- Step 6: Prepare return data ---
        let result_data = PruningResult {
            pruned_module_ids: initial_prunable_module_ids, // Return the IDs of the modules that triggered pruning
            pruned_item_ids: all_prunable_item_ids, // Return all items that were pruned as a result (as AnyNodeId)
            pruned_relations,                       // Return the actual relations removed
        };

        debug!(target: LOG_TARGET_MOD_TREE_BUILD, "{} Finished pruning ModuleTree state. Pruned {} modules, {} total items, and {} relations.", "End".log_header(), result_data.pruned_module_ids.len(), result_data.pruned_item_ids.len(), result_data.pruned_relations.len());

        #[cfg(feature = "validate")]
        assert!(self.validate_unique_rels());
        Ok(result_data)
    }

    pub(crate) fn validate_unique_rels(&self) -> bool {
        let rels = &self.tree_relations();
        let unique_rels = rels.iter().fold(Vec::new(), |mut acc, rel| {
            if !acc.contains(rel) {
                acc.push(*rel);
            } else {
                self.log_relation_verbose(*rel);
            }
            acc
        });
        unique_rels.len() == rels.len()
    }
}
// Extension trait for Path normalization
trait PathNormalize {
    fn normalize(&self) -> PathBuf;
}

impl PathNormalize for std::path::Path {
    fn normalize(&self) -> PathBuf {
        let mut components = Vec::new();

        for component in self.components() {
            match component {
                std::path::Component::ParentDir => {
                    if components
                        .last()
                        .map(|c| c != &std::path::Component::RootDir)
                        .unwrap_or(false)
                    {
                        components.pop();
                    }
                }
                std::path::Component::CurDir => continue,
                _ => components.push(component),
            }
        }

        components.iter().collect()
    }
}
