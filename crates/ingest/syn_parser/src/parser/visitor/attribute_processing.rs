// Removed cfg_expr::Expression import
use quote::ToTokens;
use syn::parse::Parser;

use crate::parser::nodes::Attribute;

// --- Functions for Item-Level (Outer) Attributes ---

/// Extracts the outer docstring (`///` or `/** ... */`) from item attributes.
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
    match &attr.meta {
        // Case 1: Simple path attribute, e.g., #[test]
        syn::Meta::Path(path) => Attribute {
            // span, // Removed, might need to put this back?
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
                // span,
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
                // span,
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
        .filter(|attr| !attr.path().is_ident("doc") && !attr.path().is_ident("cfg")) // Skip doc AND cfg comments
        .map(parse_attribute)
        .collect()
}

/// Extracts the raw string content from `#[cfg(...)]` attributes.
///
/// # Arguments
/// * `attrs` - A slice of `syn::Attribute` to parse.
///
/// # Returns
/// A `Vec<String>` containing the trimmed string representation of the tokens
/// inside each valid `#[cfg(...)]` attribute. Returns an empty Vec if none are found.
pub(crate) fn extract_cfg_strings(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .filter_map(|attr| {
            // Extract the tokens inside the cfg(...)
            match &attr.meta {
                syn::Meta::List(list) => {
                    let mut cfg_content = list.tokens.to_string();
                    // Normalize whitespace: replace multiple spaces with one, trim ends
                    cfg_content = cfg_content
                        .split_whitespace()
                        .collect::<Vec<&str>>()
                        .join(" ");
                    if cfg_content.is_empty() {
                        None // Ignore empty #[cfg()]
                    } else {
                        Some(cfg_content)
                    }
                }
                _ => {
                    // Log a warning for malformed #[cfg] attributes if needed
                    eprintln!(
                        "Warning: Found #[cfg] attribute without list-like tokens: {:?}",
                        attr.path().to_token_stream()
                    );
                    None
                }
            }
        })
        .collect()
}

// Removed parse_and_combine_cfgs_from_attrs function
/// Extracts the inner docstring (`//!`) from file attributes.
/// Expects `file.attrs` as input.
// NOTE: Purposefully duplicating logic here
// The motivation is to keep the logic and possibly extensions to the functionality of extracting
// file-level doc-strings isolated from in-file doc-string extraction
pub(crate) fn extract_file_level_docstring(attrs: &[syn::Attribute]) -> Option<String> {
    // Implementation is identical to extract_docstring for now
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

/// Extracts non-doc inner attributes (`#![...]`) from file attributes.
/// Expects `file.attrs` as input.
pub(crate) fn extract_file_level_attributes(attrs: &[syn::Attribute]) -> Vec<Attribute> {
    // Implementation is identical to extract_attributes for now
    attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("doc") && !attr.path().is_ident("cfg")) // Skip doc AND cfg comments
        .map(parse_attribute) // Uses the same helper
        .collect()
}
