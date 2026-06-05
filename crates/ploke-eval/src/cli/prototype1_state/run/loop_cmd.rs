use crate::cli::{InspectOutputFormat, Prototype1LoopCommand};
use crate::spec::PrepareError;

use crate::cli::prototype1_state::cli_facing::{
    Prototype1LoopControllerInput, prepare_prototype1_parent_setup, print_prototype1_loop_report,
    print_prototype1_setup_report, record_active_prototype1_monitor_target,
    run_prototype1_loop_controller,
};

impl Prototype1LoopCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let format = self.format;
        let input = Prototype1LoopControllerInput::from_command(&self)?;
        let report = run_prototype1_loop_controller(input).await?;

        match format {
            InspectOutputFormat::Table => print_prototype1_loop_report(&report),
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
        }

        Ok(())
    }

    pub async fn run_setup(self) -> Result<(), PrepareError> {
        let format = self.format;
        let setup = prepare_prototype1_parent_setup(&self)?;
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
