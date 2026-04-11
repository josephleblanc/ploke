// Removed cfg_expr::Expression import
use quote::ToTokens;


use crate::parser::nodes::Attribute;

// --- Functions for Item-Level (Outer) Attributes ---

/// Extracts the outer docstring (`///` or `/** ... */`) from item attributes.
pub(crate) fn extract_docstring(attrs: &[syn1::Attribute]) -> Option<String> {
    let doc_lines: Vec<String> = attrs
        .iter()
        .filter(|attr| attr.path.is_ident("doc"))
        .filter_map(|attr| {
            // In syn 1.x, doc comments are stored as attributes with tokens like `= "doc content"`
            let tokens = attr.tokens.to_string();
            tokens.strip_prefix('=').and_then(|s| {
                let s = s.trim();
                s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).map(|s| s.to_string())
            })
        })
        .collect();

    if doc_lines.is_empty() {
        None
    } else {
        Some(doc_lines.join("\n"))
    }
}

/// Parses a single syn1::Attribute into our custom Attribute struct.
/// Uses `attr.parse_meta()` for syn 1.x compatibility.
fn parse_attribute(attr: &syn1::Attribute) -> Attribute {
    let meta = match attr.parse_meta() {
        Ok(m) => m,
        Err(_) => {
            // Fallback: just use the path and raw tokens
            return Attribute {
                name: attr.path.to_token_stream().to_string(),
                args: Vec::new(),
                value: if attr.tokens.is_empty() {
                    None
                } else {
                    Some(attr.tokens.to_string())
                },
            };
        }
    };
    
    match meta {
        // Case 1: Simple path attribute, e.g., #[test]
        syn1::Meta::Path(path) => Attribute {
            name: path.to_token_stream().to_string(),
            args: Vec::new(),
            value: None,
        },
        // Case 2: List attribute, e.g., #[derive(Debug, Clone)]
        syn1::Meta::List(list) => {
            let name = list.path.to_token_stream().to_string();
            // In syn 1.x, list.nested contains already-parsed items
            let args: Vec<String> = list.nested.iter()
                .map(|nested| nested.to_token_stream().to_string())
                .collect();
            Attribute {
                name,
                args,
                value: None,
            }
        }
        // Case 3: Name-value attribute, e.g., #[must_use = "reason"], #[path = "file.rs"]
        syn1::Meta::NameValue(nv) => {
            let name = nv.path.to_token_stream().to_string();
            // Extract the value from the literal
            let value = match nv.lit {
                syn1::Lit::Str(lit_str) => Some(lit_str.value()),
                other_lit => Some(other_lit.to_token_stream().to_string()),
            };
            Attribute {
                name,
                args: Vec::new(), // NameValue attributes don't have list-style args
                value,
            }
        }
    }
}

pub(crate) fn extract_attributes(attrs: &[syn1::Attribute]) -> Vec<Attribute> {
    attrs
        .iter()
        .filter(|attr| !attr.path.is_ident("doc") && !attr.path.is_ident("cfg")) // Skip doc AND cfg comments
        .map(parse_attribute)
        .collect()
}
use crate::parser::visitor::cfg_evaluator::{ActiveCfg, CfgAtom, CfgExpr};

/// Extracts and evaluates `#[cfg(...)]` attributes against active configuration.
///
/// # Arguments
/// * `attrs` - A slice of `syn1::Attribute` to parse.
/// * `active_cfg` - The active configuration to evaluate against.
///
/// # Returns
/// `true` if the item should be included (all cfg conditions are satisfied), `false` otherwise.
#[cfg(feature = "cfg_eval")]
pub(crate) fn should_include_item(attrs: &[syn1::Attribute], active_cfg: &ActiveCfg) -> bool {
    attrs.iter().any(|attr| {
        attr.path.is_ident("cfg")
            && parse_cfg_attribute(attr).map_or_else(
                || false,
                |expr| expr == CfgExpr::Atom(CfgAtom::Feature("test".into())),
            )
    }) || attrs
        .iter()
        .filter(|attr| attr.path.is_ident("cfg"))
        .all(|attr| {
            let expr = parse_cfg_attribute(attr);
            match expr {
                Some(cfg_expr) => active_cfg.eval(&cfg_expr),
                None => false, // Treat malformed cfg as include
            }
        })
}

/// Parse the inner tokens of a `#[cfg(...)]` attribute (as stored on graph nodes) into a
/// [`CfgExpr`], using the same rules as [`parse_cfg_attribute`].
#[cfg(feature = "cfg_eval")]
pub fn parse_cfg_expr_from_inner_tokens(inner: &str) -> Option<CfgExpr> {
    let meta: syn1::Meta = syn1::parse_str(inner).ok()?;
    parse_single_meta(meta)
}

