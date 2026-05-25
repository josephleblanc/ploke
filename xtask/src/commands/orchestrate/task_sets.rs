use crate::commands::{CommandContext, XtaskError};

use super::{Board, BoardArg, BoardLock, OrchestrateOutput, Task, TaskSet, now, resolve};

/// Task-set commands.
#[derive(Debug, Clone, clap::Subcommand)]
pub enum TaskSetCommand {
    /// Create a named task set.
    Create(CreateTaskSet),
    /// Add a task to a set.
    Add(AddTaskToSet),
    /// Remove a task from a set.
    Remove(RemoveTaskFromSet),
    /// List task sets.
    List(ListTaskSets),
}

impl TaskSetCommand {
    pub(super) fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        match self {
            Self::Create(cmd) => cmd.execute(ctx),
            Self::Add(cmd) => cmd.execute(ctx),
            Self::Remove(cmd) => cmd.execute(ctx),
            Self::List(cmd) => cmd.execute(ctx),
        }
    }

    pub(super) fn board_arg(&self) -> &BoardArg {
        match self {
            Self::Create(cmd) => &cmd.board,
            Self::Add(cmd) => &cmd.board,
            Self::Remove(cmd) => &cmd.board,
            Self::List(cmd) => &cmd.board,
        }
    }

    pub(super) fn usage_key(&self) -> &'static str {
        match self {
            Self::Create(_) => "task-set create",
            Self::Add(_) => "task-set add",
            Self::Remove(_) => "task-set remove",
            Self::List(_) => "task-set list",
        }
    }
}

/// Create a named task set.
#[derive(Debug, Clone, clap::Args)]
pub struct CreateTaskSet {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Task-set id.
    pub(super) id: String,
    /// Optional description.
    #[arg(long)]
    pub(super) description: Option<String>,
}

/// Add a task to a set.
#[derive(Debug, Clone, clap::Args)]
pub struct AddTaskToSet {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Task-set id.
    pub(super) set: String,
    /// Task id.
    pub(super) task: String,
}

/// Remove a task from a set.
#[derive(Debug, Clone, clap::Args)]
pub struct RemoveTaskFromSet {
    #[command(flatten)]
    pub(super) board: BoardArg,
    /// Task-set id.
    pub(super) set: String,
    /// Task id.
    pub(super) task: String,
}

/// List task sets.
#[derive(Debug, Clone, clap::Args)]
pub struct ListTaskSets {
    #[command(flatten)]
    pub(super) board: BoardArg,
}

impl CreateTaskSet {
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, self.board.path())?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load_or_new(&path)?;
        if board.task_sets.contains_key(&self.id) {
            return Err(
                XtaskError::validation(format!("Task set `{}` already exists", self.id))
                    .with_recovery("Use a new task-set id."),
            );
        }
        let task_set = TaskSet {
            id: self.id.clone(),
            description: self.description.clone(),
            created_at: now(),
        };
        board
            .task_sets
            .insert(task_set.id.clone(), task_set.clone());
        board.record(format!("task set {} created", task_set.id));
        board.save(&path)?;
        Ok(OrchestrateOutput::TaskSet { task_set })
    }
}

impl AddTaskToSet {
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, self.board.path())?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        board.ensure_task_set(&self.set)?;
        board.ensure_task(&self.task)?;
        let task = board.tasks.get_mut(&self.task).expect("checked");
        insert_membership(task, &self.set);
        let task = task.clone();
        board.record(format!("task {} added to set {}", self.task, self.set));
        board.save(&path)?;
        Ok(OrchestrateOutput::TaskSetMembership { task })
    }
}

impl RemoveTaskFromSet {
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, self.board.path())?;
        let _lock = BoardLock::acquire(&path)?;
        let mut board = Board::load(&path)?;
        board.ensure_task_set(&self.set)?;
        board.ensure_task(&self.task)?;
        let task = board.tasks.get_mut(&self.task).expect("checked");
        task.task_sets.retain(|set| set != &self.set);
        let task = task.clone();
        board.record(format!("task {} removed from set {}", self.task, self.set));
        board.save(&path)?;
        Ok(OrchestrateOutput::TaskSetMembership { task })
    }
}

impl ListTaskSets {
    fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        let path = resolve(ctx, self.board.path())?;
        let board = Board::load(&path)?;
        Ok(OrchestrateOutput::TaskSets {
            task_sets: board.task_sets_sorted(),
        })
    }
}

fn insert_membership(task: &mut Task, set: &str) {
    if !task.task_sets.iter().any(|existing| existing == set) {
        task.task_sets.push(set.to_string());
        task.task_sets.sort();
    }
}
