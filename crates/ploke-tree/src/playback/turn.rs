use ploke_records::agent_turn::{AgentTurnArtifactRecord, ObservedTurnEventRecord};
use serde::{Deserialize, Serialize};

use crate::AgentTurnRecordSet;

/// Source artifact family for event-level agent-turn playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub enum TurnArtifactKind {
    Trace,
    Summary,
}

/// Stable event class for event-level agent-turn playback.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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

/// Stable position inside one persisted agent-turn artifact.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct TurnCursor {
    pub artifact_kind: TurnArtifactKind,
    pub artifact_path: String,
    pub event_index: usize,
}

/// Borrowed response-tape identifier for replay consumers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ResponseTapeRef<'a> {
    pub assistant_message_id: &'a str,
}

/// Borrowed event-level step inside one persisted agent-turn artifact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TurnEventStepRef<'a> {
    pub artifact_kind: TurnArtifactKind,
    pub artifact_path: &'a str,
    pub task_id: &'a str,
    pub selected_model: &'a str,
    pub artifact_assistant_message_id: Option<&'a str>,
    pub event_index: usize,
    pub event: &'a ObservedTurnEventRecord,
}

impl<'a> TurnEventStepRef<'a> {
    pub fn cursor(&self) -> TurnCursor {
        TurnCursor {
            artifact_kind: self.artifact_kind,
            artifact_path: self.artifact_path.to_owned(),
            event_index: self.event_index,
        }
    }

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

    pub fn event_assistant_message_id(&self) -> Option<&'a str> {
        match self.event {
            ObservedTurnEventRecord::TurnFinished(record) => {
                Some(record.assistant_message_id.as_str())
            }
            ObservedTurnEventRecord::MessageUpdated(record) if record.kind == "Assistant" => {
                Some(record.id.as_str())
            }
            _ => None,
        }
    }

    pub fn response_tape_ref(&self) -> Option<ResponseTapeRef<'a>> {
        self.event_assistant_message_id()
            .or(self.artifact_assistant_message_id)
            .map(|assistant_message_id| ResponseTapeRef {
                assistant_message_id,
            })
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
    let artifact_assistant_message_id = artifact_assistant_message_id(record);
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
                artifact_assistant_message_id,
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

pub fn turn_event_step_at<'a>(
    records: &'a AgentTurnRecordSet,
    cursor: &TurnCursor,
) -> Option<TurnEventStepRef<'a>> {
    let (artifact_path, record) = match cursor.artifact_kind {
        TurnArtifactKind::Trace => records
            .traces
            .get_key_value(cursor.artifact_path.as_str())?,
        TurnArtifactKind::Summary => records
            .summaries
            .get_key_value(cursor.artifact_path.as_str())?,
    };
    let event = record.events.get(cursor.event_index)?;
    Some(TurnEventStepRef {
        artifact_kind: cursor.artifact_kind,
        artifact_path: artifact_path.as_str(),
        task_id: record.task_id.as_str(),
        selected_model: record.selected_model.as_str(),
        artifact_assistant_message_id: artifact_assistant_message_id(record),
        event_index: cursor.event_index,
        event,
    })
}

fn artifact_assistant_message_id(record: &AgentTurnArtifactRecord) -> Option<&str> {
    record
        .terminal_record
        .as_ref()
        .map(|terminal| terminal.assistant_message_id.as_str())
        .or_else(|| {
            record
                .final_assistant_message
                .as_ref()
                .map(|message| message.id.as_str())
        })
}
