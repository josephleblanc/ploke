use ploke_records::agent_turn::{AgentTurnArtifactRecord, ObservedTurnEventRecord};

use crate::AgentTurnRecordSet;

/// Source artifact family for event-level agent-turn playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TurnArtifactKind {
    Trace,
    Summary,
}

/// Stable event class for event-level agent-turn playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum TurnEventKind {
    DebugCommand,
    LlmEvent,
    LlmResponse,
    ToolRequested,
    ToolCompleted,
    ToolFailed,
    MessageUpdated,
    TurnFinished,
}

impl TurnEventKind {
    pub fn from_event(event: &ObservedTurnEventRecord) -> Self {
        match event {
            ObservedTurnEventRecord::DebugCommand(_) => Self::DebugCommand,
            ObservedTurnEventRecord::LlmEvent(_) => Self::LlmEvent,
            ObservedTurnEventRecord::LlmResponse(_) => Self::LlmResponse,
            ObservedTurnEventRecord::ToolRequested(_) => Self::ToolRequested,
            ObservedTurnEventRecord::ToolCompleted(_) => Self::ToolCompleted,
            ObservedTurnEventRecord::ToolFailed(_) => Self::ToolFailed,
            ObservedTurnEventRecord::MessageUpdated(_) => Self::MessageUpdated,
            ObservedTurnEventRecord::TurnFinished(_) => Self::TurnFinished,
        }
    }
}

/// Borrowed event-level step inside one persisted agent-turn artifact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnEventStepRef<'a> {
    pub artifact_kind: TurnArtifactKind,
    pub artifact_path: &'a str,
    pub task_id: &'a str,
    pub selected_model: &'a str,
    pub event_index: usize,
    pub event: &'a ObservedTurnEventRecord,
}

impl<'a> TurnEventStepRef<'a> {
    pub fn kind(&self) -> TurnEventKind {
        TurnEventKind::from_event(self.event)
    }

    pub fn tool_name(&self) -> Option<&'a str> {
        match self.event {
            ObservedTurnEventRecord::ToolRequested(record) => Some(record.tool.as_str()),
            ObservedTurnEventRecord::ToolCompleted(record) => Some(record.tool.as_str()),
            ObservedTurnEventRecord::ToolFailed(record) => record.tool.as_deref(),
            _ => None,
        }
    }

    pub fn call_id(&self) -> Option<&'a str> {
        match self.event {
            ObservedTurnEventRecord::ToolRequested(record) => Some(record.call_id.as_str()),
            ObservedTurnEventRecord::ToolCompleted(record) => Some(record.call_id.as_str()),
            ObservedTurnEventRecord::ToolFailed(record) => Some(record.call_id.as_str()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct TurnEventPlaybackRefSteps<'a> {
    steps: Vec<TurnEventStepRef<'a>>,
}

impl<'a> TurnEventPlaybackRefSteps<'a> {
    pub fn new(steps: Vec<TurnEventStepRef<'a>>) -> Self {
        Self { steps }
    }

    pub fn iter(&self) -> std::slice::Iter<'_, TurnEventStepRef<'a>> {
        self.steps.iter()
    }

    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn len(&self) -> usize {
        self.steps.len()
    }
}

impl<'a> IntoIterator for TurnEventPlaybackRefSteps<'a> {
    type Item = TurnEventStepRef<'a>;
    type IntoIter = std::vec::IntoIter<TurnEventStepRef<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.into_iter()
    }
}

impl<'a, 'b> IntoIterator for &'b TurnEventPlaybackRefSteps<'a> {
    type Item = &'b TurnEventStepRef<'a>;
    type IntoIter = std::slice::Iter<'b, TurnEventStepRef<'a>>;

    fn into_iter(self) -> Self::IntoIter {
        self.steps.iter()
    }
}

pub fn turn_event_steps_from_artifact<'a>(
    artifact_kind: TurnArtifactKind,
    artifact_path: &'a str,
    record: &'a AgentTurnArtifactRecord,
) -> TurnEventPlaybackRefSteps<'a> {
    TurnEventPlaybackRefSteps::new(
        record
            .events
            .iter()
            .enumerate()
            .map(|(event_index, event)| TurnEventStepRef {
                artifact_kind,
                artifact_path,
                task_id: record.task_id.as_str(),
                selected_model: record.selected_model.as_str(),
                event_index,
                event,
            })
            .collect(),
    )
}

/// Build event-level steps from loaded agent-turn records.
///
/// Ordering across artifacts is deterministic by artifact family and path. The
/// causal ordering this exposes is the event order inside each artifact.
pub fn turn_event_steps_from_agent_turn_records<'a>(
    records: &'a AgentTurnRecordSet,
) -> TurnEventPlaybackRefSteps<'a> {
    let mut steps = Vec::new();
    for (path, record) in &records.traces {
        steps.extend(turn_event_steps_from_artifact(
            TurnArtifactKind::Trace,
            path,
            record,
        ));
    }
    for (path, record) in &records.summaries {
        steps.extend(turn_event_steps_from_artifact(
            TurnArtifactKind::Summary,
            path,
            record,
        ));
    }
    TurnEventPlaybackRefSteps::new(steps)
}
