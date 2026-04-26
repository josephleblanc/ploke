use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    str::FromStr,
    thread,
    time::{Duration, SystemTime},
};

use chrono::Utc;
use ploke_core::EXECUTION_DEBUG_TARGET;
use ploke_llm::{ModelId, ProviderKey};
use ploke_tui::tools::ToolName;
use serde::{Deserialize, Serialize};
use tracing::{debug, instrument};

use crate::{
    BranchDisposition, BranchEvaluationInput, BranchEvaluationResult, CampaignManifest,
    CampaignOverrides, ClosureClass, EvalBudget, EvalCampaignPolicy, OperationalRunMetrics,
    OutputMode, PrepareMsbBatchRequest, PrepareWrite, PreparedMsbBatch, ProtocolCampaignPolicy,
    RegistryDatasetSource, ResolvedCampaignConfig, batches_dir,
    campaign::campaign_closure_state_path,
    campaign_manifest_path,
    cli::{
        InspectOutputFormat, Prototype1BranchApplyCommand, Prototype1BranchEvaluateCommand,
        Prototype1BranchRestoreCommand, Prototype1BranchSelectCommand, Prototype1BranchShowCommand,
        Prototype1BranchStatusCommand, Prototype1LoopCommand, Prototype1LoopStopAfter,
        Prototype1MonitorCommand, Prototype1MonitorPeekCommand, Prototype1MonitorSubcommand,
        Prototype1MonitorWatchCommand, Prototype1RunnerCommand, Prototype1StateCommand,
        Prototype1StateStopAfter, TimingTrace, advance_eval_closure, advance_protocol_closure,
        default_batch_id, pending_prototype1_stages, persist_intervention_apply_for_record,
        persist_intervention_synthesis_for_record, persist_issue_detection_for_record,
        print_issue_case_block,
        prototype1_process::{
            Prototype1NodeExecutionOutcome, execute_prototype1_runner_invocation,
            execute_prototype1_runner_node, persist_prototype1_buildable_child_artifact,
            record_prototype1_successor_completion, record_prototype1_successor_ready,
            run_prototype1_branch_evaluation, run_prototype1_branch_evaluation_via_child,
            spawn_and_handoff_prototype1_successor, validate_prototype1_successor_continuation,
        },
        prototype1_state::{
            backend::{GitWorktreeBackend, WorkspaceBackend},
            c1::{C1, MaterializeBranch},
            c2::BuildChild,
            c3::SpawnChild,
            c4::ObserveChild,
            identity::{
                ParentIdentity, load_parent_identity_optional, parent_identity_commit_message,
                parent_identity_relpath, write_parent_identity,
            },
            journal::{PrototypeJournal, prototype1_transition_journal_path},
        },
        resolve_batch_manifest, resolve_protocol_model_id, resolve_protocol_provider_slug,
        serde_name, write_json_file_pretty, yes_no,
    },
    evaluate_branch, instances_dir,
    intervention::{
        ArtifactEdit, Intervention, InterventionApplyInput, InterventionCandidate,
        InterventionSpec, IssueCase, Outcome, Prototype1BranchRegistry,
        Prototype1ContinuationDecision, Prototype1ContinuationDisposition, Prototype1NodeRecord,
        Prototype1NodeStatus, Prototype1RunnerResult, Prototype1SchedulerState,
        Prototype1SearchPolicy, ValidationPolicy, decide_node_successor_continuation,
        execute_intervention_apply, load_node_record, load_or_default_branch_registry,
        load_or_default_scheduler_state, load_runner_request, load_runner_result,
        mark_treatment_branch_applied, prototype1_branch_registry_path, prototype1_scheduler_path,
        record_continuation_decision, record_synthesized_branches,
        register_treatment_evaluation_node, resolve_treatment_branch, restore_treatment_branch,
        select_primary_issue, select_treatment_branch, treatment_branch_id,
        update_scheduler_policy,
    },
    load_campaign_manifest, load_closure_state,
    model_registry::resolve_model_for_run,
    protocol::load_protocol_aggregate,
    provider_prefs::load_provider_for_model,
    record::read_compressed_record,
    repos_dir, resolve_campaign_config, save_campaign_manifest,
    spec::PrepareError,
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
}

struct Prototype1LoopControllerInput {
    stop_after: Prototype1LoopStopAfter,
    dry_run: bool,
    stop_on_error: bool,
    protocol_model_id: Option<String>,
    protocol_provider: Option<String>,
    search_policy: Prototype1SearchPolicy,
    source_campaign: Option<String>,
    source_branch_id: Option<String>,
    repo_root: PathBuf,
    trace_path: PathBuf,
    batch_id: String,
    batch_manifest: PathBuf,
    prepared_instances: Vec<String>,
    campaign: Prototype1LoopCampaign,
}

impl Prototype1LoopControllerInput {
    fn from_command(command: &Prototype1LoopCommand) -> Result<Self, PrepareError> {
        let (batch_manifest, prepared_batch) = prepare_or_load_prototype1_batch(command)?;
        let campaign = prepare_prototype1_loop_campaign(command, &prepared_batch)?;
        let trace_path = prototype1_trace_path(&campaign.manifest_path);
        let repo_root = std::env::current_dir().map_err(|source| PrepareError::ReadManifest {
            path: PathBuf::from("."),
            source,
        })?;

        Ok(Self {
            stop_after: command.stop_after,
            dry_run: command.dry_run,
            stop_on_error: command.stop_on_error,
            protocol_model_id: command.protocol_model_id.clone(),
            protocol_provider: command.protocol_provider.clone(),
            search_policy: Prototype1SearchPolicy {
                max_generations: command.max_generations,
                max_total_nodes: command.max_total_nodes,
                stop_on_first_keep: command.stop_on_first_keep,
                require_keep_for_continuation: command.require_keep_for_continuation,
            },
            source_campaign: command.source_campaign.clone(),
            source_branch_id: command.source_branch_id.clone(),
            repo_root,
            trace_path,
            batch_id: prepared_batch.batch_id.clone(),
            batch_manifest,
            prepared_instances: prepared_batch.instances.clone(),
            campaign,
        })
    }
}

async fn run_prototype1_loop_controller(
    input: Prototype1LoopControllerInput,
) -> Result<Prototype1LoopReport, PrepareError> {
    let _run_scope = TimingTrace::scope("loop.prototype1.run");
    let batch_manifest = input.batch_manifest;
    let campaign = input.campaign;
    let trace_path = input.trace_path;
    let branch_registry_path = prototype1_branch_registry_path(&campaign.manifest_path);
    let scheduler_path = prototype1_scheduler_path(&campaign.manifest_path);
    let search_policy = input.search_policy;
    let mut scheduler = update_scheduler_policy(
        &campaign.campaign_id,
        &campaign.manifest_path,
        search_policy.clone(),
    )?;

    let mut baseline_instances = Vec::new();
    let mut selected_targets = Vec::new();
    let mut staged_nodes = Vec::new();
    let mut branch_evaluations = Vec::new();
    let mut selected_next_branch_id = None;
    let mut continuation_decision = None;
    let mut protocol_failures = Vec::new();
    let mut protocol_task_instances = Vec::new();
    let intervention_repo_root = input.repo_root;

    if let (Some(source_campaign), Some(source_branch_id)) = (
        input.source_campaign.as_deref(),
        input.source_branch_id.as_deref(),
    ) {
        let _scope = TimingTrace::scope("loop.prototype1.materialize_source_branch");
        let source_manifest_path = campaign_manifest_path(source_campaign)?;
        let resolved =
            resolve_treatment_branch(source_campaign, &source_manifest_path, source_branch_id)?;
        ensure_treatment_branch_materialized(
            source_campaign,
            &source_manifest_path,
            &resolved,
            &intervention_repo_root,
        )?;
    }

    let mut eval_policy = campaign.resolved.eval.clone();
    if input.stop_on_error {
        eval_policy.stop_on_error = true;
    }
    let eval_report = {
        let _scope = TimingTrace::scope("loop.prototype1.advance_eval_closure");
        advance_eval_closure(&campaign.resolved, &eval_policy, false).await?
    };

    if input.stop_after >= Prototype1LoopStopAfter::BaselineProtocol {
        let mut protocol_policy = campaign.resolved.protocol.clone();
        if input.stop_on_error {
            protocol_policy.stop_on_error = true;
        }
        let protocol_report = {
            let _scope = TimingTrace::scope("loop.prototype1.advance_protocol_closure");
            advance_protocol_closure(&campaign.resolved, &protocol_policy, false).await?
        };
        protocol_failures = protocol_report.failures;
        protocol_task_instances = protocol_report
            .selected_runs
            .into_iter()
            .map(|plan| plan.instance_id)
            .collect();
    }

    let closure = load_closure_state(&campaign.campaign_id)?;

    for row in &closure.instances {
        let protocol_failure = protocol_failures
            .iter()
            .find(|failure| failure.starts_with(&format!("{}:", row.instance_id)))
            .cloned();
        let record_path = row.artifacts.record_path.clone();
        let protocol_completed = row.protocol_status == ClosureClass::Complete;
        let protocol_evidence_available = record_path
            .as_ref()
            .is_some_and(|path| load_protocol_aggregate(path).is_ok());

        if input.stop_after >= Prototype1LoopStopAfter::TargetSelection
            && row.eval_status == ClosureClass::Complete
            && protocol_evidence_available
        {
            if let Some(record_path) = record_path.as_ref() {
                let detection_output = {
                    let _scope = TimingTrace::scope(format!(
                        "loop.prototype1.issue_detection.{}",
                        row.instance_id
                    ));
                    persist_issue_detection_for_record(record_path)?
                };
                if let Some(issue) = select_primary_issue(&detection_output) {
                    let synthesis = {
                        let _scope = TimingTrace::scope(format!(
                            "loop.prototype1.intervention_synthesis.{}",
                            row.instance_id
                        ));
                        persist_intervention_synthesis_for_record(
                            record_path,
                            issue.clone(),
                            input.source_branch_id.clone().unwrap_or_else(|| {
                                row.artifacts
                                    .run_root
                                    .as_ref()
                                    .map(|path| path.display().to_string())
                                    .unwrap_or_else(|| row.instance_id.clone())
                            }),
                            input.protocol_model_id.clone(),
                            input.protocol_provider.clone(),
                        )
                        .await?
                    };
                    if let Some(candidate) = synthesis.primary_candidate() {
                        let selected_branch_id = treatment_branch_id(
                            &synthesis.candidate_set.source_state_id,
                            &synthesis.candidate_set.target_relpath,
                            candidate.candidate_id.as_str(),
                        );
                        let _ = record_synthesized_branches(
                            &campaign.campaign_id,
                            &campaign.manifest_path,
                            &row.instance_id,
                            &synthesis,
                            Some(candidate.candidate_id.as_str()),
                            input.source_branch_id.as_deref(),
                        )?;
                        let apply_output = if input.stop_after
                            >= Prototype1LoopStopAfter::InterventionApply
                            && !input.dry_run
                        {
                            let output = {
                                let _scope = TimingTrace::scope(format!(
                                    "loop.prototype1.intervention_apply.{}",
                                    row.instance_id
                                ));
                                persist_intervention_apply_for_record(
                                    record_path,
                                    &synthesis,
                                    candidate.candidate_id.as_str(),
                                    &intervention_repo_root,
                                )?
                            };
                            let _ = mark_treatment_branch_applied(
                                &campaign.campaign_id,
                                &campaign.manifest_path,
                                &synthesis.candidate_set.target_relpath,
                                &output,
                            )?;
                            Some(output)
                        } else {
                            None
                        };
                        selected_targets.push(Prototype1SelectedTarget {
                            instance_id: row.instance_id.clone(),
                            source_state_id: synthesis.candidate_set.source_state_id.clone(),
                            parent_branch_id: input.source_branch_id.clone(),
                            selected_branch_id,
                            synthesized_candidate_count: synthesis.candidate_set.candidates.len(),
                            selected_candidate_id: candidate.candidate_id.clone(),
                            synthesized_spec_id: candidate.spec.spec_id().to_string(),
                            synthesized_target_relpath: synthesis
                                .candidate_set
                                .target_relpath
                                .clone(),
                            apply_output: apply_output.as_ref().map(|output| {
                                Prototype1AppliedCandidate {
                                    candidate_id: output.candidate_id.clone(),
                                    apply_id: output.treatment_state.apply_id.clone(),
                                    changed: output.changed,
                                    source_content_hash: output.source_content_hash.clone(),
                                    applied_content_hash: output.applied_content_hash.clone(),
                                    target_relpath: output.target_relpath.clone(),
                                }
                            }),
                            apply_skipped_reason: if input.stop_after
                                >= Prototype1LoopStopAfter::InterventionApply
                                && input.dry_run
                            {
                                Some(
                                    "dry-run: synthesized candidate selected but not applied"
                                        .to_string(),
                                )
                            } else {
                                None
                            },
                            issue,
                        });
                    }
                }
            }
        }

        baseline_instances.push(Prototype1LoopInstance {
            instance_id: row.instance_id.clone(),
            eval_status: row.eval_status,
            protocol_status: row.protocol_status,
            record_path,
            protocol_completed,
            protocol_evidence_available,
            protocol_failure,
        });
    }

    if !selected_targets.is_empty() {
        let registry =
            load_or_default_branch_registry(&campaign.campaign_id, &campaign.manifest_path)?;
        for target in &selected_targets {
            let Some(source_node) = registry.source_nodes.iter().find(|node| {
                node.instance_id == target.instance_id
                    && node.source_state_id == target.source_state_id
                    && node.target_relpath == target.synthesized_target_relpath
            }) else {
                continue;
            };

            let generation = prototype1_source_generation(&registry, source_node) + 1;
            for branch in &source_node.branches {
                if scheduler.nodes.len() as u32 >= search_policy.max_total_nodes {
                    break;
                }
                let resolved = resolve_treatment_branch(
                    &campaign.campaign_id,
                    &campaign.manifest_path,
                    &branch.branch_id,
                )?;
                let (updated_scheduler, node, _) = register_treatment_evaluation_node(
                    &campaign.campaign_id,
                    &campaign.manifest_path,
                    &resolved,
                    generation,
                    &intervention_repo_root,
                    input.stop_on_error,
                )?;
                scheduler = updated_scheduler;
                staged_nodes.push(node);
            }
        }
    }

    if input.stop_after >= Prototype1LoopStopAfter::Compare && !input.dry_run {
        let registry =
            load_or_default_branch_registry(&campaign.campaign_id, &campaign.manifest_path)?;
        for target in &selected_targets {
            let Some(source_node) = registry.source_nodes.iter().find(|node| {
                node.instance_id == target.instance_id
                    && node.source_state_id == target.source_state_id
                    && node.target_relpath == target.synthesized_target_relpath
            }) else {
                continue;
            };

            for branch in &source_node.branches {
                let report = {
                    let _scope = TimingTrace::scope(format!(
                        "loop.prototype1.branch_evaluate.{}.{}",
                        source_node.instance_id, branch.branch_id
                    ));
                    run_prototype1_branch_evaluation_via_child(
                        &campaign.campaign_id,
                        &branch.branch_id,
                        &intervention_repo_root,
                        input.stop_on_error,
                    )
                    .await?
                };
                branch_evaluations.push(match report {
                    Prototype1NodeExecutionOutcome::Evaluated(report) => {
                        summarize_prototype1_branch_evaluation(
                            &source_node.instance_id,
                            &source_node.source_state_id,
                            source_node.parent_branch_id.as_deref(),
                            &branch.branch_id,
                            &branch.candidate_id,
                            &branch.branch_label,
                            &report,
                        )
                    }
                    Prototype1NodeExecutionOutcome::Failed(result) => {
                        summarize_prototype1_failed_branch_evaluation(
                            &source_node.instance_id,
                            &source_node.source_state_id,
                            source_node.parent_branch_id.as_deref(),
                            &branch.branch_id,
                            &branch.candidate_id,
                            &branch.branch_label,
                            &result,
                        )
                    }
                });
            }
        }

        selected_next_branch_id = select_most_promising_branch(&branch_evaluations);
        if let Some(branch_id) = selected_next_branch_id.as_deref() {
            let _ =
                select_treatment_branch(&campaign.campaign_id, &campaign.manifest_path, branch_id)?;
        }

        let current_generation = selected_targets
            .iter()
            .filter_map(|target| {
                registry.source_nodes.iter().find(|node| {
                    node.instance_id == target.instance_id
                        && node.source_state_id == target.source_state_id
                        && node.target_relpath == target.synthesized_target_relpath
                })
            })
            .map(|source_node| prototype1_source_generation(&registry, source_node))
            .max()
            .unwrap_or(0);
        let selected_branch_disposition = selected_next_branch_id.as_deref().and_then(|id| {
            branch_evaluations
                .iter()
                .find(|row| row.branch_id == id)
                .map(|row| serde_name(&row.overall_disposition).to_string())
        });
        let decision = crate::intervention::decide_continuation(
            &scheduler,
            current_generation,
            selected_next_branch_id.as_deref(),
            selected_branch_disposition.as_deref(),
        );
        let _ = record_continuation_decision(
            &campaign.campaign_id,
            &campaign.manifest_path,
            decision.clone(),
        )?;
        continuation_decision = Some(decision);
    }

    let report = Prototype1LoopReport {
        stage_reached: input.stop_after,
        dry_run: input.dry_run,
        search_policy,
        continuation_decision,
        continued_from_campaign: input.source_campaign.clone(),
        continued_from_branch_id: input.source_branch_id.clone(),
        batch_id: input.batch_id,
        batch_manifest,
        campaign_id: campaign.campaign_id,
        campaign_manifest: campaign.manifest_path,
        closure_state_path: campaign.closure_state_path,
        slice_dataset_path: campaign.slice_dataset_path,
        branch_registry_path,
        scheduler_path,
        trace_path: trace_path.clone(),
        prepared_instances: input.prepared_instances,
        completed_instances: eval_report
            .selected_instances
            .iter()
            .filter(|instance_id| {
                baseline_instances.iter().any(|row| {
                    row.instance_id == **instance_id && row.eval_status == ClosureClass::Complete
                })
            })
            .cloned()
            .collect(),
        protocol_task_instances,
        baseline_instances,
        selected_targets,
        staged_nodes,
        branch_evaluations,
        selected_next_branch_id,
        protocol_failures,
        pending_stages: pending_prototype1_stages(input.stop_after),
    };
    write_json_file_pretty(&trace_path, &report)?;
    Ok(report)
}

