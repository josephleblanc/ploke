use proc_macro2::TokenStream;
use quote::ToTokens;

use crate::error::MbeError;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StructuralItem {
    Module { name: String },
    Use { path: String },
    ExternCrate { name: String },
    Other { kind: String },
}

pub fn parse_expanded_items(tokens: TokenStream) -> Result<Vec<syn::Item>, MbeError> {
    syn::parse2::<syn::File>(tokens)
        .map(|file| file.items)
        .map_err(|err| MbeError::StructuralParse {
            message: err.to_string(),
        })
}

pub fn collect_structural_items(items: &[syn::Item]) -> Vec<StructuralItem> {
    items.iter().map(StructuralItem::from).collect()
}

impl From<&syn::Item> for StructuralItem {
    fn from(item: &syn::Item) -> Self {
        match item {
            syn::Item::Mod(item_mod) => StructuralItem::Module {
                name: item_mod.ident.to_string(),
            },
            syn::Item::Use(item_use) => StructuralItem::Use {
                path: item_use.tree.to_token_stream().to_string(),
            },
            syn::Item::ExternCrate(item_extern) => StructuralItem::ExternCrate {
                name: item_extern.ident.to_string(),
            },
            other => StructuralItem::Other {
                kind: item_kind_name(other).into(),
            },
        }
    }
}

fn item_kind_name(item: &syn::Item) -> &'static str {
    match item {
        syn::Item::Const(_) => "const",
        syn::Item::Enum(_) => "enum",
        syn::Item::Fn(_) => "fn",
        syn::Item::Impl(_) => "impl",
        syn::Item::Macro(_) => "macro",
        syn::Item::Static(_) => "static",
        syn::Item::Struct(_) => "struct",
        syn::Item::Trait(_) => "trait",
        syn::Item::Type(_) => "type",
        syn::Item::Union(_) => "union",
        _ => "other",
    }
}
