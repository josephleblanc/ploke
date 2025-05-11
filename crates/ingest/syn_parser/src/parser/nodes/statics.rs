#![allow(unused_must_use)]
// Needed to get rid of proc-macro induced warning for `ExpectedData`
use derive_test_helpers::ExpectedData;
use ploke_core::{TrackingHash, TypeId};
use serde::{Deserialize, Serialize};

use super::*;

// Removed the macro invocation for StaticNodeInfo

/// Represents a `static` item.
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq, ExpectedData)] // Add derive
pub struct StaticNode {
    pub id: StaticNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub type_id: TypeId,
    pub is_mutable: bool,
    pub value: Option<String>,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}

impl StaticNode {
    /// Returns the typed ID for this static node.
    pub fn static_id(&self) -> StaticNodeId {
        self.id
    }

    pub fn span(&self) -> (usize, usize) {
        self.span
    }

    pub fn visibility(&self) -> &VisibilityKind {
        &self.visibility
    }

    pub fn type_id(&self) -> TypeId {
        self.type_id
    }

    pub fn is_mutable(&self) -> bool {
        self.is_mutable
    }

    pub fn value(&self) -> Option<&String> {
        self.value.as_ref()
    }

    pub fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }

    pub fn docstring(&self) -> Option<&String> {
        self.docstring.as_ref()
    }

    pub fn tracking_hash(&self) -> Option<TrackingHash> {
        self.tracking_hash
    }

    pub fn cfgs(&self) -> &[String] {
        &self.cfgs
    }
}

impl GraphNode for StaticNode {
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
    fn as_static(&self) -> Option<&StaticNode> {
        // Changed from as_value_static
        Some(self)
    }
}

impl HasAttributes for StaticNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}
