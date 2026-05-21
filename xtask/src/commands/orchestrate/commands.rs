use std::path::PathBuf;

use crate::commands::{CommandContext, XtaskError};

use super::{
    Blocker, BlockerKind, Board, BoardLock, DEFAULT_BOARD_PATH, DEFAULT_PACKET_DIR,
    OrchestrateOutput, Task, TaskState, WorkerRole, WorkerSlot, display, now, resolve,
};

/// Shared board path argument.
#[derive(Debug, Clone, clap::Args)]
pub struct BoardArg {
    /// Board JSON path, relative to the workspace root unless absolute.
    #[arg(long, default_value = DEFAULT_BOARD_PATH)]
    pub(super) board: PathBuf,
}

/// Initialize the orchestration board.
#[derive(Debug, Clone, clap::Args)]
pub struct Init {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Directory where generated worker packet markdown files are written.
    #[arg(long, default_value = DEFAULT_PACKET_DIR)]
    pub(super) packet_dir: PathBuf,
}

/// Show board status.
#[derive(Debug, Clone, clap::Args)]
pub struct Status {
    #[command(flatten)]
    pub(super) board: BoardArg,
}

/// Add or update a worker slot.
#[derive(Debug, Clone, clap::Args)]
pub struct WorkerCommand {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Worker identifier, for example `worker-records` or `retainer`.
    pub(super) id: String,
    /// Worker role.
    #[arg(long, value_enum, default_value = "worker")]
    pub(super) role: WorkerRole,
    /// Refresh warning threshold for retainer/explorer workers.
    #[arg(long, default_value_t = 5)]
    pub(super) refresh_after_questions: u32,
}

/// Add a new task.
#[derive(Debug, Clone, clap::Args)]
pub struct AddTask {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Stable task id.
    pub(super) id: String,
    /// Lane name, such as `records`, `loader`, `graph`, `review`, or `docs`.
    #[arg(long)]
    pub(super) lane: String,
    /// Short task title.
    #[arg(long)]
    pub(super) title: String,
    /// Task priority. Lower numbers are more urgent.
    #[arg(long, default_value_t = 3)]
    pub(super) priority: u8,
    /// Allowed edit surface. May be repeated.
    #[arg(long = "allow")]
    pub(super) allowed_edit: Vec<String>,
    /// Forbidden edit surface. May be repeated.
    #[arg(long = "forbid")]
    pub(super) forbidden_edit: Vec<String>,
    /// Context document or exact file/range the worker should read. May be repeated.
    #[arg(long = "doc")]
    pub(super) docs: Vec<String>,
    /// Acceptance criterion. May be repeated.
    #[arg(long = "accept")]
    pub(super) acceptance: Vec<String>,
}

/// Assign a task to a worker.
#[derive(Debug, Clone, clap::Args)]
pub struct Assign {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Task id.
    pub(super) task: String,
    /// Worker id.
    pub(super) worker: String,
    /// Put the task in the worker's active slot instead of its queue.
    #[arg(long)]
    pub(super) active: bool,
}

/// Mark a task complete and ready for review.
#[derive(Debug, Clone, clap::Args)]
pub struct Complete {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Task id.
    pub(super) task: String,
    /// Optional worker report path.
    #[arg(long)]
    pub(super) report: Option<String>,
    /// Inline completion summary. Written to `.orchestrator/reports/` and attached as a report.
    #[arg(long)]
    pub(super) summary: Option<String>,
}

/// Mark a completed task as reviewed.
#[derive(Debug, Clone, clap::Args)]
pub struct Review {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Task id.
    pub(super) task: String,
    /// Review report or note.
    #[arg(long)]
    pub(super) report: Option<String>,
}

/// Add a blocker against a task.
#[derive(Debug, Clone, clap::Args)]
pub struct Block {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Task id.
    pub(super) task: String,
    /// Blocker id.
    pub(super) id: String,
    /// Blocker kind.
    #[arg(long, value_enum, default_value = "dependency")]
    pub(super) kind: BlockerKind,
    /// Short summary.
    #[arg(long)]
    pub(super) summary: String,
    /// Evidence path or note. May be repeated.
    #[arg(long)]
    pub(super) evidence: Vec<String>,
    /// Proposed unblock action.
    #[arg(long)]
    pub(super) unblock: Option<String>,
}

/// Generate a worker packet file.
#[derive(Debug, Clone, clap::Args)]
pub struct Packet {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Worker id.
    pub(super) worker: String,
}

impl Init {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let board_path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&board_path)?;
        if board_path.exists() {
            let board = Board::load(&board_path)?;
            return Ok(OrchestrateOutput::Initialized {
                board: display(ctx, &board_path)?,
                packet_dir: display(ctx, &resolve(ctx, &board.packet_dir)?)?,
            });
        }

        let packet_dir = self.packet_dir.clone();
        let mut board = Board::new(packet_dir);
        board.record("initialized board");
        board.save(&board_path)?;
        Ok(OrchestrateOutput::Initialized {
            board: display(ctx, &board_path)?,
            packet_dir: display(ctx, &resolve(ctx, &board.packet_dir)?)?,
        })
    }
}

impl Status {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let board = Board::load(&path)?;
        Ok(OrchestrateOutput::Status(board.status(ctx, &path)?))
    }
}

