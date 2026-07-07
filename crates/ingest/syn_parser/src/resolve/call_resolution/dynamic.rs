use crate::{
    error::SynParserError,
    parser::{
        graph::GraphAccess,
        nodes::{
            AnyCallSiteId, CallBodyOwnerId, DynamicCallCallee, DynamicCallNode, ExecutableBodyId,
            ExecutableBodyKind, FunctionNodeId,
        },
        relations::{CallRelation, CallResolutionKind, CallResolutionStatus},
    },
};

use super::{CallRelationResolver, LocalFunctionPathResolution, path::ParameterCallTarget};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DynamicPathResolution {
    Resolved(FunctionNodeId),
    Unresolved,
    Ambiguous,
    External,
    Unsupported,
}

impl CallRelationResolver<'_> {
    pub(super) fn resolve_dynamic_call(
        &self,
        call: &DynamicCallNode,
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Dynamic(call.id);
        if let DynamicCallCallee::IfBranchPaths { paths } = &call.callee {
            self.resolve_if_branch_dynamic_call(call, paths, relations, statuses)?;
            return Ok(());
        }
        if let DynamicCallCallee::MatchArmPaths { paths } = &call.callee {
            self.resolve_if_branch_dynamic_call(call, paths, relations, statuses)?;
            return Ok(());
        }
        if let DynamicCallCallee::IfBranchParameter { path }
        | DynamicCallCallee::MatchArmParameter { path } = &call.callee
        {
            if let Some(target) = self.resolve_parameter_value_call(call.owner, path)? {
                push_dynamic_parameter_target(call, target, relations);
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            } else {
                statuses.push(CallResolutionStatus::Unsupported { source });
            }
            return Ok(());
        }

        if let DynamicCallCallee::InitializedLocalBinding { init_path, .. }
        | DynamicCallCallee::FnPointerCastInitializedLocalBinding { init_path, .. }
        | DynamicCallCallee::DereferencedInitializedLocalBinding { init_path, .. }
        | DynamicCallCallee::FieldInitializedLocalBinding { init_path, .. }
        | DynamicCallCallee::IndexedInitializedLocalBinding { init_path, .. } = &call.callee
        {
            self.resolve_initialized_dynamic_binding_call(call, init_path, relations, statuses)?;
            return Ok(());
        }

        if let DynamicCallCallee::ReturnedPathCall { path } = &call.callee {
            self.resolve_returned_path_call(call, path, relations, statuses)?;
            return Ok(());
        }

        if let DynamicCallCallee::LocalBinding { path }
        | DynamicCallCallee::FnPointerCastLocalBinding { path } = &call.callee
            && let Some(target) = self.resolve_parameter_value_call(call.owner, path)?
        {
            push_dynamic_parameter_target(call, target, relations);
            statuses.push(CallResolutionStatus::Resolved {
                source,
                kind: CallResolutionKind::LocalExact,
            });
            return Ok(());
        }

        if let DynamicCallCallee::AliasedLocalBinding { source_path, .. }
        | DynamicCallCallee::FnPointerCastAliasedLocalBinding { source_path, .. } = &call.callee
            && let Some(target) = self.resolve_parameter_value_call(call.owner, source_path)?
        {
            push_dynamic_parameter_target(call, target, relations);
            statuses.push(CallResolutionStatus::Resolved {
                source,
                kind: CallResolutionKind::LocalExact,
            });
            return Ok(());
        }

        if let DynamicCallCallee::FieldLocalBinding { path } = &call.callee
            && let Some(target) = self.resolve_parameter_field_call(call.owner, path)?
        {
            push_dynamic_parameter_target(call, target, relations);
            statuses.push(CallResolutionStatus::Resolved {
                source,
                kind: CallResolutionKind::LocalExact,
            });
            return Ok(());
        }

        if let DynamicCallCallee::ClosureBinding { closure_id, .. }
        | DynamicCallCallee::FnPointerCastClosureBinding { closure_id, .. }
        | DynamicCallCallee::DereferencedClosureBinding { closure_id, .. }
        | DynamicCallCallee::ClosureLiteral { closure_id }
        | DynamicCallCallee::AwaitedAsyncClosureLiteral { closure_id } = &call.callee
        {
            relations.push(CallRelation::DynamicClosure {
                source: call.id,
                target: *closure_id,
            });
            statuses.push(CallResolutionStatus::Resolved {
                source,
                kind: CallResolutionKind::LocalExact,
            });
            return Ok(());
        }

        let path = match &call.callee {
            DynamicCallCallee::Path { path } | DynamicCallCallee::FnPointerCastPath { path } => {
                path
            }
            DynamicCallCallee::LocalBinding { .. }
            | DynamicCallCallee::AliasedLocalBinding { .. }
            | DynamicCallCallee::ClosureBinding { .. }
            | DynamicCallCallee::ClosureLiteral { .. }
            | DynamicCallCallee::AwaitedAsyncClosureLiteral { .. }
            | DynamicCallCallee::InitializedLocalBinding { .. }
            | DynamicCallCallee::ReturnedPathCall { .. }
            | DynamicCallCallee::FnPointerCastInitializedLocalBinding { .. }
            | DynamicCallCallee::FnPointerCastLocalBinding { .. }
            | DynamicCallCallee::FnPointerCastAliasedLocalBinding { .. }
            | DynamicCallCallee::FnPointerCastClosureBinding { .. }
            | DynamicCallCallee::DereferencedInitializedLocalBinding { .. }
            | DynamicCallCallee::DereferencedClosureBinding { .. }
            | DynamicCallCallee::FieldLocalBinding { .. }
            | DynamicCallCallee::FieldInitializedLocalBinding { .. }
            | DynamicCallCallee::IndexedInitializedLocalBinding { .. }
            | DynamicCallCallee::IfBranchPaths { .. }
            | DynamicCallCallee::MatchArmPaths { .. }
            | DynamicCallCallee::IfBranchParameter { .. }
            | DynamicCallCallee::MatchArmParameter { .. }
            | DynamicCallCallee::Other => {
                statuses.push(CallResolutionStatus::Unsupported { source });
                return Ok(());
            }
        };

        if self.is_external_path(path) || self.is_external_import_path(call.owner, path)? {
            statuses.push(CallResolutionStatus::External { source });
            return Ok(());
        }

        let resolution = if self.is_unqualified_path(path) {
            self.resolve_unqualified_local_function_path(call.owner, path)?
        } else if self.is_explicit_local_path(path) {
            self.resolve_local_function_path(call.owner, path)?
        } else {
            self.resolve_implicit_local_function_path(call.owner, path)?
        };

        match resolution {
            LocalFunctionPathResolution::Resolved(target) => {
                relations.push(CallRelation::DynamicFunction {
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

    fn resolve_returned_path_call(
        &self,
        call: &DynamicCallNode,
        path: &[String],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Dynamic(call.id);
        let returning_function = match self.resolve_dynamic_path(call.owner, path)? {
            DynamicPathResolution::Resolved(target) => target,
            DynamicPathResolution::Unresolved => {
                statuses.push(CallResolutionStatus::Unresolved { source });
                return Ok(());
            }
            DynamicPathResolution::Ambiguous => {
                statuses.push(CallResolutionStatus::Ambiguous { source });
                return Ok(());
            }
            DynamicPathResolution::External => {
                statuses.push(CallResolutionStatus::External { source });
                return Ok(());
            }
            DynamicPathResolution::Unsupported => {
                statuses.push(CallResolutionStatus::Unsupported { source });
                return Ok(());
            }
        };

        if let Some(target) = self.direct_return_closure(returning_function)? {
            relations.push(CallRelation::DynamicClosure {
                source: call.id,
                target,
            });
            statuses.push(CallResolutionStatus::Resolved {
                source,
                kind: CallResolutionKind::LocalExact,
            });
            return Ok(());
        }

        let Some(return_path) = self.direct_return_path(returning_function)? else {
            statuses.push(CallResolutionStatus::Unsupported { source });
            return Ok(());
        };

        match self.resolve_dynamic_path(returning_function.into(), &return_path)? {
            DynamicPathResolution::Resolved(target) => {
                relations.push(CallRelation::DynamicFunction {
                    source: call.id,
                    target,
                });
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            }
            DynamicPathResolution::Unresolved => {
                statuses.push(CallResolutionStatus::Unresolved { source });
            }
            DynamicPathResolution::Ambiguous => {
                statuses.push(CallResolutionStatus::Ambiguous { source });
            }
            DynamicPathResolution::External => {
                statuses.push(CallResolutionStatus::External { source });
            }
            DynamicPathResolution::Unsupported => {
                statuses.push(CallResolutionStatus::Unsupported { source });
            }
        }

        Ok(())
    }

    fn direct_return_path(
        &self,
        function_id: FunctionNodeId,
    ) -> Result<Option<Vec<String>>, SynParserError> {
        let Some(expr) = self.direct_return_expr(function_id, "returned-call path proof")? else {
            return Ok(None);
        };
        Ok(expr_return_path(&expr))
    }

    fn direct_return_closure(
        &self,
        function_id: FunctionNodeId,
    ) -> Result<Option<ExecutableBodyId>, SynParserError> {
        let function = self.graph.get_function_checked(function_id)?;
        let Some(body) = function.body.as_deref() else {
            return Ok(None);
        };
        let block = syn::parse_str::<syn::Block>(body).map_err(|err| {
            SynParserError::InternalState(format!(
                "failed to parse stored function body for returned-closure proof in {}: {err}",
                function.name
            ))
        })?;
        let Some(syn::Stmt::Expr(expr, None)) = block.stmts.last() else {
            return Ok(None);
        };

        if is_closure_literal(expr) {
            return self.recorded_return_closure(function_id);
        }

        if let Some(name) = expr_path_ident(expr)
            && local_closure_binding_is_closure(&block, &name)
        {
            return self.recorded_return_closure(function_id);
        }

        Ok(None)
    }

    fn recorded_return_closure(
        &self,
        function_id: FunctionNodeId,
    ) -> Result<Option<ExecutableBodyId>, SynParserError> {
        let owner = CallBodyOwnerId::Function(function_id);
        let closures = self
            .graph
            .executable_bodies()
            .iter()
            .filter(|body| body.parent == owner && body.kind == ExecutableBodyKind::Closure)
            .collect::<Vec<_>>();

        match closures.as_slice() {
            [closure] => Ok(Some(closure.id)),
            [] => Err(SynParserError::InternalState(format!(
                "function {function_id} returns a closure but no closure executable body was recorded"
            ))),
            _ => Ok(None),
        }
    }

    fn direct_return_expr(
        &self,
        function_id: FunctionNodeId,
        context: &str,
    ) -> Result<Option<syn::Expr>, SynParserError> {
        let function = self.graph.get_function_checked(function_id)?;
        let Some(body) = function.body.as_deref() else {
            return Ok(None);
        };
        let block = syn::parse_str::<syn::Block>(body).map_err(|err| {
            SynParserError::InternalState(format!(
                "failed to parse stored function body for {context} in {}: {err}",
                function.name
            ))
        })?;
        let Some(syn::Stmt::Expr(expr, None)) = block.stmts.last() else {
            return Ok(None);
        };
        Ok(Some(expr.clone()))
    }

    fn resolve_initialized_dynamic_binding_call(
        &self,
        call: &DynamicCallNode,
        init_path: &[String],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Dynamic(call.id);

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
                relations.push(CallRelation::DynamicFunction {
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

    fn resolve_if_branch_dynamic_call(
        &self,
        call: &DynamicCallNode,
        paths: &[Vec<String>],
        relations: &mut Vec<CallRelation>,
        statuses: &mut Vec<CallResolutionStatus>,
    ) -> Result<(), SynParserError> {
        let source = AnyCallSiteId::Dynamic(call.id);
        if paths.is_empty() {
            statuses.push(CallResolutionStatus::Unsupported { source });
            return Ok(());
        }

        let mut targets = Vec::new();
        let mut has_external = false;
        let mut has_unresolved = false;

        for path in paths {
            match self.resolve_dynamic_path(call.owner, path)? {
                DynamicPathResolution::Resolved(target) => targets.push(target),
                DynamicPathResolution::Unresolved => has_unresolved = true,
                DynamicPathResolution::Ambiguous => {
                    statuses.push(CallResolutionStatus::Ambiguous { source });
                    return Ok(());
                }
                DynamicPathResolution::External => has_external = true,
                DynamicPathResolution::Unsupported => {
                    statuses.push(CallResolutionStatus::Unsupported { source });
                    return Ok(());
                }
            }
        }

        targets.sort_unstable();
        targets.dedup();

        match (targets.as_slice(), has_external, has_unresolved) {
            ([target], false, false) => {
                relations.push(CallRelation::DynamicFunction {
                    source: call.id,
                    target: *target,
                });
                statuses.push(CallResolutionStatus::Resolved {
                    source,
                    kind: CallResolutionKind::LocalExact,
                });
            }
            ([], true, false) => {
                statuses.push(CallResolutionStatus::External { source });
            }
            ([], false, true) => {
                statuses.push(CallResolutionStatus::Unresolved { source });
            }
            _ => {
                relations.extend(targets.iter().copied().map(|target| {
                    CallRelation::DynamicFunction {
                        source: call.id,
                        target,
                    }
                }));
                statuses.push(CallResolutionStatus::Ambiguous { source });
            }
        }

        Ok(())
    }

    fn resolve_dynamic_path(
        &self,
        owner: CallBodyOwnerId,
        path: &[String],
    ) -> Result<DynamicPathResolution, SynParserError> {
        if self.is_external_path(path) || self.is_external_import_path(owner, path)? {
            return Ok(DynamicPathResolution::External);
        }

        let resolution = if self.is_unqualified_path(path) {
            self.resolve_unqualified_local_function_path(owner, path)?
        } else if self.is_explicit_local_path(path) {
            self.resolve_local_function_path(owner, path)?
        } else {
            self.resolve_implicit_local_function_path(owner, path)?
        };

        Ok(match resolution {
            LocalFunctionPathResolution::Resolved(target) => {
                DynamicPathResolution::Resolved(target)
            }
            LocalFunctionPathResolution::Unresolved => DynamicPathResolution::Unresolved,
            LocalFunctionPathResolution::Ambiguous => DynamicPathResolution::Ambiguous,
            LocalFunctionPathResolution::Unsupported => DynamicPathResolution::Unsupported,
        })
    }
}

fn push_dynamic_parameter_target(
    call: &DynamicCallNode,
    target: ParameterCallTarget,
    relations: &mut Vec<CallRelation>,
) {
    match target {
        ParameterCallTarget::Function(target) => {
            relations.push(CallRelation::DynamicFunction {
                source: call.id,
                target,
            });
        }
        ParameterCallTarget::Closure(target) => {
            relations.push(CallRelation::DynamicClosure {
                source: call.id,
                target,
            });
        }
    }
}

fn expr_return_path(expr: &syn::Expr) -> Option<Vec<String>> {
    let syn::Expr::Path(path) = unparen_expr(expr) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let path = path
        .path
        .segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    (!path.is_empty()).then_some(path)
}

fn expr_path_ident(expr: &syn::Expr) -> Option<String> {
    let syn::Expr::Path(path) = unparen_expr(expr) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    path.path.get_ident().map(|ident| ident.to_string())
}

fn local_closure_binding_is_closure(block: &syn::Block, name: &str) -> bool {
    local_binding_closure_source(
        block,
        name,
        block.stmts.len().saturating_sub(1),
        &mut Vec::new(),
    )
}

fn local_binding_closure_source(
    block: &syn::Block,
    name: &str,
    end: usize,
    seen: &mut Vec<String>,
) -> bool {
    if seen.iter().any(|candidate| candidate == name) {
        return false;
    }
    seen.push(name.to_string());

    for index in (0..end.min(block.stmts.len())).rev() {
        let stmt = &block.stmts[index];
        let syn::Stmt::Local(local) = stmt else {
            continue;
        };
        let syn::Pat::Ident(ident) = &local.pat else {
            continue;
        };
        if ident.ident != name {
            continue;
        }
        let Some(init) = local.init.as_ref() else {
            return false;
        };
        if is_closure_literal(init.expr.as_ref()) {
            return true;
        }
        let Some(alias) = expr_path_ident(init.expr.as_ref()) else {
            return false;
        };
        return local_binding_closure_source(block, &alias, index, seen);
    }
    false
}

fn is_closure_literal(expr: &syn::Expr) -> bool {
    matches!(unparen_expr(expr), syn::Expr::Closure(_))
}

fn unparen_expr(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Paren(paren) => unparen_expr(paren.expr.as_ref()),
        _ => expr,
    }
}
