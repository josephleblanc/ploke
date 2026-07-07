use crate::parser::nodes::MethodCallReceiver;

use super::model::LocalBindingProof;
use super::{
    if_branch_paths, literal_usize, match_arm_paths, member_name, path_call_segments,
    self_field_path, unparen_expr, visible_local_binding,
};

pub(super) fn classify_method_receiver(
    receiver: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> MethodCallReceiver {
    let receiver = unparen_expr(receiver);
    match receiver {
        syn::Expr::Path(path) if path.qself.is_none() && path.path.is_ident("self") => {
            MethodCallReceiver::SelfValue
        }
        syn::Expr::Path(path) if path.qself.is_none() => {
            let Some(name) = path.path.get_ident().map(ToString::to_string) else {
                return MethodCallReceiver::Unsupported;
            };
            if let Some(binding) = visible_local_binding(&name, local_scopes) {
                return match binding {
                    LocalBindingProof::Typed {
                        name, type_path, ..
                    } => MethodCallReceiver::TypedLocalBinding {
                        name: name.clone(),
                        type_path: type_path.clone(),
                    },
                    LocalBindingProof::TraitObject {
                        name,
                        init_path: Some(init_path),
                        ..
                    } => MethodCallReceiver::InitializedLocalBinding {
                        name: name.clone(),
                        init_path: init_path.clone(),
                    },
                    LocalBindingProof::TraitObject {
                        name, trait_path, ..
                    } => MethodCallReceiver::TypedLocalBinding {
                        name: name.clone(),
                        type_path: trait_path.clone(),
                    },
                    LocalBindingProof::Initialized { name, init_path } => {
                        MethodCallReceiver::InitializedLocalBinding {
                            name: name.clone(),
                            init_path: init_path.clone(),
                        }
                    }
                    LocalBindingProof::Constructed {
                        name, type_path, ..
                    } => MethodCallReceiver::InitializedLocalBinding {
                        name: name.clone(),
                        init_path: type_path.clone(),
                    },
                    LocalBindingProof::Array { .. } => MethodCallReceiver::Unsupported,
                    LocalBindingProof::Referenced { name, type_path } => {
                        MethodCallReceiver::InitializedLocalBinding {
                            name: name.clone(),
                            init_path: type_path.clone(),
                        }
                    }
                    LocalBindingProof::Closure { .. } => MethodCallReceiver::Unsupported,
                    LocalBindingProof::LocalFunction { .. } => MethodCallReceiver::Unsupported,
                    LocalBindingProof::Untyped { .. } => MethodCallReceiver::Unsupported,
                };
            }

            param_names
                .iter()
                .any(|candidate| candidate == &name)
                .then_some(MethodCallReceiver::LocalBinding { name })
                .unwrap_or(MethodCallReceiver::Unsupported)
        }
        syn::Expr::Field(_) => self_field_path(receiver)
            .filter(|field_path| !field_path.is_empty())
            .map(|field_path| MethodCallReceiver::SelfField { field_path })
            .or_else(|| local_field_receiver(receiver, param_names, local_scopes))
            .unwrap_or(MethodCallReceiver::Unsupported),
        syn::Expr::Reference(reference) => {
            borrowed_local_receiver(reference.expr.as_ref(), param_names, local_scopes)
                .unwrap_or(MethodCallReceiver::Unsupported)
        }
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            dereferenced_local_receiver(unary.expr.as_ref(), param_names, local_scopes)
                .unwrap_or(MethodCallReceiver::Unsupported)
        }
        syn::Expr::Call(call) => {
            receiver_path_call(call).unwrap_or(MethodCallReceiver::Unsupported)
        }
        syn::Expr::MethodCall(call) => MethodCallReceiver::MethodCallResult {
            method_name: call.method.to_string(),
        },
        syn::Expr::Await(await_expr) => {
            receiver_await_path_call(await_expr).unwrap_or(MethodCallReceiver::AwaitResult)
        }
        syn::Expr::Try(try_expr) => {
            receiver_try_call(try_expr).unwrap_or(MethodCallReceiver::TryResult)
        }
        syn::Expr::If(_) => if_branch_paths(receiver, param_names, local_scopes)
            .map(|paths| MethodCallReceiver::IfBranchPaths { paths })
            .unwrap_or(MethodCallReceiver::Unsupported),
        syn::Expr::Match(_) => match_arm_paths(receiver, param_names, local_scopes)
            .map(|paths| MethodCallReceiver::IfBranchPaths { paths })
            .unwrap_or(MethodCallReceiver::Unsupported),
        syn::Expr::Lit(_) => MethodCallReceiver::Literal,
        _ => MethodCallReceiver::Unsupported,
    }
}

fn receiver_path_call(call: &syn::ExprCall) -> Option<MethodCallReceiver> {
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    let path = path_call_segments(path);
    (!path.is_empty()).then_some(MethodCallReceiver::PathCallResult { path })
}

fn receiver_await_path_call(await_expr: &syn::ExprAwait) -> Option<MethodCallReceiver> {
    let syn::Expr::Call(call) = unparen_expr(await_expr.base.as_ref()) else {
        return None;
    };
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    let path = path_call_segments(path);
    (!path.is_empty()).then_some(MethodCallReceiver::AwaitPathCallResult { path })
}

