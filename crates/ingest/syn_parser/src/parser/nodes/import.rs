use ploke_core::NodeId;

use super::*;

/// Represents all import/export semantics in the code graph, including:
/// - Regular `use` statements
/// - `pub use` re-exports
/// - Extern crate declarations
/// - Future import-like constructs
///
/// # Key Features
/// - Tracks both source path and visible identifiers
/// - Handles rename semantics (`as` clauses) and glob imports
/// - Preserves span information for error reporting
/// - Distinguishes between import types via `ImportKind`
///
/// # Example: Basic Import
/// ```rust
/// use std::collections::HashMap;
/// ```
/// Produces:
/// ```ignore
/// ImportNode {
///     path: vec!["std", "collections", "HashMap"],
///     visible_name: "HashMap",
///     original_name: None,
///     is_glob: false,
///     kind: ImportKind::UseStatement(_),
///     ...
/// }
/// ```
///
/// # Example: Renamed Import
/// ```rust
/// use std::collections::{HashMap as Map, BTreeSet};
/// ```
/// Produces two nodes:
/// ```ignore
/// [
///     ImportNode {
///         path: vec!["std", "collections", "HashMap"],
///         visible_name: "Map",
///         original_name: Some("HashMap"),
///         is_glob: false,
///         kind: ImportKind::UseStatement(_),
///         ...
///     },
///     ImportNode {
///         path: vec!["std", "collections", "BTreeSet"],
///         visible_name: "BTreeSet",
///         original_name: None,
///         is_glob: false,
///         kind: ImportKind::UseStatement(_),
///         ...
///     }
/// ]
/// ```
///
/// # Example: Re-export
/// ```ignore
/// pub use crate::internal::api as public_api;
/// ```
/// Produces:
/// ```ignore
/// ImportNode {
///     path: vec!["crate", "internal", "api"],
///     visible_name: "public_api",
///     original_name: Some("api"),
///     is_glob: false,
///     kind: ImportKind::UseStatement(_),
///     ...
/// }
/// ```
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ImportNode {
    /// Unique identifier for this import in the graph
    pub id: NodeId,

    /// Source code span (byte offsets) of the import statement
    pub span: (usize, usize),

    /// Full path segments in original order (e.g. ["std", "collections", "HashMap"])
    pub path: Vec<String>,

    /// Type of import (regular use, extern crate, etc.)
    pub kind: ImportKind,

    /// Name as brought into scope (accounts for renames via `as`)
    pub visible_name: String,

    /// Original identifier name when renamed (None for direct imports)
    pub original_name: Option<String>,

    /// Whether this is a glob import (`use some::path::*`)
    pub is_glob: bool,

    /// Whether this is a 'self' import, e.g. `std::fs::{self}`
    pub is_self_import: bool,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

impl ImportNode {
    pub fn path(&self) -> &[String] {
        &self.path
    }

    /// Checks if this import node represents a public re-export (`pub use`).
    ///
    /// Returns `true` if the import kind is `UseStatement` and its visibility is `Public`.
    /// Returns `false` otherwise (including for `extern crate` or non-public `use` statements).
    pub fn is_reexport(&self) -> bool {
        matches!(
            self.kind,
            ImportKind::UseStatement(VisibilityKind::Public)
        )
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ImportKind {
    ImportNode, // Placeholder or potentially for future import types
    ExternCrate, // Represents an `extern crate foo;` or `extern crate foo as Bar;` statement
    UseStatement(VisibilityKind), // Represents a `use` statement, capturing its visibility
}
