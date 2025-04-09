use quote::ToTokens;
use syn::{parse::Parser, spanned::Spanned};

use crate::parser::nodes::Attribute;

// Extract doc comments from attributes
pub(crate) fn extract_docstring(attrs: &[syn::Attribute]) -> Option<String> {
    let doc_lines: Vec<String> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("doc"))
        .filter_map(|attr| {
            if let Ok(syn::MetaNameValue {
                value:
                    syn::Expr::Lit(syn::ExprLit {
                        lit: syn::Lit::Str(lit_str),
                        ..
                    }),
                ..
            }) = attr.meta.require_name_value()
            {
                Some(lit_str.value().trim().to_string())
            } else {
                None
            }
        })
        .collect();

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    }
}

/// Parses a single syn::Attribute into our custom Attribute struct.
/// Uses syn::Attribute::parse_meta for robust parsing of different attribute forms.
fn parse_attribute(attr: &syn::Attribute) -> Attribute {
    let span = {
        let byte_range = attr.span().byte_range();
        (byte_range.start, byte_range.end)
    };

    match &attr.meta {
        // Case 1: Simple path attribute, e.g., #[test]
        syn::Meta::Path(path) => Attribute {
            span,
            name: path.to_token_stream().to_string(),
            args: Vec::new(),
            value: None,
        },
        // Case 2: List attribute, e.g., #[derive(Debug, Clone)]
        syn::Meta::List(list) => {
            let name = list.path.to_token_stream().to_string();
            let args =
                match syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
                    .parse2(list.tokens)
                {
                    Ok(nested_metas) => nested_metas
                        .iter()
                        .map(|meta| meta.to_token_stream().to_string())
                        .collect(),
                    Err(_) => {
                        // Fallback if tokens inside list aren't standard Meta items
                        // Might happen with complex custom attribute syntax.
                        // Store the raw tokens as a single argument string.
                        vec![list.tokens.to_string()]
                    }
                };
            Attribute {
                span,
                name,
                args,
                value: None,
            }
        }
        // Case 3: Name-value attribute, e.g., #[must_use = "reason"], #[path = "file.rs"]
        syn::Meta::NameValue(nv) => {
            let name = nv.path.to_token_stream().to_string();
            let value = match nv.value {
                // Prioritize string literals
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit_str),
                    ..
                }) => Some(lit_str.value()),
                // Handle other literals by converting to string
                syn::Expr::Lit(syn::ExprLit { lit, .. }) => Some(lit.to_token_stream().to_string()),
                // Fallback for complex expressions
                _ => Some(nv.value.to_token_stream().to_string()),
            };
            Attribute {
                span,
                name,
                args: Vec::new(),
                value,
            }
        }
    }
}

pub(crate) fn extract_attributes(attrs: &[syn::Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("doc")) // Skip doc comments
        .map(parse_attribute)
        .collect()
}
