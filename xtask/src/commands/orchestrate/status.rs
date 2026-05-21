use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::commands::{CommandContext, XtaskError};

use super::{Board, TaskState, WorkerRole, display, resolve};

/// Bounded routine board status.
#[derive(Debug, Clone, Serialize)]
pub struct BoundedStatus {
    /// Board path.
    pub(super) board: String,
    /// Packet directory.
    pub(super) packet_dir: String,
    /// Task counts by lifecycle state.
    pub(super) tasks_by_state: Vec<TaskStateCount>,
    /// Worker slot summary.
    pub(super) workers: Vec<WorkerStatus>,
    /// Blocker count.
    pub(super) blockers: usize,
    /// Lane ownership summary.
    pub(super) lanes: Vec<LaneStatus>,
}

/// Task count for one lifecycle state.
#[derive(Debug, Clone, Serialize)]
pub struct TaskStateCount {
    /// Task lifecycle state.
    pub(super) state: String,
    /// Number of tasks in this state.
    pub(super) count: usize,
}

/// Bounded worker status.
#[derive(Debug, Clone, Serialize)]
pub struct WorkerStatus {
    /// Worker id.
    pub(super) id: String,
    /// Worker role.
    pub(super) role: WorkerRole,
    /// Active task id.
    pub(super) active: Option<String>,
    /// Queued task count.
    pub(super) queue_len: usize,
    /// Last packet path.
    pub(super) packet_path: Option<String>,
}

/// Bounded lane status.
#[derive(Debug, Clone, Serialize)]
pub struct LaneStatus {
    /// Lane id.
    pub(super) id: String,
    /// Owned edit surfaces.
    pub(super) owned_edit: Vec<String>,
    /// Lane doc count.
    pub(super) docs: usize,
    /// Lane note count.
    pub(super) notes: usize,
}

impl Board {
    pub(super) fn bounded_status(
        &self,
        ctx: &CommandContext,
        board_path: &Path,
    ) -> Result<BoundedStatus, XtaskError> {
        let mut counts = BTreeMap::new();
        for task in self.tasks.values() {
            *counts.entry(task.state.label()).or_insert(0) += 1;
        }
        let tasks_by_state = counts
            .into_iter()
            .map(|(state, count)| TaskStateCount {
                state: state.to_string(),
                count,
            })
            .collect();

        let mut workers: Vec<_> = self
            .workers
            .values()
            .map(|worker| WorkerStatus {
                id: worker.id.clone(),
                role: worker.role,
                active: worker.active.clone(),
                queue_len: worker.queue.len(),
                packet_path: worker.packet_path.clone(),
            })
            .collect();
        workers.sort_by(|a, b| a.id.cmp(&b.id));

        let mut lanes: Vec<_> = self
            .lanes
            .values()
            .map(|lane| LaneStatus {
                id: lane.id.clone(),
                owned_edit: lane.owned_edit.clone(),
                docs: lane.docs.len(),
                notes: lane.notes.len(),
            })
            .collect();
        lanes.sort_by(|a, b| a.id.cmp(&b.id));

        Ok(BoundedStatus {
            board: display(ctx, board_path)?,
            packet_dir: display(ctx, &resolve(ctx, &self.packet_dir)?)?,
            tasks_by_state,
            workers,
            blockers: self.blockers.len(),
            lanes,
        })
    }
}

impl TaskState {
    fn label(&self) -> &'static str {
        match self {
            Self::NotStarted => "not_started",
            Self::AssignedQueued { .. } => "assigned_queued",
            Self::AssignedActive { .. } => "assigned_active",
            Self::CompleteUnreviewed { .. } => "complete_unreviewed",
            Self::CompleteReviewed => "complete_reviewed",
            Self::Blocked { .. } => "blocked",
        }
    }
}
