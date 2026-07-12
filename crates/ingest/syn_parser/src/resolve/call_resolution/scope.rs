use std::collections::HashSet;

use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AnyNodeId, AsAnyNodeId, CallBodyOwnerId, FunctionNodeId, ImportKind, ImportNode,
            ImportNodeId, ModuleNodeId, OrdinaryTypeTargetId, OrdinaryTypeUseId, TypeAliasNodeId,
        },
        relations::{SyntacticRelation, TypeRelation},
        types::TypeNode,
    },
    resolve::RelationIndexer,
};

use super::{
    CallRelationResolver, LocalFunctionPathResolution, LocalModulePathResolution,
    LocalTypeResolution, MAX_IMPORT_CHAIN_DEPTH, WorkspaceTypeResolution, WorkspaceTypeTarget,
    dependency_name_matches,
};

impl<'a> CallRelationResolver<'a> {
    pub(super) fn is_unqualified_path(&self, path: &[String]) -> bool {
        path.len() == 1
    }

    pub(super) fn is_explicit_local_path(&self, path: &[String]) -> bool {
        path.len() > 1
            && matches!(
                path.first().map(String::as_str),
                Some("crate" | "self" | "super")
            )
    }

    pub(super) fn is_external_path(&self, path: &[String]) -> bool {
        path.iter()
            .find(|segment| !segment.is_empty())
            .is_some_and(|segment| {
                matches!(segment.as_str(), "std" | "core" | "alloc")
                    || self.graph.iter_dependency_names().any(|dependency| {
                        dependency == segment || dependency.replace('-', "_") == *segment
                    })
            })
    }

    fn is_workspace_dependency_path(&self, path: &[String]) -> bool {
        self.workspace_dependency_tail(path).is_some()
    }

    pub(super) fn is_external_prelude_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<bool, SynParserError> {
        match path {
            [segment] if segment == "drop" => Ok(true),
            [segment, method]
                if matches!(segment.as_str(), "Box" | "String" | "Vec") && method == "new" =>
            {
                Ok(!self.local_segment_visible(owner, segment)?)
            }
            _ => Ok(false),
        }
    }

    pub(super) fn local_segment_visible(
        &self,
        owner: CallBodyOwnerId,
        segment: &str,
    ) -> Result<bool, SynParserError> {
        let Some(module_id) = self.containing_module_for_owner(owner) else {
            return Ok(true);
        };
        let module_id = self.import_scope_module(module_id)?;
        let mut visible = false;
        self.visit_scope_candidates(module_id, segment, &mut |_| {
            visible = true;
            Ok(())
        })?;
        Ok(visible)
    }

    pub(super) fn is_external_import_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<bool, SynParserError> {
        let Some(segment) = path.first().map(String::as_str) else {
            return Ok(false);
        };
        if matches!(segment, "crate" | "self" | "super") {
            return Ok(false);
        }
        let Some(module_id) = self.containing_module_for_owner(owner) else {
            return Ok(false);
        };
        let module_id = self.import_scope_module(module_id)?;
        let Some(module_node) = self
            .graph
            .modules()
            .iter()
            .find(|module| module.id == module_id)
        else {
            return Ok(false);
        };

        let mut saw_external = false;
        let mut saw_non_external = false;
        for import_node in &module_node.imports {
            if import_node.visible_name != segment {
                continue;
            }
            if import_node.is_glob {
                continue;
            }

            if self.import_binding_is_external(import_node.id, 0)? {
                saw_external = true;
            } else {
                saw_non_external = true;
            }
        }

        Ok(saw_external && !saw_non_external)
    }

