use crate::cli::{Prototype1StateAdvanceCommand, Prototype1StateCommand};
use crate::spec::PrepareError;
use tracing::instrument;

use crate::cli::prototype1_state::cli_facing::run_prototype1_state_turn;

impl Prototype1StateCommand {
    #[instrument(
        target = "ploke_exec",
        level = "debug",
        skip(self),
        fields(phase = "prototype1_state")
    )]
    pub async fn run(self) -> Result<(), PrepareError> {
        run_prototype1_state_turn(self, false, false).await
    }
}

impl Prototype1StateAdvanceCommand {
    #[instrument(
        target = "ploke_exec",
        level = "debug",
        skip(self),
        fields(phase = "prototype1_state")
    )]
    pub async fn run(self) -> Result<(), PrepareError> {
        run_prototype1_state_turn(
            self.state,
            self.capabilities.allow_live_api,
            self.capabilities.allow_git_changes(),
        )
        .await
    }
}
