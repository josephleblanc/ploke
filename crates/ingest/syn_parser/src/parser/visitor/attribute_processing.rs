use cfg_expr::Expression; // NEW: Import Expression
use quote::ToTokens;
use syn::{parse::Parser, spanned::Spanned};

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
        .filter(|attr| !attr.path().is_ident("doc") && !attr.path().is_ident("cfg")) // Skip doc AND cfg comments
        .map(parse_attribute)
        .collect()
}

/// Parses `#[cfg(...)]` attributes from a slice, combines them deterministically,
/// and returns a single `Option<Expression>`.
///
/// # Arguments
/// * `attrs` - A slice of `syn::Attribute` to parse.
///
/// # Returns
/// * `Some(Expression)` if one or more valid `cfg` attributes are found.
/// * `None` if no `cfg` attributes are found or if parsing fails.
///
/// # Determinism
/// If multiple `#[cfg]` attributes are present, they are combined into an
/// `Expression::All([...])`. The order of expressions within the `All` vector
/// is determined by sorting the string representations of the individual expressions
/// alphabetically and combining them into a new `all(...)` expression string, which
/// is then re-parsed. This ensures that the order of attributes in the source code
/// does not affect the resulting combined `Expression`'s `.original()` string.
pub(crate) fn parse_and_combine_cfgs_from_attrs(attrs: &[syn::Attribute]) -> Option<Expression> {
    let mut initial_expressions: Vec<Expression> = attrs
        .iter()
        .filter(|attr| attr.path().is_ident("cfg"))
        .filter_map(|attr| {
            // Extract the tokens inside the cfg(...)
            let tokens = match &attr.meta {
                syn::Meta::List(list) => Some(list.tokens.clone()),
                _ => {
                    eprintln!(
                        "Warning: Found #[cfg] attribute without list-like tokens: {:?}",
                        attr.path().to_token_stream()
                    );
                    None
                }
            };

            tokens.and_then(|t| {
                match Expression::parse(&t.to_string()) {
                    Ok(expr) => Some(expr),
                    Err(e) => {
                        eprintln!(
                            "Warning: Failed to parse cfg expression '{}': {}",
                            t.to_string(),
                            e
                        );
                        None // Skip invalid expressions
                    }
                }
            })
        })
        .collect();

    match initial_expressions.len() {
        0 => None,
        1 => initial_expressions.pop(), // Take the single expression
        _ => {
            // Multiple expressions: Combine into a canonical "all(...)" string and re-parse.
            let mut original_strings: Vec<&str> =
                initial_expressions.iter().map(|e| e.original()).collect();
            // Sort alphabetically for determinism.
            original_strings.sort_unstable();

            let combined_string = format!("all({})", original_strings.join(", "));

            // Re-parse the combined string.
            match Expression::parse(&combined_string) {
                Ok(combined_expr) => Some(combined_expr),
                Err(e) => {
                    eprintln!(
                        "Warning: Failed to re-parse combined cfg expression '{}': {}",
                        combined_string, e
                    );
                    // Fallback: Return None if re-parsing fails, though this shouldn't ideally happen.
                    None
                }
            }
        }
    }
}

// --- NEW: Functions for File-Level (Inner) Attributes ---

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