/// Parse a `#[cfg(...)]` attribute into a CfgExpr
#[cfg(feature = "cfg_eval")]
fn parse_cfg_attribute(attr: &syn1::Attribute) -> Option<CfgExpr> {
    if !attr.path.is_ident("cfg") {
        return None;
    }
    let meta = attr.parse_meta().ok()?;
    let syn1::Meta::List(list) = meta else {
        return None;
    };

    // In syn 1.x, list.nested is already parsed
    let nested: Vec<syn1::NestedMeta> = list.nested.into_iter().collect();
    if nested.is_empty() {
        return None;
    }

    let mut metas: Vec<syn1::Meta> = nested.into_iter()
        .filter_map(|n| match n {
            syn1::NestedMeta::Meta(m) => Some(m),
            _ => None,
        })
        .collect();
    
    if metas.len() == 1 {
        parse_single_meta(metas.pop()?)
    } else {
        let args: Vec<CfgExpr> = metas.into_iter().filter_map(parse_single_meta).collect();
        Some(CfgExpr::Any(args))
    }
}

#[cfg(feature = "cfg_eval")]
fn parse_single_meta(meta: syn1::Meta) -> Option<CfgExpr> {
    match meta {
        syn1::Meta::Path(path) => Some(CfgExpr::Atom(CfgAtom::Feature(
            path.get_ident()?.to_string(),
        ))),
        syn1::Meta::NameValue(nv) => {
            let key = nv.path.get_ident()?.to_string();
            let value = match nv.lit {
                syn1::Lit::Str(s) => s.value(),
                _ => return None,
            };
            match key.as_str() {
                "feature" => Some(CfgExpr::Atom(CfgAtom::Feature(value))),
                "target_os" => Some(CfgExpr::Atom(CfgAtom::TargetOs(value))),
                _ => None,
            }
        }
        syn1::Meta::List(list) => {
            let ident = list.path.get_ident()?.to_string();
            let args: Vec<CfgExpr> = list.nested
                .into_iter()
                .filter_map(|n| match n {
                    syn1::NestedMeta::Meta(m) => parse_single_meta(m),
                    _ => None,
                })
                .collect();
            match ident.as_str() {
                "all" => Some(CfgExpr::All(args)),
                "any" => Some(CfgExpr::Any(args)),
                "not" => args.into_iter().next().map(|e| CfgExpr::Not(Box::new(e))),
                _ => None,
            }
        }
    }
}

/// Extracts the raw string content from `#[cfg(...)]` attributes.
///
/// # Arguments
/// * `attrs` - A slice of `syn1::Attribute` to parse.
///
/// # Returns
/// A `Vec<String>` containing the trimmed string representation of the tokens
/// inside each valid `#[cfg(...)]` attribute. Returns an empty Vec if none are found.
pub(crate) fn extract_cfg_strings(attrs: &[syn1::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter(|attr| attr.path.is_ident("cfg"))
        .filter_map(|attr| {
            // Extract the tokens inside the cfg(...)
            let meta = attr.parse_meta().ok()?;
            match meta {
                syn1::Meta::List(list) => {
                    // In syn 1.x, we need to reconstruct from nested items
                    let cfg_content: Vec<String> = list.nested.iter()
                        .map(|n| n.to_token_stream().to_string())
                        .collect();
                    let cfg_content = cfg_content.join(", ");
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
                        attr.path.to_token_stream()
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
pub(crate) fn extract_file_level_docstring(attrs: &[syn1::Attribute]) -> Option<String> {
    // Implementation is identical to extract_docstring for now
    let doc_lines: Vec<String> = attrs
        .iter()
        .filter(|attr| attr.path.is_ident("doc"))
        .filter_map(|attr| {
            // In syn 1.x, doc comments are stored as attributes with tokens like `= "doc content"`
            let tokens = attr.tokens.to_string();
            tokens.strip_prefix('=').and_then(|s| {
                let s = s.trim();
                s.strip_prefix('"').and_then(|s| s.strip_suffix('"')).map(|s| s.to_string())
            })
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
pub(crate) fn extract_file_level_attributes(attrs: &[syn1::Attribute]) -> Vec<Attribute> {
    // Implementation is identical to extract_attributes for now
    attrs
        .iter()
        .filter(|attr| !attr.path.is_ident("doc") && !attr.path.is_ident("cfg")) // Skip doc AND cfg comments
        .map(parse_attribute) // Uses the same helper
        .collect()
}
