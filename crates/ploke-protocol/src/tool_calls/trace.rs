use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolKind {
    Search,
    Read,
    Browse,
    Edit,
    Execute,
    Other,
}

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeighborhoodRequest {
    pub focal_index: usize,
    pub radius_before: usize,
    pub radius_after: usize,
}

impl NeighborhoodRequest {
    pub fn centered(focal_index: usize) -> Self {
        Self {
            focal_index,
            radius_before: 2,
            radius_after: 2,
        }
    }
}

pub trait NeighborhoodSource {
    type Error;

    fn neighborhood(
        &self,
        request: &NeighborhoodRequest,
    ) -> Result<ToolCallNeighborhood, Self::Error>;
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TurnContext {
    pub turn: u32,
    pub tool_count: usize,
    pub failed_tool_count: usize,
    pub patch_proposed: bool,
    pub patch_applied: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NeighborhoodCall {
    pub index: usize,
    pub turn: u32,
    pub tool_name: String,
    pub tool_kind: ToolKind,
    pub failed: bool,
    pub latency_ms: u64,
    pub summary: String,
    pub args_preview: String,
    pub result_preview: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub search_term: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub path_hint: Option<String>,
}

impl NeighborhoodCall {
    pub fn label(&self) -> String {
        format!("[{}] {} {}", self.index, self.tool_name, self.summary)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallNeighborhood {
    pub subject_id: String,
    pub total_calls_in_run: usize,
    pub total_calls_in_turn: usize,
    pub turn: TurnContext,
    pub before: Vec<NeighborhoodCall>,
    pub focal: NeighborhoodCall,
    pub after: Vec<NeighborhoodCall>,
}

impl ToolCallNeighborhood {
    pub fn all_calls(&self) -> impl Iterator<Item = &NeighborhoodCall> {
        self.before
            .iter()
            .chain(std::iter::once(&self.focal))
            .chain(self.after.iter())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCallSequence {
    pub subject_id: String,
    pub total_turns: usize,
    pub total_calls_in_run: usize,
    pub turns: Vec<TurnContext>,
    pub calls: Vec<NeighborhoodCall>,
}

impl ToolCallSequence {
    pub fn is_empty(&self) -> bool {
        self.calls.is_empty()
    }
}
