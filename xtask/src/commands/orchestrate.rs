//! Agent orchestration board commands.
//!
//! This command family is intentionally a small state mutator and renderer. It
//! stores worker slots, task queues, blockers, and generated worker packets so
//! the main agent can keep several disjoint lanes moving without turning a chat
//! thread into the task database.

use std::collections::BTreeMap;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use super::{CommandContext, XtaskError};

mod lanes;
#[cfg(test)]
mod tests;

use lanes::{LaneCommand, LaneSpec, LaneValidation};

const DEFAULT_BOARD_PATH: &str = ".orchestrator/board.json";
const DEFAULT_PACKET_DIR: &str = ".orchestrator/workers";
const SCHEMA_VERSION: &str = "orchestrator-board.v1";

/// Commands for the agent orchestration board.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum Orchestrate {
    /// Create the board file if it does not already exist.
    Init(Init),
    /// Show the current board state.
    Status(Status),
    /// Add or update a worker slot.
    Worker(WorkerCommand),
    /// Add a task to the board.
    Add(AddTask),
    /// Assign a task to a worker queue or active slot.
    Assign(Assign),
    /// Mark a task complete and ready for review.
    Complete(Complete),
    /// Mark a completed task as reviewed.
    Review(Review),
    /// Add a blocker against a task.
    Block(Block),
    /// Write a worker packet file from the current assignment.
    Packet(Packet),
    /// Define and validate lane-owned edit surfaces.
    #[command(subcommand)]
    Lane(LaneCommand),
}

impl Orchestrate {
    /// Execute an orchestration command.
    pub fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        match self {
            Self::Init(cmd) => cmd.execute(ctx),
            Self::Status(cmd) => cmd.execute(ctx),
            Self::Worker(cmd) => cmd.execute(ctx),
            Self::Add(cmd) => cmd.execute(ctx),
            Self::Assign(cmd) => cmd.execute(ctx),
            Self::Complete(cmd) => cmd.execute(ctx),
            Self::Review(cmd) => cmd.execute(ctx),
            Self::Block(cmd) => cmd.execute(ctx),
            Self::Packet(cmd) => cmd.execute(ctx),
            Self::Lane(cmd) => cmd.execute(ctx),
        }
    }
}

/// Shared board path argument.
#[derive(Debug, Clone, clap::Args)]
pub struct BoardArg {
    /// Board JSON path, relative to the workspace root unless absolute.
    #[arg(long, default_value = DEFAULT_BOARD_PATH)]
    board: PathBuf,
}

/// Initialize the orchestration board.
#[derive(Debug, Clone, clap::Args)]
pub struct Init {
    #[command(flatten)]
    board: BoardArg,
    /// Directory where generated worker packet markdown files are written.
    #[arg(long, default_value = DEFAULT_PACKET_DIR)]
    packet_dir: PathBuf,
}

/// Show board status.
#[derive(Debug, Clone, clap::Args)]
pub struct Status {
    #[command(flatten)]
    board: BoardArg,
}

/// Add or update a worker slot.
#[derive(Debug, Clone, clap::Args)]
pub struct WorkerCommand {
    #[command(flatten)]
    board: BoardArg,
    /// Worker identifier, for example `worker-records` or `retainer`.
    id: String,
    /// Worker role.
    #[arg(long, value_enum, default_value = "worker")]
    role: WorkerRole,
    /// Refresh warning threshold for retainer/explorer workers.
    #[arg(long, default_value_t = 5)]
    refresh_after_questions: u32,
}

/// Add a new task.
#[derive(Debug, Clone, clap::Args)]
pub struct AddTask {
    #[command(flatten)]
    board: BoardArg,
    /// Stable task id.
    id: String,
    /// Lane name, such as `records`, `loader`, `graph`, `review`, or `docs`.
    #[arg(long)]
    lane: String,
    /// Short task title.
    #[arg(long)]
    title: String,
    /// Task priority. Lower numbers are more urgent.
    #[arg(long, default_value_t = 3)]
    priority: u8,
    /// Allowed edit surface. May be repeated.
    #[arg(long = "allow")]
    allowed_edit: Vec<String>,
    /// Forbidden edit surface. May be repeated.
    #[arg(long = "forbid")]
    forbidden_edit: Vec<String>,
    /// Context document or exact file/range the worker should read. May be repeated.
    #[arg(long = "doc")]
    docs: Vec<String>,
    /// Acceptance criterion. May be repeated.
    #[arg(long = "accept")]
    acceptance: Vec<String>,
}

