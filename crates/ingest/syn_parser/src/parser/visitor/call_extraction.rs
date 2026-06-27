//! Structural call-site extraction from already-parsed `syn` bodies.
//!
//! This module records parser-owned call expression occurrences. It deliberately
//! stops at structural facts: no target method/function resolution is attempted
//! here.

use syn::spanned::Spanned;
use syn::visit::{self, Visit};

use crate::parser::nodes::{
    CallBodyOwnerId, CallNode, DynamicCallCallee, DynamicCallNode, MacroCallNode, MethodCallNode,
    MethodCallReceiver, PathCallCallee, PathCallNode, generate_dynamic_call_site_id,
    generate_macro_call_site_id, generate_method_call_site_id, generate_path_call_site_id,
};
use crate::parser::relations::CallSiteRelation;

/// Extracts structural call-site facts from one function-like body.
///
/// The caller supplies the typed owner after the surrounding function or method
/// ID has already been generated. The returned facts are ready to append to
/// `CodeGraph.call_sites` and `CodeGraph.call_site_relations`.
pub(super) fn extract_body_call_sites(
    owner: CallBodyOwnerId,
    block: &syn::Block,
    cfgs: &[String],
    receiver_names: &[String],
) -> (Vec<CallNode>, Vec<CallSiteRelation>) {
    let mut visitor = BodyCallVisitor {
        owner,
        cfgs,
        param_names: receiver_names,
        local_scopes: Vec::new(),
        calls: Vec::new(),
        relations: Vec::new(),
    };
    visitor.visit_block(block);
    (visitor.calls, visitor.relations)
}

/// Extracts structural call-site facts from one item initializer expression.
pub(super) fn extract_expr_call_sites(
    owner: CallBodyOwnerId,
    expr: &syn::Expr,
    cfgs: &[String],
) -> (Vec<CallNode>, Vec<CallSiteRelation>) {
    let mut visitor = BodyCallVisitor {
        owner,
        cfgs,
        param_names: &[],
        local_scopes: Vec::new(),
        calls: Vec::new(),
        relations: Vec::new(),
    };
    visitor.visit_expr(expr);
    (visitor.calls, visitor.relations)
}

