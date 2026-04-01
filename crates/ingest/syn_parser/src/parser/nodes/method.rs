#![allow(unused_must_use, unused_imports)]
// Proc-macro `ExpectedData` expands `use` items at module scope; keep this file separate from
// `function.rs` (which also derives `ExpectedData`) to avoid duplicate imports.

use crate::parser::types::GenericParamNode;
use derive_test_helpers::ExpectedData;
use ploke_core::{TrackingHash, TypeId};
use serde::{Deserialize, Serialize};

use super::*;

/// Represents an associated function or method within an `impl` or `trait`.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ExpectedData)]
pub struct MethodNode {
    pub id: MethodNodeId,
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
    pub fn validate(&self) -> Result<(), super::NodeError> {
        Ok(())
    }
}

impl GraphNode for MethodNode {
    fn any_id(&self) -> AnyNodeId {
        self.id.into()
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
        Some(self)
    }
}

impl HasAttributes for MethodNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
