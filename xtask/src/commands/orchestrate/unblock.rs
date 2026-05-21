use crate::commands::{CommandContext, XtaskError};

use super::{Blocker, Board, BoardArg, BoardLock, OrchestrateOutput, TaskState, now, resolve};

/// Resolve a blocker and restore the task to an explicit next state.
#[derive(Debug, Clone, clap::Args)]
pub struct Unblock {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Blocker id.
    pub(super) blocker: String,
    /// Resolution summary.
    #[arg(long)]
    pub(super) summary: String,
    /// Resolution evidence path or note. May be repeated.
    #[arg(long)]
    pub(super) evidence: Vec<String>,
    /// Next task state after the blocker is resolved.
    #[arg(long, value_enum, default_value = "not-started")]
    pub(super) next: UnblockNext,
    /// Worker id required for queued or active next states.
    #[arg(long)]
    pub(super) worker: Option<String>,
}

/// Next task state after unblock.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum UnblockNext {
    /// Return task to not-started.
    NotStarted,
    /// Queue task for a worker.
    Queued,
    /// Assign task active for a worker.
    Active,
}

impl Unblock {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, self.board.path())?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        let task_id = board
            .blockers
            .get(&self.blocker)
            .ok_or_else(|| {
                XtaskError::validation(format!("Unknown blocker `{}`", self.blocker))
                    .with_recovery("Use an existing blocker id.")
            })?
            .task_id
            .clone();
        board.ensure_task(&task_id)?;
        let worker = self.worker_for_next(&board)?;

        {
            let blocker = board.blockers.get_mut(&self.blocker).expect("checked");
            if !blocker.is_open() {
                return Err(XtaskError::validation(format!(
                    "Blocker `{}` is already resolved",
                    self.blocker
                ))
                .with_recovery("Use an open blocker id."));
            }
            blocker.resolved_at = Some(now());
            blocker.resolution_summary = Some(self.summary.clone());
            blocker.resolution_evidence = self.evidence.clone();
        }

        if let Some(task) = board.tasks.get_mut(&task_id) {
            task.blockers.retain(|blocker| blocker != &self.blocker);
        }
        let remaining = board.first_open_blocker_for_task(&task_id);
        board.remove_task_from_workers(&task_id);
        if let Some(remaining) = remaining {
            let task = board.tasks.get_mut(&task_id).expect("checked");
            task.state = TaskState::Blocked { blocker: remaining };
        } else {
            self.apply_next_state(&mut board, &task_id, worker);
        }

        let task = board.tasks.get(&task_id).expect("checked").clone();
        let blocker: Blocker = board.blockers.get(&self.blocker).expect("checked").clone();
        board.record(format!("blocker {} resolved", self.blocker));
        board.save(&path)?;
        Ok(OrchestrateOutput::Unblocked { task, blocker })
    }

    fn worker_for_next(&self, board: &Board) -> Result<Option<String>, XtaskError> {
        match self.next {
            UnblockNext::NotStarted => Ok(None),
            UnblockNext::Queued | UnblockNext::Active => {
                let worker = self.worker.clone().ok_or_else(|| {
                    XtaskError::validation("--worker is required for queued or active unblock")
                        .with_recovery("Pass `--worker <id>` or use `--next not-started`.")
                })?;
                board.ensure_worker(&worker)?;
                Ok(Some(worker))
            }
        }
    }

    fn apply_next_state(&self, board: &mut Board, task_id: &str, worker: Option<String>) {
        match self.next {
            UnblockNext::NotStarted => {
                if let Some(task) = board.tasks.get_mut(task_id) {
                    task.state = TaskState::NotStarted;
                }
            }
            UnblockNext::Queued => {
                let worker = worker.expect("checked");
                if let Some(slot) = board.workers.get_mut(&worker) {
                    if !slot.queue.iter().any(|queued| queued == task_id) {
                        slot.queue.push(task_id.to_string());
                    }
                }
                if let Some(task) = board.tasks.get_mut(task_id) {
                    task.state = TaskState::AssignedQueued { worker };
                }
            }
            UnblockNext::Active => {
                let worker = worker.expect("checked");
                if let Some(slot) = board.workers.get_mut(&worker) {
                    if let Some(previous) = slot.active.replace(task_id.to_string()) {
                        slot.queue.insert(0, previous.clone());
                        if let Some(task) = board.tasks.get_mut(&previous) {
                            task.state = TaskState::AssignedQueued {
                                worker: worker.clone(),
                            };
                        }
                    }
                }
                if let Some(task) = board.tasks.get_mut(task_id) {
                    task.state = TaskState::AssignedActive { worker };
                }
            }
        }
    }
}
