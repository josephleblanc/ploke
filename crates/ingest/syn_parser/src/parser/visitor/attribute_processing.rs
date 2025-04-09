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
/// Uses the `attr.meta` field for structured parsing of different attribute forms.
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
            // Attempt to parse the tokens within the list as comma-separated meta items
            // This handles common cases like derive, cfg, allow, etc.
            let args =
                match syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
                .parse2(list.tokens.clone()) // Clone tokens for parsing
            {
                Ok(nested_metas) => nested_metas
                    .iter()
                    .map(|meta| meta.to_token_stream().to_string()) // Convert each parsed meta back to string
                    .collect(),
                Err(_) => {
                    // Fallback if tokens inside list aren't standard Meta items
                    // (e.g., custom attribute syntax). Store the raw tokens as a single argument string.
                    let raw_args = list.tokens.to_string();
                    if raw_args.is_empty() { // Avoid storing empty strings if list is empty
                        Vec::new()
                    } else {
                        vec![raw_args]
                    }
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
            // Extract the value, prioritizing string literals
            let value = match &nv.value {
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit_str),
                    ..
                }) => Some(lit_str.value()), // Extract the actual string content
                // Handle other literals by converting their token representation to string
                syn::Expr::Lit(syn::ExprLit { lit, .. }) => Some(lit.to_token_stream().to_string()),
                // Fallback for non-literal expressions (less common in standard attributes)
                expr => Some(expr.to_token_stream().to_string()),
            };
            Attribute {
                span,
                name,
                args: Vec::new(), // NameValue attributes don't have list-style args
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
