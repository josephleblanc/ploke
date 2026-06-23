//! Canonical typed forward driver for Prototype 1 parent turns.
//!
//! This module owns the R0 -> R14 direct-edge loop used by the public
//! `prototype1-state` command. Operator/debug surfaces such as `walk` may step
//! the same edges incrementally, but this file is the batch terminal driver.

use ploke_core::EXECUTION_DEBUG_TARGET;

use crate::{
    cli::{InspectOutputFormat, Prototype1StateCommand},
    spec::PrepareError,
};

use super::super::{
    live_edges::{
        r0_to_r1, r1_to_r2a_or_r3, r3_to_r4a, r4a_to_r4b_or_r4c, r4b_to_r4c_genesis, r4c_to_r5,
        r5_to_r6, r6_to_r7, r7_to_r8, r8_to_r9, r9_to_r10, r10_to_r11, r11_to_r12, r12_to_r13,
        r13_to_r14,
    },
    typestate::{self, AsyncStepInput, Step, StepInput},
};

// ANCHOR: prototype1_run_to_terminal
/// Run a complete typed parent turn from command capture to final report.
pub(crate) async fn run_to_terminal(command: Prototype1StateCommand) -> Result<(), PrepareError> {
    let r0 = typestate::R0::new(command);
    let r1 = r0.advance(r0_to_r1)?;
    let span_campaign_id = r1.campaign_id().clone();
    let turn_span = tracing::info_span!(
        target: EXECUTION_DEBUG_TARGET,
        "prototype1.parent.turn",
        role = "parent",
        phase = "parent_turn",
        campaign = %span_campaign_id,
    );
    let _turn_entered = turn_span.enter();

    let r3 = match r1.advance(r1_to_r2a_or_r3)? {
        typestate::R1Branch::R2a(r2a) => {
            let typestate::R2aParts {
                collected,
                identity,
            } = r2a.into_parts();
            let command = collected.into_parts().command;
            match command.format {
                InspectOutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&identity).map_err(PrepareError::Serialize)?
                    );
                }
                InspectOutputFormat::Table => {
                    println!("prototype1 parent identity");
                    println!("{}", "-".repeat(40));
                    println!("campaign_id: {}", identity.campaign_id());
                    println!("parent_id: {}", identity.parent_id());
                    println!("node_id: {}", identity.node_id());
                    println!("generation: {}", identity.generation());
                    println!("branch_id: {}", identity.branch_id());
                    println!(
                        "artifact_branch: {}",
                        identity.artifact_branch().unwrap_or("-")
                    );
                }
            }
            return Ok(());
        }
        typestate::R1Branch::R3(r3) => r3,
    };
    let r4a = r3.advance(r3_to_r4a)?;
    let r4c = match r4a.advance(r4a_to_r4b_or_r4c)? {
        typestate::R4aStartupBranch::GenesisChecked(r4b) => r4b.advance(r4b_to_r4c_genesis)?,
        typestate::R4aStartupBranch::PredecessorReady(r4c) => r4c,
    };
    let r5 = r4c.advance(r4c_to_r5)?;
    let r6 = r5.advance_async(r5_to_r6).await?;
    let r7 = r6.advance(r6_to_r7)?;
    let r8 = r7.advance_async(r7_to_r8).await?;
    let r10 = r8.advance(r8_to_r9.then(r9_to_r10))?;
    let r11 = r10.advance_async(r10_to_r11).await?;
    let r13 = r11.advance(r11_to_r12.then(r12_to_r13))?;
    let _r14 = r13.advance(r13_to_r14)?;
    Ok(())
}
// ANCHOR_END: prototype1_run_to_terminal
