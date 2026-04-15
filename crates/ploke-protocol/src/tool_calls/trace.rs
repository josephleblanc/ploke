use serde::{Deserialize, Serialize};

/// Flattened tool-call trace for one reviewed run.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    pub subject_id: String,
    pub calls: Vec<Call>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Call {
    pub index: usize,
    pub turn: u32,
    pub tool_name: String,
    pub summary: String,
    pub failed: bool,
}
