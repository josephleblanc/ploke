//! # Mock Serde Derive
//!
//! This crate provides derive macros for `Serialize` and `Deserialize` traits.
//!
//! ```ignore
//! use mock_serde_derive::{Deserialize, Serialize};
//!
//! #[derive(Serialize, Deserialize)]
//! struct MyStruct {
//!     field: i32,
//! }
//! ```

extern crate proc_macro;
extern crate proc_macro2;
extern crate quote;
extern crate syn;

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, Ident};

mod internals;

/// Derive macro for the `Serialize` trait.
///
/// This is a simplified mock implementation that generates basic
/// serialization code for structs.
#[proc_macro_derive(Serialize, attributes(mock_serde))]
pub fn derive_serialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Generate a simple Serialize implementation
    let expanded = quote! {
        impl mock_serde::Serialize for #name {
            fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
            where
                S: mock_serde::Serializer,
            {
                // Mock implementation: serialize as unit
                serializer.serialize_unit()
            }
        }
    };

    TokenStream::from(expanded)
}

/// Derive macro for the `Deserialize` trait.
///
/// This is a simplified mock implementation that generates basic
/// deserialization code for structs.
#[proc_macro_derive(Deserialize, attributes(mock_serde))]
pub fn derive_deserialize(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    // Generate a simple Deserialize implementation
    let expanded = quote! {
        impl<'de> mock_serde::Deserialize<'de> for #name {
            fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
            where
                D: mock_serde::Deserializer<'de>,
            {
                // Mock implementation: create default instance
                struct __Visitor;

                impl<'de> mock_serde::de::Visitor<'de> for __Visitor {
                    type Value = #name;

                    fn expecting(&self, formatter: &mut core::fmt::Formatter) -> core::fmt::Result {
                        formatter.write_str("mock struct")
                    }

                    fn visit_unit<E>(self) -> Result<Self::Value, E>
                    where
                        E: mock_serde::de::Error,
                    {
                        // Create a default instance
                        Ok(unsafe { core::mem::zeroed() })
                    }
                }

                deserializer.deserialize_unit(__Visitor)
            }
        }
    };

    TokenStream::from(expanded)
}

/// Helper function to create a private identifier
#[allow(dead_code)]
fn private_ident() -> Ident {
    Ident::new(
        concat!("__private", env!("CARGO_PKG_VERSION_PATCH")),
        Span::call_site(),
    )
}
