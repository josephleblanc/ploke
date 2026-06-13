use crate::prelude::*;

use crate::campaign::{
    CampaignManifest, CampaignOverrides, CampaignValidationCheck, ResolvedCampaignConfig,
    adopt_campaign_manifest_from_closure_state, adopt_campaign_manifest_from_registry,
    apply_campaign_overrides, campaign_closure_state_path, campaign_manifest_path, list_campaigns,
    render_resolved_campaign_config, resolve_campaign_config, save_campaign_manifest,
    validate_campaign_config,
};
use crate::cli::{
    CampaignCommand, CampaignExportSubmissionsCommand, CampaignInitCommand, CampaignListCommand,
    CampaignOverrideArgs, CampaignShowCommand, CampaignSubcommand, CampaignValidateCommand,
    InspectOutputFormat,
};
use crate::closure::{ClosureClass, closure_state_path, load_closure_state};
use crate::run_history::{RunDirPreference, preferred_run_dir_for_instance};
use crate::runner::MultiSweBenchSubmissionRecord;

impl CampaignCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        match self.command {
            CampaignSubcommand::List(cmd) => cmd.run().await,
            CampaignSubcommand::Init(cmd) => cmd.run().await,
            CampaignSubcommand::Show(cmd) => cmd.run().await,
            CampaignSubcommand::Validate(cmd) => cmd.run().await,
            CampaignSubcommand::ExportSubmissions(cmd) => cmd.run().await,
        }
    }
}
impl CampaignOverrideArgs {
    pub(crate) fn into_overrides(self) -> CampaignOverrides {
        CampaignOverrides {
            dataset_keys: self.dataset_key,
            dataset_files: self.dataset,
            model_id: self.model_id,
            provider_slug: self.provider,
            route_source: self.route_source,
            required_procedures: self.required_procedure,
            instances_root: self.instances_root,
            batches_root: self.batches_root,
        }
    }
}

impl CampaignInitCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let path = campaign_manifest_path(&self.campaign)?;
        if path.exists() && !self.force {
            return Err(PrepareError::DatabaseSetup {
                phase: "campaign_init",
                detail: format!(
                    "campaign manifest already exists at '{}' (pass --force to overwrite)",
                    path.display()
                ),
            });
        }

        let overrides = self.overrides.into_overrides();
        let mut manifest = if self.from_closure_state {
            adopt_campaign_manifest_from_closure_state(&self.campaign)?
        } else if self.from_registry {
            adopt_campaign_manifest_from_registry(&self.campaign)?
        } else {
            let closure_path = campaign_closure_state_path(&self.campaign)?;
            if closure_path.exists() && overrides.is_empty() {
                return Err(PrepareError::DatabaseSetup {
                    phase: "campaign_init",
                    detail: format!(
                        "closure state exists at '{}' but no manifest exists; pass --from-closure-state to adopt it",
                        closure_path.display()
                    ),
                });
            }
            CampaignManifest::new(self.campaign.clone())
        };
        apply_campaign_overrides(&mut manifest, &overrides)?;
        let saved_path = save_campaign_manifest(&manifest)?;
        let resolved = resolve_campaign_config(&self.campaign, &CampaignOverrides::default())?;

        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_resolved_campaign_config(&resolved));
                println!("manifest: {}", saved_path.display());
            }
            InspectOutputFormat::Json => {
                let payload = serde_json::json!({
                    "manifest_path": saved_path,
                    "config": resolved,
                });
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignListCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let campaigns = list_campaigns()?;
        match self.format {
            InspectOutputFormat::Table => {
                if campaigns.is_empty() {
                    println!("campaigns: none");
                } else {
                    println!("campaigns");
                    for campaign in &campaigns {
                        let status = match (campaign.has_manifest, campaign.has_closure_state) {
                            (true, true) => "manifest+closure",
                            (true, false) => "manifest-only",
                            (false, true) => "closure-only",
                            (false, false) => "empty",
                        };
                        println!("  - {} | {}", campaign.campaign_id, status);
                    }
                }
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&campaigns).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignShowCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_resolved_campaign_config(&resolved));
                println!(
                    "manifest: {}",
                    campaign_manifest_path(&self.campaign)?.display()
                );
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&resolved).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignValidateCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let resolved = resolve_campaign_config(&self.campaign, &self.overrides.into_overrides())?;
        let checks = validate_campaign_config(&resolved).await?;
        match self.format {
            InspectOutputFormat::Table => {
                println!("{}", render_resolved_campaign_config(&resolved));
                println!("\nvalidation");
                for check in &checks {
                    println!("  - {}: {}", check.label, check.detail);
                }
            }
            InspectOutputFormat::Json => {
                let payload = CampaignValidationView {
                    config: resolved,
                    checks,
                };
                println!(
                    "{}",
                    serde_json::to_string_pretty(&payload).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}

impl CampaignExportSubmissionsCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let state = load_closure_state(&self.campaign)?;
        let records = collect_campaign_submission_records(&state, self.nonempty_only)?;
        let complete_eval_rows = state
            .instances
            .iter()
            .filter(|row| row.eval_status == ClosureClass::Complete)
            .count();
        let empty_patch_rows = count_campaign_empty_patch_rows(&state)?;
        let output_path = self
            .output
            .unwrap_or(default_campaign_submission_export_path(
                &self.campaign,
                self.nonempty_only,
            )?);

        if let Some(parent) = output_path.parent() {
            fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
                path: parent.to_path_buf(),
                source,
            })?;
        }

        let mut jsonl = String::new();
        for record in &records {
            let line = serde_json::to_string(record).map_err(PrepareError::Serialize)?;
            jsonl.push_str(&line);
            jsonl.push('\n');
        }
        fs::write(&output_path, jsonl).map_err(|source| PrepareError::WriteManifest {
            path: output_path.clone(),
            source,
        })?;

        let summary = CampaignSubmissionExportSummary {
            campaign_id: self.campaign,
            closure_state_path: closure_state_path(&state.campaign_id)?,
            output_path,
            exported_records: records.len(),
            nonempty_only: self.nonempty_only,
            complete_eval_rows,
            empty_patch_rows_skipped: if self.nonempty_only {
                empty_patch_rows
            } else {
                0
            },
            failed_eval_rows: state
                .instances
                .iter()
                .filter(|row| row.eval_status == ClosureClass::Failed)
                .count(),
        };

        match self.format {
            InspectOutputFormat::Table => {
                println!(
                    "campaign {} | exported {} submission records{}",
                    summary.campaign_id,
                    summary.exported_records,
                    if summary.nonempty_only {
                        " (non-empty only)"
                    } else {
                        ""
                    }
                );
                println!("closure: {}", summary.closure_state_path.display());
                println!("output: {}", summary.output_path.display());
                println!("complete eval rows: {}", summary.complete_eval_rows);
                println!(
                    "empty patch rows skipped: {}",
                    summary.empty_patch_rows_skipped
                );
                println!("failed eval rows: {}", summary.failed_eval_rows);
            }
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&summary).map_err(PrepareError::Serialize)?
                );
            }
        }
        Ok(())
    }
}
#[derive(Debug, Serialize)]
struct CampaignValidationView {
    config: ResolvedCampaignConfig,
    checks: Vec<CampaignValidationCheck>,
}

