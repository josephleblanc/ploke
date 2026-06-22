//! Typed call-resolution facts for parser-owned structural call sites.
//!
//! This resolver consumes the structural call-site inventory emitted by the
//! parser and constructs semantic call-target facts only when the target can be
//! proven from local typed graph evidence. Unsupported shapes remain visible as
//! statuses and do not produce fake target edges.

use serde::{Deserialize, Serialize};

use crate::{
    error::SynParserError,
    parser::{
        ParsedCodeGraph,
        graph::GraphAccess,
        nodes::{
            AnyCallSiteId, AnyNodeId, AsAnyNodeId, AssociatedItemNodeId, CallBodyOwnerId, CallNode,
            FunctionNodeId, ImplNodeId, MethodCallNode, MethodCallReceiver, MethodNodeId,
            ModuleNodeId, PathCallNode,
        },
        relations::{CallRelation, CallResolutionKind, CallResolutionStatus, SyntacticRelation},
    },
};

use super::{RelationIndexer, module_tree::ModuleTree};

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
/// super::function() / self::function() / crate::module::function()
///   -> local standalone function proven from the module tree
/// ```
///
/// The resolver intentionally does not attempt trait dispatch, external calls,
/// dynamic callees, macro expansion, unqualified value-binding calls, associated
/// functions, constructors, or non-`self` receiver typing.
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

        for call in self.graph.call_sites() {
            match call {
                CallNode::MethodCall(method_call) => {
                    self.resolve_method_call(method_call, &mut relations, &mut statuses)?;
                }
                CallNode::PathCall(path_call) => {
                    self.resolve_path_call(path_call, &mut relations, &mut statuses)?;
                }
                CallNode::DynamicCall(dynamic_call) => {
                    statuses.push(CallResolutionStatus::Unsupported {
                        source: AnyCallSiteId::Dynamic(dynamic_call.id),
                    });
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
        statuses.sort_unstable();
        statuses.dedup();
        let summary = CallResolutionSummary::from_statuses(&statuses);

        Ok(CallResolutionReport {
            relations,
            statuses,
            summary,
        })
    }

    fn resolve_path_call(
        &self,
        call: &PathCallNode,
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Path(call.id);

        if self.is_external_path(&call.path) {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        if !self.is_explicit_local_path(&call.path) {
            statuses.push(CallResolutionStatus::Unsupported { source });
            return Ok(());
        }

        match self.resolve_explicit_local_function_path(call.owner, &call.path)? {
            LocalFunctionPathResolution::Resolved(target) => {
                relations.push(CallRelation::Function {
                    source: call.id,
                    target,
                });
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            }
            LocalFunctionPathResolution::Unresolved => {
                statuses.push(CallResolutionStatus::Unresolved { source });
            }
            LocalFunctionPathResolution::Ambiguous => {
                statuses.push(CallResolutionStatus::Ambiguous { source });
            }
        }

        Ok(())
    }

    fn is_explicit_local_path(&self, path: &[String]) -> bool {
        path.len() > 1
            && matches!(
                path.first().map(String::as_str),
                Some("crate" | "self" | "super")
            )
    }

    fn is_external_path(&self, path: &[String]) -> bool {
        path.first().is_some_and(|segment| {
            matches!(segment.as_str(), "std" | "core" | "alloc")
                || self
                    .graph
                    .iter_dependency_names()
                    .any(|dependency| dependency == segment)
        })
    }

    fn resolve_explicit_local_function_path(
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
            if let Ok(module_id) = ModuleNodeId::try_from(candidate) {
                candidates.push(module_id);
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
                sink(target_any)?;
            }
        }

        Ok(())
    }

    fn containing_module_for_owner(&self, owner: CallBodyOwnerId) -> Option<ModuleNodeId> {
        match owner {
            CallBodyOwnerId::Function(id) => self.containing_module(id.as_any()),
            CallBodyOwnerId::Method(id) => self.containing_module(id.as_any()),
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
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Method(call.id);

        if call.receiver != MethodCallReceiver::SelfValue {
            statuses.push(CallResolutionStatus::Unsupported { source });
            return Ok(());
        }

        let CallBodyOwnerId::Method(owner_method_id) = call.owner else {
            statuses.push(CallResolutionStatus::Unsupported { source });
            return Ok(());
        };

        let Some(impl_id) = self.impl_for_owner_method(owner_method_id)? else {
            statuses.push(CallResolutionStatus::Unsupported { source });
            return Ok(());
        };

        let impl_node = self
            .graph
            .impls()
            .iter()
            .find(|node| node.id == impl_id)
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "call resolution found owner impl relation to missing impl {impl_id}"
                ))
            })?;

        let candidates: Vec<MethodNodeId> = impl_node
            .methods
            .iter()
            .filter(|method| method.name == call.method_name)
            .map(|method| method.id)
            .collect();

        match candidates.as_slice() {
            [target] => {
                relations.push(CallRelation::Method {
                    source: call.id,
                    target: *target,
                });
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            }
            [] => statuses.push(CallResolutionStatus::Unresolved { source }),
            _ => statuses.push(CallResolutionStatus::Ambiguous { source }),
        }

        Ok(())
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
}

/// Resolves structural call sites into typed semantic call facts after the
/// `ModuleTree` has been built and pruned.
pub fn resolve_call_relations_after_tree(
    graph: &ParsedCodeGraph,
    tree: &ModuleTree,
) -> Result<CallResolutionReport, SynParserError> {
    CallRelationResolver::new(graph, tree).resolve_call_relations()
}
