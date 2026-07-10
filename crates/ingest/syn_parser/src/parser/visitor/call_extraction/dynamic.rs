use syn::spanned::Spanned;

use crate::parser::nodes::{
    CallBodyOwnerId, DynamicBranchTarget, DynamicCallCallee, ExecutableBodyId,
    generate_closure_body_id,
};

use super::model::{ConstructedFields, FieldInitProof, LocalBindingProof};
use super::receiver::local_field_path;
use super::{
    block_path_expr, expr_path_segments, if_branch_paths, is_callable_trait,
    is_unshadowed_item_path, literal_usize, match_arm_paths, path_segments, unparen_expr,
    visible_local_binding,
};

pub(super) fn classify_dynamic_callee(
    callee: &syn::Expr,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
    is_awaited: bool,
) -> DynamicCallCallee {
    if is_awaited && let Some(closure_id) = async_closure_literal_callee(callee, owner, cfgs) {
        return DynamicCallCallee::AwaitedAsyncClosureLiteral { closure_id };
    }

    if let Some(closure_id) = closure_literal_callee(callee, owner, cfgs) {
        return DynamicCallCallee::ClosureLiteral { closure_id };
    }

    if let Some((path, init_path)) = indexed_initialized_path(callee, param_names, local_scopes) {
        return DynamicCallCallee::IndexedInitializedLocalBinding { path, init_path };
    }

    if let Some((name, field_path)) = local_field_path(callee, param_names, local_scopes) {
        let mut path = Vec::with_capacity(field_path.len() + 1);
        path.push(name.clone());
        path.extend(field_path.iter().cloned());
        if let Some(LocalBindingProof::Constructed { fields, .. }) =
            visible_local_binding(&name, local_scopes)
            && let Some(init_path) = constructed_field_init_path(&field_path, fields)
        {
            return DynamicCallCallee::FieldInitializedLocalBinding { path, init_path };
        }
        return DynamicCallCallee::FieldLocalBinding { path };
    }

    if let Some(callee) = dereferenced_local_binding_callee(callee, local_scopes) {
        return callee;
    }

    if let Some(path) = fn_pointer_cast_path(callee) {
        if path.len() == 1 {
            let name = &path[0];
            if let Some(proof) = visible_local_binding(name, local_scopes) {
                return match proof {
                    LocalBindingProof::Typed {
                        init_path: Some(init_path),
                        ..
                    }
                    | LocalBindingProof::Initialized { init_path, .. } => {
                        DynamicCallCallee::FnPointerCastInitializedLocalBinding {
                            path,
                            init_path: init_path.clone(),
                        }
                    }
                    LocalBindingProof::Typed {
                        init_path: None, ..
                    }
                    | LocalBindingProof::TraitObject { .. }
                    | LocalBindingProof::AmbiguousInitialized { .. }
                    | LocalBindingProof::TupleReturn { .. }
                    | LocalBindingProof::TupleMethodReturn { .. }
                    | LocalBindingProof::Constructed { .. }
                    | LocalBindingProof::Array { .. }
                    | LocalBindingProof::Referenced { .. }
                    | LocalBindingProof::LocalFunction { .. }
                    | LocalBindingProof::Untyped { .. } => DynamicCallCallee::Other,
                    LocalBindingProof::ValueAlias { source_path, .. } => {
                        DynamicCallCallee::FnPointerCastAliasedLocalBinding {
                            path,
                            source_path: source_path.clone(),
                        }
                    }
                    LocalBindingProof::Closure {
                        closure_id,
                        is_async,
                        ..
                    } => {
                        if *is_async {
                            DynamicCallCallee::Other
                        } else {
                            DynamicCallCallee::FnPointerCastClosureBinding {
                                path,
                                closure_id: *closure_id,
                            }
                        }
                    }
                };
            }
            if param_names.iter().any(|candidate| candidate == name) {
                return DynamicCallCallee::FnPointerCastLocalBinding { path };
            }
        }
        return DynamicCallCallee::FnPointerCastPath { path };
    }

    if let Some(path) = returned_path_call(callee) {
        return DynamicCallCallee::ReturnedPathCall { path };
    }

    if let Some(paths) = if_branch_paths(callee, param_names, local_scopes) {
        return DynamicCallCallee::IfBranchPaths { paths };
    }

    if let Some(targets) = if_branch_targets(callee, owner, cfgs, param_names, local_scopes) {
        return DynamicCallCallee::IfBranchTargets { targets };
    }

    if let Some(path) = if_branch_parameter_path(callee, param_names, local_scopes) {
        return DynamicCallCallee::IfBranchParameter { path };
    }

    if let Some(paths) = match_arm_paths(callee, param_names, local_scopes) {
        return DynamicCallCallee::MatchArmPaths { paths };
    }

    if let Some(targets) = match_arm_targets(callee, owner, cfgs, param_names, local_scopes) {
        return DynamicCallCallee::MatchArmTargets { targets };
    }

    if let Some(path) = match_arm_parameter_path(callee, param_names, local_scopes) {
        return DynamicCallCallee::MatchArmParameter { path };
    }

    if let Some(path) = block_path_expr(callee) {
        return classify_dynamic_path_expr(path, param_names, local_scopes, is_awaited);
    }

    let syn::Expr::Path(path) = unparen_expr(callee) else {
        return DynamicCallCallee::Other;
    };
    classify_dynamic_path_expr(path, param_names, local_scopes, is_awaited)
}