fn prepare_or_load_prototype1_batch(
    command: &Prototype1LoopCommand,
) -> Result<(PathBuf, PreparedMsbBatch), PrepareError> {
    if command.batch.is_some() || command.batch_id.is_some() {
        return load_prepared_batch_for_loop(resolve_batch_manifest(
            command.batch.clone(),
            command.batch_id.clone(),
        )?);
    }

    let batch_id = command.prepare_batch_id.clone().unwrap_or_else(|| {
        default_batch_id(
            command.dataset_key.as_deref(),
            command.dataset.as_ref(),
            command.all,
            &command.instance,
            &command.specific,
        )
    });
    let prepared = PrepareMsbBatchRequest {
        dataset_file: command.dataset.clone(),
        dataset_key: command.dataset_key.clone(),
        batch_id,
        select_all: command.all,
        instance_ids: command.instance.clone(),
        specifics: command.specific.clone(),
        limit: command.limit,
        repo_cache: command.repo_cache.clone().unwrap_or(repos_dir()?),
        instances_root: command.instances_root.clone().unwrap_or(instances_dir()?),
        batches_root: command.batches_root.clone().unwrap_or(batches_dir()?),
        budget: EvalBudget {
            max_turns: command.max_turns,
            max_tool_calls: command.max_tool_calls,
            wall_clock_secs: command.wall_clock_secs,
        },
    }
    .prepare()?;

    for run in &prepared.runs {
        run.write_manifest(OutputMode::Pretty, PrepareWrite::File(run.manifest_path()))?;
    }
    prepared
        .batch
        .write_manifest(OutputMode::Pretty)
        .map_err(PrepareError::from)?;
    let manifest_path = prepared.batch.manifest_path();
    Ok((manifest_path, prepared.batch))
}

impl Prototype1BranchStatusCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let manifest_path = campaign_manifest_path(&self.campaign)?;
        let registry = load_or_default_branch_registry(&self.campaign, &manifest_path)?;
        let report = prototype1_branch_status_report(&self.campaign, &manifest_path, &registry);
        match self.format {
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
            InspectOutputFormat::Table => print_prototype1_branch_status_report(&report),
        }
        Ok(())
    }
}

impl Prototype1BranchShowCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let manifest_path = campaign_manifest_path(&self.campaign)?;
        let branch_registry_path = prototype1_branch_registry_path(&manifest_path);
        let resolved = resolve_treatment_branch(&self.campaign, &manifest_path, &self.branch_id)?;
        let report = Prototype1BranchShowReport {
            campaign_id: self.campaign,
            branch_registry_path,
            instance_id: resolved.instance_id.clone(),
            source_state_id: resolved.source_state_id.clone(),
            parent_branch_id: resolved.parent_branch_id.clone(),
            target_relpath: resolved.target_relpath.clone(),
            source_content_hash: resolved.source_content_hash.clone(),
            selected_branch_id: resolved.selected_branch_id.clone(),
            branch_id: resolved.branch.branch_id.clone(),
            candidate_id: resolved.branch.candidate_id.clone(),
            branch_label: resolved.branch.branch_label.clone(),
            status: format!("{:?}", resolved.branch.status).to_ascii_lowercase(),
            apply_id: resolved.branch.apply_id.clone(),
            proposed_content_hash: resolved.branch.proposed_content_hash.clone(),
            proposed_content: resolved.branch.proposed_content.clone(),
        };
        match self.format {
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
            InspectOutputFormat::Table => print_prototype1_branch_show_report(&report),
        }
        Ok(())
    }
}

impl Prototype1BranchApplyCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let manifest_path = campaign_manifest_path(&self.campaign)?;
        let branch_registry_path = prototype1_branch_registry_path(&manifest_path);
        let repo_root = if let Some(path) = self.repo_root {
            path
        } else {
            std::env::current_dir().map_err(|source| PrepareError::ReadManifest {
                path: PathBuf::from("."),
                source,
            })?
        };
        let resolved = resolve_treatment_branch(&self.campaign, &manifest_path, &self.branch_id)?;
        let tool = tool_name_for_description_relpath(&resolved.target_relpath)?;
        let candidate = InterventionCandidate {
            candidate_id: resolved.branch.candidate_id.clone(),
            branch_label: resolved.branch.branch_label.clone(),
            proposed_content: resolved.branch.proposed_content.clone(),
            // This manual operator path reconstructs a candidate from the
            // branch handle. The registry will recover patch provenance from
            // its stored branch record or text-file fallback; future materialize
            // paths should pass registry provenance directly.
            patch_id: None,
            spec: InterventionSpec::ToolGuidanceMutation {
                spec_id: resolved.branch.synthesized_spec_id.clone(),
                evidence_basis: "prototype1_branch_apply".to_string(),
                intended_effect: "materialize stored treatment branch content".to_string(),
                tool,
                edit: ArtifactEdit::ReplaceWholeText {
                    new_text: resolved.branch.proposed_content.clone(),
                },
                validation_policy: ValidationPolicy::for_tool_description_target(tool),
            },
        };
        let input = InterventionApplyInput {
            source_state_id: resolved.source_state_id.clone(),
            candidate,
            target_relpath: resolved.target_relpath.clone(),
            expected_source_content: resolved.source_content.clone(),
            repo_root,
            // No whole-worktree artifact identity is available in this manual
            // apply command yet. `execute_intervention_apply` will derive only
            // text-file surface identities.
            base_artifact_id: None,
            patch_id: None,
        };
        let output =
            execute_intervention_apply(&input).map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_branch_apply",
                detail: source.to_string(),
            })?;
        let _ = mark_treatment_branch_applied(
            &self.campaign,
            &manifest_path,
            &input.target_relpath,
            &output,
        )?;
        let report = Prototype1BranchApplyReport {
            campaign_id: self.campaign,
            branch_registry_path,
            branch_id: self.branch_id,
            candidate_id: output.candidate_id.clone(),
            source_state_id: output.treatment_state.source_state_id.clone(),
            target_relpath: output.target_relpath.clone(),
            absolute_path: output.absolute_path.clone(),
            changed: output.changed,
            apply_id: output.treatment_state.apply_id.clone(),
            source_content_hash: output.source_content_hash,
            applied_content_hash: output.applied_content_hash,
        };
        match self.format {
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
            InspectOutputFormat::Table => print_prototype1_branch_apply_report(&report),
        }
        Ok(())
    }
}

impl Prototype1BranchEvaluateCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        let repo_root = if let Some(path) = self.repo_root {
            path
        } else {
            std::env::current_dir().map_err(|source| PrepareError::ReadManifest {
                path: PathBuf::from("."),
                source,
            })?
        };
        let report = run_prototype1_branch_evaluation(
            &self.campaign,
            &self.branch_id,
            &repo_root,
            self.stop_on_error,
        )
        .await?;

        match self.format {
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
            InspectOutputFormat::Table => print_prototype1_branch_evaluation_report(&report),
        }
        Ok(())
    }
}

impl Prototype1BranchSelectCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let manifest_path = campaign_manifest_path(&self.campaign)?;
        let registry = select_treatment_branch(&self.campaign, &manifest_path, &self.branch_id)?;
        let report = prototype1_branch_status_report(&self.campaign, &manifest_path, &registry);
        match self.format {
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
            InspectOutputFormat::Table => print_prototype1_branch_status_report(&report),
        }
        Ok(())
    }
}

