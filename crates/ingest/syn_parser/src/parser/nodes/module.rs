use ploke_core::{NodeId, TrackingHash};
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};
use syn_parser_macros::GenerateNodeInfo; // Import the derive macro

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Module Node ---

// Removed the macro invocation for ModuleNodeInfo

/// Represents a module (`mod`) item encountered during parsing.
///
/// This node captures the syntactic representation of a module, which can be
/// a file-based module (`mod foo;` pointing to `foo.rs` or `foo/mod.rs`),
/// an inline module (`mod foo { ... }`), or just a declaration (`mod foo;`)
/// that needs to be resolved later.
///
/// It stores metadata like name, visibility, attributes, documentation, span,
/// contained imports, and its definition kind (`ModuleKind`).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, GenerateNodeInfo)] // Add derive
pub struct ModuleNode {
    /// The type-safe identifier for this specific module node.
    pub id: ModuleNodeId,
    /// The simple name of the module (e.g., "foo" for `mod foo;`).
    pub name: String,
    /// The fully resolved definition path of this module (e.g., `["crate", "foo", "bar"]`).
    /// This path is constructed during parsing and module tree building.
    pub path: Vec<String>,
    /// The visibility of the module declaration (`pub`, `pub(crate)`, etc.).
    pub visibility: VisibilityKind,
    /// Attributes applied directly to the `mod` item (e.g., `#[cfg(...)] mod foo;`).
    /// Does not include inner attributes (`#![...]`) from file-based modules.
    pub attributes: Vec<Attribute>,
    /// Doc comment associated with the `mod` item (e.g., `/// Docs for mod foo;`).
    /// Does not include inner doc comments (`//! ...`) from file-based modules.
    pub docstring: Option<String>,
    /// Import statements (`use ...;`, `extern crate ...;`) found directly within this module.
    /// Populated during the visitor phase.
    pub imports: Vec<ImportNode>,
    /// List of NodeIds that are re-exported (`pub use`) from this module.
    /// Populated during the resolution phase (ModuleTree processing).
    pub exports: Vec<NodeId>, // Keep as NodeId for now, resolution determines specific item types
    /// The byte span (start, end) of the module definition or declaration in the source file.
    pub span: (usize, usize),
    /// A hash representing the content relevant for tracking changes (currently experimental).
    pub tracking_hash: Option<TrackingHash>,
    /// Specifies whether this node represents a file-based module, inline module, or declaration.
    pub module_def: ModuleKind,
    /// Conditional compilation flags (`#[cfg(...)]`) associated directly with this `mod` item.
    pub cfgs: Vec<String>,
}

impl ModuleNode {
    /// Returns the type-safe identifier (`ModuleNodeId`) for this module node.
    ///
    /// This is the preferred way to access the ID when the type context (module) is known,
    /// ensuring compile-time safety for operations like defining relations.
    pub fn module_id(&self) -> ModuleNodeId {
        self.id
    }

    /// Returns the canonical definition path for this module.
    ///
    /// This path represents how the module is addressed within the crate structure,
    /// regardless of whether it's defined inline, in a separate file, or declared
    /// with a `#[path]` attribute.

    /// Definition path to file as it would be called by a `use` statement,
    /// Examples:
    ///     module declaration in project/main.rs
    ///         "mod module_one;" -> ["crate", "module_one"]
    ///     file module:
    ///         project/module_one/mod.rs -> ["crate", "module_one"]
    ///     in-line module in project/module_one/mod.rs
    ///         `mod module_two {}` -> ["crate", "module_one", "module_two"]
    pub fn defn_path(&self) -> &Vec<String> {
        &self.path
    }

    /// Returns true if this is a file-based module
    pub fn is_file_based(&self) -> bool {
        matches!(self.module_def, ModuleKind::FileBased { .. })
    }

    /// Returns true if this is an inline module
    pub fn is_inline(&self) -> bool {
        matches!(self.module_def, ModuleKind::Inline { .. })
    }

    /// Returns true if this is just a module declaration
    pub fn is_declaration(&self) -> bool {
        matches!(self.module_def, ModuleKind::Declaration { .. })
    }

    /// Returns the items if this is an inline module, None otherwise
    pub fn items(&self) -> Option<&[NodeId]> {
        match &self.module_def {
            ModuleKind::Inline { items, .. } => Some(items),
            ModuleKind::FileBased { items, .. } => Some(items),
            ModuleKind::Declaration { .. } => None,
        }
    }

    /// Returns the file path if this is a file-based module, None otherwise
    pub fn file_path(&self) -> Option<&PathBuf> {
        if let ModuleKind::FileBased { file_path, .. } = &self.module_def {
            Some(file_path)
        } else {
            None
        }
    }

    /// Returns the file path relative to a given `Path` if this is a file-based module,
    /// None otherwise.
    pub fn file_path_relative_to(&self, base: &Path) -> Option<&Path> {
        if let ModuleKind::FileBased { file_path, .. } = &self.module_def {
            file_path.strip_prefix(base).ok()
        } else {
            None
        }
    }

    pub fn file_name(&self) -> Option<&OsStr> {
        if let ModuleKind::FileBased { file_path, .. } = &self.module_def {
            file_path.file_name()
        } else {
            None
        }
    }

