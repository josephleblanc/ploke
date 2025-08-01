//! Error types for the `syn_parser` crate.

use crate::{parser::nodes::{AnyNodeId, ImportNodeId, TryFromPrimaryError}, resolve::ModuleTreeError};
use ploke_core::{IdConversionError, TypeId};
use thiserror::Error;

use crate::parser::nodes::{ModuleNode, NodeError};
use ploke_core::ItemKind; // Import ItemKind

/// Errors specific to the CodeVisitor processing.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum CodeVisitorError {
    /// Failed to register a node ID, likely because the parent module couldn't be found.
    #[error("Failed to register node ID for item '{item_name}' ({item_kind:?})")]
    RegistrationFailed {
        item_name: String,
        item_kind: ItemKind,
    },
    /// Failed to convert AnyNodeId to a specific typed ID during visitation.
    #[error(
        "Failed to convert AnyNodeId to {expected_type} for item '{item_name}' ({item_kind:?})"
    )]
    IdConversionFailed {
        item_name: String,
        item_kind: ItemKind,
        expected_type: &'static str, // e.g., "ImportNodeId"
        source_error: crate::parser::nodes::AnyNodeIdConversionError, // Keep original error info
    },
    // Add other visitor-specific errors here if needed
}

/// Custom error type for the syn_parser crate.
use crate::parser::graph::ParsedGraphError;

/// The primary error type for the `syn_parser` crate.
#[derive(Error, Debug, Clone, PartialEq)]
pub enum SynParserError {
    /// Multiple errors occurred during parsing.
    #[error("Multiple errors occurred:\n{}", .0.iter().map(|e| e.to_string()).collect::<Vec<_>>().join("\n"))]
    MultipleErrors(Vec<SynParserError>),

    /// An error occurred in a test helper.
    #[error("Test helper error: {0}")]
    TestHelperError(String), // Wrap the error message

