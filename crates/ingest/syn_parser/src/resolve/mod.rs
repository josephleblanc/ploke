mod error;
pub mod id_resolver;
mod logging;
pub mod module_tree;
mod path_resolver;
mod relation_indexer;

#[cfg(not(feature = "not_wip_marker"))]
pub mod traversal;

use std::{collections::HashSet, path::PathBuf};

pub use error::ModuleTreeError;

// -- local re-exports for children
use logging::LogTree;
use module_tree::*;
use path_resolver::*;
use relation_indexer::*;
use serde::{Deserialize, Serialize};

use crate::parser::{
    nodes::{
        AnyNodeId, AsAnyNodeId, ImportNodeId, PrimaryNodeId, PrimaryNodeIdTrait, ReexportNodeId,
    }, // Removed GraphNode import
};
use crate::parser::{
    nodes::{ImportNode, ModuleNodeId, NodePath},
    relations::SyntacticRelation,
};
pub use colored::Colorize;
use log::debug; // Import the debug macro
use std::{collections::HashMap, path::Path};

#[allow(unused_imports)]
use std::collections::VecDeque;

use crate::{
    error::SynParserError,
    parser::{
        nodes::{extract_path_attr_from_node, ModuleNode}, // Removed GraphNode import
        types::VisibilityKind,
        ParsedCodeGraph,
    },
    utils::{
        logging::{LogDataStructure, PathProcessingContext},
        LogStyle, LOG_TARGET_MOD_TREE_BUILD, LOG_TARGET_VIS,
    },
};

#[cfg(test)]
mod tests;

// -- end mods --

// -- common types --

/// Relations useful in the module tree.
#[derive(Debug, Serialize, Deserialize, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TreeRelation(SyntacticRelation); // Keep inner field private

impl TreeRelation {
    pub fn new(relation: SyntacticRelation) -> Self {
        Self(relation)
    }

    /// Returns a reference to the inner `Relation`.
    pub fn rel(&self) -> &SyntacticRelation {
        &self.0
    }
}

impl From<SyntacticRelation> for TreeRelation {
    fn from(value: SyntacticRelation) -> Self {
        Self::new(value)
    }
}

// Struct to hold info about unlinked modules
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnlinkedModuleInfo {
    pub module_id: ModuleNodeId,
    pub definition_path: NodePath, // Store the path that couldn't be linked
}

impl std::fmt::Display for UnlinkedModuleInfo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "Unlinked module with path {} ({})",
            self.definition_path, self.module_id
        )
    }
}

/// Holds the IDs and relations pruned from the ModuleTree.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct PruningResult {
    // Renamed struct
    /// IDs of the top-level file ModuleNodes that were pruned because they were unlinked.
    pub pruned_module_ids: HashSet<ModuleNodeId>,
    /// IDs of all items (including the modules themselves and items they contained)
    /// that were associated with the pruned modules.
    pub pruned_item_ids: HashSet<AnyNodeId>, // Changed: Use AnyNodeId
    /// The actual TreeRelation instances that were removed from the ModuleTree.
    pub pruned_relations: Vec<TreeRelation>,
}

// Add near other public structs/enums related to ModuleTree resolution
#[derive(Debug, Clone, PartialEq, Eq)] // Eq requires NodeId and PathBuf to be Eq
pub struct ResolvedItemInfo {
    /// The shortest public module path leading to the item's accessibility point.
    /// Example: `NodePath(["crate", "some_mod"])` for `crate::some_mod::MyItem`.
    pub path: NodePath, // Changed from Vec<String>

    /// The name under which the item is publicly accessible at the end of `path`.
    /// This is the name to use in code (e.g., `MyItem`, `RenamedItem`).
    /// Example: `"MyItem"` or `"RenamedItem"`
    pub public_name: String,

    /// The NodeId of the item ultimately resolved to by the public path.
    /// For internal items, this is the ID of the definition node (e.g., FunctionNode, StructNode).
    /// For external items, this is the ID of the `ImportNode` representing the `pub use`.
    pub resolved_id: AnyNodeId, // Changed: Use AnyNodeId

    /// Provides context about the nature of the `resolved_id`.
    pub target_kind: ResolvedTargetKind,

    /// The original name of the item at its definition site, if it's an internal definition
    /// and its `public_name` is different due to renaming via re-exports.
    /// `None` if the public name matches the definition name, or if the target is external.
    pub definition_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedTargetKind {
    /// The `resolved_id` points to an item defined within the current crate.
    InternalDefinition {
        /// The NodeId of the actual definition node (e.g., FunctionNode, StructNode).
        /// This will always match the outer `resolved_id` in this variant.
        definition_id: AnyNodeId, // Changed: Use AnyNodeId
    },
    /// The `resolved_id` points to an `ImportNode` that re-exports an item
    /// from an external crate.
    ExternalReExport {
        /// The path of the item within the external crate (e.g., ["log", "debug"]).
        /// The public name in the external crate is the last segment.
        external_path: Vec<String>,
    },
    // Add other kinds later if needed (e.g., Ambiguous, Private)
}

/// Indicates a file-level module whose path has been resolved from a declaration that has the
/// `#[path]` attribute, e.g.
/// ```rust,ignore
/// // somewhere in project, e.g. project/src/my_module.rs
/// #[path = "path/to/file.rs"]
/// pub mod path_attr_mod;
///
/// // In project/src/path/to/file.rs
/// pub(crate) struct HiddenStruct;
/// ```
/// The module represented by the file `path/to/file.rs`, here containing `HiddenStruct`, will have
/// its `ModuleNode { path: .. }` field resolved to ``
#[allow(dead_code)]
struct ResolvedModule {
    original_path: NodePath,     // The declared path (e.g. "path::to::file")
    filesystem_path: PathBuf,    // The resolved path from #[path] attribute
    source_span: (usize, usize), // Where the module was declared
    is_path_override: bool,      // Whether this used #[path]
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PendingImport {
    containing_mod_id: ModuleNodeId, // Keep private
    import_node: ImportNode,         // Keep private
}

impl PendingImport {
    pub(crate) fn from_import(import_node: ImportNode, containing_mod_id: ModuleNodeId) -> Self {
        // Make crate-visible if needed internally
        PendingImport {
            containing_mod_id,
            import_node,
        }
    }

    /// Returns the ID of the module containing this pending import.
    pub fn containing_mod_id(&self) -> ModuleNodeId {
        self.containing_mod_id
    }

    /// Returns a reference to the `ImportNode` associated with this pending import.
    pub fn import_node(&self) -> &ImportNode {
        &self.import_node
    }
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct PendingExport {
    containing_mod_id: ModuleNodeId, // Keep private
    export_node: ImportNode,         // Keep private
}

impl PendingExport {
    #[allow(unused_variables)]
    pub(crate) fn from_export(export: ImportNode, containing_module_id: ModuleNodeId) -> Self {
        // Make crate-visible if needed internally
        PendingExport {
            containing_mod_id: containing_module_id,
            export_node: export,
        }
    }

    /// Returns the ID of the module containing this pending export.
    pub fn containing_mod_id(&self) -> ModuleNodeId {
        self.containing_mod_id
    }

    /// Returns a reference to the `ImportNode` associated with this pending export.
    pub fn export_node(&self) -> &ImportNode {
        &self.export_node
    }
}
