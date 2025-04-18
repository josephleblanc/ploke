use super::{GraphNode, NodeId, TypeId, TrackingHash, VisibilityKind};
use crate::parser::types::GenericParamNode;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct FunctionNode {
    // ... existing fields
}

impl GraphNode for FunctionNode {
    // ... existing implementation
}

impl FunctionNode {
    /// Creates a new builder instance
    pub fn builder(name: impl Into<String>) -> FunctionNodeBuilder {
        FunctionNodeBuilder::new(name)
    }

    /// Validates the function node structure
    pub fn validate(&self) -> Result<(), super::NodeError> {
        // ... validation logic
    }
}

/// Builder pattern for FunctionNode
#[derive(Default)]
pub struct FunctionNodeBuilder {
    // ... builder fields
}

impl FunctionNodeBuilder {
    // ... builder methods
}
