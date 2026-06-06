use crate::cli::Prototype1StateCommand;
use crate::spec::PrepareError;
use tracing::instrument;

use crate::cli::prototype1_state::cli_facing::{
    record_failed_successor_turn, run_prototype1_state_turn,
};

impl Prototype1StateCommand {
    #[instrument(
        target = "ploke_exec",
        level = "debug",
        skip(self),
        fields(phase = "prototype1_state")
    )]
    pub async fn run(self) -> Result<(), PrepareError> {
        let handoff_invocation = self.handoff_invocation.clone();
        let result = run_prototype1_state_turn(self).await;
        if let Err(error) = &result
            && let Some(invocation_path) = handoff_invocation.as_deref()
        {
            record_failed_successor_turn(invocation_path, error);
        }
        result
    }
}
