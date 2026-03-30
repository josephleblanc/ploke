mod error;
mod ir;
mod parse;
mod structural;

pub use error::MbeError;
pub use ir::{
    DeclarativeMacro, MacroInvocation, MetaTemplate, MetaVarKind, Op, RepeatKind, Rule, Separator,
};
pub use parse::{parse_invocation, parse_macro_rules_item, parse_macro_rules_tokens};
pub use structural::{StructuralItem, collect_structural_items, parse_expanded_items};

#[cfg(test)]
mod tests {
    use quote::quote;

    use crate::{
        MbeError, Op, RepeatKind, StructuralItem, collect_structural_items, parse_expanded_items,
        parse_macro_rules_item,
    };

    #[test]
    fn parses_simple_macro_rules_definition() {
        let item: syn::ItemMacro = syn::parse2(quote! {
            macro_rules! make_struct {
                ($name:ident) => { struct $name; };
            }
        })
        .expect("macro_rules item should parse");

        let parsed = parse_macro_rules_item(&item).expect("macro should parse");

        assert_eq!(parsed.name, "make_struct");
        assert_eq!(parsed.rules.len(), 1);

        let matcher = &parsed.rules[0].matcher.ops;
        assert!(matches!(matcher.as_slice(), [Op::Subtree { .. }]));
    }

    #[test]
    fn parses_repetition_with_separator() {
        let item: syn::ItemMacro = syn::parse2(quote! {
            macro_rules! listify {
                ($($name:ident),*) => { [$($name),*] };
            }
        })
        .expect("macro_rules item should parse");

        let parsed = parse_macro_rules_item(&item).expect("macro should parse");
        let matcher = &parsed.rules[0].matcher.ops;

        let Op::Subtree { tokens, .. } = &matcher[0] else {
            panic!("expected matcher subtree");
        };

        assert!(tokens.ops.iter().any(|op| matches!(
            op,
            Op::Repeat {
                separator: Some(separator),
                kind: RepeatKind::ZeroOrMore,
                ..
            } if separator.text == ","
        )));
    }

    #[test]
    fn rejects_non_macro_rules_items() {
        let item: syn::ItemMacro = syn::parse2(quote! {
            println!("hello");
        })
        .expect("macro invocation item should parse");

        let err = parse_macro_rules_item(&item).expect_err("non-macro_rules item should fail");
        assert!(matches!(err, MbeError::NotMacroRules { .. }));
    }

    #[test]
    fn extracts_structural_items_from_file_body() {
        let items = parse_expanded_items(quote! {
            mod inner;
            pub use crate::inner::Thing;
            extern crate alloc;
            struct NotStructural;
        })
        .expect("file body should parse");

        let structural = collect_structural_items(&items);
        assert_eq!(
            structural,
            vec![
                StructuralItem::Module {
                    name: "inner".into()
                },
                StructuralItem::Use {
                    path: "crate :: inner :: Thing".into()
                },
                StructuralItem::ExternCrate {
                    name: "alloc".into()
                },
                StructuralItem::Other {
                    kind: "struct".into()
                },
            ]
        );
    }
}
