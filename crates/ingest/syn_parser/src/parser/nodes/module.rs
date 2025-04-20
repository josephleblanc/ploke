use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
};

use ploke_core::{NodeId, TrackingHash};

use super::*;

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
// #[derive(Error, Debug, Clone, PartialEq, Eq)]
pub struct ModuleNode {
    pub id: NodeId,
    pub name: String,
    pub path: Vec<String>,
    pub visibility: VisibilityKind,
    pub attributes: Vec<Attribute>, // Attributes on the `mod foo { ... }` item itself
    pub docstring: Option<String>,
    pub imports: Vec<ImportNode>,
    pub exports: Vec<NodeId>, // TODO: Confirm if exports need tracking hash? Likely not.
    pub span: (usize, usize), // Add span field
    pub tracking_hash: Option<TrackingHash>,
    pub module_def: ModuleDef,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item (`#[cfg] mod foo;` or `#[cfg] mod foo {}`)
}

impl ModuleNode {
    /// Definition path to file as it would be called by a `use` statement,
    /// Examples:
    ///     module declaration in project/main.rs
    ///         "mod module_one;" -> ["crate", "module_one"]
    ///     file module:
    ///         project/module_one/mod.rs -> ["crate", "module_one"]
    ///     in-line module in project/module_one/mod.rs
    ///         `mod module_two {}` -> ["crate", "module_one", "module_two"]
    pub fn defn_path(&self) -> Vec<String> {
        let path = self.path.clone();
        path.to_vec().push(self.name.clone());
        path
    }

    /// Returns true if this is a file-based module
    pub fn is_file_based(&self) -> bool {
        matches!(self.module_def, ModuleDef::FileBased { .. })
    }

    /// Returns true if this is an inline module
    pub fn is_inline(&self) -> bool {
        matches!(self.module_def, ModuleDef::Inline { .. })
    }

    /// Returns true if this is just a module declaration
    pub fn is_declaration(&self) -> bool {
        matches!(self.module_def, ModuleDef::Declaration { .. })
    }

    /// Returns the items if this is an inline module, None otherwise
    pub fn items(&self) -> Option<&[NodeId]> {
        match &self.module_def {
            ModuleDef::Inline { items, .. } => Some(items),
            ModuleDef::FileBased { items, .. } => Some(items),
            ModuleDef::Declaration { .. } => None,
        }
    }

    /// Returns the file path if this is a file-based module, None otherwise
    pub fn file_path(&self) -> Option<&PathBuf> {
        if let ModuleDef::FileBased { file_path, .. } = &self.module_def {
            Some(file_path)
        } else {
            None
        }
    }

    /// Returns the file path relative to a given `Path` if this is a file-based module,
    /// None otherwise.
    pub fn file_path_relative_to(&self, base: &Path) -> Option<&Path> {
        if let ModuleDef::FileBased { file_path, .. } = &self.module_def {
            file_path.strip_prefix(base).ok()
        } else {
            None
        }
    }

    pub fn file_name(&self) -> Option<&OsStr> {
        if let ModuleDef::FileBased { file_path, .. } = &self.module_def {
            file_path.file_name()
        } else {
            None
        }
    }

    /// Returns the file attributes if this is a file-based module, None otherwise
    pub fn file_attrs(&self) -> Option<&[Attribute]> {
        if let ModuleDef::FileBased { file_attrs, .. } = &self.module_def {
            Some(file_attrs)
        } else {
            None
        }
    }

    /// Returns the file docs if this is a file-based module, None otherwise
    pub fn file_docs(&self) -> Option<&String> {
        if let ModuleDef::FileBased { file_docs, .. } = &self.module_def {
            // Want to return the reference to the inner type, not Option (using .as_ref())
            file_docs.as_ref()
        } else {
            None
        }
    }

    /// Returns the span if this is an inline module, None otherwise
    pub fn inline_span(&self) -> Option<(usize, usize)> {
        if let ModuleDef::Inline { span, .. } = &self.module_def {
            Some(*span)
        } else {
            None
        }
    }

    /// Returns the declaration span if this is a module declaration, None otherwise
    pub fn declaration_span(&self) -> Option<(usize, usize)> {
        if let ModuleDef::Declaration {
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
        if let ModuleDef::Declaration {
            resolved_definition,
            ..
        } = &self.module_def
        {
            *resolved_definition
        } else {
            None
        }
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, Eq)]
pub enum ModuleDef {
    /// File-based module (src/module/mod.rs)
    FileBased {
        items: Vec<NodeId>, // Probably temporary while gaining confidence in Relation::Contains
        file_path: PathBuf,
        file_attrs: Vec<Attribute>, // Non-CFG #![...] attributes
        file_docs: Option<String>,  // //!
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
        self.id
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
}
