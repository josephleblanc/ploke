use super::{NodeId, TypeId, TrackingHash};
use serde::{Deserialize, Serialize};

/// Core trait for all graph nodes
pub trait GraphNode {
    fn id(&self) -> NodeId;
    fn visibility(&self) -> VisibilityKind;
    fn name(&self) -> &str;
    fn cfgs(&self) -> &[String];
}

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub enum VisibilityKind {
    // ... existing variants
}

// Shared error types
#[derive(Debug, thiserror::Error)]
pub enum NodeError {
    #[error("Invalid node configuration: {0}")]
    Validation(String),
    // ... others
}

// Re-export all node types from submodules
pub use function::FunctionNode;
pub use module::{ModuleNode, ModuleDef};
// ... other re-exports