    pub(super) fn import_scope_module(
        &self,
        module_id: ModuleNodeId,
    ) -> Result<ModuleNodeId, SynParserError> {
        let mut targets = self
            .tree
            .get_iter_relations_from(&module_id.as_any())
            .into_iter()
            .flatten()
            .filter_map(|relation| match relation.rel() {
                SyntacticRelation::ResolvesToDefinition { source, target }
                | SyntacticRelation::CustomPath { source, target }
                    if *source == module_id =>
                {
                    Some(*target)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();

        match targets.as_slice() {
            [] => Ok(module_id),
            [target] => Ok(*target),
            _ => Err(SynParserError::InternalState(format!(
                "call resolution found multiple import-scope module targets for {module_id}"
            ))),
        }
    }

    fn import_binding_is_external(
        &self,
        import_id: ImportNodeId,
        depth: usize,
    ) -> Result<bool, SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {}",
                import_id.as_any()
            )));
        }

        let import_node = self.graph.get_import_checked(import_id)?;
        if matches!(import_node.kind, ImportKind::ExternFunction { .. }) {
            return Ok(true);
        }
        if self.is_workspace_dependency_path(import_node.source_path()) {
            return Ok(false);
        }
        if self.is_external_path(import_node.source_path()) {
            return Ok(true);
        }

        let mut had_sources = false;
        let mut all_sources_external = true;
        for relation in self.tree.get_iter_relations_to(&import_id.as_any()) {
            let SyntacticRelation::ImportedBy { source, target } = relation.rel() else {
                continue;
            };
            if *target != import_id {
                continue;
            }
            had_sources = true;
            if let Ok(source_import) = ImportNodeId::try_from(source.as_any()) {
                if !self.import_binding_is_external(source_import, depth + 1)? {
                    all_sources_external = false;
                }
                continue;
            }

            let Ok(source_alias) = TypeAliasNodeId::try_from(source.as_any()) else {
                all_sources_external = false;
                continue;
            };
            if !self.type_alias_binding_is_external(source_alias, depth + 1)? {
                all_sources_external = false;
            }
        }

        if had_sources {
            return Ok(all_sources_external);
        }

        Ok(false)
    }

    fn type_alias_binding_is_external(
        &self,
        alias_id: TypeAliasNodeId,
        depth: usize,
    ) -> Result<bool, SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {}",
                alias_id.as_any()
            )));
        }

        let alias_node = self.graph.get_type_alias_checked(alias_id)?;
        self.ordinary_type_binding_is_external(alias_node.type_id, depth + 1)
    }

    fn ordinary_type_binding_is_external(
        &self,
        type_id: OrdinaryTypeUseId,
        depth: usize,
    ) -> Result<bool, SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {type_id:?}"
            )));
        }

        match self.type_node(type_id)? {
            TypeNode::Named(node) => {
                Ok(!self.is_workspace_dependency_path(&node.path)
                    && self.is_external_path(&node.path))
            }
            TypeNode::Reference(node) => {
                self.ordinary_type_binding_is_external(node.referenced, depth + 1)
            }
            TypeNode::Paren(node) => self.ordinary_type_binding_is_external(node.inner, depth + 1),
            _ => Ok(false),
        }
    }

    pub(super) fn ordinary_type_target_alias_is_external(
        &self,
        target: OrdinaryTypeTargetId,
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        for receiver in self.ordinary_receiver_targets(target, type_relations)? {
            let Ok(alias_id) = TypeAliasNodeId::try_from(receiver) else {
                continue;
            };
            if self.type_alias_binding_is_external(alias_id, 0)? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub(super) fn external_type_path(
        &self,
        module_id: ModuleNodeId,
        type_id: OrdinaryTypeUseId,
    ) -> Result<Option<Vec<String>>, SynParserError> {
        let module_id = self.import_scope_module(module_id)?;
        self.external_path_at(module_id, type_id, 0)
    }

    fn external_path_at(
        &self,
        module_id: ModuleNodeId,
        type_id: OrdinaryTypeUseId,
        depth: usize,
    ) -> Result<Option<Vec<String>>, SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} while resolving external type path for {type_id:?}"
            )));
        }

        match self.type_node(type_id)? {
            TypeNode::Named(node) => self.external_named_path(module_id, &node.path, depth + 1),
            TypeNode::Reference(node) => {
                self.external_path_at(module_id, node.referenced, depth + 1)
            }
            TypeNode::Paren(node) => self.external_path_at(module_id, node.inner, depth + 1),
            _ => Ok(None),
        }
    }

    fn external_named_path(
        &self,
        module_id: ModuleNodeId,
        path: &[String],
        depth: usize,
    ) -> Result<Option<Vec<String>>, SynParserError> {
        if path.is_empty() || self.is_workspace_dependency_path(path) {
            return Ok(None);
        }
        if self.is_external_path(path) {
            return Ok(Some(path.to_vec()));
        }

        let [segment] = path else {
            return Ok(None);
        };
        let mut paths = self.segment_external_paths(module_id, segment, depth + 1)?;
        paths.sort();
        paths.dedup();

        Ok(match paths.as_slice() {
            [path] => Some(path.clone()),
            [] | [_, ..] => None,
        })
    }

    pub(super) fn segment_external_paths(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
        depth: usize,
    ) -> Result<Vec<Vec<String>>, SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} while resolving external import `{segment}`"
            )));
        }

        let Some(module_node) = self
            .graph
            .modules()
            .iter()
            .find(|module| module.id == module_id)
        else {
            return Ok(Vec::new());
        };

        let mut paths = Vec::new();
        for import_node in &module_node.imports {
            if import_node.is_glob {
                paths.extend(self.ancestor_external_paths(import_node, segment, depth + 1)?);
            } else if import_node.visible_name == segment {
                paths.extend(self.import_external_paths(import_node.id, depth + 1)?);
            }
        }
        if paths.is_empty() {
            paths.extend(self.equivalent_module_external_paths(module_id, segment, depth + 1)?);
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    fn equivalent_module_external_paths(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
        depth: usize,
    ) -> Result<Vec<Vec<String>>, SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} while resolving external import `{segment}` through equivalent modules"
            )));
        }

        let Some(module_node) = self
            .graph
            .modules()
            .iter()
            .find(|module| module.id == module_id)
        else {
            return Ok(Vec::new());
        };

        let mut paths = Vec::new();
        for candidate in self
            .graph
            .modules()
            .iter()
            .filter(|candidate| candidate.id != module_id && candidate.path == module_node.path)
        {
            for import_node in &candidate.imports {
                if import_node.visible_name == segment && !import_node.is_glob {
                    paths.extend(self.import_external_paths(import_node.id, depth + 1)?);
                }
            }
        }
        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    fn ancestor_external_paths(
        &self,
        import_node: &ImportNode,
        segment: &str,
        depth: usize,
    ) -> Result<Vec<Vec<String>>, SynParserError> {
        let Some(module_id) =
            self.ancestor_glob_module(import_node, "resolving external glob import")?
        else {
            return Ok(Vec::new());
        };

        let module_id = self.import_scope_module(module_id)?;
        self.segment_external_paths(module_id, segment, depth + 1)
    }

    fn import_external_paths(
        &self,
        import_id: ImportNodeId,
        depth: usize,
    ) -> Result<Vec<Vec<String>>, SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {}",
                import_id.as_any()
            )));
        }

        let import_node = self.graph.get_import_checked(import_id)?;
        if !self.is_workspace_dependency_path(import_node.source_path())
            && self.is_external_path(import_node.source_path())
        {
            return Ok(vec![import_node.source_path().to_vec()]);
        }

        let mut paths = Vec::new();
        for relation in self.tree.get_iter_relations_to(&import_id.as_any()) {
            let SyntacticRelation::ImportedBy { source, target } = relation.rel() else {
                continue;
            };
            if *target != import_id {
                continue;
            }

            if let Ok(source_import) = ImportNodeId::try_from(source.as_any()) {
                paths.extend(self.import_external_paths(source_import, depth + 1)?);
                continue;
            }

            let Ok(source_alias) = TypeAliasNodeId::try_from(source.as_any()) else {
                continue;
            };
            let alias_node = self.graph.get_type_alias_checked(source_alias)?;
            let Some(module_id) = self.containing_module(source_alias.as_any()) else {
                continue;
            };
            if let Some(path) = self.external_type_path(module_id, alias_node.type_id)? {
                paths.push(path);
            }
        }

        paths.sort();
        paths.dedup();
        Ok(paths)
    }

    pub(super) fn resolve_unqualified_local_function_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<LocalFunctionPathResolution, SynParserError> {
        self.resolve_implicit_local_function_path(owner, path)
    }

    pub(super) fn resolve_implicit_local_function_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<LocalFunctionPathResolution, SynParserError> {
        match self.resolve_local_function_path(owner, path)? {
            LocalFunctionPathResolution::Unresolved => Ok(LocalFunctionPathResolution::Unsupported),
            resolution => Ok(resolution),
        }
    }

    pub(super) fn resolve_workspace_type_import(
        &self,
        owner: CallBodyOwnerId,
        segment: &str,
    ) -> Result<WorkspaceTypeResolution<'a>, SynParserError> {
        if self.workspace.is_none() {
            return Ok(WorkspaceTypeResolution::Unresolved);
        }

        let Some(module_id) = self.containing_module_for_owner(owner) else {
            return Ok(WorkspaceTypeResolution::Unresolved);
        };
        let module_id = self.import_scope_module(module_id)?;
        self.resolve_workspace_type_in_module(module_id, segment)
    }

    pub(super) fn resolve_workspace_type_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<WorkspaceTypeResolution<'a>, SynParserError> {
        let Some(mut module_id) = self.containing_module_for_owner(owner) else {
            return Ok(WorkspaceTypeResolution::Unresolved);
        };
        module_id = self.import_scope_module(module_id)?;
        self.resolve_workspace_type_path_from_module(module_id, path)
    }

    fn resolve_workspace_type_path_from_module(
        &self,
        mut current_module: ModuleNodeId,
        path: &[String],
    ) -> Result<WorkspaceTypeResolution<'a>, SynParserError> {
        let start_idx = self.start_segment_index(path, &mut current_module)?;
        if start_idx >= path.len() {
            return Ok(WorkspaceTypeResolution::Unresolved);
        }

        for idx in start_idx..path.len() {
            let segment = path[idx].as_str();
            let is_last = idx == path.len() - 1;
            if is_last {
                return self.resolve_workspace_type_in_module(current_module, segment);
            }

            current_module = match self.resolve_module_segment(current_module, segment)? {
                LocalModulePathResolution::Resolved(module_id) => module_id,
                LocalModulePathResolution::Unresolved => {
                    return Ok(WorkspaceTypeResolution::Unresolved);
                }
                LocalModulePathResolution::Ambiguous => {
                    return Ok(WorkspaceTypeResolution::Ambiguous);
                }
            };
        }

        Ok(WorkspaceTypeResolution::Unresolved)
    }

    fn resolve_workspace_type_in_module(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
    ) -> Result<WorkspaceTypeResolution<'a>, SynParserError> {
        let mut candidates = Vec::new();
        let mut saw_ambiguous = false;
        self.collect_workspace_type_candidates_in_module(
            module_id,
            segment,
            &mut candidates,
            &mut saw_ambiguous,
            0,
        )?;

        candidates.sort_by_key(|candidate| candidate.target);
        candidates.dedup_by_key(|candidate| candidate.target);

        if saw_ambiguous {
            return Ok(WorkspaceTypeResolution::Ambiguous);
        }

        Ok(match candidates.as_slice() {
            [candidate] => WorkspaceTypeResolution::Resolved(*candidate),
            [] => WorkspaceTypeResolution::Unresolved,
            _ => WorkspaceTypeResolution::Ambiguous,
        })
    }

    fn collect_workspace_type_candidates_in_module(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
        candidates: &mut Vec<WorkspaceTypeTarget<'a>>,
        saw_ambiguous: &mut bool,
        depth: usize,
    ) -> Result<(), SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} while resolving workspace type `{segment}`"
            )));
        }

        let Some(module_node) = self
            .graph
            .modules()
            .iter()
            .find(|module| module.id == module_id)
        else {
            return Ok(());
        };

        for import_node in &module_node.imports {
            if import_node.is_glob {
                self.collect_workspace_glob_type_candidates(
                    import_node,
                    segment,
                    candidates,
                    saw_ambiguous,
                    depth + 1,
                )?;
            } else if import_node.visible_name == segment {
                self.collect_workspace_direct_type_candidates(
                    import_node,
                    candidates,
                    saw_ambiguous,
                    depth + 1,
                )?;
            }
        }
        Ok(())
    }

    fn collect_workspace_direct_type_candidates(
        &self,
        import_node: &ImportNode,
        candidates: &mut Vec<WorkspaceTypeTarget<'a>>,
        saw_ambiguous: &mut bool,
        depth: usize,
    ) -> Result<(), SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {}",
                import_node.id.as_any()
            )));
        }

        if let Some((krate, tail)) = self.workspace_dependency_tail(import_node.source_path()) {
            let resolver = CallRelationResolver::new(krate.graph, krate.tree);
            match resolver.resolve_type_path_from_root(tail)? {
                LocalTypeResolution::Resolved(target) => {
                    candidates.push(WorkspaceTypeTarget { krate, target });
                }
                LocalTypeResolution::Unresolved => {}
                LocalTypeResolution::Ambiguous => {
                    *saw_ambiguous = true;
                }
            }
            return Ok(());
        }

        let Some(mut module_id) = self.containing_module(import_node.id.as_any()) else {
            return Ok(());
        };
        module_id = self.import_scope_module(module_id)?;
        match self.resolve_workspace_type_path_from_module(module_id, import_node.source_path())? {
            WorkspaceTypeResolution::Resolved(candidate) => candidates.push(candidate),
            WorkspaceTypeResolution::Unresolved => {}
            WorkspaceTypeResolution::Ambiguous => *saw_ambiguous = true,
        }
        Ok(())
    }

    fn collect_workspace_dependency_glob_type_candidates(
        &self,
        import_node: &ImportNode,
        segment: &str,
        candidates: &mut Vec<WorkspaceTypeTarget<'a>>,
        saw_ambiguous: &mut bool,
    ) -> Result<(), SynParserError> {
        let Some((krate, tail)) = self.workspace_dependency_tail(import_node.source_path()) else {
            return Ok(());
        };
        let resolver = CallRelationResolver::new(krate.graph, krate.tree);
        let module_id = match resolver.resolve_module_path_from_root(tail)? {
            LocalModulePathResolution::Resolved(module_id) => module_id,
            LocalModulePathResolution::Unresolved => return Ok(()),
            LocalModulePathResolution::Ambiguous => {
                *saw_ambiguous = true;
                return Ok(());
            }
        };

        resolver.visit_scope_candidates(module_id, segment, &mut |candidate| {
            if let Some(target) = Self::ordinary_type_target(candidate) {
                candidates.push(WorkspaceTypeTarget { krate, target });
            }
            Ok(())
        })
    }

    fn collect_workspace_ancestor_glob_type_candidates(
        &self,
        import_node: &ImportNode,
        segment: &str,
        candidates: &mut Vec<WorkspaceTypeTarget<'a>>,
        saw_ambiguous: &mut bool,
        depth: usize,
    ) -> Result<bool, SynParserError> {
        if import_node.source_path().is_empty()
            || !import_node.source_path().iter().all(|part| part == "super")
        {
            return Ok(false);
        }

        let Some(module_id) =
            self.ancestor_glob_module(import_node, "resolving workspace glob import")?
        else {
            return Ok(true);
        };

        let module_id = self.import_scope_module(module_id)?;
        self.collect_workspace_type_candidates_in_module(
            module_id,
            segment,
            candidates,
            saw_ambiguous,
            depth + 1,
        )?;
        Ok(true)
    }

    fn collect_workspace_glob_type_candidates(
        &self,
        import_node: &ImportNode,
        segment: &str,
        candidates: &mut Vec<WorkspaceTypeTarget<'a>>,
        saw_ambiguous: &mut bool,
        depth: usize,
    ) -> Result<(), SynParserError> {
        if self.collect_workspace_ancestor_glob_type_candidates(
            import_node,
            segment,
            candidates,
            saw_ambiguous,
            depth + 1,
        )? {
            return Ok(());
        }

        match self.resolve_local_glob_module(import_node)? {
            LocalModulePathResolution::Resolved(module_id) => {
                return self.collect_workspace_type_candidates_in_module(
                    module_id,
                    segment,
                    candidates,
                    saw_ambiguous,
                    depth + 1,
                );
            }
            LocalModulePathResolution::Unresolved => {}
            LocalModulePathResolution::Ambiguous => {
                *saw_ambiguous = true;
                return Ok(());
            }
        }

        self.collect_workspace_dependency_glob_type_candidates(
            import_node,
            segment,
            candidates,
            saw_ambiguous,
        )
    }

    fn workspace_dependency_tail<'p>(
        &self,
        path: &'p [String],
    ) -> Option<(super::WorkspaceCrate<'a>, &'p [String])> {
        let (root, tail) = path.split_first()?;
        let workspace = self.workspace?;
        workspace
            .crates
            .iter()
            .find(|krate| dependency_name_matches(krate.dependency_name, root))
            .copied()
            .map(|krate| (krate, tail))
    }

    pub(super) fn resolve_local_function_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<LocalFunctionPathResolution, SynParserError> {
        let Some(mut current_module) = self.containing_module_for_owner(owner) else {
            return Ok(LocalFunctionPathResolution::Unresolved);
        };

        let start_idx = self.start_segment_index(path, &mut current_module)?;
        if start_idx >= path.len() {
            return Ok(LocalFunctionPathResolution::Unresolved);
        }

        for idx in start_idx..path.len() {
            let segment = path[idx].as_str();
            let is_last = idx == path.len() - 1;
            if is_last {
                return self.resolve_terminal_function(current_module, segment);
            }

            current_module = match self.resolve_module_segment(current_module, segment)? {
                LocalModulePathResolution::Resolved(module_id) => module_id,
                LocalModulePathResolution::Unresolved => {
                    return Ok(LocalFunctionPathResolution::Unresolved);
                }
                LocalModulePathResolution::Ambiguous => {
                    return Ok(LocalFunctionPathResolution::Ambiguous);
                }
            };
        }

        Ok(LocalFunctionPathResolution::Unresolved)
    }

    pub(super) fn start_segment_index(
        &self,
        path: &[String],
        current_module: &mut ModuleNodeId,
    ) -> Result<usize, SynParserError> {
        let mut idx = 0usize;
        while let Some(segment) = path.get(idx).map(String::as_str) {
            match segment {
                "crate" => {
                    *current_module = self.tree.root();
                    idx += 1;
                }
                "self" => {
                    idx += 1;
                }
                "super" => {
                    *current_module =
                        self.tree.get_parent_module_id(*current_module).ok_or_else(|| {
                            SynParserError::InternalState(format!(
                                "call resolution could not find parent module for {} while resolving {}",
                                current_module,
                                path.join("::")
                            ))
                        })?;
                    idx += 1;
                }
                _ => break,
            }
        }

        Ok(idx)
    }

    pub(super) fn resolve_module_segment(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
    ) -> Result<LocalModulePathResolution, SynParserError> {
        let mut candidates = Vec::new();
        self.visit_scope_candidates(module_id, segment, &mut |candidate| {
            if let Ok(candidate_module) = ModuleNodeId::try_from(candidate) {
                candidates.push(self.import_scope_module(candidate_module)?);
            }
            Ok(())
        })?;
        candidates.sort_unstable();
        candidates.dedup();

        Ok(match candidates.as_slice() {
            [module_id] => LocalModulePathResolution::Resolved(*module_id),
            [] => LocalModulePathResolution::Unresolved,
            _ => LocalModulePathResolution::Ambiguous,
        })
    }

    fn resolve_terminal_function(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
    ) -> Result<LocalFunctionPathResolution, SynParserError> {
        let mut candidates = Vec::new();
        self.visit_scope_candidates(module_id, segment, &mut |candidate| {
            if let Ok(function_id) = FunctionNodeId::try_from(candidate)
                && self
                    .graph
                    .functions()
                    .iter()
                    .any(|function| function.id == function_id)
            {
                candidates.push(function_id);
            }
            Ok(())
        })?;
        candidates.sort_unstable();
        candidates.dedup();

        Ok(match candidates.as_slice() {
            [function_id] => LocalFunctionPathResolution::Resolved(*function_id),
            [] => LocalFunctionPathResolution::Unresolved,
            _ => LocalFunctionPathResolution::Ambiguous,
        })
    }

    pub(super) fn visit_scope_candidates(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        for relation in self
            .tree
            .get_iter_relations_from(&module_id.as_any())
            .into_iter()
            .flatten()
        {
            match relation.rel() {
                SyntacticRelation::Contains { target, .. } => {
                    let target_any = target.as_any();
                    let Ok(node) = self.graph.find_node_unique(target_any) else {
                        continue;
                    };

                    if node.name() == segment {
                        self.visit_binding_terminals(target_any, 0, sink)?;
                    }
                }
                SyntacticRelation::ModuleImports { target, .. } => {
                    let import_node = self.graph.get_import_checked(*target)?;
                    if import_node.visible_name == segment {
                        self.visit_binding_terminals(import_node.id.as_any(), 0, sink)?;
                    }

                    if import_node.is_glob {
                        self.visit_glob_candidates(import_node.id, segment, sink)?;
                    }
                }
                _ => {}
            }
        }

        if let Some(module_node) = self
            .graph
            .modules()
            .iter()
            .find(|module| module.id == module_id)
        {
            for import_node in &module_node.imports {
                if import_node.visible_name == segment {
                    self.visit_binding_terminals(import_node.id.as_any(), 0, sink)?;
                }

                if import_node.is_glob {
                    self.visit_glob_candidates(import_node.id, segment, sink)?;
                }
            }
        }

        Ok(())
    }

    fn visit_glob_candidates(
        &self,
        import_id: ImportNodeId,
        segment: &str,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        self.visit_ancestor_glob_candidates(import_id, segment, sink)?;

        let import_node = self.graph.get_import_checked(import_id)?;
        if let LocalModulePathResolution::Resolved(module_id) =
            self.resolve_local_glob_module(import_node)?
        {
            self.visit_scope_candidates(module_id, segment, sink)?;
        }

        for relation in self.tree.get_iter_relations_to(&import_id.as_any()) {
            let SyntacticRelation::ImportedBy { source, target } = relation.rel() else {
                continue;
            };
            if *target != import_id {
                continue;
            }
            let source_any = source.as_any();
            self.visit_named_binding_terminals(source_any, segment, 0, sink)?;
        }

        Ok(())
    }

    fn resolve_local_glob_module(
        &self,
        import_node: &ImportNode,
    ) -> Result<LocalModulePathResolution, SynParserError> {
        if import_node.source_path().is_empty()
            || !import_node
                .source_path()
                .first()
                .is_some_and(|segment| segment == "crate")
            || import_node.source_path().iter().all(|part| part == "super")
            || self.is_workspace_dependency_path(import_node.source_path())
            || self.is_external_path(import_node.source_path())
        {
            return Ok(LocalModulePathResolution::Unresolved);
        }

        let candidates = self
            .graph
            .modules()
            .iter()
            .filter(|module| module.path == import_node.source_path())
            .map(|module| self.import_scope_module(module.id))
            .collect::<Result<Vec<_>, _>>()?;
        Self::local_module_resolution(candidates)
    }

    fn local_module_resolution(
        mut candidates: Vec<ModuleNodeId>,
    ) -> Result<LocalModulePathResolution, SynParserError> {
        candidates.sort_unstable();
        candidates.dedup();

        Ok(match candidates.as_slice() {
            [module_id] => LocalModulePathResolution::Resolved(*module_id),
            [] => LocalModulePathResolution::Unresolved,
            _ => LocalModulePathResolution::Ambiguous,
        })
    }

    fn visit_ancestor_glob_candidates(
        &self,
        import_id: ImportNodeId,
        segment: &str,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        let import_node = self.graph.get_import_checked(import_id)?;
        let Some(module_id) = self.ancestor_glob_module(import_node, "resolving glob import")?
        else {
            return Ok(());
        };

        let module_id = self.import_scope_module(module_id)?;
        self.visit_scope_candidates(module_id, segment, sink)
    }

    fn visit_named_binding_terminals(
        &self,
        start: AnyNodeId,
        segment: &str,
        depth: usize,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        let mut visited = HashSet::new();
        self.visit_named_binding_terminals_inner(start, segment, depth, &mut visited, sink)
    }

    fn visit_named_binding_terminals_inner(
        &self,
        start: AnyNodeId,
        segment: &str,
        depth: usize,
        visited: &mut HashSet<AnyNodeId>,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {start}"
            )));
        }

        let Ok(import_id) = ImportNodeId::try_from(start) else {
            let Ok(node) = self.graph.find_node_unique(start) else {
                return Ok(());
            };
            if node.name() == segment {
                sink(start)?;
            }
            return Ok(());
        };

        if !visited.insert(import_id.as_any()) {
            return Ok(());
        }

        let mut had_sources = false;
        for relation in self.tree.get_iter_relations_to(&import_id.as_any()) {
            let SyntacticRelation::ImportedBy { source, target } = relation.rel() else {
                continue;
            };
            if *target != import_id {
                continue;
            }
            let source_any = source.as_any();
            if matches!(source_any, AnyNodeId::Unresolved(_)) {
                continue;
            }
            had_sources = true;
            self.visit_named_binding_terminals_inner(
                source_any,
                segment,
                depth + 1,
                visited,
                sink,
            )?;
        }

        if had_sources {
            return Ok(());
        }

        let import_node = self.graph.get_import_checked(import_id)?;
        if self.is_external_path(import_node.source_path()) {
            return Ok(());
        }

        self.visit_local_import_source_terminals(import_node, depth + 1, &mut |candidate| {
            let Ok(node) = self.graph.find_node_unique(candidate) else {
                return Ok(());
            };
            if node.name() == segment {
                sink(candidate)?;
            }
            Ok(())
        })
    }

    pub(super) fn visit_binding_terminals(
        &self,
        start: AnyNodeId,
        depth: usize,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        let mut visited = HashSet::new();
        self.visit_binding_terminals_inner(start, depth, &mut visited, sink)
    }

    fn visit_binding_terminals_inner(
        &self,
        start: AnyNodeId,
        depth: usize,
        visited: &mut HashSet<AnyNodeId>,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {start}"
            )));
        }

        let Ok(import_id) = ImportNodeId::try_from(start) else {
            return sink(start);
        };

        if !visited.insert(import_id.as_any()) {
            return Ok(());
        }

        let mut had_sources = false;
        for relation in self.tree.get_iter_relations_to(&import_id.as_any()) {
            let SyntacticRelation::ImportedBy { source, target } = relation.rel() else {
                continue;
            };
            if *target != import_id {
                continue;
            }
            let source_any = source.as_any();
            if matches!(source_any, AnyNodeId::Unresolved(_)) {
                continue;
            }
            had_sources = true;
            self.visit_binding_terminals_inner(source_any, depth + 1, visited, sink)?;
        }

        if had_sources {
            return Ok(());
        }

        let import_node = self.graph.get_import_checked(import_id)?;
        if self.is_external_path(import_node.source_path()) {
            return Ok(());
        }

        self.visit_local_import_source_terminals(import_node, depth + 1, sink)
    }

    fn visit_local_import_source_terminals(
        &self,
        import_node: &ImportNode,
        depth: usize,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        if depth > MAX_IMPORT_CHAIN_DEPTH {
            return Err(SynParserError::InternalState(format!(
                "call resolution exceeded import chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {}",
                import_node.id.as_any()
            )));
        }

        let path = import_node.source_path();
        if path.is_empty()
            || self.is_workspace_dependency_path(path)
            || self.is_external_path(path)
            || matches!(path, [segment] if !matches!(segment.as_str(), "crate" | "self" | "super"))
        {
            return Ok(());
        }

        let Some(mut current_module) = self.containing_module(import_node.id.as_any()) else {
            return Ok(());
        };
        current_module = self.import_scope_module(current_module)?;
        let start_idx = self.start_segment_index(path, &mut current_module)?;
        if start_idx >= path.len() {
            return Ok(());
        }

        for idx in start_idx..path.len() {
            let segment = path[idx].as_str();
            let is_last = idx == path.len() - 1;
            if is_last {
                return self.visit_direct_module_terminals(current_module, segment, sink);
            }

            current_module = match self.resolve_module_segment(current_module, segment)? {
                LocalModulePathResolution::Resolved(module_id) => module_id,
                LocalModulePathResolution::Unresolved => return Ok(()),
                LocalModulePathResolution::Ambiguous => return Ok(()),
            };
        }

        Ok(())
    }

    fn visit_direct_module_terminals(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
        sink: &mut impl FnMut(AnyNodeId) -> Result<(), SynParserError>,
    ) -> Result<(), SynParserError> {
        for relation in self
            .tree
            .get_iter_relations_from(&module_id.as_any())
            .into_iter()
            .flatten()
        {
            let SyntacticRelation::Contains { target, .. } = relation.rel() else {
                continue;
            };
            let target_any = target.as_any();
            let Ok(node) = self.graph.find_node_unique(target_any) else {
                continue;
            };
            if node.name() == segment {
                sink(target_any)?;
            }
        }
        Ok(())
    }

    pub(super) fn containing_module_for_owner(
        &self,
        owner: CallBodyOwnerId,
    ) -> Option<ModuleNodeId> {
        match owner {
            CallBodyOwnerId::Function(id) => self.containing_module(id.as_any()),
            CallBodyOwnerId::Macro(id) => self.containing_module(id.as_any()),
            CallBodyOwnerId::Method(id) => self.containing_module(id.as_any()),
            CallBodyOwnerId::Const(id) => self.containing_module(id.as_any()),
            CallBodyOwnerId::Static(id) => self.containing_module(id.as_any()),
            CallBodyOwnerId::Executable(id) => self
                .graph
                .executable_bodies()
                .iter()
                .find(|body| body.id == id)
                .and_then(|body| self.containing_module_for_owner(body.parent)),
        }
    }

    pub(super) fn module_for_node(&self, owner: AnyNodeId) -> Option<ModuleNodeId> {
        self.containing_module(owner)
    }

    fn ancestor_glob_module(
        &self,
        import_node: &ImportNode,
        context: &str,
    ) -> Result<Option<ModuleNodeId>, SynParserError> {
        if import_node.source_path().is_empty()
            || !import_node.source_path().iter().all(|part| part == "super")
        {
            return Ok(None);
        }

        let Some(mut module_id) = self.containing_module(import_node.id.as_any()) else {
            return Ok(None);
        };
        for _ in import_node.source_path() {
            module_id = self.tree.get_parent_module_id(module_id).ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "call resolution could not find parent module for {module_id} while {context} {}",
                    import_node.source_path().join("::")
                ))
            })?;
        }

        Ok(Some(module_id))
    }

    fn containing_module(&self, owner: AnyNodeId) -> Option<ModuleNodeId> {
        if let Ok(module_id) = ModuleNodeId::try_from(owner) {
            return Some(module_id);
        }

        self.tree
            .get_iter_relations_to(&owner)
            .find_map(|relation| match relation.rel() {
                SyntacticRelation::Contains { source, target } if target.as_any() == owner => {
                    Some(*source)
                }
                SyntacticRelation::ImplAssociatedItem { source, target }
                    if target.as_any() == owner =>
                {
                    self.containing_module(source.as_any())
                }
                SyntacticRelation::TraitAssociatedItem { source, target }
                    if target.as_any() == owner =>
                {
                    self.containing_module(source.as_any())
                }
                _ => None,
            })
    }
}
