use ploke_core::{NodeId, TrackingHash};

use super::*;

// Represents a macro definition
impl MacroNode {
    /// Returns the typed ID for this macro node.
    pub fn macro_id(&self) -> MacroNodeId {
        self.id
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
#[derive(Debug, Serialize, Deserialize, Clone, PartialEq)]
pub struct MacroNode {
    pub id: MacroNodeId, // Use typed ID
    pub name: String,
    pub span: (usize, usize), // Add span field
    pub visibility: VisibilityKind,
    pub kind: MacroKind,
    pub attributes: Vec<Attribute>,
    pub docstring: Option<String>,
    pub body: Option<String>,
    pub tracking_hash: Option<TrackingHash>,
    pub cfgs: Vec<String>, // NEW: Store raw CFG strings for this item
}

// Represents a macro rule
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
