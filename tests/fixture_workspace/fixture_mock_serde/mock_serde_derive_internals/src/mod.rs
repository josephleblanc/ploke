//! Internal AST module
//!
//! This module provides the actual AST representation used by derive macros.
//! In the real serde, this would be shared between serde_derive and
/// serde_derive_internals crates.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, DeriveInput, Field, Ident, Type, Visibility};

/// Representation of a container (struct or enum) being processed
#[derive(Clone)]
pub struct Container {
    /// The name of the container
    pub ident: Ident,
    /// The visibility of the container
    pub vis: Visibility,
    /// The attributes on the container
    pub attrs: Vec<Attribute>,
}

impl Container {
    /// Create a container from a DeriveInput
    pub fn from_derive_input(input: &DeriveInput) -> Self {
        Container {
            ident: input.ident.clone(),
            vis: syn::Visibility::Inherited, // Default visibility
            attrs: input.attrs.clone(),
        }
    }

    /// Get the identifier as a string
    pub fn name(&self) -> String {
        self.ident.to_string()
    }
}

impl core::fmt::Debug for Container {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("Container")
            .field("ident", &self.ident)
            .finish()
    }
}

/// Representation of a struct field
#[derive(Clone)]
pub struct FieldInfo {
    /// The field name (None for tuple fields)
    pub ident: Option<Ident>,
    /// The field type
    pub ty: Type,
    /// The field visibility
    pub vis: Visibility,
    /// The field attributes
    pub attrs: Vec<Attribute>,
}

impl FieldInfo {
    /// Create FieldInfo from a syn Field
    pub fn from_field(field: &Field) -> Self {
        FieldInfo {
            ident: field.ident.clone(),
            ty: field.ty.clone(),
            vis: field.vis.clone(),
            attrs: field.attrs.clone(),
        }
    }

    /// Get the name of this field (index for tuple fields)
    pub fn name(&self) -> String {
        self.ident
            .as_ref()
            .map(|i| i.to_string())
            .unwrap_or_else(|| "<tuple_field>".to_string())
    }

    /// Check if this field has a specific attribute
    pub fn has_attr(&self, name: &str) -> bool {
        self.attrs.iter().any(|attr| {
            attr.path()
                .get_ident()
                .map(|i| i == name)
                .unwrap_or(false)
        })
    }
}

impl core::fmt::Debug for FieldInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("FieldInfo")
            .field("ident", &self.ident)
            .finish()
    }
}

/// Representation of an enum variant
#[derive(Clone)]
pub struct VariantInfo {
    /// The variant identifier
    pub ident: Ident,
    /// The variant attributes
    pub attrs: Vec<Attribute>,
    /// The discriminant expression (if any)
    pub discriminant: Option<syn::Expr>,
}

impl VariantInfo {
    /// Get the variant name
    pub fn name(&self) -> String {
        self.ident.to_string()
    }

    /// Check if this variant is the default variant
    pub fn is_default(&self) -> bool {
        self.has_attr("default")
    }

    /// Check if this variant has a specific attribute
    pub fn has_attr(&self, name: &str) -> bool {
        self.attrs.iter().any(|attr| {
            attr.path()
                .get_ident()
                .map(|i| i == name)
                .unwrap_or(false)
        })
    }
}

impl core::fmt::Debug for VariantInfo {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("VariantInfo")
            .field("ident", &self.ident)
            .finish()
    }
}

/// Symbol type for attribute parsing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Symbol(&'static str);

impl Symbol {
    /// Create a new symbol
    pub const fn new(s: &'static str) -> Self {
        Symbol(s)
    }

    /// Get the string value
    pub fn as_str(&self) -> &'static str {
        self.0
    }
}

/// Common symbols used in attribute parsing
pub mod symbol {
    use super::Symbol;

    pub const SERDE: Symbol = Symbol::new("serde");
    pub const DEFAULT: Symbol = Symbol::new("default");
    pub const RENAME: Symbol = Symbol::new("rename");
    pub const RENAME_ALL: Symbol = Symbol::new("rename_all");
    pub const SKIP: Symbol = Symbol::new("skip");
    pub const SKIP_SERIALIZING: Symbol = Symbol::new("skip_serializing");
    pub const SKIP_DESERIALIZING: Symbol = Symbol::new("skip_deserializing");
    pub const WITH: Symbol = Symbol::new("with");
}

/// Helper function to generate a parse error
pub fn error(_span: proc_macro2::Span, message: &str) -> TokenStream {
    quote! {
        compile_error!(#message);
    }
}

/// Rename rule for converting between naming conventions
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenameRule {
    /// Keep the original name
    None,
    /// Convert to lowercase
    LowerCase,
    /// Convert to UPPERCASE
    UpperCase,
    /// Convert to PascalCase
    PascalCase,
    /// Convert to camelCase
    CamelCase,
    /// Convert to snake_case
    SnakeCase,
    /// Convert to SCREAMING_SNAKE_CASE
    ScreamingSnakeCase,
    /// Convert to kebab-case
    KebabCase,
    /// Convert to SCREAMING-KEBAB-CASE
    ScreamingKebabCase,
}

impl RenameRule {
    /// Apply the rename rule to an identifier
    pub fn apply_to_field(&self, field: &str) -> String {
        match self {
            RenameRule::None => field.to_owned(),
            RenameRule::LowerCase => field.to_lowercase(),
            RenameRule::UpperCase => field.to_uppercase(),
            RenameRule::PascalCase => Self::to_pascal_case(field),
            RenameRule::CamelCase => Self::to_camel_case(field),
            RenameRule::SnakeCase => Self::to_snake_case(field),
            RenameRule::ScreamingSnakeCase => Self::to_snake_case(field).to_uppercase(),
            RenameRule::KebabCase => Self::to_snake_case(field).replace('_', "-"),
            RenameRule::ScreamingKebabCase => {
                Self::to_snake_case(field).replace('_', "-").to_uppercase()
            }
        }
    }

    /// Convert to PascalCase
    fn to_pascal_case(s: &str) -> String {
        // Simplified implementation
        s.split('_')
            .map(|w| {
                let mut chars = w.chars();
                match chars.next() {
                    Some(c) => c.to_uppercase().chain(chars).collect(),
                    None => String::new(),
                }
            })
            .collect()
    }

    /// Convert to camelCase
    fn to_camel_case(s: &str) -> String {
        let pascal = Self::to_pascal_case(s);
        let mut chars = pascal.chars();
        match chars.next() {
            Some(c) => c.to_lowercase().chain(chars).collect(),
            None => String::new(),
        }
    }

    /// Convert to snake_case
    fn to_snake_case(s: &str) -> String {
        // Simplified - just lowercase for now
        s.to_lowercase()
    }
}

impl Default for RenameRule {
    fn default() -> Self {
        RenameRule::None
    }
}
