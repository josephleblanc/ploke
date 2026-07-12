use std::collections::{HashMap, HashSet};

use proc_macro2::{Spacing, TokenStream, TokenTree};
use syn::ext::IdentExt;
use syn::parse::{Parse, ParseStream};
use syn::{Ident, Token, braced};

use crate::parser::nodes::{MacroKind, MacroNode};

#[derive(Default)]
pub(crate) struct MacroExpansionContext {
    items_by_macro: HashMap<String, Vec<syn::Item>>,
    exprs_by_macro: HashMap<String, syn::Expr>,
    returned_by_macro: HashSet<String>,
    transparent_blocks: HashSet<String>,
}

pub(super) struct GeneratedCall {
    pub path: Vec<String>,
    pub path_arg_count: usize,
    pub generic_arg_count: usize,
    pub dynamic_arg_count: usize,
    pub unsafe_block: bool,
}

struct TransparentStmtBlock {
    block: syn::Block,
}

impl Parse for TransparentStmtBlock {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let _self_ident = Ident::parse_any(input)?;
        input.parse::<Token![,]>()?;
        input.parse::<Token![mut]>()?;
        let _inner_ident = Ident::parse_any(input)?;
        input.parse::<Token![=>]>()?;

        let content;
        let brace_token = braced!(content in input);
        let stmts = syn::Block::parse_within(&content)?;
        if !input.is_empty() {
            return Err(input.error("unsupported transparent macro trailing tokens"));
        }

        Ok(Self {
            block: syn::Block { brace_token, stmts },
        })
    }
}

impl MacroExpansionContext {
    pub(crate) fn from_macro_nodes(macros: &[MacroNode]) -> Self {
        let mut items_by_macro = HashMap::new();
        let mut exprs_by_macro = HashMap::new();
        let mut returned_by_macro = HashSet::new();
        let mut transparent_blocks = HashSet::new();
        let mut duplicate_names = HashSet::new();

        for macro_node in macros {
            if !matches!(macro_node.kind, MacroKind::DeclarativeMacro) {
                continue;
            }
            let Some(body) = &macro_node.body else {
                continue;
            };
            let Ok(body_tokens) = body.parse::<TokenStream>() else {
                continue;
            };
            if supports_ifunc(&macro_node.name, &body_tokens) {
                returned_by_macro.insert(macro_node.name.clone());
            }
            if supports_transparent_stmt_block(&macro_node.name, &body_tokens) {
                transparent_blocks.insert(macro_node.name.clone());
            }
            if let Ok(Some(items)) = ploke_mbe::parse_no_arg_macro_rule_items(body_tokens.clone())
                && is_supported_local_item_expansion(&items)
            {
                if items_by_macro
                    .insert(macro_node.name.clone(), items)
                    .is_some()
                {
                    duplicate_names.insert(macro_node.name.clone());
                }
                continue;
            }

            let Ok(Some(stmts)) = ploke_mbe::parse_no_arg_macro_rule_stmts(body_tokens) else {
                continue;
            };
            let Some(expr) = supported_path_expr_expansion(&stmts) else {
                continue;
            };
            if exprs_by_macro
                .insert(macro_node.name.clone(), expr.clone())
                .is_some()
            {
                duplicate_names.insert(macro_node.name.clone());
            }
        }

        for name in duplicate_names {
            items_by_macro.remove(&name);
            exprs_by_macro.remove(&name);
        }

        Self {
            items_by_macro,
            exprs_by_macro,
            returned_by_macro,
            transparent_blocks,
        }
    }

    pub(super) fn single_local_item_for(&self, mac: &syn::Macro) -> Option<&syn::Item> {
        if !mac.tokens.is_empty() {
            return None;
        }
        let mut segments = mac.path.segments.iter();
        let name = segments.next()?.ident.to_string();
        if segments.next().is_some() {
            return None;
        }
        let [item] = self.items_by_macro.get(&name)?.as_slice() else {
            return None;
        };
        Some(item)
    }