fn closure_binding_callee(
    path: Vec<String>,
    closure_id: ExecutableBodyId,
    is_async: bool,
    is_awaited: bool,
) -> DynamicCallCallee {
    if is_async && is_awaited {
        DynamicCallCallee::AwaitedAsyncClosureBinding { path, closure_id }
    } else if is_async {
        DynamicCallCallee::AsyncClosureBinding { path, closure_id }
    } else {
        DynamicCallCallee::ClosureBinding { path, closure_id }
    }
}

fn closure_literal_callee(
    callee: &syn::Expr,
    owner: CallBodyOwnerId,
    cfgs: &[String],
) -> Option<ExecutableBodyId> {
    let syn::Expr::Closure(closure) = unparen_expr(callee) else {
        return None;
    };
    if closure.asyncness.is_some() {
        return None;
    }
    let byte_range = closure.span().byte_range();
    let span = (byte_range.start, byte_range.end);
    Some(ExecutableBodyId::Closure(generate_closure_body_id(
        owner, span, cfgs,
    )))
}

fn async_closure_literal_callee(
    callee: &syn::Expr,
    owner: CallBodyOwnerId,
    cfgs: &[String],
) -> Option<ExecutableBodyId> {
    let syn::Expr::Closure(closure) = unparen_expr(callee) else {
        return None;
    };
    closure.asyncness?;
    let byte_range = closure.span().byte_range();
    let span = (byte_range.start, byte_range.end);
    Some(ExecutableBodyId::Closure(generate_closure_body_id(
        owner, span, cfgs,
    )))
}