impl Prototype1BranchRestoreCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let manifest_path = campaign_manifest_path(&self.campaign)?;
        let repo_root = if let Some(path) = self.repo_root {
            path
        } else {
            std::env::current_dir().map_err(|source| PrepareError::ReadManifest {
                path: PathBuf::from("."),
                source,
            })?
        };
        let restored =
            restore_treatment_branch(&self.campaign, &manifest_path, &self.branch_id, &repo_root)?;
        match self.format {
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&restored).map_err(PrepareError::Serialize)?
                );
            }
            InspectOutputFormat::Table => {
                println!("prototype1 branch restore");
                println!("{}", "-".repeat(40));
                println!("campaign_id: {}", self.campaign);
                println!("branch_id: {}", restored.branch_id);
                println!("source_state_id: {}", restored.source_state_id);
                println!("target: {}", restored.target_relpath.display());
                println!("changed: {}", yes_no(restored.changed));
            }
        }
        Ok(())
    }
}

impl Prototype1RunnerCommand {
    pub async fn run(self) -> Result<(), PrepareError> {
        if let Some(invocation_path) = self.invocation.clone() {
            if self.campaign.is_some() || self.node_id.is_some() {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: "prototype1-runner expects either --invocation or --campaign/--node-id, not both"
                        .to_string(),
                });
            }

            if self.execute {
                match crate::cli::prototype1_state::invocation::load_executable(&invocation_path)? {
                    crate::cli::prototype1_state::invocation::InvocationAuthority::Child(_) => {
                        let result = execute_prototype1_runner_invocation(&invocation_path).await?;
                        match self.format {
                            InspectOutputFormat::Json => {
                                println!(
                                    "{}",
                                    serde_json::to_string_pretty(&result)
                                        .map_err(PrepareError::Serialize)?
                                );
                            }
                            InspectOutputFormat::Table => {
                                println!("prototype1 runner invocation");
                                println!("{}", "-".repeat(40));
                                println!("invocation: {}", invocation_path.display());
                                println!("campaign_id: {}", result.campaign_id);
                                println!("node_id: {}", result.node_id);
                                println!("branch_id: {}", result.branch_id);
                                println!("status: {:?}", result.status);
                                println!("disposition: {:?}", result.disposition);
                            }
                        }
                    }
                    crate::cli::prototype1_state::invocation::InvocationAuthority::Successor(_) => {
                        return Err(PrepareError::InvalidBatchSelection {
                            detail: format!(
                                "successor invocation '{}' must be executed by loop prototype1-state --handoff-invocation",
                                invocation_path.display()
                            ),
                        });
                    }
                }
                return Ok(());
            }

            let invocation = crate::cli::prototype1_state::invocation::load(&invocation_path)?;
            match self.format {
                InspectOutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&invocation)
                            .map_err(PrepareError::Serialize)?
                    );
                }
                InspectOutputFormat::Table => {
                    println!("prototype1 runner invocation");
                    println!("{}", "-".repeat(40));
                    println!("path: {}", invocation_path.display());
                    println!("role: {:?}", invocation.role);
                    println!("campaign_id: {}", invocation.campaign_id);
                    println!("node_id: {}", invocation.node_id);
                    println!("runtime_id: {}", invocation.runtime_id);
                    println!("journal_path: {}", invocation.journal_path.display());
                }
            }
            return Ok(());
        }

        let campaign = self
            .campaign
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "prototype1-runner requires --invocation or both --campaign and --node-id"
                    .to_string(),
            })?;
        let node_id = self
            .node_id
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "prototype1-runner requires --invocation or both --campaign and --node-id"
                    .to_string(),
            })?;
        let manifest_path = campaign_manifest_path(&campaign)?;
        if self.execute {
            let _ = execute_prototype1_runner_node(&campaign, &node_id, self.stop_on_error).await?;
        }
        let scheduler = load_or_default_scheduler_state(&campaign, &manifest_path)?;
        let node = load_node_record(&manifest_path, &node_id)?;
        let request = load_runner_request(&manifest_path, &node_id)?;
        let runner_result = if node.runner_result_path.exists() {
            Some(load_runner_result(&manifest_path, &node_id)?)
        } else {
            None
        };
        let workspace_exists = node.workspace_root.exists();
        let binary_exists = node.binary_path.exists();
        let runner_result_exists = node.runner_result_path.exists();

        let report = Prototype1RunnerReport {
            campaign_id: campaign,
            scheduler_path: prototype1_scheduler_path(&manifest_path),
            scheduler,
            node: Prototype1RunnerNodeReport {
                node,
                workspace_exists,
                binary_exists,
                runner_result_exists,
                runner_args: request.runner_args,
            },
            runner_result,
        };

        match self.format {
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
            InspectOutputFormat::Table => print_prototype1_runner_report(&report),
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct Prototype1MonitorLocation {
    label: &'static str,
    path: PathBuf,
    volatility: &'static str,
    description: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Prototype1MonitorSnapshotEntry {
    is_dir: bool,
    len: u64,
    modified: Option<SystemTime>,
}

impl Prototype1MonitorCommand {
    pub fn run(self) -> Result<(), PrepareError> {
        let manifest_path = campaign_manifest_path(&self.campaign)?;
        let repo_root = match self.repo_root {
            Some(path) => Some(path),
            None => std::env::current_dir().ok(),
        };

        match self.command {
            Prototype1MonitorSubcommand::List => {
                print_prototype1_monitor_locations(&manifest_path, repo_root.as_deref());
                Ok(())
            }
            Prototype1MonitorSubcommand::Peek(command) => {
                peek_prototype1_monitor_locations(&manifest_path, repo_root.as_deref(), &command)
            }
            Prototype1MonitorSubcommand::Watch(command) => {
                watch_prototype1_monitor_locations(&manifest_path, repo_root.as_deref(), &command)
            }
        }
    }
}

fn prototype1_campaign_root(campaign_manifest_path: &Path) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
}

fn prototype1_monitor_locations(
    manifest_path: &Path,
    repo_root: Option<&Path>,
) -> Vec<Prototype1MonitorLocation> {
    let prototype_root = prototype1_campaign_root(manifest_path);
    let mut locations = vec![
        Prototype1MonitorLocation {
            label: "campaign manifest",
            path: manifest_path.to_path_buf(),
            volatility: "stable file; may be edited by campaign setup, not by git checkout",
            description: "Baseline campaign manifest reused by the live controller path.",
        },
        Prototype1MonitorLocation {
            label: "prototype1 root",
            path: prototype_root.clone(),
            volatility: "directory; accumulates and mutates during the loop",
            description: "Campaign-local Prototype 1 state root.",
        },
        Prototype1MonitorLocation {
            label: "scheduler",
            path: prototype1_scheduler_path(manifest_path),
            volatility: "mutable JSON; overwritten when scheduler state changes",
            description: "Search policy, node frontier, completed/failed nodes, and latest continuation decision.",
        },
        Prototype1MonitorLocation {
            label: "branch registry",
            path: prototype1_branch_registry_path(manifest_path),
            volatility: "mutable JSON; overwritten as branches are synthesized, selected, applied, and evaluated",
            description: "Synthesized branch records and latest evaluation summaries.",
        },
        Prototype1MonitorLocation {
            label: "legacy loop trace",
            path: prototype_root.join("prototype1-loop-trace.json"),
            volatility: "mutable JSON; overwritten by the legacy loop controller path",
            description: "Legacy controller trace retained while the old path remains available.",
        },
        Prototype1MonitorLocation {
            label: "transition journal",
            path: prototype1_transition_journal_path(manifest_path),
            volatility: "append-only JSONL; should not be overwritten by normal loop progress",
            description: "Typed transition journal for materialize/build/spawn/observe events.",
        },
        Prototype1MonitorLocation {
            label: "evaluations",
            path: prototype_root.join("evaluations"),
            volatility: "directory; per-branch JSON files are created or replaced by evaluation runs",
            description: "Treatment-vs-baseline branch evaluation artifacts.",
        },
        Prototype1MonitorLocation {
            label: "nodes",
            path: prototype_root.join("nodes"),
            volatility: "directory; node subdirectories are created and later updated",
            description: "Scheduler-owned node records plus per-runtime invocation and result artifacts.",
        },
        Prototype1MonitorLocation {
            label: "node record",
            path: prototype_root.join("nodes/<node-id>/node.json"),
            volatility: "node-scoped JSON; created at node registration and may be updated with status/workspace fields",
            description: "Durable scheduler mirror for one candidate node.",
        },
        Prototype1MonitorLocation {
            label: "runner request",
            path: prototype_root.join("nodes/<node-id>/runner-request.json"),
            volatility: "node-scoped JSON; mutable request consumed by child runtime",
            description: "Runner invocation configuration for one node.",
        },
        Prototype1MonitorLocation {
            label: "latest runner result",
            path: prototype_root.join("nodes/<node-id>/runner-result.json"),
            volatility: "node-scoped JSON; overwritten or cleared before a fresh attempt",
            description: "Latest runner outcome used by the parent/controller.",
        },
        Prototype1MonitorLocation {
            label: "runtime invocation",
            path: prototype_root.join("nodes/<node-id>/invocations/<runtime-id>.json"),
            volatility: "attempt-scoped JSON; created per runtime and normally retained",
            description: "Child or successor authority token for one spawned process.",
        },
        Prototype1MonitorLocation {
            label: "attempt result",
            path: prototype_root.join("nodes/<node-id>/results/<runtime-id>.json"),
            volatility: "attempt-scoped JSON; created per child runtime and normally retained",
            description: "Runner result for one concrete runtime attempt.",
        },
        Prototype1MonitorLocation {
            label: "successor ready",
            path: prototype_root.join("nodes/<node-id>/successor-ready/<runtime-id>.json"),
            volatility: "attempt-scoped JSON; created when detached successor acknowledges handoff",
            description: "Parent-observed successor acknowledgement.",
        },
        Prototype1MonitorLocation {
            label: "successor completion",
            path: prototype_root.join("nodes/<node-id>/successor-completion/<runtime-id>.json"),
            volatility: "attempt-scoped JSON; created after successor finishes its parent turn",
            description: "Terminal status for a successor parent process.",
        },
        Prototype1MonitorLocation {
            label: "child worktree",
            path: prototype_root.join("nodes/<node-id>/worktree"),
            volatility: "temporary directory; should be removed after artifact persistence/handoff",
            description: "Backend-managed child workspace, not the next parent home.",
        },
        Prototype1MonitorLocation {
            label: "child build products",
            path: prototype_root.join("nodes/<node-id>/{bin,target}"),
            volatility: "temporary directories; may be removed by cleanup",
            description: "Build output for child evaluation, not durable identity.",
        },
    ];

    if let Some(repo_root) = repo_root {
        locations.push(Prototype1MonitorLocation {
            label: "active parent identity",
            path: crate::cli::prototype1_state::identity::parent_identity_path(repo_root),
            volatility: "git-tracked artifact file; replaced by git switch during parent handoff",
            description: "Identity the active checkout uses to know which Parent it is.",
        });
    }

    locations
}

fn print_prototype1_monitor_locations(manifest_path: &Path, repo_root: Option<&Path>) {
    println!("prototype1 monitor locations");
    println!("{}", "-".repeat(40));
    for location in prototype1_monitor_locations(manifest_path, repo_root) {
        let exists = if location.path.exists() { "yes" } else { "no" };
        println!("{}", location.label);
        println!("  path: {}", location.path.display());
        println!("  exists: {exists}");
        println!("  volatility: {}", location.volatility);
        println!("  description: {}", location.description);
    }
}

fn peek_prototype1_monitor_locations(
    manifest_path: &Path,
    repo_root: Option<&Path>,
    command: &Prototype1MonitorPeekCommand,
) -> Result<(), PrepareError> {
    let prototype_root = prototype1_campaign_root(manifest_path);
    let mut files = BTreeSet::new();
    for location in prototype1_monitor_locations(manifest_path, repo_root) {
        if location.path.is_file() {
            files.insert(location.path);
        }
    }
    collect_prototype1_monitor_files(&prototype_root, &mut files)?;
    if let Some(repo_root) = repo_root {
        let identity_path = crate::cli::prototype1_state::identity::parent_identity_path(repo_root);
        if identity_path.is_file() {
            files.insert(identity_path);
        }
    }

    if files.is_empty() {
        println!("no existing Prototype 1 monitor files found");
        return Ok(());
    }

    for path in files {
        print_prototype1_file_excerpt(&prototype_root, &path, command.lines, command.bytes)?;
    }
    Ok(())
}

fn watch_prototype1_monitor_locations(
    manifest_path: &Path,
    repo_root: Option<&Path>,
    command: &Prototype1MonitorWatchCommand,
) -> Result<(), PrepareError> {
    let prototype_root = prototype1_campaign_root(manifest_path);
    let identity_path = repo_root.map(crate::cli::prototype1_state::identity::parent_identity_path);
    let interval = Duration::from_millis(command.interval_ms.max(1));
    println!("watching Prototype 1 outputs");
    println!("campaign_root: {}", prototype_root.display());
    if let Some(path) = identity_path.as_ref() {
        println!("active_parent_identity: {}", path.display());
    }
    println!("interval_ms: {}", interval.as_millis());
    println!("press Ctrl-C to stop");

    let mut previous =
        collect_prototype1_monitor_snapshot(&prototype_root, identity_path.as_deref())?;
    if command.print_initial {
        for (path, entry) in &previous {
            print_prototype1_monitor_event("initial", &prototype_root, path, Some(*entry));
        }
    }

    loop {
        thread::sleep(interval);
        let current =
            collect_prototype1_monitor_snapshot(&prototype_root, identity_path.as_deref())?;

        for (path, entry) in &current {
            match previous.get(path) {
                None => {
                    print_prototype1_monitor_event("created", &prototype_root, path, Some(*entry))
                }
                Some(old) if old != entry => {
                    print_prototype1_monitor_event("modified", &prototype_root, path, Some(*entry))
                }
                Some(_) => {}
            }
        }
        for path in previous.keys() {
            if !current.contains_key(path) {
                print_prototype1_monitor_event("removed", &prototype_root, path, None);
            }
        }
        previous = current;
    }
}

fn collect_prototype1_monitor_snapshot(
    prototype_root: &Path,
    identity_path: Option<&Path>,
) -> Result<BTreeMap<PathBuf, Prototype1MonitorSnapshotEntry>, PrepareError> {
    let mut snapshot = BTreeMap::new();
    collect_prototype1_monitor_tree(prototype_root, &mut snapshot)?;
    if let Some(path) = identity_path {
        if let Ok(metadata) = fs::metadata(path) {
            snapshot.insert(path.to_path_buf(), snapshot_entry(&metadata));
        }
    }
    Ok(snapshot)
}

fn collect_prototype1_monitor_files(
    root: &Path,
    files: &mut BTreeSet<PathBuf>,
) -> Result<(), PrepareError> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|source| PrepareError::ReadManifest {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: root.to_path_buf(),
            source,
        })?;
        let path = entry.path();
        let metadata = entry
            .metadata()
            .map_err(|source| PrepareError::ReadManifest {
                path: path.clone(),
                source,
            })?;
        if metadata.is_file() {
            files.insert(path);
        } else if metadata.is_dir() && should_descend_prototype1_monitor_dir(&path) {
            collect_prototype1_monitor_files(&path, files)?;
        }
    }
    Ok(())
}