    pub(super) fn single_path_expr_for(&self, mac: &syn::Macro) -> Option<&syn::Expr> {
        if !mac.tokens.is_empty() {
            return None;
        }
        let mut segments = mac.path.segments.iter();
        let name = segments.next()?.ident.to_string();
        if segments.next().is_some() {
            return None;
        }
        self.exprs_by_macro.get(&name)
    }

    pub(super) fn generated_call_for(&self, mac: &syn::Macro) -> Option<GeneratedCall> {
        let mut segments = mac.path.segments.iter();
        let name = segments.next()?.ident.to_string();
        if segments.next().is_some() || !self.returned_by_macro.contains(&name) {
            return None;
        }
        let dynamic_arg_count = unsafe_ifunc_arg_count(mac.tokens.clone())?;
        Some(GeneratedCall {
            path: ["core", "mem", "transmute"]
                .into_iter()
                .map(str::to_string)
                .collect(),
            path_arg_count: 1,
            generic_arg_count: 2,
            dynamic_arg_count,
            unsafe_block: true,
        })
    }

    pub(super) fn transparent_stmt_block_for(&self, mac: &syn::Macro) -> Option<syn::Block> {
        let mut segments = mac.path.segments.iter();
        let name = segments.next()?.ident.to_string();
        if segments.next().is_some() || !self.transparent_blocks.contains(&name) {
            return None;
        }

        syn::parse2::<TransparentStmtBlock>(mac.tokens.clone())
            .ok()
            .map(|parsed| parsed.block)
    }
}

fn is_supported_local_item_expansion(items: &[syn::Item]) -> bool {
    matches!(
        items,
        [syn::Item::Fn(_) | syn::Item::Const(_) | syn::Item::Static(_)]
    )
}

fn supported_path_expr_expansion(stmts: &[syn::Stmt]) -> Option<syn::Expr> {
    let [syn::Stmt::Expr(expr, _)] = stmts else {
        return None;
    };
    let syn::Expr::Call(call) = expr else {
        return None;
    };
    if matches!(call.func.as_ref(), syn::Expr::Path(_)) {
        Some(expr.clone())
    } else {
        None
    }
}

fn supports_ifunc(name: &str, body: &TokenStream) -> bool {
    if name != "unsafe_ifunc" {
        return false;
    }
    let body = body.to_string();
    body.contains("core :: mem :: transmute")
        && body.contains("Fn")
        && body.contains("RealFn")
        && body.contains("FN . load")
}

fn supports_transparent_stmt_block(name: &str, body: &TokenStream) -> bool {
    if name != "tap_inner" {
        return false;
    }

    let body = body.to_string();
    body.contains("into_inner")
        && body.contains("Arc :: new")
        && body.contains("$ stmt")
        && body.contains("$ inner")
}

fn unsafe_ifunc_arg_count(tokens: TokenStream) -> Option<usize> {
    let args = top_level_args(tokens);
    if args.len() < 7 {
        return None;
    }

    // Matcher shape:
    // ($memchrty, $memchrfind, $fnty, $retty, $hay_start, $hay_end, $($needle),+)
    // Generated call shape:
    // core::mem::transmute::<Fn, RealFn>(fun)($($needle),+, $hay_start, $hay_end)
    Some(args.len() - 4)
}

fn top_level_args(tokens: TokenStream) -> Vec<TokenStream> {
    let mut args = Vec::new();
    let mut current = TokenStream::new();

    for token in tokens {
        if let TokenTree::Punct(punct) = &token
            && punct.as_char() == ','
            && punct.spacing() == Spacing::Alone
        {
            args.push(current);
            current = TokenStream::new();
            continue;
        }
        current.extend(std::iter::once(token));
    }

    if !current.is_empty() {
        args.push(current);
    }
    args
}
