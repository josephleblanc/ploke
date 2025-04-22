use ploke_core::{NodeId, TypeId};
use thiserror::Error;

use crate::parser::{
    module_tree::ModuleTreeError,
    nodes::{ModuleNode, NodeError},
};

/// Custom error type for the syn_parser crate.
#[derive(Error, Debug, Clone, PartialEq)] // Removed Eq
pub enum SynParserError {
    /// Indicates that a requested node was not found in the graph.
    #[error("Node with ID {0} not found in the graph.")]
    NotFound(NodeId),

    /// Indicates that multiple nodes were found when exactly one was expected.
    #[error("Duplicate node found for ID {0} when only one was expected.")]
    DuplicateNode(NodeId),

    #[error("Duplicate node found for ModuleNode in ModuleTree construction: {0:?} ")]
    DuplicateInModuleTree(Box<ModuleNode>), // Box the large ModuleNode

    /// Represents an I/O error during file discovery or reading.
    #[error("I/O error: {0}")]
    Io(String), // Wrap std::io::Error details in a String for simplicity

    /// Represents a parsing error from the `syn` crate.
    #[error("Syn parsing error: {0}")]
    Syn(String), // Wrap syn::Error details in a String

    /// Indicates an invalid state or inconsistency within the visitor or graph.
    #[error("Internal state error: {0}")]
    InternalState(String),

    /// Indicates a failure to merge graphs
    #[error("Failed to merge CodeGraphs")]
    MergeError,

    /// Indicates that merging requires at least one graph.
    #[error("Merging code graphs requires at least one graph as input.")]
    MergeRequiresInput,

    /// Indicates that the root module ("crate") could not be found.
    #[error("Root module ('crate') not found in the graph.")]
    RootModuleNotFound,

    /// Indicates that a module with the specified path was not found.
    #[error("Module with path {0:?} not found.")]
    ModulePathNotFound(Vec<String>),

    /// Indicates that multiple modules were found for the specified path.
    #[error("Duplicate modules found for path {0:?}.")]
    DuplicateModulePath(Vec<String>),

