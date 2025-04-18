use ploke_core::NodeId;
use thiserror::Error;

/// Custom error type for the syn_parser crate.
#[derive(Error, Debug, Clone, PartialEq, Eq)]
pub enum SynParserError {
    /// Indicates that a requested node was not found in the graph.
    #[error("Node with ID {0} not found in the graph.")]
    NotFound(NodeId),

    /// Indicates that multiple nodes were found when exactly one was expected.
    #[error("Duplicate node found for ID {0} when only one was expected.")]
    DuplicateNode(NodeId),

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
}

// Optional: Implement From<std::io::Error> for SynParserError
impl From<std::io::Error> for SynParserError {
    fn from(err: std::io::Error) -> Self {
        SynParserError::Io(err.to_string())
    }
}

// Optional: Implement From<syn::Error> for SynParserError
impl From<syn::Error> for SynParserError {
    fn from(err: syn::Error) -> Self {
        SynParserError::Syn(err.to_string())
    }
}
