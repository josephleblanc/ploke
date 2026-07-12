//! Structural call-site extraction from already-parsed `syn` bodies.
//!
//! This module records parser-owned call expression occurrences. It deliberately
//! stops at structural facts: no target method/function resolution is attempted
//! here.

use syn::spanned::Spanned;
use syn::visit::{self, Visit};

mod dynamic;
mod macro_expansion;
mod model;
mod receiver;

use dynamic::classify_dynamic_callee;
use macro_expansion::GeneratedCall;
pub(super) use macro_expansion::MacroExpansionContext;
use model::{ConstructedFields, FieldInitProof, LocalBindingProof};
use receiver::classify_method_receiver;

use crate::parser::nodes::{
    ArgumentFieldInit, CallArgument, CallBodyOwnerId, CallNode, DynamicCallCallee, DynamicCallNode,
    ExecutableBodyId, ExecutableBodyNode, MacroCallNode, MethodCallNode, PathCallCallee,
    PathCallNode, generate_async_block_body_id, generate_closure_body_id,
    generate_dynamic_call_site_id, generate_local_item_body_id, generate_macro_call_site_id,
    generate_method_call_site_id, generate_path_call_site_id,
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
    macro_expansions: &MacroExpansionContext,
) -> (
    Vec<CallNode>,
    Vec<CallSiteRelation>,
    Vec<ExecutableBodyNode>,
) {
    let mut visitor = BodyCallVisitor {
        owner,
        cfgs,
        param_names: receiver_names,
        macro_expansions,
        local_scopes: Vec::new(),
        calls: Vec::new(),
        relations: Vec::new(),
        executable_bodies: Vec::new(),
        awaited_call_spans: Vec::new(),
        unsafe_depth: 0,
    };
    visitor.visit_block(block);
    (visitor.calls, visitor.relations, visitor.executable_bodies)
}

/// Extracts structural call-site facts from one item initializer expression.
pub(super) fn extract_expr_call_sites(
    owner: CallBodyOwnerId,
    expr: &syn::Expr,
    cfgs: &[String],
) -> (
    Vec<CallNode>,
    Vec<CallSiteRelation>,
    Vec<ExecutableBodyNode>,
) {
    let macro_expansions = MacroExpansionContext::default();
    let mut visitor = BodyCallVisitor {
        owner,
        cfgs,
        param_names: &[],
        macro_expansions: &macro_expansions,
        local_scopes: Vec::new(),
        calls: Vec::new(),
        relations: Vec::new(),
        executable_bodies: Vec::new(),
        awaited_call_spans: Vec::new(),
        unsafe_depth: 0,
    };
    visitor.visit_expr(expr);
    (visitor.calls, visitor.relations, visitor.executable_bodies)
}