fn collect_prototype1_monitor_tree(
    root: &Path,
    snapshot: &mut BTreeMap<PathBuf, Prototype1MonitorSnapshotEntry>,
) -> Result<(), PrepareError> {
    if !root.exists() {
        return Ok(());
    }
    let metadata = fs::metadata(root).map_err(|source| PrepareError::ReadManifest {
        path: root.to_path_buf(),
        source,
    })?;
    snapshot.insert(root.to_path_buf(), snapshot_entry(&metadata));
    if !metadata.is_dir() || !should_descend_prototype1_monitor_dir(root) {
        return Ok(());
    }
    for entry in fs::read_dir(root).map_err(|source| PrepareError::ReadManifest {
        path: root.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: root.to_path_buf(),
            source,
        })?;
        collect_prototype1_monitor_tree(&entry.path(), snapshot)?;
    }
    Ok(())
}

fn should_descend_prototype1_monitor_dir(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return true;
    };
    !matches!(name, "worktree" | "target" | "bin")
}

fn snapshot_entry(metadata: &fs::Metadata) -> Prototype1MonitorSnapshotEntry {
    Prototype1MonitorSnapshotEntry {
        is_dir: metadata.is_dir(),
        len: metadata.len(),
        modified: metadata.modified().ok(),
    }
}

fn print_prototype1_file_excerpt(
    prototype_root: &Path,
    path: &Path,
    lines: usize,
    bytes: usize,
) -> Result<(), PrepareError> {
    let content = fs::read(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let start = content.len().saturating_sub(bytes);
    let text = String::from_utf8_lossy(&content[start..]);
    let excerpt_lines: Vec<_> = text.lines().rev().take(lines).collect();
    println!(
        "\n== {} ==",
        prototype1_monitor_display_path(prototype_root, path)
    );
    println!("{}", prototype1_monitor_description(prototype_root, path));
    for line in excerpt_lines.into_iter().rev() {
        println!("{line}");
    }
    Ok(())
}

fn print_prototype1_monitor_event(
    event: &str,
    prototype_root: &Path,
    path: &Path,
    entry: Option<Prototype1MonitorSnapshotEntry>,
) {
    let timestamp = Utc::now().to_rfc3339();
    let kind = entry
        .map(|entry| if entry.is_dir { "dir" } else { "file" })
        .unwrap_or("-");
    let size = entry
        .map(|entry| entry.len.to_string())
        .unwrap_or_else(|| "-".to_string());
    println!(
        "{timestamp} {event:<8} {kind:<4} bytes={size:<8} {} -- {}",
        prototype1_monitor_display_path(prototype_root, path),
        prototype1_monitor_description(prototype_root, path)
    );
}

fn prototype1_monitor_display_path(prototype_root: &Path, path: &Path) -> String {
    path.strip_prefix(prototype_root)
        .map(|relative| relative.display().to_string())
        .unwrap_or_else(|_| path.display().to_string())
}

fn prototype1_monitor_description(prototype_root: &Path, path: &Path) -> &'static str {
    let Ok(relative) = path.strip_prefix(prototype_root) else {
        return "active checkout artifact identity or campaign-level file";
    };
    let parts: Vec<_> = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect();
    match parts.as_slice() {
        [] => "campaign-local Prototype 1 state root",
        ["scheduler.json"] => "scheduler frontier, policy, node list, and continuation decision",
        ["branches.json"] => "synthesized branch registry and evaluation summaries",
        ["prototype1-loop-trace.json"] => "legacy loop controller trace, overwritten per run",
        ["transition-journal.jsonl"] => "append-only typed transition journal",
        ["evaluations"] => "branch evaluation artifact directory",
        ["evaluations", _] => "branch evaluation artifact",
        ["nodes"] => "node artifact directory",
        ["nodes", _] => "node-specific artifact directory",
        ["nodes", _, "node.json"] => "scheduler-owned node record",
        ["nodes", _, "runner-request.json"] => "node runner request",
        ["nodes", _, "runner-result.json"] => "latest node runner result",
        ["nodes", _, "invocations"] => "runtime invocation directory",
        ["nodes", _, "invocations", _] => "attempt-scoped runtime invocation",
        ["nodes", _, "results"] => "runtime result directory",
        ["nodes", _, "results", _] => "attempt-scoped runtime result",
        ["nodes", _, "successor-ready"] => "successor acknowledgement directory",
        ["nodes", _, "successor-ready", _] => "successor ready acknowledgement",
        ["nodes", _, "successor-completion"] => "successor completion directory",
        ["nodes", _, "successor-completion", _] => "successor terminal status",
        ["nodes", _, "worktree"] => "temporary child worktree root",
        ["nodes", _, "bin"] | ["nodes", _, "target"] => "temporary child build output root",
        _ => "Prototype 1 campaign artifact",
    }
}

fn prototype1_state_transition_error(
    phase: &'static str,
    detail: impl Into<String>,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase,
        detail: detail.into(),
    }
}

fn same_existing_path(left: &Path, right: &Path) -> bool {
    let normalize = |path: &Path| fs::canonicalize(path).unwrap_or_else(|_| path.to_path_buf());
    normalize(left) == normalize(right)
}

fn initialize_prototype1_parent_identity(
    command: &Prototype1StateCommand,
    manifest_path: &Path,
    repo_root: &Path,
) -> Result<ParentIdentity, PrepareError> {
    let node_id =
        command
            .node_id
            .as_deref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: "--init-parent-identity requires --node-id for the bootstrap parent"
                    .to_string(),
            })?;
    let Some(branch) = command.identity_branch.as_deref() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail:
                "--init-parent-identity requires --identity-branch so gen0 starts on a fresh branch"
                    .to_string(),
        });
    };
    let backend = GitWorktreeBackend;
    let node = load_node_record(manifest_path, node_id)?;
    if node.generation != 0 {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "--init-parent-identity can only initialize generation 0, node '{node_id}' is generation {}",
                node.generation
            ),
        });
    }
    let _ = backend
        .checkout_fresh_parent_branch(repo_root, branch)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_parent_identity_branch",
            detail: source.to_string(),
        })?;
    let identity = ParentIdentity::from_node(
        command.campaign.clone(),
        &node,
        None,
        Some(branch.to_string()),
    );
    let _ = write_parent_identity(repo_root, &identity)?;
    let message = parent_identity_commit_message(&identity);
    let _ = backend
        .persist_active_checkout_files(repo_root, &[parent_identity_relpath()], &message)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_parent_identity_commit",
            detail: source.to_string(),
        })?;
    backend
        .validate_parent_admission(repo_root, &identity)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_parent_admission",
            detail: source.to_string(),
        })?;
    Ok(identity)
}

fn resolve_prototype1_parent_identity(
    command: &Prototype1StateCommand,
    manifest_path: &Path,
    repo_root: &Path,
) -> Result<ParentIdentity, PrepareError> {
    if let Some(identity) = load_parent_identity_optional(repo_root)? {
        identity.validate_for_command(&command.campaign, command.node_id.as_deref())?;
        return Ok(identity);
    }

    let node_id =
        command
            .node_id
            .as_deref()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "parent identity is missing at '{}' and --node-id was not provided",
                    crate::cli::prototype1_state::identity::parent_identity_path(repo_root)
                        .display()
                ),
            })?;
    let node = load_node_record(manifest_path, node_id)?;
    Ok(ParentIdentity::from_node(
        command.campaign.clone(),
        &node,
        None,
        None,
    ))
}

fn acknowledge_prototype1_state_handoff(
    command: &Prototype1StateCommand,
    identity: &ParentIdentity,
    manifest_path: &Path,
    repo_root: &Path,
) -> Result<Option<crate::cli::prototype1_state::invocation::SuccessorInvocation>, PrepareError> {
    let Some(invocation_path) = command.handoff_invocation.as_deref() else {
        return Ok(None);
    };
    let invocation =
        match crate::cli::prototype1_state::invocation::load_executable(invocation_path)? {
            crate::cli::prototype1_state::invocation::InvocationAuthority::Successor(
                invocation,
            ) => invocation,
            crate::cli::prototype1_state::invocation::InvocationAuthority::Child(_) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "handoff invocation '{}' is a child invocation, expected successor",
                        invocation_path.display()
                    ),
                });
            }
        };

    if invocation.campaign_id() != command.campaign {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff invocation campaign '{}' does not match command campaign '{}'",
                invocation.campaign_id(),
                command.campaign
            ),
        });
    }
    if invocation.node_id() != identity.node_id {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff invocation node '{}' does not match parent identity node '{}'",
                invocation.node_id(),
                identity.node_id
            ),
        });
    }
    let active_parent_root =
        invocation
            .active_parent_root()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "handoff invocation '{}' is missing active_parent_root",
                    invocation_path.display()
                ),
            })?;
    if !same_existing_path(active_parent_root, repo_root) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff invocation active_parent_root '{}' does not match command repo_root '{}'",
                active_parent_root.display(),
                repo_root.display()
            ),
        });
    }

    validate_prototype1_successor_continuation(&invocation, manifest_path)?;
    let ready = record_prototype1_successor_ready(&invocation, manifest_path)?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %invocation.campaign_id(),
        node_id = %invocation.node_id(),
        runtime_id = %invocation.runtime_id(),
        pid = ready.pid,
        invocation_path = %invocation_path.display(),
        active_parent_root = %active_parent_root.display(),
        "prototype1 successor acknowledged handoff before entering typed parent run"
    );
    Ok(Some(invocation))
}

