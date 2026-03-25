//! # Mock Serde Derive Internals
//!
//! This crate provides AST representation used by the derive macros.
//! 
//! **NOTE**: This crate uses a NON-STANDARD layout! The lib.rs is at the
//! crate root, NOT in src/. This is to test the parser's ability to handle
/// non-standard crate layouts.

extern crate proc_macro2;
extern crate quote;
extern crate syn;

// The internal module is conditionally compiled based on build configuration
// In the real serde, this uses cfg_attr to switch between different paths
#[cfg_attr(
    mock_serde_build_from_git,
    path = "../mock_serde_derive/src/internals/mod.rs"
)]
#[cfg_attr(
    not(mock_serde_build_from_git),
    path = "src/mod.rs"
)]
mod internals;

pub use internals::*;

/// Public helper function for AST parsing
/// Returns a parsed expression or type (simplified from original syn::File)
pub fn parse_ast(input: &str) -> Result<syn::Expr, syn::Error> {
    syn::parse_str(input)
}

/// Helper struct for tracking AST metadata
#[derive(Clone)]
pub struct AstMetadata {
    /// Source file path (if available)
    pub source_path: Option<String>,
    /// Crate name
    pub crate_name: String,
    /// Module path
    pub module_path: Vec<String>,
}

impl AstMetadata {
    /// Create new AST metadata
    pub fn new(crate_name: impl Into<String>) -> Self {
        AstMetadata {
            source_path: None,
            crate_name: crate_name.into(),
            module_path: Vec::new(),
        }
    }

    /// Set the source path
    pub fn with_source_path(mut self, path: impl Into<String>) -> Self {
        self.source_path = Some(path.into());
        self
    }

    /// Push a module to the path
    pub fn push_module(&mut self, module: impl Into<String>) {
        self.module_path.push(module.into());
    }

    /// Get the full module path as a string
    pub fn module_path_string(&self) -> String {
        self.module_path.join("::")
    }
}

impl core::fmt::Debug for AstMetadata {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("AstMetadata")
            .field("source_path", &self.source_path)
            .field("crate_name", &self.crate_name)
            .field("module_path", &self.module_path)
            .finish()
    }
}

/// Error type for AST operations
#[derive(Debug)]
pub struct AstError {
    message: String,
}

impl AstError {
    /// Create a new AST error
    pub fn new(message: impl Into<String>) -> Self {
        AstError {
            message: message.into(),
        }
    }

    /// Get the error message
    pub fn message(&self) -> &str {
        &self.message
    }
}

impl core::fmt::Display for AstError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "AST error: {}", self.message)
    }
}

impl core::error::Error for AstError {}

/// Trait for items that can provide span information
pub trait Spanned {
    /// Get the span for this item
    fn span(&self) -> proc_macro2::Span;
}

/// Wrapper around a syn item that provides span info
pub struct SpannedItem<T> {
    item: T,
    span: proc_macro2::Span,
}

impl<T> SpannedItem<T> {
    /// Create a new spanned item
    pub fn new(item: T, span: proc_macro2::Span) -> Self {
        SpannedItem { item, span }
    }

    /// Get the inner item
    pub fn into_inner(self) -> T {
        self.item
    }

    /// Get a reference to the item
    pub fn inner(&self) -> &T {
        &self.item
    }
}

impl<T> Spanned for SpannedItem<T> {
    fn span(&self) -> proc_macro2::Span {
        self.span
    }
}
