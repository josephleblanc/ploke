use crate::commands::{CommandContext, XtaskError};

use super::board::TaskPlacement;
use super::{Board, BoardArg, BoardLock, OrchestrateOutput, resolve};

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
        let next = self.next_placement()?;
        let (task, blocker) = board.resolve_blocker(
            &self.blocker,
            self.summary.clone(),
            self.evidence.clone(),
            next,
        )?;
        board.save(&path)?;
        Ok(OrchestrateOutput::Unblocked { task, blocker })
    }

    fn next_placement(&self) -> Result<TaskPlacement, XtaskError> {
        match self.next {
            UnblockNext::NotStarted => Ok(TaskPlacement::NotStarted),
            UnblockNext::Queued | UnblockNext::Active => {
                let worker = self.worker.clone().ok_or_else(|| {
                    XtaskError::validation("--worker is required for queued or active unblock")
                        .with_recovery("Pass `--worker <id>` or use `--next not-started`.")
                })?;
                if self.next == UnblockNext::Queued {
                    Ok(TaskPlacement::Queued { worker })
                } else {
                    Ok(TaskPlacement::Active { worker })
                }
            }
        }
    }
}
