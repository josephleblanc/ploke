use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AnyCallSiteId, AsAnyNodeId, CallArgument, CallBodyOwnerId, CallNode, ExecutableBodyId,
            FunctionNodeId, LocalBindingKind, LocalBindingSource, MethodCallNode,
            MethodCallReceiver, MethodNodeId, PathCallCallee, PathCallNode,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum ParameterCallResolution {
    Exact(ParameterCallTarget),
    Ambiguous(Vec<ParameterCallTarget>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum InitializedCallableTarget {
    Function(FunctionNodeId),
    AssociatedFunction(MethodNodeId),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InitializedCallableResolution {
    Resolved(InitializedCallableTarget),
    External,
    Unresolved,
    Ambiguous,
    Unsupported,
}

#[derive(Debug, Clone, Copy)]
enum ParameterProof<'a> {
    Value,
    CallableField(&'a str),
    Field(&'a [String]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ParameterOwner {
    Function(FunctionNodeId),
    Method(MethodNodeId),
}

// Bound interprocedural callable-parameter proof to explicit private forwarding chains.
const PARAMETER_FORWARDING_DEPTH: usize = 2;

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
                if self.resolve_parameter_value_path_call(
                    call,
                    path,
                    type_relations,
                    relations,
                    statuses,
                )? {
                    return Ok(());
                }
                statuses.push(CallResolutionStatus::Unsupported { source });
                return Ok(());
            }
            PathCallCallee::AliasedValueBinding { source_path, .. } => {
                if self.resolve_parameter_value_path_call(
                    call,
                    source_path,
                    type_relations,
                    relations,
                    statuses,
                )? {
                    return Ok(());
                }
                statuses.push(CallResolutionStatus::Unsupported { source });
                return Ok(());
            }
            PathCallCallee::ClosureBinding { closure_id, .. }
            | PathCallCallee::AwaitedAsyncClosureBinding { closure_id, .. } => {
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
            PathCallCallee::AsyncClosureBinding { .. } => {
                statuses.push(CallResolutionStatus::Unsupported { source });
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
                self.resolve_initialized_value_binding_call(
                    call,
                    init_path,
                    type_relations,
                    relations,
                    statuses,
                )?;
                return Ok(());
            }
            PathCallCallee::AmbiguousInitializedValueBinding { init_paths, .. } => {
                self.resolve_ambiguous_init_call(
                    call,
                    init_paths,
                    type_relations,
                    relations,
                    statuses,
                )?;
                return Ok(());
            }
            PathCallCallee::SelfFieldBinding { field_path, .. } => {
                if self.resolve_self_field_binding_path_call(
                    call,
                    field_path,
                    type_relations,
                    relations,
                    statuses,
                )? {
                    return Ok(());
                }
                statuses.push(CallResolutionStatus::Unsupported { source });
                return Ok(());
            }
        }

        let external_path = self.is_external_path(&call.path);
        let generic_root = self.path_starts_with_owner_generic_param(call.owner, &call.path)?;
        let external_import =
            !generic_root && self.is_external_import_path(call.owner, &call.path)?;
        if external_path || external_import {
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

        if self.is_external_generic_bound_assoc_path(call.owner, &call.path, call.arg_count)? {
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

    fn path_starts_with_owner_generic_param(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<bool, SynParserError> {
        let Some(type_segment) = path.first() else {
            return Ok(false);
        };
        self.owner_has_generic_param_named(owner, type_segment)
    }

    fn resolve_initialized_value_binding_call(
        &self,
        call: &PathCallNode,
        init_path: &[String],
        type_relations: &[TypeRelation],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Path(call.id);

        match self.resolve_initialized_callable_path(
            call.owner,
            init_path,
            call.arg_count,
            type_relations,
        )? {
            InitializedCallableResolution::Resolved(target) => {
                push_initialized_callable_target(call, target, relations);
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            }
            InitializedCallableResolution::External => {
                statuses.push(CallResolutionStatus::External { source });
            }
            InitializedCallableResolution::Unresolved => {
                statuses.push(CallResolutionStatus::Unresolved { source });
            }
            InitializedCallableResolution::Ambiguous => {
                statuses.push(CallResolutionStatus::Ambiguous { source });
            }
            InitializedCallableResolution::Unsupported => {
                statuses.push(CallResolutionStatus::Unsupported { source });
            }
        }

        Ok(())
    }

    fn resolve_ambiguous_init_call(
        &self,
        call: &PathCallNode,
        init_paths: &[Vec<String>],
        type_relations: &[TypeRelation],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Path(call.id);
        let mut targets = Vec::new();

        for init_path in init_paths {
            match self.resolve_initialized_callable_path(
                call.owner,
                init_path,
                call.arg_count,
                type_relations,
            )? {
                InitializedCallableResolution::Resolved(target) => targets.push(target),
                InitializedCallableResolution::External
                | InitializedCallableResolution::Unsupported => {
                    statuses.push(CallResolutionStatus::Unsupported { source });
                    return Ok(());
                }
                InitializedCallableResolution::Unresolved => {
                    statuses.push(CallResolutionStatus::Unresolved { source });
                    return Ok(());
                }
                InitializedCallableResolution::Ambiguous => {
                    statuses.push(CallResolutionStatus::Ambiguous { source });
                    return Ok(());
                }
            }
        }

        targets.sort_unstable();
        targets.dedup();

        match targets.as_slice() {
            [] => statuses.push(CallResolutionStatus::Unsupported { source }),
            [target] => {
                push_initialized_callable_target(call, *target, relations);
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            }
            _ => {
                for target in targets {
                    push_initialized_callable_target(call, target, relations);
                }
                statuses.push(CallResolutionStatus::Ambiguous { source });
            }
        }

        Ok(())
    }

    fn resolve_initialized_callable_path(
        &self,
        owner: CallBodyOwnerId,
        init_path: &[String],
        arg_count: usize,
        type_relations: &[TypeRelation],
    ) -> Result<InitializedCallableResolution, SynParserError> {
        if self.is_external_path(init_path) || self.is_external_import_path(owner, init_path)? {
            return Ok(InitializedCallableResolution::External);
        }

        if let Some(resolution) =
            self.resolve_associated_function_path(owner, init_path, arg_count, type_relations)?
        {
            return Ok(match resolution {
                AssocPathResolution::Resolved(target) => InitializedCallableResolution::Resolved(
                    InitializedCallableTarget::AssociatedFunction(target),
                ),
                AssocPathResolution::Unresolved => InitializedCallableResolution::Unresolved,
                AssocPathResolution::Ambiguous => InitializedCallableResolution::Ambiguous,
                AssocPathResolution::Unsupported => InitializedCallableResolution::Unsupported,
            });
        }

        let resolution = if self.is_unqualified_path(init_path) {
            self.resolve_unqualified_local_function_path(owner, init_path)?
        } else if self.is_explicit_local_path(init_path) {
            self.resolve_local_function_path(owner, init_path)?
        } else {
            self.resolve_implicit_local_function_path(owner, init_path)?
        };

        Ok(match resolution {
            LocalFunctionPathResolution::Resolved(target) => {
                InitializedCallableResolution::Resolved(InitializedCallableTarget::Function(target))
            }
            LocalFunctionPathResolution::Unresolved => InitializedCallableResolution::Unresolved,
            LocalFunctionPathResolution::Ambiguous => InitializedCallableResolution::Ambiguous,
            LocalFunctionPathResolution::Unsupported => InitializedCallableResolution::Unsupported,
        })
    }

    fn resolve_parameter_value_path_call(
        &self,
        call: &PathCallNode,
        path: &[String],
        type_relations: &[TypeRelation],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<bool, SynParserError> {
        let Some(resolution) =
            self.resolve_parameter_value_call(call.owner, path, type_relations)?
        else {
            return Ok(false);
        };

        push_path_parameter_resolution(call, resolution, relations, statuses);
        Ok(true)
    }

    fn resolve_self_field_binding_path_call(
        &self,
        call: &PathCallNode,
        field_path: &[String],
        type_relations: &[TypeRelation],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<bool, SynParserError> {
        let mut self_path = Vec::with_capacity(field_path.len() + 1);
        self_path.push("self".to_string());
        self_path.extend(field_path.iter().cloned());

        let Some(resolution) =
            self.resolve_self_field_callable_call(call.owner, &self_path, type_relations)?
        else {
            return Ok(false);
        };

        push_path_parameter_resolution(call, resolution, relations, statuses);
        Ok(true)
    }
}

fn push_path_parameter_resolution(
    call: &PathCallNode,
    resolution: ParameterCallResolution,
    relations: &mut Vec<CallRelation>,
    statuses: &mut Vec<CallResolutionStatus>,
) {
    let source = AnyCallSiteId::Path(call.id);
    match resolution {
        ParameterCallResolution::Exact(target) => {
            push_path_parameter_target(call, target, relations);
            statuses.push(CallResolutionStatus::Resolved {
                source,
                kind: CallResolutionKind::LocalExact,
            });
        }
        ParameterCallResolution::Ambiguous(targets) => {
            for target in targets {
                push_path_parameter_target(call, target, relations);
            }
            statuses.push(CallResolutionStatus::Ambiguous { source });
        }
    }
}

fn push_initialized_callable_target(
    call: &PathCallNode,
    target: InitializedCallableTarget,
    relations: &mut Vec<CallRelation>,
) {
    match target {
        InitializedCallableTarget::Function(target) => relations.push(CallRelation::Function {
            source: call.id,
            target,
        }),
        InitializedCallableTarget::AssociatedFunction(target) => {
            relations.push(CallRelation::AssociatedFunction {
                source: call.id,
                target,
            });
        }
    }
}

impl CallRelationResolver<'_> {
    pub(super) fn resolve_parameter_value_call(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        self.resolve_parameter_value_call_with_depth(
            owner,
            path,
            ParameterProof::Value,
            type_relations,
            PARAMETER_FORWARDING_DEPTH,
        )
    }

    pub(super) fn resolve_parameter_value_call_for_callable_field(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        field_name: &str,
        type_relations: &[TypeRelation],
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        self.resolve_parameter_value_call_with_depth(
            owner,
            path,
            ParameterProof::CallableField(field_name),
            type_relations,
            PARAMETER_FORWARDING_DEPTH,
        )
    }

    fn resolve_parameter_value_call_with_depth(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        proof: ParameterProof<'_>,
        type_relations: &[TypeRelation],
        depth: usize,
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        let [name] = path else {
            return Ok(None);
        };

        self.resolve_parameter_call(owner, name, proof, type_relations, depth)
    }

    pub(super) fn resolve_parameter_field_call(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        type_relations: &[TypeRelation],
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        self.resolve_parameter_field_call_with_depth(
            owner,
            path,
            type_relations,
            PARAMETER_FORWARDING_DEPTH,
        )
    }

    fn resolve_parameter_field_call_with_depth(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        type_relations: &[TypeRelation],
        depth: usize,
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        let Some((name, field_path)) = path.split_first() else {
            return Ok(None);
        };
        if field_path.is_empty() {
            return Ok(None);
        }

        self.resolve_parameter_field_name_call_with_depth(
            owner,
            name,
            field_path,
            type_relations,
            depth,
        )
    }

    fn resolve_parameter_field_name_call_with_depth(
        &self,
        owner: CallBodyOwnerId,
        name: &str,
        field_path: &[String],
        type_relations: &[TypeRelation],
        depth: usize,
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        if let Some(resolution) = self.resolve_parameter_call(
            owner,
            name,
            ParameterProof::Field(field_path),
            type_relations,
            depth,
        )? {
            return Ok(Some(resolution));
        }

        if depth == 0 {
            return Ok(None);
        }

        let Some(source_path) = self.local_value_alias_source(owner, name) else {
            return Ok(None);
        };
        let [source_name] = source_path.as_slice() else {
            return Ok(None);
        };
        if source_name == name {
            return Ok(None);
        }

        self.resolve_parameter_field_name_call_with_depth(
            owner,
            source_name,
            field_path,
            type_relations,
            depth - 1,
        )
    }

    fn local_value_alias_source(&self, owner: CallBodyOwnerId, name: &str) -> Option<Vec<String>> {
        let matches = self
            .graph
            .graph
            .local_bindings
            .iter()
            .filter(|binding| {
                binding.owner == owner
                    && binding.kind == LocalBindingKind::LetBinding
                    && binding.name == name
            })
            .filter_map(|binding| match &binding.source {
                LocalBindingSource::ValueAlias { source_path } if source_path.len() == 1 => {
                    Some(source_path.clone())
                }
                _ => None,
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [source_path] => Some(source_path.clone()),
            _ => None,
        }
    }

    fn resolve_parameter_call(
        &self,
        owner: CallBodyOwnerId,
        name: &str,
        proof: ParameterProof<'_>,
        type_relations: &[TypeRelation],
        depth: usize,
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        let Some(parameter_owner) = self.parameter_owner(owner)? else {
            return Ok(None);
        };
        if !self.parameter_owner_allows_local_proof(parameter_owner)? {
            return Ok(None);
        }
        let params = self.parameter_owner_params(parameter_owner)?;
        let value_params = params
            .iter()
            .filter(|param| !param.is_self)
            .collect::<Vec<_>>();
        let Some(index) = value_params
            .iter()
            .position(|param| param.name.as_deref() == Some(name))
        else {
            return Ok(None);
        };

        let param_type = self.type_node(value_params[index].type_id)?;
        if !self.parameter_allows_local_caller_proof(parameter_owner, param_type, proof)? {
            return Ok(None);
        }
        let expected_type = parameter_proof_type_path(param_type, proof);

        let mut targets = Vec::new();
        let mut caller_count = 0usize;
        match parameter_owner {
            ParameterOwner::Function(function_id) => {
                for site in self.graph.call_sites() {
                    let CallNode::PathCall(site) = site else {
                        continue;
                    };
                    if !self.path_call_targets_function(site, function_id)? {
                        continue;
                    }
                    caller_count += 1;
                    if site.arguments.len() <= index {
                        return Ok(None);
                    }
                    let Some(resolution) = self.resolve_call_argument(
                        site.owner,
                        &site.arguments[index],
                        param_type,
                        proof,
                        expected_type,
                        type_relations,
                        depth,
                    )?
                    else {
                        return Ok(None);
                    };
                    extend_parameter_targets(&mut targets, resolution);
                }
            }
            ParameterOwner::Method(method_id) => {
                let method_name = self.method_node(method_id)?.name.as_str();
                for site in self.graph.call_sites() {
                    let CallNode::MethodCall(site) = site else {
                        continue;
                    };
                    if !matches!(
                        self.resolve_method_call_target(site, type_relations)?,
                        AssocPathResolution::Resolved(target) if target == method_id
                    ) {
                        continue;
                    }
                    caller_count += 1;
                    if site.arguments.len() <= index {
                        return Ok(None);
                    }
                    let Some(resolution) = self.resolve_call_argument(
                        site.owner,
                        &site.arguments[index],
                        param_type,
                        proof,
                        expected_type,
                        type_relations,
                        depth,
                    )?
                    else {
                        return Ok(None);
                    };
                    extend_parameter_targets(&mut targets, resolution);
                }
                caller_count += self.resolve_unsupported_method_argument_callers(
                    method_name,
                    index,
                    param_type,
                    proof,
                    expected_type,
                    type_relations,
                    depth,
                    &mut targets,
                )?;
            }
        }

        if caller_count == 0 {
            return Ok(None);
        }

        targets.sort_unstable();
        targets.dedup();

        Ok(match targets.as_slice() {
            [target] => Some(ParameterCallResolution::Exact(*target)),
            [_, _, ..] => Some(ParameterCallResolution::Ambiguous(targets)),
            [] => None,
        })
    }

    fn parameter_owner(
        &self,
        owner: CallBodyOwnerId,
    ) -> Result<Option<ParameterOwner>, SynParserError> {
        match owner {
            CallBodyOwnerId::Function(id) => Ok(Some(ParameterOwner::Function(id))),
            CallBodyOwnerId::Method(id) => Ok(Some(ParameterOwner::Method(id))),
            CallBodyOwnerId::Executable(id) => {
                match self.executable_parent_owner(id, "parameter call proof")? {
                    CallBodyOwnerId::Function(id) => Ok(Some(ParameterOwner::Function(id))),
                    CallBodyOwnerId::Method(id) => Ok(Some(ParameterOwner::Method(id))),
                    _ => Ok(None),
                }
            }
            _ => Ok(None),
        }
    }

    fn parameter_owner_allows_local_proof(
        &self,
        owner: ParameterOwner,
    ) -> Result<bool, SynParserError> {
        match owner {
            ParameterOwner::Function(function_id) => {
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
            ParameterOwner::Method(method_id) => {
                if self.impl_for_owner_method(method_id)?.is_none() {
                    return Ok(false);
                }
                let method = self.method_node(method_id)?;
                Ok(!matches!(method.visibility, VisibilityKind::Public))
            }
        }
    }

    fn parameter_owner_params(
        &self,
        owner: ParameterOwner,
    ) -> Result<&[crate::parser::nodes::ParamData], SynParserError> {
        match owner {
            ParameterOwner::Function(function_id) => self
                .graph
                .functions()
                .iter()
                .find(|function| function.id == function_id)
                .map(|function| function.parameters.as_slice())
                .ok_or_else(|| {
                    SynParserError::InternalState(format!(
                        "call resolution found parameter call owned by missing function {function_id}"
                    ))
                }),
            ParameterOwner::Method(method_id) => Ok(self.method_node(method_id)?.parameters.as_slice()),
        }
    }

    fn method_node(
        &self,
        method_id: MethodNodeId,
    ) -> Result<&crate::parser::nodes::MethodNode, SynParserError> {
        self.graph
            .find_node_unique(method_id.as_any())
            .map_err(|err| {
                SynParserError::InternalState(format!(
                    "call resolution found parameter call owned by missing or non-unique method {method_id}: {err}"
                ))
            })?
            .as_method()
            .ok_or_else(|| {
                SynParserError::InternalState(format!(
                    "call resolution parameter owner {method_id} did not resolve to a method node"
                ))
            })
    }

    fn parameter_allows_local_caller_proof(
        &self,
        owner: ParameterOwner,
        param_type: &TypeNode,
        proof: ParameterProof<'_>,
    ) -> Result<bool, SynParserError> {
        match (proof, param_type) {
            (ParameterProof::Value, TypeNode::Function(_)) => Ok(true),
            (ParameterProof::CallableField(_), TypeNode::Function(_)) => Ok(true),
            (ParameterProof::Value, param_type) if self.boxed_callable_type(param_type)? => {
                Ok(true)
            }
            (ParameterProof::CallableField(_), param_type)
                if self.boxed_callable_type(param_type)? =>
            {
                Ok(true)
            }
            (ParameterProof::Value, TypeNode::Named(node)) => {
                self.owner_type_parameter_has_callable_bound(owner, &node.path)
            }
            (ParameterProof::CallableField(_), TypeNode::Named(node)) => {
                self.owner_type_parameter_has_callable_bound(owner, &node.path)
            }
            (ParameterProof::Value, TypeNode::ImplTrait(node)) => {
                self.bounds_include_callable_trait(&node.bounds)
            }
            (ParameterProof::CallableField(_), TypeNode::ImplTrait(node)) => {
                self.bounds_include_callable_trait(&node.bounds)
            }
            (ParameterProof::Value, TypeNode::Reference(node)) => {
                self.referenced_callable_type(node.referenced)
            }
            (ParameterProof::CallableField(_), TypeNode::Reference(node)) => {
                self.referenced_callable_type(node.referenced)
            }
            (ParameterProof::Field(_), TypeNode::Named(_)) => Ok(true),
            (ParameterProof::Field(field_path), TypeNode::Array(_)) => {
                Ok(field_path_index(field_path).is_some())
            }
            _ => Ok(false),
        }
    }

    pub(super) fn boxed_callable_type(
        &self,
        param_type: &TypeNode,
    ) -> Result<bool, SynParserError> {
        let TypeNode::Named(node) = param_type else {
            return Ok(false);
        };
        if !node.path.last().is_some_and(|name| name == "Box") {
            return Ok(false);
        }
        let [argument] = node.arguments.as_slice() else {
            return Ok(false);
        };
        match self.type_node(*argument)? {
            TypeNode::TraitObject(node) => self.bounds_include_callable_trait(&node.bounds),
            _ => Ok(false),
        }
    }

    fn referenced_callable_type(
        &self,
        type_id: crate::parser::nodes::OrdinaryTypeUseId,
    ) -> Result<bool, SynParserError> {
        match self.type_node(type_id)? {
            TypeNode::TraitObject(node) => self.bounds_include_callable_trait(&node.bounds),
            TypeNode::Reference(node) => self.referenced_callable_type(node.referenced),
            _ => Ok(false),
        }
    }

    fn owner_type_parameter_has_callable_bound(
        &self,
        owner: ParameterOwner,
        path: &[String],
    ) -> Result<bool, SynParserError> {
        let [type_name] = path else {
            return Ok(false);
        };

        let owner = match owner {
            ParameterOwner::Function(id) => CallBodyOwnerId::Function(id),
            ParameterOwner::Method(id) => CallBodyOwnerId::Method(id),
        };
        for scope in self.generic_bound_scopes(owner)? {
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

    pub(super) fn bounds_include_callable_trait(
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
        owner: CallBodyOwnerId,
        arg: &CallArgument,
        param_type: &TypeNode,
        proof: ParameterProof<'_>,
        expected_type: Option<&[String]>,
        type_relations: &[TypeRelation],
        depth: usize,
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        match (proof, arg) {
            (
                ParameterProof::Value | ParameterProof::CallableField(_),
                CallArgument::Path { path },
            )
            | (
                ParameterProof::Value | ParameterProof::CallableField(_),
                CallArgument::ReferencedPath { path },
            ) => {
                if let Some(target) = self.resolve_argument_path(owner, path)? {
                    return Ok(Some(ParameterCallResolution::Exact(
                        ParameterCallTarget::Function(target),
                    )));
                }
                if depth == 0 {
                    return Ok(None);
                }
                self.resolve_parameter_value_call_with_depth(
                    owner,
                    path,
                    proof,
                    type_relations,
                    depth - 1,
                )
            }
            (
                ParameterProof::Value | ParameterProof::CallableField(_),
                CallArgument::BoxedPath { path },
            ) if self.boxed_callable_type(param_type)? => {
                Ok(self.resolve_argument_path(owner, path)?.map(|target| {
                    ParameterCallResolution::Exact(ParameterCallTarget::Function(target))
                }))
            }
            (
                ParameterProof::Value | ParameterProof::CallableField(_),
                CallArgument::Closure { closure_id },
            )
            | (
                ParameterProof::Value | ParameterProof::CallableField(_),
                CallArgument::ClosureBinding { closure_id, .. },
            ) => Ok(Some(ParameterCallResolution::Exact(
                ParameterCallTarget::Closure(*closure_id),
            ))),
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
                Ok(self
                    .resolve_argument_path(owner, &field.init_path)?
                    .map(|target| {
                        ParameterCallResolution::Exact(ParameterCallTarget::Function(target))
                    }))
            }
            (ParameterProof::Field(field_path), CallArgument::Array { element_init_paths }) => {
                let Some(index) = field_path_index(field_path) else {
                    return Ok(None);
                };
                let Some(Some(init_path)) = element_init_paths.get(index) else {
                    return Ok(None);
                };
                Ok(self.resolve_argument_path(owner, init_path)?.map(|target| {
                    ParameterCallResolution::Exact(ParameterCallTarget::Function(target))
                }))
            }
            (ParameterProof::Field(field_path), CallArgument::Path { path }) => {
                if depth == 0 || !self.parameter_type_matches(owner, path, expected_type)? {
                    return Ok(None);
                }
                let [name] = path.as_slice() else {
                    return Ok(None);
                };
                self.resolve_parameter_field_name_call_with_depth(
                    owner,
                    name,
                    field_path,
                    type_relations,
                    depth - 1,
                )
            }
            _ => Ok(None),
        }
    }

    fn resolve_unsupported_method_argument_callers(
        &self,
        method_name: &str,
        index: usize,
        param_type: &TypeNode,
        proof: ParameterProof<'_>,
        expected_type: Option<&[String]>,
        type_relations: &[TypeRelation],
        depth: usize,
        targets: &mut Vec<ParameterCallTarget>,
    ) -> Result<usize, SynParserError> {
        let mut caller_count = 0usize;

        for site in self.graph.call_sites() {
            let CallNode::MethodCall(site) = site else {
                continue;
            };
            if site.method_name != method_name || site.arguments.len() <= index {
                continue;
            }
            if !matches!(site.receiver, MethodCallReceiver::Unsupported) {
                continue;
            }
            if !matches!(
                self.resolve_method_call_target(site, type_relations)?,
                AssocPathResolution::Unsupported
            ) {
                continue;
            }
            let Some(resolution) = self.resolve_unsupported_method_argument(
                site,
                &site.arguments[index],
                param_type,
                proof,
                expected_type,
                type_relations,
                depth,
            )?
            else {
                continue;
            };
            caller_count += 1;
            extend_parameter_targets(targets, resolution);
        }

        Ok(caller_count)
    }

    fn resolve_unsupported_method_argument(
        &self,
        site: &MethodCallNode,
        arg: &CallArgument,
        param_type: &TypeNode,
        proof: ParameterProof<'_>,
        expected_type: Option<&[String]>,
        type_relations: &[TypeRelation],
        depth: usize,
    ) -> Result<Option<ParameterCallResolution>, SynParserError> {
        match (proof, arg) {
            (ParameterProof::CallableField(field_name), CallArgument::Closure { closure_id })
                if self.closure_body_calls_field_method(*closure_id, field_name) =>
            {
                self.resolve_call_argument(
                    site.owner,
                    arg,
                    param_type,
                    proof,
                    expected_type,
                    type_relations,
                    depth,
                )
            }
            _ => Ok(None),
        }
    }

    fn closure_body_calls_field_method(
        &self,
        closure_id: ExecutableBodyId,
        field_name: &str,
    ) -> bool {
        self.graph.call_sites().iter().any(|site| {
            matches!(
                site,
                CallNode::MethodCall(site)
                    if site.owner == CallBodyOwnerId::Executable(closure_id)
                        && site.method_name == field_name
                        && site.arg_count > 0
            )
        })
    }

    fn parameter_type_matches(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
        expected_type: Option<&[String]>,
    ) -> Result<bool, SynParserError> {
        let Some(expected_type) = expected_type else {
            return Ok(false);
        };
        let [name] = path else {
            return Ok(false);
        };
        let Some(params) = self.owner_parameters(owner)? else {
            return Ok(false);
        };
        let Some(param) = params
            .iter()
            .find(|param| param.name.as_deref() == Some(name.as_str()))
        else {
            return Ok(false);
        };
        match self.type_node(param.type_id)? {
            TypeNode::Named(node) => Ok(path_leaf_matches(&node.path, expected_type)),
            _ => Ok(false),
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

fn extend_parameter_targets(
    targets: &mut Vec<ParameterCallTarget>,
    resolution: ParameterCallResolution,
) {
    match resolution {
        ParameterCallResolution::Exact(target) => targets.push(target),
        ParameterCallResolution::Ambiguous(candidates) => targets.extend(candidates),
    }
}

fn push_path_parameter_target(
    call: &PathCallNode,
    target: ParameterCallTarget,
    relations: &mut Vec<CallRelation>,
) {
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

fn field_path_index(field_path: &[String]) -> Option<usize> {
    let [index] = field_path else {
        return None;
    };
    index.parse().ok()
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
