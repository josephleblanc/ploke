use crate::parser::graph::GraphAccess;
use derive_test_helpers::ExpectedData;
use ploke_core::TrackingHash;
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};
use syn_parser_macros::GenerateNodeInfo;
// removed GenerateNodeInfo

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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, GenerateNodeInfo, ExpectedData)] // Add derive
pub struct ModuleNode {
    /// The type-safe identifier for this specific module node.
    pub id: ModuleNodeId,
    /// The simple name of the module (e.g., "foo" for `mod foo;`).
    pub name: String,
    /// The fully resolved definition path of this module (e.g., `["crate", "foo", "bar"]`).
    /// This path is constructed during parsing and module tree building.
    /// During initial parsing this path is not verifiable due to inherent limitations of parallel
    /// parsing with no shared cross-thread communication, however after module tree construction
    /// the path of all modules should be canonical. Until then it is a best guess (pending
    /// resolution of the `[#path]` attribute, either for itself or potentially parent modules.)
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
    /// List ofImportNodeIds that are re-exported (`pub use`) from this module.
    /// Populated during the resolution phase (ModuleTree processing).
    pub exports: Vec<ImportNodeId>,
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
    ///
    /// Definition path to file as it would be called by a `use` statement,
    /// Examples:
    ///     `mod module_two {}` -> ["crate", "module_one", "module_two"]
    pub fn path(&self) -> &Vec<String> {
        &self.path
    }

    /// Returns `true` if this module node represents a definition stored in a separate file
    /// (e.g., `mod foo;` pointing to `foo.rs` or `foo/mod.rs`).
    pub fn is_file_based(&self) -> bool {
        matches!(self.module_def, ModuleKind::FileBased { .. })
    }

    /// Returns `true` if this module node represents a definition written inline
    /// (e.g., `mod foo { ... }`).
    pub fn is_inline(&self) -> bool {
        matches!(self.module_def, ModuleKind::Inline { .. })
    }

    /// Returns `true` if this module node represents only a declaration
    /// (e.g., `mod foo;`) whose definition needs to be resolved.
    pub fn is_decl(&self) -> bool {
        matches!(self.module_def, ModuleKind::Declaration { .. })
    }

    /// Returns thePrimaryNodeIds of items directly contained within this module definition,
    /// if this represents a `FileBased` or `Inline` module definition.
    /// Returns `None` if this is only a `Declaration`.
    /// Note: This might be temporary; `Relation::Contains` is the primary way to query containment.
    pub fn items(&self) -> Option<&[PrimaryNodeId]> {
        match &self.module_def {
            ModuleKind::Inline { items, .. } => Some(items),
            ModuleKind::FileBased { items, .. } => Some(items),
            ModuleKind::Declaration { .. } => None,
        }
    }

    /// Returns the absolute file path if this is a `FileBased` module, `None` otherwise.
    pub fn file_path(&self) -> Option<&PathBuf> {
        if let ModuleKind::FileBased { file_path, .. } = &self.module_def {
            Some(file_path)
        } else {
            None
        }
    }

    /// Returns the file path relative to a given base path if this is a `FileBased` module
    /// and the path is relative to the base, `None` otherwise.
    pub fn file_path_relative_to(&self, base: &Path) -> Option<&Path> {
        if let ModuleKind::FileBased { file_path, .. } = &self.module_def {
            file_path.strip_prefix(base).ok()
        } else {
            None
        }
    }

    /// Returns the file name (e.g., "mod.rs", "foo.rs") if this is a `FileBased` module,
    /// `None` otherwise.
    pub fn file_name(&self) -> Option<&OsStr> {
        if let ModuleKind::FileBased { file_path, .. } = &self.module_def {
            file_path.file_name()
        } else {
            None
        }
    }

    /// Returns the inner attributes (`#![...]`) if this is a `FileBased` module, `None` otherwise.
    pub fn file_attrs(&self) -> Option<&[Attribute]> {
        if let ModuleKind::FileBased { file_attrs, .. } = &self.module_def {
            Some(file_attrs)
        } else {
            None
        }
    }

    /// Returns the inner documentation (`//! ...`) if this is a `FileBased` module, `None` otherwise.
    pub fn file_docs(&self) -> Option<&String> {
        if let ModuleKind::FileBased { file_docs, .. } = &self.module_def {
            // Want to return the reference to the inner type, not Option (using .as_ref())
            file_docs.as_ref()
        } else {
            None
        }
    }

    /// Returns the byte span of the inline module definition (`mod foo { ... }`)
    /// if this is an `Inline` module, `None` otherwise.
    pub fn inline_span(&self) -> Option<(usize, usize)> {
        if let ModuleKind::Inline { span, .. } = &self.module_def {
            Some(*span)
        } else {
            None
        }
    }

    /// Returns the byte span of the module declaration (`mod foo;`)
    /// if this is a `Declaration` module, `None` otherwise.
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

    /// Returns the `NodeId` of the resolved definition module if this is a `Declaration`
    /// and resolution has occurred, `None` otherwise.
    pub fn resolved_definition(&self) -> Option<ModuleNodeId> {
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

    /// Checks if this module node (specifically a `Declaration`) has a `#[path = "..."]` attribute.
    ///
    /// Example:
    /// ```rust,ignore
    /// #[path = "path/to/file.rs"]
    /// mod my_mod;
    /// ```
    pub fn has_path_attr(&self) -> bool {
        self.is_decl() && self.attributes.iter().any(|attr| attr.name == "path")
    }
}

/// Distinguishes how a module is syntactically represented in the source code.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ModuleKind {
    /// Represents a module whose definition resides in a separate file
    /// (e.g., `foo.rs` or `foo/mod.rs`) referenced by `mod foo;`.
    FileBased {
        /// PrimaryNodeIds of items directly contained within the module's file.
        /// Note: May be temporary; `Relation::Contains` is the primary source.
        items: Vec<PrimaryNodeId>,
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
        /// PrimaryNodeIds of items directly contained within the inline module block.
        /// Note: May be temporary; `Relation::Contains` is the primary source.
        items: Vec<PrimaryNodeId>,
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
        resolved_definition: Option<ModuleNodeId>,
    },
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq, Copy, Hash, PartialOrd, Ord)]
pub enum ModDisc {
    FileBased,
    Inline,
    Declaration,
}

impl GraphNode for ModuleNode {
    fn any_id(&self) -> AnyNodeId {
        self.id.into() // Return base ModuleNodeId
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
