use syn::spanned::Spanned;

use crate::parser::nodes::DynamicCallCallee;

use super::{model::LocalBindingProof, visible_local_binding};

pub(super) struct FieldProjectionBinding {
    pub(super) name: String,
    pub(super) span: (usize, usize),
    pub(super) base_name: String,
    pub(super) base_span: (usize, usize),
    pub(super) base_type_path: Vec<String>,
    pub(super) field_path: Vec<String>,
    pub(super) init_path: Vec<String>,
}

pub(super) fn field_projection_binding(
    callee_expr: &syn::Expr,
    callee: &DynamicCallCallee,
    local_scopes: &[Vec<LocalBindingProof>],
) -> Option<FieldProjectionBinding> {
    let DynamicCallCallee::FieldInitializedLocalBinding { path, init_path } = callee else {
        return None;
    };
    let (base, field_path) = path.split_first()?;
    if field_path.is_empty() {
        return None;
    }
    let Some((base_span, base_type_path)) =
        visible_local_binding(base, local_scopes).and_then(|binding| match binding {
            LocalBindingProof::Constructed {
                span, type_path, ..
            } => Some((*span, type_path.clone())),
            _ => None,
        })
    else {
        return None;
    };

    let byte_range = callee_expr.span().byte_range();
    Some(FieldProjectionBinding {
        name: path.join("."),
        span: (byte_range.start, byte_range.end),
        base_name: base.clone(),
        base_span,
        base_type_path,
        field_path: field_path.to_vec(),
        init_path: init_path.clone(),
    })
}
