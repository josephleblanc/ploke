use crate::parser::types::GenericParamNode; // Removed define_node_info_struct import
use ploke_core::{NodeId, TrackingHash, TypeId};
use serde::{Deserialize, Serialize};
use syn_parser_macros::GenerateNodeInfo; // Import the derive macro

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Method Node ---

// Removed the macro invocation for MethodNodeInfo

/// Represents an associated function or method within an `impl` or `trait`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct MethodNode {
    pub id: MethodNodeId, // Use typed ID
    pub name: String,
        span: (usize, usize),
        visibility: VisibilityKind,
        parameters: Vec<ParamData>,
        return_type: Option<TypeId>,
        generic_params: Vec<GenericParamNode>,
        attributes: Vec<Attribute>,
        docstring: Option<String>,
        body: Option<String>,
        tracking_hash: Option<TrackingHash>,
        cfgs: Vec<String>,
    }
}

/// Represents an associated function or method within an `impl` or `trait`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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

    /// Creates a new `MethodNode` from `MethodNodeInfo`.
    /// This is the controlled entry point for creating a `MethodNode` with a typed ID.
    pub(crate) fn new(info: MethodNodeInfo) -> Self {
        Self {
            id: MethodNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            span: info.span,
            visibility: info.visibility,
            parameters: info.parameters,
            return_type: info.return_type,
            generic_params: info.generic_params,
            attributes: info.attributes,
            docstring: info.docstring,
            body: info.body,
            tracking_hash: info.tracking_hash,
            cfgs: info.cfgs,
        }
    }
}

impl GraphNode for MethodNode {
    // Renamed from FunctionNode
    fn id(&self) -> NodeId {
        self.id.into_inner() // Return base NodeId
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, GenerateNodeInfo)] // Add derive
pub struct FunctionNode {
    pub id: FunctionNodeId, // Use typed ID
    pub name: String,
        span: (usize, usize),
        visibility: VisibilityKind,
        parameters: Vec<ParamData>,
        return_type: Option<TypeId>,
        generic_params: Vec<GenericParamNode>,
        attributes: Vec<Attribute>,
        docstring: Option<String>,
        body: Option<String>,
        tracking_hash: Option<TrackingHash>,
        cfgs: Vec<String>,
    }
}

/// Represents a standalone function item (`fn`).
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
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

    /// Creates a new `FunctionNode` from `FunctionNodeInfo`.
    /// This is the controlled entry point for creating a `FunctionNode` with a typed ID.
    pub(crate) fn new(info: FunctionNodeInfo) -> Self {
        Self {
            id: FunctionNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            span: info.span,
            visibility: info.visibility,
            parameters: info.parameters,
            return_type: info.return_type,
            generic_params: info.generic_params,
            attributes: info.attributes,
            docstring: info.docstring,
            body: info.body,
            tracking_hash: info.tracking_hash,
            cfgs: info.cfgs,
        }
    }
}

impl GraphNode for FunctionNode {
    fn id(&self) -> NodeId {
        self.id.into_inner() // Return base NodeId
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
