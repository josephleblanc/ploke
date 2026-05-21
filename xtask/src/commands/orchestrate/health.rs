use std::collections::BTreeMap;
use std::path::Path;

use serde::Serialize;

use crate::commands::{CommandContext, XtaskError};

use super::views::TaskQuery;
use super::{
    Board, BoardArg, BoardLock, LaneValidation, OrchestrateOutput, Task, TaskState, WorkerRole,
    display, resolve,
};

/// Check board integrity and warning-only health signals.
#[derive(Debug, Clone, clap::Args)]
pub struct Check {
    #[command(flatten)]
    pub(super) board: BoardArg,
}

/// Read-only board health projection.
#[derive(Debug, Clone, Serialize)]
pub struct BoardHealth {
    /// Board path.
    pub(super) board: String,
    /// True when no error-severity findings were found.
    pub(super) ok: bool,
    /// Lane validation details.
    pub(super) lane_validation: LaneValidation,
    /// Board health findings.
    pub(super) findings: Vec<HealthFinding>,
}

/// One board health finding.
#[derive(Debug, Clone, Serialize)]
pub struct HealthFinding {
    /// Finding severity.
    pub(super) severity: HealthSeverity,
    /// Machine-readable finding kind.
    pub(super) kind: String,
    /// Task, worker, blocker, or board id.
    pub(super) subject: String,
    /// Human-readable finding summary.
    pub(super) message: String,
}

/// Health finding severity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthSeverity {
    /// Warning-only signal.
    Warning,
    /// Board inconsistency.
    Error,
}

impl Check {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, self.board.path())?;
        let _lock = BoardLock::acquire(&path)?;
        let board = Board::load(&path)?;
        Ok(OrchestrateOutput::Health(board.health(ctx, &path)?))
    }
}

impl Board {
    pub(super) fn health(
        &self,
        ctx: &CommandContext,
        board_path: &Path,
    ) -> Result<BoardHealth, XtaskError> {
        let lane_validation = self.validate_lanes(ctx)?;
        let mut findings = Vec::new();
        if !lane_validation.is_ok() {
            push(
                &mut findings,
                HealthSeverity::Error,
                "lane_validation_failed",
                "board",
                "lane validation reported ownership or task/lane issues",
            );
        }

        self.push_worker_reference_findings(&mut findings);
        self.push_task_state_findings(&mut findings);
        self.push_blocker_findings(&mut findings);
        self.push_writer_surface_findings(&mut findings);
        self.push_staleness_findings(ctx, board_path, &mut findings)?;

        findings.sort_by(|a, b| {
            severity_rank(a.severity)
                .cmp(&severity_rank(b.severity))
                .then_with(|| a.kind.cmp(&b.kind))
                .then_with(|| a.subject.cmp(&b.subject))
        });
        let ok = findings
            .iter()
            .all(|finding| finding.severity != HealthSeverity::Error);

        Ok(BoardHealth {
            board: display(ctx, board_path)?,
            ok,
            lane_validation,
            findings,
        })
    }

    fn push_worker_reference_findings(&self, findings: &mut Vec<HealthFinding>) {
        let mut active_by_task: BTreeMap<&str, Vec<&str>> = BTreeMap::new();
        for worker in self.workers.values() {
            if let Some(active) = worker.active.as_deref() {
                if !self.tasks.contains_key(active) {
                    push(
                        findings,
                        HealthSeverity::Error,
                        "worker_active_missing_task",
                        &worker.id,
                        format!("worker active task `{active}` does not exist"),
                    );
                } else {
                    active_by_task.entry(active).or_default().push(&worker.id);
                }
            }
            for queued in &worker.queue {
                if !self.tasks.contains_key(queued) {
                    push(
                        findings,
                        HealthSeverity::Error,
                        "worker_queue_missing_task",
                        &worker.id,
                        format!("worker queued task `{queued}` does not exist"),
                    );
                }
            }
        }

        for (task, workers) in active_by_task {
            if workers.len() > 1 {
                push(
                    findings,
                    HealthSeverity::Error,
                    "task_active_in_multiple_workers",
                    task,
                    format!("task is active in workers {}", workers.join(", ")),
                );
            }
        }
    }

