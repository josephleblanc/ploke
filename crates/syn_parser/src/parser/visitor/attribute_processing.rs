use quote::ToTokens;
use syn::parse::Parser;

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

fn parse_attribute(attr: &syn::Attribute) -> Attribute {
    let name = attr.path().to_token_stream().to_string();
    let mut args = Vec::new();

    match &attr.meta {
        syn::Meta::List(list) => {
            let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
            let nested_metas = parser.parse2(list.tokens.clone()).unwrap_or_default();
            for meta in nested_metas {
                args.push(meta.to_token_stream().to_string());
            }
        }
        syn::Meta::NameValue(name_value) => {
            args.push(name_value.value.to_token_stream().to_string());
        }
        syn::Meta::Path(path) => {
            args.push(path.to_token_stream().to_string());
        }
    }

    Attribute {
        name,
        args,
        value: Some(attr.to_token_stream().to_string()),
    }
}

pub(crate) fn extract_attributes(attrs: &[syn::Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("doc")) // Skip doc comments
        .map(parse_attribute)
        .collect()
}