/// Assign a task to a worker.
#[derive(Debug, Clone, clap::Args)]
pub struct Assign {
    #[command(flatten)]
    board: BoardArg,
    /// Task id.
    task: String,
    /// Worker id.
    worker: String,
    /// Put the task in the worker's active slot instead of its queue.
    #[arg(long)]
    active: bool,
}

/// Mark a task complete and ready for review.
#[derive(Debug, Clone, clap::Args)]
pub struct Complete {
    #[command(flatten)]
    board: BoardArg,
    /// Task id.
    task: String,
    /// Optional worker report path.
    #[arg(long)]
    report: Option<String>,
    /// Inline completion summary. Written to `.orchestrator/reports/` and attached as a report.
    #[arg(long)]
    summary: Option<String>,
}

/// Mark a completed task as reviewed.
#[derive(Debug, Clone, clap::Args)]
pub struct Review {
    #[command(flatten)]
    board: BoardArg,
    /// Task id.
    task: String,
    /// Review report or note.
    #[arg(long)]
    report: Option<String>,
}

/// Add a blocker against a task.
#[derive(Debug, Clone, clap::Args)]
pub struct Block {
    #[command(flatten)]
    board: BoardArg,
    /// Task id.
    task: String,
    /// Blocker id.
    id: String,
    /// Blocker kind.
    #[arg(long, value_enum, default_value = "dependency")]
    kind: BlockerKind,
    /// Short summary.
    #[arg(long)]
    summary: String,
    /// Evidence path or note. May be repeated.
    #[arg(long)]
    evidence: Vec<String>,
    /// Proposed unblock action.
    #[arg(long)]
    unblock: Option<String>,
}

/// Generate a worker packet file.
#[derive(Debug, Clone, clap::Args)]
pub struct Packet {
    #[command(flatten)]
    board: BoardArg,
    /// Worker id.
    worker: String,
}

/// Worker role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum WorkerRole {
    /// Implementation worker.
    Worker,
    /// Independent reviewer.
    Reviewer,
    /// Read-only explorer or retainer.
    Retainer,
}

/// Blocker kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, clap::ValueEnum)]
#[serde(rename_all = "snake_case")]
pub enum BlockerKind {
    /// Missing type or passive record owner.
    MissingType,
    /// Ownership or crate boundary is ambiguous.
    AmbiguousOwner,
    /// Verification failed.
    TestFailure,
    /// Another task must complete first.
    Dependency,
    /// Safety boundary or forbidden edit surface.
    SafetyBoundary,
    /// Other blocker.
    Other,
}

/// Command output for orchestration commands.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum OrchestrateOutput {
    /// Board was initialized.
    Initialized {
        /// Board path.
        board: String,
        /// Packet directory.
        packet_dir: String,
    },
    /// Board status summary.
    Status(BoardStatus),
    /// Worker was added or updated.
    Worker {
        /// Worker slot after the change.
        worker: WorkerSlot,
    },
    /// Task was added.
    Task {
        /// Task after the change.
        task: Task,
    },
    /// Assignment changed.
    Assigned {
        /// Worker slot after assignment.
        worker: WorkerSlot,
    },
    /// Task completed.
    Completed {
        /// Task after completion.
        task: Task,
    },
    /// Task reviewed.
    Reviewed {
        /// Task after review.
        task: Task,
    },
    /// Blocker added.
    Blocked {
        /// Task after blocker attachment.
        task: Task,
        /// Newly created blocker.
        blocker: Blocker,
    },
    /// Worker packet was written.
    Packet {
        /// Worker id.
        worker: String,
        /// Packet path.
        path: String,
    },
    /// Lane was added or replaced.
    Lane {
        /// Lane definition after the change.
        lane: LaneSpec,
    },
    /// Lane validation result.
    LaneValidation {
        /// Validation result.
        validation: LaneValidation,
    },
}

