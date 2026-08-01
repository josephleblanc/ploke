use crate::cli::{InspectOutputFormat, Prototype1LoopCommand, Prototype1SetupCommand};
use crate::spec::PrepareError;

use crate::cli::prototype1_state::cli_facing::{
    prepare_prototype1_parent_setup, preview_prototype1_parent_setup, print_prototype1_setup_plan,
    print_prototype1_setup_report, record_active_prototype1_monitor_target,
};

impl Prototype1LoopCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let _ = self;
        Err(PrepareError::InvalidBatchSelection {
            detail: "the legacy `loop prototype1` executor is disabled because it bypasses the durable controller session; use `prototype1-setup`, then `prototype1-state` for Continuous mode or `prototype1-step`/`walk step` for Step mode"
                .to_string(),
        })
    }
}

impl Prototype1SetupCommand {
    pub async fn run_setup(self) -> Result<(), PrepareError> {
        let format = self.input.format;
        if self.preview {
            let plan = preview_prototype1_parent_setup(&self.input)?;
            match format {
                InspectOutputFormat::Table => print_prototype1_setup_plan(&plan),
                InspectOutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&plan).map_err(PrepareError::Serialize)?
                    );
                }
            }
            return Ok(());
        }

        let setup =
            prepare_prototype1_parent_setup(&self.input, self.expect_plan_sha256.as_deref())?;
        record_active_prototype1_monitor_target(&setup.campaign_id, &setup.repo_root);

        match format {
            InspectOutputFormat::Table => print_prototype1_setup_report(&setup),
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&setup).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }
}
