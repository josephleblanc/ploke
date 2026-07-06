use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AnyCallSiteId, CallArgument, CallBodyOwnerId, CallNode, ExecutableBodyId,
            FunctionNodeId, PathCallCallee, PathCallNode,
        },
        relations::{CallRelation, CallResolutionKind, CallResolutionStatus, TypeRelation},
        types::{TypeNode, VisibilityKind},
    },
};

use super::{
    AssocPathResolution, CallRelationResolver, ConstructorPathResolution,
    LocalFunctionPathResolution,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum ParameterCallTarget {
    Function(FunctionNodeId),
    Closure(ExecutableBodyId),
}

#[derive(Debug, Clone, Copy)]
enum ParameterProof<'a> {
    Value,
    Field(&'a [String]),
}

impl CallRelationResolver<'_> {
    pub(super) fn resolve_path_call(
        &self,
        call: &PathCallNode,
        type_relations: &[TypeRelation],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Path(call.id);

        match &call.callee {
            PathCallCallee::ItemPath => {}
            PathCallCallee::ValueBinding { path } => {
                if let Some(target) = self.resolve_parameter_value_call(call.owner, path)? {
                    match target {
                        ParameterCallTarget::Function(target) => {
                            relations.push(CallRelation::Function {
                                source: call.id,
                                target,
                            });
                        }
                        ParameterCallTarget::Closure(target) => {
                            relations.push(CallRelation::Closure {
                                source: call.id,
                                target,
                            });
                        }
                    }
                    statuses.push(CallResolutionStatus::Resolved {
                        source,
                        kind: CallResolutionKind::LocalExact,
                    });
                    return Ok(());
                }
                statuses.push(CallResolutionStatus::Unsupported { source });
                return Ok(());
            }
            PathCallCallee::ClosureBinding { closure_id, .. } => {
                relations.push(CallRelation::Closure {
                    source: call.id,
                    target: *closure_id,
                });
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
                return Ok(());
            }
            PathCallCallee::LocalFunctionBinding { body_id, .. } => {
                relations.push(CallRelation::LocalFunction {
                    source: call.id,
                    target: *body_id,
                });
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
                return Ok(());
            }
            PathCallCallee::InitializedValueBinding { init_path, .. } => {
                self.resolve_initialized_value_binding_call(call, init_path, relations, statuses)?;
                return Ok(());
            }
        }

        if self.is_external_path(&call.path)
            || self.is_external_import_path(call.owner, &call.path)?
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        if self.is_external_self_assoc(call.owner, &call.path)? {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        if self.is_external_alias_assoc_path(call.owner, &call.path, type_relations)? {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        let mut constructor_unresolved = false;
        if let Some(resolution) = self.resolve_constructor_path(call, type_relations)? {
            match resolution {
                ConstructorPathResolution::TupleStruct(target) => {
                    relations.push(CallRelation::TupleStructConstructor {
                        source: call.id,
                        target,
                    });
                    statuses.push(CallResolutionStatus::Resolved {
                        source,
                        kind: CallResolutionKind::LocalExact,
                    });
                    return Ok(());
                }
                ConstructorPathResolution::EnumVariant(target) => {
                    relations.push(CallRelation::EnumVariantConstructor {
                        source: call.id,
                        target,
                    });
                    statuses.push(CallResolutionStatus::Resolved {
                        source,
                        kind: CallResolutionKind::LocalExact,
                    });
                    return Ok(());
                }
                ConstructorPathResolution::Unresolved => {
                    constructor_unresolved = true;
                }
                ConstructorPathResolution::Ambiguous => {
                    statuses.push(CallResolutionStatus::Ambiguous { source });
                    return Ok(());
                }
            }
        }

        if let Some(resolution) = self.resolve_associated_function_path(
            call.owner,
            &call.path,
            call.arg_count,
            type_relations,
        )? {
            match resolution {
                AssocPathResolution::Resolved(target) => {
                    relations.push(CallRelation::AssociatedFunction {
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
            return Ok(());
        }

        if constructor_unresolved {
            statuses.push(CallResolutionStatus::Unresolved { source });
            return Ok(());
        }

        let resolution = if self.is_unqualified_path(&call.path) {
            self.resolve_unqualified_local_function_path(call.owner, &call.path)?
        } else if self.is_explicit_local_path(&call.path) {
            self.resolve_local_function_path(call.owner, &call.path)?
        } else {
            self.resolve_implicit_local_function_path(call.owner, &call.path)?
        };

        let external_prelude = self.is_external_prelude_path(call.owner, &call.path)?;

        match resolution {
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
            LocalFunctionPathResolution::Unsupported if external_prelude => {
                statuses.push(CallResolutionStatus::External { source });
            }
            LocalFunctionPathResolution::Unsupported => {
                statuses.push(CallResolutionStatus::Unsupported { source });
            }
        }

        Ok(())
    }

    fn resolve_initialized_value_binding_call(
        &self,
        call: &PathCallNode,
        init_path: &[String],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Path(call.id);

        if self.is_external_path(init_path)
            || self.is_external_import_path(call.owner, init_path)?
        {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        let resolution = if self.is_unqualified_path(init_path) {
            self.resolve_unqualified_local_function_path(call.owner, init_path)?
        } else if self.is_explicit_local_path(init_path) {
            self.resolve_local_function_path(call.owner, init_path)?
        } else {
            self.resolve_implicit_local_function_path(call.owner, init_path)?
        };

        match resolution {
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
            LocalFunctionPathResolution::Unsupported => {
                statuses.push(CallResolutionStatus::Unsupported { source });
            }
        }

        Ok(())
    }

    pub(super) fn resolve_parameter_value_call(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<Option<ParameterCallTarget>, SynParserError> {
        let [name] = path else {
            return Ok(None);
        };

        self.resolve_parameter_call(owner, name, ParameterProof::Value)
    }

    pub(super) fn resolve_parameter_field_call(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<Option<ParameterCallTarget>, SynParserError> {
        let Some((name, field_path)) = path.split_first() else {
            return Ok(None);
        };
        if field_path.is_empty() {
            return Ok(None);
        }

        self.resolve_parameter_call(owner, name, ParameterProof::Field(field_path))
    }

    fn resolve_parameter_call(
        &self,
        owner: CallBodyOwnerId,
        name: &str,
        proof: ParameterProof<'_>,
    ) -> Result<Option<ParameterCallTarget>, SynParserError> {
        let Some(parameter_owner) = self.parameter_function_owner(owner)? else {
            return Ok(None);
        };
        if !self.function_allows_local_parameter_proof(parameter_owner)? {
            return Ok(None);
        }
        let Some(params) = self.owner_parameters(owner)? else {
            return Ok(None);
        };
        let Some(index) = params
            .iter()
            .position(|param| param.name.as_deref() == Some(name))
        else {
            return Ok(None);
        };

        let param_type = self.type_node(params[index].type_id)?;
        if !self.parameter_allows_local_caller_proof(parameter_owner, param_type, proof)? {
            return Ok(None);
        }
        let expected_type = parameter_proof_type_path(param_type, proof);

        let mut targets = Vec::new();
        let mut caller_count = 0usize;
        for site in self.graph.call_sites() {
            let CallNode::PathCall(site) = site else {
                continue;
            };
            if !self.path_call_targets_function(site, parameter_owner)? {
                continue;
            }
            caller_count += 1;
            if site.arguments.len() <= index {
                return Ok(None);
            }
            if let Some(target) =
                self.resolve_call_argument(site, &site.arguments[index], proof, expected_type)?
            {
                targets.push(target);
            } else {
                return Ok(None);
            }
        }

        if caller_count == 0 {
            return Ok(None);
        }

        targets.sort_unstable();
        targets.dedup();

        Ok(match targets.as_slice() {
            [target] => Some(*target),
            _ => None,
        })
    }

    fn parameter_function_owner(
        &self,
        owner: CallBodyOwnerId,
    ) -> Result<Option<FunctionNodeId>, SynParserError> {
        match owner {
            CallBodyOwnerId::Function(id) => Ok(Some(id)),
            CallBodyOwnerId::Executable(id) => {
                match self.executable_parent_owner(id, "parameter call proof")? {
                    CallBodyOwnerId::Function(id) => Ok(Some(id)),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    fn function_allows_local_parameter_proof(
        &self,
        function_id: FunctionNodeId,
    ) -> Result<bool, SynParserError> {
        let function = self
            .graph
            .functions()
            .iter()
            .find(|function| function.id == function_id)
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "call resolution found parameter call owned by missing function {function_id}"
                ))
            })?;
        Ok(matches!(function.visibility, VisibilityKind::Inherited))
    }

    fn parameter_allows_local_caller_proof(
        &self,
        function_id: FunctionNodeId,
        param_type: &TypeNode,
        proof: ParameterProof<'_>,
    ) -> Result<bool, SynParserError> {
        match (proof, param_type) {
            (ParameterProof::Value, TypeNode::Function(_)) => Ok(true),
            (ParameterProof::Value, TypeNode::Named(node)) => {
                self.type_parameter_has_callable_bound(function_id, &node.path)
            }
            (ParameterProof::Field(_), TypeNode::Named(_)) => Ok(true),
            _ => Ok(false),
        }
    }

    fn type_parameter_has_callable_bound(
        &self,
        function_id: FunctionNodeId,
        path: &[String],
    ) -> Result<bool, SynParserError> {
        let [type_name] = path else {
            return Ok(false);
        };

        for scope in self.generic_bound_scopes(CallBodyOwnerId::Function(function_id))? {
            for param in scope.params {
                if param.kind.name() != Some(type_name.as_str()) {
                    continue;
                }
                if let Some(bounds) = param.kind.bounds()
                    && self.bounds_include_callable_trait(bounds)?
                {
                    return Ok(true);
                }
            }

            for predicate in scope.predicates {
                if !self.type_path_matches_segment(predicate.subject, type_name)? {
                    continue;
                }
                if self.bounds_include_callable_trait(&predicate.bounds)? {
                    return Ok(true);
                }
            }
        }

        Ok(false)
    }

    fn bounds_include_callable_trait(
        &self,
        bounds: &[crate::parser::type_slots::TraitTypeUseId],
    ) -> Result<bool, SynParserError> {
        for bound in bounds {
            if type_node_is_callable_trait_bound(self.type_node(*bound)?) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    fn path_call_targets_function(
        &self,
        site: &PathCallNode,
        target: FunctionNodeId,
    ) -> Result<bool, SynParserError> {
        if !matches!(site.callee, PathCallCallee::ItemPath) {
            return Ok(false);
        }

        let resolution = if self.is_unqualified_path(&site.path) {
            self.resolve_unqualified_local_function_path(site.owner, &site.path)?
        } else if self.is_explicit_local_path(&site.path) {
            self.resolve_local_function_path(site.owner, &site.path)?
        } else {
            self.resolve_implicit_local_function_path(site.owner, &site.path)?
        };

        Ok(matches!(
            resolution,
            LocalFunctionPathResolution::Resolved(candidate) if candidate == target
        ))
    }

    fn resolve_call_argument(
        &self,
        site: &PathCallNode,
        arg: &CallArgument,
        proof: ParameterProof<'_>,
        expected_type: Option<&[String]>,
    ) -> Result<Option<ParameterCallTarget>, SynParserError> {
        match (proof, arg) {
            (ParameterProof::Value, CallArgument::Path { path }) => self
                .resolve_argument_path(site.owner, path)
                .map(|target| target.map(ParameterCallTarget::Function)),
            (ParameterProof::Value, CallArgument::Closure { closure_id }) => {
                Ok(Some(ParameterCallTarget::Closure(*closure_id)))
            }
            (
                ParameterProof::Field(field_path),
                CallArgument::Constructed { type_path, fields },
            ) => {
                if !expected_type.is_some_and(|expected| path_leaf_matches(type_path, expected)) {
                    return Ok(None);
                }
                let Some(field) = fields.iter().find(|field| field.field_path == field_path) else {
                    return Ok(None);
                };
                self.resolve_argument_path(site.owner, &field.init_path)
                    .map(|target| target.map(ParameterCallTarget::Function))
            }
            _ => Ok(None),
        }
    }

    fn resolve_argument_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<Option<FunctionNodeId>, SynParserError> {
        if self.is_external_path(path) || self.is_external_import_path(owner, path)? {
            return Ok(None);
        }

        let resolution = if self.is_unqualified_path(path) {
            self.resolve_unqualified_local_function_path(owner, path)?
        } else if self.is_explicit_local_path(path) {
            self.resolve_local_function_path(owner, path)?
        } else {
            self.resolve_implicit_local_function_path(owner, path)?
        };

        match resolution {
            LocalFunctionPathResolution::Resolved(target) => Ok(Some(target)),
            LocalFunctionPathResolution::Unresolved
            | LocalFunctionPathResolution::Ambiguous
            | LocalFunctionPathResolution::Unsupported => Ok(None),
        }
    }
}

fn parameter_proof_type_path<'a>(
    param_type: &'a TypeNode,
    proof: ParameterProof<'_>,
) -> Option<&'a [String]> {
    match (proof, param_type) {
        (ParameterProof::Field(_), TypeNode::Named(node)) => Some(&node.path),
        _ => None,
    }
}

fn path_leaf_matches(path: &[String], expected: &[String]) -> bool {
    path.last().is_some() && path.last() == expected.last()
}

fn type_node_is_callable_trait_bound(node: &TypeNode) -> bool {
    let path = match node {
        TypeNode::TraitBound(node) => &node.path,
        TypeNode::Named(node) => &node.path,
        _ => return false,
    };
    path.last()
        .is_some_and(|name| matches!(name.as_str(), "Fn" | "FnMut" | "FnOnce"))
}