impl Prototype1StateCommand {
    #[instrument(
        target = "ploke_exec",
        level = "debug",
        skip(self),
        fields(campaign = %self.campaign, node_id = ?self.node_id, stop_after = ?self.stop_after)
    )]
    pub async fn run(self) -> Result<(), PrepareError> {
        let manifest_path = campaign_manifest_path(&self.campaign)?;
        let repo_root = if let Some(path) = self.repo_root.clone() {
            path
        } else {
            std::env::current_dir().map_err(|source| PrepareError::ReadManifest {
                path: PathBuf::from("."),
                source,
            })?
        };
        let journal_path = prototype1_transition_journal_path(&manifest_path);
        let mut journal = PrototypeJournal::new(journal_path.clone());

        if self.init_parent_identity {
            let identity =
                initialize_prototype1_parent_identity(&self, &manifest_path, &repo_root)?;
            match self.format {
                InspectOutputFormat::Json => {
                    println!(
                        "{}",
                        serde_json::to_string_pretty(&identity).map_err(PrepareError::Serialize)?
                    );
                }
                InspectOutputFormat::Table => {
                    println!("prototype1 parent identity");
                    println!("{}", "-".repeat(40));
                    println!("campaign_id: {}", identity.campaign_id);
                    println!("parent_id: {}", identity.parent_id);
                    println!("node_id: {}", identity.node_id);
                    println!("generation: {}", identity.generation);
                    println!("branch_id: {}", identity.branch_id);
                    println!(
                        "artifact_branch: {}",
                        identity.artifact_branch.as_deref().unwrap_or("-")
                    );
                }
            }
            return Ok(());
        }

        let parent_identity =
            resolve_prototype1_parent_identity(&self, &manifest_path, &repo_root)?;
        let backend = GitWorktreeBackend;
        backend
            .validate_parent_admission(&repo_root, &parent_identity)
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "prototype1_parent_admission",
                detail: source.to_string(),
            })?;
        let node_id = parent_identity.node_id.clone();
        let handoff_invocation = acknowledge_prototype1_state_handoff(
            &self,
            &parent_identity,
            &manifest_path,
            &repo_root,
        )?;

        debug!(
            target: EXECUTION_DEBUG_TARGET,
            campaign = %self.campaign,
            node_id = %node_id,
            parent_id = %parent_identity.parent_id,
            generation = parent_identity.generation,
            repo_root = %repo_root.display(),
            journal_path = %journal_path.display(),
            "starting typed prototype1 state run"
        );
        let c1 = C1::load(
            self.campaign.clone(),
            manifest_path.clone(),
            &node_id,
            repo_root.clone(),
        )
        .map_err(|err| {
            prototype1_state_transition_error("prototype1_state_load_c1", err.to_string())
        })?;

        let c2 = match MaterializeBranch::new()
            .transition(c1, &mut journal)
            .map_err(|err| {
                prototype1_state_transition_error(
                    "prototype1_state_materialize",
                    format!("{err:?}"),
                )
            })? {
            Outcome::Advanced(next) => {
                debug!(
                    target: EXECUTION_DEBUG_TARGET,
                    campaign = %self.campaign,
                    node_id = %node_id,
                    workspace_root = %next.artifact.repo_root.display(),
                    "materialize completed"
                );
                next
            }
            Outcome::Rejected(never) => match never {},
        };

        let (outcome, child_runtime, successor_runtime, successor_pid, successor_ready_path) =
            if self.stop_after == Prototype1StateStopAfter::Materialize {
                ("materialized".to_string(), None, None, None, None)
            } else {
                match BuildChild::new()
                    .transition(c2, &mut journal)
                    .map_err(|err| {
                        prototype1_state_transition_error(
                            "prototype1_state_build",
                            format!("{err:?}"),
                        )
                    })? {
                    Outcome::Rejected(rejected) => {
                        debug!(
                            target: EXECUTION_DEBUG_TARGET,
                            campaign = %self.campaign,
                            node_id = %node_id,
                            rejected = ?rejected,
                            "build rejected"
                        );
                        (
                            format!("build_rejected:{rejected:?}"),
                            None,
                            None,
                            None,
                            None,
                        )
                    }
                    Outcome::Advanced(c3) => {
                        persist_prototype1_buildable_child_artifact(
                            &self.campaign,
                            &manifest_path,
                            &repo_root,
                            c3.node(),
                        )?;
                        debug!(
                            target: EXECUTION_DEBUG_TARGET,
                            campaign = %self.campaign,
                            node_id = %node_id,
                            binary_path = %c3.binary.child_path.display(),
                            "build completed"
                        );
                        if self.stop_after == Prototype1StateStopAfter::Build {
                            ("built".to_string(), None, None, None, None)
                        } else {
                            match SpawnChild::new()
                                .transition(c3, &mut journal)
                                .map_err(|err| {
                                    prototype1_state_transition_error(
                                        "prototype1_state_spawn",
                                        format!("{err:?}"),
                                    )
                                })? {
                                Outcome::Rejected(rejected) => {
                                    debug!(
                                        target: EXECUTION_DEBUG_TARGET,
                                        campaign = %self.campaign,
                                        node_id = %node_id,
                                        rejected = ?rejected,
                                        "spawn rejected"
                                    );
                                    (
                                        format!("spawn_rejected:{rejected:?}"),
                                        None,
                                        None,
                                        None,
                                        None,
                                    )
                                }
                                Outcome::Advanced(c4) => {
                                    let child_runtime = c4.binary.child_runtime.map(
                                        |id: crate::cli::prototype1_state::event::RuntimeId| {
                                            id.to_string()
                                        },
                                    );
                                    debug!(
                                        target: EXECUTION_DEBUG_TARGET,
                                        campaign = %self.campaign,
                                        node_id = %node_id,
                                        child_runtime = ?child_runtime,
                                        "spawn completed"
                                    );
                                    if self.stop_after == Prototype1StateStopAfter::Spawn {
                                        ("spawned".to_string(), child_runtime, None, None, None)
                                    } else {
                                        match ObserveChild::new()
                                            .transition(c4, &mut journal)
                                            .map_err(|err| {
                                                prototype1_state_transition_error(
                                                    "prototype1_state_complete",
                                                    format!("{err:?}"),
                                                )
                                            })? {
                                            Outcome::Rejected(rejected) => {
                                                debug!(
                                                    target: EXECUTION_DEBUG_TARGET,
                                                    campaign = %self.campaign,
                                                    node_id = %node_id,
                                                    rejected = ?rejected,
                                                    "child completion rejected"
                                                );
                                                (
                                                    format!("completion_rejected:{rejected:?}"),
                                                    child_runtime,
                                                    None,
                                                    None,
                                                    None,
                                                )
                                            }
                                            Outcome::Advanced(c5) => {
                                                debug!(
                                                    target: EXECUTION_DEBUG_TARGET,
                                                    campaign = %self.campaign,
                                                    node_id = %node_id,
                                                    disposition = ?c5.report.overall_disposition,
                                                    "child completion observed"
                                                );
                                                if c5.report.overall_disposition
                                                    == BranchDisposition::Keep
                                                {
                                                    let node =
                                                        load_node_record(&manifest_path, &node_id)?;
                                                    let scheduler =
                                                        load_or_default_scheduler_state(
                                                            &self.campaign,
                                                            &manifest_path,
                                                        )?;
                                                    let selected_disposition =
                                                        serde_name(&c5.report.overall_disposition)
                                                            .to_string();
                                                    let decision =
                                                        decide_node_successor_continuation(
                                                            &scheduler,
                                                            &node,
                                                            Some(&selected_disposition),
                                                        );
                                                    let _ = record_continuation_decision(
                                                        &self.campaign,
                                                        &manifest_path,
                                                        decision.clone(),
                                                    )?;
                                                    if decision.disposition
                                                        == Prototype1ContinuationDisposition::ContinueReady
                                                    {
                                                        match spawn_and_handoff_prototype1_successor(
                                                            &self.campaign,
                                                            &node_id,
                                                            &repo_root,
                                                        )? {
                                                            Some(successor) => (
                                                                format!(
                                                                    "completed:{:?};successor_handoff=acknowledged",
                                                                    c5.report.overall_disposition
                                                                ),
                                                                child_runtime,
                                                                Some(successor.runtime_id.to_string()),
                                                                Some(successor.pid),
                                                                Some(successor.ready_path),
                                                            ),
                                                            None => (
                                                                format!(
                                                                    "completed:{:?};successor_handoff=timed_out",
                                                                    c5.report.overall_disposition
                                                                ),
                                                                child_runtime,
                                                                None,
                                                                None,
                                                                None,
                                                            ),
                                                        }
                                                    } else {
                                                        (
                                                            format!(
                                                                "completed:{:?};successor_handoff=skipped:{:?}",
                                                                c5.report.overall_disposition,
                                                                decision.disposition
                                                            ),
                                                            child_runtime,
                                                            None,
                                                            None,
                                                            None,
                                                        )
                                                    }
                                                } else {
                                                    (
                                                        format!(
                                                            "completed:{:?}",
                                                            c5.report.overall_disposition
                                                        ),
                                                        child_runtime,
                                                        None,
                                                        None,
                                                        None,
                                                    )
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };

        let node = load_node_record(&manifest_path, &node_id)?;
        let report = Prototype1StateReport {
            campaign_id: self.campaign,
            node_id,
            repo_root,
            journal_path,
            stop_after: self.stop_after,
            outcome,
            node_status: node.status,
            workspace_root: node.workspace_root,
            binary_path: node.binary_path,
            child_runtime,
            successor_runtime,
            successor_pid,
            successor_ready_path,
        };

        match self.format {
            InspectOutputFormat::Json => {
                println!(
                    "{}",
                    serde_json::to_string_pretty(&report).map_err(PrepareError::Serialize)?
                );
            }
            InspectOutputFormat::Table => {
                println!("prototype1 state");
                println!("{}", "-".repeat(40));
                println!("campaign_id: {}", report.campaign_id);
                println!("node_id: {}", report.node_id);
                println!("repo_root: {}", report.repo_root.display());
                println!("journal_path: {}", report.journal_path.display());
                println!("stop_after: {:?}", report.stop_after);
                println!("outcome: {}", report.outcome);
                println!("node_status: {:?}", report.node_status);
                println!("workspace_root: {}", report.workspace_root.display());
                println!("binary_path: {}", report.binary_path.display());
                println!(
                    "child_runtime: {}",
                    report.child_runtime.as_deref().unwrap_or("-")
                );
                println!(
                    "successor_runtime: {}",
                    report.successor_runtime.as_deref().unwrap_or("-")
                );
                println!(
                    "successor_pid: {}",
                    report
                        .successor_pid
                        .map(|pid| pid.to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
                println!(
                    "successor_ready_path: {}",
                    report
                        .successor_ready_path
                        .as_ref()
                        .map(|path| path.display().to_string())
                        .unwrap_or_else(|| "-".to_string())
                );
            }
        }
        if let Some(invocation) = handoff_invocation {
            let _ = record_prototype1_successor_completion(
                &invocation,
                &manifest_path,
                crate::cli::prototype1_state::invocation::SuccessorCompletionStatus::Succeeded,
                None,
                None,
            )?;
        }
        Ok(())
    }
}

fn tool_name_for_description_relpath(relpath: &Path) -> Result<ToolName, PrepareError> {
    ToolName::ALL
        .into_iter()
        .find(|tool| Path::new(tool.description_artifact_relpath()) == relpath)
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "unsupported branch apply target '{}': expected a known tool description relpath",
                relpath.display()
            ),
        })
}

pub(crate) fn ensure_treatment_branch_materialized(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    resolved: &crate::intervention::ResolvedTreatmentBranch,
    repo_root: &Path,
) -> Result<(), PrepareError> {
    let absolute_path = repo_root.join(&resolved.target_relpath);
    let current =
        fs::read_to_string(&absolute_path).map_err(|source| PrepareError::ReadManifest {
            path: absolute_path.clone(),
            source,
        })?;

    if current == resolved.branch.proposed_content {
        return Ok(());
    }
    if current != resolved.source_content {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "cannot materialize branch '{}' at '{}': current content matches neither the stored source state nor the branch content",
                resolved.branch.branch_id,
                absolute_path.display()
            ),
        });
    }

    let tool = tool_name_for_description_relpath(&resolved.target_relpath)?;
    let candidate = InterventionCandidate {
        candidate_id: resolved.branch.candidate_id.clone(),
        branch_label: resolved.branch.branch_label.clone(),
        proposed_content: resolved.branch.proposed_content.clone(),
        // Branch evaluation currently replays stored branch content through
        // the text-file adapter. Keep this absent until the materialization
        // path can pass the registry's graph provenance directly.
        patch_id: None,
        spec: InterventionSpec::ToolGuidanceMutation {
            spec_id: resolved.branch.synthesized_spec_id.clone(),
            evidence_basis: "prototype1_branch_evaluate".to_string(),
            intended_effect: "materialize stored treatment branch content".to_string(),
            tool,
            edit: ArtifactEdit::ReplaceWholeText {
                new_text: resolved.branch.proposed_content.clone(),
            },
            validation_policy: ValidationPolicy::for_tool_description_target(tool),
        },
    };
    let input = InterventionApplyInput {
        source_state_id: resolved.source_state_id.clone(),
        candidate,
        target_relpath: resolved.target_relpath.clone(),
        expected_source_content: resolved.source_content.clone(),
        repo_root: repo_root.to_path_buf(),
        // This path is still file-surface based; do not invent a whole
        // ArtifactId from the evaluation worktree.
        base_artifact_id: None,
        patch_id: None,
    };
    let output =
        execute_intervention_apply(&input).map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_branch_evaluate_apply",
            detail: source.to_string(),
        })?;
    let _ = mark_treatment_branch_applied(
        campaign_id,
        campaign_manifest_path,
        &input.target_relpath,
        &output,
    )?;
    Ok(())
}

pub(crate) fn prepare_prototype1_treatment_campaign(
    baseline: &ResolvedCampaignConfig,
    branch_id: &str,
) -> Result<Prototype1LoopCampaign, PrepareError> {
    let campaign_id = format!(
        "{}-treatment-{}-{}",
        baseline.campaign_id,
        branch_id,
        Utc::now().timestamp_millis()
    );
    let manifest_path = campaign_manifest_path(&campaign_id)?;
    let baseline_manifest = load_campaign_manifest(&baseline.campaign_id)?;

    let mut manifest = CampaignManifest::new(campaign_id.clone());
    manifest.dataset_sources = baseline_manifest.dataset_sources.clone();
    manifest.model_id = Some(baseline.model_id.clone());
    manifest.provider_slug = baseline.provider_slug.clone();
    manifest.instances_root = Some(
        baseline
            .instances_root
            .join("treatments")
            .join(branch_id)
            .join("instances"),
    );
    manifest.batches_root = Some(
        baseline
            .batches_root
            .join("treatments")
            .join(branch_id)
            .join("batches"),
    );
    manifest.eval = baseline_manifest.eval.clone();
    manifest.protocol = baseline_manifest.protocol.clone();
    save_campaign_manifest(&manifest)?;
    let resolved = resolve_campaign_config(&campaign_id, &CampaignOverrides::default())?;
    let closure_state_path = campaign_closure_state_path(&campaign_id)?;

    Ok(Prototype1LoopCampaign {
        campaign_id,
        manifest_path,
        closure_state_path,
        slice_dataset_path: baseline_manifest
            .dataset_sources
            .first()
            .map(|source| source.path.clone())
            .unwrap_or_else(|| PathBuf::from("<unknown>")),
        resolved,
    })
}

pub(crate) fn prototype1_branch_evaluation_path(
    campaign_manifest_path: &Path,
    branch_id: &str,
) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
        .join("evaluations")
        .join(format!("{branch_id}.json"))
}

