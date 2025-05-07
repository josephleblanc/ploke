//! Procedural derive macro (`ExpectedData`) for generating test helper structs
//! (`Expected*Data`) and associated checking/finding methods used in
//! syn_parser tests.

extern crate proc_macro;

use proc_macro::TokenStream;
use proc_macro_error::{abort, proc_macro_error};
use quote::{format_ident, quote};
use syn::{parse_macro_input, spanned::Spanned, Data, DeriveInput, Fields, FieldsNamed, Type};
use heck::ToPascalCase;


// Placeholder for the base trait that generated structs will implement.
// The actual trait definition might live here or be defined in syn_parser::tests::common
// and referenced via a known path in the generated code. For now, just a comment.
//
// trait ExpectedNodeData<N> {
//     fn find_node_by_values<'a>(...) -> ...;
//     fn check_all_fields(...) -> bool;
// }


#[proc_macro_error]
#[proc_macro_derive(ExpectedData)]
pub fn derive_expected_data(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // TODO: Implement the derive logic here:
    // 1. Parse the input struct definition (e.g., ConstNode).
    // 2. Generate the corresponding `Expected*Data` struct definition (e.g., ExpectedConstData).
    // 3. Generate the `impl ExpectedNodeData<NodeType> for Expected*Data` block.
    // 4. Generate the `impl Expected*Data` block containing `is_*_match_debug` methods.

    // Placeholder implementation:
    let struct_name = &input.ident;
    let generated_struct_name = format_ident!("Expected{}", struct_name);

    let expanded = quote! {
        // Placeholder struct - actual fields will be generated based on #struct_name
        #[derive(Debug, Clone, PartialEq)]
        pub struct #generated_struct_name {
           // Generated fields will go here...
           // Example: pub name: &'static str,
        }

        // Placeholder impls - actual methods will be generated
        // impl ploke_derive_test_helpers::ExpectedNodeData<#struct_name> for #generated_struct_name {
        //    // ... find_node_by_values, check_all_fields ...
        // }
        //
        // impl #generated_struct_name {
        //    // ... is_*_match_debug methods ...
        // }
    };

    // For now, return an empty token stream to avoid errors until implemented
    // expanded.into()
     TokenStream::new() // Return empty stream
}
