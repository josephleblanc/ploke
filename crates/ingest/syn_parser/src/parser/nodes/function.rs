use super::*;
use crate::parser::types::GenericParamNode;
use ploke_core::{TrackingHash, TypeId};
use serde::{Deserialize, Serialize};

/// Represents an associated function or method within an `impl` or `trait`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MethodNode { // Renamed from FunctionNode
    pub id: NodeId,
    pub name: String,
    pub span: (usize, usize), // Byte start/end offsets
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

impl GraphNode for MethodNode { // Renamed from FunctionNode
    fn id(&self) -> NodeId {
        self.id
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

    fn as_method(&self) -> Option<&MethodNode> { // Changed from as_function
        Some(self)
    }
}

impl HasAttributes for MethodNode { // Renamed from FunctionNode
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

impl MethodNode { // Renamed from FunctionNode
    /// Validates the method node structure
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