    /// An error occurred during ID conversion.
    #[error(transparent)]
    // This allows converting *from* IdConversionError *to* SynParserError using .into() or ?
    IdConversionError(#[from] IdConversionError),

    /// An error occurred in the parsed graph.
    #[error(transparent)]
    ParsedGraphError(#[from] ParsedGraphError), // Add the new error variant

    /// A requested node was not found in the graph.
    #[error("Node with ID {0} not found in the graph.")]
    NotFound(AnyNodeId),

    /// A re-exported node was not found in the graph.
    #[error("Reexport Node with path {1:?} name {0} not found in the graph, id: {2}.")]
    ReexportNotFound(String, Vec<String>, ImportNodeId),
    /// A node with the given name and path was not found in the graph.
    #[error("Node with path {1:?} name {0} not found in the graph.")]
    NotFoundInModuleByName(String, Vec<String>),
    /// A node with the given name, path, and kind was not found in the graph.
    #[error("Node with path {1} name {0}, kind {2:?} not found in the graph.")]
    NotFoundInModuleByNameKind(String, String, ploke_core::ItemKind),
    /// Multiple nodes were found when exactly one was expected.
    #[error("Duplicate node found for ID {0} when only one was expected.")]
    DuplicateNode(AnyNodeId),

    /// A duplicate `ModuleNode` was found during `ModuleTree` construction.
    #[error("Duplicate node found for ModuleNode in ModuleTree construction: {0:?} ")]
    DuplicateInModuleTree(Box<ModuleNode>), // Box the large ModuleNode

    /// An I/O error occurred during file discovery or reading.
    #[error("I/O error: {0}")]
    Io(String), // Wrap std::io::Error details in a String for simplicity

    /// An I/O error occurred during file discovery or reading.
    #[error("Simple Discovery error: {path}")]
    SimpleDiscovery{ path: String }, // Wrap std::io::Error details in a String for simplicity
    
    /// An I/O error occurred during file discovery or reading.
    #[error("ComplexDiscovery error: {name} on path {path} from source: {source_string}")]
    ComplexDiscovery {name: String, path: String, source_string: String}, // Wrap std::io::Error details in a String for simplicity
    
    /// A parsing error from the `syn` crate occurred.
    #[error("Syn parsing error: {0}")]
    Syn(String), // Wrap syn::Error details in a String

    /// An invalid state or inconsistency within the visitor or graph was detected.
    #[error("Internal state error: {0}")]
    InternalState(String),

    /// A failure to merge `CodeGraph`s occurred.
    #[error("Failed to merge CodeGraphs")]
    MergeError,

    /// Merging requires at least one graph.
    #[error("Merging code graphs requires at least one graph as input.")]
    MergeRequiresInput,

    /// The root module ("crate") could not be found.
    #[error("Root module ('crate') not found in the graph.")]
    RootModuleNotFound,

    /// A module with the specified path was not found.
    #[error("Module with path {0:?} not found.")]
    ModulePathNotFound(Vec<String>),
    /// An item with the specified path was not found.
    #[error("Item with path {0:?} not found.")]
    ItemPathNotFound(Vec<String>),

    /// Multiple modules were found for the specified path.
    #[error("Duplicate modules found for path {0:?}.")]
    DuplicateModulePath(Vec<String>),

    /// An error occurred during resolution.
    #[error("Resolution error: {0}")]
    ResolutionError(#[from] ResolutionError),

    /// A validation error related to node structure (e.g., `NodePath`) occurred.
    #[error("Node validation error: {0}")]
    NodeValidation(String),

    /// A duplicate definition path was encountered when building the `ModuleTree`.
    #[error("Duplicate definition path '{path}' found in module tree. Existing ID: {existing_id}, Conflicting ID: {conflicting_id}")]
    ModuleTreeDuplicateDefnPath {
        // New variant
        path: String, // Store path as String for simplicity in SynParserError
        existing_id: AnyNodeId,
        conflicting_id: AnyNodeId,
    },

    /// A duplicate module ID was encountered when building the `ModuleTree`.
    #[error("Duplicate module ID found in module tree for ModuleNode: {0}")]
    ModuleTreeDuplicateModuleId(String), // Store Debug representation

    /// A relation was not found in the `ModuleTree` during resolution.
    #[error("Relation not found in ModuleTree during resolution: {0}\nNode with no relations found: {1}")]
    ModuletreeRelationNotFound(AnyNodeId, String),
    // Removed ModuleKindinitionNotFound - covered by ModuleTreeError::FoundUnlinkedModules
    // #[error("Module definition not found for path: {0}")]
    // ModuleKindinitionNotFound(String), // Store path string representation
    /// An error occurred during relation conversion.
    #[error("Relation conversion error: {0}")]
    RelationConversion(#[from] crate::parser::relations::RelationConversionError),

    // Forward ModuleTreeError variants - REMOVED #[from]
    /// An error occurred in the `ModuleTree`.
    #[error(transparent)]
    ModuleTreeError(ModuleTreeError),

    /// An error occurred during `TypeId` conversion.
    #[error("Relation conversion error: {0}")]
    TypeIdConversionError(TypeId), // Consider renaming if it's not just TypeId

    /// Shortest public path resolution failed for an external item.
    #[error("Shortest public path resolution failed for external item: {0}")]
    ExternalItemNotResolved(AnyNodeId),

    /// An error occurred in the `CodeVisitor`.
    #[error(transparent)]
    VisitorError(#[from] CodeVisitorError), // Add conversion from CodeVisitorError

    /// An error occurred during `AnyNodeId` conversion.
    #[error(transparent)]
    AnyNodeIdConversion(#[from] crate::parser::nodes::AnyNodeIdConversionError),

    /// An error occurred during `TryFromPrimary` conversion.
    #[error(transparent)]
    TryFromPrimaryError(#[from] TryFromPrimaryError),
}

/// An error that can occur during resolution.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum ResolutionError {
    /// The item is private and cannot be accessed.
    #[error("Private item: {0:?}")]
    PrivateItem(Vec<String>),

    /// The path is ambiguous and could refer to multiple items.
    #[error("Ambiguous path. Candidates: {0:?}")]
    AmbiguousPath(Vec<Vec<String>>),

    /// The item was not found.
    #[error("Not found: {0:?}")]
    NotFound(Vec<String>),
}

// Optional: Implement From<std::io::Error> for SynParserError
impl From<std::io::Error> for SynParserError {
    fn from(err: std::io::Error) -> Self {
        SynParserError::Io(err.to_string())
    }
}

// Convert ModuleTreeError to ploke_error::Error
impl From<ModuleTreeError> for ploke_error::Error {
    fn from(err: ModuleTreeError) -> Self {
        match err {
            ModuleTreeError::DuplicatePath {
                path,
                existing_id,
                conflicting_id,
            } => ploke_error::FatalError::DuplicateModulePath {
                path: path.into_vec(),
                existing_id: existing_id.to_string(),
                conflicting_id: conflicting_id.to_string(),
            }
            .into(),
            ModuleTreeError::FoundUnlinkedModules(unlinked_infos) => {
                ploke_error::WarningError::UnlinkedModules {
                    modules: unlinked_infos
                        .into_iter()
                        .map(|info| info.to_string())
                        .collect(),
                    // backtrace: Backtrace::capture(), // requires nightly
                }
                .into()
            }
            _ => ploke_error::Error::Internal(ploke_error::InternalError::CompilerError(
                format!("Unhandled ModuleTreeError: {}", err),
                // Backtrace::capture(), // requires nightly
            )),
        }
    }
}

// NOTE: This should be expanded when I'm ready to refactor error handling more broadly.
impl From<SynParserError> for ploke_error::Error {
    fn from(err: SynParserError) -> Self {
        #[allow(clippy::match_single_binding)]
        match err {
            _ => ploke_error::Error::Internal(ploke_error::InternalError::NotImplemented(
                err.to_string(),
            )),
        }
    }
}

// Implement From<ModuleTreeError> for SynParserError
impl From<ModuleTreeError> for SynParserError {
    fn from(err: ModuleTreeError) -> Self {
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
            ModuleTreeError::DuplicateDefinition(msg) => {
                SynParserError::InternalState(format!("ModuleTree processing error: {}", msg))
            }
            ModuleTreeError::ModuleKindinitionNotFound(msg) => {
                SynParserError::InternalState(format!("ModuleTree processing error: {}", msg))
            }
            ModuleTreeError::ExternalItemNotResolved(id) => {
                SynParserError::ExternalItemNotResolved(id)
            }
            ModuleTreeError::NoRelationsFound(id, msg) => {
                SynParserError::ModuletreeRelationNotFound(id, msg)
            }
            ModuleTreeError::UnresolvedReExportTarget {
                import_node_id,
                path,
            } => SynParserError::ModuleTreeError(ModuleTreeError::UnresolvedReExportTarget {
                import_node_id,
                path,
            }),
            ModuleTreeError::InvalidStatePendingExportsMissing { module_id } => {
                SynParserError::InternalState(format!(
                    "Invalid internal state: pending_exports was None when adding module {}",
                    module_id
                ))
            }
            ModuleTreeError::InternalState(msg) => SynParserError::InternalState(msg),
            ModuleTreeError::Warning(msg) => {
                SynParserError::InternalState(format!("ModuleTree Warning: {}", msg))
            }
            ModuleTreeError::DuplicateContains(tree_relation) => {
                // Indicates an internal inconsistency in the graph structure.
                SynParserError::InternalState(format!(
                    "Duplicate Contains relation found: {:?}",
                    tree_relation
                ))
            }
            ModuleTreeError::NoRelationsFoundForId(node_id) => {
                // Indicates an item expected to have relations (like containment) was found without any.
                SynParserError::InternalState(format!(
                    "No relations found for AnyNodeId {} during ModuleTree processing.",
                    node_id
                ))
            }
            ModuleTreeError::RecursionLimitExceeded {
                start_node_id,
                limit,
            } => {
                // Indicates a safety limit was hit, likely due to cycles or extreme depth.
                SynParserError::InternalState(format!(
                    "Recursion limit ({}) exceeded starting from node {}",
                    limit, start_node_id
                ))
            }
            ModuleTreeError::RootModuleNotFound(_module_node_id) => todo!(),
            ModuleTreeError::TypedIdConversionError(_try_from_primary_error) => todo!(),
            ModuleTreeError::AnyNodeIdConversionError(_any_node_id_conversion_error) => todo!(),
        }
    }
}

// Implement From<NodeError> for SynParserError
impl From<NodeError> for SynParserError {
    fn from(err: crate::parser::nodes::NodeError) -> Self {
        match err {
            NodeError::Validation(msg) => SynParserError::NodeValidation(msg),
            NodeError::Conversion(type_id) => SynParserError::TypeIdConversionError(type_id), // Keep existing
        }
    }
}

// Optional: Implement From<syn::Error> for SynParserError
impl From<syn::Error> for SynParserError {
    fn from(err: syn::Error) -> Self {                 // Print immediately so you see it even if the caller swallows it                                                  
        eprintln!("   syn::Error: {}", err);
        eprintln!("   span: {}", err.to_compile_error());
        SynParserError::Syn(err.to_string())
    }
}
