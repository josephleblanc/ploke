use std::path::PathBuf;

use crate::{
    error::SynParserError,
    parser::nodes::{
        self, AnyNodeId, AnyNodeIdConversionError, GraphNode, ModuleNode, ModuleNodeId, NodePath,
        ReexportNodeId, TryFromPrimaryError,
    },
    utils::{LogStyle, LogStyleDebug},
};

use super::{TreeRelation, UnlinkedModuleInfo};

// Define the new ModuleTreeError enum
#[derive(thiserror::Error, Debug, Clone, PartialEq)]
pub enum ModuleTreeError {
    #[error("Duplicate definition path '{path}' found in module tree. Existing ID: {existing_id}, Conflicting ID: {conflicting_id}")]
    DuplicatePath {
        // Change to a struct variant
        path: NodePath,
        existing_id: AnyNodeId,
        conflicting_id: AnyNodeId,
    },
    #[error("Duplicate definition module_id '{module_id}' found in module tree. Existing path attribute: {existing_path}, Conflicting path attribute: {conflicting_path}")]
    DuplicatePathAttribute {
        module_id: ModuleNodeId,
        existing_path: PathBuf,
        conflicting_path: PathBuf,
    },

    #[error("Duplicate module ID found in module tree for ModuleNode: {0:?}")]
    DuplicateModuleId(Box<ModuleNode>), // Box the large ModuleNode

    #[error("Duplicate Contains relation found: {0:?}")]
    DuplicateContains(TreeRelation), // Box the large ModuleNode

    /// Wraps SynParserError for convenience when using TryFrom<Vec<String>> for NodePath
    #[error("Node path validation error: {0}")]
    NodePathValidation(Box<SynParserError>), // Box the recursive type

    #[error("Containing module not found for node ID: {0}")]
    ContainingModuleNotFound(AnyNodeId), // Added error variant

    // NEW: Variant holding a collection of UnlinkedModuleInfo
    // Corrected format string - the caller logs the count/details
    #[error("Found unlinked module file(s) (no corresponding 'mod' declaration).")]
    FoundUnlinkedModules(Box<Vec<UnlinkedModuleInfo>>), // Use Box as requested

    #[error("Item with ID {0} is not publicly accessible from the crate root.")]
    ItemNotPubliclyAccessible(AnyNodeId), // New error variant for SPP

    #[error("Node error: {0}")]
    NodeError(#[from] nodes::NodeError), // Add #[from] for NodeError

    #[error("Syn parser error: {0}")]
    SynParserError(Box<SynParserError>), // REMOVE #[from]
    //
    #[error("Could not determine parent directory for file path: {0}")]
    FilePathMissingParent(PathBuf), // Store the problematic path
    #[error("Root module {0} is not file-based, which is required for path resolution.")]
    RootModuleNotFileBased(ModuleNodeId),
    #[error("Invalid State: Root module {0} not found")]
    RootModuleNotFound(ModuleNodeId),

    // --- NEW VARIANT ---
    #[error("Conflicting re-export path '{path}' detected. Existing ID: {existing_id}, Conflicting ID: {conflicting_id}")]
    ConflictingReExportPath {
        path: NodePath,
        existing_id: ReexportNodeId,    // Changed: Use ReexportNodeId
        conflicting_id: ReexportNodeId, // Changed: Use ReexportNodeId
    },

    // --- NEW VARIANT ---
    #[error("Re-export chain starting from {start_node_id} exceeded maximum depth (32). Potential cycle or excessively deep re-export.")]
    ReExportChainTooLong { start_node_id: AnyNodeId },

    #[error("Implement me!")]
    UnresolvedPathAttr(Box<ModuleTreeError>), // Placeholder, fill in with contextual information

    #[error("ModuleId not found in ModuleTree.modules: {0}")]
    ModuleNotFound(ModuleNodeId),

    // --- NEW VARIANTS for process_path_attributes ---
    #[error("Duplicate module definitions found for path attribute target: {0}")]
    DuplicateDefinition(String), // Store detailed message
    #[error("Module definition not found for path attribute target: {0}")]
    ModuleKindinitionNotFound(String), // Store detailed message

    // --- NEW VARIANT ---
    #[error("Shortest public path resolution failed for external item re-export: {0}")]
    ExternalItemNotResolved(AnyNodeId),

    #[error("No relations found for node {0}: {1}")]
    NoRelationsFound(AnyNodeId, String),
    #[error("No relations found for node {0}")]
    NoRelationsFoundForId(AnyNodeId), // Placeholder, trying out copy-only values
    #[error("Could not resolve target for re-export '{path}'. Import Node ID: {import_node_id:?}")]
    UnresolvedReExportTarget {
        import_node_id: Option<AnyNodeId>,
        path: NodePath, // The original path that failed to resolve
    },

    // --- NEW VARIANT ---
    #[error("Invalid internal state: pending_exports was None when adding module {module_id}")]
    InvalidStatePendingExportsMissing { module_id: ModuleNodeId },
    #[error("Internal state error: {0}")]
    InternalState(String),
    #[error("Warning: {0}")]
    Warning(String),

    // --- NEW VARIANT ---
    #[error("Recursion limit ({limit}) exceeded while finding defining file path for node {start_node_id}")]
    RecursionLimitExceeded { start_node_id: AnyNodeId, limit: u8 },
    #[error("Error Converting from {0}")]
    TypedIdConversionError(#[from] TryFromPrimaryError),
    #[error("Error converting AnyNodeId: {0}")]
    AnyNodeIdConversionError(#[from] AnyNodeIdConversionError), // New variant
}

impl ModuleTreeError {
    #[allow(
        dead_code,
        reason = "clippy is wrong, this is actually used in the test directory"
    )]
    pub(super) fn no_relations_found(g_node: &dyn GraphNode) -> Self {
        Self::NoRelationsFound(
            g_node.any_id(),
            format!(
                "{} {: <12} {: <20} | {: <12} | {: <15}",
                "NodeInfo".log_header(),
                g_node.name().log_name(),
                g_node.any_id().to_string().log_id(),
                g_node.kind().log_vis_debug(),
                g_node.visibility().log_name_debug(),
            ),
        )
    }
}

// Manual implementation to satisfy the `?` operator
impl From<SynParserError> for ModuleTreeError {
    fn from(err: SynParserError) -> Self {
        ModuleTreeError::SynParserError(Box::new(err))
    }
}
