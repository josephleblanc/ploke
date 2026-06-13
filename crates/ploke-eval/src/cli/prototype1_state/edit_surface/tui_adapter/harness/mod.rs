//! Trait boundary between `ploke-eval` policy and the headless `ploke-tui` executor.

pub(crate) mod fixture;
pub(crate) mod timeouts;
mod tui;

pub(crate) use timeouts::Timeouts;
pub(crate) use tui::TuiHarness;

use std::{path::PathBuf, time::Instant};

use uuid::Uuid;

use super::Error;
use super::harness_io::{HeadlessRun, PromptDiagnostic};

/// Opening parameters for one headless edit attempt.
#[derive(Debug, Clone)]
pub(crate) struct SessionSpec {
    pub workspace_path: PathBuf,
    pub timeouts: Timeouts,
}

/// Cursor over the headless turn event stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Progress {
    Prompt(PromptInfo),
    Tool(ToolTrace),
    PendingEdit(Batch),
    TurnEnded(TurnStop),
    ProviderUnavailable(String),
    ContextUnavailable(String),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PromptInfo {
    pub diagnostic: PromptDiagnostic,
}

impl PromptInfo {
    pub(crate) fn context_unavailable(&self) -> Option<String> {
        self.diagnostic.context_unavailable_reason()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ToolTrace {
    pub call_id: String,
    pub tool: String,
    pub completed: bool,
    pub preview: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Batch {
    pub request_id: Uuid,
    pub staged: Vec<Staged>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StagedKind {
    Edit,
    Create,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) struct Staged {
    pub id: Uuid,
    pub kind: StagedKind,
    pub paths: Vec<PathBuf>,
    pub proposed_at_ms: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DenyItem {
    pub item: Staged,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Decision {
    pub request_id: Uuid,
    pub approve: Vec<Staged>,
    pub deny: Vec<DenyItem>,
}

impl Decision {
    pub(crate) fn approved_any(&self) -> bool {
        !self.approve.is_empty()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Settled {
    pub applied: Vec<Uuid>,
    pub changed_paths: Vec<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TurnStop {
    pub outcome: String,
    pub summary: String,
    pub attempts: u32,
    pub request_id: Uuid,
}

pub(crate) trait Harness {
    async fn next(&mut self, deadline: Instant) -> Result<Progress, Error>;

    /// Resolve a pending edit batch. Returns whether this batch newly applied an
    /// edit, so callers can gate per-batch settle/validation work.
    async fn decide(&mut self, decision: Decision) -> Result<bool, Error>;

    async fn settle(&mut self, deadline: Instant) -> Result<Settled, Error>;

    fn run(&self) -> &HeadlessRun;

    fn run_mut(&mut self) -> &mut HeadlessRun;
}
