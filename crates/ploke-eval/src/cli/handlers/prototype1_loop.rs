use crate::cli::prototype1_state::run;
use crate::cli::{LoopCommand, LoopSubcommand};
use crate::spec::PrepareError;

impl LoopCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            LoopSubcommand::Prototype1(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Setup(cmd) => cmd.run_setup().await,
            LoopSubcommand::Prototype1Doctor(cmd) => run::doctor(cmd).await,
            LoopSubcommand::Prototype1Prompt(cmd) => run::prompt(cmd).await,
            LoopSubcommand::Prototype1Continue(cmd) => run::resume(cmd).await,
            LoopSubcommand::Prototype1Step(cmd) => run::step(cmd).await,
            LoopSubcommand::Prototype1State(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1StateWalk(cmd) => run::state_walk(cmd).await,
            LoopSubcommand::Prototype1Runner(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Harness(cmd) => cmd.run().await,
        }
    }
}