pub(crate) fn build_prototype1_branch_evaluation_report(
    baseline_campaign_id: &str,
    branch_id: &str,
    branch_registry_path: &Path,
    evaluation_artifact_path: &Path,
    treatment_campaign: &Prototype1LoopCampaign,
    baseline_state: &crate::closure::ClosureState,
    treatment_state: &crate::closure::ClosureState,
) -> Result<Prototype1BranchEvaluationReport, PrepareError> {
    let mut treatment_by_instance = BTreeMap::new();
    for row in &treatment_state.instances {
        treatment_by_instance.insert(row.instance_id.clone(), row);
    }

    let mut compared_instances = Vec::new();
    let mut reasons = Vec::new();

    for row in &baseline_state.instances {
        let treatment_row = treatment_by_instance.get(&row.instance_id).copied();
        let baseline_record_path = row.artifacts.record_path.clone();
        let treatment_record_path = treatment_row.and_then(|row| row.artifacts.record_path.clone());

        let baseline_metrics = if row.eval_status == ClosureClass::Complete {
            baseline_record_path
                .as_ref()
                .map(|path| {
                    read_compressed_record(path)
                        .map(|record| record.operational_metrics())
                        .map_err(|source| PrepareError::ReadManifest {
                            path: path.clone(),
                            source,
                        })
                })
                .transpose()?
        } else {
            None
        };
        let treatment_metrics =
            if treatment_row.is_some_and(|row| row.eval_status == ClosureClass::Complete) {
                treatment_record_path
                    .as_ref()
                    .map(|path| {
                        read_compressed_record(path)
                            .map(|record| record.operational_metrics())
                            .map_err(|source| PrepareError::ReadManifest {
                                path: path.clone(),
                                source,
                            })
                    })
                    .transpose()?
            } else {
                None
            };

        let (status, evaluation) = match (&baseline_metrics, &treatment_metrics) {
            (Some(baseline_metrics), Some(treatment_metrics)) => {
                let evaluation = evaluate_branch(&BranchEvaluationInput {
                    baseline_metrics: baseline_metrics.clone(),
                    treatment_metrics: treatment_metrics.clone(),
                });
                if evaluation.disposition == BranchDisposition::Reject {
                    for reason in &evaluation.reasons {
                        reasons.push(format!("{}: {}", row.instance_id, reason));
                    }
                }
                ("compared".to_string(), Some(evaluation))
            }
            (Some(_), None) => {
                reasons.push(format!(
                    "{}: treatment arm did not produce a complete record",
                    row.instance_id
                ));
                ("missing_treatment_record".to_string(), None)
            }
            (None, _) => {
                reasons.push(format!(
                    "{}: baseline arm does not have a complete record",
                    row.instance_id
                ));
                ("missing_baseline_record".to_string(), None)
            }
        };

        compared_instances.push(Prototype1ComparedInstanceReport {
            instance_id: row.instance_id.clone(),
            baseline_record_path,
            treatment_record_path,
            baseline_metrics,
            treatment_metrics,
            evaluation,
            status,
        });
    }

    let overall_disposition = if reasons.is_empty() {
        BranchDisposition::Keep
    } else {
        BranchDisposition::Reject
    };

    Ok(Prototype1BranchEvaluationReport {
        baseline_campaign_id: baseline_campaign_id.to_string(),
        branch_id: branch_id.to_string(),
        treatment_campaign_id: treatment_campaign.campaign_id.clone(),
        branch_registry_path: branch_registry_path.to_path_buf(),
        evaluation_artifact_path: evaluation_artifact_path.to_path_buf(),
        treatment_campaign_manifest: treatment_campaign.manifest_path.clone(),
        treatment_closure_state_path: treatment_campaign.closure_state_path.clone(),
        overall_disposition,
        reasons,
        compared_instances,
    })
}

fn summarize_prototype1_branch_evaluation(
    instance_id: &str,
    source_state_id: &str,
    parent_branch_id: Option<&str>,
    branch_id: &str,
    candidate_id: &str,
    branch_label: &str,
    report: &Prototype1BranchEvaluationReport,
) -> Prototype1LoopBranchEvaluationSummary {
    let mut oracle_eligible_instances = 0;
    let mut converged_instances = 0;
    let mut nonempty_submission_instances = 0;
    let mut applied_patch_instances = 0;
    let mut total_tool_calls = 0;
    let mut failed_tool_calls = 0;

    for row in &report.compared_instances {
        if let Some(metrics) = row.treatment_metrics.as_ref() {
            if metrics.oracle_eligible {
                oracle_eligible_instances += 1;
            }
            if metrics.convergence {
                converged_instances += 1;
            }
            if metrics.submission_artifact_state == crate::record::SubmissionArtifactState::Nonempty
            {
                nonempty_submission_instances += 1;
            }
            if metrics.patch_apply_state == crate::operational_metrics::PatchApplyState::Applied {
                applied_patch_instances += 1;
            }
            total_tool_calls += metrics.tool_calls_total;
            failed_tool_calls += metrics.tool_calls_failed;
        }
    }

    Prototype1LoopBranchEvaluationSummary {
        instance_id: instance_id.to_string(),
        source_state_id: source_state_id.to_string(),
        parent_branch_id: parent_branch_id.map(ToOwned::to_owned),
        branch_id: branch_id.to_string(),
        candidate_id: candidate_id.to_string(),
        branch_label: branch_label.to_string(),
        treatment_campaign_id: report.treatment_campaign_id.clone(),
        overall_disposition: report.overall_disposition.clone(),
        evaluation_artifact_path: report.evaluation_artifact_path.clone(),
        oracle_eligible_instances,
        converged_instances,
        nonempty_submission_instances,
        applied_patch_instances,
        total_tool_calls,
        failed_tool_calls,
    }
}

fn summarize_prototype1_failed_branch_evaluation(
    instance_id: &str,
    source_state_id: &str,
    parent_branch_id: Option<&str>,
    branch_id: &str,
    candidate_id: &str,
    branch_label: &str,
    result: &Prototype1RunnerResult,
) -> Prototype1LoopBranchEvaluationSummary {
    Prototype1LoopBranchEvaluationSummary {
        instance_id: instance_id.to_string(),
        source_state_id: source_state_id.to_string(),
        parent_branch_id: parent_branch_id.map(ToOwned::to_owned),
        branch_id: branch_id.to_string(),
        candidate_id: candidate_id.to_string(),
        branch_label: branch_label.to_string(),
        treatment_campaign_id: result
            .treatment_campaign_id
            .clone()
            .unwrap_or_else(|| format!("runner:{}", serde_name(&result.disposition))),
        overall_disposition: BranchDisposition::Reject,
        evaluation_artifact_path: result
            .evaluation_artifact_path
            .clone()
            .unwrap_or_else(|| PathBuf::from("<runner-result-only>")),
        oracle_eligible_instances: 0,
        converged_instances: 0,
        nonempty_submission_instances: 0,
        applied_patch_instances: 0,
        total_tool_calls: 0,
        failed_tool_calls: 0,
    }
}

fn select_most_promising_branch(
    evaluations: &[Prototype1LoopBranchEvaluationSummary],
) -> Option<String> {
    evaluations
        .iter()
        .max_by_key(|row| {
            (
                row.oracle_eligible_instances,
                row.converged_instances,
                row.nonempty_submission_instances,
                row.applied_patch_instances,
                matches!(row.overall_disposition, BranchDisposition::Keep) as usize,
                usize::MAX.saturating_sub(row.failed_tool_calls),
                usize::MAX.saturating_sub(row.total_tool_calls),
            )
        })
        .map(|row| row.branch_id.clone())
}

pub(crate) fn prototype1_source_generation(
    registry: &Prototype1BranchRegistry,
    source_node: &crate::intervention::InterventionSourceNode,
) -> u32 {
    fn recurse(
        registry: &Prototype1BranchRegistry,
        source_node: &crate::intervention::InterventionSourceNode,
        depth: u32,
    ) -> u32 {
        let Some(parent_branch_id) = source_node.parent_branch_id.as_deref() else {
            return depth;
        };
        let Some(parent_source) = registry.source_nodes.iter().find(|candidate| {
            candidate
                .branches
                .iter()
                .any(|branch| branch.branch_id == parent_branch_id)
        }) else {
            return depth + 1;
        };
        recurse(registry, parent_source, depth + 1)
    }

    recurse(registry, source_node, 0)
}

fn prototype1_branch_status_report(
    campaign_id: &str,
    campaign_manifest_path: &Path,
    registry: &crate::intervention::Prototype1BranchRegistry,
) -> Prototype1BranchStatusReport {
    let mut branches = Vec::new();
    for source in &registry.source_nodes {
        for branch in &source.branches {
            branches.push(Prototype1BranchStateRow {
                instance_id: source.instance_id.clone(),
                source_state_id: source.source_state_id.clone(),
                parent_branch_id: source.parent_branch_id.clone(),
                target_relpath: source.target_relpath.clone(),
                source_content_hash: source.source_content_hash.clone(),
                selected_branch_id: source.selected_branch_id.clone(),
                branch_id: branch.branch_id.clone(),
                candidate_id: branch.candidate_id.clone(),
                branch_label: branch.branch_label.clone(),
                status: serde_name(&branch.status).to_string(),
                apply_id: branch.apply_id.clone(),
            });
        }
    }

    Prototype1BranchStatusReport {
        campaign_id: campaign_id.to_string(),
        branch_registry_path: prototype1_branch_registry_path(campaign_manifest_path),
        source_nodes: registry.source_nodes.len(),
        active_targets: registry.active_targets.len(),
        active_target_state: registry
            .active_targets
            .iter()
            .map(|entry| Prototype1ActiveTargetReport {
                target_relpath: entry.target_relpath.clone(),
                source_state_id: entry.source_state_id.clone(),
                active_branch_id: entry.active_branch_id.clone(),
                active_apply_id: entry.active_apply_id.clone(),
            })
            .collect(),
        branches,
    }
}

fn load_prepared_batch_for_loop(
    batch_manifest: PathBuf,
) -> Result<(PathBuf, PreparedMsbBatch), PrepareError> {
    let manifest_path = if batch_manifest.exists() {
        fs::canonicalize(&batch_manifest).map_err(|source| PrepareError::ReadBatchManifest {
            path: batch_manifest.clone(),
            source,
        })?
    } else {
        return Err(PrepareError::MissingBatchManifest(batch_manifest));
    };
    let manifest_text =
        fs::read_to_string(&manifest_path).map_err(|source| PrepareError::ReadBatchManifest {
            path: manifest_path.clone(),
            source,
        })?;
    let prepared = serde_json::from_str(&manifest_text).map_err(|source| {
        PrepareError::ParseBatchManifest {
            path: manifest_path.clone(),
            source,
        }
    })?;
    Ok((manifest_path, prepared))
}

