//! Structural call-site extraction from already-parsed `syn` bodies.
//!
//! This module records parser-owned call expression occurrences. It deliberately
//! stops at structural facts: no target method/function resolution is attempted
//! here.

use syn::spanned::Spanned;
use syn::visit::{self, Visit};

use crate::parser::nodes::{
    CallBodyOwnerId, CallNode, MethodCallNode, MethodCallReceiver, generate_method_call_site_id,
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
    fn visit_expr_method_call(&mut self, call: &'ast syn::ExprMethodCall) {
        self.record_method_call(call);
        visit::visit_expr_method_call(self, call);
    }
}

fn classify_method_receiver(receiver: &syn::Expr) -> Option<MethodCallReceiver> {
    match receiver {
        syn::Expr::Path(path) if path.qself.is_none() && path.path.is_ident("self") => {
            Some(MethodCallReceiver::SelfValue)
        }
        _ => None,
    }
}
