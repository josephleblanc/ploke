use crate::cli::{LoopCommand, LoopSubcommand};
use crate::spec::PrepareError;

impl LoopCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            LoopSubcommand::Prototype1(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Setup(cmd) => cmd.run_setup().await,
            LoopSubcommand::Prototype1Doctor(cmd) => {
                crate::cli::prototype1_state::run::doctor(cmd).await
            }
            LoopSubcommand::Prototype1Prompt(cmd) => {
                crate::cli::prototype1_state::run::prompt(cmd).await
            }
            LoopSubcommand::Prototype1Continue(cmd) => {
                crate::cli::prototype1_state::run::resume(cmd).await
            }
            LoopSubcommand::Prototype1Step(cmd) => {
                crate::cli::prototype1_state::run::step(cmd).await
            }
            LoopSubcommand::Prototype1State(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Runner(cmd) => cmd.run().await,
            LoopSubcommand::Prototype1Harness(cmd) => cmd.run().await,
        }
    }
}
