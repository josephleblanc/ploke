use syn::spanned::Spanned;

use crate::parser::nodes::{
    CallBodyOwnerId, LocalBindingKind, LocalBindingNode, LocalBindingSource,
    generate_local_binding_id,
};
use crate::parser::relations::LocalBindingRelation;

#[derive(Debug, Clone, PartialEq, Eq)]
struct ParameterBinding {
    name: String,
    span: (usize, usize),
}

pub(in crate::parser::visitor) fn parameter_names(
    inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
) -> Vec<String> {
    inputs
        .iter()
        .filter_map(|arg| typed_arg_binding(arg).map(|binding| binding.name))
        .collect()
}

pub(in crate::parser::visitor) fn extract_parameter_bindings(
    owner: CallBodyOwnerId,
    inputs: &syn::punctuated::Punctuated<syn::FnArg, syn::token::Comma>,
    cfgs: &[String],
) -> (Vec<LocalBindingRelation>, Vec<LocalBindingNode>) {
    inputs
        .iter()
        .filter_map(typed_arg_binding)
        .map(|binding| {
            let id = generate_local_binding_id(
                owner,
                &binding.name,
                binding.span,
                LocalBindingKind::ParameterBinding,
                cfgs,
            );
            (
                LocalBindingRelation::OwnerContainsBinding {
                    source: owner,
                    target: id,
                },
                LocalBindingNode {
                    id,
                    owner,
                    span: binding.span,
                    cfgs: cfgs.to_vec(),
                    kind: LocalBindingKind::ParameterBinding,
                    name: binding.name,
                    source: LocalBindingSource::Parameter,
                },
            )
        })
        .unzip()
}

fn typed_arg_binding(arg: &syn::FnArg) -> Option<ParameterBinding> {
    let syn::FnArg::Typed(typed) = arg else {
        return None;
    };
    let syn::Pat::Ident(ident) = typed.pat.as_ref() else {
        return None;
    };
    let byte_range = typed.pat.span().byte_range();
    Some(ParameterBinding {
        name: ident.ident.to_string(),
        span: (byte_range.start, byte_range.end),
    })
}
