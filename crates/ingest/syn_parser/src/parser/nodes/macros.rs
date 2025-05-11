use ploke_core::TrackingHash;
use serde::{Deserialize, Serialize};
// removed GenerateNodeInfo

use super::*; // Keep for other node types, VisibilityKind etc.

// --- Macro Node ---

// Removed the macro invocation for MacroNodeInfo

// Represents a macro definition
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)] // Add derive
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
}

impl GraphNode for MacroNode {
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

// Removed MacroRuleNode for now (complex to implement)

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