fn receiver_try_call(try_expr: &syn::ExprTry) -> Option<MethodCallReceiver> {
    match unparen_expr(try_expr.expr.as_ref()) {
        syn::Expr::Call(call) => {
            let syn::Expr::Path(path) = call.func.as_ref() else {
                return None;
            };
            let path = path_call_segments(path);
            (!path.is_empty()).then_some(MethodCallReceiver::TryPathCallResult { path })
        }
        syn::Expr::MethodCall(call) => Some(MethodCallReceiver::TryMethodCallResult {
            method_name: call.method.to_string(),
        }),
        _ => None,
    }
}

fn borrowed_local_receiver(
    receiver: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<MethodCallReceiver> {
    let syn::Expr::Path(path) = unparen_expr(receiver) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let name = path.path.get_ident()?.to_string();
    if let Some(LocalBindingProof::Typed {
        name, type_path, ..
    }) = visible_local_binding(&name, local_scopes)
    {
        return Some(MethodCallReceiver::BorrowedTypedLocalBinding {
            name: name.clone(),
            type_path: type_path.clone(),
        });
    }
    if let Some(LocalBindingProof::Initialized { name, init_path }) =
        visible_local_binding(&name, local_scopes)
    {
        return Some(MethodCallReceiver::BorrowedInitializedLocalBinding {
            name: name.clone(),
            init_path: init_path.clone(),
        });
    }

    (visible_local_binding(&name, local_scopes).is_some()
        || param_names.iter().any(|candidate| candidate == &name))
    .then_some(MethodCallReceiver::BorrowedLocalBinding { name })
}

fn dereferenced_local_receiver(
    receiver: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<MethodCallReceiver> {
    let syn::Expr::Path(path) = unparen_expr(receiver) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let name = path.path.get_ident()?.to_string();
    if let Some(LocalBindingProof::Referenced {
        name, type_path, ..
    }) = visible_local_binding(&name, local_scopes)
    {
        return Some(MethodCallReceiver::DereferencedInitializedLocalBinding {
            name: name.clone(),
            init_path: type_path.clone(),
        });
    }

    (visible_local_binding(&name, local_scopes).is_some()
        || param_names.iter().any(|candidate| candidate == &name))
    .then_some(MethodCallReceiver::DereferencedLocalBinding { name })
}

pub(super) fn local_field_path(
    receiver: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<(String, Vec<String>)> {
    let path = local_field_segments(receiver, param_names, local_scopes)?;
    let (name, fields) = path.split_first()?;
    (!fields.is_empty()).then(|| (name.clone(), fields.to_vec()))
}

fn local_field_receiver(
    receiver: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<MethodCallReceiver> {
    let (name, field_path) = local_field_path(receiver, param_names, local_scopes)?;
    if let Some(binding) = visible_local_binding(&name, local_scopes) {
        return match binding {
            LocalBindingProof::Typed {
                name, type_path, ..
            } => Some(MethodCallReceiver::FieldTypedLocalBinding {
                name: name.clone(),
                type_path: type_path.clone(),
                field_path,
            }),
            LocalBindingProof::TraitObject { .. } => {
                Some(MethodCallReceiver::FieldLocalBinding { name, field_path })
            }
            LocalBindingProof::Initialized { name, init_path } => {
                Some(MethodCallReceiver::FieldInitializedLocalBinding {
                    name: name.clone(),
                    init_path: init_path.clone(),
                    field_path,
                })
            }
            LocalBindingProof::Constructed {
                name, type_path, ..
            } => Some(MethodCallReceiver::FieldInitializedLocalBinding {
                name: name.clone(),
                init_path: type_path.clone(),
                field_path,
            }),
            LocalBindingProof::Array { .. } => {
                Some(MethodCallReceiver::FieldLocalBinding { name, field_path })
            }
            LocalBindingProof::Referenced { .. } => {
                Some(MethodCallReceiver::FieldLocalBinding { name, field_path })
            }
            LocalBindingProof::Closure { .. } => {
                Some(MethodCallReceiver::FieldLocalBinding { name, field_path })
            }
            LocalBindingProof::LocalFunction { .. } => {
                Some(MethodCallReceiver::FieldLocalBinding { name, field_path })
            }
            LocalBindingProof::Untyped { .. } => {
                Some(MethodCallReceiver::FieldLocalBinding { name, field_path })
            }
        };
    }

    param_names
        .iter()
        .any(|candidate| candidate == &name)
        .then_some(MethodCallReceiver::FieldLocalBinding { name, field_path })
}

fn local_field_segments(
    receiver: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    match unparen_expr(receiver) {
        syn::Expr::Path(path) if path.qself.is_none() => {
            let name = path.path.get_ident()?.to_string();
            (visible_local_binding(&name, local_scopes).is_some()
                || param_names.iter().any(|candidate| candidate == &name))
            .then_some(vec![name])
        }
        syn::Expr::Field(field) => {
            let mut path = local_field_segments(field.base.as_ref(), param_names, local_scopes)?;
            path.push(member_name(&field.member));
            Some(path)
        }
        syn::Expr::Index(index) => {
            let index_value = literal_usize(index.index.as_ref())?;
            let mut path = local_field_segments(index.expr.as_ref(), param_names, local_scopes)?;
            path.push(index_value.to_string());
            Some(path)
        }
        _ => None,
    }
}