    /// Returns the file attributes if this is a file-based module, None otherwise
    pub fn file_attrs(&self) -> Option<&[Attribute]> {
        if let ModuleKind::FileBased { file_attrs, .. } = &self.module_def {
            Some(file_attrs)
        } else {
            None
        }
    }

    /// Returns the file docs if this is a file-based module, None otherwise
    pub fn file_docs(&self) -> Option<&String> {
        if let ModuleKind::FileBased { file_docs, .. } = &self.module_def {
            // Want to return the reference to the inner type, not Option (using .as_ref())
            file_docs.as_ref()
        } else {
            None
        }
    }

    /// Returns the span if this is an inline module, None otherwise
    pub fn inline_span(&self) -> Option<(usize, usize)> {
        if let ModuleKind::Inline { span, .. } = &self.module_def {
            Some(*span)
        } else {
            None
        }
    }

    /// Returns the declaration span if this is a module declaration, None otherwise
    pub fn declaration_span(&self) -> Option<(usize, usize)> {
        if let ModuleKind::Declaration {
            declaration_span, ..
        } = &self.module_def
        {
            Some(*declaration_span)
        } else {
            None
        }
    }

    /// Returns the resolved definition if this is a module declaration, None otherwise
    pub fn resolved_definition(&self) -> Option<NodeId> {
        if let ModuleKind::Declaration {
            resolved_definition,
            ..
        } = &self.module_def
        {
            *resolved_definition
        } else {
            None
        }
    }

    /// Checks module to see if it has a #[path = "..."] attribute.
    /// Only checks module declarations, e.g.
    /// ```rust,ignore
    /// #[path = "path/to/file.rs"]
    /// mod my_mod;
    /// ```
    pub fn has_path_attr(&self) -> bool {
        self.is_declaration() && self.attributes.iter().any(|attr| attr.name == "path")
    }
}

/// Distinguishes how a module is syntactically represented in the source code.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ModuleKind {
    /// Represents a module whose definition resides in a separate file
    /// (e.g., `foo.rs` or `foo/mod.rs`) referenced by `mod foo;`.
    FileBased {
        /// NodeIds of items directly contained within the module's file.
        /// Note: May be temporary; `Relation::Contains` is the primary source.
        items: Vec<NodeId>,
        /// The absolute path to the file containing the module definition.
        file_path: PathBuf,
        /// Inner attributes (`#![...]`) found at the top of the module file.
        file_attrs: Vec<Attribute>,
        /// Inner documentation (`//! ...`) found at the top of the module file.
        file_docs: Option<String>,
    },
    /// Represents a module defined inline within another file
    /// (e.g., `mod foo { ... }`).
    Inline {
        /// NodeIds of items directly contained within the inline module block.
        /// Note: May be temporary; `Relation::Contains` is the primary source.
        items: Vec<NodeId>,
        /// The byte span (start, end) of the inline module block (`{ ... }`).
        span: (usize, usize),
    },
    /// Represents only the declaration of a module (`mod foo;`) whose definition
    /// needs to be found elsewhere (either inline or file-based).
    Declaration {
        /// The byte span (start, end) of the `mod foo;` declaration statement.
        declaration_span: (usize, usize),
        /// The `NodeId` of the corresponding `FileBased` or `Inline` `ModuleNode`
        /// after the module tree has been resolved. `None` before resolution.
        resolved_definition: Option<NodeId>,
    },
}

impl GraphNode for ModuleNode {
    fn id(&self) -> NodeId {
        self.id.into_inner() // Return base NodeId
    }
    fn visibility(&self) -> &VisibilityKind {
        &self.visibility
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }

    fn as_module(&self) -> Option<&ModuleNode> {
        Some(self)
    }
}

impl HasAttributes for ModuleNode {
    fn attributes(&self) -> &[Attribute] {
        // Return the attributes associated with the `mod` item itself,
        // not the inner file attributes (`#![...]`).
        &self.attributes
    }
}

/// A type-safe wrapper around a `NodeId` that specifically identifies a `ModuleNode`.
///
/// This prevents accidentally using the ID of a different node type (e.g., `FunctionNodeId`)
/// where a module ID is expected, particularly in defining `SyntacticRelation` variants.
///
/// Instances of this type should only be obtained via `ModuleNode::module_id()`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ModuleNodeId(NodeId);

impl ModuleNodeId {
    // Note: No public `::new(NodeId)` constructor is provided to enforce that
    // a `ModuleNodeId` can only be obtained from an actual `ModuleNode` instance.

    /// Consumes the wrapper and returns the underlying base `NodeId`.
    ///
    /// This is an "escape hatch" necessary for certain operations like hashing,
    /// storing in generic collections keyed by `NodeId`, or interfacing with
    /// functions that require the base ID. Use sparingly where type safety
    /// is not the primary concern for the operation.
    pub fn into_inner(self) -> NodeId {
        self.0
    }

    /// Returns a reference to the underlying base `NodeId`.
    ///
    /// Similar to `into_inner`, this provides access to the base ID when needed,
    /// without consuming the typed wrapper. Use deliberately.
    pub fn as_inner(&self) -> &NodeId {
        &self.0
    }
}

impl std::fmt::Display for ModuleNodeId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Delegate to the inner NodeId's Display implementation
        write!(f, "{}", self.0)
    }
}
