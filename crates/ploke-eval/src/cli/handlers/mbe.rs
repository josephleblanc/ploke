use std::path::{Path, PathBuf};

use crate::cli::{
    MbeCampaignCandidatesCommand, MbeCommand, MbeRequestArgs, MbeRunCampaignCandidateCommand,
    MbeRunCommand, MbeRunsCommand, MbeSubcommand, MbeVerdictCommand, MbeWriteConfigCommand,
};
use crate::spec::PrepareError;

impl MbeCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        match self.command {
            MbeSubcommand::Run(cmd) => cmd.run(),
            MbeSubcommand::CampaignCandidates(cmd) => cmd.run(),
            MbeSubcommand::RunCampaignCandidate(cmd) => cmd.run(),
            MbeSubcommand::Runs(cmd) => cmd.run(),
            MbeSubcommand::WriteConfig(cmd) => cmd.run(),
            MbeSubcommand::Verdict(cmd) => cmd.run(),
        }
    }
}

impl MbeRequestArgs {
    fn into_request(self) -> Result<crate::mbe::Request, PrepareError> {
        let options = mbe_options_with_workers(self.workers);

        match (self.run, self.instance) {
            (Some(run), None) => {
                if self.attempt.is_some() {
                    return Err(PrepareError::InvalidMbeRequest {
                        detail: "--attempt requires --instance, not --run".to_string(),
                    });
                }
                crate::mbe::Request::from_manifest(
                    run,
                    self.submission,
                    self.output_dir,
                    self.repo_dir,
                    options,
                )
            }
            (None, Some(instance)) => crate::mbe::Request::from_instance(
                &instance,
                self.attempt,
                self.submission,
                self.output_dir,
                self.repo_dir,
                options,
            ),
            (Some(_), Some(_)) => Err(PrepareError::InvalidMbeRequest {
                detail: "specify only one of --run or --instance".to_string(),
            }),
            (None, None) => Err(PrepareError::InvalidMbeRequest {
                detail: "specify --run <path> or --instance <id>".to_string(),
            }),
        }
    }
}

fn mbe_options_with_workers(workers: u32) -> crate::mbe::Options {
    let mut options = crate::mbe::Options::default();
    options.workers = crate::mbe::Workers {
        general: workers,
        build_image: workers,
        run_instance: workers,
    };
    options
}

impl MbeRunsCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        for candidate in crate::mbe::run_candidates(&self.instance)? {
            let latest = if candidate.latest { "latest" } else { "" };
            let submission = candidate
                .submission_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "-".to_string());
            println!(
                "{}\t{}\t{:?}\t{:?}\t{}\t{}\t{}",
                candidate.attempt,
                latest,
                candidate.execution_status,
                candidate.submission_status,
                candidate.run_id,
                candidate.run_manifest.display(),
                submission
            );
        }
        Ok(())
    }
}

impl MbeCampaignCandidatesCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        println!(
            "generation\tnode\tparent\tprimary_instance\tcohort\tnonempty\toracle\tbytes\tlines\tinstances\ttreatment_campaign"
        );
        for candidate in crate::mbe::campaign_candidates(&self.campaign, self.nonempty_only)? {
            let oracle = candidate.oracle_eligible_instance_count()?;
            println!(
                "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
                candidate.generation(),
                candidate.node_id(),
                candidate.parent_node_id().unwrap_or("-"),
                candidate.primary_instance_id(),
                candidate.cohort_size(),
                candidate.nonempty_instance_count(),
                oracle,
                candidate.total_fix_patch_bytes(),
                candidate.total_fix_patch_lines(),
                candidate.instance_ids().join(","),
                candidate.treatment_campaign_id()?,
            );
        }
        Ok(())
    }
}

impl MbeRunCampaignCandidateCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let candidate = crate::mbe::campaign_candidate_by_node(&self.campaign, &self.node)?;
        let request = crate::mbe::CohortRequest::from_campaign_candidate(
            &candidate,
            self.output_dir,
            self.repo_dir,
            mbe_options_with_workers(self.workers),
        )?;
        let run = request.run_harness(self.python)?;
        println!("node: {}", candidate.node_id());
        println!("cohort_instances: {}", candidate.cohort_size());
        println!("config: {}", run.written.path.display());
        println!("report: {}", run.written.report_path.display());
        println!("command: {}", run.invocation.command_line());
        println!(
            "submitted/completed/resolved/unresolved: {}/{}/{}/{}",
            run.report.submitted_instances,
            run.report.completed_instances,
            run.report.resolved_instances,
            run.report.unresolved_instances
        );
        println!("instance\tverdict\tdiagnostic\tusable\tinstance_report");
        for evaluation in run.evaluations {
            println!(
                "{}\t{}\t{}\t{}\t{}",
                evaluation.evidence.instance_id,
                evaluation.evidence.verdict,
                evaluation.diagnostic,
                evaluation.usable_for_selection,
                evaluation.instance_report_path.display()
            );
        }
        Ok(())
    }
}

impl MbeRunCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let request = self.request.into_request()?;
        let run = request.run_harness(self.python)?;
        println!("config: {}", run.written.path.display());
        println!("report: {}", run.written.report_path.display());
        println!("command: {}", run.invocation.command_line());
        println!(
            "verdict: {}\t{}",
            run.evidence.report_id, run.evidence.verdict
        );
        println!(
            "instance_report: {}",
            run.evaluation.instance_report_path.display()
        );
        println!("diagnostic: {}", run.evaluation.diagnostic);
        println!(
            "usable_for_selection: {}",
            run.evaluation.usable_for_selection
        );
        Ok(())
    }
}

impl MbeWriteConfigCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let request = self.request.into_request()?;
        let written = request.write_config()?;
        let invocation = written.harness_invocation(self.python);
        println!("config: {}", written.path.display());
        println!("report: {}", written.report_path.display());
        println!("command: {}", invocation.command_line());
        Ok(())
    }
}

impl MbeVerdictCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let prepared = crate::spec::PreparedSingleRun::load_manifest(self.run)?;
        let report = crate::mbe::FinalReport::load(&self.report)?;
        let evidence = crate::mbe::OracleEvidence::from_report(&prepared, self.report, &report)?;
        let layout = crate::mbe::Layout::under(
            evidence
                .report_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .to_path_buf(),
            PathBuf::new(),
        );
        let evaluation = crate::mbe::OracleEvaluation::from_evidence(&prepared, evidence, &layout)?;
        println!(
            "{}\t{}\t{}\t{}",
            evaluation.evidence.report_id,
            evaluation.evidence.verdict,
            evaluation.diagnostic,
            evaluation.usable_for_selection
        );
        Ok(())
    }
}
