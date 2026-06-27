use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AnyNodeId, AsAnyNodeId, CallBodyOwnerId, FunctionNodeId, ImportKind, ImportNodeId,
            ModuleNodeId,
        },
        relations::SyntacticRelation,
    },
    resolve::RelationIndexer,
};

use super::{
    CallRelationResolver, LocalFunctionPathResolution, LocalModulePathResolution,
    MAX_IMPORT_CHAIN_DEPTH,
};

impl CallRelationResolver<'_> {
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
            let Ok(source_import) = ImportNodeId::try_from(source.as_any()) else {
                all_sources_external = false;
                continue;
            };
            if !self.import_binding_is_external(source_import, depth + 1)? {
                all_sources_external = false;
            }
        }

        if had_sources {
            return Ok(all_sources_external);
        }

        Ok(false)
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
                candidates.push(candidate_module);
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
            if let Ok(function_id) = FunctionNodeId::try_from(candidate) {
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
            let SyntacticRelation::Contains { target, .. } = relation.rel() else {
                continue;
            };
            let target_any = target.as_any();
            let Ok(node) = self.graph.find_node_unique(target_any) else {
                continue;
            };

            if node.name() == segment {
                self.visit_binding_terminals(target_any, 0, sink)?;
            }

            if let Some(import_node) = node.as_import()
                && import_node.is_glob
            {
                self.visit_glob_candidates(import_node.id, segment, sink)?;
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
        for relation in self.tree.get_iter_relations_to(&import_id.as_any()) {
            let SyntacticRelation::ImportedBy { source, target } = relation.rel() else {
                continue;
            };
            if *target != import_id {
                continue;
            }
            let source_any = source.as_any();
            let Ok(source_node) = self.graph.find_node_unique(source_any) else {
                continue;
            };
            if source_node.name() == segment {
                self.visit_binding_terminals(source_any, 0, sink)?;
            }
        }

        Ok(())
    }

    pub(super) fn visit_binding_terminals(
        &self,
        start: AnyNodeId,
        depth: usize,
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

        let mut had_sources = false;
        for relation in self.tree.get_iter_relations_to(&import_id.as_any()) {
            let SyntacticRelation::ImportedBy { source, target } = relation.rel() else {
                continue;
            };
            if *target != import_id {
                continue;
            }
            had_sources = true;
            self.visit_binding_terminals(source.as_any(), depth + 1, sink)?;
        }

        if had_sources {
            return Ok(());
        }

        let import_node = self.graph.get_import_checked(import_id)?;
        if self.is_external_path(import_node.source_path()) {
            return Ok(());
        }

        Ok(())
    }

    pub(super) fn containing_module_for_owner(
        &self,
        owner: CallBodyOwnerId,
    ) -> Option<ModuleNodeId> {
        match owner {
            CallBodyOwnerId::Function(id) => self.containing_module(id.as_any()),
            CallBodyOwnerId::Method(id) => self.containing_module(id.as_any()),
            CallBodyOwnerId::Const(id) => self.containing_module(id.as_any()),
            CallBodyOwnerId::Static(id) => self.containing_module(id.as_any()),
        }
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
