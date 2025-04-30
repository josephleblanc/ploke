use crate::define_node_info_struct; // Import macro
use ploke_core::{NodeId, TrackingHash}; // Import NodeId
use serde::{Deserialize, Serialize};
use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Module Node ---

define_node_info_struct! {
    /// Temporary info struct for creating a ModuleNode.
    ModuleNodeInfo {
        name: String,
        path: Vec<String>,
        visibility: VisibilityKind,
        attributes: Vec<Attribute>,
        docstring: Option<String>,
        imports: Vec<ImportNode>,
        exports: Vec<NodeId>, // Keep as NodeId for now, resolution might change this
        span: (usize, usize),
        tracking_hash: Option<TrackingHash>,
        module_def: ModuleKind,
        cfgs: Vec<String>,
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub struct ModuleNode {
    pub id: ModuleNodeId, // Use typed ID
    pub name: String,
    pub path: Vec<String>,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub imports: Vec<ImportNode>,
    pub exports: Vec<NodeId>, // Keep as NodeId for now
    pub span: (usize, usize),
    pub tracking_hash: Option<TrackingHash>,
    pub module_def: ModuleKind,
    pub cfgs: Vec<String>,
}

impl ModuleNode {
    /// Returns the typed ID for this module node.
    pub fn module_id(&self) -> ModuleNodeId {
        self.id
    }

    /// Creates a new `ModuleNode` from `ModuleNodeInfo`.
    pub(crate) fn new(info: ModuleNodeInfo) -> Self {
        Self {
            id: ModuleNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            path: info.path,
            visibility: info.visibility,
            attributes: info.attributes,
            docstring: info.docstring,
            imports: info.imports,
            exports: info.exports,
            span: info.span,
            tracking_hash: info.tracking_hash,
            module_def: info.module_def,
            cfgs: info.cfgs,
        }
    }

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

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ModuleKind {
    /// File-based module (src/module/mod.rs)
    FileBased {
        items: Vec<NodeId>, // Probably temporary while gaining confidence in Relation::Contains
        file_path: PathBuf,
        file_attrs: Vec<Attribute>, // Non-CFG #![...] attributes
        file_docs: Option<String>,  // e.g. `//! Doc comment`
    },
    /// Inline module (mod name { ... })
    Inline {
        items: Vec<NodeId>, // References to contained items
        span: (usize, usize),
    },
    /// Declaration only (mod name;)
    Declaration {
        declaration_span: (usize, usize),
        resolved_definition: Option<NodeId>, // Populated during resolution phase
    },
}

impl GraphNode for ModuleNode {
    fn id(&self) -> NodeId {
        self.id.into_inner() // Return base NodeId
    }
    fn visibility(&self) -> VisibilityKind {
        self.visibility.clone()
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, PartialOrd, Ord)]
pub struct ModuleNodeId(NodeId);
impl ModuleNodeId {
    /// Create from raw NodeId
    pub fn new(id: NodeId) -> Self {
        Self(id)
    }

    /// Get inner NodeId
    pub fn into_inner(self) -> NodeId {
        self.0
    }

    /// Get reference to inner NodeId
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
