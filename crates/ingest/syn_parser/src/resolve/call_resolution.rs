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
            AnyCallSiteId, AnyNodeId, AnyTypeId, AsAnyNodeId, CallBodyOwnerId, CallNode,
            ExecutableWherePredicate, FunctionNodeId, ImplNode, MethodNodeId, ModuleNodeId,
            OrdinaryTypeSourceId, OrdinaryTypeTargetId, OrdinaryTypeUseId, StructNodeId, TraitNode,
            TraitNodeId, TraitTypeSourceId, TypeAliasNodeId, TypeGenericParamNodeId, VariantNodeId,
        },
        relations::{CallRelation, CallResolutionStatus, TypeRelation},
        types::{GenericParamNode, TypeNode, TypeWherePredicate},
    },
};

use super::module_tree::ModuleTree;

mod associated;
mod constructors;
mod dynamic;
mod external_summary;
mod method;
mod owners;
mod path;
mod scope;
mod traits;

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

/// Parsed workspace crates available as dependency roots during call resolution.
#[derive(Debug, Clone, Copy)]
pub struct CallWorkspace<'a> {
    pub crates: &'a [WorkspaceCrate<'a>],
}

/// One parsed dependency crate that may satisfy a dependency-root path.
#[derive(Debug, Clone, Copy)]
pub struct WorkspaceCrate<'a> {
    pub dependency_name: &'a str,
    pub graph: &'a ParsedCodeGraph,
    pub tree: &'a ModuleTree,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LocalFunctionPathResolution {
    Resolved(FunctionNodeId),
    Unresolved,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum LocalTypeResolution {
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

#[derive(Debug, Clone, Copy)]
struct WorkspaceTypeTarget<'a> {
    krate: WorkspaceCrate<'a>,
    target: OrdinaryTypeTargetId,
}

#[derive(Debug, Clone, Copy)]
enum WorkspaceTypeResolution<'a> {
    Resolved(WorkspaceTypeTarget<'a>),
    Unresolved,
    Ambiguous,
}

#[derive(Debug, Clone, Copy)]
struct GenericBoundScope<'a> {
    params: &'a [GenericParamNode],
    predicates: &'a [TypeWherePredicate],
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
/// DependencyType::function() for a type proven through a parsed workspace import
///   -> associated function proven from type-resolution facts
///
/// TupleStruct(args...) / Enum::TupleVariant(args...)
///   -> local tuple constructor proven from local type-resolution facts
/// ```
///
/// The resolver intentionally does not attempt macro expansion, arbitrary
/// dynamic dispatch, or receiver typing beyond the current conservative proof
/// carriers.
pub struct CallRelationResolver<'a> {
    graph: &'a ParsedCodeGraph,
    tree: &'a ModuleTree,
    workspace: Option<CallWorkspace<'a>>,
}

impl<'a> CallRelationResolver<'a> {
    pub fn new(graph: &'a ParsedCodeGraph, tree: &'a ModuleTree) -> Self {
        Self {
            graph,
            tree,
            workspace: None,
        }
    }

    pub fn with_workspace(mut self, workspace: CallWorkspace<'a>) -> Self {
        self.workspace = Some(workspace);
        self
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
                    self.resolve_dynamic_call(
                        dynamic_call,
                        &type_report.relations,
                        &mut relations,
                        &mut statuses,
                    )?;
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
        assert_unique_status_sources(&statuses, Some(self.graph))?;
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
        let inherent =
            self.resolve_inherent_instance_method(target, method_name, type_relations)?;
        if let Some(resolution) = inherent
            && !matches!(resolution, AssocPathResolution::Unresolved)
        {
            return Ok(Some(resolution));
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

        if inherent.is_some() {
            Ok(Some(AssocPathResolution::Unresolved))
        } else {
            Ok(None)
        }
    }

    fn resolve_inherent_instance_method(
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
                    .filter(|method| {
                        method.name == method_name
                            && method.parameters.iter().any(|param| param.is_self)
                    })
                    .map(|method| method.id),
            );
        }

