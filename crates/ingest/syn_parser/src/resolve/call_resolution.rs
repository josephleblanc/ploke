//! Typed call-resolution facts for parser-owned structural call sites.
//!
//! This resolver consumes the structural call-site inventory emitted by the
//! parser and constructs semantic call-target facts only when the target can be
//! proven from local typed graph evidence. Unsupported shapes remain visible as
//! statuses and do not produce fake target edges.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use crate::{
    error::SynParserError,
    parser::{
        ParsedCodeGraph,
        graph::GraphAccess,
        nodes::{
            AnyCallSiteId, AnyNodeId, AnyTypeId, AsAnyNodeId, AssociatedItemNodeId,
            CallBodyOwnerId, CallNode, FunctionNodeId, ImplNode, ImplNodeId, ImportKind,
            ImportNodeId, MethodCallNode, MethodCallReceiver, MethodNodeId, ModuleNodeId,
            OrdinaryTypeSourceId, OrdinaryTypeTargetId, OrdinaryTypeUseId, ParamData, StructNodeId,
            TraitNode, TraitNodeId, TraitTypeSourceId, TypeAliasNodeId, TypeGenericParamNodeId,
            VariantNodeId,
        },
        relations::{
            CallRelation, CallResolutionKind, CallResolutionStatus, SyntacticRelation, TypeRelation,
        },
        types::{GenericParamNode, TypeNode, TypeWherePredicate},
    },
};

use super::{RelationIndexer, module_tree::ModuleTree};

mod associated;
mod constructors;
mod dynamic;
mod path;

const MAX_IMPORT_CHAIN_DEPTH: usize = 100;
const MAX_BLANKET_BOUND_DEPTH: usize = 16;

/// Owned collection adapter for typed call-resolution output.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallResolutionReport {
    /// Typed semantic call-target edges proven by the resolver.
    pub relations: Vec<CallRelation>,
    /// Resolver outcome for each structural call site considered by this pass.
    pub statuses: Vec<CallResolutionStatus>,
    /// Compact counters for validating the resolver during incremental rollout.
    pub summary: CallResolutionSummary,
}

/// Compact counters for call-resolution outcomes.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct CallResolutionSummary {
    pub resolved: usize,
    pub unresolved: usize,
    pub ambiguous: usize,
    pub external: usize,
    pub unsupported: usize,
}