    #[error("Resolution error: {0}")]
    ResolutionError(#[from] ResolutionError),

    /// Indicates a validation error related to node structure (e.g., NodePath).
    #[error("Node validation error: {0}")]
    NodeValidation(String),

    /// Indicates a duplicate definition path was encountered when building the ModuleTree.
    #[error("Duplicate definition path '{path}' found in module tree. Existing ID: {existing_id}, Conflicting ID: {conflicting_id}")]
    ModuleTreeDuplicateDefnPath {
        // New variant
        path: String, // Store path as String for simplicity in SynParserError
        existing_id: NodeId,
        conflicting_id: NodeId,
    },

    /// Indicates a duplicate module ID was encountered when building the ModuleTree.
    #[error("Duplicate module ID found in module tree for ModuleNode: {0}")]
    ModuleTreeDuplicateModuleId(String), // Store Debug representation

    // Removed ModuleDefinitionNotFound - covered by ModuleTreeError::FoundUnlinkedModules
    // #[error("Module definition not found for path: {0}")]
    // ModuleDefinitionNotFound(String), // Store path string representation
    #[error("Relation conversion error: {0}")]
    RelationConversion(#[from] crate::parser::relations::RelationConversionError),

    // Forward ModuleTreeError variants - REMOVED #[from]
    #[error(transparent)]
    ModuleTreeError(ModuleTreeError),

    #[error("Relation conversion error: {0}")]
    TypeIdConversionError(TypeId), // Consider renaming if it's not just TypeId
    #[error("Graph ID conversion error: {0}")]
    GraphIdConversionError(String), // Store string representation
}

#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ResolutionError {
    #[error("Private item: {0:?}")]
    PrivateItem(Vec<String>),

    #[error("Ambiguous path. Candidates: {0:?}")]
    AmbiguousPath(Vec<Vec<String>>),

    #[error("Not found: {0:?}")]
    NotFound(Vec<String>),
}

// Optional: Implement From<std::io::Error> for SynParserError
impl From<std::io::Error> for SynParserError {
    fn from(err: std::io::Error) -> Self {
        SynParserError::Io(err.to_string())
    }
}

// Implement From<ModuleTreeError> for SynParserError
impl From<crate::parser::module_tree::ModuleTreeError> for SynParserError {
    fn from(err: crate::parser::module_tree::ModuleTreeError) -> Self {
        match err {
            ModuleTreeError::DuplicatePath {
                path,
                existing_id,
                conflicting_id,
            } => {
                SynParserError::ModuleTreeDuplicateDefnPath {
                    path: path.to_string(), // Convert NodePath to String
                    existing_id,
                    conflicting_id,
                }
            }
            ModuleTreeError::DuplicateModuleId(node) => {
                // The `node` variable is already a Box<ModuleNode> from the ModuleTreeError variant.
                // Pass it directly to the SynParserError variant which expects a Box<ModuleNode>.
                SynParserError::DuplicateInModuleTree(node)
                // Note: We are now using DuplicateInModuleTree instead of ModuleTreeDuplicateModuleId
                // Consider if ModuleTreeDuplicateModuleId(String) is still needed or if
                // DuplicateInModuleTree(Box<ModuleNode>) covers the necessary cases.
                // For now, let's assume DuplicateInModuleTree is sufficient.
                // If ModuleTreeDuplicateModuleId is still needed elsewhere, it should remain.
            }
            ModuleTreeError::NodePathValidation(boxed_syn_err) => *boxed_syn_err,
            ModuleTreeError::ContainingModuleNotFound(node_id) => SynParserError::InternalState(
                format!("Containing module not found for node ID: {}", node_id),
            ),
            ModuleTreeError::FoundUnlinkedModules(_) => {
                // This conversion shouldn't usually happen if handled in the caller.
                // If it does, it indicates an unexpected flow.
                SynParserError::InternalState(
                    "FoundUnlinkedModules error encountered unexpectedly during conversion."
                        .to_string(),
                )
            }
            ModuleTreeError::ItemNotPubliclyAccessible(node_id) => {
                SynParserError::InternalState(format!(
                    "Item {} is not publicly accessible.", // Keep error message simple
                    node_id
                ))
                // Or define a new SynParserError variant if more specific handling is needed
            }
            ModuleTreeError::GraphIdConversion(graph_id_err) => {
                SynParserError::from(NodeError::GraphIdConversion(graph_id_err))
            }
            ModuleTreeError::NodeError(node_err) => {
                // Convert the inner NodeError into SynParserError using its existing From impl
                SynParserError::from(node_err)
            }
            ModuleTreeError::SynParserError(boxed_syn_err) => {
                // Simply unbox the inner SynParserError
                *boxed_syn_err
            }
            ModuleTreeError::FilePathMissingParent(path_buf) => {
                // Convert to a general InternalState error, as this indicates an unexpected
                // file system structure or inconsistent path handling within the tree.
                SynParserError::InternalState(format!(
                    "Could not determine parent directory for file path: {}",
                    path_buf.display()
                ))
            }
            ModuleTreeError::RootModuleNotFileBased(module_node_id) => {
                // This is a critical internal error indicating the ModuleTree was
                // constructed incorrectly (root must be file-based).
                SynParserError::InternalState(format!(
                    "Root module {} is not file-based, which is required.",
                    module_node_id
                ))
            }
            ModuleTreeError::ConflictingReExportPath {
                path,
                existing_id,
                conflicting_id,
            } => {
                // Treat conflicting re-exports as an internal state error,
                // as it indicates an inconsistency discovered during processing.
                SynParserError::InternalState(format!(
                    "Conflicting re-export path '{}' detected. Existing ID: {}, Conflicting ID: {}",
                    path, // NodePath implements Display
                    existing_id,
                    conflicting_id
                ))
            }
            ModuleTreeError::ReExportChainTooLong { start_node_id } => {
                // Treat excessively long re-export chains as an internal state error,
                // indicating a potential cycle or problematic structure.
                SynParserError::InternalState(format!(
                    "Re-export chain starting from {} exceeded maximum depth (32).",
                    start_node_id
                ))
            }
            // --- NEW ARMS ---
            ModuleTreeError::DuplicatePathAttribute {
                module_id,
                existing_path,
                conflicting_path,
            } => SynParserError::InternalState(format!(
                "Duplicate path attribute found for module {}. Existing: '{}', Conflicting: '{}'",
                module_id,
                existing_path.display(),
                conflicting_path.display()
            )),
            ModuleTreeError::UnresolvedPathAttr(inner_err) => {
                // Recursively convert the inner error, then wrap in InternalState
                let syn_err: SynParserError = (*inner_err).into(); // Convert Box<ModuleTreeError>
                SynParserError::InternalState(format!(
                    "Failed to resolve path attribute: {}",
                    syn_err // Display the converted inner error
                ))
            }
            ModuleTreeError::ModuleNotFound(module_id) => SynParserError::InternalState(format!(
                "Module with ID {} not found in ModuleTree.modules map.",
                module_id
            )),
            ModuleTreeError::DuplicateDefinition(msg) => SynParserError::InternalState(format!(
                "ModuleTree processing error: {}",
                msg
            )),
            ModuleTreeError::ModuleDefinitionNotFound(msg) => SynParserError::InternalState(
                format!("ModuleTree processing error: {}", msg),
            ),
            // --- END NEW ARMS ---
        }
    }
}

// Implement From<NodeError> for SynParserError
impl From<NodeError> for SynParserError {
    fn from(err: crate::parser::nodes::NodeError) -> Self {
        match err {
            NodeError::Validation(msg) => SynParserError::NodeValidation(msg),
            NodeError::Conversion(type_id) => SynParserError::TypeIdConversionError(type_id), // Keep existing
            NodeError::GraphIdConversion(graph_id_err) => {
                // Convert the GraphIdConversionError into a String for SynParserError
                SynParserError::GraphIdConversionError(graph_id_err.to_string())
            }
        }
    }
}

// Optional: Implement From<syn::Error> for SynParserError
impl From<syn::Error> for SynParserError {
    fn from(err: syn::Error) -> Self {
        SynParserError::Syn(err.to_string())
    }
}
