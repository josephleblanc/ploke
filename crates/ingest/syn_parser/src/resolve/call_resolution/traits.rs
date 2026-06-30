use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AsAnyNodeId, CallBodyOwnerId, ImplNode, ModuleNodeId, OrdinaryTypeTargetId, TraitNode,
            TraitNodeId, TypeGenericParamNodeId,
        },
        relations::{SyntacticRelation, TypeRelation},
    },
    resolve::RelationIndexer,
};

use super::{
    AssocPathResolution, CallRelationResolver, MAX_BLANKET_BOUND_DEPTH,
    trait_declares_instance_method,
};

impl CallRelationResolver<'_> {
    pub(super) fn resolve_trait_impl_instance_method(
        &self,
        owner: CallBodyOwnerId,
        target: OrdinaryTypeTargetId,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let receiver_targets = self.ordinary_receiver_targets(target, type_relations)?;
        let mut candidates = Vec::new();
        let mut matched_trait_impl = false;

        for impl_node in self
            .graph
            .impls()
            .iter()
            .filter(|impl_node| impl_node.trait_type.is_some())
        {
            let Some(self_target) = self.impl_self_target(impl_node, type_relations)? else {
                continue;
            };
            if !self.exact_trait_impl_self_type_applies(impl_node, self_target, &receiver_targets)
                && !Self::is_unconstrained_blanket_impl(impl_node, self_target)
                && !self.constrained_blanket_impl_applies(
                    impl_node,
                    self_target,
                    &receiver_targets,
                    type_relations,
                )?
            {
                continue;
            }

            let Some(trait_node) = self.local_trait_node_for_impl(impl_node, type_relations)?
            else {
                continue;
            };
            if !trait_declares_instance_method(trait_node, method_name) {
                continue;
            }
            if !self.trait_is_visible_from_owner(owner, trait_node.id)? {
                continue;
            }

            matched_trait_impl = true;
            candidates.extend(
                impl_node
                    .methods
                    .iter()
                    .filter(|method| {
                        method.name == method_name
                            && method.parameters.iter().any(|param| param.is_self)
                    })
                    .map(|method| method.id),
            );
        }

        if matched_trait_impl {
            Ok(Some(Self::method_resolution(candidates)))
        } else {
            Ok(None)
        }
    }

    fn exact_trait_impl_self_type_applies(
        &self,
        impl_node: &ImplNode,
        self_target: OrdinaryTypeTargetId,
        receiver_targets: &[OrdinaryTypeTargetId],
    ) -> bool {
        impl_node.generic_params.is_empty()
            && impl_node.where_predicates.is_empty()
            && receiver_targets.contains(&self_target)
    }

    fn is_unconstrained_blanket_impl(
        impl_node: &ImplNode,
        self_target: OrdinaryTypeTargetId,
    ) -> bool {
        if !impl_node.where_predicates.is_empty() || impl_node.generic_params.len() != 1 {
            return false;
        }

        let Some(param) = impl_node.generic_params.first() else {
            return false;
        };
        let Ok(param_id) = TypeGenericParamNodeId::try_from(self_target) else {
            return false;
        };
        if TypeGenericParamNodeId::try_refine(param.id, &param.kind).ok() != Some(param_id) {
            return false;
        }

        match &param.kind {
            crate::parser::types::GenericParamKind::Type {
                bounds, default, ..
            } => bounds.is_empty() && default.is_none(),
            crate::parser::types::GenericParamKind::Lifetime { .. }
            | crate::parser::types::GenericParamKind::Const { .. } => false,
        }
    }

    fn constrained_blanket_impl_applies(
        &self,
        impl_node: &ImplNode,
        self_target: OrdinaryTypeTargetId,
        receiver_targets: &[OrdinaryTypeTargetId],
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        self.constrained_blanket_impl_applies_at_depth(
            impl_node,
            self_target,
            receiver_targets,
            type_relations,
            0,
        )
    }

    fn constrained_blanket_impl_applies_at_depth(
        &self,
        impl_node: &ImplNode,
        self_target: OrdinaryTypeTargetId,
        receiver_targets: &[OrdinaryTypeTargetId],
        type_relations: &[TypeRelation],
        depth: usize,
    ) -> Result<bool, SynParserError> {
        if depth >= MAX_BLANKET_BOUND_DEPTH {
            return Ok(false);
        }

        if impl_node.generic_params.len() != 1 {
            return Ok(false);
        }

        let Some(param) = impl_node.generic_params.first() else {
            return Ok(false);
        };
        let Ok(param_id) = TypeGenericParamNodeId::try_from(self_target) else {
            return Ok(false);
        };
        if TypeGenericParamNodeId::try_refine(param.id, &param.kind).ok() != Some(param_id) {
            return Ok(false);
        }

        let mut sources = Vec::new();
        match &param.kind {
            crate::parser::types::GenericParamKind::Type {
                bounds, default, ..
            } => {
                if default.is_some() {
                    return Ok(false);
                }
                sources.extend(bounds.iter().copied());
            }
            crate::parser::types::GenericParamKind::Lifetime { .. }
            | crate::parser::types::GenericParamKind::Const { .. } => return Ok(false),
        }

        for predicate in &impl_node.where_predicates {
            if self.where_subject_matches(predicate, self_target, type_relations) {
                sources.extend(predicate.bounds.iter().copied());
            }
        }

        sources.sort_unstable();
        sources.dedup();
        if sources.is_empty() {
            return Ok(false);
        }

        let traits = self.bound_trait_targets(&sources, type_relations)?;
        if traits.len() != sources.len() {
            return Ok(false);
        }

        for bound in traits {
            if !self.receiver_satisfies_trait_at_depth(
                receiver_targets,
                bound,
                type_relations,
                depth + 1,
            )? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    pub(super) fn receiver_satisfies_trait_at_depth(
        &self,
        receiver_targets: &[OrdinaryTypeTargetId],
        bound: TraitNodeId,
        type_relations: &[TypeRelation],
        depth: usize,
    ) -> Result<bool, SynParserError> {
        if depth >= MAX_BLANKET_BOUND_DEPTH {
            return Ok(false);
        }

        for impl_node in self
            .graph
            .impls()
            .iter()
            .filter(|impl_node| impl_node.trait_type.is_some())
        {
            let Some(trait_node) = self.local_trait_node_for_impl(impl_node, type_relations)?
            else {
                continue;
            };
            if trait_node.id != bound {
                continue;
            }

            let Some(target) = self.impl_self_target(impl_node, type_relations)? else {
                continue;
            };
            if receiver_targets.contains(&target)
                || Self::is_unconstrained_blanket_impl(impl_node, target)
            {
                return Ok(true);
            }
            if self.constrained_blanket_impl_applies_at_depth(
                impl_node,
                target,
                receiver_targets,
                type_relations,
                depth + 1,
            )? {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub(super) fn trait_is_visible_from_owner(
        &self,
        owner: CallBodyOwnerId,
        trait_id: TraitNodeId,
    ) -> Result<bool, SynParserError> {
        let Some(module_id) = self.containing_module_for_owner(owner) else {
            return Ok(false);
        };
        let module_id = self.import_scope_module(module_id)?;
        self.trait_is_visible_in_module(module_id, trait_id)
    }

    fn trait_is_visible_in_module(
        &self,
        module_id: ModuleNodeId,
        trait_id: TraitNodeId,
    ) -> Result<bool, SynParserError> {
        let target_trait = trait_id.as_any();

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
            if target_any == target_trait {
                return Ok(true);
            }

            let Ok(node) = self.graph.find_node_unique(target_any) else {
                continue;
            };
            if node.as_import().is_none() {
                continue;
            }

            let mut visible = false;
            self.visit_binding_terminals(target_any, 0, &mut |candidate| {
                if candidate == target_trait {
                    visible = true;
                }
                Ok(())
            })?;
            if visible {
                return Ok(true);
            }
        }

        Ok(false)
    }

    pub(super) fn local_trait_node_for_impl(
        &self,
        impl_node: &ImplNode,
        type_relations: &[TypeRelation],
    ) -> Result<Option<&TraitNode>, SynParserError> {
        let Some(trait_source) = impl_node.trait_type else {
            return Ok(None);
        };

        let mut targets = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Trait { source, target } if *source == trait_source => {
                    TraitNodeId::try_from(*target).ok()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();

        let trait_id = match targets.as_slice() {
            [target] => *target,
            [] => return Ok(None),
            _ => {
                return Err(SynParserError::InternalState(format!(
                    "call resolution found multiple trait targets for impl {}",
                    impl_node.id
                )));
            }
        };

        self.graph
            .traits()
            .iter()
            .find(|node| node.id == trait_id)
            .map(Some)
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "call resolution found trait relation to missing trait {trait_id}"
                ))
            })
    }
}
