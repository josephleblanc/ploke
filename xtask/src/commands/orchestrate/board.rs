use std::collections::BTreeMap;
use std::collections::BTreeSet;
use std::fs;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Utc;
use serde::{Deserialize, Serialize};

use crate::commands::{CommandContext, XtaskError};

use super::lanes::LaneSpec;

pub(super) const DEFAULT_BOARD_PATH: &str = ".orchestrator/board.json";
pub(super) const DEFAULT_PACKET_DIR: &str = ".orchestrator/workers";
pub(super) const SCHEMA_VERSION: &str = "orchestrator-board.v1";

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

/// Serializable board state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Board {
    /// Schema version.
    pub(super) schema_version: String,
    /// Creation time.
    pub(super) created_at: String,
    /// Update time.
    pub(super) updated_at: String,
    /// Packet directory relative to workspace root unless absolute.
    pub(super) packet_dir: PathBuf,
    /// Lane-owned edit surface groups.
    #[serde(default)]
    pub(super) lanes: BTreeMap<String, LaneSpec>,
    /// Known workers.
    pub(super) workers: BTreeMap<String, WorkerSlot>,
    /// Known tasks.
    pub(super) tasks: BTreeMap<String, Task>,
    /// Known blockers.
    pub(super) blockers: BTreeMap<String, Blocker>,
    /// Named task sets.
    #[serde(default)]
    pub(super) task_sets: BTreeMap<String, TaskSet>,
    /// Append-only event summaries.
    pub(super) events: Vec<Event>,
}

/// Worker slot state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkerSlot {
    /// Worker id.
    pub(super) id: String,
    /// Worker role.
    pub(super) role: WorkerRole,
    /// Current active task.
    pub(super) active: Option<String>,
    /// Queued task ids.
    pub(super) queue: Vec<String>,
    /// Last generated packet path.
    pub(super) packet_path: Option<String>,
    /// Retainer refresh threshold.
    pub(super) refresh_after_questions: u32,
    /// Number of answered retainer questions since refresh.
    pub(super) answered_questions: u32,
}

/// Task state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Task {
    /// Task id.
    pub(super) id: String,
    /// Lane name.
    pub(super) lane: String,
    /// Short title.
    pub(super) title: String,
    /// Lower numbers are more urgent.
    pub(super) priority: u8,
    /// Current assignment state.
    pub(super) state: TaskState,
    /// Allowed edit surfaces.
    pub(super) allowed_edit: Vec<String>,
    /// Forbidden edit surfaces.
    pub(super) forbidden_edit: Vec<String>,
    /// Context docs or file ranges.
    pub(super) docs: Vec<String>,
    /// Acceptance criteria.
    pub(super) acceptance: Vec<String>,
    /// Worker report paths.
    pub(super) reports: Vec<String>,
    /// Attached blocker ids.
    pub(super) blockers: Vec<String>,
    /// Named task-set memberships.
    #[serde(default)]
    pub(super) task_sets: Vec<String>,
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
    pub(super) id: String,
    /// Blocked task id.
    pub(super) task_id: String,
    /// Blocker kind.
    pub(super) kind: BlockerKind,
    /// Short summary.
    pub(super) summary: String,
    /// Evidence paths or notes.
    pub(super) evidence: Vec<String>,
    /// Proposed unblock action.
    pub(super) proposed_unblock: Option<String>,
    /// Creation time.
    pub(super) created_at: String,
    /// Resolution time.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) resolved_at: Option<String>,
    /// Resolution summary.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(super) resolution_summary: Option<String>,
    /// Resolution evidence paths or notes.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(super) resolution_evidence: Vec<String>,
}

/// Named task-set metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskSet {
    /// Task-set id.
    pub(super) id: String,
    /// Optional description.
    pub(super) description: Option<String>,
    /// Creation time.
    pub(super) created_at: String,
}

/// Board event.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Event {
    /// Event time.
    pub(super) at: String,
    /// Event summary.
    pub(super) summary: String,
}

/// Board status summary.
#[derive(Debug, Clone, Serialize)]
pub struct BoardStatus {
    /// Board path.
    pub(super) board: String,
    /// Applied task-set filter.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(super) task_set: Option<String>,
    /// Packet directory.
    pub(super) packet_dir: String,
    /// Lane-owned edit surfaces.
    pub(super) lanes: Vec<LaneSpec>,
    /// Workers.
    pub(super) workers: Vec<WorkerSlot>,
    /// Tasks grouped by state.
    pub(super) tasks: Vec<Task>,
    /// Blockers.
    pub(super) blockers: Vec<Blocker>,
}

