use std::collections::{HashMap, HashSet};

use crate::parser::nodes::{MacroKind, MacroNode};

#[derive(Default)]
pub(crate) struct MacroExpansionContext {
    items_by_macro: HashMap<String, Vec<syn::Item>>,
}

impl MacroExpansionContext {
    pub(crate) fn from_macro_nodes(macros: &[MacroNode]) -> Self {
        let mut items_by_macro = HashMap::new();
        let mut duplicate_names = HashSet::new();

        for macro_node in macros {
            if !matches!(macro_node.kind, MacroKind::DeclarativeMacro) {
                continue;
            }
            let Some(body) = &macro_node.body else {
                continue;
            };
            let Ok(body_tokens) = body.parse() else {
                continue;
            };
            let Ok(Some(items)) = ploke_mbe::parse_no_arg_macro_rule_items(body_tokens) else {
                continue;
            };
            if !is_supported_local_item_expansion(&items) {
                continue;
            }
            if items_by_macro
                .insert(macro_node.name.clone(), items)
                .is_some()
            {
                duplicate_names.insert(macro_node.name.clone());
            }
        }

        for name in duplicate_names {
            items_by_macro.remove(&name);
        }

        Self { items_by_macro }
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
}

fn is_supported_local_item_expansion(items: &[syn::Item]) -> bool {
    matches!(items, [syn::Item::Fn(_) | syn::Item::Const(_)])
}
