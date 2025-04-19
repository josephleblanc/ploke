use ploke_core::NodeId;
use thiserror::Error;

use crate::parser::nodes::{ModuleNode, NodeError};

/// Custom error type for the syn_parser crate.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
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

    /// Indicates a duplicate path was encountered when building the ModuleTree.
    #[error("Duplicate path found in module tree for NodeId: {0}")]
    ModuleTreeDuplicatePath(NodeId),

    /// Indicates a duplicate module ID was encountered when building the ModuleTree.
    #[error("Duplicate module ID found in module tree for ModuleNode: {0}")]
    ModuleTreeDuplicateModuleId(String), // Store Debug representation

    #[error("Module definition not found for path: {0}")]
    ModuleDefinitionNotFound(String), // Store path string representation

    #[error("Relation conversion error: {0}")]
    RelationConversion(#[from] crate::parser::relations::RelationConversionError),
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
            crate::parser::module_tree::ModuleTreeError::DuplicatePath(id) => {
                SynParserError::ModuleTreeDuplicatePath(id)
            }
            crate::parser::module_tree::ModuleTreeError::DuplicateModuleId(node) => {
                // The `node` variable is already a Box<ModuleNode> from the ModuleTreeError variant.
                // Pass it directly to the SynParserError variant which expects a Box<ModuleNode>.
                SynParserError::DuplicateInModuleTree(node)
                // Note: We are now using DuplicateInModuleTree instead of ModuleTreeDuplicateModuleId
                // Consider if ModuleTreeDuplicateModuleId(String) is still needed or if
                // DuplicateInModuleTree(Box<ModuleNode>) covers the necessary cases.
                // For now, let's assume DuplicateInModuleTree is sufficient.
                // If ModuleTreeDuplicateModuleId is still needed elsewhere, it should remain.
            }
            crate::parser::module_tree::ModuleTreeError::NodePathValidation(syn_err) => {
                // If it's already a SynParserError, just return it
                syn_err
            }
            crate::parser::module_tree::ModuleTreeError::DefinitionNotFound(path) => {
                SynParserError::ModuleDefinitionNotFound(path.to_string())
            }
        }
    }
}

// Implement From<NodeError> for SynParserError
impl From<NodeError> for SynParserError {
    fn from(err: crate::parser::nodes::NodeError) -> Self {
        match err {
            NodeError::Validation(msg) => SynParserError::NodeValidation(msg),
            // Add other NodeError variants here if they exist in the future
        }
    }
}

// Optional: Implement From<syn::Error> for SynParserError
impl From<syn::Error> for SynParserError {
    fn from(err: syn::Error) -> Self {
        SynParserError::Syn(err.to_string())
    }
}
