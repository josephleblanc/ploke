#![allow(dead_code)]

// 1. Declarative Macro (macro_rules!) - Exported
#[macro_export]
macro_rules! exported_macro {
    () => { println!("Exported!"); };
    ($e:expr) => { println!("Expr: {}", $e); };
}

// 2. Declarative Macro (macro_rules!) - Local (not exported)
macro_rules! local_macro {
    ($name:ident) => {
        let $name = "local";
    };
}

// Usage of local macro
pub fn use_local_macro() {
    local_macro!(my_var);
    println!("{}", my_var);
}

// 3. Procedural Macro - Derive (Placeholder Definition)
// Actual proc macro code would be in a separate crate with `proc-macro = true`
// For parsing purposes, we just need to recognize the function signature and attributes.
// Assume this function exists in a proc macro crate:
/*
#[proc_macro_derive(MyDerive)]
pub fn my_derive_macro(input: TokenStream) -> TokenStream {
    // ... implementation ...
    input
}
*/
// We can represent its definition signature in this fixture:
#[cfg(feature = "proc_macro")] // Simulate conditional compilation
mod proc_macros {
    // Use dummy TokenStream if proc_macro crate is not available
    #[cfg(not(feature = "proc_macro"))]
    type TokenStream = String;
    #[cfg(feature = "proc_macro")]
    use proc_macro::TokenStream;

    #[proc_macro_derive(MyDerive)]
    pub fn my_derive_macro_signature(input: TokenStream) -> TokenStream {
        // This body won't be parsed deeply, but its presence is noted
        input
    }

    // 4. Procedural Macro - Attribute (Placeholder Definition)
    #[proc_macro_attribute]
    pub fn my_attribute_macro_signature(attr: TokenStream, item: TokenStream) -> TokenStream {
        item
    }

    // 5. Procedural Macro - Function-like (Placeholder Definition)
    #[proc_macro]
    pub fn my_function_like_macro_signature(input: TokenStream) -> TokenStream {
        input
    }
}

// Usage of the derive macro (requires the actual proc macro crate to compile fully)
// #[derive(MyDerive)]
// pub struct StructUsingDerive {
//     field: i32,
// }

// Usage of the attribute macro
// #[my_attribute_macro]
// pub fn function_using_attribute() {}

// Usage of the function-like macro
// my_function_like_macro!();