fn prepare_prototype1_loop_campaign(
    command: &Prototype1LoopCommand,
    prepared_batch: &PreparedMsbBatch,
) -> Result<Prototype1LoopCampaign, PrepareError> {
    let eval_model = resolve_model_for_run(
        command
            .model_id
            .as_deref()
            .map(ModelId::from_str)
            .transpose()
            .map_err(|err| PrepareError::DatabaseSetup {
                phase: "prototype1_loop_model_id",
                detail: err.to_string(),
            })?
            .as_ref(),
        command.use_default_model,
    )?
    .id;
    let eval_provider_slug = if let Some(provider) = command.provider.clone() {
        Some(
            ProviderKey::new(&provider)
                .map_err(|err| PrepareError::DatabaseSetup {
                    phase: "prototype1_loop_provider",
                    detail: err.to_string(),
                })?
                .slug
                .as_str()
                .to_string(),
        )
    } else {
        load_provider_for_model(&eval_model)?.map(|provider| provider.slug.as_str().to_string())
    };

    if command.stop_after >= Prototype1LoopStopAfter::BaselineProtocol {
        let protocol_model = resolve_protocol_model_id(command.protocol_model_id.clone())?;
        let protocol_provider =
            resolve_protocol_provider_slug(&protocol_model, command.protocol_provider.clone())?;
        if protocol_model != eval_model || protocol_provider != eval_provider_slug {
            return Err(PrepareError::DatabaseSetup {
                phase: "prototype1_loop_campaign",
                detail: format!(
                    "prototype1 baseline arm now delegates to closure/campaign and currently requires one shared model/provider; eval={} {:?}, protocol={} {:?}",
                    eval_model, eval_provider_slug, protocol_model, protocol_provider
                ),
            });
        }
    }

    let campaign_id = format!(
        "prototype1-{}-{}",
        prepared_batch
            .batch_id
            .chars()
            .map(
                |ch| if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
                    ch
                } else {
                    '-'
                }
            )
            .collect::<String>(),
        Utc::now().timestamp_millis()
    );
    let manifest_path = campaign_manifest_path(&campaign_id)?;
    let campaign_dir = manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    let slice_dataset_path = campaign_dir.join("slice.jsonl");
    write_prototype1_slice_dataset(
        &prepared_batch.dataset_file,
        &prepared_batch.instances,
        &slice_dataset_path,
    )?;

    let mut manifest = CampaignManifest::new(campaign_id.clone());
    manifest.dataset_sources = vec![RegistryDatasetSource {
        key: None,
        path: slice_dataset_path.clone(),
        label: format!("prototype1/{}", prepared_batch.batch_id),
        url: prepared_batch.dataset_url.clone(),
    }];
    manifest.model_id = Some(eval_model.to_string());
    manifest.provider_slug = eval_provider_slug;
    manifest.instances_root = Some(
        command
            .instances_root
            .clone()
            .unwrap_or(instances_dir()?)
            .join("prototype1")
            .join(&campaign_id),
    );
    manifest.batches_root = Some(
        command
            .batches_root
            .clone()
            .unwrap_or(batches_dir()?)
            .join("prototype1")
            .join(&campaign_id),
    );
    manifest.eval = EvalCampaignPolicy {
        include_partial: false,
        stop_on_error: command.stop_on_error,
        limit: None,
        include_dataset_labels: Vec::new(),
        exclude_dataset_labels: Vec::new(),
        budget: prepared_batch.budget.clone(),
        batch_prefix: Some(prepared_batch.batch_id.clone()),
    };
    manifest.protocol = ProtocolCampaignPolicy {
        stop_on_error: command.stop_on_error,
        ..ProtocolCampaignPolicy::default()
    };
    save_campaign_manifest(&manifest)?;
    let resolved = resolve_campaign_config(&campaign_id, &CampaignOverrides::default())?;
    let closure_state_path = campaign_closure_state_path(&campaign_id)?;

    Ok(Prototype1LoopCampaign {
        campaign_id,
        manifest_path,
        closure_state_path,
        slice_dataset_path,
        resolved,
    })
}