fn classify_dynamic_path_expr(
    path: &syn::ExprPath,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
    is_awaited: bool,
) -> DynamicCallCallee {
    if path.qself.is_some() {
        return DynamicCallCallee::Other;
    }

    let path = path_segments(&path.path);
    if path.is_empty() {
        return DynamicCallCallee::Other;
    }

    if path.len() == 1 {
        let name = &path[0];
        if let Some(proof) = visible_local_binding(name, local_scopes) {
            return match proof {
                LocalBindingProof::Typed {
                    init_path: Some(init_path),
                    ..
                }
                | LocalBindingProof::Initialized { init_path, .. } => {
                    DynamicCallCallee::InitializedLocalBinding {
                        path,
                        init_path: init_path.clone(),
                    }
                }
                LocalBindingProof::TraitObject {
                    trait_path,
                    init_path: Some(init_path),
                    ..
                } if is_callable_trait(trait_path) => DynamicCallCallee::InitializedLocalBinding {
                    path,
                    init_path: init_path.clone(),
                },
                LocalBindingProof::Closure {
                    closure_id,
                    is_async,
                    ..
                } => closure_binding_callee(path, *closure_id, *is_async, is_awaited),
                LocalBindingProof::ValueAlias { source_path, .. } => {
                    DynamicCallCallee::AliasedLocalBinding {
                        path,
                        source_path: source_path.clone(),
                    }
                }
                LocalBindingProof::Typed {
                    init_path: None, ..
                }
                | LocalBindingProof::TraitObject { .. }
                | LocalBindingProof::AmbiguousInitialized { .. }
                | LocalBindingProof::TupleReturn { .. }
                | LocalBindingProof::TupleMethodReturn { .. }
                | LocalBindingProof::Constructed { .. }
                | LocalBindingProof::Array { .. }
                | LocalBindingProof::Referenced { .. }
                | LocalBindingProof::LocalFunction { .. }
                | LocalBindingProof::Untyped { .. } => DynamicCallCallee::LocalBinding { path },
            };
        }
        if param_names.iter().any(|candidate| candidate == name) {
            return DynamicCallCallee::LocalBinding { path };
        }
    }

    DynamicCallCallee::Path { path }
}

