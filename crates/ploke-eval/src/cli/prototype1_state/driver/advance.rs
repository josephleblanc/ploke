//! Batch adapter over the canonical one-edge Prototype 1 controller.

use std::path::PathBuf;

use ploke_core::EXECUTION_DEBUG_TARGET;

use crate::{cli::Prototype1StateCommand, spec::PrepareError};

use super::control::{ControlState, StepAdmission, advance_one};

// ANCHOR: prototype1_run_to_terminal
/// Run a complete typed parent turn from command capture to final report.
pub(crate) async fn run_to_terminal(command: Prototype1StateCommand) -> Result<(), PrepareError> {
    let repo_root = command
        .repo_root
        .clone()
        .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));
    let first = advance_one(
        &repo_root,
        ControlState::new(command),
        StepAdmission::continuous(),
    )
    .await
    .map_err(|failure| failure.error)?;
    let span_campaign_id = match &first.state {
        ControlState::R1(r1) => r1.campaign_id().clone(),
        state => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "first controlled Prototype 1 edge reached {} instead of R1",
                    state.phase()
                ),
            });
        }
    };
    let turn_span = tracing::info_span!(
        target: EXECUTION_DEBUG_TARGET,
        "prototype1.parent.turn",
        role = "parent",
        phase = "parent_turn",
        campaign = %span_campaign_id,
    );
    let _turn_entered = turn_span.enter();

    let mut state = first.state;
    let mut guard = 0_u8;
    while !state.complete() {
        guard = guard.saturating_add(1);
        if guard > 24 {
            return Err(PrepareError::InvalidBatchSelection {
                detail: "controlled Prototype 1 batch exceeded the R0-R14 step bound".to_string(),
            });
        }
        state = advance_one(&repo_root, state, StepAdmission::continuous())
            .await
            .map_err(|failure| failure.error)?
            .state;
    }
    Ok(())
}
// ANCHOR_END: prototype1_run_to_terminal