/// Serializable board state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    /// Schema version.
    schema_version: String,
    /// Creation time.
    created_at: String,
    /// Update time.
    updated_at: String,
    /// Packet directory relative to workspace root unless absolute.
    packet_dir: PathBuf,
    /// Lane-owned edit surface groups.
    #[serde(default)]
    lanes: BTreeMap<String, LaneSpec>,
    /// Known workers.
    workers: BTreeMap<String, WorkerSlot>,
    /// Known tasks.
    tasks: BTreeMap<String, Task>,
    /// Known blockers.
    blockers: BTreeMap<String, Blocker>,
    /// Append-only event summaries.
    events: Vec<Event>,
}

/// Worker slot state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerSlot {
    /// Worker id.
    id: String,
    /// Worker role.
    role: WorkerRole,
    /// Current active task.
    active: Option<String>,
    /// Queued task ids.
    queue: Vec<String>,
    /// Last generated packet path.
    packet_path: Option<String>,
    /// Retainer refresh threshold.
    refresh_after_questions: u32,
    /// Number of answered retainer questions since refresh.
    answered_questions: u32,
}

/// Task state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Task id.
    id: String,
    /// Lane name.
    lane: String,
    /// Short title.
    title: String,
    /// Lower numbers are more urgent.
    priority: u8,
    /// Current assignment state.
    state: TaskState,
    /// Allowed edit surfaces.
    allowed_edit: Vec<String>,
    /// Forbidden edit surfaces.
    forbidden_edit: Vec<String>,
    /// Context docs or file ranges.
    docs: Vec<String>,
    /// Acceptance criteria.
    acceptance: Vec<String>,
    /// Worker report paths.
    reports: Vec<String>,
    /// Attached blocker ids.
    blockers: Vec<String>,
}

/// Task assignment state.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "state", rename_all = "snake_case")]
pub enum TaskState {
    /// Task is not assigned.
    NotStarted,
    /// Task is queued for a worker.
    AssignedQueued {
        /// Assigned worker id.
        worker: String,
    },
    /// Task is active for a worker.
    AssignedActive {
        /// Active worker id.
        worker: String,
    },
    /// Task is complete and awaiting review.
    CompleteUnreviewed {
        /// Worker that completed the task, if known.
        worker: Option<String>,
    },
    /// Task is complete and reviewed.
    CompleteReviewed,
    /// Task is blocked.
    Blocked {
        /// Blocking blocker id.
        blocker: String,
    },
}

/// Blocker record.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Blocker {
    /// Blocker id.
    id: String,
    /// Blocked task id.
    task_id: String,
    /// Blocker kind.
    kind: BlockerKind,
    /// Short summary.
    summary: String,
    /// Evidence paths or notes.
    evidence: Vec<String>,
    /// Proposed unblock action.
    proposed_unblock: Option<String>,
    /// Creation time.
    created_at: String,
}

/// Board event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event time.
    at: String,
    /// Event summary.
    summary: String,
}

/// Board status summary.
#[derive(Debug, Clone, Serialize)]
pub struct BoardStatus {
    /// Board path.
    board: String,
    /// Packet directory.
    packet_dir: String,
    /// Lane-owned edit surfaces.
    lanes: Vec<LaneSpec>,
    /// Workers.
    workers: Vec<WorkerSlot>,
    /// Tasks grouped by state.
    tasks: Vec<Task>,
    /// Blockers.
    blockers: Vec<Blocker>,
}

impl Init {
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
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
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, &self.board.board)?;
        let board = Board::load(&path)?;
        Ok(OrchestrateOutput::Status(board.status(ctx, &path)?))
    }
}

impl WorkerCommand {
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
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
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
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
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
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
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
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
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
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
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
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
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
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

impl Board {
    fn new(packet_dir: PathBuf) -> Self {
        let now = now();
        Self {
            schema_version: SCHEMA_VERSION.to_string(),
            created_at: now.clone(),
            updated_at: now,
            packet_dir,
            lanes: BTreeMap::new(),
            workers: BTreeMap::new(),
            tasks: BTreeMap::new(),
            blockers: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    fn load_or_new(path: &Path) -> Result<Self, XtaskError> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::new(PathBuf::from(DEFAULT_PACKET_DIR)))
        }
    }

