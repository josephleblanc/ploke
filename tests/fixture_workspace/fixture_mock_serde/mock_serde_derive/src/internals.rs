//! Internal implementation details for the derive macros
//!
//! This module provides helper functions and types used by the
/// Serialize and Deserialize derive implementations.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Fields, Ident, Type, Visibility};

/// Information about a struct or enum being derived
pub struct Container<'a> {
    /// The input AST
    pub input: &'a DeriveInput,
    /// The name of the container
    pub ident: &'a syn::Ident,
    /// The generics of the container
    pub generics: &'a syn::Generics,
    /// The data type (struct or enum)
    pub data: &'a Data,
}

impl<'a> Container<'a> {
    /// Parse the derive input into a container
    pub fn from_input(input: &'a DeriveInput) -> Self {
        Container {
            input,
            ident: &input.ident,
            generics: &input.generics,
            data: &input.data,
        }
    }

    /// Get the name of the container as a string
    pub fn name(&self) -> String {
        self.ident.to_string()
    }

    /// Check if this is a struct
    pub fn is_struct(&self) -> bool {
        matches!(self.data, Data::Struct(_))
    }

    /// Check if this is an enum
    pub fn is_enum(&self) -> bool {
        matches!(self.data, Data::Enum(_))
    }
}

/// Information about a field in a struct or enum variant
pub struct FieldInfo<'a> {
    /// The field identifier (may be None for tuple structs)
    pub ident: Option<&'a syn::Ident>,
    /// The field type
    pub ty: &'a syn::Type,
    /// The field attributes
    pub attrs: &'a [Attribute],
}

impl<'a> FieldInfo<'a> {
    /// Extract field information from syn::Field
    pub fn from_field(field: &'a syn::Field) -> Self {
        FieldInfo {
            ident: field.ident.as_ref(),
            ty: &field.ty,
            attrs: &field.attrs,
        }
    }
}

/// Generate serialization code for fields
pub fn serialize_fields(fields: &Fields) -> TokenStream {
    match fields {
        Fields::Named(_) => {
            quote! {
                // Serialize named fields as a map
                let mut map = serializer.serialize_map(None)?;
                // Field serialization would go here
                map.end()
            }
        }
        Fields::Unnamed(_) => {
            quote! {
                // Serialize unnamed fields as a sequence
                let mut seq = serializer.serialize_seq(None)?;
                // Element serialization would go here
                seq.end()
            }
        }
        Fields::Unit => {
            quote! {
                serializer.serialize_unit()
            }
        }
    }
}

/// Generate deserialization code for fields
pub fn deserialize_fields(fields: &Fields) -> TokenStream {
    match fields {
        Fields::Named(_) => {
            quote! {
                deserializer.deserialize_map(__Visitor)
            }
        }
        Fields::Unnamed(_) => {
            quote! {
                deserializer.deserialize_seq(__Visitor)
            }
        }
        Fields::Unit => {
            quote! {
                deserializer.deserialize_unit(__Visitor)
            }
        }
    }
}

/// Check if a type is a primitive that can be serialized directly
pub fn is_primitive_type(ty: &syn::Type) -> bool {
    // Simplified check - in reality this would be more complex
    if let syn::Type::Path(type_path) = ty {
        let path = &type_path.path;
        if let Some(segment) = path.segments.last() {
            let ident = &segment.ident;
            return matches!(
                ident.to_string().as_str(),
                "bool" | "i8" | "i16" | "i32" | "i64" | "i128"
                    | "u8" | "u16" | "u32" | "u64" | "u128"
                    | "f32" | "f64" | "char" | "str" | "String"
            );
        }
    }
    false
}