struct BodyCallVisitor<'a> {
    owner: CallBodyOwnerId,
    cfgs: &'a [String],
    param_names: &'a [String],
    local_scopes: Vec<Vec<LocalBindingProof>>,
    calls: Vec<CallNode>,
    relations: Vec<CallSiteRelation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LocalBindingProof {
    Typed {
        name: String,
        type_path: Vec<String>,
        init_path: Option<Vec<String>>,
    },
    TraitObject {
        name: String,
        trait_path: Vec<String>,
        init_path: Option<Vec<String>>,
    },
    Initialized {
        name: String,
        init_path: Vec<String>,
    },
    Constructed {
        name: String,
        type_path: Vec<String>,
        fields: ConstructedFields,
    },
    Array {
        name: String,
        element_init_paths: Vec<Option<Vec<String>>>,
    },
    Referenced {
        name: String,
        type_path: Vec<String>,
    },
    Untyped {
        name: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum ConstructedFields {
    Tuple(Vec<Option<FieldInitProof>>),
    Named(Vec<(String, Option<FieldInitProof>)>),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum FieldInitProof {
    Path(Vec<String>),
    Array(Vec<Option<Vec<String>>>),
}

impl LocalBindingProof {
    fn name(&self) -> &str {
        match self {
            Self::Typed { name, .. }
            | Self::TraitObject { name, .. }
            | Self::Initialized { name, .. }
            | Self::Constructed { name, .. }
            | Self::Array { name, .. }
            | Self::Referenced { name, .. }
            | Self::Untyped { name } => name,
        }
    }
}

impl BodyCallVisitor<'_> {
    fn record_macro_call(&mut self, mac: &syn::Macro) {
        let macro_name = path_discriminator(&mac.path);
        if macro_name.is_empty() {
            return;
        }

        let byte_range = mac.span().byte_range();
        let span = (byte_range.start, byte_range.end);
        let id = generate_macro_call_site_id(self.owner, &macro_name, span, self.cfgs);
        let target = id.into();

        self.calls.push(CallNode::MacroCall(MacroCallNode {
            id,
            owner: self.owner,
            span,
            cfgs: self.cfgs.to_vec(),
            macro_name,
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }

    fn record_path_call(&mut self, call: &syn::ExprCall) {
        let syn::Expr::Path(callee) = call.func.as_ref() else {
            return;
        };

        let path = path_call_segments(callee);
        if path.is_empty() {
            return;
        }

        let byte_range = call.span().byte_range();
        let span = (byte_range.start, byte_range.end);
        let id = generate_path_call_site_id(self.owner, &path, span, self.cfgs);
        let target = id.into();

        self.calls.push(CallNode::PathCall(PathCallNode {
            id,
            owner: self.owner,
            span,
            cfgs: self.cfgs.to_vec(),
            callee: classify_path_callee(&path, callee, self.param_names, &self.local_scopes),
            path,
            arg_count: call.args.len(),
            generic_arg_count: path_generic_arg_count(&callee.path),
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }

    fn record_dynamic_call(&mut self, call: &syn::ExprCall) {
        let byte_range = call.span().byte_range();
        let span = (byte_range.start, byte_range.end);
        let id = generate_dynamic_call_site_id(self.owner, span, self.cfgs);
        let target = id.into();

        self.calls.push(CallNode::DynamicCall(DynamicCallNode {
            id,
            owner: self.owner,
            span,
            cfgs: self.cfgs.to_vec(),
            arg_count: call.args.len(),
            callee: classify_dynamic_callee(&call.func, self.param_names, &self.local_scopes),
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }

    fn record_method_call(&mut self, call: &syn::ExprMethodCall) {
        let Some(receiver) =
            classify_method_receiver(&call.receiver, self.param_names, &self.local_scopes)
        else {
            return;
        };

        let method_name = call.method.to_string();
        let byte_range = call.span().byte_range();
        let span = (byte_range.start, byte_range.end);
        let id = generate_method_call_site_id(self.owner, &method_name, span, self.cfgs);
        let target = id.into();

        self.calls.push(CallNode::MethodCall(MethodCallNode {
            id,
            owner: self.owner,
            span,
            cfgs: self.cfgs.to_vec(),
            method_name,
            receiver,
            arg_count: call.args.len(),
            generic_arg_count: call
                .turbofish
                .as_ref()
                .map_or(0, |turbofish| turbofish.args.len()),
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }
}

impl<'ast> Visit<'ast> for BodyCallVisitor<'_> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.local_scopes.push(Vec::new());
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
        self.local_scopes.pop();
    }

    fn visit_local(&mut self, local: &'ast syn::Local) {
        let init_expr = local.init.as_ref().map(|init| init.expr.as_ref());

        if let Some(init) = &local.init {
            self.visit_expr(init.expr.as_ref());
            if let Some((_else_token, diverge)) = &init.diverge {
                self.visit_expr(diverge.as_ref());
            }
        }

        if let Some(binding) =
            local_binding_proof(&local.pat, init_expr, self.param_names, &self.local_scopes)
            && let Some(scope) = self.local_scopes.last_mut()
        {
            scope.push(binding);
        }
    }

    fn visit_expr_macro(&mut self, call: &'ast syn::ExprMacro) {
        self.record_macro_call(&call.mac);
        visit::visit_expr_macro(self, call);
    }

    fn visit_stmt_macro(&mut self, call: &'ast syn::StmtMacro) {
        self.record_macro_call(&call.mac);
        visit::visit_stmt_macro(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        if matches!(call.func.as_ref(), syn::Expr::Path(_)) {
            self.record_path_call(call);
        } else {
            self.record_dynamic_call(call);
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.record_method_call(call);
        visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_closure(&mut self, _closure: &'ast syn::ExprClosure) {}

    fn visit_expr_async(&mut self, _async_block: &'ast syn::ExprAsync) {}
}

fn path_segments(path: &syn::Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect()
}

fn path_call_segments(path: &syn::ExprPath) -> Vec<String> {
    let Some(qself) = &path.qself else {
        return path_segments(&path.path);
    };

    let associated_path = path
        .path
        .segments
        .iter()
        .skip(qself.position)
        .map(|segment| segment.ident.to_string())
        .collect::<Vec<_>>();
    if associated_path.is_empty() {
        return Vec::new();
    }

    let mut qualifier_path = if qself.position == 0 {
        type_path_segments(qself.ty.as_ref()).unwrap_or_default()
    } else {
        path.path
            .segments
            .iter()
            .take(qself.position)
            .map(|segment| segment.ident.to_string())
            .collect()
    };
    if qualifier_path.is_empty() {
        return Vec::new();
    }

    qualifier_path.extend(associated_path);
    qualifier_path
}

fn path_discriminator(path: &syn::Path) -> String {
    path_segments(path).join("::")
}

fn path_generic_arg_count(path: &syn::Path) -> usize {
    path.segments
        .iter()
        .map(|segment| match &segment.arguments {
            syn::PathArguments::AngleBracketed(args) => args.args.len(),
            syn::PathArguments::Parenthesized(_) | syn::PathArguments::None => 0,
        })
        .sum()
}

fn classify_method_receiver(
    receiver: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<MethodCallReceiver> {
    let receiver = unparen_expr(receiver);
    match receiver {
        syn::Expr::Path(path) if path.qself.is_none() && path.path.is_ident("self") => {
            Some(MethodCallReceiver::SelfValue)
        }
        syn::Expr::Path(path) if path.qself.is_none() => {
            let name = path.path.get_ident()?.to_string();
            if let Some(binding) = visible_local_binding(&name, local_scopes) {
                return match binding {
                    LocalBindingProof::Typed {
                        name, type_path, ..
                    } => Some(MethodCallReceiver::TypedLocalBinding {
                        name: name.clone(),
                        type_path: type_path.clone(),
                    }),
                    LocalBindingProof::TraitObject {
                        name,
                        init_path: Some(init_path),
                        ..
                    } => Some(MethodCallReceiver::InitializedLocalBinding {
                        name: name.clone(),
                        init_path: init_path.clone(),
                    }),
                    LocalBindingProof::TraitObject {
                        name, trait_path, ..
                    } => Some(MethodCallReceiver::TypedLocalBinding {
                        name: name.clone(),
                        type_path: trait_path.clone(),
                    }),
                    LocalBindingProof::Initialized { name, init_path } => {
                        Some(MethodCallReceiver::InitializedLocalBinding {
                            name: name.clone(),
                            init_path: init_path.clone(),
                        })
                    }
                    LocalBindingProof::Constructed {
                        name, type_path, ..
                    } => Some(MethodCallReceiver::InitializedLocalBinding {
                        name: name.clone(),
                        init_path: type_path.clone(),
                    }),
                    LocalBindingProof::Array { .. } => None,
                    LocalBindingProof::Referenced { name, type_path } => {
                        Some(MethodCallReceiver::InitializedLocalBinding {
                            name: name.clone(),
                            init_path: type_path.clone(),
                        })
                    }
                    LocalBindingProof::Untyped { .. } => None,
                };
            }

            param_names
                .iter()
                .any(|candidate| candidate == &name)
                .then_some(MethodCallReceiver::LocalBinding { name })
        }
        syn::Expr::Field(_) => self_field_path(receiver)
            .filter(|field_path| !field_path.is_empty())
            .map(|field_path| MethodCallReceiver::SelfField { field_path })
            .or_else(|| local_field_receiver(receiver, param_names, local_scopes)),
        syn::Expr::Reference(reference) => {
            borrowed_local_receiver(reference.expr.as_ref(), param_names, local_scopes)
        }
        syn::Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Deref(_)) => {
            dereferenced_local_receiver(unary.expr.as_ref(), param_names, local_scopes)
        }
        syn::Expr::Call(call) => receiver_path_call(call),
        syn::Expr::MethodCall(call) => Some(MethodCallReceiver::MethodCallResult {
            method_name: call.method.to_string(),
        }),
        syn::Expr::Await(await_expr) => {
            receiver_await_path_call(await_expr).or(Some(MethodCallReceiver::AwaitResult))
        }
        syn::Expr::Try(try_expr) => {
            receiver_try_path_call(try_expr).or(Some(MethodCallReceiver::TryResult))
        }
        syn::Expr::Lit(_) => Some(MethodCallReceiver::Literal),
        _ => None,
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

fn receiver_try_path_call(try_expr: &syn::ExprTry) -> Option<MethodCallReceiver> {
    let syn::Expr::Call(call) = unparen_expr(try_expr.expr.as_ref()) else {
        return None;
    };
    let syn::Expr::Path(path) = call.func.as_ref() else {
        return None;
    };
    let path = path_call_segments(path);
    (!path.is_empty()).then_some(MethodCallReceiver::TryPathCallResult { path })
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

fn local_field_path(
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
        _ => None,
    }
}

fn classify_path_callee(
    path: &[String],
    expr_path: &syn::ExprPath,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> PathCallCallee {
    if expr_path.qself.is_some() || path.len() != 1 {
        return PathCallCallee::ItemPath;
    }

    let name = &path[0];
    if let Some(binding) = visible_local_binding(name, local_scopes) {
        match binding {
            LocalBindingProof::Typed {
                init_path: Some(init_path),
                ..
            }
            | LocalBindingProof::Initialized { init_path, .. } => {
                PathCallCallee::InitializedValueBinding {
                    path: path.to_vec(),
                    init_path: init_path.clone(),
                }
            }
            LocalBindingProof::Typed {
                init_path: None, ..
            }
            | LocalBindingProof::TraitObject { .. }
            | LocalBindingProof::Constructed { .. }
            | LocalBindingProof::Array { .. }
            | LocalBindingProof::Referenced { .. }
            | LocalBindingProof::Untyped { .. } => PathCallCallee::ValueBinding {
                path: path.to_vec(),
            },
        }
    } else if param_names.iter().any(|candidate| candidate == name) {
        PathCallCallee::ValueBinding {
            path: path.to_vec(),
        }
    } else {
        PathCallCallee::ItemPath
    }
}

fn classify_dynamic_callee(
    callee: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> DynamicCallCallee {
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

    if let Some((path, init_path)) = indexed_initialized_path(callee, param_names, local_scopes) {
        return DynamicCallCallee::IndexedInitializedLocalBinding { path, init_path };
    }

    if let Some((path, init_path)) = dereferenced_initialized_path(callee, local_scopes) {
        return DynamicCallCallee::DereferencedInitializedLocalBinding { path, init_path };
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
                    | LocalBindingProof::Constructed { .. }
                    | LocalBindingProof::Array { .. }
                    | LocalBindingProof::Referenced { .. }
                    | LocalBindingProof::Untyped { .. } => DynamicCallCallee::Other,
                };
            }
            if param_names.iter().any(|candidate| candidate == name) {
                return DynamicCallCallee::FnPointerCastLocalBinding { path };
            }
        }
        return DynamicCallCallee::FnPointerCastPath { path };
    }

    if let Some(paths) = if_branch_paths(callee, param_names, local_scopes) {
        return DynamicCallCallee::IfBranchPaths { paths };
    }

    if let Some(paths) = match_arm_paths(callee, param_names, local_scopes) {
        return DynamicCallCallee::MatchArmPaths { paths };
    }

    if let Some(path) = block_path_expr(callee) {
        return classify_dynamic_path_expr(path, param_names, local_scopes);
    }

    let syn::Expr::Path(path) = unparen_expr(callee) else {
        return DynamicCallCallee::Other;
    };
    classify_dynamic_path_expr(path, param_names, local_scopes)
}

fn classify_dynamic_path_expr(
    path: &syn::ExprPath,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
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
                LocalBindingProof::Typed {
                    init_path: None, ..
                }
                | LocalBindingProof::TraitObject { .. }
                | LocalBindingProof::Constructed { .. }
                | LocalBindingProof::Array { .. }
                | LocalBindingProof::Referenced { .. }
                | LocalBindingProof::Untyped { .. } => DynamicCallCallee::LocalBinding { path },
            };
        }
        if param_names.iter().any(|candidate| candidate == name) {
            return DynamicCallCallee::LocalBinding { path };
        }
    }

    DynamicCallCallee::Path { path }
}

fn block_path_expr(callee: &syn::Expr) -> Option<&syn::ExprPath> {
    let syn::Expr::Block(block) = unparen_expr(callee) else {
        return None;
    };
    let [syn::Stmt::Expr(syn::Expr::Path(path), None)] = block.block.stmts.as_slice() else {
        return None;
    };
    Some(path)
}

fn if_branch_paths(
    callee: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<Vec<String>>> {
    let syn::Expr::If(branch) = unparen_expr(callee) else {
        return None;
    };
    let mut paths = block_branch_paths(&branch.then_branch, param_names, local_scopes)?;
    let (_else_token, else_expr) = branch.else_branch.as_ref()?;
    paths.extend(branch_paths(else_expr.as_ref(), param_names, local_scopes)?);

    paths
        .iter()
        .all(|path| is_unshadowed_item_path(path, param_names, local_scopes))
        .then_some(paths)
}

fn match_arm_paths(
    callee: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<Vec<String>>> {
    let syn::Expr::Match(expr) = unparen_expr(callee) else {
        return None;
    };
    if expr.arms.iter().any(|arm| arm.guard.is_some()) {
        return None;
    }

    let paths = expr
        .arms
        .iter()
        .map(|arm| branch_paths(arm.body.as_ref(), param_names, local_scopes))
        .collect::<Option<Vec<_>>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if paths.is_empty() {
        return None;
    }

    paths
        .iter()
        .all(|path| is_unshadowed_item_path(path, param_names, local_scopes))
        .then_some(paths)
}

fn branch_paths(
    expr: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<Vec<String>>> {
    match unparen_expr(expr) {
        syn::Expr::Path(path) => expr_path_segments(path).map(|path| vec![path]),
        syn::Expr::Block(block) => block_branch_paths(&block.block, param_names, local_scopes),
        syn::Expr::If(_) => if_branch_paths(expr, param_names, local_scopes),
        syn::Expr::Match(_) => match_arm_paths(expr, param_names, local_scopes),
        _ => None,
    }
}

fn block_branch_paths(
    block: &syn::Block,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<Vec<String>>> {
    let [syn::Stmt::Expr(expr, None)] = block.stmts.as_slice() else {
        return None;
    };
    branch_paths(expr, param_names, local_scopes)
}

fn expr_path_segments(path: &syn::ExprPath) -> Option<Vec<String>> {
    if path.qself.is_some() {
        return None;
    }

    let path = path_segments(&path.path);
    (!path.is_empty()).then_some(path)
}

fn is_unshadowed_item_path(
    path: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> bool {
    let [name] = path else {
        return true;
    };

    visible_local_binding(name, local_scopes).is_none()
        && !param_names.iter().any(|candidate| candidate == name)
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

fn literal_usize(expr: &syn::Expr) -> Option<usize> {
    let syn::Expr::Lit(lit) = unparen_expr(expr) else {
        return None;
    };
    let syn::Lit::Int(int) = &lit.lit else {
        return None;
    };
    int.base10_parse().ok()
}

fn dereferenced_initialized_path(
    callee: &syn::Expr,
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<(Vec<String>, Vec<String>)> {
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
    let init_path = match visible_local_binding(name, local_scopes)? {
        LocalBindingProof::Typed {
            init_path: Some(init_path),
            ..
        }
        | LocalBindingProof::Initialized { init_path, .. } => init_path.clone(),
        LocalBindingProof::Typed {
            init_path: None, ..
        }
        | LocalBindingProof::TraitObject { .. }
        | LocalBindingProof::Constructed { .. }
        | LocalBindingProof::Array { .. }
        | LocalBindingProof::Referenced { .. }
        | LocalBindingProof::Untyped { .. } => return None,
    };

    Some((path, init_path))
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

fn unparen_expr(expr: &syn::Expr) -> &syn::Expr {
    match expr {
        syn::Expr::Paren(paren) => unparen_expr(paren.expr.as_ref()),
        _ => expr,
    }
}

fn visible_local_binding<'a>(
    name: &str,
    local_scopes: &'a [Vec<LocalBindingProof>],
) -> Option<&'a LocalBindingProof> {
    local_scopes
        .iter()
        .rev()
        .flat_map(|scope| scope.iter().rev())
        .find(|binding| binding.name() == name)
}

fn local_binding_proof(
    pat: &syn::Pat,
    init_expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<LocalBindingProof> {
    match pat {
        syn::Pat::Ident(ident) => {
            let name = ident.ident.to_string();
            let proof = constructed_init(init_expr, param_names, local_scopes)
                .or_else(|| constructed_binding_init(init_expr, local_scopes))
                .map(|(type_path, fields)| LocalBindingProof::Constructed {
                    name: name.clone(),
                    type_path,
                    fields,
                })
                .or_else(|| {
                    array_init(init_expr, param_names, local_scopes).map(|element_init_paths| {
                        LocalBindingProof::Array {
                            name: name.clone(),
                            element_init_paths,
                        }
                    })
                })
                .or_else(|| {
                    array_binding_init(init_expr, local_scopes).map(|element_init_paths| {
                        LocalBindingProof::Array {
                            name: name.clone(),
                            element_init_paths,
                        }
                    })
                })
                .or_else(|| {
                    inferred_init_path(init_expr)
                        .and_then(|path| init_target_path(&path, param_names, local_scopes))
                        .map(|init_path| LocalBindingProof::Initialized {
                            name: name.clone(),
                            init_path,
                        })
                })
                .or_else(|| {
                    referenced_init_path(init_expr, param_names, local_scopes).map(|type_path| {
                        LocalBindingProof::Referenced {
                            name: name.clone(),
                            type_path,
                        }
                    })
                });
            let proof = proof.or_else(|| {
                referenced_alias_path(init_expr, param_names, local_scopes).map(|type_path| {
                    LocalBindingProof::Referenced {
                        name: name.clone(),
                        type_path,
                    }
                })
            });
            proof.or(Some(LocalBindingProof::Untyped { name }))
        }
        syn::Pat::Type(typed) => {
            let name = pat_ident_name(typed.pat.as_ref())?;
            if let Some((type_path, fields)) = constructed_binding_init(init_expr, local_scopes) {
                return Some(LocalBindingProof::Constructed {
                    name,
                    type_path,
                    fields,
                });
            }
            if let Some(element_init_paths) = array_init(init_expr, param_names, local_scopes)
                .or_else(|| array_binding_init(init_expr, local_scopes))
            {
                return Some(LocalBindingProof::Array {
                    name,
                    element_init_paths,
                });
            }
            let init_path = inferred_init_path(init_expr)
                .and_then(|path| init_target_path(&path, param_names, local_scopes));
            if let Some(trait_path) = typed_local_trait_object_path_segments(typed.ty.as_ref()) {
                return Some(LocalBindingProof::TraitObject {
                    name,
                    trait_path,
                    init_path: trait_object_init_path(init_expr, param_names, local_scopes),
                });
            }
            match typed_local_type_path_segments(typed.ty.as_ref()) {
                Some(type_path) => Some(LocalBindingProof::Typed {
                    name,
                    type_path,
                    init_path,
                }),
                None => init_path
                    .map(|init_path| LocalBindingProof::Initialized {
                        name: name.clone(),
                        init_path,
                    })
                    .or(Some(LocalBindingProof::Untyped { name })),
            }
        }
        _ => None,
    }
}

fn inferred_init_path(expr: Option<&syn::Expr>) -> Option<Vec<String>> {
    let path = match unparen_expr(expr?) {
        syn::Expr::Path(path) if path.qself.is_none() => &path.path,
        syn::Expr::Struct(expr) if expr.qself.is_none() => &expr.path,
        _ => return None,
    };
    let path = path_segments(path);
    (!path.is_empty()).then_some(path)
}

fn constructed_init(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<(Vec<String>, ConstructedFields)> {
    constructed_call_init(expr, param_names, local_scopes)
        .or_else(|| constructed_struct_init(expr, param_names, local_scopes))
}

fn constructed_binding_init(
    expr: Option<&syn::Expr>,
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<(Vec<String>, ConstructedFields)> {
    let syn::Expr::Path(path) = unparen_expr(expr?) else {
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
        LocalBindingProof::Constructed {
            type_path, fields, ..
        } => Some((type_path.clone(), fields.clone())),
        _ => None,
    }
}

fn constructed_call_init(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<(Vec<String>, ConstructedFields)> {
    let syn::Expr::Call(call) = unparen_expr(expr?) else {
        return None;
    };
    let syn::Expr::Path(path) = unparen_expr(call.func.as_ref()) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let path = path_segments(&path.path);
    if path.len() != 1 {
        return None;
    }

    let field_init_paths = call
        .args
        .iter()
        .map(|arg| constructed_field_init(arg, param_names, local_scopes))
        .collect::<Vec<_>>();

    Some((path, ConstructedFields::Tuple(field_init_paths)))
}

fn constructed_struct_init(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<(Vec<String>, ConstructedFields)> {
    let syn::Expr::Struct(expr) = unparen_expr(expr?) else {
        return None;
    };
    if expr.qself.is_some() || expr.rest.is_some() {
        return None;
    }

    let path = path_segments(&expr.path);
    if path.len() != 1 {
        return None;
    }

    let fields = expr
        .fields
        .iter()
        .map(|field| {
            let init = constructed_field_init(&field.expr, param_names, local_scopes);
            (member_name(&field.member), init)
        })
        .collect::<Vec<_>>();

    Some((path, ConstructedFields::Named(fields)))
}

fn constructed_field_init(
    expr: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<FieldInitProof> {
    match unparen_expr(expr) {
        syn::Expr::Path(path) if path.qself.is_none() => {
            let path = path_segments(&path.path);
            let [name] = path.as_slice() else {
                return init_target_path(&path, param_names, local_scopes)
                    .map(FieldInitProof::Path);
            };
            match visible_local_binding(name, local_scopes) {
                Some(LocalBindingProof::Array {
                    element_init_paths, ..
                }) => Some(FieldInitProof::Array(element_init_paths.clone())),
                _ => init_target_path(&path, param_names, local_scopes).map(FieldInitProof::Path),
            }
        }
        syn::Expr::Array(_) => {
            array_init(Some(expr), param_names, local_scopes).map(FieldInitProof::Array)
        }
        _ => None,
    }
}

fn array_init(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<Option<Vec<String>>>> {
    let syn::Expr::Array(array) = unparen_expr(expr?) else {
        return None;
    };

    Some(
        array
            .elems
            .iter()
            .map(|elem| {
                let syn::Expr::Path(path) = unparen_expr(elem) else {
                    return None;
                };
                if path.qself.is_some() {
                    return None;
                }
                let path = path_segments(&path.path);
                init_target_path(&path, param_names, local_scopes)
            })
            .collect(),
    )
}

fn array_binding_init(
    expr: Option<&syn::Expr>,
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<Option<Vec<String>>>> {
    let syn::Expr::Path(path) = unparen_expr(expr?) else {
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
        LocalBindingProof::Array {
            element_init_paths, ..
        } => Some(element_init_paths.clone()),
        _ => None,
    }
}

fn referenced_init_path(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    let syn::Expr::Reference(reference) = unparen_expr(expr?) else {
        return None;
    };
    let syn::Expr::Path(path) = unparen_expr(reference.expr.as_ref()) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let path = path_segments(&path.path);
    if path.is_empty() {
        return None;
    }
    if let [name] = path.as_slice() {
        if param_names.iter().any(|candidate| candidate == name) {
            return None;
        }
        if visible_local_binding(name, local_scopes).is_some() {
            return init_target_path(&path, param_names, local_scopes);
        }
    }

    Some(path)
}

fn trait_object_init_path(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    referenced_init_path(expr, param_names, local_scopes)
        .or_else(|| referenced_alias_path(expr, param_names, local_scopes))
}

fn referenced_alias_path(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    let syn::Expr::Path(path) = unparen_expr(expr?) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    let path = path_segments(&path.path);
    let [name] = path.as_slice() else {
        return None;
    };
    if param_names.iter().any(|candidate| candidate == name) {
        return None;
    }
    match visible_local_binding(name, local_scopes) {
        Some(LocalBindingProof::Referenced { type_path, .. }) => Some(type_path.clone()),
        _ => None,
    }
}

fn init_target_path(
    path: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    let [name] = path else {
        return Some(path.to_vec());
    };

    if param_names.iter().any(|candidate| candidate == name) {
        return None;
    }

    match visible_local_binding(name, local_scopes) {
        Some(
            LocalBindingProof::Typed {
                init_path: Some(init_path),
                ..
            }
            | LocalBindingProof::TraitObject {
                init_path: Some(init_path),
                ..
            }
            | LocalBindingProof::Initialized { init_path, .. },
        ) => Some(init_path.clone()),
        Some(
            LocalBindingProof::Typed {
                init_path: None, ..
            }
            | LocalBindingProof::TraitObject {
                init_path: None, ..
            }
            | LocalBindingProof::Constructed { .. }
            | LocalBindingProof::Array { .. }
            | LocalBindingProof::Referenced { .. }
            | LocalBindingProof::Untyped { .. },
        ) => None,
        None => Some(path.to_vec()),
    }
}

fn pat_ident_name(pat: &syn::Pat) -> Option<String> {
    match pat {
        syn::Pat::Ident(ident) => Some(ident.ident.to_string()),
        _ => None,
    }
}

fn type_path_segments(ty: &syn::Type) -> Option<Vec<String>> {
    match ty {
        syn::Type::Path(path) if path.qself.is_none() => Some(path_segments(&path.path)),
        _ => None,
    }
}

fn typed_local_type_path_segments(ty: &syn::Type) -> Option<Vec<String>> {
    direct_typed_local_type_path_segments(unparen_type(ty))
        .or_else(|| referenced_type_path_segments(ty))
}

fn typed_local_trait_object_path_segments(ty: &syn::Type) -> Option<Vec<String>> {
    match unparen_type(ty) {
        syn::Type::Reference(reference) => {
            trait_object_bound_path_segments(unparen_type(reference.elem.as_ref()))
        }
        ty => trait_object_bound_path_segments(ty),
    }
}

fn direct_typed_local_type_path_segments(ty: &syn::Type) -> Option<Vec<String>> {
    type_path_segments(ty).or_else(|| trait_object_bound_path_segments(ty))
}

fn referenced_type_path_segments(ty: &syn::Type) -> Option<Vec<String>> {
    match unparen_type(ty) {
        syn::Type::Reference(reference) => {
            direct_typed_local_type_path_segments(unparen_type(reference.elem.as_ref()))
                .or_else(|| referenced_type_path_segments(reference.elem.as_ref()))
        }
        _ => None,
    }
}

fn trait_object_bound_path_segments(ty: &syn::Type) -> Option<Vec<String>> {
    let syn::Type::TraitObject(object) = ty else {
        return None;
    };

    let trait_paths = object
        .bounds
        .iter()
        .filter_map(|bound| match bound {
            syn::TypeParamBound::Trait(trait_bound) => {
                let path = path_segments(&trait_bound.path);
                (!path.is_empty()).then_some(path)
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    match trait_paths.as_slice() {
        [trait_path] => Some(trait_path.clone()),
        [] | [_, ..] => None,
    }
}

fn unparen_type(ty: &syn::Type) -> &syn::Type {
    match ty {
        syn::Type::Paren(paren) => unparen_type(paren.elem.as_ref()),
        _ => ty,
    }
}

fn self_field_path(expr: &syn::Expr) -> Option<Vec<String>> {
    match expr {
        syn::Expr::Path(path) if path.qself.is_none() && path.path.is_ident("self") => {
            Some(Vec::new())
        }
        syn::Expr::Field(field) => {
            let mut field_path = self_field_path(field.base.as_ref())?;
            field_path.push(member_name(&field.member));
            Some(field_path)
        }
        _ => None,
    }
}

fn member_name(member: &syn::Member) -> String {
    match member {
        syn::Member::Named(ident) => ident.to_string(),
        syn::Member::Unnamed(index) => index.index.to_string(),
    }
}