    fn push_task_state_findings(&self, findings: &mut Vec<HealthFinding>) {
        for task in self.tasks.values() {
            match &task.state {
                TaskState::AssignedQueued { worker } => match self.workers.get(worker) {
                    Some(slot) if slot.queue.iter().any(|queued| queued == &task.id) => {}
                    Some(_) => push(
                        findings,
                        HealthSeverity::Error,
                        "task_queue_missing_worker_backref",
                        &task.id,
                        format!("task is queued for `{worker}` but not in that worker queue"),
                    ),
                    None => push_missing_state_worker(findings, task, worker),
                },
                TaskState::AssignedActive { worker } => match self.workers.get(worker) {
                    Some(slot) if slot.active.as_deref() == Some(task.id.as_str()) => {}
                    Some(_) => push(
                        findings,
                        HealthSeverity::Error,
                        "task_active_missing_worker_backref",
                        &task.id,
                        format!("task is active for `{worker}` but not in that worker active slot"),
                    ),
                    None => push_missing_state_worker(findings, task, worker),
                },
                TaskState::Blocked { blocker } => {
                    let Some(blocker) = self.blockers.get(blocker) else {
                        push(
                            findings,
                            HealthSeverity::Error,
                            "blocked_task_missing_blocker",
                            &task.id,
                            "blocked task references a missing blocker",
                        );
                        continue;
                    };
                    if !blocker.is_open() {
                        push(
                            findings,
                            HealthSeverity::Error,
                            "blocked_task_resolved_blocker",
                            &task.id,
                            "blocked task references a resolved blocker",
                        );
                    }
                }
                _ => {
                    let open_blockers = self.open_blockers_for_task(&task.id);
                    if !open_blockers.is_empty() {
                        push(
                            findings,
                            HealthSeverity::Error,
                            "open_blocker_on_unblocked_task",
                            &task.id,
                            format!("task has open blockers: {}", open_blockers.join(", ")),
                        );
                    }
                }
            }
        }
    }

    fn push_blocker_findings(&self, findings: &mut Vec<HealthFinding>) {
        for blocker in self.blockers.values() {
            if !self.tasks.contains_key(&blocker.task_id) {
                push(
                    findings,
                    HealthSeverity::Error,
                    "blocker_missing_task",
                    &blocker.id,
                    format!("blocker task `{}` does not exist", blocker.task_id),
                );
            }
            if blocker.is_open() && blocker.proposed_unblock.is_none() {
                push(
                    findings,
                    HealthSeverity::Warning,
                    "blocker_missing_unblock_action",
                    &blocker.id,
                    "open blocker has no proposed unblock action",
                );
            }
        }

        for task in self.tasks.values() {
            for blocker_id in &task.blockers {
                match self.blockers.get(blocker_id) {
                    Some(blocker) if blocker.is_open() => {}
                    Some(_) => push(
                        findings,
                        HealthSeverity::Warning,
                        "task_attaches_resolved_blocker",
                        &task.id,
                        format!("task still attaches resolved blocker `{blocker_id}`"),
                    ),
                    None => push(
                        findings,
                        HealthSeverity::Error,
                        "task_attaches_missing_blocker",
                        &task.id,
                        format!("task attaches missing blocker `{blocker_id}`"),
                    ),
                }
            }
        }
    }

    fn push_writer_surface_findings(&self, findings: &mut Vec<HealthFinding>) {
        for worker in self.workers.values() {
            if worker.role != WorkerRole::Worker {
                continue;
            }
            for task_id in worker.active.iter().chain(worker.queue.iter()) {
                let Some(task) = self.tasks.get(task_id) else {
                    continue;
                };
                if task.allowed_edit.is_empty() {
                    push(
                        findings,
                        HealthSeverity::Warning,
                        "writer_task_missing_allowed_edit",
                        &task.id,
                        format!(
                            "writer `{}` has a task without allowed edit surfaces",
                            worker.id
                        ),
                    );
                }
            }
        }
    }

    fn push_staleness_findings(
        &self,
        ctx: &CommandContext,
        board_path: &Path,
        findings: &mut Vec<HealthFinding>,
    ) -> Result<(), XtaskError> {
        let status = self.bounded_status(ctx, board_path, &TaskQuery::all())?;
        for warning in status.warnings {
            push(
                findings,
                HealthSeverity::Warning,
                warning.kind,
                warning.subject,
                warning.message,
            );
        }
        Ok(())
    }
}

fn push_missing_state_worker(findings: &mut Vec<HealthFinding>, task: &Task, worker: &str) {
    push(
        findings,
        HealthSeverity::Error,
        "task_state_missing_worker",
        &task.id,
        format!("task references missing worker `{worker}`"),
    );
}

fn push(
    findings: &mut Vec<HealthFinding>,
    severity: HealthSeverity,
    kind: impl Into<String>,
    subject: impl Into<String>,
    message: impl Into<String>,
) {
    findings.push(HealthFinding {
        severity,
        kind: kind.into(),
        subject: subject.into(),
        message: message.into(),
    });
}

fn severity_rank(severity: HealthSeverity) -> u8 {
    match severity {
        HealthSeverity::Error => 0,
        HealthSeverity::Warning => 1,
    }
}
