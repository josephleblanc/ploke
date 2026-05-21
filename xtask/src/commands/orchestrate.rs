//! Agent orchestration board commands.
//!
//! This command family is intentionally a small state mutator and renderer. It
//! stores worker slots, task queues, blockers, and generated worker packets so
//! the main agent can keep several disjoint lanes moving without turning a chat
//! thread into the task database.

use super::{CommandContext, XtaskError};

mod board;
mod commands;
mod lanes;
mod output;
mod packet;
mod reports;
mod status;
mod task_sets;
#[cfg(test)]
mod tests;
mod unblock;
mod usage;

pub use board::{
    Blocker, BlockerKind, Board, BoardStatus, Task, TaskSet, TaskState, WorkerRole, WorkerSlot,
};
use board::{BoardLock, DEFAULT_BOARD_PATH, DEFAULT_PACKET_DIR, display, now, resolve};
pub use commands::{
    AddTask, Assign, Block, BoardArg, Complete, Init, Packet, Review, Status, Usage, WorkerCommand,
};
pub use lanes::{LaneCommand, LaneSpec, LaneValidation};
pub use output::OrchestrateOutput;
pub use status::BoundedStatus;
pub use task_sets::TaskSetCommand;
pub use unblock::Unblock;
pub use usage::UsageSummary;

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
    /// Resolve a blocker and restore the task to a next state.
    Unblock(Unblock),
    /// Write a worker packet file from the current assignment.
    Packet(Packet),
    /// Manage named task sets.
    #[command(subcommand)]
    TaskSet(TaskSetCommand),
    /// Show local command usage counters.
    Usage(Usage),
    /// Define and validate lane-owned edit surfaces.
    #[command(subcommand)]
    Lane(LaneCommand),
}

impl Orchestrate {
    /// Execute an orchestration command.
    pub fn execute(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        if matches!(self, Self::Usage(_)) {
            usage::record_usage_best_effort(ctx, self.board_arg(), self.usage_key());
            return self.dispatch(ctx);
        }

        let output = self.dispatch(ctx)?;
        usage::record_usage_best_effort(ctx, self.board_arg(), self.usage_key());
        Ok(output)
    }

    fn dispatch(&self, ctx: &CommandContext) -> Result<OrchestrateOutput, XtaskError> {
        match self {
            Self::Init(cmd) => cmd.execute(ctx),
            Self::Status(cmd) => cmd.execute(ctx),
            Self::Worker(cmd) => cmd.execute(ctx),
            Self::Add(cmd) => cmd.execute(ctx),
            Self::Assign(cmd) => cmd.execute(ctx),
            Self::Complete(cmd) => cmd.execute(ctx),
            Self::Review(cmd) => cmd.execute(ctx),
            Self::Block(cmd) => cmd.execute(ctx),
            Self::Unblock(cmd) => cmd.execute(ctx),
            Self::Packet(cmd) => cmd.execute(ctx),
            Self::TaskSet(cmd) => cmd.execute(ctx),
            Self::Usage(cmd) => cmd.execute(ctx),
            Self::Lane(cmd) => cmd.execute(ctx),
        }
    }

    fn board_arg(&self) -> &BoardArg {
        match self {
            Self::Init(cmd) => &cmd.board,
            Self::Status(cmd) => &cmd.board,
            Self::Worker(cmd) => &cmd.board,
            Self::Add(cmd) => &cmd.board,
            Self::Assign(cmd) => &cmd.board,
            Self::Complete(cmd) => &cmd.board,
            Self::Review(cmd) => &cmd.board,
            Self::Block(cmd) => &cmd.board,
            Self::Unblock(cmd) => &cmd.board,
            Self::Packet(cmd) => &cmd.board,
            Self::TaskSet(cmd) => cmd.board_arg(),
            Self::Usage(cmd) => &cmd.board,
            Self::Lane(cmd) => cmd.board_arg(),
        }
    }

    fn usage_key(&self) -> &'static str {
        match self {
            Self::Init(_) => "init",
            Self::Status(cmd) if cmd.task_set.is_some() => "status --set",
            Self::Status(cmd) if cmd.brief => "status --brief",
            Self::Status(_) => "status",
            Self::Worker(_) => "worker",
            Self::Add(_) => "add",
            Self::Assign(_) => "assign",
            Self::Complete(cmd) if cmd.summary.is_some() => "complete --summary",
            Self::Complete(_) => "complete",
            Self::Review(_) => "review",
            Self::Block(_) => "block",
            Self::Unblock(_) => "unblock",
            Self::Packet(_) => "packet",
            Self::TaskSet(cmd) => cmd.usage_key(),
            Self::Usage(_) => "usage",
            Self::Lane(cmd) => cmd.usage_key(),
        }
    }
}
