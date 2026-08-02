use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AsAnyNodeId, AssociatedItemNodeId, CallBodyOwnerId, ExecutableBodyId, FunctionNodeId,
            ImplNode, ImplNodeId, MethodNodeId, OrdinaryTypeUseId, ParamData, TraitNodeId,
        },
        relations::SyntacticRelation,
    },
    resolve::RelationIndexer,
};

use super::CallRelationResolver;

impl CallRelationResolver<'_> {
    pub(super) fn impl_for_owner_method(
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

    pub(super) fn trait_for_owner_method(
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

    pub(super) fn maybe_impl_node(&self, impl_id: ImplNodeId) -> Option<&ImplNode> {
        self.graph.impls().iter().find(|node| node.id == impl_id)
    }

    pub(super) fn function_return_type(
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

    pub(super) fn method_return_type(
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

    pub(super) fn method_is_async(&self, method_id: MethodNodeId) -> Result<bool, SynParserError> {
        let mut async_values = self
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
            .map(|method| method.is_async)
            .collect::<Vec<_>>();
        async_values.sort_unstable();
        async_values.dedup();

        match async_values.as_slice() {
            [is_async] => Ok(*is_async),
            [] => Err(SynParserError::InternalState(format!(
                "call resolution found awaited method result to missing method {method_id}"
            ))),
            _ => Err(SynParserError::InternalState(format!(
                "call resolution found conflicting async flags for method {method_id}"
            ))),
        }
    }

    pub(super) fn owner_parameters(
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
            CallBodyOwnerId::Macro(_)
            | CallBodyOwnerId::Const(_)
            | CallBodyOwnerId::Static(_) => Ok(None),
            CallBodyOwnerId::Executable(id) => {
                self.owner_parameters(self.executable_parent_owner(id, "parameter lookup")?)
            }
        }
    }

    pub(super) fn executable_parent_owner(
        &self,
        id: ExecutableBodyId,
        context: &str,
    ) -> Result<CallBodyOwnerId, SynParserError> {
        self.graph
            .executable_bodies()
            .iter()
            .find(|body| body.id == id)
            .map(|body| body.parent)
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "call resolution found missing executable body {id} during {context}"
                ))
            })
    }
}
