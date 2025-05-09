use crate::parser::graph::GraphAccess;
use crate::parser::types::GenericParamNode; // Removed define_node_info_struct import
use derive_test_helpers::ExpectedData;
use ploke_core::{TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
// removed GenerateNodeInfo

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Method Node ---

// Removed the macro invocation for MethodNodeInfo

/// Represents an associated function or method within an `impl` or `trait`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Add derive
pub struct MethodNode {
    pub id: MethodNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub parameters: Vec<ParamData>,
    pub return_type: Option<TypeId>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}

impl MethodNode {
    /// Returns the typed ID for this method node.
    pub fn method_id(&self) -> MethodNodeId {
        self.id
    }
}

impl GraphNode for MethodNode {
    // Renamed from FunctionNode
    fn any_id(&self) -> AnyNodeId {
        self.id.into() // Return base NodeId
    }
    fn visibility(&self) -> &VisibilityKind {
        &self.visibility
    }

    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }

    fn as_method(&self) -> Option<&MethodNode> {
        // Changed from as_function
        Some(self)
    }
}

impl HasAttributes for MethodNode {
    // Renamed from FunctionNode
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

impl MethodNode {
    // Renamed from FunctionNode
    /// Validates the method node structure
    pub fn validate(&self) -> Result<(), super::NodeError> {
        // TODO: Implement validation logic if needed
        Ok(())
        // ... validation logic
    }
}

// --- Function Node ---

// Removed the macro invocation for FunctionNodeInfo

/// Represents a standalone function item (`fn`).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ExpectedData)] // Add derive
pub struct FunctionNode {
    pub id: FunctionNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub parameters: Vec<ParamData>,
    pub return_type: Option<TypeId>,
    pub generic_params: Vec<GenericParamNode>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}

impl FunctionNode {
    /// Returns the typed ID for this function node.
    pub fn function_id(&self) -> FunctionNodeId {
        self.id
    }
}

impl GraphNode for FunctionNode {
    fn any_id(&self) -> AnyNodeId {
        self.id.into() // Return base NodeId
    }
    fn visibility(&self) -> &VisibilityKind {
        &self.visibility
    }
    fn name(&self) -> &str {
        &self.name
    }
    fn cfgs(&self) -> &[String] {
        &self.cfgs
    }
    fn as_function(&self) -> Option<&FunctionNode> {
        Some(self)
    }
}

impl HasAttributes for FunctionNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

impl FunctionNode {
    /// Validates the function node structure
    pub fn validate(&self) -> Result<(), super::NodeError> {
        // TODO: Implement validation logic if needed
        Ok(())
        // ... validation logic
    }
}

// Represents a parameter in a function
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct ParamData {
    pub name: Option<String>,
    pub type_id: TypeId, // The ID of the parameter's type
    pub is_mutable: bool,
    pub is_self: bool,
}