#[derive(Debug, Clone, Serialize)]
struct CampaignSubmissionExportSummary {
    campaign_id: String,
    closure_state_path: PathBuf,
    output_path: PathBuf,
    exported_records: usize,
    complete_eval_rows: usize,
    empty_patch_rows_skipped: usize,
    failed_eval_rows: usize,
    nonempty_only: bool,
}
pub(crate) fn default_campaign_submission_export_path(
    campaign_id: &str,
    nonempty_only: bool,
) -> Result<PathBuf, PrepareError> {
    let file_name = if nonempty_only {
        "multi-swe-bench-submission.nonempty.jsonl"
    } else {
        "multi-swe-bench-submission.jsonl"
    };
    Ok(crate::layout::campaigns_dir()?
        .join(campaign_id)
        .join(file_name))
}

fn collect_campaign_submission_records(
    state: &crate::closure::ClosureState,
    nonempty_only: bool,
) -> Result<Vec<MultiSweBenchSubmissionRecord>, PrepareError> {
    let mut records = Vec::new();
    for row in &state.instances {
        if row.eval_status != ClosureClass::Complete {
            continue;
        }
        let record = load_submission_record_for_row(state, row)?;
        if nonempty_only && record.fix_patch.trim().is_empty() {
            continue;
        }
        records.push(record);
    }
    Ok(records)
}

fn count_campaign_empty_patch_rows(
    state: &crate::closure::ClosureState,
) -> Result<usize, PrepareError> {
    let mut count = 0;
    for row in &state.instances {
        if row.eval_status != ClosureClass::Complete {
            continue;
        }
        let record = load_submission_record_for_row(state, row)?;
        if record.fix_patch.trim().is_empty() {
            count += 1;
        }
    }
    Ok(count)
}

fn load_submission_record_for_row(
    state: &crate::closure::ClosureState,
    row: &crate::closure::ClosureInstanceRow,
) -> Result<MultiSweBenchSubmissionRecord, PrepareError> {
    let path = if let Some(path) = row.artifacts.msb_submission.as_ref() {
        path.clone()
    } else {
        let instance_root = state.config.instances_root.join(&row.instance_id);
        let run_dir = preferred_run_dir_for_instance(
            &state.config.instances_root,
            &row.instance_id,
            RunDirPreference::PreferTreatmentWithSubmission,
        )?
        .unwrap_or(instance_root);
        run_dir.join("multi-swe-bench-submission.jsonl")
    };
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    serde_json::from_str(text.trim()).map_err(|source| PrepareError::ParseManifest { path, source })
}