impl Board {
    pub(super) fn new(packet_dir: PathBuf) -> Self {
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
            task_sets: BTreeMap::new(),
            events: Vec::new(),
        }
    }

    pub(super) fn load_or_new(path: &Path) -> Result<Self, XtaskError> {
        if path.exists() {
            Self::load(path)
        } else {
            Ok(Self::new(PathBuf::from(DEFAULT_PACKET_DIR)))
        }
    }

    pub(super) fn load(path: &Path) -> Result<Self, XtaskError> {
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

    pub(super) fn save(&mut self, path: &Path) -> Result<(), XtaskError> {
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

    pub(super) fn status(
        &self,
        ctx: &CommandContext,
        board_path: &Path,
        task_set: Option<&str>,
    ) -> Result<BoardStatus, XtaskError> {
        let task_filter = self.task_filter(task_set)?;
        let mut workers: Vec<_> = self
            .workers
            .values()
            .cloned()
            .map(|mut worker| {
                if let Some(filter) = &task_filter {
                    if !worker
                        .active
                        .as_ref()
                        .is_some_and(|task_id| filter.contains(task_id))
                    {
                        worker.active = None;
                    }
                    worker.queue.retain(|task_id| filter.contains(task_id));
                }
                worker
            })
            .collect();
        workers.sort_by(|a, b| a.id.cmp(&b.id));
        let mut tasks: Vec<_> = self
            .tasks
            .values()
            .filter(|task| {
                task_filter
                    .as_ref()
                    .map_or(true, |filter| filter.contains(&task.id))
            })
            .cloned()
            .collect();
        tasks.sort_by(|a, b| a.priority.cmp(&b.priority).then_with(|| a.id.cmp(&b.id)));
        let mut blockers: Vec<_> = self
            .blockers
            .values()
            .filter(|blocker| {
                blocker.is_open()
                    && task_filter
                        .as_ref()
                        .map_or(true, |filter| filter.contains(&blocker.task_id))
            })
            .cloned()
            .collect();
        blockers.sort_by(|a, b| a.id.cmp(&b.id));
        let task_lanes: BTreeSet<_> = tasks.iter().map(|task| task.lane.as_str()).collect();
        let mut lanes: Vec<_> = self.lanes.values().cloned().collect();
        if task_set.is_some() {
            lanes.retain(|lane| task_lanes.contains(lane.id.as_str()));
        }
        lanes.sort_by(|a, b| a.id.cmp(&b.id));
        Ok(BoardStatus {
            board: display(ctx, board_path)?,
            task_set: task_set.map(str::to_string),
            packet_dir: display(ctx, &resolve(ctx, &self.packet_dir)?)?,
            lanes,
            workers,
            tasks,
            blockers,
        })
    }

    pub(super) fn open_blockers_for_task(&self, task_id: &str) -> Vec<String> {
        let mut blockers: Vec<_> = self
            .blockers
            .values()
            .filter(|blocker| blocker.task_id == task_id && blocker.is_open())
            .map(|blocker| blocker.id.clone())
            .collect();
        blockers.sort();
        blockers
    }

    pub(super) fn first_open_blocker_for_task(&self, task_id: &str) -> Option<String> {
        self.open_blockers_for_task(task_id).into_iter().next()
    }

    pub(super) fn record(&mut self, summary: impl Into<String>) {
        self.events.push(Event {
            at: now(),
            summary: summary.into(),
        });
        if self.events.len() > 200 {
            let keep_from = self.events.len() - 200;
            self.events.drain(0..keep_from);
        }
    }

    pub(super) fn ensure_worker(&self, id: &str) -> Result<(), XtaskError> {
        if self.workers.contains_key(id) {
            Ok(())
        } else {
            Err(XtaskError::validation(format!("Unknown worker `{id}`"))
                .with_recovery("Add it with `target/debug/xtask orchestrate worker <id>`."))
        }
    }

    pub(super) fn ensure_task(&self, id: &str) -> Result<(), XtaskError> {
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

    pub(super) fn remove_task_from_workers(&mut self, task_id: &str) {
        for worker in self.workers.values_mut() {
            if worker.active.as_deref() == Some(task_id) {
                worker.active = None;
            }
            worker.queue.retain(|queued| queued != task_id);
        }
    }

    pub(super) fn assigned_worker(&self, task_id: &str) -> Option<String> {
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

    pub(super) fn ensure_task_set(&self, id: &str) -> Result<(), XtaskError> {
        if self.task_sets.contains_key(id) {
            Ok(())
        } else {
            Err(
                XtaskError::validation(format!("Unknown task set `{id}`")).with_recovery(
                    "Create it with `target/debug/xtask orchestrate task-set create <id>`.",
                ),
            )
        }
    }

    pub(super) fn task_sets_sorted(&self) -> Vec<TaskSet> {
        let mut task_sets: Vec<_> = self.task_sets.values().cloned().collect();
        task_sets.sort_by(|a, b| a.id.cmp(&b.id));
        task_sets
    }

    pub(super) fn task_filter(
        &self,
        task_set: Option<&str>,
    ) -> Result<Option<BTreeSet<String>>, XtaskError> {
        let Some(task_set) = task_set else {
            return Ok(None);
        };
        self.ensure_task_set(task_set)?;
        Ok(Some(
            self.tasks
                .values()
                .filter(|task| task.task_sets.iter().any(|set| set == task_set))
                .map(|task| task.id.clone())
                .collect(),
        ))
    }
}

impl Blocker {
    pub(super) fn is_open(&self) -> bool {
        self.resolved_at.is_none()
    }
}

impl WorkerSlot {
    pub(super) fn new(id: String, role: WorkerRole) -> Self {
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

pub(super) fn resolve(ctx: &CommandContext, path: &Path) -> Result<PathBuf, XtaskError> {
    if path.is_absolute() {
        Ok(path.to_path_buf())
    } else {
        Ok(ctx.workspace_root()?.join(path))
    }
}

pub(super) fn display(ctx: &CommandContext, path: &Path) -> Result<String, XtaskError> {
    let root = ctx.workspace_root()?;
    Ok(path
        .strip_prefix(root)
        .unwrap_or(path)
        .display()
        .to_string())
}

pub(super) fn now() -> String {
    Utc::now().to_rfc3339()
}

pub(super) struct BoardLock {
    path: PathBuf,
}

impl BoardLock {
    pub(super) fn acquire(board_path: &Path) -> Result<Self, XtaskError> {
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