fn write_prototype1_slice_dataset(
    dataset_path: &Path,
    instances: &[String],
    output_path: &Path,
) -> Result<(), PrepareError> {
    let wanted = instances.iter().cloned().collect::<BTreeSet<_>>();
    let text = fs::read_to_string(dataset_path).map_err(|source| PrepareError::ReadManifest {
        path: dataset_path.to_path_buf(),
        source,
    })?;
    let mut kept = Vec::new();
    let mut found = BTreeSet::new();

    for (line_idx, line) in text.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let value: serde_json::Value =
            serde_json::from_str(trimmed).map_err(|source| PrepareError::ParseDatasetLine {
                path: dataset_path.to_path_buf(),
                line: line_idx + 1,
                source,
            })?;
        let Some(instance_id) = value.get("instance_id").and_then(|value| value.as_str()) else {
            continue;
        };
        if wanted.contains(instance_id) {
            kept.push(trimmed.to_string());
            found.insert(instance_id.to_string());
        }
    }

    let missing = wanted.difference(&found).cloned().collect::<Vec<_>>();
    if !missing.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1 slice dataset is missing {} selected instances: {}",
                missing.len(),
                missing.join(", ")
            ),
        });
    }

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(output_path, kept.join("\n") + "\n").map_err(|source| PrepareError::WriteManifest {
        path: output_path.to_path_buf(),
        source,
    })
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1LoopReport {
    stage_reached: Prototype1LoopStopAfter,
    dry_run: bool,
    search_policy: Prototype1SearchPolicy,
    continuation_decision: Option<Prototype1ContinuationDecision>,
    continued_from_campaign: Option<String>,
    continued_from_branch_id: Option<String>,
    batch_id: String,
    batch_manifest: PathBuf,
    campaign_id: String,
    campaign_manifest: PathBuf,
    closure_state_path: PathBuf,
    slice_dataset_path: PathBuf,
    branch_registry_path: PathBuf,
    scheduler_path: PathBuf,
    trace_path: PathBuf,
    prepared_instances: Vec<String>,
    completed_instances: Vec<String>,
    protocol_task_instances: Vec<String>,
    baseline_instances: Vec<Prototype1LoopInstance>,
    selected_targets: Vec<Prototype1SelectedTarget>,
    staged_nodes: Vec<Prototype1NodeRecord>,
    branch_evaluations: Vec<Prototype1LoopBranchEvaluationSummary>,
    selected_next_branch_id: Option<String>,
    protocol_failures: Vec<String>,
    pending_stages: Vec<&'static str>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1LoopInstance {
    instance_id: String,
    eval_status: ClosureClass,
    protocol_status: ClosureClass,
    record_path: Option<PathBuf>,
    protocol_completed: bool,
    protocol_evidence_available: bool,
    protocol_failure: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1SelectedTarget {
    instance_id: String,
    issue: IssueCase,
    source_state_id: String,
    parent_branch_id: Option<String>,
    selected_branch_id: String,
    synthesized_candidate_count: usize,
    selected_candidate_id: String,
    synthesized_spec_id: String,
    synthesized_target_relpath: PathBuf,
    apply_output: Option<Prototype1AppliedCandidate>,
    apply_skipped_reason: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1AppliedCandidate {
    candidate_id: String,
    apply_id: String,
    changed: bool,
    source_content_hash: String,
    applied_content_hash: String,
    target_relpath: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1LoopBranchEvaluationSummary {
    instance_id: String,
    source_state_id: String,
    parent_branch_id: Option<String>,
    branch_id: String,
    candidate_id: String,
    branch_label: String,
    treatment_campaign_id: String,
    overall_disposition: BranchDisposition,
    evaluation_artifact_path: PathBuf,
    oracle_eligible_instances: usize,
    converged_instances: usize,
    nonempty_submission_instances: usize,
    applied_patch_instances: usize,
    total_tool_calls: usize,
    failed_tool_calls: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1RunnerReport {
    campaign_id: String,
    scheduler_path: PathBuf,
    scheduler: Prototype1SchedulerState,
    node: Prototype1RunnerNodeReport,
    runner_result: Option<Prototype1RunnerResult>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1RunnerNodeReport {
    node: Prototype1NodeRecord,
    workspace_exists: bool,
    binary_exists: bool,
    runner_result_exists: bool,
    runner_args: Vec<String>,
}

#[derive(Debug, Serialize)]
pub(crate) struct Prototype1StateReport {
    campaign_id: String,
    node_id: String,
    repo_root: PathBuf,
    journal_path: PathBuf,
    stop_after: Prototype1StateStopAfter,
    outcome: String,
    node_status: Prototype1NodeStatus,
    workspace_root: PathBuf,
    binary_path: PathBuf,
    child_runtime: Option<String>,
    successor_runtime: Option<String>,
    successor_pid: Option<u32>,
    successor_ready_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1BranchStatusReport {
    campaign_id: String,
    branch_registry_path: PathBuf,
    source_nodes: usize,
    active_targets: usize,
    active_target_state: Vec<Prototype1ActiveTargetReport>,
    branches: Vec<Prototype1BranchStateRow>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1ActiveTargetReport {
    target_relpath: PathBuf,
    source_state_id: String,
    active_branch_id: Option<String>,
    active_apply_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1BranchStateRow {
    instance_id: String,
    source_state_id: String,
    parent_branch_id: Option<String>,
    target_relpath: PathBuf,
    source_content_hash: String,
    selected_branch_id: Option<String>,
    branch_id: String,
    candidate_id: String,
    branch_label: String,
    status: String,
    apply_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1BranchShowReport {
    campaign_id: String,
    branch_registry_path: PathBuf,
    instance_id: String,
    source_state_id: String,
    parent_branch_id: Option<String>,
    target_relpath: PathBuf,
    source_content_hash: String,
    selected_branch_id: Option<String>,
    branch_id: String,
    candidate_id: String,
    branch_label: String,
    status: String,
    apply_id: Option<String>,
    proposed_content_hash: String,
    proposed_content: String,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct Prototype1BranchApplyReport {
    campaign_id: String,
    branch_registry_path: PathBuf,
    branch_id: String,
    candidate_id: String,
    source_state_id: String,
    target_relpath: PathBuf,
    absolute_path: PathBuf,
    changed: bool,
    apply_id: String,
    source_content_hash: String,
    applied_content_hash: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Prototype1BranchEvaluationReport {
    pub(crate) baseline_campaign_id: String,
    pub(crate) branch_id: String,
    pub(crate) treatment_campaign_id: String,
    pub(crate) branch_registry_path: PathBuf,
    pub(crate) evaluation_artifact_path: PathBuf,
    pub(crate) treatment_campaign_manifest: PathBuf,
    pub(crate) treatment_closure_state_path: PathBuf,
    pub(crate) overall_disposition: BranchDisposition,
    pub(crate) reasons: Vec<String>,
    pub(crate) compared_instances: Vec<Prototype1ComparedInstanceReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Prototype1ComparedInstanceReport {
    pub(crate) instance_id: String,
    pub(crate) baseline_record_path: Option<PathBuf>,
    pub(crate) treatment_record_path: Option<PathBuf>,
    pub(crate) baseline_metrics: Option<OperationalRunMetrics>,
    pub(crate) treatment_metrics: Option<OperationalRunMetrics>,
    pub(crate) evaluation: Option<BranchEvaluationResult>,
    pub(crate) status: String,
}

pub(crate) struct Prototype1LoopCampaign {
    pub(crate) campaign_id: String,
    pub(crate) manifest_path: PathBuf,
    pub(crate) closure_state_path: PathBuf,
    pub(crate) slice_dataset_path: PathBuf,
    pub(crate) resolved: ResolvedCampaignConfig,
}

fn print_prototype1_loop_report(report: &Prototype1LoopReport) {
    println!("prototype1 loop");
    println!("{}", "-".repeat(40));
    println!("stage_reached: {}", serde_name(&report.stage_reached));
    println!("dry_run: {}", yes_no(report.dry_run));
    println!(
        "search_policy: generations<={} nodes<={} stop_on_first_keep={} require_keep_for_continuation={}",
        report.search_policy.max_generations,
        report.search_policy.max_total_nodes,
        yes_no(report.search_policy.stop_on_first_keep),
        yes_no(report.search_policy.require_keep_for_continuation)
    );
    if let Some(decision) = report.continuation_decision.as_ref() {
        println!(
            "continuation: {} next_generation={} total_nodes_after_continue={} selected_next_branch_id={} selected_branch_disposition={}",
            serde_name(&decision.disposition),
            decision.next_generation,
            decision.total_nodes_after_continue,
            decision
                .selected_next_branch_id
                .as_deref()
                .unwrap_or("(none)"),
            decision
                .selected_branch_disposition
                .as_deref()
                .unwrap_or("(none)")
        );
    }
    println!(
        "continued_from_campaign: {}",
        report
            .continued_from_campaign
            .as_deref()
            .unwrap_or("(none)")
    );
    println!(
        "continued_from_branch_id: {}",
        report
            .continued_from_branch_id
            .as_deref()
            .unwrap_or("(none)")
    );
    println!("batch_id: {}", report.batch_id);
    println!("batch_manifest: {}", report.batch_manifest.display());
    println!("campaign_id: {}", report.campaign_id);
    println!("campaign_manifest: {}", report.campaign_manifest.display());
    println!("closure_state: {}", report.closure_state_path.display());
    println!("slice_dataset: {}", report.slice_dataset_path.display());
    println!("branch_registry: {}", report.branch_registry_path.display());
    println!("scheduler: {}", report.scheduler_path.display());
    println!("trace: {}", report.trace_path.display());
    println!(
        "prepared/completed/protocol_tasks: {}/{}/{}",
        report.prepared_instances.len(),
        report.completed_instances.len(),
        report.protocol_task_instances.len()
    );
    println!();
    println!("baseline instances");
    println!("{}", "-".repeat(40));
    for instance in &report.baseline_instances {
        println!(
            "- {} eval_status={} protocol_status={} protocol_completed={} protocol_evidence={}",
            instance.instance_id,
            serde_name(&instance.eval_status),
            serde_name(&instance.protocol_status),
            yes_no(instance.protocol_completed),
            yes_no(instance.protocol_evidence_available)
        );
        if let Some(record_path) = instance.record_path.as_ref() {
            println!("  record: {}", record_path.display());
        }
        if let Some(protocol_failure) = instance.protocol_failure.as_ref() {
            println!("  protocol_failure: {}", protocol_failure);
        }
    }
    println!();
    println!("selected targets");
    println!("{}", "-".repeat(40));
    if report.selected_targets.is_empty() {
        println!("(none)");
    } else {
        for target in &report.selected_targets {
            println!("- {}", target.instance_id);
            print_issue_case_block("  primary_issue", &target.issue);
            println!("  source_state_id: {}", target.source_state_id);
            println!(
                "  parent_branch_id: {}",
                target.parent_branch_id.as_deref().unwrap_or("(none)")
            );
            println!("  selected_branch_id: {}", target.selected_branch_id);
            println!(
                "  synthesized_candidates: {}",
                target.synthesized_candidate_count
            );
            println!("  selected_candidate_id: {}", target.selected_candidate_id);
            println!("  synthesized_spec_id: {}", target.synthesized_spec_id);
            println!(
                "  synthesized_target: {}",
                target.synthesized_target_relpath.display()
            );
            if let Some(apply_output) = target.apply_output.as_ref() {
                println!("  applied_candidate_id: {}", apply_output.candidate_id);
                println!("  apply_id: {}", apply_output.apply_id);
                println!("  apply_changed: {}", yes_no(apply_output.changed));
                println!("  apply_target: {}", apply_output.target_relpath.display());
            }
            if let Some(reason) = target.apply_skipped_reason.as_ref() {
                println!("  apply_skipped: {reason}");
            }
        }
    }
    println!();
    println!("staged nodes");
    println!("{}", "-".repeat(40));
    if report.staged_nodes.is_empty() {
        println!("(none)");
    } else {
        for node in &report.staged_nodes {
            println!("- {}", node.node_id);
            println!(
                "  parent_node_id: {}",
                node.parent_node_id.as_deref().unwrap_or("(none)")
            );
            println!("  generation: {}", node.generation);
            println!("  instance_id: {}", node.instance_id);
            println!("  source_state_id: {}", node.source_state_id);
            println!(
                "  parent_branch_id: {}",
                node.parent_branch_id.as_deref().unwrap_or("(none)")
            );
            println!("  branch_id: {}", node.branch_id);
            println!("  candidate_id: {}", node.candidate_id);
            println!("  status: {}", serde_name(&node.status));
            println!("  target: {}", node.target_relpath.display());
            println!("  workspace_root: {}", node.workspace_root.display());
            println!("  binary_path: {}", node.binary_path.display());
        }
    }
    println!();
    println!("branch evaluations");
    println!("{}", "-".repeat(40));
    if report.branch_evaluations.is_empty() {
        println!("(none)");
    } else {
        for evaluation in &report.branch_evaluations {
            println!("- {}", evaluation.branch_id);
            println!("  instance_id: {}", evaluation.instance_id);
            println!("  source_state_id: {}", evaluation.source_state_id);
            println!(
                "  parent_branch_id: {}",
                evaluation.parent_branch_id.as_deref().unwrap_or("(none)")
            );
            println!("  candidate_id: {}", evaluation.candidate_id);
            println!("  branch_label: {}", evaluation.branch_label);
            println!(
                "  overall_disposition: {}",
                serde_name(&evaluation.overall_disposition)
            );
            println!(
                "  oracle/converged/nonempty/applied: {}/{}/{}/{}",
                evaluation.oracle_eligible_instances,
                evaluation.converged_instances,
                evaluation.nonempty_submission_instances,
                evaluation.applied_patch_instances
            );
            println!(
                "  tool_calls_total/failed: {}/{}",
                evaluation.total_tool_calls, evaluation.failed_tool_calls
            );
            println!(
                "  treatment_campaign_id: {}",
                evaluation.treatment_campaign_id
            );
        }
    }
    println!(
        "selected_next_branch_id: {}",
        report
            .selected_next_branch_id
            .as_deref()
            .unwrap_or("(none)")
    );
    println!();
    if !report.protocol_failures.is_empty() {
        println!("protocol failures");
        println!("{}", "-".repeat(40));
        for failure in &report.protocol_failures {
            println!("- {}", failure);
        }
        println!();
    }
    println!("pending");
    println!("{}", "-".repeat(40));
    for stage in &report.pending_stages {
        println!("- {}", stage);
    }
}

fn print_prototype1_branch_status_report(report: &Prototype1BranchStatusReport) {
    println!("prototype1 branch state");
    println!("{}", "-".repeat(40));
    println!("campaign_id: {}", report.campaign_id);
    println!("branch_registry: {}", report.branch_registry_path.display());
    println!(
        "source_nodes/active_targets/branches: {}/{}/{}",
        report.source_nodes,
        report.active_targets,
        report.branches.len()
    );
    println!();
    println!("active targets");
    println!("{}", "-".repeat(40));
    if report.active_target_state.is_empty() {
        println!("(none)");
    } else {
        for target in &report.active_target_state {
            println!("- {}", target.target_relpath.display());
            println!("  source_state_id: {}", target.source_state_id);
            if let Some(branch_id) = target.active_branch_id.as_ref() {
                println!("  active_branch_id: {}", branch_id);
            } else {
                println!("  active_branch_id: (none)");
            }
            if let Some(apply_id) = target.active_apply_id.as_ref() {
                println!("  active_apply_id: {}", apply_id);
            }
        }
    }
    println!();
    println!("branches");
    println!("{}", "-".repeat(40));
    if report.branches.is_empty() {
        println!("(none)");
    } else {
        for branch in &report.branches {
            println!("- {}", branch.branch_id);
            println!("  instance_id: {}", branch.instance_id);
            println!("  source_state_id: {}", branch.source_state_id);
            println!(
                "  parent_branch_id: {}",
                branch.parent_branch_id.as_deref().unwrap_or("(none)")
            );
            println!("  target: {}", branch.target_relpath.display());
            println!("  candidate_id: {}", branch.candidate_id);
            println!("  branch_label: {}", branch.branch_label);
            println!("  status: {}", branch.status);
            if let Some(selected_branch_id) = branch.selected_branch_id.as_ref() {
                println!("  selected_branch_id: {}", selected_branch_id);
            }
            if let Some(apply_id) = branch.apply_id.as_ref() {
                println!("  apply_id: {}", apply_id);
            }
        }
    }
}

fn print_prototype1_branch_show_report(report: &Prototype1BranchShowReport) {
    println!("prototype1 branch show");
    println!("{}", "-".repeat(40));
    println!("campaign_id: {}", report.campaign_id);
    println!("branch_id: {}", report.branch_id);
    println!("candidate_id: {}", report.candidate_id);
    println!("branch_label: {}", report.branch_label);
    println!("status: {}", report.status);
    println!("instance_id: {}", report.instance_id);
    println!("source_state_id: {}", report.source_state_id);
    println!(
        "parent_branch_id: {}",
        report.parent_branch_id.as_deref().unwrap_or("(none)")
    );
    println!("target: {}", report.target_relpath.display());
    println!(
        "selected_branch_id: {}",
        report.selected_branch_id.as_deref().unwrap_or("(none)")
    );
    println!(
        "apply_id: {}",
        report.apply_id.as_deref().unwrap_or("(none)")
    );
    println!("source_content_hash: {}", report.source_content_hash);
    println!("proposed_content_hash: {}", report.proposed_content_hash);
    println!("content:");
    println!("{}", "-".repeat(40));
    print!("{}", report.proposed_content);
    if !report.proposed_content.ends_with('\n') {
        println!();
    }
}

fn print_prototype1_branch_apply_report(report: &Prototype1BranchApplyReport) {
    println!("prototype1 branch apply");
    println!("{}", "-".repeat(40));
    println!("campaign_id: {}", report.campaign_id);
    println!("branch_id: {}", report.branch_id);
    println!("candidate_id: {}", report.candidate_id);
    println!("source_state_id: {}", report.source_state_id);
    println!("target: {}", report.target_relpath.display());
    println!("absolute_path: {}", report.absolute_path.display());
    println!("changed: {}", yes_no(report.changed));
    println!("apply_id: {}", report.apply_id);
    println!("source_content_hash: {}", report.source_content_hash);
    println!("applied_content_hash: {}", report.applied_content_hash);
}

fn print_prototype1_branch_evaluation_report(report: &Prototype1BranchEvaluationReport) {
    println!("prototype1 branch evaluation");
    println!("{}", "-".repeat(40));
    println!("baseline_campaign_id: {}", report.baseline_campaign_id);
    println!("branch_id: {}", report.branch_id);
    println!("treatment_campaign_id: {}", report.treatment_campaign_id);
    println!(
        "treatment_campaign_manifest: {}",
        report.treatment_campaign_manifest.display()
    );
    println!(
        "treatment_closure_state: {}",
        report.treatment_closure_state_path.display()
    );
    println!("branch_registry: {}", report.branch_registry_path.display());
    println!(
        "evaluation_artifact: {}",
        report.evaluation_artifact_path.display()
    );
    println!(
        "overall_disposition: {}",
        serde_name(&report.overall_disposition)
    );
    println!();
    println!("instances");
    println!("{}", "-".repeat(40));
    for row in &report.compared_instances {
        println!("- {} [{}]", row.instance_id, row.status);
        if let Some(path) = row.baseline_record_path.as_ref() {
            println!("  baseline_record: {}", path.display());
        }
        if let Some(path) = row.treatment_record_path.as_ref() {
            println!("  treatment_record: {}", path.display());
        }
        if let Some(evaluation) = row.evaluation.as_ref() {
            println!("  disposition: {}", serde_name(&evaluation.disposition));
            for reason in &evaluation.reasons {
                println!("  reason: {}", reason);
            }
        }
    }
    if !report.reasons.is_empty() {
        println!();
        println!("reasons");
        println!("{}", "-".repeat(40));
        for reason in &report.reasons {
            println!("- {}", reason);
        }
    }
}

fn print_prototype1_runner_report(report: &Prototype1RunnerReport) {
    println!("prototype1 runner");
    println!("{}", "-".repeat(40));
    println!("campaign_id: {}", report.campaign_id);
    println!("scheduler: {}", report.scheduler_path.display());
    let scheduler = &report.scheduler;
    println!(
        "search_policy: generations<={} nodes<={} stop_on_first_keep={} require_keep_for_continuation={}",
        scheduler.policy.max_generations,
        scheduler.policy.max_total_nodes,
        yes_no(scheduler.policy.stop_on_first_keep),
        yes_no(scheduler.policy.require_keep_for_continuation)
    );
    if let Some(decision) = scheduler.last_continuation_decision.as_ref() {
        println!(
            "continuation: {} next_generation={} total_nodes_after_continue={} selected_next_branch_id={} selected_branch_disposition={}",
            serde_name(&decision.disposition),
            decision.next_generation,
            decision.total_nodes_after_continue,
            decision
                .selected_next_branch_id
                .as_deref()
                .unwrap_or("(none)"),
            decision
                .selected_branch_disposition
                .as_deref()
                .unwrap_or("(none)")
        );
    }
    println!("frontier: {}", scheduler.frontier_node_ids.join(", "));
    if !scheduler.completed_node_ids.is_empty() {
        println!("completed: {}", scheduler.completed_node_ids.join(", "));
    }
    if !scheduler.failed_node_ids.is_empty() {
        println!("failed: {}", scheduler.failed_node_ids.join(", "));
    }
    println!();
    println!("node");
    println!("{}", "-".repeat(40));
    let node = &report.node.node;
    println!("node_id: {}", node.node_id);
    println!(
        "parent_node_id: {}",
        node.parent_node_id.as_deref().unwrap_or("(none)")
    );
    println!("generation: {}", node.generation);
    println!("status: {}", serde_name(&node.status));
    println!("instance_id: {}", node.instance_id);
    println!("source_state_id: {}", node.source_state_id);
    println!(
        "parent_branch_id: {}",
        node.parent_branch_id.as_deref().unwrap_or("(none)")
    );
    println!("branch_id: {}", node.branch_id);
    println!("candidate_id: {}", node.candidate_id);
    println!("target: {}", node.target_relpath.display());
    println!("node_dir: {}", node.node_dir.display());
    println!("workspace_root: {}", node.workspace_root.display());
    println!(
        "workspace_exists/binary_exists/result_exists: {}/{}/{}",
        yes_no(report.node.workspace_exists),
        yes_no(report.node.binary_exists),
        yes_no(report.node.runner_result_exists)
    );
    println!("binary_path: {}", node.binary_path.display());
    println!(
        "runner_request_path: {}",
        node.runner_request_path.display()
    );
    println!("runner_result_path: {}", node.runner_result_path.display());
    println!("runner_args: {}", report.node.runner_args.join(" "));
    if let Some(result) = report.runner_result.as_ref() {
        println!();
        println!("runner result");
        println!("{}", "-".repeat(40));
        println!("disposition: {}", serde_name(&result.disposition));
        println!("status: {}", serde_name(&result.status));
        println!(
            "treatment_campaign_id: {}",
            result.treatment_campaign_id.as_deref().unwrap_or("(none)")
        );
        println!(
            "evaluation_artifact_path: {}",
            result
                .evaluation_artifact_path
                .as_ref()
                .map(|path| path.display().to_string())
                .unwrap_or_else(|| "(none)".to_string())
        );
        if let Some(detail) = result.detail.as_deref() {
            println!("detail: {detail}");
        }
        if let Some(code) = result.exit_code {
            println!("exit_code: {code}");
        }
    }
}

fn prototype1_trace_path(campaign_manifest_path: &Path) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1-loop-trace.json")
}
