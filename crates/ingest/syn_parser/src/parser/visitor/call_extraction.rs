//! Structural call-site extraction from already-parsed `syn` bodies.
//!
//! This module records parser-owned call expression occurrences. It deliberately
//! stops at structural facts: no target method/function resolution is attempted
//! here.

use syn::spanned::Spanned;
use syn::visit::{self, Visit};

use crate::parser::nodes::{
    CallBodyOwnerId, CallNode, MethodCallNode, MethodCallReceiver, PathCallNode,
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
) -> (Vec<CallNode>, Vec<CallSiteRelation>) {
    let mut visitor = BodyCallVisitor {
        owner,
        cfgs,
        calls: Vec::new(),
        relations: Vec::new(),
    };
    visitor.visit_block(block);
    (visitor.calls, visitor.relations)
}

struct BodyCallVisitor<'a> {
    owner: CallBodyOwnerId,
    cfgs: &'a [String],
    calls: Vec<CallNode>,
    relations: Vec<CallSiteRelation>,
}

impl BodyCallVisitor<'_> {
    fn record_path_call(&mut self, call: &syn::ExprCall) {
        let syn::Expr::Path(callee) = call.func.as_ref() else {
            return;
        };

        let path = callee
            .path
            .segments
            .iter()
            .map(|segment| segment.ident.to_string())
            .collect::<Vec<_>>();
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
            path,
            arg_count: call.args.len(),
            generic_arg_count: path_generic_arg_count(&callee.path),
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }

    fn record_method_call(&mut self, call: &syn::ExprMethodCall) {
        let Some(receiver) = classify_method_receiver(&call.receiver) else {
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
    fn visit_expr_call(&mut self, call: &'ast syn::ExprCall) {
        self.record_path_call(call);
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.record_method_call(call);
        visit::visit_expr_method_call(self, call);
    }
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

fn classify_method_receiver(receiver: &syn::Expr) -> Option<MethodCallReceiver> {
    match receiver {
        syn::Expr::Path(path) if path.qself.is_none() && path.path.is_ident("self") => {
            Some(MethodCallReceiver::SelfValue)
        }
        _ => None,
    }
}