    fn load(path: &Path) -> Result<Self, XtaskError> {
        let contents = fs::read_to_string(path)?;
        let board: Self = serde_json::from_str(&contents)?;
        if board.schema_version != SCHEMA_VERSION {
            return Err(XtaskError::validation(format!(
                "Unsupported board schema `{}`",
                board.schema_version
            ))
            .with_recovery(format!("Expected `{SCHEMA_VERSION}`.")));
        }
        Ok(board)
    }

    fn save(&mut self, path: &Path) -> Result<(), XtaskError> {
        self.updated_at = now();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let body = serde_json::to_string_pretty(self)?;
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, format!("{body}\n"))?;
        fs::rename(tmp, path)?;
        Ok(())
    }

    fn status(&self, ctx: &CommandContext, board_path: &Path) -> Result<BoardStatus, XtaskError> {
        let mut workers: Vec<_> = self.workers.values().cloned().collect();
        workers.sort_by(|a, b| a.id.cmp(&b.id));
        let mut tasks: Vec<_> = self.tasks.values().cloned().collect();
        tasks.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.id.cmp(&b.id)));
        let mut blockers: Vec<_> = self.blockers.values().cloned().collect();
        blockers.sort_by(|a, b| a.id.cmp(&b.id));
        let mut lanes: Vec<_> = self.lanes.values().cloned().collect();
        lanes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(BoardStatus {
            board: display(ctx, board_path)?,
            packet_dir: display(ctx, &resolve(ctx, &self.packet_dir)?)?,
            lanes,
            workers,
            tasks,
            blockers,
        })
    }

    fn record(&mut self, summary: impl Into<String>) {
        self.events.push(Event {
            at: now(),
            summary: summary.into(),
        });
        if self.events.len() > 200 {
            let keep_from = self.events.len() - 200;
            self.events.drain(0..keep_from);
        }
    }

    fn ensure_worker(&self, id: &str) -> Result<(), XtaskError> {
        if self.workers.contains_key(id) {
            Ok(())
        } else {
            Err(XtaskError::validation(format!("Unknown worker `{id}`"))
                .with_recovery("Add it with `target/debug/xtask orchestrate worker <id>`."))
        }
    }

    fn ensure_task(&self, id: &str) -> Result<(), XtaskError> {
        if self.tasks.contains_key(id) {
            Ok(())
        } else {
            Err(
                XtaskError::validation(format!("Unknown task `{id}`")).with_recovery(
                    "Add it with `target/debug/xtask orchestrate add <id> --lane <lane> --title <title>`.",
                ),
            )
        }
    }

    fn remove_task_from_workers(&mut self, task_id: &str) {
        for worker in self.workers.values_mut() {
            if worker.active.as_deref() == Some(task_id) {
                worker.active = None;
            }
            worker.queue.retain(|queued| queued != task_id);
        }
    }

    fn assigned_worker(&self, task_id: &str) -> Option<String> {
        self.workers.values().find_map(|worker| {
            if worker.active.as_deref() == Some(task_id)
                || worker.queue.iter().any(|queued| queued == task_id)
            {
                Some(worker.id.clone())
            } else {
                None
            }
        })
    }

    fn write_packet(&self, ctx: &CommandContext, worker_id: &str) -> Result<PathBuf, XtaskError> {
        let worker = self.workers.get(worker_id).expect("checked");
        let packet_dir = resolve(ctx, &self.packet_dir)?;
        fs::create_dir_all(&packet_dir)?;
        let path = packet_dir.join(format!("{worker_id}.md"));
        fs::write(&path, self.packet_body(worker))?;
        Ok(path)
    }

    fn write_inline_report(
        &self,
        ctx: &CommandContext,
        board_path: &Path,
        task_id: &str,
        kind: &str,
        summary: &str,
    ) -> Result<String, XtaskError> {
        let report_dir = board_path
            .parent()
            .unwrap_or_else(|| Path::new("."))
            .join("reports");
        fs::create_dir_all(&report_dir)?;
        let timestamp = now();
        let file_name = format!(
            "{}-{}-{}.md",
            file_fragment(task_id),
            kind,
            file_fragment(&timestamp)
        );
        let path = report_dir.join(file_name);
        let body = format!(
            "# Orchestrator {kind} Summary\n\n- task: {task_id}\n- recorded_at: {timestamp}\n\n{summary}\n"
        );
        fs::write(&path, body)?;
        display(ctx, &path)
    }

    fn packet_body(&self, worker: &WorkerSlot) -> String {
        let mut out = String::new();
        out.push_str(&format!("# Worker Packet: {}\n\n", worker.id));
        out.push_str(&format!("- role: {:?}\n", worker.role));
        if worker.role == WorkerRole::Retainer {
            out.push_str(&format!(
                "- refresh_after_questions: {}\n",
                worker.refresh_after_questions
            ));
        }
        out.push_str("\n## Active Task\n\n");
        match worker
            .active
            .as_ref()
            .and_then(|task_id| self.tasks.get(task_id))
        {
            Some(task) => push_task(&mut out, task),
            None => out.push_str("No active task assigned.\n"),
        }

        out.push_str("\n## Queued Tasks\n\n");
        if worker.queue.is_empty() {
            out.push_str("No queued tasks.\n");
        } else {
            for task_id in &worker.queue {
                if let Some(task) = self.tasks.get(task_id) {
                    push_task(&mut out, task);
                    out.push('\n');
                }
            }
        }

        out.push_str("\n## Report Contract\n\n");
        out.push_str("- Report changed files or `none`.\n");
        out.push_str("- Report verification commands and outcomes.\n");
        out.push_str("- Report blockers with exact file paths or evidence.\n");
        out.push_str("- Do not edit outside allowed surfaces.\n");
        out.push_str("- Do not touch forbidden files.\n");
        out
    }
}

