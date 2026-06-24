//! Structural call-site extraction from legacy `syn1` initializer expressions.

use syn1::spanned::Spanned;
use syn1::visit::{self, Visit};

use crate::parser::nodes::{
    CallBodyOwnerId, CallNode, DynamicCallCallee, DynamicCallNode, MacroCallNode, MethodCallNode,
    MethodCallReceiver, PathCallCallee, PathCallNode, generate_dynamic_call_site_id,
    generate_macro_call_site_id, generate_method_call_site_id, generate_path_call_site_id,
};
use crate::parser::relations::CallSiteRelation;

pub(super) fn extract_expr_call_sites(
    owner: CallBodyOwnerId,
    expr: &syn1::Expr,
    cfgs: &[String],
) -> (Vec<CallNode>, Vec<CallSiteRelation>) {
    let mut visitor = ExprCallVisitor {
        owner,
        cfgs,
        receiver_names: &[],
        calls: Vec::new(),
        relations: Vec::new(),
    };
    visitor.visit_expr(expr);
    (visitor.calls, visitor.relations)
}

struct ExprCallVisitor<'a> {
    owner: CallBodyOwnerId,
    cfgs: &'a [String],
    receiver_names: &'a [String],
    calls: Vec<CallNode>,
    relations: Vec<CallSiteRelation>,
}

impl ExprCallVisitor<'_> {
    fn record_macro_call(&mut self, mac: &syn1::Macro) {
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

    fn record_path_call(&mut self, call: &syn1::ExprCall) {
        let syn1::Expr::Path(callee) = call.func.as_ref() else {
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
            path,
            callee: PathCallCallee::ItemPath,
            arg_count: call.args.len(),
            generic_arg_count: path_generic_arg_count(&callee.path),
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }

    fn record_dynamic_call(&mut self, call: &syn1::ExprCall) {
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
            callee: classify_dynamic_callee(&call.func),
        }));
        self.relations.push(CallSiteRelation::BodyContainsCall {
            source: self.owner,
            target,
        });
    }

    fn record_method_call(&mut self, call: &syn1::ExprMethodCall) {
        let Some(receiver) = classify_method_receiver(&call.receiver, self.receiver_names) else {
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

impl<'ast> Visit<'ast> for ExprCallVisitor<'_> {
    fn visit_expr_macro(&mut self, call: &'ast syn1::ExprMacro) {
        self.record_macro_call(&call.mac);
        visit::visit_expr_macro(self, call);
    }

    fn visit_expr_call(&mut self, call: &'ast syn1::ExprCall) {
        if matches!(call.func.as_ref(), syn1::Expr::Path(_)) {
            self.record_path_call(call);
        } else {
            self.record_dynamic_call(call);
        }
        visit::visit_expr_call(self, call);
    }

    fn visit_expr_method_call(&mut self, call: &'ast syn1::ExprMethodCall) {
        self.record_method_call(call);
        visit::visit_expr_method_call(self, call);
    }

    fn visit_expr_closure(&mut self, _closure: &'ast syn1::ExprClosure) {}

    fn visit_expr_async(&mut self, _async_block: &'ast syn1::ExprAsync) {}
}

fn path_segments(path: &syn1::Path) -> Vec<String> {
    path.segments
        .iter()
        .map(|segment| segment.ident.to_string())
        .collect()
}

fn path_call_segments(path: &syn1::ExprPath) -> Vec<String> {
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

fn path_discriminator(path: &syn1::Path) -> String {
    path_segments(path).join("::")
}

fn path_generic_arg_count(path: &syn1::Path) -> usize {
    path.segments
        .iter()
        .map(|segment| match &segment.arguments {
            syn1::PathArguments::AngleBracketed(args) => args.args.len(),
            syn1::PathArguments::Parenthesized(_) | syn1::PathArguments::None => 0,
        })
        .sum()
}

fn classify_method_receiver(
    receiver: &syn1::Expr,
    receiver_names: &[String],
) -> Option<MethodCallReceiver> {
    match receiver {
        syn1::Expr::Path(path) if path.qself.is_none() && path.path.is_ident("self") => {
            Some(MethodCallReceiver::SelfValue)
        }
        syn1::Expr::Path(path) if path.qself.is_none() => path
            .path
            .get_ident()
            .map(ToString::to_string)
            .filter(|name| receiver_names.iter().any(|candidate| candidate == name))
            .map(|name| MethodCallReceiver::LocalBinding { name }),
        syn1::Expr::Field(_) => self_field_path(receiver)
            .filter(|field_path| !field_path.is_empty())
            .map(|field_path| MethodCallReceiver::SelfField { field_path }),
        _ => None,
    }
}

fn classify_dynamic_callee(callee: &syn1::Expr) -> DynamicCallCallee {
    if let Some(path) = fn_pointer_cast_path(callee) {
        return DynamicCallCallee::FnPointerCastPath { path };
    }

    let syn1::Expr::Path(path) = unparen_expr(callee) else {
        return DynamicCallCallee::Other;
    };
    if path.qself.is_some() {
        return DynamicCallCallee::Other;
    }

    let path = path_segments(&path.path);
    if path.is_empty() {
        DynamicCallCallee::Other
    } else {
        DynamicCallCallee::Path { path }
    }
}

fn fn_pointer_cast_path(callee: &syn1::Expr) -> Option<Vec<String>> {
    let syn1::Expr::Cast(cast) = unparen_expr(callee) else {
        return None;
    };
    if !matches!(cast.ty.as_ref(), syn1::Type::BareFn(_)) {
        return None;
    }

    let syn1::Expr::Path(path) = unparen_expr(cast.expr.as_ref()) else {
        return None;
    };
    if path.qself.is_some() {
        return None;
    }

    let path = path_segments(&path.path);
    (!path.is_empty()).then_some(path)
}

fn unparen_expr(expr: &syn1::Expr) -> &syn1::Expr {
    match expr {
        syn1::Expr::Paren(paren) => unparen_expr(paren.expr.as_ref()),
        _ => expr,
    }
}

fn type_path_segments(ty: &syn1::Type) -> Option<Vec<String>> {
    match ty {
        syn1::Type::Path(path) if path.qself.is_none() => Some(path_segments(&path.path)),
        _ => None,
    }
}

fn self_field_path(expr: &syn1::Expr) -> Option<Vec<String>> {
    match expr {
        syn1::Expr::Path(path) if path.qself.is_none() && path.path.is_ident("self") => {
            Some(Vec::new())
        }
        syn1::Expr::Field(field) => {
            let mut field_path = self_field_path(field.base.as_ref())?;
            field_path.push(member_name(&field.member));
            Some(field_path)
        }
        _ => None,
    }
}

fn member_name(member: &syn1::Member) -> String {
    match member {
        syn1::Member::Named(ident) => ident.to_string(),
        syn1::Member::Unnamed(index) => index.index.to_string(),
    }
}
