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
            AnyCallSiteId, AsAnyNodeId, AssociatedItemNodeId, CallBodyOwnerId, CallNode,
            ImplNodeId, MethodCallNode, MethodCallReceiver, MethodNodeId,
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

/// Resolver for the first narrow local call-graph slice.
///
/// Currently supported semantic proof:
///
/// ```text
/// self.method() inside an inherent impl method
///   -> method with the same name in the same impl block
/// ```
///
/// The resolver intentionally does not attempt trait dispatch, external calls,
/// dynamic callees, macro expansion, or non-`self` receiver typing.
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
                    statuses.push(CallResolutionStatus::Unsupported {
                        source: AnyCallSiteId::Path(path_call.id),
                    });
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
