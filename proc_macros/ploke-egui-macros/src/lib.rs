use proc_macro::TokenStream;
use std::collections::{BTreeMap, BTreeSet};

use quote::{format_ident, quote};
use syn::parse::{Parse, ParseStream};
use syn::punctuated::Punctuated;
use syn::{
    Expr, Ident, ItemFn, LitStr, Stmt, Token, Visibility, braced, parse_macro_input, parse_quote,
};

#[proc_macro_attribute]
pub fn profile_scope(attr: TokenStream, item: TokenStream) -> TokenStream {
    let scope_name = parse_macro_input!(attr as Expr);
    let mut item = parse_macro_input!(item as ItemFn);
    let scope_stmt: Stmt = parse_quote! {
        #[cfg(all(not(target_arch = "wasm32"), feature = "native-benchmark"))]
        let _profile_scope = tracing::trace_span!(#scope_name).entered();
    };
    item.block.stmts.insert(0, scope_stmt);

    quote! {
        #item
    }
    .into()
}

#[proc_macro]
pub fn profile_scope_catalog(input: TokenStream) -> TokenStream {
    let catalog = parse_macro_input!(input as ScopeCatalog);
    if let Some(error) = validate_catalog(&catalog) {
        return error.to_compile_error().into();
    }

    let vis = &catalog.vis;
    let mod_ident = &catalog.mod_ident;
    let const_defs = catalog.entries.iter().map(|entry| {
        let ident = &entry.ident;
        let name = &entry.name;
        quote! {
            #vis const #ident: &str = #name;
        }
    });
    let index_defs = catalog.entries.iter().enumerate().map(|(index, entry)| {
        let index_ident = entry.index_ident();
        quote! {
            #vis const #index_ident: usize = #index;
        }
    });
    let all_names = catalog.entries.iter().map(|entry| &entry.ident);
    let match_arms = catalog.entries.iter().map(|entry| {
        let name = &entry.name;
        let index_ident = entry.index_ident();
        quote! {
            #name => Some(#index_ident),
        }
    });

    quote! {
        #vis mod #mod_ident {
            #(#const_defs)*
            #(#index_defs)*

            #vis const ALL: &[&str] = &[#(#all_names),*];

            #vis fn index(name: &str) -> Option<usize> {
                match name {
                    #(#match_arms)*
                    _ => None,
                }
            }
        }
    }
    .into()
}

struct ScopeCatalog {
    vis: Visibility,
    mod_ident: Ident,
    entries: Vec<ScopeEntry>,
}

impl Parse for ScopeCatalog {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let vis = input.parse::<Visibility>()?;
        input.parse::<Token![mod]>()?;
        let mod_ident = input.parse::<Ident>()?;
        let content;
        braced!(content in input);
        let entries = Punctuated::<ScopeEntry, Token![,]>::parse_terminated(&content)?
            .into_iter()
            .collect();

        Ok(Self {
            vis,
            mod_ident,
            entries,
        })
    }
}

struct ScopeEntry {
    ident: Ident,
    name: LitStr,
}

impl ScopeEntry {
    fn index_ident(&self) -> Ident {
        format_ident!("{}_INDEX", self.ident)
    }
}

impl Parse for ScopeEntry {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let ident = input.parse::<Ident>()?;
        input.parse::<Token![=]>()?;
        let name = input.parse::<LitStr>()?;

        Ok(Self { ident, name })
    }
}

fn validate_catalog(catalog: &ScopeCatalog) -> Option<syn::Error> {
    let mut error: Option<syn::Error> = None;
    let mut idents = BTreeSet::new();
    let mut names = BTreeMap::new();

    if catalog.entries.is_empty() {
        push_error(
            &mut error,
            syn::Error::new(
                catalog.mod_ident.span(),
                "profile scope catalog cannot be empty",
            ),
        );
    }

    for entry in &catalog.entries {
        let ident = entry.ident.to_string();
        if !idents.insert(ident.clone()) {
            push_error(
                &mut error,
                syn::Error::new_spanned(&entry.ident, format!("duplicate scope constant {ident}")),
            );
        }

        let name = entry.name.value();
        if !is_valid_scope_name(&name) {
            push_error(
                &mut error,
                syn::Error::new_spanned(
                    &entry.name,
                    "scope names must be non-empty ASCII snake_case",
                ),
            );
        }

        if let Some(previous) = names.insert(name.clone(), entry.name.span()) {
            let mut duplicate =
                syn::Error::new_spanned(&entry.name, format!("duplicate scope name {name:?}"));
            duplicate.combine(syn::Error::new(previous, "first declared here"));
            push_error(&mut error, duplicate);
        }
    }

    error
}

fn push_error(target: &mut Option<syn::Error>, new_error: syn::Error) {
    match target {
        Some(existing) => existing.combine(new_error),
        None => *target = Some(new_error),
    }
}

fn is_valid_scope_name(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
}

#[cfg(test)]
mod tests {
    use super::{ScopeCatalog, validate_catalog};
    use syn::parse_quote;

    #[test]
    fn catalog_accepts_snake_case_scope_names() {
        let catalog: ScopeCatalog = parse_quote! {
            pub(crate) mod scope {
                ROOT = "root",
                EGUI_PANEL = "egui_panel",
            }
        };

        assert!(validate_catalog(&catalog).is_none());
    }

    #[test]
    fn catalog_rejects_duplicate_scope_names() {
        let catalog: ScopeCatalog = parse_quote! {
            pub(crate) mod scope {
                ROOT_A = "root",
                ROOT_B = "root",
            }
        };

        assert!(validate_catalog(&catalog).is_some());
    }

    #[test]
    fn catalog_rejects_non_snake_scope_names() {
        let catalog: ScopeCatalog = parse_quote! {
            pub(crate) mod scope {
                BAD = "BadScope",
            }
        };

        assert!(validate_catalog(&catalog).is_some());
    }
}
