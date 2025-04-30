use crate::define_node_info_struct; // Import macro
use ploke_core::{NodeId, TrackingHash}; // Import NodeId
use serde::{Deserialize, Serialize};

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Macro Node ---

define_node_info_struct! {
    /// Temporary info struct for creating a MacroNode.
    MacroNodeInfo {
        name: String,
        span: (usize, usize),
        visibility: VisibilityKind,
        kind: MacroKind,
        attributes: Vec<Attribute>,
        docstring: Option<String>,
        body: Option<String>,
        tracking_hash: Option<TrackingHash>,
        cfgs: Vec<String>,
    }
}

// Represents a macro definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MacroNode {
    pub id: MacroNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize),
    pub visibility: VisibilityKind,
    pub kind: MacroKind,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>,
}

impl MacroNode {
    /// Returns the typed ID for this macro node.
    pub fn macro_id(&self) -> MacroNodeId {
        self.id
    }

    /// Creates a new `MacroNode` from `MacroNodeInfo`.
    pub(crate) fn new(info: MacroNodeInfo) -> Self {
        Self {
            id: MacroNodeId(info.id), // Wrap the raw ID here
            name: info.name,
            span: info.span,
            visibility: info.visibility,
            kind: info.kind,
            attributes: info.attributes,
            docstring: info.docstring,
            body: info.body,
            tracking_hash: info.tracking_hash,
            cfgs: info.cfgs,
        }
    }
}

impl GraphNode for MacroNode {
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
        &self.cfgs // Simply return a slice reference to the stored cfgs
    }

    fn as_macro(&self) -> Option<&MacroNode> {
        Some(self)
    }
}

impl HasAttributes for MacroNode {
    fn attributes(&self) -> &[Attribute] {
        &self.attributes
    }
}

// Represents a macro rule (Note: Currently unused in MacroNode, consider removal if not needed)
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MacroRuleNode {
    pub id: NodeId,
    pub pattern: String,
    pub expansion: String,
}

// Different kinds of macros
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum MacroKind {
    DeclarativeMacro,
    ProcedureMacro { kind: ProcMacroKind },
}

// Different kinds of procedural macros
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub enum ProcMacroKind {
    Derive,
    Attribute,
    Function,
}