        if !candidates.is_empty() {
            Ok(Some(Self::method_resolution(candidates)))
        } else if matched_inherent_impl {
            Ok(Some(AssocPathResolution::Unresolved))
        } else {
            Ok(None)
        }
    }

    fn resolve_named_self_inherent_method(
        &self,
        owner_impl: &ImplNode,
        method_name: &str,
    ) -> Result<Option<AssocPathResolution>, SynParserError> {
        let TypeNode::Named(owner_self) = self.type_node(owner_impl.self_type)? else {
            return Ok(None);
        };

        let mut candidates = Vec::new();
        let mut matched_inherent_impl = false;
        for impl_node in self
            .graph
            .impls()
            .iter()
            .filter(|impl_node| impl_node.trait_type.is_none())
        {
            let TypeNode::Named(self_type) = self.type_node(impl_node.self_type)? else {
                continue;
            };
            if self_type.path != owner_self.path {
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

        if matched_inherent_impl {
            Ok(Some(Self::method_resolution(candidates)))
        } else {
            Ok(None)
        }
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

    fn resolve_trait_path_item(
        &self,
        trait_node: &TraitNode,
        method_name: &str,
        arg_count: usize,
    ) -> AssocPathResolution {
        Self::method_resolution(
            trait_node
                .methods
                .iter()
                .filter(|method| method.name == method_name && method.parameters.len() == arg_count)
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
        let module_id = self.import_scope_module(module_id)?;

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

    fn resolve_trait_path_from_root(
        &self,
        path: &[String],
    ) -> Result<LocalTraitResolution, SynParserError> {
        if path.is_empty() {
            return Ok(LocalTraitResolution::Unresolved);
        }

        let mut current_module = self.tree.root();
        let start_idx = if path.first().is_some_and(|segment| segment == "crate") {
            1
        } else {
            0
        };
        if start_idx >= path.len() {
            return Ok(LocalTraitResolution::Unresolved);
        }

        for idx in start_idx..path.len() {
            let segment = path[idx].as_str();
            let is_last = idx == path.len() - 1;
            if is_last {
                return self.resolve_terminal_trait(current_module, segment);
            }

            current_module = match self.resolve_module_segment(current_module, segment)? {
                LocalModulePathResolution::Resolved(module_id) => module_id,
                LocalModulePathResolution::Unresolved => {
                    return Ok(LocalTraitResolution::Unresolved);
                }
                LocalModulePathResolution::Ambiguous => return Ok(LocalTraitResolution::Ambiguous),
            };
        }

        Ok(LocalTraitResolution::Unresolved)
    }

    fn resolve_type_path_from_root(
        &self,
        path: &[String],
    ) -> Result<LocalTypeResolution, SynParserError> {
        if path.is_empty() {
            return Ok(LocalTypeResolution::Unresolved);
        }

        let mut current_module = self.tree.root();
        let start_idx = if path.first().is_some_and(|segment| segment == "crate") {
            1
        } else {
            0
        };
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

    fn resolve_module_path_from_root(
        &self,
        path: &[String],
    ) -> Result<LocalModulePathResolution, SynParserError> {
        let mut current_module = self.tree.root();
        if path.is_empty() {
            return Ok(LocalModulePathResolution::Resolved(current_module));
        }

        let start_idx = if path.first().is_some_and(|segment| segment == "crate") {
            1
        } else {
            0
        };
        if start_idx >= path.len() {
            return Ok(LocalModulePathResolution::Resolved(current_module));
        }

        for segment in &path[start_idx..] {
            current_module = match self.resolve_module_segment(current_module, segment)? {
                LocalModulePathResolution::Resolved(module_id) => module_id,
                LocalModulePathResolution::Unresolved => {
                    return Ok(LocalModulePathResolution::Unresolved);
                }
                LocalModulePathResolution::Ambiguous => {
                    return Ok(LocalModulePathResolution::Ambiguous);
                }
            };
        }

        Ok(LocalModulePathResolution::Resolved(current_module))
    }

    fn resolve_terminal_trait(
        &self,
        module_id: ModuleNodeId,
        segment: &str,
    ) -> Result<LocalTraitResolution, SynParserError> {
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
        if !self.type_use_is_self(return_type)? {
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

    fn type_use_is_self(&self, type_id: OrdinaryTypeUseId) -> Result<bool, SynParserError> {
        match self.type_node(type_id)? {
            TypeNode::Named(type_node) => Ok(type_node.path.as_slice() == ["Self"]),
            TypeNode::Paren(node) => self.type_use_is_self(node.inner),
            _ => Ok(false),
        }
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

    fn resolve_method_result_ok_return_type_method(
        &self,
        owner: CallBodyOwnerId,
        method_id: MethodNodeId,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(return_type) = self.method_return_type(method_id)? else {
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

    fn resolve_method_option_some_return_type_method(
        &self,
        owner: CallBodyOwnerId,
        method_id: MethodNodeId,
        associated_type_target: Option<OrdinaryTypeTargetId>,
        method_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<AssocPathResolution, SynParserError> {
        let Some(return_type) = self.method_return_type(method_id)? else {
            return Ok(AssocPathResolution::Unsupported);
        };
        let Some(some_type) = self.option_some_type(owner, return_type)? else {
            return Ok(AssocPathResolution::Unsupported);
        };

        if let Some(target) =
            self.method_self_return_target(method_id, some_type, type_relations)?
        {
            return self
                .resolve_type_instance_method(owner, target, method_name, type_relations)?
                .map_or(Ok(AssocPathResolution::Unsupported), Ok);
        }
        if let Some(target) = associated_type_target
            && self.type_use_is_self(some_type)?
        {
            return self
                .resolve_type_instance_method(owner, target, method_name, type_relations)?
                .map_or(Ok(AssocPathResolution::Unsupported), Ok);
        }

        let Ok(source) = OrdinaryTypeSourceId::try_from(some_type) else {
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

    fn option_some_type(
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
            ["Option"] => {
                if self.local_segment_visible(owner, "Option")? {
                    return Ok(None);
                }
            }
            ["std", "option", "Option"] | ["core", "option", "Option"] => {}
            _ => return Ok(None),
        }

        Ok(match type_node.arguments.as_slice() {
            [some_type] => Some(*some_type),
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

        let traits = self.generic_bound_traits(owner, target, Some(param_id), type_relations)?;
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
        param_id: Option<TypeGenericParamNodeId>,
        type_relations: &[TypeRelation],
    ) -> Result<Vec<TraitNodeId>, SynParserError> {
        let sources = self.generic_bound_sources(owner, target, param_id, type_relations)?;
        self.bound_traits_from_sources(&sources, type_relations)
    }

    fn generic_bound_sources(
        &self,
        owner: CallBodyOwnerId,
        target: OrdinaryTypeTargetId,
        param_id: Option<TypeGenericParamNodeId>,
        type_relations: &[TypeRelation],
    ) -> Result<Vec<TraitTypeSourceId>, SynParserError> {
        let mut sources = Vec::new();
        for scope in self.generic_bound_scopes(owner)? {
            if let Some(param_id) = param_id {
                for param in scope.params {
                    if TypeGenericParamNodeId::try_refine(param.id, &param.kind).ok()
                        == Some(param_id)
                        && let Some(bounds) = param.kind.bounds()
                    {
                        sources.extend(bounds.iter().copied());
                    }
                }
            }

            for predicate in scope.predicates {
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
        }

        sources.sort_unstable();
        sources.dedup();
        Ok(sources)
    }

    fn bound_traits_from_sources(
        &self,
        sources: &[TraitTypeSourceId],
        type_relations: &[TypeRelation],
    ) -> Result<Vec<TraitNodeId>, SynParserError> {
        let mut sources = sources.to_vec();
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

    fn where_bound_traits_for_type_name(
        &self,
        owner: CallBodyOwnerId,
        type_segment: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Vec<TraitNodeId>, SynParserError> {
        let mut sources = Vec::new();
        for scope in self.generic_bound_scopes(owner)? {
            for predicate in scope.predicates {
                if self.type_path_matches_segment(predicate.subject, type_segment)? {
                    sources.extend(predicate.bounds.iter().copied());
                }
            }
        }

        let mut traits = self.bound_traits_from_sources(&sources, type_relations)?;
        for source in sources {
            if self.has_trait_relation(source, type_relations) {
                continue;
            }
            traits.extend(self.workspace_trait_targets_for_source(owner, source)?);
        }
        traits.extend(self.executable_where_bound_traits_for_type_name(owner, type_segment)?);
        traits.sort_unstable();
        traits.dedup();
        Ok(traits)
    }

    fn executable_where_bound_traits_for_type_name(
        &self,
        owner: CallBodyOwnerId,
        type_segment: &str,
    ) -> Result<Vec<TraitNodeId>, SynParserError> {
        let CallBodyOwnerId::Executable(id) = owner else {
            return Ok(Vec::new());
        };
        let body = self
            .graph
            .executable_bodies()
            .iter()
            .find(|body| body.id == id)
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "call resolution found missing executable body {id} during executable where-bound lookup"
                ))
            })?;

        let mut traits = Vec::new();
        for predicate in &body.where_predicates {
            if !executable_predicate_matches_type(predicate, type_segment) {
                continue;
            }
            for bound in &predicate.trait_bounds {
                traits.extend(self.executable_trait_bound_targets(owner, bound)?);
            }
        }
        traits.sort_unstable();
        traits.dedup();
        Ok(traits)
    }

    fn executable_trait_bound_targets(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<Vec<TraitNodeId>, SynParserError> {
        let mut targets = Vec::new();
        if let [segment] = path {
            if let LocalTraitResolution::Resolved(target) =
                self.resolve_local_trait_segment(owner, segment)?
            {
                targets.push(target);
            }
            targets.extend(self.resolve_workspace_trait_import(owner, segment)?);
        } else {
            targets.extend(self.resolve_workspace_trait_path(path)?);
        }

        targets.sort_unstable();
        targets.dedup();
        Ok(targets)
    }

    fn type_path_matches_segment(
        &self,
        type_id: OrdinaryTypeUseId,
        segment: &str,
    ) -> Result<bool, SynParserError> {
        match self.type_node(type_id)? {
            TypeNode::Named(node) => Ok(node.path.last().is_some_and(|part| part == segment)),
            TypeNode::Reference(node) => self.type_path_matches_segment(node.referenced, segment),
            TypeNode::Paren(node) => self.type_path_matches_segment(node.inner, segment),
            _ => Ok(false),
        }
    }

    fn has_trait_relation(
        &self,
        source: TraitTypeSourceId,
        type_relations: &[TypeRelation],
    ) -> bool {
        type_relations.iter().any(|relation| {
            matches!(
                relation,
                TypeRelation::Trait {
                    source: relation_source,
                    ..
                } if *relation_source == source
            )
        })
    }

    fn workspace_trait_targets_for_source(
        &self,
        owner: CallBodyOwnerId,
        source: TraitTypeSourceId,
    ) -> Result<Vec<TraitNodeId>, SynParserError> {
        let path = match self.type_node(source)? {
            TypeNode::Named(node) => &node.path,
            TypeNode::TraitBound(node) => &node.path,
            _ => return Ok(Vec::new()),
        };

        let mut targets = self.resolve_workspace_trait_path(path)?;
        if targets.is_empty()
            && let [segment] = path.as_slice()
        {
            targets.extend(self.resolve_workspace_trait_import(owner, segment)?);
        }
        targets.sort_unstable();
        targets.dedup();
        Ok(targets)
    }

    fn resolve_workspace_trait_path(
        &self,
        path: &[String],
    ) -> Result<Vec<TraitNodeId>, SynParserError> {
        let Some((root, tail)) = path.split_first() else {
            return Ok(Vec::new());
        };
        let Some(workspace) = self.workspace else {
            return Ok(Vec::new());
        };

        let mut targets = Vec::new();
        for dep in workspace.crates {
            if !dependency_name_matches(dep.dependency_name, root) {
                continue;
            }
            let resolver = CallRelationResolver::new(dep.graph, dep.tree);
            match resolver.resolve_trait_path_from_root(tail)? {
                LocalTraitResolution::Resolved(target) => targets.push(target),
                LocalTraitResolution::Unresolved => {}
                LocalTraitResolution::Ambiguous => {}
            }
        }
        targets.sort_unstable();
        targets.dedup();
        Ok(targets)
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
            let trait_node = self.trait_node(trait_id)?;
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

    fn trait_node(&self, trait_id: TraitNodeId) -> Result<&TraitNode, SynParserError> {
        if let Some(node) = self.graph.traits().iter().find(|node| node.id == trait_id) {
            return Ok(node);
        }
        if let Some(workspace) = self.workspace {
            for dep in workspace.crates {
                if let Some(node) = dep.graph.traits().iter().find(|node| node.id == trait_id) {
                    return Ok(node);
                }
            }
        }
        Err(SynParserError::InternalState(format!(
            "call resolution found missing trait node {trait_id}"
        )))
    }

    fn generic_bound_scopes(
        &self,
        owner: CallBodyOwnerId,
    ) -> Result<Vec<GenericBoundScope<'_>>, SynParserError> {
        match owner {
            CallBodyOwnerId::Function(id) => self
                .graph
                .functions()
                .iter()
                .find(|node| node.id == id)
                .map(|node| {
                    vec![GenericBoundScope {
                        params: node.generic_params.as_slice(),
                        predicates: node.where_predicates.as_slice(),
                    }]
                })
                .ok_or_else(|| {
                    SynParserError::InternalState(format!(
                        "call resolution found call owned by missing function {id}"
                    ))
                }),
            CallBodyOwnerId::Method(id) => {
                let node = self.graph.find_node_unique(id.as_any()).map_err(|err| {
                    SynParserError::InternalState(format!(
                        "call resolution found call owned by missing or non-unique method {id}: {err}"
                    ))
                })?;
                let method = node.as_method().ok_or_else(|| {
                    SynParserError::InternalState(format!(
                        "call resolution owner {id} did not resolve to a method node"
                    ))
                })?;

                let mut scopes = vec![GenericBoundScope {
                    params: method.generic_params.as_slice(),
                    predicates: method.where_predicates.as_slice(),
                }];

                if let Some(impl_id) = self.impl_for_owner_method(id)? {
                    let Some(impl_node) = self.maybe_impl_node(impl_id) else {
                        return Err(SynParserError::InternalState(format!(
                            "call resolution found method {id} owned by missing impl {impl_id}"
                        )));
                    };
                    scopes.push(GenericBoundScope {
                        params: impl_node.generic_params.as_slice(),
                        predicates: impl_node.where_predicates.as_slice(),
                    });
                } else if let Some(trait_id) = self.trait_for_owner_method(id)? {
                    let trait_node = self.graph.get_trait_checked(trait_id)?;
                    scopes.push(GenericBoundScope {
                        params: trait_node.generic_params.as_slice(),
                        predicates: trait_node.where_predicates.as_slice(),
                    });
                }

                Ok(scopes)
            }
            CallBodyOwnerId::Macro(_) | CallBodyOwnerId::Const(_) | CallBodyOwnerId::Static(_) => {
                Ok(Vec::new())
            }
            CallBodyOwnerId::Executable(id) => self.generic_bound_scopes(
                self.executable_parent_owner(id, "generic bound scope lookup")?,
            ),
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

    pub(super) fn resolve_local_type_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<LocalTypeResolution, SynParserError> {
        let Some(mut current_module) = self.containing_module_for_owner(owner) else {
            return Ok(LocalTypeResolution::Unresolved);
        };
        current_module = self.import_scope_module(current_module)?;

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
}

fn executable_predicate_matches_type(predicate: &ExecutableWherePredicate, segment: &str) -> bool {
    predicate
        .subject_path
        .last()
        .is_some_and(|subject| subject == segment)
}

fn assert_unique_status_sources(
    statuses: &[CallResolutionStatus],
    graph: Option<&ParsedCodeGraph>,
) -> Result<(), SynParserError> {
    let mut by_source = BTreeMap::new();
    for status in statuses {
        if let Some(existing) = by_source.insert(status.source(), status) {
            let source = status.source();
            let call_site = graph
                .and_then(|graph| describe_call_site(graph, source))
                .unwrap_or_else(|| "<call site unavailable>".to_string());
            return Err(SynParserError::InternalState(format!(
                "duplicate call_resolution_status for {source}: first={existing:?}, second={status:?}, call_site={call_site}"
            )));
        }
    }
    Ok(())
}

fn describe_call_site(graph: &ParsedCodeGraph, source: AnyCallSiteId) -> Option<String> {
    graph
        .call_sites()
        .iter()
        .filter(|call| match (source, *call) {
            (AnyCallSiteId::Path(source), CallNode::PathCall(call)) => source == call.id,
            (AnyCallSiteId::Method(source), CallNode::MethodCall(call)) => source == call.id,
            (AnyCallSiteId::Dynamic(source), CallNode::DynamicCall(call)) => source == call.id,
            (AnyCallSiteId::Macro(source), CallNode::MacroCall(call)) => source == call.id,
            _ => false,
        })
        .map(|call| format!("{call:?}"))
        .next()
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

/// Resolves call sites with parsed workspace dependency crates available as
/// additional proof carriers for dependency-root paths.
pub fn resolve_call_relations_after_tree_with_workspace<'a>(
    graph: &'a ParsedCodeGraph,
    tree: &'a ModuleTree,
    workspace: CallWorkspace<'a>,
) -> Result<CallResolutionReport, SynParserError> {
    CallRelationResolver::new(graph, tree)
        .with_workspace(workspace)
        .resolve_call_relations()
}

fn dependency_name_matches(dependency_name: &str, root: &str) -> bool {
    dependency_name == root || dependency_name.replace('-', "_") == root
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

        let err = assert_unique_status_sources(&statuses, None)
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
