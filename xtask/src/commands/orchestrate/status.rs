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
    /// Applied task-set filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) task_set: Option<String>,
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
    /// Active task id, even when it is outside the selected filter.
    pub(super) active: Option<String>,
    /// Whether the active task is included by the selected filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) active_in_filter: Option<bool>,
    /// Total queued task count, including tasks outside the selected filter.
    pub(super) queue_len: usize,
    /// Queued task count included by the selected filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) queue_in_filter: Option<usize>,
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
        task_set: Option<&str>,
    ) -> Result<BoundedStatus, XtaskError> {
        let task_filter = self.task_filter(task_set)?;
        let mut counts = BTreeMap::new();
        for task in self.tasks.values().filter(|task| {
            task_filter
                .as_ref()
                .map_or(true, |filter| filter.contains(&task.id))
        }) {
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
            .map(|worker| {
                let active_in_filter = task_filter.as_ref().map(|filter| {
                    worker
                        .active
                        .as_ref()
                        .is_some_and(|task_id| filter.contains(task_id))
                });
                let queue_in_filter = task_filter.as_ref().map(|filter| {
                    worker
                        .queue
                        .iter()
                        .filter(|task_id| filter.contains(*task_id))
                        .count()
                });
                WorkerStatus {
                    id: worker.id.clone(),
                    role: worker.role,
                    active: worker.active.clone(),
                    active_in_filter,
                    queue_len: worker.queue.len(),
                    queue_in_filter,
                    packet_path: worker.packet_path.clone(),
                }
            })
            .collect();
        workers.sort_by(|a, b| a.id.cmp(&b.id));

        let task_lanes: std::collections::BTreeSet<_> = self
            .tasks
            .values()
            .filter(|task| {
                task_filter
                    .as_ref()
                    .map_or(true, |filter| filter.contains(&task.id))
            })
            .map(|task| task.lane.as_str())
            .collect();
        let mut lanes: Vec<_> = self
            .lanes
            .values()
            .filter(|lane| task_set.is_none() || task_lanes.contains(lane.id.as_str()))
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
            task_set: task_set.map(str::to_string),
            packet_dir: display(ctx, &resolve(ctx, &self.packet_dir)?)?,
            tasks_by_state,
            workers,
            blockers: self
                .blockers
                .values()
                .filter(|blocker| {
                    blocker.is_open()
                        && task_filter
                            .as_ref()
                            .map_or(true, |filter| filter.contains(&blocker.task_id))
                })
                .count(),
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