impl CallResolutionSummary {
    fn from_statuses(statuses: &[CallResolutionStatus]) -> Self {
        let mut summary = Self::default();
        for status in statuses {
            match status {
                CallResolutionStatus::Resolved { .. } => summary.resolved += 1,
                CallResolutionStatus::Unresolved { .. } => summary.unresolved += 1,
                CallResolutionStatus::Ambiguous { .. } => summary.ambiguous += 1,
                CallResolutionStatus::External { .. } => summary.external += 1,
                CallResolutionStatus::Unsupported { .. } => summary.unsupported += 1,
            }
        }
        summary
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalFunctionPathResolution {
    Resolved(FunctionNodeId),
    Unresolved,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalTypeResolution {
    Resolved(OrdinaryTypeTargetId),
    Unresolved,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalTraitResolution {
    Resolved(TraitNodeId),
    Unresolved,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum AssocPathResolution {
    Resolved(MethodNodeId),
    Unresolved,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ConstructorPathResolution {
    TupleStruct(StructNodeId),
    EnumVariant(VariantNodeId),
    Unresolved,
    Ambiguous,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalModulePathResolution {
    Resolved(ModuleNodeId),
    Unresolved,
    Ambiguous,
}

/// Resolver for the first narrow local call-graph slice.
///
/// Currently supported semantic proof:
///
/// ```text
/// self.method() inside an inherent impl method
///   -> method with the same name in the same impl block
///
/// function() / super::function() / self::function() / crate::module::function()
///   -> local standalone function proven from the module tree
///
/// Self::function() inside an inherent impl method
/// LocalType::function() for a directly visible local type with an inherent impl
///   -> associated function proven from local type-resolution facts
///
/// TupleStruct(args...) / Enum::TupleVariant(args...)
///   -> local tuple constructor proven from local type-resolution facts
/// ```
///
/// The resolver intentionally does not attempt trait dispatch, external calls,
/// dynamic callees, macro expansion, unqualified value-binding calls,
/// imported associated functions, or receiver typing beyond parameters and
/// explicit local binding annotations.
pub struct CallRelationResolver<'a> {
    graph: &'a ParsedCodeGraph,
    tree: &'a ModuleTree,
}

impl<'a> CallRelationResolver<'a> {
    pub fn new(graph: &'a ParsedCodeGraph, tree: &'a ModuleTree) -> Self {
        Self { graph, tree }
    }

    pub fn resolve_call_relations(&self) -> Result<CallResolutionReport, SynParserError> {
        let mut relations = Vec::new();
        let mut statuses = Vec::new();
        let type_report =
            super::type_resolution_v2::resolve_type_relations_after_tree(self.graph, self.tree)?;

        for call in self.graph.call_sites() {
            match call {
                CallNode::MethodCall(method_call) => {
                    self.resolve_method_call(
                        method_call,
                        &type_report.relations,
                        &mut relations,
                        &mut statuses,
                    )?;
                }
                CallNode::PathCall(path_call) => {
                    self.resolve_path_call(
                        path_call,
                        &type_report.relations,
                        &mut relations,
                        &mut statuses,
                    )?;
                }
                CallNode::DynamicCall(dynamic_call) => {
                    self.resolve_dynamic_call(dynamic_call, &mut relations, &mut statuses)?;
                }
                CallNode::MacroCall(macro_call) => {
                    statuses.push(CallResolutionStatus::Unsupported {
                        source: AnyCallSiteId::Macro(macro_call.id),
                    });
                }
            }
        }

        relations.sort_unstable();
        relations.dedup();
        assert_unique_status_sources(&statuses)?;
        statuses.sort_unstable();
        statuses.dedup();
        let summary = CallResolutionSummary::from_statuses(&statuses);

        Ok(CallResolutionReport {
            relations,
            statuses,
            summary,
        })
    }

    fn ordinary_receiver_targets(
        &self,
        target: OrdinaryTypeTargetId,
        type_relations: &[TypeRelation],
    ) -> Result<Vec<OrdinaryTypeTargetId>, SynParserError> {
        let mut targets = Vec::new();
        let mut queue = vec![(target, 0usize)];

        while let Some((current, depth)) = queue.pop() {
            if targets.contains(&current) {
                continue;
            }
            targets.push(current);

            let Ok(alias_id) = TypeAliasNodeId::try_from(current) else {
                continue;
            };
            if depth >= MAX_IMPORT_CHAIN_DEPTH {
                return Err(SynParserError::InternalState(format!(
                    "call resolution exceeded type alias chain depth limit of {MAX_IMPORT_CHAIN_DEPTH} at {alias_id}"
                )));
            }

            let alias_node = self.graph.get_type_alias_checked(alias_id)?;
            let Ok(source) = OrdinaryTypeSourceId::try_from(alias_node.type_id) else {
                continue;
            };

            let mut next = type_relations
                .iter()
                .filter_map(|relation| match relation {
                    TypeRelation::Ordinary {
                        source: relation_source,
                        target,
                    } if *relation_source == source => Some(*target),
                    _ => None,
                })
                .collect::<Vec<_>>();
            next.sort_unstable();
            next.dedup();

            for next_target in next.into_iter().rev() {
                if !targets.contains(&next_target) {
                    queue.push((next_target, depth + 1));
                }
            }
        }

        targets.sort_unstable();
        targets.dedup();
        Ok(targets)
    }

    fn resolve_type_instance_method(
        &self,
        owner: CallBodyOwnerId,
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
                    .filter(|method| {
                        method.name == method_name
                            && method.parameters.iter().any(|param| param.is_self)
                    })
                    .map(|method| method.id),
            );
        }

        if !candidates.is_empty() {
            return Ok(Some(Self::method_resolution(candidates)));
        }

        if let Some(bound_resolution) =
            self.resolve_generic_bound_method(owner, target, method_name, type_relations)?
        {
            return Ok(Some(bound_resolution));
        }

        if let Some(trait_resolution) =
            self.resolve_trait_impl_instance_method(owner, target, method_name, type_relations)?
        {
            return Ok(Some(trait_resolution));
        }

        if matched_inherent_impl {
            Ok(Some(AssocPathResolution::Unresolved))
        } else {
            Ok(None)
        }
    }

    fn resolve_trait_impl_instance_method(
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

    fn where_subject_matches(
        &self,
        predicate: &TypeWherePredicate,
        target: OrdinaryTypeTargetId,
        type_relations: &[TypeRelation],
    ) -> bool {
        let Ok(source) = OrdinaryTypeSourceId::try_from(predicate.subject) else {
            return false;
        };

        type_relations.iter().any(|relation| {
            matches!(
                relation,
                TypeRelation::Ordinary {
                    source: relation_source,
                    target: relation_target,
                } if *relation_source == source && *relation_target == target
            )
        })
    }

    fn bound_trait_targets(
        &self,
        sources: &[TraitTypeSourceId],
        type_relations: &[TypeRelation],
    ) -> Result<Vec<TraitNodeId>, SynParserError> {
        let mut traits = Vec::new();
        for source in sources {
            let mut targets = type_relations
                .iter()
                .filter_map(|relation| match relation {
                    TypeRelation::Trait {
                        source: relation_source,
                        target,
                    } if relation_source == source => TraitNodeId::try_from(*target).ok(),
                    _ => None,
                })
                .collect::<Vec<_>>();
            targets.sort_unstable();
            targets.dedup();
            match targets.as_slice() {
                [target] => traits.push(*target),
                [] => return Ok(Vec::new()),
                _ => {
                    return Err(SynParserError::InternalState(format!(
                        "call resolution found multiple trait targets for bound source {source}"
                    )));
                }
            }
        }
        traits.sort_unstable();
        traits.dedup();
        Ok(traits)
    }

    fn receiver_satisfies_trait_at_depth(
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

    fn trait_is_visible_from_owner(
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

    fn local_trait_node_for_impl(
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

    fn resolve_method_in_impl(
        &self,
        impl_node: &ImplNode,
        method_name: &str,
    ) -> AssocPathResolution {
        Self::method_resolution(
            impl_node
                .methods
                .iter()
                .filter(|method| method.name == method_name)
                .map(|method| method.id)
                .collect(),
        )
    }

    fn resolve_instance_method_in_trait(
        &self,
        trait_node: &TraitNode,
        method_name: &str,
    ) -> AssocPathResolution {
        Self::method_resolution(
            trait_node
                .methods
                .iter()
                .filter(|method| {
                    method.name == method_name
                        && method.parameters.iter().any(|param| param.is_self)
                })
                .map(|method| method.id)
                .collect(),
        )
    }

    fn resolve_associated_function_in_trait(
        &self,
        trait_node: &TraitNode,
        method_name: &str,
    ) -> AssocPathResolution {
        Self::method_resolution(
            trait_node
                .methods
                .iter()
                .filter(|method| {
                    method.name == method_name
                        && !method.parameters.iter().any(|param| param.is_self)
                })
                .map(|method| method.id)
                .collect(),
        )
    }

    fn method_resolution(mut candidates: Vec<MethodNodeId>) -> AssocPathResolution {
        candidates.sort_unstable();
        candidates.dedup();
        match candidates.as_slice() {
            [target] => AssocPathResolution::Resolved(*target),
            [] => AssocPathResolution::Unresolved,
            _ => AssocPathResolution::Ambiguous,
        }
    }

    fn resolve_local_type_segment(
        &self,
        owner: CallBodyOwnerId,
        segment: &str,
    ) -> Result<LocalTypeResolution, SynParserError> {
        let Some(module_id) = self.containing_module_for_owner(owner) else {
            return Ok(LocalTypeResolution::Unresolved);
        };

        let mut candidates = Vec::new();
        self.visit_scope_candidates(module_id, segment, &mut |candidate| {
            if let Some(target) = Self::ordinary_type_target(candidate) {
                candidates.push(target);
            }
            Ok(())
        })?;
        candidates.sort_unstable();
        candidates.dedup();

        Ok(match candidates.as_slice() {
            [target] => LocalTypeResolution::Resolved(*target),
            [] => LocalTypeResolution::Unresolved,
            _ => LocalTypeResolution::Ambiguous,
        })
    }

    fn resolve_local_trait_segment(
        &self,
        owner: CallBodyOwnerId,
        segment: &str,
    ) -> Result<LocalTraitResolution, SynParserError> {
        let Some(module_id) = self.containing_module_for_owner(owner) else {
            return Ok(LocalTraitResolution::Unresolved);
        };

        let mut candidates = Vec::new();
        self.visit_scope_candidates(module_id, segment, &mut |candidate| {
            if let Ok(trait_id) = TraitNodeId::try_from(candidate) {
                candidates.push(trait_id);
            }
            Ok(())
        })?;
        candidates.sort_unstable();
        candidates.dedup();

        Ok(match candidates.as_slice() {
            [target] => LocalTraitResolution::Resolved(*target),
            [] => LocalTraitResolution::Unresolved,
            _ => LocalTraitResolution::Ambiguous,
        })
    }

    fn ordinary_type_target(candidate: AnyNodeId) -> Option<OrdinaryTypeTargetId> {
        match candidate {
            AnyNodeId::Struct(id) => Some(id.into()),
            AnyNodeId::Enum(id) => Some(id.into()),
            AnyNodeId::Union(id) => Some(id.into()),
            AnyNodeId::TypeAlias(id) => Some(id.into()),
            _ => None,
        }
    }

    fn impl_self_target(
        &self,
        impl_node: &ImplNode,
        type_relations: &[TypeRelation],
    ) -> Result<Option<OrdinaryTypeTargetId>, SynParserError> {
        let Ok(source) = OrdinaryTypeSourceId::try_from(impl_node.self_type) else {
            return Ok(None);
        };

        let mut targets = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Ordinary {
                    source: relation_source,
                    target,
                } if *relation_source == source => Some(*target),
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();

        match targets.as_slice() {
            [target] => Ok(Some(*target)),
            [] => Ok(None),
            _ => Err(SynParserError::InternalState(format!(
                "call resolution found multiple self type targets for impl {}",
                impl_node.id
            ))),
        }
    }

    fn is_unqualified_path(&self, path: &[String]) -> bool {
        path.len() == 1
    }

    fn is_explicit_local_path(&self, path: &[String]) -> bool {
        path.len() > 1
            && matches!(
                path.first().map(String::as_str),
                Some("crate" | "self" | "super")
            )
    }

    fn is_external_path(&self, path: &[String]) -> bool {
        path.iter()
            .find(|segment| !segment.is_empty())
            .is_some_and(|segment| {
                matches!(segment.as_str(), "std" | "core" | "alloc")
                    || self.graph.iter_dependency_names().any(|dependency| {
                        dependency == segment || dependency.replace('-', "_") == *segment
                    })
            })
    }

    fn is_external_prelude_path(
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

    fn local_segment_visible(
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

    fn is_external_import_path(
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

    fn import_scope_module(&self, module_id: ModuleNodeId) -> Result<ModuleNodeId, SynParserError> {
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

    fn resolve_unqualified_local_function_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<LocalFunctionPathResolution, SynParserError> {
        self.resolve_implicit_local_function_path(owner, path)
    }

    fn resolve_implicit_local_function_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<LocalFunctionPathResolution, SynParserError> {
        match self.resolve_local_function_path(owner, path)? {
            LocalFunctionPathResolution::Unresolved => Ok(LocalFunctionPathResolution::Unsupported),
            resolution => Ok(resolution),
        }
    }

    fn resolve_local_function_path(
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

    fn start_segment_index(
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

    fn resolve_module_segment(
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

    fn visit_scope_candidates(
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

    fn visit_binding_terminals(
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

    fn containing_module_for_owner(&self, owner: CallBodyOwnerId) -> Option<ModuleNodeId> {
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

    fn resolve_method_call(
        &self,
        call: &MethodCallNode,
        type_relations: &[TypeRelation],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Method(call.id);

        if matches!(call.receiver, MethodCallReceiver::Literal)
            && Self::is_external_literal_method(&call.method_name)
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }
        if let MethodCallReceiver::PathCallResult { path } = &call.receiver
            && self.is_external_path_result_method(call.owner, path, &call.method_name)?
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }
        if let MethodCallReceiver::SelfField { field_path } = &call.receiver
            && self.is_external_self_field_method(call, field_path, type_relations)?
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }
        if let MethodCallReceiver::TypedLocalBinding { type_path, .. }
        | MethodCallReceiver::BorrowedTypedLocalBinding { type_path, .. } = &call.receiver
            && self.is_external_type_path_method(call.owner, type_path, &call.method_name)?
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        let resolution = match &call.receiver {
            MethodCallReceiver::SelfValue => self.resolve_self_method_call(call)?,
            MethodCallReceiver::SelfField { .. } => AssocPathResolution::Unsupported,
            MethodCallReceiver::LocalBinding { name } => {
                self.resolve_param_method_call(call, name, type_relations)?
            }
            MethodCallReceiver::TypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)?
            }
            MethodCallReceiver::InitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)?
            }
            MethodCallReceiver::BorrowedTypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)?
            }
            MethodCallReceiver::DereferencedInitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)?
            }
            MethodCallReceiver::PathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)?
            }
            MethodCallReceiver::AwaitPathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)?
            }
            MethodCallReceiver::TryPathCallResult { path } => {
                self.resolve_try_path_result_method_call(call, path, type_relations)?
            }
            MethodCallReceiver::MethodCallResult { method_name } => {
                self.resolve_method_result_method_call(call, method_name, type_relations)?
            }
            MethodCallReceiver::FieldTypedLocalBinding {
                type_path,
                field_path,
                ..
            } => {
                self.resolve_field_local_method_call(call, type_path, field_path, type_relations)?
            }
            MethodCallReceiver::FieldInitializedLocalBinding {
                init_path,
                field_path,
                ..
            } => {
                self.resolve_field_local_method_call(call, init_path, field_path, type_relations)?
            }
            MethodCallReceiver::DereferencedLocalBinding { name } => {
                self.resolve_dereferenced_param_method_call(call, name, type_relations)?
            }
            MethodCallReceiver::BorrowedLocalBinding { .. }
            | MethodCallReceiver::FieldLocalBinding { .. }
            | MethodCallReceiver::AwaitResult
            | MethodCallReceiver::TryResult
            | MethodCallReceiver::Literal => AssocPathResolution::Unsupported,
        };

        match resolution {
            AssocPathResolution::Resolved(target) => {
                relations.push(CallRelation::Method {
                    source: call.id,
                    target,
                });
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            }
            AssocPathResolution::Unresolved => {
                statuses.push(CallResolutionStatus::Unresolved { source });
            }
            AssocPathResolution::Ambiguous => {
                statuses.push(CallResolutionStatus::Ambiguous { source });
            }
            AssocPathResolution::Unsupported => {
                statuses.push(CallResolutionStatus::Unsupported { source });
            }
        }

        Ok(())
    }

    fn is_external_literal_method(method_name: &str) -> bool {
        matches!(method_name, "to_string")
    }

    fn is_external_self_field_method(
        &self,
        call: &MethodCallNode,
        field_path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        let [field_name] = field_path else {
            return Ok(false);
        };
        let Some(field_type) = self.self_field_type(call.owner, field_name, type_relations)? else {
            return Ok(false);
        };
        self.is_external_type_method(call.owner, field_type, &call.method_name)
    }

    fn self_field_type(
        &self,
        owner: CallBodyOwnerId,
        field_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        let CallBodyOwnerId::Method(owner_method_id) = owner else {
            return Ok(None);
        };

        let Some(impl_id) = self.impl_for_owner_method(owner_method_id)? else {
            return Ok(None);
        };
        let Some(impl_node) = self.maybe_impl_node(impl_id) else {
            return Ok(None);
        };
        let Some(self_target) = self.impl_self_target(impl_node, type_relations)? else {
            return Ok(None);
        };
        let Ok(struct_id) = StructNodeId::try_from(self_target) else {
            return Ok(None);
        };
        let struct_node = self.graph.get_struct_checked(struct_id)?;

        let mut matches = struct_node
            .fields
            .iter()
            .filter(|field| {
                field.name.as_deref().is_some_and(|name| {
                    Self::struct_field_name_matches(name, field_name, &struct_node.name)
                })
            })
            .map(|field| field.type_id)
            .collect::<Vec<_>>();
        matches.sort_unstable();
        matches.dedup();

        Ok(match matches.as_slice() {
            [type_id] => Some(*type_id),
            _ => None,
        })
    }

    fn struct_field_name_matches(actual: &str, expected: &str, struct_name: &str) -> bool {
        if actual == expected {
            return true;
        }

        let marker = format!("unnamed_field{struct_name}");
        actual
            .rsplit_once(&marker)
            .is_some_and(|(prefix, _)| prefix == expected)
    }

    fn is_external_type_method(
        &self,
        owner: CallBodyOwnerId,
        type_id: OrdinaryTypeUseId,
        method_name: &str,
    ) -> Result<bool, SynParserError> {
        let TypeNode::Named(type_node) = self.type_node(type_id)? else {
            return Ok(false);
        };

        if !matches!(method_name, "len") {
            return Ok(false);
        }

        if self.is_external_path(&type_node.path) {
            return Ok(true);
        }

        match type_node.path.as_slice() {
            [segment] if matches!(segment.as_str(), "String" | "Vec") => {
                Ok(!self.local_segment_visible(owner, segment)?)
            }
            _ => Ok(false),
        }
    }

    fn is_external_type_path_method(
        &self,
        owner: CallBodyOwnerId,
        type_path: &[String],
        method_name: &str,
    ) -> Result<bool, SynParserError> {
        if !matches!(method_name, "len") {
            return Ok(false);
        }

        if self.is_external_path(type_path) || self.is_external_import_path(owner, type_path)? {
            return Ok(true);
        }

        match type_path {
            [segment] if matches!(segment.as_str(), "String" | "Vec") => {
                Ok(!self.local_segment_visible(owner, segment)?)
            }
            _ => Ok(false),
        }
    }

    fn is_external_path_result_method(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        method_name: &str,
    ) -> Result<bool, SynParserError> {
        if !matches!(method_name, "unwrap" | "uuid") {
            return Ok(false);
        }

        Ok(self.is_external_path(path) || self.is_external_import_path(owner, path)?)
    }

    fn resolve_self_method_call(
        &self,
        call: &MethodCallNode,
    ) -> Result<AssocPathResolution, SynParserError> {
        let CallBodyOwnerId::Method(owner_method_id) = call.owner else {
            return Ok(AssocPathResolution::Unsupported);
        };

        if let Some(impl_id) = self.impl_for_owner_method(owner_method_id)? {
            let Some(impl_node) = self.maybe_impl_node(impl_id) else {
                return Ok(AssocPathResolution::Unsupported);
            };
            return Ok(self.resolve_method_in_impl(impl_node, &call.method_name));
        }

        if let Some(trait_id) = self.trait_for_owner_method(owner_method_id)? {
            let trait_node = self.graph.get_trait_checked(trait_id)?;
            return Ok(self.resolve_instance_method_in_trait(trait_node, &call.method_name));
        }

        Ok(AssocPathResolution::Unsupported)
    }

    fn resolve_param_method_call(
        &self,
        call: &MethodCallNode,
        name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(parameters) = self.owner_parameters(call.owner)? else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let params = parameters
            .iter()
            .filter(|param| !param.is_self && param.name.as_deref() == Some(name))
            .collect::<Vec<_>>();
        if params.is_empty() {
            return Ok(AssocPathResolution::Unsupported);
        }

        let type_ids = params.iter().map(|param| param.type_id).collect::<Vec<_>>();
        if let Some(resolution) =
            self.resolve_type_use_method(call.owner, &type_ids, &call.method_name, type_relations)?
        {
            return Ok(resolution);
        }

        let mut dereferenced_ids = Vec::new();
        for type_id in &type_ids {
            if let Some(type_id) = self.dereferenced_type_use(*type_id)? {
                dereferenced_ids.push(type_id);
            }
        }
        dereferenced_ids.sort_unstable();
        dereferenced_ids.dedup();
        if let Some(resolution) = self.resolve_type_use_method(
            call.owner,
            &dereferenced_ids,
            &call.method_name,
            type_relations,
        )? {
            return Ok(resolution);
        }

        self.resolve_bound_method_from_types(&type_ids, &call.method_name, type_relations)?
            .map_or(Ok(AssocPathResolution::Unresolved), Ok)
    }

    fn resolve_type_use_method(
        &self,
        owner: CallBodyOwnerId,
        type_ids: &[OrdinaryTypeUseId],
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        if type_ids.is_empty() {
            return Ok(None);
        }

        let sources = type_ids
            .iter()
            .filter_map(|type_id| OrdinaryTypeSourceId::try_from(*type_id).ok())
            .collect::<Vec<_>>();

        let mut targets = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Ordinary { source, target } if sources.contains(source) => {
                    Some(*target)
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();

        match targets.as_slice() {
            [target] => {
                let resolution = self
                    .resolve_type_instance_method(owner, *target, method_name, type_relations)?
                    .unwrap_or(AssocPathResolution::Unsupported);

                if matches!(resolution, AssocPathResolution::Unsupported)
                    && let [type_id] = type_ids
                    && let Some(resolution) = self.resolve_type_use_trait_impl_method(
                        owner,
                        *type_id,
                        *target,
                        method_name,
                        type_relations,
                    )?
                {
                    return Ok(Some(resolution));
                }

                Ok(Some(resolution))
            }
            [] => Ok(None),
            _ => Ok(Some(AssocPathResolution::Ambiguous)),
        }
    }

    fn resolve_type_use_trait_impl_method(
        &self,
        owner: CallBodyOwnerId,
        type_id: OrdinaryTypeUseId,
        target: OrdinaryTypeTargetId,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let TypeNode::Named(receiver) = self.type_node(type_id)? else {
            return Ok(None);
        };
        if receiver.arguments.is_empty() {
            return Ok(None);
        }

        let receiver_targets = self.ordinary_receiver_targets(target, type_relations)?;
        let mut candidates = Vec::new();
        let mut matched = false;

        for impl_node in self
            .graph
            .impls()
            .iter()
            .filter(|impl_node| impl_node.trait_type.is_some())
        {
            let Some(self_target) = self.impl_self_target(impl_node, type_relations)? else {
                continue;
            };
            if !receiver_targets.contains(&self_target) {
                continue;
            }
            if !self.generic_self_args_apply(impl_node, &receiver.arguments, type_relations)? {
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

            matched = true;
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

        if matched {
            Ok(Some(Self::method_resolution(candidates)))
        } else {
            Ok(None)
        }
    }

    fn generic_self_args_apply(
        &self,
        impl_node: &ImplNode,
        args: &[OrdinaryTypeUseId],
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        let TypeNode::Named(self_type) = self.type_node(impl_node.self_type)? else {
            return Ok(false);
        };
        if self_type.arguments.len() != args.len() || self_type.arguments.is_empty() {
            return Ok(false);
        }

        for (impl_arg, arg) in self_type.arguments.iter().zip(args) {
            let Some(param_id) = self.type_param_target(*impl_arg, type_relations)? else {
                return Ok(false);
            };
            if !self.generic_arg_satisfies(impl_node, param_id, *arg, type_relations)? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn type_param_target(
        &self,
        type_id: OrdinaryTypeUseId,
        type_relations: &[TypeRelation],
    ) -> Result<Option<TypeGenericParamNodeId>, SynParserError> {
        let Some(target) = self.single_ordinary_target(type_id, type_relations)? else {
            return Ok(None);
        };
        Ok(TypeGenericParamNodeId::try_from(target).ok())
    }

    fn generic_arg_satisfies(
        &self,
        impl_node: &ImplNode,
        param_id: TypeGenericParamNodeId,
        type_id: OrdinaryTypeUseId,
        type_relations: &[TypeRelation],
    ) -> Result<bool, SynParserError> {
        let Some(target) = self.single_ordinary_target(type_id, type_relations)? else {
            return Ok(false);
        };
        let receiver_targets = self.ordinary_receiver_targets(target, type_relations)?;
        let Some(sources) = self.generic_arg_bounds(impl_node, param_id, type_relations) else {
            return Ok(false);
        };
        if sources.is_empty() {
            return Ok(true);
        }

        let traits = self.bound_trait_targets(&sources, type_relations)?;
        if traits.len() != sources.len() {
            return Ok(false);
        }

        for bound in traits {
            if !self.receiver_satisfies_trait_at_depth(
                &receiver_targets,
                bound,
                type_relations,
                0,
            )? {
                return Ok(false);
            }
        }

        Ok(true)
    }

    fn generic_arg_bounds(
        &self,
        impl_node: &ImplNode,
        param_id: TypeGenericParamNodeId,
        type_relations: &[TypeRelation],
    ) -> Option<Vec<TraitTypeSourceId>> {
        let param = impl_node.generic_params.iter().find(|param| {
            TypeGenericParamNodeId::try_refine(param.id, &param.kind).ok() == Some(param_id)
        })?;

        let mut sources = Vec::new();
        match &param.kind {
            crate::parser::types::GenericParamKind::Type {
                bounds, default, ..
            } => {
                if default.is_some() {
                    return None;
                }
                sources.extend(bounds.iter().copied());
            }
            crate::parser::types::GenericParamKind::Lifetime { .. }
            | crate::parser::types::GenericParamKind::Const { .. } => return None,
        }

        let target = OrdinaryTypeTargetId::from(param_id);
        for predicate in &impl_node.where_predicates {
            if self.where_subject_matches(predicate, target, type_relations) {
                sources.extend(predicate.bounds.iter().copied());
            }
        }

        sources.sort_unstable();
        sources.dedup();
        Some(sources)
    }

    fn single_ordinary_target(
        &self,
        type_id: OrdinaryTypeUseId,
        type_relations: &[TypeRelation],
    ) -> Result<Option<OrdinaryTypeTargetId>, SynParserError> {
        let Ok(source) = OrdinaryTypeSourceId::try_from(type_id) else {
            return Ok(None);
        };

        let mut targets = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Ordinary {
                    source: relation_source,
                    target,
                } if *relation_source == source => Some(*target),
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();

        match targets.as_slice() {
            [target] => Ok(Some(*target)),
            [] => Ok(None),
            _ => Err(SynParserError::InternalState(format!(
                "call resolution found multiple ordinary targets for type use {type_id}"
            ))),
        }
    }

    fn resolve_dereferenced_param_method_call(
        &self,
        call: &MethodCallNode,
        name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(parameters) = self.owner_parameters(call.owner)? else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let params = parameters
            .iter()
            .filter(|param| !param.is_self && param.name.as_deref() == Some(name))
            .collect::<Vec<_>>();
        if params.is_empty() {
            return Ok(AssocPathResolution::Unsupported);
        }

        let mut dereferenced_types = Vec::new();
        for param in params {
            if let Some(type_id) = self.dereferenced_type_use(param.type_id)? {
                dereferenced_types.push(type_id);
            }
        }
        dereferenced_types.sort_unstable();
        dereferenced_types.dedup();
        if dereferenced_types.is_empty() {
            return Ok(AssocPathResolution::Unsupported);
        }

        if let Some(resolution) = self.resolve_type_use_method(
            call.owner,
            &dereferenced_types,
            &call.method_name,
            type_relations,
        )? {
            return Ok(resolution);
        }

        self.resolve_bound_method_from_types(
            &dereferenced_types,
            &call.method_name,
            type_relations,
        )?
        .map_or(Ok(AssocPathResolution::Unresolved), Ok)
    }

    fn dereferenced_type_use(
        &self,
        type_id: OrdinaryTypeUseId,
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        match self.type_node(type_id)? {
            TypeNode::Reference(node) => Ok(Some(node.referenced)),
            TypeNode::Paren(node) => self.dereferenced_type_use(node.inner),
            _ => Ok(None),
        }
    }

    fn resolve_typed_local_method_call(
        &self,
        call: &MethodCallNode,
        type_path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        if type_path.is_empty() || self.is_external_path(type_path) {
            return Ok(AssocPathResolution::Unsupported);
        }

        let resolution = if type_path.len() == 1 || self.is_explicit_local_path(type_path) {
            self.resolve_local_type_path(call.owner, type_path)?
        } else {
            return Ok(AssocPathResolution::Unsupported);
        };

        match resolution {
            LocalTypeResolution::Resolved(target) => self
                .resolve_type_instance_method(
                    call.owner,
                    target,
                    &call.method_name,
                    type_relations,
                )?
                .map_or(Ok(AssocPathResolution::Unsupported), Ok),
            LocalTypeResolution::Unresolved => {
                self.resolve_typed_local_trait_method_call(call.owner, type_path, &call.method_name)
            }
            LocalTypeResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn resolve_typed_local_trait_method_call(
        &self,
        owner: CallBodyOwnerId,
        trait_path: &[String],
        method_name: &str,
    ) -> Result<AssocPathResolution, SynParserError> {
        let [trait_segment] = trait_path else {
            return Ok(AssocPathResolution::Unsupported);
        };

        match self.resolve_local_trait_segment(owner, trait_segment)? {
            LocalTraitResolution::Resolved(trait_id) => {
                let trait_node = self.graph.get_trait_checked(trait_id)?;
                Ok(self.resolve_instance_method_in_trait(trait_node, method_name))
            }
            LocalTraitResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            LocalTraitResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn resolve_path_result_method_call(
        &self,
        call: &MethodCallNode,
        path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        if path.is_empty()
            || self.is_external_path(path)
            || self.is_external_import_path(call.owner, path)?
        {
            return Ok(AssocPathResolution::Unsupported);
        }

        let resolution = if self.is_unqualified_path(path) {
            self.resolve_unqualified_local_function_path(call.owner, path)?
        } else if self.is_explicit_local_path(path) {
            self.resolve_local_function_path(call.owner, path)?
        } else {
            self.resolve_implicit_local_function_path(call.owner, path)?
        };

        match resolution {
            LocalFunctionPathResolution::Resolved(function_id) => self
                .resolve_function_return_type_method(
                    call.owner,
                    function_id,
                    &call.method_name,
                    type_relations,
                ),
            LocalFunctionPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            LocalFunctionPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
            LocalFunctionPathResolution::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn resolve_try_path_result_method_call(
        &self,
        call: &MethodCallNode,
        path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        if path.is_empty()
            || self.is_external_path(path)
            || self.is_external_import_path(call.owner, path)?
        {
            return Ok(AssocPathResolution::Unsupported);
        }

        let resolution = if self.is_unqualified_path(path) {
            self.resolve_unqualified_local_function_path(call.owner, path)?
        } else if self.is_explicit_local_path(path) {
            self.resolve_local_function_path(call.owner, path)?
        } else {
            self.resolve_implicit_local_function_path(call.owner, path)?
        };

        match resolution {
            LocalFunctionPathResolution::Resolved(function_id) => self
                .resolve_result_ok_return_type_method(
                    call.owner,
                    function_id,
                    &call.method_name,
                    type_relations,
                ),
            LocalFunctionPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            LocalFunctionPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
            LocalFunctionPathResolution::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn resolve_method_result_method_call(
        &self,
        call: &MethodCallNode,
        inner_method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(inner_call) = self.direct_inner_method_call(call, inner_method_name) else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let inner_resolution = self.resolve_method_call_target(inner_call, type_relations)?;
        match inner_resolution {
            AssocPathResolution::Resolved(method_id) => self.resolve_method_return_type_method(
                call.owner,
                method_id,
                &call.method_name,
                type_relations,
            ),
            AssocPathResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            AssocPathResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
            AssocPathResolution::Unsupported => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn direct_inner_method_call(
        &self,
        call: &MethodCallNode,
        inner_method_name: &str,
    ) -> Option<&MethodCallNode> {
        let mut candidates = self
            .graph
            .call_sites()
            .iter()
            .filter_map(|candidate| match candidate {
                CallNode::MethodCall(inner)
                    if inner.owner == call.owner
                        && inner.method_name == inner_method_name
                        && inner.span.0 == call.span.0
                        && inner.span.1 < call.span.1 =>
                {
                    Some(inner)
                }
                _ => None,
            })
            .collect::<Vec<_>>();

        let max_end = candidates.iter().map(|inner| inner.span.1).max()?;
        candidates.retain(|inner| inner.span.1 == max_end);
        match candidates.as_slice() {
            [inner] => Some(*inner),
            _ => None,
        }
    }

    fn resolve_method_call_target(
        &self,
        call: &MethodCallNode,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        match &call.receiver {
            MethodCallReceiver::SelfValue => self.resolve_self_method_call(call),
            MethodCallReceiver::SelfField { .. } => Ok(AssocPathResolution::Unsupported),
            MethodCallReceiver::LocalBinding { name } => {
                self.resolve_param_method_call(call, name, type_relations)
            }
            MethodCallReceiver::TypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)
            }
            MethodCallReceiver::InitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)
            }
            MethodCallReceiver::BorrowedTypedLocalBinding { type_path, .. } => {
                self.resolve_typed_local_method_call(call, type_path, type_relations)
            }
            MethodCallReceiver::DereferencedInitializedLocalBinding { init_path, .. } => {
                self.resolve_typed_local_method_call(call, init_path, type_relations)
            }
            MethodCallReceiver::PathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)
            }
            MethodCallReceiver::AwaitPathCallResult { path } => {
                self.resolve_path_result_method_call(call, path, type_relations)
            }
            MethodCallReceiver::TryPathCallResult { path } => {
                self.resolve_try_path_result_method_call(call, path, type_relations)
            }
            MethodCallReceiver::MethodCallResult { method_name } => {
                self.resolve_method_result_method_call(call, method_name, type_relations)
            }
            MethodCallReceiver::FieldTypedLocalBinding {
                type_path,
                field_path,
                ..
            } => self.resolve_field_local_method_call(call, type_path, field_path, type_relations),
            MethodCallReceiver::FieldInitializedLocalBinding {
                init_path,
                field_path,
                ..
            } => self.resolve_field_local_method_call(call, init_path, field_path, type_relations),
            MethodCallReceiver::DereferencedLocalBinding { name } => {
                self.resolve_dereferenced_param_method_call(call, name, type_relations)
            }
            MethodCallReceiver::BorrowedLocalBinding { .. }
            | MethodCallReceiver::FieldLocalBinding { .. }
            | MethodCallReceiver::AwaitResult
            | MethodCallReceiver::TryResult
            | MethodCallReceiver::Literal => Ok(AssocPathResolution::Unsupported),
        }
    }

    fn resolve_function_return_type_method(
        &self,
        owner: CallBodyOwnerId,
        function_id: FunctionNodeId,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(return_type) = self.function_return_type(function_id)? else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let Ok(source) = OrdinaryTypeSourceId::try_from(return_type) else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let mut targets = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Ordinary {
                    source: relation_source,
                    target,
                } if *relation_source == source => Some(*target),
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();

        match targets.as_slice() {
            [target] => self
                .resolve_type_instance_method(owner, *target, method_name, type_relations)?
                .map_or(Ok(AssocPathResolution::Unsupported), Ok),
            [] => Ok(AssocPathResolution::Unsupported),
            _ => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn resolve_method_return_type_method(
        &self,
        owner: CallBodyOwnerId,
        method_id: MethodNodeId,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(return_type) = self.method_return_type(method_id)? else {
            return Ok(AssocPathResolution::Unsupported);
        };

        if let Some(target) =
            self.method_self_return_target(method_id, return_type, type_relations)?
        {
            return self
                .resolve_type_instance_method(owner, target, method_name, type_relations)?
                .map_or(Ok(AssocPathResolution::Unsupported), Ok);
        }

        let Ok(source) = OrdinaryTypeSourceId::try_from(return_type) else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let mut targets = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Ordinary {
                    source: relation_source,
                    target,
                } if *relation_source == source => Some(*target),
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();

        match targets.as_slice() {
            [target] => self
                .resolve_type_instance_method(owner, *target, method_name, type_relations)?
                .map_or(Ok(AssocPathResolution::Unsupported), Ok),
            [] => Ok(AssocPathResolution::Unsupported),
            _ => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn method_self_return_target(
        &self,
        method_id: MethodNodeId,
        return_type: OrdinaryTypeUseId,
        type_relations: &[TypeRelation],
    ) -> Result<Option<OrdinaryTypeTargetId>, SynParserError> {
        let TypeNode::Named(type_node) = self.type_node(return_type)? else {
            return Ok(None);
        };
        if type_node.path.as_slice() != ["Self"] {
            return Ok(None);
        }

        let Some(impl_id) = self.impl_for_owner_method(method_id)? else {
            return Ok(None);
        };
        let Some(impl_node) = self.maybe_impl_node(impl_id) else {
            return Ok(None);
        };
        self.impl_self_target(impl_node, type_relations)
    }

    fn resolve_result_ok_return_type_method(
        &self,
        owner: CallBodyOwnerId,
        function_id: FunctionNodeId,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(return_type) = self.function_return_type(function_id)? else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let Some(ok_type) = self.result_ok_type(owner, return_type)? else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let Ok(source) = OrdinaryTypeSourceId::try_from(ok_type) else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let mut targets = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Ordinary {
                    source: relation_source,
                    target,
                } if *relation_source == source => Some(*target),
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();

        match targets.as_slice() {
            [target] => self
                .resolve_type_instance_method(owner, *target, method_name, type_relations)?
                .map_or(Ok(AssocPathResolution::Unsupported), Ok),
            [] => Ok(AssocPathResolution::Unsupported),
            _ => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn result_ok_type(
        &self,
        owner: CallBodyOwnerId,
        return_type: OrdinaryTypeUseId,
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        let TypeNode::Named(type_node) = self.type_node(return_type)? else {
            return Ok(None);
        };

        let path = type_node
            .path
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>();
        match path.as_slice() {
            ["Result"] => {
                if self.local_segment_visible(owner, "Result")? {
                    return Ok(None);
                }
            }
            ["std", "result", "Result"] | ["core", "result", "Result"] => {}
            _ => return Ok(None),
        }

        Ok(match type_node.arguments.as_slice() {
            [ok_type, _err_type] => Some(*ok_type),
            _ => None,
        })
    }

    fn resolve_field_local_method_call(
        &self,
        call: &MethodCallNode,
        root_path: &[String],
        field_path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        if root_path.is_empty()
            || self.is_external_path(root_path)
            || self.is_external_import_path(call.owner, root_path)?
        {
            return Ok(AssocPathResolution::Unsupported);
        }

        let root_resolution = if root_path.len() == 1 || self.is_explicit_local_path(root_path) {
            self.resolve_local_type_path(call.owner, root_path)?
        } else {
            return Ok(AssocPathResolution::Unsupported);
        };

        match root_resolution {
            LocalTypeResolution::Resolved(root_target) => self.resolve_field_type_method(
                call.owner,
                root_target,
                field_path,
                &call.method_name,
                type_relations,
            ),
            LocalTypeResolution::Unresolved => Ok(AssocPathResolution::Unresolved),
            LocalTypeResolution::Ambiguous => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn resolve_field_type_method(
        &self,
        owner: CallBodyOwnerId,
        root_target: OrdinaryTypeTargetId,
        field_path: &[String],
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(field_type) = self.field_type(root_target, field_path)? else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let Ok(source) = OrdinaryTypeSourceId::try_from(field_type) else {
            return Ok(AssocPathResolution::Unsupported);
        };

        let mut targets = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Ordinary {
                    source: relation_source,
                    target,
                } if *relation_source == source => Some(*target),
                _ => None,
            })
            .collect::<Vec<_>>();
        targets.sort_unstable();
        targets.dedup();

        match targets.as_slice() {
            [target] => self
                .resolve_type_instance_method(owner, *target, method_name, type_relations)?
                .map_or(Ok(AssocPathResolution::Unsupported), Ok),
            [] => Ok(AssocPathResolution::Unsupported),
            _ => Ok(AssocPathResolution::Ambiguous),
        }
    }

    fn field_type(
        &self,
        root_target: OrdinaryTypeTargetId,
        field_path: &[String],
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        let [field_name] = field_path else {
            return Ok(None);
        };
        let Ok(struct_id) = StructNodeId::try_from(root_target) else {
            return Ok(None);
        };
        let struct_node = self.graph.get_struct_checked(struct_id)?;

        if let Ok(field_idx) = field_name.parse::<usize>() {
            return Ok(struct_node.fields.get(field_idx).map(|field| field.type_id));
        }

        let mut matches = struct_node
            .fields
            .iter()
            .filter(|field| {
                field.name.as_deref().is_some_and(|name| {
                    Self::struct_field_name_matches(name, field_name, &struct_node.name)
                })
            })
            .map(|field| field.type_id)
            .collect::<Vec<_>>();
        matches.sort_unstable();
        matches.dedup();

        Ok(match matches.as_slice() {
            [type_id] => Some(*type_id),
            _ => None,
        })
    }

    fn resolve_generic_bound_method(
        &self,
        owner: CallBodyOwnerId,
        target: OrdinaryTypeTargetId,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let Ok(param_id) = TypeGenericParamNodeId::try_from(target) else {
            return Ok(None);
        };

        let traits = self.generic_bound_traits(owner, target, param_id, type_relations)?;
        if traits.is_empty() {
            return Ok(None);
        }

        let mut candidates = Vec::new();
        for trait_id in traits {
            let trait_node = self.graph.get_trait_checked(trait_id)?;
            candidates.extend(
                trait_node
                    .methods
                    .iter()
                    .filter(|method| {
                        method.name == method_name
                            && method.parameters.iter().any(|param| param.is_self)
                    })
                    .map(|method| method.id),
            );
        }

        Ok(Some(Self::method_resolution(candidates)))
    }

    fn generic_bound_traits(
        &self,
        owner: CallBodyOwnerId,
        target: OrdinaryTypeTargetId,
        param_id: TypeGenericParamNodeId,
        type_relations: &[TypeRelation],
    ) -> Result<Vec<TraitNodeId>, SynParserError> {
        let Some((params, predicates)) = self.owner_generic_bounds(owner)? else {
            return Ok(Vec::new());
        };

        let mut sources = Vec::new();
        for param in params {
            if TypeGenericParamNodeId::try_refine(param.id, &param.kind).ok() == Some(param_id)
                && let Some(bounds) = param.kind.bounds()
            {
                sources.extend(bounds.iter().copied());
            }
        }

        for predicate in predicates {
            let Ok(source) = OrdinaryTypeSourceId::try_from(predicate.subject) else {
                continue;
            };
            if type_relations.iter().any(|relation| {
                matches!(
                    relation,
                    TypeRelation::Ordinary {
                        source: relation_source,
                        target: relation_target,
                    } if *relation_source == source && *relation_target == target
                )
            }) {
                sources.extend(predicate.bounds.iter().copied());
            }
        }

        sources.sort_unstable();
        sources.dedup();

        let mut traits = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Trait { source, target } if sources.contains(source) => {
                    TraitNodeId::try_from(*target).ok()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        traits.sort_unstable();
        traits.dedup();

        Ok(traits)
    }

    fn resolve_bound_method_from_types(
        &self,
        type_ids: &[OrdinaryTypeUseId],
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let mut sources = Vec::new();
        for type_id in type_ids {
            self.collect_bound_sources(*type_id, &mut sources)?;
        }
        self.resolve_trait_method_sources(&sources, method_name, type_relations)
    }

    fn collect_bound_sources(
        &self,
        type_id: OrdinaryTypeUseId,
        sources: &mut Vec<TraitTypeSourceId>,
    ) -> Result<(), SynParserError> {
        match self.type_node(type_id)? {
            TypeNode::Reference(node) => self.collect_bound_sources(node.referenced, sources),
            TypeNode::Paren(node) => self.collect_bound_sources(node.inner, sources),
            TypeNode::TraitObject(node) => {
                sources.extend(node.bounds.iter().copied());
                Ok(())
            }
            TypeNode::ImplTrait(node) => {
                sources.extend(node.bounds.iter().copied());
                Ok(())
            }
            _ => Ok(()),
        }
    }

    fn resolve_trait_method_sources(
        &self,
        sources: &[TraitTypeSourceId],
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        if sources.is_empty() {
            return Ok(None);
        }

        let mut traits = type_relations
            .iter()
            .filter_map(|relation| match relation {
                TypeRelation::Trait { source, target } if sources.contains(source) => {
                    TraitNodeId::try_from(*target).ok()
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        traits.sort_unstable();
        traits.dedup();

        if traits.is_empty() {
            return Ok(None);
        }

        let mut candidates = Vec::new();
        for trait_id in traits {
            let trait_node = self.graph.get_trait_checked(trait_id)?;
            candidates.extend(
                trait_node
                    .methods
                    .iter()
                    .filter(|method| {
                        method.name == method_name
                            && method.parameters.iter().any(|param| param.is_self)
                    })
                    .map(|method| method.id),
            );
        }

        Ok(Some(Self::method_resolution(candidates)))
    }

    fn owner_generic_bounds(
        &self,
        owner: CallBodyOwnerId,
    ) -> Result<Option<(&[GenericParamNode], &[TypeWherePredicate])>, SynParserError> {
        match owner {
            CallBodyOwnerId::Function(id) => self
                .graph
                .functions()
                .iter()
                .find(|node| node.id == id)
                .map(|node| Some((node.generic_params.as_slice(), node.where_predicates.as_slice())))
                .ok_or_else(|| {
                    SynParserError::InternalState(format!(
                        "call resolution found call owned by missing function {id}"
                    ))
                }),
            CallBodyOwnerId::Method(id) => self
                .graph
                .find_node_unique(id.as_any())
                .map_err(|err| {
                    SynParserError::InternalState(format!(
                        "call resolution found call owned by missing or non-unique method {id}: {err}"
                    ))
                })
                .and_then(|node| {
                    node.as_method()
                        .map(|node| {
                            Some((node.generic_params.as_slice(), node.where_predicates.as_slice()))
                        })
                        .ok_or_else(|| {
                            SynParserError::InternalState(format!(
                                "call resolution owner {id} did not resolve to a method node"
                            ))
                        })
                }),
            CallBodyOwnerId::Const(_) | CallBodyOwnerId::Static(_) => Ok(None),
        }
    }

    fn type_node(&self, type_id: impl Into<AnyTypeId>) -> Result<&TypeNode, SynParserError> {
        let type_id = type_id.into();
        self.graph
            .type_graph()
            .iter()
            .find(|node| node.id() == type_id)
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "call resolution found missing type node {type_id:?}"
                ))
            })
    }

    fn resolve_local_type_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<LocalTypeResolution, SynParserError> {
        let Some(mut current_module) = self.containing_module_for_owner(owner) else {
            return Ok(LocalTypeResolution::Unresolved);
        };

        let start_idx = self.start_segment_index(path, &mut current_module)?;
        if start_idx >= path.len() {
            return Ok(LocalTypeResolution::Unresolved);
        }

        for idx in start_idx..path.len() {
            let segment = path[idx].as_str();
            let is_last = idx == path.len() - 1;
            if is_last {
                return self.resolve_terminal_type(current_module, segment);
            }

            current_module = match self.resolve_module_segment(current_module, segment)? {
                LocalModulePathResolution::Resolved(module_id) => module_id,
                LocalModulePathResolution::Unresolved => {
                    return Ok(LocalTypeResolution::Unresolved);
                }
                LocalModulePathResolution::Ambiguous => return Ok(LocalTypeResolution::Ambiguous),
            };
        }

        Ok(LocalTypeResolution::Unresolved)
    }

    fn resolve_terminal_type(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
    ) -> Result<LocalTypeResolution, SynParserError> {
        let mut candidates = Vec::new();
        self.visit_scope_candidates(module_id, segment, &mut |candidate| {
            if let Some(target) = Self::ordinary_type_target(candidate) {
                candidates.push(target);
            }
            Ok(())
        })?;
        candidates.sort_unstable();
        candidates.dedup();

        Ok(match candidates.as_slice() {
            [target] => LocalTypeResolution::Resolved(*target),
            [] => LocalTypeResolution::Unresolved,
            _ => LocalTypeResolution::Ambiguous,
        })
    }

    fn impl_for_owner_method(
        &self,
        method_id: MethodNodeId,
    ) -> Result<Option<ImplNodeId>, SynParserError> {
        let target = AssociatedItemNodeId::from(method_id);
        let mut owner_impl = None;

        for relation in self.tree.get_iter_relations_to(&method_id.as_any()) {
            match relation.rel() {
                SyntacticRelation::ImplAssociatedItem {
                    source,
                    target: relation_target,
                } if *relation_target == target => {
                    if let Some(existing) = owner_impl
                        && existing != *source
                    {
                        return Err(SynParserError::InternalState(format!(
                            "method {method_id} belongs to multiple impls: {existing} and {source}"
                        )));
                    }
                    owner_impl = Some(*source);
                }
                SyntacticRelation::TraitAssociatedItem {
                    target: relation_target,
                    ..
                } if *relation_target == target => {
                    return Ok(None);
                }
                _ => {}
            }
        }

        Ok(owner_impl)
    }

    fn trait_for_owner_method(
        &self,
        method_id: MethodNodeId,
    ) -> Result<Option<TraitNodeId>, SynParserError> {
        let target = AssociatedItemNodeId::from(method_id);
        let mut owner_trait = None;

        for relation in self.tree.get_iter_relations_to(&method_id.as_any()) {
            match relation.rel() {
                SyntacticRelation::TraitAssociatedItem {
                    source,
                    target: relation_target,
                } if *relation_target == target => {
                    if let Some(existing) = owner_trait
                        && existing != *source
                    {
                        return Err(SynParserError::InternalState(format!(
                            "method {method_id} belongs to multiple traits: {existing} and {source}"
                        )));
                    }
                    owner_trait = Some(*source);
                }
                SyntacticRelation::ImplAssociatedItem {
                    target: relation_target,
                    ..
                } if *relation_target == target => {
                    return Ok(None);
                }
                _ => {}
            }
        }

        Ok(owner_trait)
    }

    fn maybe_impl_node(&self, impl_id: ImplNodeId) -> Option<&ImplNode> {
        self.graph.impls().iter().find(|node| node.id == impl_id)
    }

    fn function_return_type(
        &self,
        function_id: FunctionNodeId,
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        self.graph
            .functions()
            .iter()
            .find(|node| node.id == function_id)
            .map(|node| node.return_type)
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "call resolution found function path result to missing function {function_id}"
                ))
            })
    }

    fn method_return_type(
        &self,
        method_id: MethodNodeId,
    ) -> Result<Option<OrdinaryTypeUseId>, SynParserError> {
        let mut return_types = self
            .graph
            .impls()
            .iter()
            .flat_map(|node| node.methods.iter())
            .chain(
                self.graph
                    .traits()
                    .iter()
                    .flat_map(|node| node.methods.iter()),
            )
            .filter(|method| method.id == method_id)
            .map(|method| method.return_type)
            .collect::<Vec<_>>();
        return_types.sort_unstable();
        return_types.dedup();

        match return_types.as_slice() {
            [return_type] => Ok(*return_type),
            [] => Err(SynParserError::InternalState(format!(
                "call resolution found method result to missing method {method_id}"
            ))),
            _ => Err(SynParserError::InternalState(format!(
                "call resolution found multiple return types for method {method_id}"
            ))),
        }
    }

    fn owner_parameters(
        &self,
        owner: CallBodyOwnerId,
    ) -> Result<Option<&[ParamData]>, SynParserError> {
        match owner {
            CallBodyOwnerId::Function(id) => self
                .graph
                .functions()
                .iter()
                .find(|node| node.id == id)
                .map(|node| node.parameters.as_slice())
                .map(Some)
                .ok_or_else(|| {
                    SynParserError::InternalState(format!(
                        "call resolution found call owned by missing function {id}"
                    ))
                }),
            CallBodyOwnerId::Method(id) => self
                .graph
                .find_node_unique(id.as_any())
                .map_err(|err| {
                    SynParserError::InternalState(format!(
                        "call resolution found call owned by missing or non-unique method {id}: {err}"
                    ))
                })
                .and_then(|node| {
                    node.as_method()
                        .map(|node| Some(node.parameters.as_slice()))
                        .ok_or_else(|| {
                            SynParserError::InternalState(format!(
                                "call resolution owner {id} did not resolve to a method node"
                            ))
                        })
                }),
            CallBodyOwnerId::Const(_) | CallBodyOwnerId::Static(_) => Ok(None),
        }
    }
}

fn assert_unique_status_sources(statuses: &[CallResolutionStatus]) -> Result<(), SynParserError> {
    let mut by_source = BTreeMap::new();
    for status in statuses {
        if let Some(existing) = by_source.insert(status.source(), status) {
            return Err(SynParserError::InternalState(format!(
                "duplicate call_resolution_status for {}: first={:?}, second={:?}",
                status.source(),
                existing,
                status
            )));
        }
    }
    Ok(())
}

fn trait_declares_instance_method(trait_node: &TraitNode, method_name: &str) -> bool {
    trait_node.methods.iter().any(|method| {
        method.name == method_name && method.parameters.iter().any(|param| param.is_self)
    })
}

/// Resolves structural call sites into typed semantic call facts after the
/// `ModuleTree` has been built and pruned.
pub fn resolve_call_relations_after_tree(
    graph: &ParsedCodeGraph,
    tree: &ModuleTree,
) -> Result<CallResolutionReport, SynParserError> {
    CallRelationResolver::new(graph, tree).resolve_call_relations()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::nodes::{
        CallSiteKind, FunctionNodeId, PathCallSiteId,
        test_ids::{TestCallIds, TestIds, generate_test_call_id},
    };
    use ploke_core::NodeId;
    use uuid::Uuid;

    #[test]
    fn duplicate_status_sources_are_rejected_before_dedup() {
        let owner = CallBodyOwnerId::Function(FunctionNodeId::new_test(NodeId::Synthetic(
            Uuid::from_u128(1),
        )));
        let source = AnyCallSiteId::Path(PathCallSiteId::new_call_test(generate_test_call_id(
            owner,
            CallSiteKind::Path,
            "duplicated",
            (10, 20),
            &[],
        )));
        let statuses = [
            CallResolutionStatus::Unresolved { source },
            CallResolutionStatus::Unresolved { source },
        ];

        let err = assert_unique_status_sources(&statuses)
            .expect_err("duplicate call-site status should be rejected before dedup");
        assert!(
            matches!(
                err,
                SynParserError::InternalState(ref message)
                    if message.contains("duplicate call_resolution_status")
                        && message.contains(&source.to_string())
            ),
            "unexpected invariant error: {err:?}"
        );
    }
}