impl WorkerCommand {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load_or_new(&path)?;
        let worker = board
            .workers
            .entry(self.id.clone())
            .or_insert_with(|| WorkerSlot::new(self.id.clone(), self.role));
        worker.role = self.role;
        worker.refresh_after_questions = self.refresh_after_questions;
        let worker = worker.clone();
        board.record(format!("worker {} set to {:?}", self.id, self.role));
        board.save(&path)?;
        Ok(OrchestrateOutput::Worker { worker })
    }
}

impl AddTask {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load_or_new(&path)?;
        if board.tasks.contains_key(&self.id) {
            return Err(
                XtaskError::validation(format!("Task `{}` already exists", self.id))
                    .with_recovery("Use a new task id or update the existing task manually."),
            );
        }
        let task = Task {
            id: self.id.clone(),
            lane: self.lane.clone(),
            title: self.title.clone(),
            priority: self.priority,
            state: TaskState::NotStarted,
            allowed_edit: self.allowed_edit.clone(),
            forbidden_edit: self.forbidden_edit.clone(),
            docs: self.docs.clone(),
            acceptance: self.acceptance.clone(),
            reports: Vec::new(),
            blockers: Vec::new(),
        };
        board.tasks.insert(task.id.clone(), task.clone());
        board.record(format!("task {} added to lane {}", task.id, task.lane));
        board.save(&path)?;
        Ok(OrchestrateOutput::Task { task })
    }
}

impl Assign {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        board.ensure_worker(&self.worker)?;
        board.ensure_task(&self.task)?;
        board.remove_task_from_workers(&self.task);

        let worker = board.workers.get_mut(&self.worker).expect("checked");
        if self.active {
            if let Some(previous) = worker.active.replace(self.task.clone()) {
                worker.queue.insert(0, previous.clone());
                if let Some(task) = board.tasks.get_mut(&previous) {
                    task.state = TaskState::AssignedQueued {
                        worker: self.worker.clone(),
                    };
                }
            }
            board.tasks.get_mut(&self.task).expect("checked").state = TaskState::AssignedActive {
                worker: self.worker.clone(),
            };
        } else {
            if !worker.queue.contains(&self.task) {
                worker.queue.push(self.task.clone());
            }
            board.tasks.get_mut(&self.task).expect("checked").state = TaskState::AssignedQueued {
                worker: self.worker.clone(),
            };
        }

        let worker = board.workers.get(&self.worker).expect("checked").clone();
        board.record(format!(
            "task {} assigned to {}{}",
            self.task,
            self.worker,
            if self.active { " active" } else { " queue" }
        ));
        board.save(&path)?;
        Ok(OrchestrateOutput::Assigned { worker })
    }
}

impl Complete {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        board.ensure_task(&self.task)?;
        let worker = board.assigned_worker(&self.task);
        if let Some(summary) = &self.summary {
            let report = board.write_inline_report(ctx, &path, &self.task, "complete", summary)?;
            board
                .tasks
                .get_mut(&self.task)
                .expect("checked")
                .reports
                .push(report);
        }
        if let Some(report) = &self.report {
            board
                .tasks
                .get_mut(&self.task)
                .expect("checked")
                .reports
                .push(report.clone());
        }
        board.remove_task_from_workers(&self.task);
        let task = board.tasks.get_mut(&self.task).expect("checked");
        task.state = TaskState::CompleteUnreviewed { worker };
        let task = task.clone();
        board.record(format!("task {} completed", self.task));
        board.save(&path)?;
        Ok(OrchestrateOutput::Completed { task })
    }
}

impl Review {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        board.ensure_task(&self.task)?;
        let task = board.tasks.get_mut(&self.task).expect("checked");
        if let Some(report) = &self.report {
            task.reports.push(report.clone());
        }
        task.state = TaskState::CompleteReviewed;
        let task = task.clone();
        board.record(format!("task {} reviewed", self.task));
        board.save(&path)?;
        Ok(OrchestrateOutput::Reviewed { task })
    }
}

impl Block {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        board.ensure_task(&self.task)?;
        if board.blockers.contains_key(&self.id) {
            return Err(
                XtaskError::validation(format!("Blocker `{}` already exists", self.id))
                    .with_recovery("Use a new blocker id."),
            );
        }
        let blocker = Blocker {
            id: self.id.clone(),
            task_id: self.task.clone(),
            kind: self.kind,
            summary: self.summary.clone(),
            evidence: self.evidence.clone(),
            proposed_unblock: self.unblock.clone(),
            created_at: now(),
        };
        board.blockers.insert(blocker.id.clone(), blocker.clone());
        board.remove_task_from_workers(&self.task);
        let task = board.tasks.get_mut(&self.task).expect("checked");
        task.blockers.push(blocker.id.clone());
        task.state = TaskState::Blocked {
            blocker: blocker.id.clone(),
        };
        let task = task.clone();
        board.record(format!("task {} blocked by {}", self.task, self.id));
        board.save(&path)?;
        Ok(OrchestrateOutput::Blocked { task, blocker })
    }
}

impl Packet {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        board.ensure_worker(&self.worker)?;
        let packet_path = board.write_packet(ctx, &self.worker)?;
        if let Some(worker) = board.workers.get_mut(&self.worker) {
            worker.packet_path = Some(display(ctx, &packet_path)?);
        }
        board.record(format!("packet generated for {}", self.worker));
        board.save(&path)?;
        Ok(OrchestrateOutput::Packet {
            worker: self.worker.clone(),
            path: display(ctx, &packet_path)?,
        })
    }
}