struct BodyCallVisitor<'a> {
    owner: CallBodyOwnerId,
    cfgs: &'a [String],
    param_names: &'a [String],
    macro_expansions: &'a MacroExpansionContext,
    local_scopes: Vec<Vec<LocalBindingProof>>,
    calls: Vec<CallNode>,
    relations: Vec<CallSiteRelation>,
    executable_bodies: Vec<ExecutableBodyNode>,
    awaited_call_spans: Vec<(usize, usize)>,
    unsafe_depth: usize,
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
            unsafe_block: self.unsafe_depth > 0,
            macro_name,
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }

    fn record_macro_path_expr_call(&mut self, expr: &syn::Expr, span: (usize, usize)) {
        let syn::Expr::Call(call) = expr else {
            return;
        };
        let syn::Expr::Path(callee) = call.func.as_ref() else {
            return;
        };

        let path = path_call_segments(callee);
        if path.is_empty() {
            return;
        }

        let id = generate_path_call_site_id(self.owner, &path, span, self.cfgs);
        let target = id.into();

        self.calls.push(CallNode::PathCall(PathCallNode {
            id,
            owner: self.owner,
            span,
            cfgs: self.cfgs.to_vec(),
            unsafe_block: self.unsafe_depth > 0,
            callee: classify_path_callee(
                &path,
                callee,
                self.param_names,
                &self.local_scopes,
                false,
            ),
            path,
            arg_count: call.args.len(),
            generic_arg_count: path_generic_arg_count(&callee.path),
            arguments: call_arguments(
                &call.args,
                self.owner,
                self.cfgs,
                self.param_names,
                &self.local_scopes,
            ),
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }

    fn record_generated_call(&mut self, call: GeneratedCall, span: (usize, usize)) {
        let unsafe_block = self.unsafe_depth > 0 || call.unsafe_block;
        let path_id = generate_path_call_site_id(self.owner, &call.path, span, self.cfgs);
        let path_target = path_id.into();

        self.calls.push(CallNode::PathCall(PathCallNode {
            id: path_id,
            owner: self.owner,
            span,
            cfgs: self.cfgs.to_vec(),
            unsafe_block,
            callee: PathCallCallee::ItemPath,
            path: call.path.clone(),
            arg_count: call.path_arg_count,
            generic_arg_count: call.generic_arg_count,
            arguments: vec![CallArgument::Other; call.path_arg_count],
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target: path_target,
        });

        let dynamic_id = generate_dynamic_call_site_id(self.owner, span, self.cfgs);
        let dynamic_target = dynamic_id.into();
        self.calls.push(CallNode::DynamicCall(DynamicCallNode {
            id: dynamic_id,
            owner: self.owner,
            span,
            cfgs: self.cfgs.to_vec(),
            unsafe_block,
            arg_count: call.dynamic_arg_count,
            callee: DynamicCallCallee::ReturnedPathCall { path: call.path },
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target: dynamic_target,
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
        let is_awaited = self.awaited_call_spans.contains(&span);

        self.calls.push(CallNode::PathCall(PathCallNode {
            id,
            owner: self.owner,
            span,
            cfgs: self.cfgs.to_vec(),
            unsafe_block: self.unsafe_depth > 0,
            callee: classify_path_callee(
                &path,
                callee,
                self.param_names,
                &self.local_scopes,
                is_awaited,
            ),
            path,
            arg_count: call.args.len(),
            generic_arg_count: path_generic_arg_count(&callee.path),
            arguments: call_arguments(
                &call.args,
                self.owner,
                self.cfgs,
                self.param_names,
                &self.local_scopes,
            ),
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
        let is_awaited = self.awaited_call_spans.contains(&span);

        self.calls.push(CallNode::DynamicCall(DynamicCallNode {
            id,
            owner: self.owner,
            span,
            cfgs: self.cfgs.to_vec(),
            unsafe_block: self.unsafe_depth > 0,
            arg_count: call.args.len(),
            callee: classify_dynamic_callee(
                &call.func,
                self.owner,
                self.cfgs,
                self.param_names,
                &self.local_scopes,
                is_awaited,
            ),
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }

    fn record_method_call(&mut self, call: &syn::ExprMethodCall) {
        let receiver =
            classify_method_receiver(&call.receiver, self.param_names, &self.local_scopes);

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
            unsafe_block: self.unsafe_depth > 0,
            method_name,
            receiver,
            arg_count: call.args.len(),
            generic_arg_count: call
                .turbofish
                .as_ref()
                .map_or(0, |turbofish| turbofish.args.len()),
            arguments: call_arguments(
                &call.args,
                self.owner,
                self.cfgs,
                self.param_names,
                &self.local_scopes,
            ),
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }
}

impl<'ast> Visit<'ast> for BodyCallVisitor<'_> {
    fn visit_block(&mut self, block: &'ast syn::Block) {
        self.local_scopes.push(local_function_bindings_in_block(
            block, self.owner, self.cfgs,
        ));
        let original_awaits = self.awaited_call_spans.len();
        self.awaited_call_spans.extend(awaited_future_spans(block));
        for stmt in &block.stmts {
            self.visit_stmt(stmt);
        }
        self.awaited_call_spans.truncate(original_awaits);
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

        let bindings = local_binding_proofs(
            &local.pat,
            init_expr,
            self.owner,
            self.cfgs,
            self.param_names,
            &self.local_scopes,
        );
        if let Some(scope) = self.local_scopes.last_mut() {
            scope.extend(bindings);
        }
    }

    fn visit_expr_macro(&mut self, call: &'ast syn::ExprMacro) {
        self.record_macro_call(&call.mac);
        if let Some(generated) = self.macro_expansions.generated_call_for(&call.mac) {
            let byte_range = call.mac.span().byte_range();
            self.record_generated_call(generated, (byte_range.start, byte_range.end));
        }
        visit::visit_expr_macro(self, call);
    }

    fn visit_stmt_macro(&mut self, call: &'ast syn::StmtMacro) {
        self.record_macro_call(&call.mac);
        let byte_range = call.mac.span().byte_range();
        let span = (byte_range.start, byte_range.end);
        if let Some(item) = self.macro_expansions.single_local_item_for(&call.mac) {
            self.record_macro_local_item(item, span);
        } else if let Some(expr) = self.macro_expansions.single_path_expr_for(&call.mac) {
            self.record_macro_path_expr_call(expr, span);
        }
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

    fn visit_expr_await(&mut self, await_expr: &'ast syn::ExprAwait) {
        if let syn::Expr::Call(call) = unparen_expr(await_expr.base.as_ref()) {
            let byte_range = call.span().byte_range();
            self.awaited_call_spans
                .push((byte_range.start, byte_range.end));
            visit::visit_expr_await(self, await_expr);
            self.awaited_call_spans.pop();
        } else {
            visit::visit_expr_await(self, await_expr);
        }
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.record_method_call(call);
        visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_match(&mut self, expr_match: &'ast syn::ExprMatch) {
        self.visit_expr(expr_match.expr.as_ref());

        for arm in &expr_match.arms {
            let bindings = match_arm_binding_proofs(
                &arm.pat,
                expr_match.expr.as_ref(),
                self.owner,
                self.cfgs,
                self.param_names,
                &self.local_scopes,
            );
            self.local_scopes.push(bindings);
            if let Some((_if_token, guard)) = &arm.guard {
                self.visit_expr(guard.as_ref());
            }
            self.visit_expr(arm.body.as_ref());
            self.local_scopes.pop();
        }
    }

    fn visit_expr_unsafe(&mut self, unsafe_expr: &'ast syn::ExprUnsafe) {
        self.unsafe_depth += 1;
        visit::visit_expr_unsafe(self, unsafe_expr);
        self.unsafe_depth -= 1;
    }

    fn visit_item_const(&mut self, item_const: &'ast syn::ItemConst) {
        let byte_range = item_const.span().byte_range();
        self.record_local_const_item(item_const, (byte_range.start, byte_range.end));
    }

    fn visit_item_fn(&mut self, item_fn: &'ast syn::ItemFn) {
        let byte_range = item_fn.span().byte_range();
        self.record_local_fn_item(item_fn, (byte_range.start, byte_range.end));
    }

    fn visit_item_impl(&mut self, item_impl: &'ast syn::ItemImpl) {
        for item in &item_impl.items {
            let syn::ImplItem::Fn(method) = item else {
                continue;
            };
            let byte_range = method.span().byte_range();
            let span = (byte_range.start, byte_range.end);
            let label = format!("local_impl_method:{}", method.sig.ident);
            let owner = self.record_local_item_owner(span, &label);

            let params = impl_method_param_names(method);
            let mut visitor = BodyCallVisitor {
                owner,
                cfgs: self.cfgs,
                param_names: &params,
                macro_expansions: self.macro_expansions,
                local_scopes: Vec::new(),
                calls: Vec::new(),
                relations: Vec::new(),
                executable_bodies: Vec::new(),
                awaited_call_spans: Vec::new(),
                unsafe_depth: 0,
            };
            visitor.visit_block(&method.block);
            self.append_child(visitor);
        }
    }

    fn visit_expr_closure(&mut self, closure: &'ast syn::ExprClosure) {
        let byte_range = closure.span().byte_range();
        let span = (byte_range.start, byte_range.end);
        let closure_id = generate_closure_body_id(self.owner, span, self.cfgs);
        let owner = CallBodyOwnerId::Executable(ExecutableBodyId::Closure(closure_id));
        self.executable_bodies.push(ExecutableBodyNode::new(
            closure_id.into(),
            self.owner,
            span,
            self.cfgs.to_vec(),
            Some(
                if closure.asyncness.is_some() {
                    "async_closure"
                } else {
                    "closure"
                }
                .to_string(),
            ),
        ));

        let params = closure_visible_param_names(self.param_names, closure);
        let mut visitor = BodyCallVisitor {
            owner,
            cfgs: self.cfgs,
            param_names: &params,
            macro_expansions: self.macro_expansions,
            local_scopes: Vec::new(),
            calls: Vec::new(),
            relations: Vec::new(),
            executable_bodies: Vec::new(),
            awaited_call_spans: Vec::new(),
            unsafe_depth: self.unsafe_depth,
        };
        visitor.visit_expr(closure.body.as_ref());
        self.calls.append(&mut visitor.calls);
        self.relations.append(&mut visitor.relations);
        self.executable_bodies
            .append(&mut visitor.executable_bodies);
    }

    fn visit_expr_async(&mut self, async_block: &'ast syn::ExprAsync) {
        let byte_range = async_block.span().byte_range();
        let span = (byte_range.start, byte_range.end);
        let body_id = generate_async_block_body_id(self.owner, span, self.cfgs);
        let owner = CallBodyOwnerId::Executable(ExecutableBodyId::AsyncBlock(body_id));
        self.executable_bodies.push(ExecutableBodyNode::new(
            body_id.into(),
            self.owner,
            span,
            self.cfgs.to_vec(),
            Some("async_block".to_string()),
        ));

        let mut visitor = BodyCallVisitor {
            owner,
            cfgs: self.cfgs,
            param_names: &[],
            macro_expansions: self.macro_expansions,
            local_scopes: Vec::new(),
            calls: Vec::new(),
            relations: Vec::new(),
            executable_bodies: Vec::new(),
            awaited_call_spans: Vec::new(),
            unsafe_depth: self.unsafe_depth,
        };
        visitor.visit_block(&async_block.block);
        self.calls.append(&mut visitor.calls);
        self.relations.append(&mut visitor.relations);
        self.executable_bodies
            .append(&mut visitor.executable_bodies);
    }

    fn visit_item_static(&mut self, item_static: &'ast syn::ItemStatic) {
        let byte_range = item_static.span().byte_range();
        let span = (byte_range.start, byte_range.end);
        self.record_local_static_item(item_static, span);
    }
}

impl BodyCallVisitor<'_> {
    fn record_macro_local_item(&mut self, item: &syn::Item, span: (usize, usize)) {
        match item {
            syn::Item::Fn(item_fn) => self.record_local_fn_item(item_fn, span),
            syn::Item::Const(item_const) => self.record_local_const_item(item_const, span),
            syn::Item::Static(item_static) => self.record_local_static_item(item_static, span),
            _ => {}
        }
    }

    fn record_local_const_item(&mut self, item_const: &syn::ItemConst, span: (usize, usize)) {
        let owner = self.record_local_item_owner(span, "local_const");

        let mut visitor = BodyCallVisitor {
            owner,
            cfgs: self.cfgs,
            param_names: &[],
            macro_expansions: self.macro_expansions,
            local_scopes: Vec::new(),
            calls: Vec::new(),
            relations: Vec::new(),
            executable_bodies: Vec::new(),
            awaited_call_spans: Vec::new(),
            unsafe_depth: 0,
        };
        visitor.visit_expr(item_const.expr.as_ref());
        self.append_child(visitor);
    }

    fn record_local_static_item(&mut self, item_static: &syn::ItemStatic, span: (usize, usize)) {
        let owner = self.record_local_item_owner(span, "local_static");

        let mut visitor = BodyCallVisitor {
            owner,
            cfgs: self.cfgs,
            param_names: &[],
            macro_expansions: self.macro_expansions,
            local_scopes: Vec::new(),
            calls: Vec::new(),
            relations: Vec::new(),
            executable_bodies: Vec::new(),
            awaited_call_spans: Vec::new(),
            unsafe_depth: 0,
        };
        visitor.visit_expr(item_static.expr.as_ref());
        self.append_child(visitor);
    }

    fn record_local_fn_item(&mut self, item_fn: &syn::ItemFn, span: (usize, usize)) {
        let name = item_fn.sig.ident.to_string();
        let label = format!("local_fn:{name}");
        let owner = self.record_local_item_owner(span, &label);
        let CallBodyOwnerId::Executable(body_id @ ExecutableBodyId::LocalItem(_)) = owner else {
            unreachable!("record_local_item_owner must return a local-item executable owner")
        };

        if let Some(scope) = self.local_scopes.last_mut() {
            scope.push(LocalBindingProof::LocalFunction {
                name: name.clone(),
                body_id,
            });
        }

        let params = local_fn_param_names(item_fn);
        let mut visitor = BodyCallVisitor {
            owner,
            cfgs: self.cfgs,
            param_names: &params,
            macro_expansions: self.macro_expansions,
            local_scopes: vec![vec![LocalBindingProof::LocalFunction { name, body_id }]],
            calls: Vec::new(),
            relations: Vec::new(),
            executable_bodies: Vec::new(),
            awaited_call_spans: Vec::new(),
            unsafe_depth: 0,
        };
        visitor.visit_block(item_fn.block.as_ref());
        self.append_child(visitor);
    }
    fn record_local_item_owner(&mut self, span: (usize, usize), label: &str) -> CallBodyOwnerId {
        let body_id = generate_local_item_body_id(self.owner, span, self.cfgs);
        let owner = CallBodyOwnerId::Executable(ExecutableBodyId::LocalItem(body_id));
        self.executable_bodies.push(ExecutableBodyNode::new(
            body_id.into(),
            self.owner,
            span,
            self.cfgs.to_vec(),
            Some(label.to_string()),
        ));
        owner
    }

    fn append_child(&mut self, mut visitor: BodyCallVisitor<'_>) {
        self.calls.append(&mut visitor.calls);
        self.relations.append(&mut visitor.relations);
        self.executable_bodies
            .append(&mut visitor.executable_bodies);
    }
}

fn local_function_bindings_in_block(
    block: &syn::Block,
    owner: CallBodyOwnerId,
    cfgs: &[String],
) -> Vec<LocalBindingProof> {
    block
        .stmts
        .iter()
        .filter_map(|stmt| {
            let syn::Stmt::Item(syn::Item::Fn(item_fn)) = stmt else {
                return None;
            };
            let byte_range = item_fn.span().byte_range();
            let span = (byte_range.start, byte_range.end);
            Some(LocalBindingProof::LocalFunction {
                name: item_fn.sig.ident.to_string(),
                body_id: ExecutableBodyId::LocalItem(generate_local_item_body_id(
                    owner, span, cfgs,
                )),
            })
        })
        .collect()
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

fn call_arguments(
    args: &syn::punctuated::Punctuated<syn::Expr, syn::Token![,]>,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Vec<CallArgument> {
    args.iter()
        .map(|arg| call_argument(arg, owner, cfgs, param_names, local_scopes))
        .collect()
}

fn call_argument(
    arg: &syn::Expr,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> CallArgument {
    match unparen_expr(arg) {
        syn::Expr::Path(path) if path.qself.is_none() => {
            let path = path_segments(&path.path);
            if path.is_empty() {
                CallArgument::Other
            } else if let Some(CallArgument::ClosureBinding { closure_id, .. }) =
                closure_binding_argument(arg, local_scopes)
            {
                CallArgument::ClosureBinding { path, closure_id }
            } else {
                CallArgument::Path { path }
            }
        }
        syn::Expr::Closure(closure) if closure.asyncness.is_none() => {
            let byte_range = closure.span().byte_range();
            let span = (byte_range.start, byte_range.end);
            CallArgument::Closure {
                closure_id: ExecutableBodyId::Closure(generate_closure_body_id(owner, span, cfgs)),
            }
        }
        syn::Expr::Reference(reference) => referenced_path_argument(reference),
        _ => closure_binding_argument(arg, local_scopes)
            .or_else(|| boxed_path_argument(arg, param_names, local_scopes))
            .or_else(|| array_argument(arg, param_names, local_scopes))
            .or_else(|| constructed_argument(arg, param_names, local_scopes))
            .unwrap_or(CallArgument::Other),
    }
}

fn referenced_path_argument(reference: &syn::ExprReference) -> CallArgument {
    let syn::Expr::Path(path) = unparen_expr(reference.expr.as_ref()) else {
        return CallArgument::Other;
    };
    if path.qself.is_some() {
        return CallArgument::Other;
    }
    let path = path_segments(&path.path);
    if path.is_empty() {
        CallArgument::Other
    } else {
        CallArgument::ReferencedPath { path }
    }
}

fn closure_binding_argument(
    arg: &syn::Expr,
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<CallArgument> {
    let path = closure_binding_arg_path(arg)?;
    let [name] = path.as_slice() else {
        return None;
    };
    let Some(LocalBindingProof::Closure {
        closure_id,
        is_async: false,
        ..
    }) = visible_local_binding(name, local_scopes)
    else {
        return None;
    };
    Some(CallArgument::ClosureBinding {
        path,
        closure_id: *closure_id,
    })
}

fn closure_binding_arg_path(arg: &syn::Expr) -> Option<Vec<String>> {
    match unparen_expr(arg) {
        syn::Expr::Path(path) if path.qself.is_none() => {
            let path = path_segments(&path.path);
            (!path.is_empty()).then_some(path)
        }
        syn::Expr::MethodCall(call) if call.method == "clone" && call.args.is_empty() => {
            let syn::Expr::Path(path) = unparen_expr(call.receiver.as_ref()) else {
                return None;
            };
            if path.qself.is_some() {
                return None;
            }
            let path = path_segments(&path.path);
            (!path.is_empty()).then_some(path)
        }
        _ => None,
    }
}

fn boxed_path_argument(
    arg: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<CallArgument> {
    boxed_init_path(Some(arg), param_names, local_scopes)
        .map(|path| CallArgument::BoxedPath { path })
}

fn array_argument(
    arg: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<CallArgument> {
    array_init(Some(arg), param_names, local_scopes)
        .map(|element_init_paths| CallArgument::Array { element_init_paths })
}

fn constructed_argument(
    arg: &syn::Expr,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<CallArgument> {
    let (type_path, fields) = constructed_init(Some(arg), param_names, local_scopes)?;
    let fields = argument_field_inits(&fields);
    (!fields.is_empty()).then_some(CallArgument::Constructed { type_path, fields })
}

fn argument_field_inits(fields: &ConstructedFields) -> Vec<ArgumentFieldInit> {
    let mut inits = Vec::new();
    match fields {
        ConstructedFields::Tuple(fields) => {
            for (index, init) in fields.iter().enumerate() {
                push_argument_init(&mut inits, vec![index.to_string()], init);
            }
        }
        ConstructedFields::Named(fields) => {
            for (name, init) in fields {
                push_argument_init(&mut inits, vec![name.clone()], init);
            }
        }
    }
    inits
}

fn push_argument_init(
    inits: &mut Vec<ArgumentFieldInit>,
    path: Vec<String>,
    init: &Option<FieldInitProof>,
) {
    match init {
        Some(FieldInitProof::Path(init_path)) => inits.push(ArgumentFieldInit {
            field_path: path,
            init_path: init_path.clone(),
        }),
        Some(FieldInitProof::Array(elements)) => {
            for (index, init_path) in elements.iter().enumerate() {
                if let Some(init_path) = init_path {
                    let mut field_path = path.clone();
                    field_path.push(index.to_string());
                    inits.push(ArgumentFieldInit {
                        field_path,
                        init_path: init_path.clone(),
                    });
                }
            }
        }
        None => {}
    }
}

fn classify_path_callee(
    path: &[String],
    expr_path: &syn::ExprPath,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
    is_awaited: bool,
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
            LocalBindingProof::AmbiguousInitialized { init_paths, .. } => {
                PathCallCallee::AmbiguousInitializedValueBinding {
                    path: path.to_vec(),
                    init_paths: init_paths.clone(),
                }
            }
            LocalBindingProof::TraitObject {
                trait_path,
                init_path: Some(init_path),
                ..
            } if is_callable_trait(trait_path) => PathCallCallee::InitializedValueBinding {
                path: path.to_vec(),
                init_path: init_path.clone(),
            },
            LocalBindingProof::Closure {
                closure_id,
                is_async,
                ..
            } => {
                if *is_async && is_awaited {
                    PathCallCallee::AwaitedAsyncClosureBinding {
                        path: path.to_vec(),
                        closure_id: *closure_id,
                    }
                } else if *is_async {
                    PathCallCallee::AsyncClosureBinding {
                        path: path.to_vec(),
                        closure_id: *closure_id,
                    }
                } else {
                    PathCallCallee::ClosureBinding {
                        path: path.to_vec(),
                        closure_id: *closure_id,
                    }
                }
            }
            LocalBindingProof::LocalFunction { body_id, .. } => {
                PathCallCallee::LocalFunctionBinding {
                    path: path.to_vec(),
                    body_id: *body_id,
                }
            }
            LocalBindingProof::ValueAlias { source_path, .. } => {
                PathCallCallee::AliasedValueBinding {
                    path: path.to_vec(),
                    source_path: source_path.clone(),
                }
            }
            LocalBindingProof::Typed {
                init_path: None, ..
            }
            | LocalBindingProof::TraitObject { .. }
            | LocalBindingProof::TupleReturn { .. }
            | LocalBindingProof::TupleMethodReturn { .. }
            | LocalBindingProof::MethodResult { .. }
            | LocalBindingProof::EnumVariantField { .. }
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

fn literal_usize(expr: &syn::Expr) -> Option<usize> {
    let syn::Expr::Lit(lit) = unparen_expr(expr) else {
        return None;
    };
    let syn::Lit::Int(int) = &lit.lit else {
        return None;
    };
    int.base10_parse().ok()
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
    owner: CallBodyOwnerId,
    cfgs: &[String],
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
                    path_call_init_path(init_expr)
                        .and_then(|path| init_target_path(&path, param_names, local_scopes))
                        .map(|init_path| LocalBindingProof::Initialized {
                            name: name.clone(),
                            init_path,
                        })
                })
                .or_else(|| {
                    method_result_init(init_expr).map(|(method_name, method_span)| {
                        LocalBindingProof::MethodResult {
                            name: name.clone(),
                            method_name,
                            method_span,
                        }
                    })
                })
                .or_else(|| {
                    branch_init_path(init_expr, param_names, local_scopes).map(|init_path| {
                        LocalBindingProof::Initialized {
                            name: name.clone(),
                            init_path,
                        }
                    })
                })
                .or_else(|| {
                    ambiguous_branch_init_paths(init_expr, param_names, local_scopes).map(
                        |init_paths| LocalBindingProof::AmbiguousInitialized {
                            name: name.clone(),
                            init_paths,
                        },
                    )
                })
                .or_else(|| {
                    value_alias_path(init_expr, param_names, local_scopes).map(|source_path| {
                        LocalBindingProof::ValueAlias {
                            name: name.clone(),
                            source_path,
                        }
                    })
                })
                .or_else(|| {
                    referenced_init_path(init_expr, param_names, local_scopes).map(|type_path| {
                        LocalBindingProof::Referenced {
                            name: name.clone(),
                            type_path,
                        }
                    })
                })
                .or_else(|| {
                    closure_binding_id(init_expr, owner, cfgs).map(|(closure_id, is_async)| {
                        LocalBindingProof::Closure {
                            name: name.clone(),
                            closure_id,
                            is_async,
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
                .and_then(|path| init_target_path(&path, param_names, local_scopes))
                .or_else(|| {
                    path_call_init_path(init_expr)
                        .and_then(|path| init_target_path(&path, param_names, local_scopes))
                })
                .or_else(|| branch_init_path(init_expr, param_names, local_scopes));
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
                None => {
                    let ambiguous =
                        ambiguous_branch_init_paths(init_expr, param_names, local_scopes);
                    init_path
                        .map(|init_path| LocalBindingProof::Initialized {
                            name: name.clone(),
                            init_path,
                        })
                        .or_else(|| {
                            ambiguous.map(|init_paths| LocalBindingProof::AmbiguousInitialized {
                                name: name.clone(),
                                init_paths,
                            })
                        })
                        .or(Some(LocalBindingProof::Untyped { name }))
                }
            }
        }
        _ => None,
    }
}

fn method_result_init(init_expr: Option<&syn::Expr>) -> Option<(String, (usize, usize))> {
    let syn::Expr::MethodCall(call) = unparen_expr(init_expr?) else {
        return None;
    };
    let byte_range = call.span().byte_range();
    Some((call.method.to_string(), (byte_range.start, byte_range.end)))
}

fn local_binding_proofs(
    pat: &syn::Pat,
    init_expr: Option<&syn::Expr>,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Vec<LocalBindingProof> {
    if let Some(proof) = local_binding_proof(pat, init_expr, owner, cfgs, param_names, local_scopes)
    {
        return vec![proof];
    }

    let struct_proofs =
        struct_binding_proofs(pat, init_expr, owner, cfgs, param_names, local_scopes);
    if !struct_proofs.is_empty() {
        return struct_proofs;
    }

    let enum_proofs = enum_variant_binding_proofs(pat, init_expr);
    if !enum_proofs.is_empty() {
        return enum_proofs;
    }

    tuple_binding_proofs(pat, init_expr, owner, cfgs, param_names, local_scopes)
}

fn match_arm_binding_proofs(
    pat: &syn::Pat,
    scrutinee: &syn::Expr,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Vec<LocalBindingProof> {
    local_binding_proofs(pat, Some(scrutinee), owner, cfgs, param_names, local_scopes)
}

fn tuple_binding_proofs(
    pat: &syn::Pat,
    init_expr: Option<&syn::Expr>,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Vec<LocalBindingProof> {
    if let Some(proofs) =
        direct_tuple_binding_proofs(pat, init_expr, owner, cfgs, param_names, local_scopes)
    {
        return proofs;
    }

    let typed = typed_tuple_binding_proofs(pat);
    if !typed.is_empty() {
        return typed;
    }

    tuple_return_binding_proofs(pat, init_expr)
}

fn struct_binding_proofs(
    pat: &syn::Pat,
    init_expr: Option<&syn::Expr>,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Vec<LocalBindingProof> {
    let syn::Pat::Struct(pattern) = pat else {
        return Vec::new();
    };
    if pattern.qself.is_some() || pattern.rest.is_some() {
        return Vec::new();
    }

    let Some(init_expr) = init_expr else {
        return Vec::new();
    };
    let syn::Expr::Struct(init) = unparen_expr(init_expr) else {
        return Vec::new();
    };
    if init.qself.is_some() || init.rest.is_some() {
        return Vec::new();
    }
    if path_segments(&pattern.path) != path_segments(&init.path) {
        return Vec::new();
    }

    pattern
        .fields
        .iter()
        .filter_map(|field| {
            let field_name = member_name(&field.member);
            let init_field = init
                .fields
                .iter()
                .find(|init_field| member_name(&init_field.member) == field_name)?;
            local_binding_proof(
                field.pat.as_ref(),
                Some(&init_field.expr),
                owner,
                cfgs,
                param_names,
                local_scopes,
            )
        })
        .collect()
}

fn enum_variant_binding_proofs(
    pat: &syn::Pat,
    init_expr: Option<&syn::Expr>,
) -> Vec<LocalBindingProof> {
    let syn::Pat::TupleStruct(pattern) = pat else {
        return Vec::new();
    };
    let Some(init_expr) = init_expr else {
        return Vec::new();
    };
    let scrutinee_path = match unparen_expr(init_expr) {
        syn::Expr::Path(path) => expr_path_segments(path),
        _ => None,
    };
    if pattern.qself.is_some()
        || !matches!(scrutinee_path.as_deref(), Some(path) if path == ["self"])
    {
        return Vec::new();
    }

    let path = path_segments(&pattern.path);
    let Some((variant_name, enum_path)) = path.split_last() else {
        return Vec::new();
    };
    if enum_path.is_empty() {
        return Vec::new();
    }

    pattern
        .elems
        .iter()
        .enumerate()
        .filter_map(|(field_index, pat)| {
            let name = pat_ident_name(pat)?;
            Some(LocalBindingProof::EnumVariantField {
                name,
                enum_path: enum_path.to_vec(),
                variant_name: variant_name.clone(),
                field_index,
            })
        })
        .collect()
}

fn direct_tuple_binding_proofs(
    pat: &syn::Pat,
    init_expr: Option<&syn::Expr>,
    owner: CallBodyOwnerId,
    cfgs: &[String],
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<LocalBindingProof>> {
    let syn::Pat::Tuple(pattern) = pat else {
        return None;
    };
    let init_expr = init_expr?;
    let syn::Expr::Tuple(init) = unparen_expr(init_expr) else {
        return None;
    };
    if pattern.elems.len() != init.elems.len() {
        return None;
    }

    Some(
        pattern
            .elems
            .iter()
            .zip(init.elems.iter())
            .filter_map(|(pat, expr)| {
                local_binding_proof(pat, Some(expr), owner, cfgs, param_names, local_scopes)
            })
            .collect(),
    )
}

fn typed_tuple_binding_proofs(pat: &syn::Pat) -> Vec<LocalBindingProof> {
    let syn::Pat::Type(typed) = pat else {
        return Vec::new();
    };
    let syn::Pat::Tuple(pattern) = typed.pat.as_ref() else {
        return Vec::new();
    };
    let syn::Type::Tuple(tuple) = unparen_type(typed.ty.as_ref()) else {
        return Vec::new();
    };
    if pattern.elems.len() != tuple.elems.len() {
        return Vec::new();
    }

    pattern
        .elems
        .iter()
        .zip(tuple.elems.iter())
        .filter_map(|(pat, ty)| {
            let name = pat_ident_name(pat)?;
            typed_local_type_path_segments(ty).map(|type_path| LocalBindingProof::Typed {
                name,
                type_path,
                init_path: None,
            })
        })
        .collect()
}

fn tuple_return_binding_proofs(
    pat: &syn::Pat,
    init_expr: Option<&syn::Expr>,
) -> Vec<LocalBindingProof> {
    let syn::Pat::Tuple(pattern) = pat else {
        return Vec::new();
    };

    if let Some(path) = tuple_return_call_path(init_expr) {
        return pattern
            .elems
            .iter()
            .enumerate()
            .filter_map(|(index, pat)| {
                Some(LocalBindingProof::TupleReturn {
                    name: pat_ident_name(pat)?,
                    path: path.clone(),
                    index,
                })
            })
            .collect();
    }

    let Some((method_name, method_span)) = tuple_return_method_call(init_expr) else {
        return Vec::new();
    };

    pattern
        .elems
        .iter()
        .enumerate()
        .filter_map(|(index, pat)| {
            Some(LocalBindingProof::TupleMethodReturn {
                name: pat_ident_name(pat)?,
                method_name: method_name.clone(),
                method_span,
                index,
            })
        })
        .collect()
}

fn tuple_return_call_path(expr: Option<&syn::Expr>) -> Option<Vec<String>> {
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
    (!path.is_empty()).then_some(path)
}

fn tuple_return_method_call(expr: Option<&syn::Expr>) -> Option<(String, (usize, usize))> {
    let syn::Expr::MethodCall(call) = unparen_expr(expr?) else {
        return None;
    };
    let byte_range = call.span().byte_range();
    Some((call.method.to_string(), (byte_range.start, byte_range.end)))
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct FutureBinding {
    path: Vec<String>,
    span: (usize, usize),
}

fn awaited_future_spans(block: &syn::Block) -> Vec<(usize, usize)> {
    let mut future_bindings = Vec::new();
    let mut awaited_spans = Vec::new();

    for stmt in &block.stmts {
        if let Some(path) = direct_await_path(stmt)
            && let Some(binding) = future_bindings
                .iter()
                .rev()
                .find(|binding: &&FutureBinding| binding.path == path)
        {
            awaited_spans.push(binding.span);
        }
        if let Some(binding) = future_call_binding(stmt) {
            future_bindings.push(binding);
        } else if let Some(bindings) = future_tuple_bindings(stmt) {
            future_bindings.extend(bindings);
        } else if let Some(bindings) = future_struct_bindings(stmt) {
            future_bindings.extend(bindings);
        } else if let Some((name, source)) = future_alias_binding(stmt)
            && let Some(binding) = future_bindings
                .iter()
                .rev()
                .find(|binding: &&FutureBinding| binding.path == source)
        {
            future_bindings.push(FutureBinding {
                path: vec![name],
                span: binding.span,
            });
        }
    }

    awaited_spans.sort();
    awaited_spans.dedup();
    awaited_spans
}

fn future_call_binding(stmt: &syn::Stmt) -> Option<FutureBinding> {
    let syn::Stmt::Local(local) = stmt else {
        return None;
    };
    let name = pat_ident_name(&local.pat)?;
    let init_expr = local.init.as_ref()?.expr.as_ref();
    let syn::Expr::Call(call) = unparen_expr(init_expr) else {
        return None;
    };
    let syn::Expr::Path(path) = unparen_expr(call.func.as_ref()) else {
        return None;
    };
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }

    let byte_range = call.span().byte_range();
    Some(FutureBinding {
        path: vec![name],
        span: (byte_range.start, byte_range.end),
    })
}

fn future_tuple_bindings(stmt: &syn::Stmt) -> Option<Vec<FutureBinding>> {
    let syn::Stmt::Local(local) = stmt else {
        return None;
    };
    let name = pat_ident_name(&local.pat)?;
    let init_expr = local.init.as_ref()?.expr.as_ref();
    let syn::Expr::Tuple(tuple) = unparen_expr(init_expr) else {
        return None;
    };

    let bindings = tuple
        .elems
        .iter()
        .enumerate()
        .filter_map(|(index, expr)| {
            let syn::Expr::Call(call) = unparen_expr(expr) else {
                return None;
            };
            let syn::Expr::Path(path) = unparen_expr(call.func.as_ref()) else {
                return None;
            };
            if path.qself.is_some() || path.path.segments.len() != 1 {
                return None;
            }
            let byte_range = call.span().byte_range();
            Some(FutureBinding {
                path: vec![name.clone(), index.to_string()],
                span: (byte_range.start, byte_range.end),
            })
        })
        .collect::<Vec<_>>();

    (!bindings.is_empty()).then_some(bindings)
}

fn future_struct_bindings(stmt: &syn::Stmt) -> Option<Vec<FutureBinding>> {
    let syn::Stmt::Local(local) = stmt else {
        return None;
    };
    let name = pat_ident_name(&local.pat)?;
    let init_expr = local.init.as_ref()?.expr.as_ref();
    let syn::Expr::Struct(expr) = unparen_expr(init_expr) else {
        return None;
    };
    if expr.qself.is_some() || expr.rest.is_some() {
        return None;
    }
    let type_path = path_segments(&expr.path);
    if type_path.len() != 1 {
        return None;
    }

    let bindings = expr
        .fields
        .iter()
        .filter_map(|field| {
            let syn::Expr::Call(call) = unparen_expr(&field.expr) else {
                return None;
            };
            let syn::Expr::Path(path) = unparen_expr(call.func.as_ref()) else {
                return None;
            };
            if path.qself.is_some() || path.path.segments.len() != 1 {
                return None;
            }
            let byte_range = call.span().byte_range();
            Some(FutureBinding {
                path: vec![name.clone(), member_name(&field.member)],
                span: (byte_range.start, byte_range.end),
            })
        })
        .collect::<Vec<_>>();

    (!bindings.is_empty()).then_some(bindings)
}

fn future_alias_binding(stmt: &syn::Stmt) -> Option<(String, Vec<String>)> {
    let syn::Stmt::Local(local) = stmt else {
        return None;
    };
    let name = pat_ident_name(&local.pat)?;
    let init_expr = local.init.as_ref()?.expr.as_ref();
    let source = match unparen_expr(init_expr) {
        syn::Expr::Path(path) => {
            if path.qself.is_some() {
                return None;
            }
            let segments = path_segments(&path.path);
            let [name] = segments.as_slice() else {
                return None;
            };
            vec![name.clone()]
        }
        syn::Expr::Field(_) => await_expr_path(init_expr)?,
        syn::Expr::Block(_) => {
            let path = block_path_expr(init_expr)?;
            if path.qself.is_some() {
                return None;
            }
            let segments = path_segments(&path.path);
            let [name] = segments.as_slice() else {
                return None;
            };
            vec![name.clone()]
        }
        _ => return None,
    };
    Some((name, source))
}

fn direct_await_path(stmt: &syn::Stmt) -> Option<Vec<String>> {
    let syn::Stmt::Expr(expr, _) = stmt else {
        return None;
    };
    let syn::Expr::Await(await_expr) = unparen_expr(expr) else {
        return None;
    };
    await_expr_path(await_expr.base.as_ref())
}

fn await_expr_path(expr: &syn::Expr) -> Option<Vec<String>> {
    match unparen_expr(expr) {
        syn::Expr::Path(path) => {
            if path.qself.is_some() {
                return None;
            }
            let segments = path_segments(&path.path);
            let [name] = segments.as_slice() else {
                return None;
            };
            Some(vec![name.clone()])
        }
        syn::Expr::Field(field) => {
            let syn::Expr::Path(base) = unparen_expr(field.base.as_ref()) else {
                return None;
            };
            if base.qself.is_some() {
                return None;
            }
            let segments = path_segments(&base.path);
            let [name] = segments.as_slice() else {
                return None;
            };
            Some(vec![name.clone(), member_name(&field.member)])
        }
        _ => None,
    }
}

fn closure_binding_id(
    expr: Option<&syn::Expr>,
    owner: CallBodyOwnerId,
    cfgs: &[String],
) -> Option<(ExecutableBodyId, bool)> {
    let syn::Expr::Closure(closure) = unparen_expr(expr?) else {
        return None;
    };
    let byte_range = closure.span().byte_range();
    let span = (byte_range.start, byte_range.end);
    let closure_id = ExecutableBodyId::Closure(generate_closure_body_id(owner, span, cfgs));
    Some((closure_id, closure.asyncness.is_some()))
}

fn inferred_init_path(expr: Option<&syn::Expr>) -> Option<Vec<String>> {
    let path = match unparen_expr(expr?) {
        syn::Expr::Path(path) if path.qself.is_none() => &path.path,
        syn::Expr::Struct(expr) if expr.qself.is_none() => &expr.path,
        syn::Expr::Block(_) => return block_path_expr(expr?).and_then(expr_path_segments),
        _ => return None,
    };
    let path = path_segments(path);
    (!path.is_empty()).then_some(path)
}

fn path_call_init_path(expr: Option<&syn::Expr>) -> Option<Vec<String>> {
    let syn::Expr::Call(call) = unparen_expr(expr?) else {
        return None;
    };
    let syn::Expr::Path(path) = unparen_expr(call.func.as_ref()) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }
    if !call.args.iter().all(is_unit_initializer_arg) {
        return None;
    }

    let path = path_segments(&path.path);
    (path.len() > 1).then_some(path)
}

fn is_unit_initializer_arg(expr: &syn::Expr) -> bool {
    matches!(unparen_expr(expr), syn::Expr::Tuple(tuple) if tuple.elems.is_empty())
}

fn branch_init_path(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    let targets = branch_init_targets(expr, param_names, local_scopes)?;

    match targets.as_slice() {
        [target] => Some(target.clone()),
        _ => None,
    }
}

fn ambiguous_branch_init_paths(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<Vec<String>>> {
    let targets = branch_init_targets(expr, param_names, local_scopes)?;
    (targets.len() > 1).then_some(targets)
}

fn branch_init_targets(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<Vec<String>>> {
    let expr = expr?;
    let branch_paths = match unparen_expr(expr) {
        syn::Expr::If(_) => if_branch_paths(expr, param_names, local_scopes)?,
        syn::Expr::Match(_) => match_arm_paths(expr, param_names, local_scopes)?,
        _ => return None,
    };

    let mut targets = branch_paths
        .iter()
        .map(|path| init_target_path(path, param_names, local_scopes))
        .collect::<Option<Vec<_>>>()?;
    targets.sort();
    targets.dedup();
    (!targets.is_empty()).then_some(targets)
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
    boxed_init_path(expr, param_names, local_scopes)
        .or_else(|| referenced_init_path(expr, param_names, local_scopes))
        .or_else(|| referenced_alias_path(expr, param_names, local_scopes))
}

fn boxed_init_path(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    let syn::Expr::Call(call) = unparen_expr(expr?) else {
        return None;
    };
    let syn::Expr::Path(func) = unparen_expr(call.func.as_ref()) else {
        return None;
    };
    if func.qself.is_some() {
        return None;
    }
    let func = path_segments(&func.path);
    if !is_box_new(&func) {
        return None;
    }

    let mut args = call.args.iter();
    let arg = args.next()?;
    if args.next().is_some() {
        return None;
    }
    let syn::Expr::Path(path) = unparen_expr(arg) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let path = path_segments(&path.path);
    if path.is_empty() {
        return None;
    }
    init_target_path(&path, param_names, local_scopes)
}

fn is_box_new(path: &[String]) -> bool {
    match path {
        [box_, new] => box_ == "Box" && new == "new",
        [root, boxed, box_, new] => {
            (root == "std" || root == "alloc") && boxed == "boxed" && box_ == "Box" && new == "new"
        }
        _ => false,
    }
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

fn value_alias_path(
    expr: Option<&syn::Expr>,
    param_names: &[String],
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<Vec<String>> {
    let path = match unparen_expr(expr?) {
        syn::Expr::Path(path) => path,
        syn::Expr::Reference(reference) => {
            let syn::Expr::Path(path) = unparen_expr(reference.expr.as_ref()) else {
                return None;
            };
            path
        }
        _ => return None,
    };
    if path.qself.is_some() {
        return None;
    }
    let path = path_segments(&path.path);
    let [name] = path.as_slice() else {
        return None;
    };
    if param_names.iter().any(|candidate| candidate == name) {
        return Some(path);
    }
    match visible_local_binding(name, local_scopes) {
        Some(LocalBindingProof::ValueAlias { source_path, .. }) => Some(source_path.clone()),
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
            | LocalBindingProof::AmbiguousInitialized { .. }
            | LocalBindingProof::TupleReturn { .. }
            | LocalBindingProof::TupleMethodReturn { .. }
            | LocalBindingProof::MethodResult { .. }
            | LocalBindingProof::EnumVariantField { .. }
            | LocalBindingProof::Closure { .. }
            | LocalBindingProof::LocalFunction { .. }
            | LocalBindingProof::ValueAlias { .. }
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

fn closure_param_names(closure: &syn::ExprClosure) -> Vec<String> {
    closure.inputs.iter().filter_map(pat_ident_name).collect()
}

fn closure_visible_param_names(parent: &[String], closure: &syn::ExprClosure) -> Vec<String> {
    let mut params = parent.to_vec();
    params.extend(closure_param_names(closure));
    params
}

fn local_fn_param_names(item_fn: &syn::ItemFn) -> Vec<String> {
    item_fn
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(typed) => pat_ident_name(typed.pat.as_ref()),
            syn::FnArg::Receiver(_) => None,
        })
        .collect()
}

fn impl_method_param_names(method: &syn::ImplItemFn) -> Vec<String> {
    method
        .sig
        .inputs
        .iter()
        .filter_map(|arg| match arg {
            syn::FnArg::Typed(typed) => pat_ident_name(typed.pat.as_ref()),
            syn::FnArg::Receiver(_) => None,
        })
        .collect()
}

fn type_path_segments(ty: &syn::Type) -> Option<Vec<String>> {
    match ty {
        syn::Type::Path(path) if path.qself.is_none() => Some(path_segments(&path.path)),
        ty => trait_object_bound_path_segments(ty),
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
                .or_else(|| boxed_trait_path(unparen_type(reference.elem.as_ref())))
        }
        ty => trait_object_bound_path_segments(ty).or_else(|| boxed_trait_path(ty)),
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

fn boxed_trait_path(ty: &syn::Type) -> Option<Vec<String>> {
    let syn::Type::Path(path) = ty else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let segment = path.path.segments.last()?;
    if segment.ident != "Box" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return None;
    };
    let traits = args
        .args
        .iter()
        .filter_map(|arg| match arg {
            syn::GenericArgument::Type(ty) => trait_object_bound_path_segments(unparen_type(ty)),
            _ => None,
        })
        .collect::<Vec<_>>();

    match traits.as_slice() {
        [trait_path] if is_callable_trait(trait_path) => Some(trait_path.clone()),
        [] | [_, _, ..] => None,
        [_] => None,
    }
}

fn is_callable_trait(path: &[String]) -> bool {
    path.last()
        .is_some_and(|name| matches!(name.as_str(), "Fn" | "FnMut" | "FnOnce"))
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
