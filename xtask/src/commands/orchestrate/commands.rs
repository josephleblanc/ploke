use std::path::{Path, PathBuf};

use crate::commands::{CommandContext, XtaskError};

use super::board::AssignmentSlot;
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

impl BoardArg {
    pub(super) fn path(&self) -> &Path {
        &self.board
    }
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
    /// Show a bounded routine summary instead of the full board.
    #[arg(long)]
    pub(super) brief: bool,
    /// Only show tasks that belong to this task set.
    #[arg(long = "set")]
    pub(super) task_set: Option<String>,
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

/// Show local orchestrator command usage counters.
#[derive(Debug, Clone, clap::Args)]
pub struct Usage {
    #[command(flatten)]
    pub(super) board: BoardArg,
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
        if self.brief {
            Ok(OrchestrateOutput::StatusBrief(board.bounded_status(
                ctx,
                &path,
                self.task_set.as_deref(),
            )?))
        } else {
            Ok(OrchestrateOutput::Status(board.status(
                ctx,
                &path,
                self.task_set.as_deref(),
            )?))
        }
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
        let created_at = now();
        let task = Task {
            id: self.id.clone(),
            lane: self.lane.clone(),
            title: self.title.clone(),
            priority: self.priority,
            state: TaskState::NotStarted,
            created_at: created_at.clone(),
            updated_at: created_at,
            assigned_at: None,
            activated_at: None,
            completed_at: None,
            reviewed_at: None,
            blocked_at: None,
            unblocked_at: None,
            allowed_edit: self.allowed_edit.clone(),
            forbidden_edit: self.forbidden_edit.clone(),
            docs: self.docs.clone(),
            acceptance: self.acceptance.clone(),
            reports: Vec::new(),
            blockers: Vec::new(),
            task_sets: Vec::new(),
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
        let worker = board.assign_task(
            &self.task,
            &self.worker,
            if self.active {
                AssignmentSlot::Active
            } else {
                AssignmentSlot::Queue
            },
        )?;
        board.save(&path)?;
        Ok(OrchestrateOutput::Assigned { worker })
    }
}

impl Complete {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        board.ensure_task_can_complete(&self.task)?;
        let mut reports = Vec::new();
        if let Some(summary) = &self.summary {
            let report = board.write_inline_report(ctx, &path, &self.task, "complete", summary)?;
            reports.push(report);
        }
        if let Some(report) = &self.report {
            reports.push(report.clone());
        }
        let task = board.complete_task(&self.task, reports)?;
        board.save(&path)?;
        Ok(OrchestrateOutput::Completed { task })
    }
}

impl Review {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        let task = board.review_task(&self.task, self.report.clone())?;
        board.save(&path)?;
        Ok(OrchestrateOutput::Reviewed { task })
    }
}

impl Block {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        let blocker = Blocker {
            id: self.id.clone(),
            task_id: self.task.clone(),
            kind: self.kind,
            summary: self.summary.clone(),
            evidence: self.evidence.clone(),
            proposed_unblock: self.unblock.clone(),
            created_at: now(),
            resolved_at: None,
            resolution_summary: None,
            resolution_evidence: Vec::new(),
        };
        let (task, blocker) = board.block_task(&self.task, blocker)?;
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
        let packet_path_display = display(ctx, &packet_path)?;
        board.record_packet_generated(&self.worker, packet_path_display.clone());
        board.save(&path)?;
        Ok(OrchestrateOutput::Packet {
            worker: self.worker.clone(),
            path: packet_path_display,
        })
    }
}

impl Usage {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        Ok(OrchestrateOutput::Usage(super::usage::usage_summary(
            ctx,
            &self.board,
        )?))
    }
}
