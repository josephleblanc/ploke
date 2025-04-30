use ploke_core::NodeId;
use serde::{Deserialize, Serialize};
use syn_parser_macros::GenerateNodeInfo; // Import the derive macro

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Import Node ---

// Removed the macro invocation for ImportNodeInfo

/// Represents all import/export semantics in the code graph, including:
/// - Regular `use` statements
/// - `pub use` re-exports
        source_path: Vec<String>,
        kind: ImportKind,
        visible_name: String,
        original_name: Option<String>,
        is_glob: bool,
        is_self_import: bool,
        cfgs: Vec<String>,
    }
}

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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, GenerateNodeInfo)] // Add derive
pub struct ImportNode {
    /// Unique identifier for this import in the graph
    pub id: ImportNodeId, // Use typed ID

    /// Source code span (byte offsets) of the import statement
    pub span: (usize, usize),

    /// Full path segments in original order (e.g. ["std", "collections", "HashMap"]) of the item
    /// being imported.
    /// e.g. for the import statement `use std::collections::HashMap;`
    // Note that this is NOT the path to the import declaration itself.
    pub source_path: Vec<String>,

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
    pub cfgs: Vec<String>,
}

impl ImportNode {
    /// Returns the typed ID for this import node.
    pub fn import_id(&self) -> ImportNodeId {
        self.id
    }

    /// Creates a new `ImportNode` from `ImportNodeInfo`.
    pub(crate) fn new(info: ImportNodeInfo) -> Self {
        Self {
            id: ImportNodeId(info.id), // Wrap the raw ID here
            span: info.span,
            source_path: info.source_path,
            kind: info.kind,
            visible_name: info.visible_name,
            original_name: info.original_name,
            is_glob: info.is_glob,
            is_self_import: info.is_self_import,
            cfgs: info.cfgs,
        }
    }

    pub fn source_path(&self) -> &[String] {
        &self.source_path
    }

    /// Checks if this import node represents any kind of re-export.
    ///
    /// A re-export makes an imported item visible outside the current module scope,
    /// as if it were defined within that module (respecting the specified visibility).
    /// This is achieved through `use` statements with explicit `pub` visibility, `pub(crate)`
    /// visibility, or `pub(in path)` visibility.
    ///
    /// Returns `true` if the import kind is `UseStatement` and its visibility is
    /// `Public`, `Crate`, or `Restricted`.
    /// Returns `false` for `extern crate` statements and `use` statements with
    /// inherited (private) visibility, as these do not make the item visible externally
    /// via this specific import statement.
    pub fn is_any_reexport(&self) -> bool {
        matches!(
            self.kind,
            ImportKind::UseStatement(
                VisibilityKind::Public | VisibilityKind::Crate | VisibilityKind::Restricted(_)
            )
        )
    }

    /// Checks if this import is a `pub use` statement.
    pub fn is_public_use(&self) -> bool {
        matches!(self.kind, ImportKind::UseStatement(VisibilityKind::Public))
    }

    /// Checks if this import is a `pub(crate) use` statement.
    pub fn is_crate_use(&self) -> bool {
        matches!(self.kind, ImportKind::UseStatement(VisibilityKind::Crate))
    }

    /// Checks if this import is a restricted `pub(in path) use` statement.
    pub fn is_restricted_use(&self) -> bool {
        matches!(
            self.kind,
            ImportKind::UseStatement(VisibilityKind::Restricted(_))
        )
    }

    /// Checks if this import is effectively private to the current scope,
    /// meaning either a `use` statement with inherited visibility or an `extern crate` statement.
    /// This is used by `ModuleTree::add_module` to populate `pending_imports`.
    pub fn is_inherited_use(&self) -> bool {
        matches!(
            self.kind,
            ImportKind::UseStatement(VisibilityKind::Inherited) | ImportKind::ExternCrate
        )
    }

    /// Checks specifically if this import is a `use` statement with inherited (private) visibility.
    pub fn is_inherited_visibility(&self) -> bool {
        matches!(
            self.kind,
            ImportKind::UseStatement(VisibilityKind::Inherited)
        )
    }

    /// Checks specifically if this import is an `extern crate` statement.
    pub fn is_extern_crate(&self) -> bool {
        matches!(self.kind, ImportKind::ExternCrate)
    }

    /// If this import was renamed (`use ... as ...`), returns the path segments
    /// ending with the `visible_name` (the name after `as`).
    ///
    /// For example, for `use std::collections::HashMap as Map;`, this would return
    /// `Some(["std", "collections", "Map"])`.
    ///
    /// Returns `None` if the import was not renamed.
    pub fn as_renamed_path(&self) -> Option<Vec<String>> {
        if self.original_name.is_some() {
            let mut path = self.source_path.clone();
            path.pop(); // Remove the original name segment
            path.push(self.visible_name.clone());
            Some(path)
        } else {
            None
        }
    }

    /// Checks if this import statement uses the `as` keyword to rename the imported item.
    ///
    /// Returns `true` if `original_name` is `Some`, indicating a rename occurred.
    /// Returns `false` otherwise.
    pub fn is_renamed(&self) -> bool {
        self.original_name.is_some()
    }
}

impl GraphNode for ImportNode {
    fn id(&self) -> NodeId {
        self.id.into_inner() // Return base NodeId
    }

    fn visibility(&self) -> VisibilityKind {
        // The visibility of the *use statement itself* determines its effect.
        // `extern crate` is effectively private to the module.
        match &self.kind {
            ImportKind::UseStatement(vis) => vis.clone(),
            ImportKind::ExternCrate => VisibilityKind::Inherited,
            // ImportKind::ImportNode => VisibilityKind::Inherited, // Placeholder default
        }
    }

    fn name(&self) -> &str {
        // The "name" of an import is the identifier it brings into scope.
        &self.visible_name
    }

    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }

    fn as_import(&self) -> Option<&ImportNode> {
        Some(self)
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ImportKind {
    ExternCrate, // Represents an `extern crate foo;` or `extern crate foo as Bar;` statement
    UseStatement(VisibilityKind), // Represents a `use` statement, capturing its visibility
}
