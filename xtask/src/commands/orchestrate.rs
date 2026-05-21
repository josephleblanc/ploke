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
#[cfg(test)]
mod tests;

pub use board::{
    Blocker, BlockerKind, Board, BoardStatus, Task, TaskState, WorkerRole, WorkerSlot,
};
use board::{BoardLock, DEFAULT_BOARD_PATH, DEFAULT_PACKET_DIR, display, now, resolve};
pub use commands::{
    AddTask, Assign, Block, BoardArg, Complete, Init, Packet, Review, Status, WorkerCommand,
};
pub use lanes::{LaneCommand, LaneSpec, LaneValidation};
pub use output::OrchestrateOutput;

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