impl WorkerSlot {
    fn new(id: String, role: WorkerRole) -> Self {
        Self {
            id,
            role,
            active: None,
            queue: Vec::new(),
            packet_path: None,
            refresh_after_questions: 5,
            answered_questions: 0,
        }
    }
}

fn push_task(out: &mut String, task: &Task) {
    out.push_str(&format!("### {}: {}\n\n", task.id, task.title));
    out.push_str(&format!("- lane: {}\n", task.lane));
    out.push_str(&format!("- priority: {}\n", task.priority));
    out.push_str(&format!("- state: {:?}\n", task.state));
    push_list(out, "allowed_edit", &task.allowed_edit);
    push_list(out, "forbidden_edit", &task.forbidden_edit);
    push_list(out, "docs", &task.docs);
    push_list(out, "acceptance", &task.acceptance);
    push_list(out, "blockers", &task.blockers);
}

fn push_list(out: &mut String, label: &str, items: &[String]) {
    if items.is_empty() {
        return;
    }
    out.push_str(&format!("- {label}:\n"));
    for item in items {
        out.push_str(&format!("  - {item}\n"));
    }
}

fn file_fragment(raw: &str) -> String {
    let mut out = String::new();
    for ch in raw.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.') {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    if out.is_empty() {
        "item".to_string()
    } else {
        out
    }
}

fn resolve(ctx: &CommandContext, path: &Path) -> Result<PathBuf, XtaskError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(ctx.workspace_root()?.join(path))
    }
}

fn display(ctx: &CommandContext, path: &Path) -> Result<String, XtaskError> {
    let root = ctx.workspace_root()?;
    Ok(path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string())
}

fn now() -> String {
    Utc::now().to_rfc3339()
}

struct BoardLock {
    path: PathBuf,
}

impl BoardLock {
    fn acquire(board_path: &Path) -> Result<Self, XtaskError> {
        if let Some(parent) = board_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let lock_path = board_path.with_extension("lock");
        let started = Instant::now();
        loop {
            match OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&lock_path)
            {
                Ok(mut file) => {
                    write!(file, "pid={}\n", std::process::id())?;
                    return Ok(Self { path: lock_path });
                }
                Err(err) if err.kind() == std::io::ErrorKind::AlreadyExists => {
                    if started.elapsed() > Duration::from_secs(10) {
                        return Err(XtaskError::validation(format!(
                            "Timed out waiting for board lock `{}`",
                            lock_path.display()
                        ))
                        .with_recovery(
                            "Check for a stale lock file if no xtask process is running.",
                        ));
                    }
                    thread::sleep(Duration::from_millis(50));
                }
                Err(err) => return Err(err.into()),
            }
        }
    }
}

impl Drop for BoardLock {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.path);
    }
}