fn if_branch_targets(
    callee: &syn::Expr,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<DynamicBranchTarget>> {
    let syn::Expr::If(branch) = unparen_expr(callee) else {
        return None;
    };
    let mut targets =
        block_branch_targets(&branch.then_branch, owner, cfgs, param_names, local_scopes)?;
    let (_else_token, else_expr) = branch.else_branch.as_ref()?;
    targets.extend(branch_targets(
        else_expr.as_ref(),
        owner,
        cfgs,
        param_names,
        local_scopes,
    )?);
    mixed_branch_targets(targets)
}

fn match_arm_targets(
    callee: &syn::Expr,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<DynamicBranchTarget>> {
    let syn::Expr::Match(expr) = unparen_expr(callee) else {
        return None;
    };

    let targets = expr
        .arms
        .iter()
        .map(|arm| branch_targets(arm.body.as_ref(), owner, cfgs, param_names, local_scopes))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    mixed_branch_targets(targets)
}

fn branch_targets(
    expr: &syn::Expr,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<DynamicBranchTarget>> {
    match unparen_expr(expr) {
        syn::Expr::Path(path) => {
            let path = expr_path_segments(path)?;
            is_unshadowed_item_path(&path, param_names, local_scopes)
                .then_some(vec![DynamicBranchTarget::Path { path }])
        }
        syn::Expr::Closure(_) => {
            let closure_id = closure_literal_callee(expr, owner, cfgs)?;
            Some(vec![DynamicBranchTarget::Closure { closure_id }])
        }
        syn::Expr::Block(block) => {
            block_branch_targets(&block.block, owner, cfgs, param_names, local_scopes)
        }
        syn::Expr::If(_) => if_branch_targets(expr, owner, cfgs, param_names, local_scopes),
        syn::Expr::Match(_) => match_arm_targets(expr, owner, cfgs, param_names, local_scopes),
        _ => None,
    }
}

fn block_branch_targets(
    block: &syn::Block,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<DynamicBranchTarget>> {
    let [syn::Stmt::Expr(expr, None)] = block.stmts.as_slice() else {
        return None;
    };
    branch_targets(expr, owner, cfgs, param_names, local_scopes)
}

fn mixed_branch_targets(targets: Vec<DynamicBranchTarget>) -> Option<Vec<DynamicBranchTarget>> {
    let has_closure = targets
        .iter()
        .any(|target| matches!(target, DynamicBranchTarget::Closure { .. }));
    (has_closure && !targets.is_empty()).then_some(targets)
}

fn if_branch_parameter_path(
    callee: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    let syn::Expr::If(branch) = unparen_expr(callee) else {
        return None;
    };
    let mut paths = block_parameter_paths(&branch.then_branch, param_names, local_scopes)?;
    let (_else_token, else_expr) = branch.else_branch.as_ref()?;
    paths.extend(parameter_branch_paths(
        else_expr.as_ref(),
        param_names,
        local_scopes,
    )?);
    same_parameter_path(paths)
}

fn match_arm_parameter_path(
    callee: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    let syn::Expr::Match(expr) = unparen_expr(callee) else {
        return None;
    };

    let paths = expr
        .arms
        .iter()
        .map(|arm| parameter_branch_paths(arm.body.as_ref(), param_names, local_scopes))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    same_parameter_path(paths)
}

fn parameter_branch_paths(
    expr: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<Vec<String>>> {
    match unparen_expr(expr) {
        syn::Expr::Path(path) => {
            parameter_path(path, param_names, local_scopes).map(|path| vec![path])
        }
        syn::Expr::Block(block) => block_parameter_paths(&block.block, param_names, local_scopes),
        syn::Expr::If(_) => {
            if_branch_parameter_path(expr, param_names, local_scopes).map(|path| vec![path])
        }
        syn::Expr::Match(_) => {
            match_arm_parameter_path(expr, param_names, local_scopes).map(|path| vec![path])
        }
        _ => None,
    }
}

fn block_parameter_paths(
    block: &syn::Block,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<Vec<String>>> {
    let [syn::Stmt::Expr(expr, None)] = block.stmts.as_slice() else {
        return None;
    };
    parameter_branch_paths(expr, param_names, local_scopes)
}

fn parameter_path(
    path: &syn::ExprPath,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    if path.qself.is_some() {
        return None;
    }

    let path = path_segments(&path.path);
    let [name] = path.as_slice() else {
        return None;
    };

    if visible_local_binding(name, local_scopes).is_some() {
        return None;
    }

    param_names
        .iter()
        .any(|candidate| candidate == name)
        .then_some(path)
}

fn same_parameter_path(paths: Vec<Vec<String>>) -> Option<Vec<String>> {
    let (first, rest) = paths.split_first()?;
    rest.iter()
        .all(|path| path == first)
        .then_some(first.clone())
}

fn indexed_initialized_path(
    callee: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<(Vec<String>, Vec<String>)> {
    let syn::Expr::Index(index) = unparen_expr(callee) else {
        return None;
    };
    let index_value = literal_usize(index.index.as_ref())?;

    match unparen_expr(index.expr.as_ref()) {
        syn::Expr::Path(path) if path.qself.is_none() => {
            let path = path_segments(&path.path);
            let [name] = path.as_slice() else {
                return None;
            };
            let init_path = match visible_local_binding(name, local_scopes)? {
                LocalBindingProof::Array {
                    element_init_paths, ..
                } => element_init_paths.get(index_value)?.clone()?,
                _ => return None,
            };

            let mut indexed_path = path;
            indexed_path.push(index_value.to_string());
            Some((indexed_path, init_path))
        }
        _ => {
            let (name, field_path) =
                local_field_path(index.expr.as_ref(), param_names, local_scopes)?;
            let init_path = match visible_local_binding(&name, local_scopes)? {
                LocalBindingProof::Constructed { fields, .. } => {
                    constructed_field_array_init_path(&field_path, index_value, fields)?
                }
                _ => return None,
            };

            let mut indexed_path = Vec::with_capacity(field_path.len() + 2);
            indexed_path.push(name);
            indexed_path.extend(field_path);
            indexed_path.push(index_value.to_string());
            Some((indexed_path, init_path))
        }
    }
}

fn dereferenced_local_binding_callee(
    callee: &syn::Expr,
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<DynamicCallCallee> {
    let syn::Expr::Unary(unary) = unparen_expr(callee) else {
        return None;
    };
    if !matches!(unary.op, syn::UnOp::Deref(_)) {
        return None;
    }

    let syn::Expr::Path(path) = unparen_expr(unary.expr.as_ref()) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let path = path_segments(&path.path);
    let [name] = path.as_slice() else {
        return None;
    };
    match visible_local_binding(name, local_scopes)? {
        LocalBindingProof::TraitObject {
            trait_path,
            init_path: Some(init_path),
            ..
        } if is_callable_trait(trait_path) => {
            Some(DynamicCallCallee::DereferencedInitializedLocalBinding {
                path,
                init_path: init_path.clone(),
            })
        }
        LocalBindingProof::Typed {
            init_path: Some(init_path),
            ..
        }
        | LocalBindingProof::Initialized { init_path, .. } => {
            Some(DynamicCallCallee::DereferencedInitializedLocalBinding {
                path,
                init_path: init_path.clone(),
            })
        }
        LocalBindingProof::Closure {
            closure_id,
            is_async,
            ..
        } if !is_async => Some(DynamicCallCallee::DereferencedClosureBinding {
            path,
            closure_id: *closure_id,
        }),
        LocalBindingProof::Closure { .. } => None,
        LocalBindingProof::Typed {
            init_path: None, ..
        }
        | LocalBindingProof::TraitObject { .. }
        | LocalBindingProof::AmbiguousInitialized { .. }
        | LocalBindingProof::TupleReturn { .. }
        | LocalBindingProof::TupleMethodReturn { .. }
        | LocalBindingProof::Constructed { .. }
        | LocalBindingProof::Array { .. }
        | LocalBindingProof::Referenced { .. }
        | LocalBindingProof::LocalFunction { .. }
        | LocalBindingProof::ValueAlias { .. }
        | LocalBindingProof::Untyped { .. } => None,
    }
}

fn fn_pointer_cast_path(callee: &syn::Expr) -> Option<Vec<String>> {
    let syn::Expr::Cast(cast) = unparen_expr(callee) else {
        return None;
    };
    if !matches!(cast.ty.as_ref(), syn::Type::BareFn(_)) {
        return None;
    }

    let syn::Expr::Path(path) = unparen_expr(cast.expr.as_ref()) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let path = path_segments(&path.path);
    (!path.is_empty()).then_some(path)
}

fn returned_path_call(callee: &syn::Expr) -> Option<Vec<String>> {
    let syn::Expr::Call(call) = unparen_expr(callee) else {
        return None;
    };
    let syn::Expr::Path(path) = unparen_expr(call.func.as_ref()) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let path = path_segments(&path.path);
    (!path.is_empty()).then_some(path)
}

fn constructed_field_init_path(
    field_path: &[String],
    fields: &ConstructedFields,
) -> Option<Vec<String>> {
    let [field] = field_path else {
        return None;
    };
    match fields {
        ConstructedFields::Tuple(field_paths) => {
            let index = field.parse::<usize>().ok()?;
            field_paths
                .get(index)?
                .as_ref()
                .and_then(|init| match init {
                    FieldInitProof::Path(path) => Some(path.clone()),
                    FieldInitProof::Array(_) => None,
                })
        }
        ConstructedFields::Named(field_paths) => field_paths
            .iter()
            .find(|(name, _)| name == field)
            .and_then(|(_, init)| match init {
                Some(FieldInitProof::Path(path)) => Some(path.clone()),
                Some(FieldInitProof::Array(_)) | None => None,
            }),
    }
}

fn constructed_field_array_init_path(
    field_path: &[String],
    index: usize,
    fields: &ConstructedFields,
) -> Option<Vec<String>> {
    let [field] = field_path else {
        return None;
    };
    match fields {
        ConstructedFields::Tuple(field_paths) => {
            let field_index = field.parse::<usize>().ok()?;
            field_paths
                .get(field_index)?
                .as_ref()
                .and_then(|init| match init {
                    FieldInitProof::Array(element_paths) => element_paths.get(index)?.clone(),
                    FieldInitProof::Path(_) => None,
                })
        }
        ConstructedFields::Named(field_paths) => field_paths
            .iter()
            .find(|(name, _)| name == field)
            .and_then(|(_, init)| match init {
                Some(FieldInitProof::Array(element_paths)) => element_paths.get(index)?.clone(),
                Some(FieldInitProof::Path(_)) | None => None,
            }),
    }
}
