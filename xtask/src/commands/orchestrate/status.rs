use std::collections::BTreeMap;
use std::path::Path;

use chrono::{DateTime, Utc};
use serde::Serialize;

use crate::commands::{CommandContext, XtaskError};

use super::board::{ACTIVE_STALE_AFTER_SECS, REVIEW_STALE_AFTER_SECS, parse_time};
use super::views::TaskQuery;
use super::{Board, TaskState, WorkerRole, display, resolve};

/// Bounded routine board status.
#[derive(Debug, Clone, Serialize)]
pub struct BoundedStatus {
    /// Board path.
    pub(super) board: String,
    /// Applied task-set filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) task_set: Option<String>,
    /// Applied built-in view.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) view: Option<String>,
    /// Applied filter expressions.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) filters: Vec<String>,
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
    /// Warning-only stale state signals.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(super) warnings: Vec<StatusWarning>,
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
    /// Last packet generation time.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) packet_generated_at: Option<String>,
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

/// Warning emitted by bounded status.
#[derive(Debug, Clone, Serialize)]
pub struct StatusWarning {
    /// Machine-readable warning kind.
    pub(super) kind: String,
    /// Task or worker id the warning applies to.
    pub(super) subject: String,
    /// Human-readable warning summary.
    pub(super) message: String,
    /// Age in seconds when applicable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) age_seconds: Option<i64>,
}

impl Board {
    pub(super) fn bounded_status(
        &self,
        ctx: &CommandContext,
        board_path: &Path,
        query: &TaskQuery,
    ) -> Result<BoundedStatus, XtaskError> {
        let task_filter = self.task_filter(query)?;
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
                    packet_generated_at: worker.packet_generated_at.clone(),
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
            .filter(|lane| !query.has_filters() || task_lanes.contains(lane.id.as_str()))
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
            task_set: query.task_set().map(str::to_string),
            view: query.view_label().map(str::to_string),
            filters: query.filter_labels().to_vec(),
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
            warnings: self.status_warnings(task_filter.as_ref()),
        })
    }

    fn status_warnings(
        &self,
        task_filter: Option<&std::collections::BTreeSet<String>>,
    ) -> Vec<StatusWarning> {
        let now = Utc::now();
        let mut warnings = Vec::new();

        for task in self
            .tasks
            .values()
            .filter(|task| task_filter.map_or(true, |filter| filter.contains(&task.id)))
        {
            match &task.state {
                TaskState::AssignedActive { .. } => {
                    let timestamp = task.activated_at.as_deref().unwrap_or(&task.updated_at);
                    if let Some(age) = age_seconds(timestamp, now) {
                        if age > ACTIVE_STALE_AFTER_SECS {
                            warnings.push(StatusWarning {
                                kind: "stale_active_task".to_string(),
                                subject: task.id.clone(),
                                message: format!("active task has been active for {age} seconds"),
                                age_seconds: Some(age),
                            });
                        }
                    }
                }
                TaskState::CompleteUnreviewed { .. } => {
                    let timestamp = task.completed_at.as_deref().unwrap_or(&task.updated_at);
                    if let Some(age) = age_seconds(timestamp, now) {
                        if age > REVIEW_STALE_AFTER_SECS {
                            warnings.push(StatusWarning {
                                kind: "stale_review_task".to_string(),
                                subject: task.id.clone(),
                                message: format!(
                                    "completed task has waited {age} seconds for review"
                                ),
                                age_seconds: Some(age),
                            });
                        }
                    }
                }
                _ => {}
            }
        }

        for worker in self.workers.values() {
            let assigned_tasks: Vec<_> = worker
                .active
                .iter()
                .chain(worker.queue.iter())
                .filter(|task_id| task_filter.map_or(true, |filter| filter.contains(*task_id)))
                .filter_map(|task_id| self.tasks.get(task_id))
                .collect();
            if assigned_tasks.is_empty() {
                continue;
            }
            let Some(packet_generated_at) = worker.packet_generated_at.as_deref() else {
                warnings.push(StatusWarning {
                    kind: "missing_worker_packet".to_string(),
                    subject: worker.id.clone(),
                    message: "worker has assigned tasks but no generated packet".to_string(),
                    age_seconds: None,
                });
                continue;
            };
            let Some(packet_time) = parse_time(packet_generated_at) else {
                continue;
            };
            let stale = assigned_tasks.iter().any(|task| {
                parse_time(&task.updated_at).is_some_and(|task_time| task_time > packet_time)
            });
            if stale {
                warnings.push(StatusWarning {
                    kind: "stale_worker_packet".to_string(),
                    subject: worker.id.clone(),
                    message: "worker packet is older than its assigned task state".to_string(),
                    age_seconds: age_seconds(packet_generated_at, now),
                });
            }
        }

        warnings.sort_by(|a, b| a.kind.cmp(&b.kind).then_with(|| a.subject.cmp(&b.subject)));
        warnings
    }
}

fn age_seconds(timestamp: &str, now: DateTime<Utc>) -> Option<i64> {
    parse_time(timestamp).map(|then| (now - then).num_seconds())
}
