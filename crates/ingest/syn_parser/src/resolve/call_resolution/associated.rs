use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            CallBodyOwnerId, MethodNodeId, OrdinaryTypeTargetId, OrdinaryTypeUseId,
            TypeGenericParamNodeId,
        },
        relations::TypeRelation,
        types::TypeNode,
    },
    resolve::type_resolution_v2,
};

use super::{
    AssocPathResolution, CallRelationResolver, LocalTraitResolution, LocalTypeResolution,
    WorkspaceTypeResolution, WorkspaceTypeTarget,
};

impl CallRelationResolver<'_> {
    pub(super) fn is_external_self_assoc(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<bool, SynParserError> {
        if !matches!(path, [segment, _] if segment == "Self") {
            return Ok(false);
        }

        let Some(owner_method_id) = self.assoc_owner_method(owner)? else {
            return Ok(false);
        };
        let Some(impl_id) = self.impl_for_owner_method(owner_method_id)? else {
            return Ok(false);
        };
        let Some(impl_node) = self.maybe_impl_node(impl_id) else {
            return Ok(false);
        };
        if impl_node.trait_type.is_none() {
            return Ok(false);
        }

        self.type_use_is_external(owner, impl_node.self_type)
    }

    pub(super) fn is_external_alias_assoc_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        let Some((_method_name, type_path)) = path.split_last() else {
            return Ok(false);
        };
        let [type_segment] = type_path else {
            return Ok(false);
        };
        if matches!(type_segment.as_str(), "Self" | "crate" | "self" | "super") {
            return Ok(false);
        }
        if self.owner_has_generic_param_named(owner, type_segment)? {
            return Ok(false);
        }

        if self.local_type_target_alias_is_external(owner, type_segment, type_relations)? {
            return Ok(true);
        }

        match self.resolve_workspace_type_import(owner, type_segment)? {
            WorkspaceTypeResolution::Resolved(candidate) => {
                Self::workspace_type_target_alias_is_external(candidate)
            }
            WorkspaceTypeResolution::Unresolved => Ok(false),
            WorkspaceTypeResolution::Ambiguous => Ok(false),
        }
    }

    pub(super) fn is_external_generic_bound_assoc_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        arg_count: usize,
    ) -> Result<bool, SynParserError> {
        let Some((method_name, type_path)) = path.split_last() else {
            return Ok(false);
        };
        let [type_segment] = type_path else {
            return Ok(false);
        };
        let expected_trait = match method_name.as_str() {
            "default" if arg_count == 0 => "Default",
            _ => return Ok(false),
        };

        for scope in self.generic_bound_scopes(owner)? {
            for param in scope.params {
                if param.kind.name() != Some(type_segment.as_str()) {
                    continue;
                }
                if let Some(bounds) = param.kind.bounds()
                    && self.bounds_include_external_trait(owner, bounds, expected_trait)?
                {
                    return Ok(true);
                }
            }

            for predicate in scope.predicates {
                if !self.type_path_matches_segment(predicate.subject, type_segment)? {
                    continue;
                }
                if self.bounds_include_external_trait(owner, &predicate.bounds, expected_trait)? {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn bounds_include_external_trait(
        &self,
        owner: CallBodyOwnerId,
        bounds: &[crate::parser::type_slots::TraitTypeUseId],
        expected_trait: &str,
    ) -> Result<bool, SynParserError> {
        for bound in bounds {
            if self.is_external_trait_bound(owner, *bound, expected_trait)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn local_type_target_alias_is_external(
        &self,
        owner: CallBodyOwnerId,
        type_segment: &str,
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        match self.resolve_local_type_segment(owner, type_segment)? {
            LocalTypeResolution::Resolved(target) => {
                self.ordinary_type_target_alias_is_external(target, type_relations)
            }
            LocalTypeResolution::Unresolved => Ok(false),
            LocalTypeResolution::Ambiguous => Ok(false),
        }
    }

    pub(super) fn owner_has_generic_param_named(
        &self,
        owner: CallBodyOwnerId,
        type_segment: &str,
    ) -> Result<bool, SynParserError> {
        for scope in self.generic_bound_scopes(owner)? {
            if scope
                .params
                .iter()
                .any(|param| param.kind.name() == Some(type_segment))
            {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub(super) fn workspace_type_target_alias_is_external(
        candidate: WorkspaceTypeTarget<'_>,
    ) -> Result<bool, SynParserError> {
        let type_report = type_resolution_v2::resolve_type_relations_after_tree(
            candidate.krate.graph,
            candidate.krate.tree,
        )?;
        let resolver = CallRelationResolver::new(candidate.krate.graph, candidate.krate.tree);
        resolver.ordinary_type_target_alias_is_external(candidate.target, &type_report.relations)
    }

    fn assoc_owner_method(
        &self,
        owner: CallBodyOwnerId,
    ) -> Result<Option<MethodNodeId>, SynParserError> {
        match owner {
            CallBodyOwnerId::Method(id) => Ok(Some(id)),
            CallBodyOwnerId::Executable(id) => self.assoc_owner_method(
                self.executable_parent_owner(id, "external self associated lookup")?,
            ),
            _ => Ok(None),
        }
    }

    pub(super) fn type_use_is_external(
        &self,
        owner: CallBodyOwnerId,
        type_id: OrdinaryTypeUseId,
    ) -> Result<bool, SynParserError> {
        match self.type_node(type_id)? {
            TypeNode::Named(node) => Ok(self.is_external_path(&node.path)
                || self.is_external_import_path(owner, &node.path)?),
            TypeNode::Reference(node) => self.type_use_is_external(owner, node.referenced),
            TypeNode::Paren(node) => self.type_use_is_external(owner, node.inner),
            _ => Ok(false),
        }
    }

    pub(super) fn resolve_associated_function_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        arg_count: usize,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let Some((method_name, type_path)) = path.split_last() else {
            return Ok(None);
        };
        if type_path.is_empty() {
            return Ok(None);
        }
        let [type_segment] = type_path else {
            return self.resolve_local_type_assoc_function(
                owner,
                type_path,
                method_name,
                arg_count,
                type_relations,
            );
        };

        if type_segment == "Self" {
            return self
                .resolve_self_associated_function(owner, method_name, arg_count, type_relations)
                .map(Some);
        }

        if matches!(type_segment.as_str(), "crate" | "self" | "super") {
            return Ok(None);
        }

        if let Some(resolution) = self.resolve_trait_associated_function_path(
            owner,
            type_segment,
            method_name,
            arg_count,
        )? {
            return Ok(Some(resolution));
        }

        if let Some(resolution) = self.resolve_generic_bound_assoc_function_by_name(
            owner,
            type_segment,
            method_name,
            arg_count,
            type_relations,
        )? {
            return Ok(Some(resolution));
        }

        match self.resolve_local_type_segment(owner, type_segment)? {
            LocalTypeResolution::Resolved(target) => {
                let resolution = self.resolve_type_associated_function(
                    owner,
                    target,
                    method_name,
                    Some(arg_count),
                    type_relations,
                )?;
                match resolution {
                    Some(resolution) => Ok(Some(resolution)),
                    None => self.resolve_where_bound_assoc_function_by_name(
                        owner,
                        type_segment,
                        method_name,
                        arg_count,
                        type_relations,
                    ),
                }
            }
            LocalTypeResolution::Unresolved => {
                if let Some(resolution) = self.resolve_where_bound_assoc_function_by_name(
                    owner,
                    type_segment,
                    method_name,
                    arg_count,
                    type_relations,
                )? {
                    return Ok(Some(resolution));
                }
                self.resolve_workspace_type_assoc_function(owner, type_segment, method_name)
            }
            LocalTypeResolution::Ambiguous => Ok(Some(AssocPathResolution::Ambiguous)),
        }
    }

    fn resolve_generic_bound_assoc_function_by_name(
        &self,
        owner: CallBodyOwnerId,
        type_segment: &str,
        method_name: &str,
        arg_count: usize,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let mut matches = Vec::new();
        for scope in self.generic_bound_scopes(owner)? {
            matches.extend(scope.params.iter().filter_map(|param| {
                (param.kind.name() == Some(type_segment))
                    .then(|| TypeGenericParamNodeId::try_refine(param.id, &param.kind).ok())
                    .flatten()
            }));
        }
        matches.sort_unstable();
        matches.dedup();

        match matches.as_slice() {
            [] => Ok(None),
            [param_id] => {
                let target = OrdinaryTypeTargetId::from(*param_id);
                self.resolve_generic_bound_path_item(
                    owner,
                    target,
                    method_name,
                    arg_count,
                    type_relations,
                )
            }
            _ => Ok(Some(AssocPathResolution::Ambiguous)),
        }
    }

    fn resolve_where_bound_assoc_function_by_name(
        &self,
        owner: CallBodyOwnerId,
        type_segment: &str,
        method_name: &str,
        arg_count: usize,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let traits = self.where_bound_traits_for_type_name(owner, type_segment, type_relations)?;
        if traits.is_empty() {
            return Ok(None);
        }

        self.resolve_bound_assoc_function(traits, method_name, arg_count)
            .map(Some)
    }

    fn resolve_local_type_assoc_function(
        &self,
        owner: CallBodyOwnerId,
        type_path: &[String],
        method_name: &str,
        arg_count: usize,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        match self.resolve_local_type_path(owner, type_path)? {
            LocalTypeResolution::Resolved(target) => self.resolve_type_associated_function(
                owner,
                target,
                method_name,
                Some(arg_count),
                type_relations,
            ),
            LocalTypeResolution::Unresolved => self.resolve_workspace_type_path_assoc_function(
                owner,
                type_path,
                method_name,
                arg_count,
            ),
            LocalTypeResolution::Ambiguous => Ok(Some(AssocPathResolution::Ambiguous)),
        }
    }

    fn resolve_workspace_type_path_assoc_function(
        &self,
        owner: CallBodyOwnerId,
        type_path: &[String],
        method_name: &str,
        _arg_count: usize,
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        match self.resolve_workspace_type_path(owner, type_path)? {
            WorkspaceTypeResolution::Resolved(candidate) => {
                let type_report = type_resolution_v2::resolve_type_relations_after_tree(
                    candidate.krate.graph,
                    candidate.krate.tree,
                )?;
                let resolver =
                    CallRelationResolver::new(candidate.krate.graph, candidate.krate.tree);
                resolver.resolve_inherent_type_associated_function(
                    candidate.target,
                    method_name,
                    &type_report.relations,
                )
            }
            WorkspaceTypeResolution::Unresolved => Ok(None),
            WorkspaceTypeResolution::Ambiguous => Ok(Some(AssocPathResolution::Ambiguous)),
        }
    }

    fn resolve_trait_associated_function_path(
        &self,
        owner: CallBodyOwnerId,
        trait_segment: &str,
        method_name: &str,
        arg_count: usize,
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        match self.resolve_local_trait_segment(owner, trait_segment)? {
            LocalTraitResolution::Resolved(trait_id) => {
                let trait_node = self.graph.get_trait_checked(trait_id)?;
                Ok(Some(self.resolve_trait_path_item(
                    trait_node,
                    method_name,
                    arg_count,
                )))
            }
            LocalTraitResolution::Unresolved => Ok(None),
            LocalTraitResolution::Ambiguous => Ok(Some(AssocPathResolution::Ambiguous)),
        }
    }

    fn resolve_self_associated_function(
        &self,
        owner: CallBodyOwnerId,
        method_name: &str,
        arg_count: usize,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(owner_method_id) = self.assoc_owner_method(owner)? else {
            return Ok(AssocPathResolution::Unsupported);
        };

        if let Some(impl_id) = self.impl_for_owner_method(owner_method_id)? {
            let Some(impl_node) = self.maybe_impl_node(impl_id) else {
                return Ok(AssocPathResolution::Unsupported);
            };
            if impl_node.trait_type.is_some() {
                let Some(self_target) = self.impl_self_target(impl_node, type_relations)? else {
                    return Ok(AssocPathResolution::Unsupported);
                };
                return self
                    .resolve_type_associated_function(
                        owner,
                        self_target,
                        method_name,
                        Some(arg_count),
                        type_relations,
                    )
                    .map(|resolution| resolution.unwrap_or(AssocPathResolution::Unsupported));
            }
            return Ok(self.resolve_method_in_impl(impl_node, method_name));
        }

        let Some(trait_id) = self.trait_for_owner_method(owner_method_id)? else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let trait_node = self.graph.get_trait_checked(trait_id)?;
        Ok(self.resolve_associated_function_in_trait(trait_node, method_name))
    }

    fn resolve_type_associated_function(
        &self,
        owner: CallBodyOwnerId,
        target: OrdinaryTypeTargetId,
        method_name: &str,
        arg_count: Option<usize>,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        if let Some(resolution) =
            self.resolve_inherent_type_associated_function(target, method_name, type_relations)?
        {
            return Ok(Some(resolution));
        }

        if let Some(arg_count) = arg_count {
            self.resolve_generic_bound_path_item(
                owner,
                target,
                method_name,
                arg_count,
                type_relations,
            )
        } else {
            Ok(None)
        }
    }

    fn resolve_workspace_type_assoc_function(
        &self,
        owner: CallBodyOwnerId,
        type_segment: &str,
        method_name: &str,
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        match self.resolve_workspace_type_import(owner, type_segment)? {
            WorkspaceTypeResolution::Resolved(candidate) => {
                let type_report = type_resolution_v2::resolve_type_relations_after_tree(
                    candidate.krate.graph,
                    candidate.krate.tree,
                )?;
                let resolver =
                    CallRelationResolver::new(candidate.krate.graph, candidate.krate.tree);
                resolver.resolve_inherent_type_associated_function(
                    candidate.target,
                    method_name,
                    &type_report.relations,
                )
            }
            WorkspaceTypeResolution::Unresolved => Ok(None),
            WorkspaceTypeResolution::Ambiguous => Ok(Some(AssocPathResolution::Ambiguous)),
        }
    }

    fn resolve_inherent_type_associated_function(
        &self,
        target: OrdinaryTypeTargetId,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let receiver_targets = self.ordinary_receiver_targets(target, type_relations)?;
        let mut candidates = Vec::new();
        let mut matched_inherent_impl = false;
        for impl_node in self
            .graph
            .impls()
            .iter()
            .filter(|impl_node| impl_node.trait_type.is_none())
        {
            let Some(self_target) = self.impl_self_target(impl_node, type_relations)? else {
                continue;
            };
            if !receiver_targets.contains(&self_target) {
                continue;
            }
            matched_inherent_impl = true;
            candidates.extend(
                impl_node
                    .methods
                    .iter()
                    .filter(|method| method.name == method_name)
                    .map(|method| method.id),
            );
        }

        if matched_inherent_impl {
            Ok(Some(Self::method_resolution(candidates)))
        } else {
            Ok(None)
        }
    }

    fn resolve_generic_bound_path_item(
        &self,
        owner: CallBodyOwnerId,
        target: OrdinaryTypeTargetId,
        method_name: &str,
        arg_count: usize,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let param_id = TypeGenericParamNodeId::try_from(target).ok();
        let traits = self.generic_bound_traits(owner, target, param_id, type_relations)?;
        if traits.is_empty() {
            return Ok(None);
        }

        self.resolve_bound_assoc_function(traits, method_name, arg_count)
            .map(Some)
    }

    fn resolve_bound_assoc_function(
        &self,
        traits: Vec<crate::parser::nodes::TraitNodeId>,
        method_name: &str,
        arg_count: usize,
    ) -> Result<AssocPathResolution, SynParserError> {
        let mut candidates = Vec::new();
        for trait_id in traits {
            let trait_node = self.trait_node(trait_id)?;
            candidates.extend(
                trait_node
                    .methods
                    .iter()
                    .filter(|method| {
                        method.name == method_name && method.parameters.len() == arg_count
                    })
                    .map(|method| method.id),
            );
        }

        Ok(Self::method_resolution(candidates))
    }
}
