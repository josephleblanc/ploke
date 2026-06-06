use crate::cli::{HistoryCommand, HistorySubcommand};
use crate::{campaign_manifest_path, spec::PrepareError};

use crate::cli::prototype1_state::cli_facing::{
    current_dir_as_repo_root, resolve_history_campaign, run_child_evidence, run_metric_slice,
    run_score_report, run_score_selection_review, run_selection_show,
};

impl HistoryCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let repo_root = match self.repo_root.clone() {
            Some(path) => path,
            None => current_dir_as_repo_root()?,
        };
        let campaign_id = resolve_history_campaign(&self, &repo_root)?;
        let manifest_path = campaign_manifest_path(&campaign_id)?;

        match self.command {
            HistorySubcommand::ChildEvidence(command) => {
                run_child_evidence(&campaign_id, &manifest_path, &command)
            }
            HistorySubcommand::Metrics(command) => {
                run_metric_slice(&campaign_id, &manifest_path, &command)
            }
            HistorySubcommand::Scores(command) => {
                run_score_report(&campaign_id, &manifest_path, &command)
            }
            HistorySubcommand::ScoreSelectionReview(command) => {
                run_score_selection_review(&campaign_id, &manifest_path, &command)
            }
            HistorySubcommand::SelectionShow(command) => {
                run_selection_show(&campaign_id, &manifest_path, &command)
            }
        }
    }
}
