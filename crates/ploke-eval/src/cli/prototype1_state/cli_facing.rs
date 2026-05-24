#[cfg(test)]
#[path = "tests/cli_tests.rs"]
mod tests;

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    io::{self, Write},
    path::{Path, PathBuf},
    str::FromStr,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant, SystemTime},
};

use chrono::{DateTime, Utc};
use ploke_core::EXECUTION_DEBUG_TARGET;
use ploke_llm::{
    HttpBodyFailure, ModelId, ProviderAttempt, ProviderAttemptOutcome, ProviderAttemptTimeline,
    ProviderKey, request::models::ModelRouteSource,
};
use ploke_tui::tools::ToolName;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{Instrument, debug, error, info, instrument, warn};

use crate::{
    BenchmarkFamily, BranchDisposition, BranchEvaluationInput, BranchEvaluationResult,
    CampaignManifest, CampaignOverrides, ClosureClass, EvalBudget, EvalCampaignPolicy,
    OperationalRunMetrics, OutputMode, PrepareMsbBatchRequest, PrepareWrite, PreparedMsbBatch,
    ProtocolCampaignPolicy, RegistryDatasetSource, ResolvedCampaignConfig, batches_dir,
    campaign::campaign_closure_state_path,
    campaign_manifest_path,
    cli::{
        Depth, HistoryCommand, HistorySubcommand, InspectOutputFormat,
        Prototype1CandidateGenerator, Prototype1ChildEvidenceCommand,
        Prototype1ChildScheduleMode as CliChildScheduleMode, Prototype1EditSurface,
        Prototype1LoopCommand, Prototype1LoopStopAfter, Prototype1MetricsCommand,
        Prototype1MonitorPeekCommand, Prototype1MonitorTimingCommand,
        Prototype1MonitorWatchCommand, Prototype1ScoreCommand, Prototype1SelectionShowCommand,
        Prototype1StateCommand, Prototype1StateStopAfter, Prototype1SuccessorSelection,
        Prototype1TraversalMetrics, TimingTrace, advance_eval_closure, advance_protocol_closure,
        default_batch_id, pending_prototype1_stages, persist_intervention_apply_for_record,
        persist_intervention_synthesis_for_record, persist_issue_detection_for_record,
        print_issue_case_block,
        prototype1_process::{
            SuccessorHandoffMode, cleanup_prototype1_child_build_products,
            persist_prototype1_buildable_child_artifact, record_prototype1_successor_completion,
            record_prototype1_successor_ready, spawn_and_handoff_prototype1_successor,
            validate_child_surface, validate_prototype1_successor_continuation,
        },
        prototype1_state::{
            backend::{
                AdmittedBroadHarnessResult, AttemptRejection, CheckedSurfaceEdit,
                EVAL_CORE_SURFACE_ROOT, EditProposal, EditSurfaceAdmission, GitWorktreeBackend,
                ProposedTouch, TuiAttemptOutcome, WorkspaceBackend, edit_surface_paths,
            },
            c1::{C1, MaterializeBranch},
            c2::BuildChild,
            c3::SpawnChild,
            c4::{ObserveChild, ObservedChild},
            edit_surface::{
                harness_result::{
                    SubmittedBroadHarnessResult, SubmittedChangeSummary,
                    SubmittedCheckRecommendation, SubmittedEvidenceCitation, SubmittedFileChange,
                    SubmittedHarnessReturnEvidence, SubmittedImprovementRationale, transaction,
                },
                tui_adapter,
            },
            event::RecordedAt,
            history::{
                ArtifactSurface, CandidateArtifact, CandidateCoordinate, CandidateLifecycle,
                CandidateMembershipId, CandidateOccurrenceId, CandidateSetCommitment,
                CheckedSurface, CheckedSurfaceTransition, EvaluationPayload, Generation, History,
                HistoryHash, ProcedureRef, Scope, ScopeFor, SealedBranchEvidence,
                SealedCandidateEvidence, SealedComparedRunEvidence, SealedEvalSetIdentity,
                SealedEvaluationEvidence, SealedEvaluatorIdentity, SealedEvidenceCitation,
                SealedRuntimeEvidence, SelectionDecisionEntry, SelectionProjectionFailure,
                SelectionProjectionFailureKind, SelectionScope, SubjectRef, SurfaceArtifactRef,
                SurfaceEvidence, SurfaceTouch, SurfaceWritable, TraversalCandidateSource,
                TraversalEvidence, surface_attempt,
            },
            identity::{
                ParentIdentity, load_parent_identity_optional, parent_identity_commit_message,
                parent_identity_relpath, write_parent_identity,
            },
            inner::{Locked, Open, Received},
            invocation::{
                self, InvocationAuthority, SuccessorCompletionStatus, SuccessorInvocation,
            },
            journal::{
                self, JournalEntry, ParentStartedEntry, PrototypeJournal,
                prototype1_transition_journal_path,
            },
            observe,
            parent::{
                AwaitingHarnessPlan, Check, ChildFiles, ChildPlan, ChildPlanFile, ChildPlanFiles,
                Genesis, Parent, Planned, Predecessor, Ready, Selectable, Startup, Unchecked,
            },
            profile, selection as state_selection,
            successor::Record as SuccessorRecord,
            telemetry::RuntimeTelemetry,
        },
        resolve_batch_manifest, resolve_protocol_model_id, resolve_protocol_provider_slug,
        sanitize_batch_component, serde_name, write_json_file_pretty, yes_no,
    },
    evaluate_branch, instances_dir,
    intervention::{
        ArtifactEdit, BaselineInstance, CompleteBaseline, Intervention, InterventionApplyInput,
        InterventionCandidate, InterventionSpec, IssueCase, Outcome,
        PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION, Prototype1ChildBudget,
        Prototype1ChildScheduleMode, Prototype1ContinuationDecision,
        Prototype1ContinuationDisposition, Prototype1NodeRecord, Prototype1NodeStatus,
        Prototype1SearchPolicy, RecordStore, TreatmentBranchNode, TreatmentBranchStatus,
        ValidationPolicy, branch_log, execute_intervention_apply, load_node_record,
        load_scheduler_state, project_node_status, prototype1_branch_registry_path,
        prototype1_node_id, prototype1_nodes_dir, prototype1_scheduler_path,
        register_root_parent_node, resolved_treatment_branches_from_synthesis,
        select_primary_issue, treatment_branch_id, write_node_projection,
        write_treatment_evaluation_projection,
    },
    load_campaign_manifest, load_closure_state,
    model_registry::resolve_model_for_run,
    projection::OperatorProjectionRead,
    protocol::load_protocol_aggregate,
    provider_prefs::load_provider_for_model,
    recompute_closure_state,
    record::{AgentTurnTraceProjection, RunRecord, read_compressed_record},
    repos_dir, resolve_campaign_config, save_campaign_manifest,
    selection::{
        ActivePrototype1MonitorTarget, load_active_selection, save_active_prototype1_monitor_target,
    },
    spec::PrepareError,
    successor_selection::{
        CandidateRef, RunComparison, SelectionInput, SuccessorDecision,
        decision::SuccessorOutcome,
        traversal::{self as traversal_selection, StrategyKind},
    },
};
use ploke_records::llm_response::{FULL_RESPONSE_TRACE_FILE, RawFullResponseRecord};

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

#[derive(Debug, Clone, Serialize)]
struct Prototype1SetupReport {
    campaign_id: String,
    campaign_manifest: PathBuf,
    closure_state_path: PathBuf,
    slice_dataset_path: PathBuf,
    scheduler_path: PathBuf,
    batch_id: String,
    batch_manifest: PathBuf,
    primary_instance_id: String,
    eval_instances: Vec<String>,
    repo_root: PathBuf,
    artifact_branch: String,
    parent_identity_path: PathBuf,
    parent_id: String,
    node_id: String,
    generation: u32,
    branch_id: String,
    search_policy: Prototype1SearchPolicy,
    run_profile: Option<profile::RunProfileCommitment>,
}

fn prepare_prototype1_parent_setup(
    command: &Prototype1LoopCommand,
) -> Result<Prototype1SetupReport, PrepareError> {
    let operator_profile = command
        .profile
        .as_deref()
        .map(profile::load_operator_profile)
        .transpose()?;
    let profile_ref = operator_profile.as_ref().map(|profile| &profile.profile);
    let (batch_manifest, prepared_batch) = prepare_or_load_prototype1_batch(command, profile_ref)?;
    if prepared_batch.instances.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "prototype1 setup requires at least one prepared benchmark instance"
                .to_string(),
        });
    }
    if prepared_batch.instances.len() != 1
        && !profile_ref.is_some_and(|profile| {
            !matches!(profile.generation.source, profile::GenerationSource::Legacy)
        })
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1 setup supports multi-instance child self-eval only through a non-legacy run profile; got {} prepared instances",
                prepared_batch.instances.len()
            ),
        });
    }
    let primary_instance_id =
        resolve_setup_primary_instance(command, profile_ref, &prepared_batch.instances)?;

    let campaign = prepare_prototype1_loop_campaign(command, &prepared_batch, profile_ref)?;
    let admitted_profile = operator_profile
        .as_ref()
        .map(|profile| profile::admit_run_profile(&campaign.manifest_path, profile))
        .transpose()?;
    let closure_state_path = ensure_prototype1_baseline_closure_state(&campaign.resolved)?;
    let repo_root = std::env::current_dir().map_err(|source| PrepareError::ReadManifest {
        path: PathBuf::from("."),
        source,
    })?;
    let search_policy = if let Some(profile) = admitted_profile.as_ref() {
        profile.profile.search_policy()
    } else {
        search_policy_from_command(command)?
    };
    let artifact_branch = format!(
        "prototype1-parent-{}-gen0",
        sanitize_batch_component(&campaign.campaign_id)
    );
    let node = register_root_parent_node(
        &campaign.campaign_id,
        &campaign.manifest_path,
        &primary_instance_id,
        &artifact_branch,
        &repo_root,
        search_policy.clone(),
    )?;

    let backend = GitWorktreeBackend;
    let _ = backend
        .checkout_fresh_parent_branch(&repo_root, &artifact_branch)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_setup_parent_branch",
            detail: source.to_string(),
        })?;
    let identity = ParentIdentity::from_node(
        campaign.campaign_id.clone(),
        &node,
        None,
        Some(artifact_branch.clone()),
    );
    let parent_identity_path = write_parent_identity(&repo_root, &identity)?;
    let message = parent_identity_commit_message(&identity);
    let _ = backend
        .persist_active_checkout_files(&repo_root, &[parent_identity_relpath()], &message)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_setup_parent_identity_commit",
            detail: source.to_string(),
        })?;
    backend
        .validate_parent_checkout(&repo_root, &identity)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_setup_parent_checkout",
            detail: source.to_string(),
        })?;

    Ok(Prototype1SetupReport {
        campaign_id: campaign.campaign_id,
        campaign_manifest: campaign.manifest_path.clone(),
        closure_state_path,
        slice_dataset_path: campaign.slice_dataset_path,
        scheduler_path: prototype1_scheduler_path(&campaign.manifest_path),
        batch_id: prepared_batch.batch_id,
        batch_manifest,
        primary_instance_id,
        eval_instances: prepared_batch.instances,
        repo_root,
        artifact_branch,
        parent_identity_path,
        parent_id: identity.parent_id().to_string(),
        node_id: identity.node_id().to_string(),
        generation: identity.generation(),
        branch_id: identity.branch_id().to_string(),
        search_policy,
        run_profile: admitted_profile.map(|profile| profile.commitment),
    })
}

fn print_prototype1_setup_report(report: &Prototype1SetupReport) {
    println!("prototype1 setup");
    println!("{}", "-".repeat(40));
    println!("campaign_id: {}", report.campaign_id);
    println!("campaign_manifest: {}", report.campaign_manifest.display());
    println!("closure_state: {}", report.closure_state_path.display());
    println!("slice_dataset: {}", report.slice_dataset_path.display());
    println!("scheduler: {}", report.scheduler_path.display());
    println!("batch_id: {}", report.batch_id);
    println!("batch_manifest: {}", report.batch_manifest.display());
    println!("primary_instance_id: {}", report.primary_instance_id);
    println!("eval_instances: {}", report.eval_instances.join(", "));
    println!("repo_root: {}", report.repo_root.display());
    println!("artifact_branch: {}", report.artifact_branch);
    println!("parent_identity: {}", report.parent_identity_path.display());
    println!("parent_id: {}", report.parent_id);
    println!("node_id: {}", report.node_id);
    println!("generation: {}", report.generation);
    println!("branch_id: {}", report.branch_id);
    println!(
        "search_policy: generations<={} nodes<={} children={}..={} mode={} stop_on_first_keep={} require_keep_for_continuation={} explore_from_rejected={}",
        report.search_policy.max_generations,
        report.search_policy.max_total_nodes,
        report.search_policy.child_budget.min,
        report.search_policy.child_budget.max,
        serde_name(&report.search_policy.child_schedule_mode),
        yes_no(report.search_policy.stop_on_first_keep),
        yes_no(report.search_policy.require_keep_for_continuation),
        yes_no(report.search_policy.explore_from_rejected)
    );
    if let Some(commitment) = report.run_profile.as_ref() {
        println!("run_profile: {}", commitment.profile_path.display());
        println!("run_profile_sha256: {}", commitment.sha256);
    }
    println!();
    println!("next:");
    println!(
        "  ./target/debug/ploke-eval select campaign {}",
        report.campaign_id
    );
    println!(
        "  ./target/debug/ploke-eval select instance {}",
        report.primary_instance_id
    );
    println!("  ./target/debug/ploke-eval loop prototype1-state --repo-root .");
}

fn resolve_setup_primary_instance(
    command: &Prototype1LoopCommand,
    run_profile: Option<&profile::Prototype1RunProfile>,
    prepared_instances: &[String],
) -> Result<String, PrepareError> {
    if let Some(primary) = command.instance.first() {
        return Ok(primary.clone());
    }
    if let Some(primary) = run_profile.and_then(|profile| profile.target.primary_instance()) {
        if prepared_instances
            .iter()
            .any(|instance| instance == primary)
        {
            return Ok(primary.to_string());
        }
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "run profile primary target.instance '{}' is not present in the prepared eval cohort",
                primary
            ),
        });
    }
    prepared_instances
        .first()
        .cloned()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "prototype1 setup requires at least one prepared benchmark instance"
                .to_string(),
        })
}

fn load_existing_prototype1_campaign(
    campaign_id: &str,
) -> Result<Prototype1LoopCampaign, PrepareError> {
    let manifest_path = campaign_manifest_path(campaign_id)?;
    let manifest = load_campaign_manifest(campaign_id)?;
    let resolved = resolve_campaign_config(campaign_id, &CampaignOverrides::default())?;
    let closure_state_path = campaign_closure_state_path(campaign_id)?;
    let slice_dataset_path = manifest
        .dataset_sources
        .first()
        .map(|source| source.path.clone())
        .unwrap_or_else(|| {
            manifest_path
                .parent()
                .unwrap_or_else(|| Path::new("."))
                .join("slice.jsonl")
        });

    Ok(Prototype1LoopCampaign {
        campaign_id: campaign_id.to_string(),
        manifest_path,
        closure_state_path,
        slice_dataset_path,
        resolved,
    })
}

pub(crate) fn ensure_prototype1_baseline_closure_state(
    config: &ResolvedCampaignConfig,
) -> Result<PathBuf, PrepareError> {
    let path = campaign_closure_state_path(&config.campaign_id)?;
    if path.exists() {
        load_closure_state(&config.campaign_id)?;
        return Ok(path);
    }
    recompute_closure_state(config.closure_recompute_request()).map(|(path, _)| path)
}

pub(crate) async fn establish_parent_baseline(
    campaign_id: &str,
    config: &ResolvedCampaignConfig,
    manifest_path: &Path,
    parent: &ParentIdentity,
) -> Result<CompleteBaseline, PrepareError> {
    let baseline = if parent.generation() == 0 {
        establish_initial_parent_baseline(config, parent).await?
    } else {
        promote_selected_child_baseline(campaign_id, manifest_path, parent)?
    };
    baseline.validate_for_parent(campaign_id, parent.node_id(), parent.branch_id())?;
    Ok(baseline)
}

async fn establish_initial_parent_baseline(
    config: &ResolvedCampaignConfig,
    parent: &ParentIdentity,
) -> Result<CompleteBaseline, PrepareError> {
    let mut eval_policy = config.eval.clone();
    eval_policy.stop_on_error = false;
    advance_eval_closure(config, &eval_policy, false, None).await?;

    let mut protocol_policy = config.protocol.clone();
    protocol_policy.stop_on_error = false;
    advance_protocol_closure(config, &protocol_policy, false).await?;

    let closure = load_closure_state(&config.campaign_id)?;
    complete_baseline_from_closure(parent, &closure, &config.eval)
}

fn promote_selected_child_baseline(
    campaign_id: &str,
    manifest_path: &Path,
    parent: &ParentIdentity,
) -> Result<CompleteBaseline, PrepareError> {
    let report_path = prototype1_branch_evaluation_path(manifest_path, parent.branch_id());
    let text = fs::read_to_string(&report_path).map_err(|source| PrepareError::ReadManifest {
        path: report_path.clone(),
        source,
    })?;
    let report: Prototype1BranchEvaluationReport =
        serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
            path: report_path.clone(),
            source,
        })?;
    if report.baseline_campaign_id != campaign_id {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selected child baseline report campaign mismatch for parent '{}': expected {}, got {}",
                parent.node_id(),
                campaign_id,
                report.baseline_campaign_id
            ),
        });
    }
    if report.branch_id != parent.branch_id() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selected child baseline report branch mismatch for parent '{}': expected {}, got {}",
                parent.node_id(),
                parent.branch_id(),
                report.branch_id
            ),
        });
    }
    complete_baseline_from_selected_treatment(parent, &report)
}

fn complete_baseline_from_closure(
    parent: &ParentIdentity,
    closure: &crate::closure::ClosureState,
    eval_policy: &EvalCampaignPolicy,
) -> Result<CompleteBaseline, PrepareError> {
    let rows = closure
        .instances
        .iter()
        .map(|row| {
            if row.eval_status != ClosureClass::Complete {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "Parent<{}> cannot spawn children: baseline instance '{}' is {:?}",
                        parent.node_id(),
                        row.instance_id,
                        row.eval_status
                    ),
                });
            }
            let record_path =
                row.artifacts
                    .record_path
                    .clone()
                    .ok_or_else(|| PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "Parent<{}> cannot spawn children: baseline instance '{}' has no record_path",
                            parent.node_id(),
                            row.instance_id
                        ),
                    })?;
            let metrics = read_compressed_record(&record_path)
                .map(|record| record.operational_metrics())
                .map_err(|source| PrepareError::ReadManifest {
                    path: record_path.clone(),
                    source,
                })?;
            Ok(BaselineInstance {
                instance_id: row.instance_id.clone(),
                registration_path: row.artifacts.registration_path.clone(),
                record_path,
                metrics,
            })
        })
        .collect::<Result<Vec<_>, PrepareError>>()?;
    let instance_ids = rows
        .iter()
        .map(|row| row.instance_id.clone())
        .collect::<Vec<_>>();
    CompleteBaseline::complete(
        closure.campaign_id.clone(),
        parent.node_id().to_string(),
        parent.branch_id().to_string(),
        prototype1_eval_set_id(
            &closure.campaign_id,
            &closure.campaign_id,
            closure.config.benchmark_family,
            &closure.config.dataset_sources,
            eval_policy,
            &instance_ids,
        ),
        rows,
    )
}

fn complete_baseline_from_selected_treatment(
    parent: &ParentIdentity,
    report: &Prototype1BranchEvaluationReport,
) -> Result<CompleteBaseline, PrepareError> {
    let eval_set_id = report
        .eval_set_identity
        .as_ref()
        .map(|identity| identity.id.clone())
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "Parent<{}> cannot promote selected child baseline: evaluation report has no eval_set_identity",
                parent.node_id()
            ),
        })?;
    let rows = report
        .compared_instances
        .iter()
        .map(|row| {
            let record_path =
                row.treatment_record_path
                    .clone()
                    .ok_or_else(|| PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "Parent<{}> cannot promote selected child baseline: treatment instance '{}' has no record_path",
                            parent.node_id(),
                            row.instance_id
                        ),
                    })?;
            let metrics =
                row.treatment_metrics
                    .clone()
                    .ok_or_else(|| PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "Parent<{}> cannot promote selected child baseline: treatment instance '{}' has no metrics",
                            parent.node_id(),
                            row.instance_id
                        ),
                    })?;
            Ok(BaselineInstance {
                instance_id: row.instance_id.clone(),
                registration_path: row.treatment_registration_path.clone(),
                record_path,
                metrics,
            })
        })
        .collect::<Result<Vec<_>, PrepareError>>()?;
    CompleteBaseline::complete(
        report.baseline_campaign_id.clone(),
        parent.node_id().to_string(),
        parent.branch_id().to_string(),
        eval_set_id,
        rows,
    )
}

struct ChildPlanReceipt {
    parent: Parent<Selectable>,
    plan: Received<ChildPlan>,
    rejected_surface_attempts: Vec<surface_attempt::Evidence>,
}

struct HarnessRequestReceipt {
    parent: Parent<AwaitingHarnessPlan>,
    request_path: PathBuf,
    published:
        crate::cli::prototype1_state::edit_surface::harness_request::PublishedBroadHarnessRequest,
}

struct HarnessRequestSlot {
    request_path: PathBuf,
    published:
        crate::cli::prototype1_state::edit_surface::harness_request::PublishedBroadHarnessRequest,
}

struct HarnessRequestBatch {
    parent: Parent<AwaitingHarnessPlan>,
    slots: Vec<HarnessRequestSlot>,
    child_budget: Prototype1ChildBudget,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct BroadTuiAttemptOptions {
    model: Option<tui_adapter::ModelSelection>,
    max_attempts: Option<u32>,
    timeout_secs: Option<u64>,
}

impl BroadTuiAttemptOptions {
    pub(crate) fn for_parent_patcher_defaults(
        max_attempts: Option<u32>,
        timeout_secs: Option<u64>,
    ) -> Result<Self, PrepareError> {
        Ok(Self {
            model: Some(crate::cli::load_parent_patcher_model_selection()?),
            max_attempts,
            timeout_secs,
        })
    }

    pub(crate) fn from_cli(
        model_id: Option<String>,
        provider: Option<String>,
        max_attempts: Option<u32>,
        timeout_secs: Option<u64>,
    ) -> Result<Self, PrepareError> {
        let model = match (model_id, provider) {
            (Some(model_id), provider) => {
                let model_id =
                    ModelId::from_str(&model_id).map_err(|err| PrepareError::DatabaseSetup {
                        phase: "broad_tui_attempt_model_id",
                        detail: format!("invalid model id '{model_id}': {err}"),
                    })?;
                let provider = provider
                    .map(|provider| {
                        ProviderKey::new(&provider).map_err(|err| PrepareError::DatabaseSetup {
                            phase: "broad_tui_attempt_provider",
                            detail: format!("invalid provider slug '{provider}': {err}"),
                        })
                    })
                    .transpose()?;
                Some(crate::cli::headless_model_selection(model_id, provider)?)
            }
            (None, Some(provider)) => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!("broad TUI attempt provider '{provider}' requires --model-id"),
                });
            }
            (None, None) => Some(crate::cli::load_parent_patcher_model_selection()?),
        };
        Ok(Self {
            model,
            max_attempts,
            timeout_secs,
        })
    }

    fn model(&self) -> Option<&tui_adapter::ModelSelection> {
        self.model.as_ref()
    }

    fn model_label(&self) -> Option<String> {
        self.model().map(|model| model.model_id().to_string())
    }

    fn provider_label(&self) -> Option<String> {
        self.model()
            .and_then(|model| model.provider())
            .map(|provider| provider.slug.as_str().to_string())
    }
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct BroadHarnessAttemptProjection {
    pub(crate) request_path: PathBuf,
    pub(crate) request_id: String,
    pub(crate) workspace_path: PathBuf,
    pub(crate) submitted_result_path: PathBuf,
    pub(crate) diagnostics_path: PathBuf,
    pub(crate) model_id: Option<String>,
    pub(crate) provider: Option<String>,
    pub(crate) max_attempts: u32,
    pub(crate) timeout_secs: u64,
    pub(crate) elapsed_ms: u128,
    pub(crate) status: String,
    pub(crate) executor_run_id: Option<String>,
    pub(crate) executor_attempt_id: Option<String>,
    pub(crate) changed_paths: Vec<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

enum ParentTargetSelection {
    ChildPlan(ChildPlanReceipt),
    AwaitingHarnessBatch(HarnessRequestBatch),
}

pub(crate) struct PlannedChildren {
    pub(crate) parent: Parent<Selectable>,
    pub(crate) plan: Received<ChildPlan>,
    pub(crate) children: Vec<ChildFiles>,
    pub(crate) rejected_surface_attempts: Vec<surface_attempt::Evidence>,
}

#[derive(Debug)]
pub(crate) struct PlannedChildOutcome {
    pub(crate) plan_index: usize,
    pub(crate) node: Prototype1NodeRecord,
    pub(crate) node_id: String,
    pub(crate) outcome: String,
    pub(crate) node_status: Prototype1NodeStatus,
    pub(crate) workspace_root: PathBuf,
    pub(crate) binary_path: PathBuf,
    pub(crate) resolved: crate::intervention::ResolvedTreatmentBranch,
    pub(crate) child_runtime: Option<String>,
    pub(crate) evaluation_report: Option<Prototype1BranchEvaluationReport>,
    pub(crate) selection_input: Option<SelectionInput>,
    pub(crate) surface: Option<SurfaceEvidence>,
    pub(crate) artifact_surface: Option<ArtifactSurface>,
}

const BROAD_TUI_ATTEMPT_LIMIT: usize = 3;
const BROAD_TUI_FRESH_ATTEMPTS_PER_CHILD: usize = 3;
const BROAD_TUI_STASH_TRANSFER_ENV: &str = "PLOKE_EVAL_HEADLESS_TUI_STASH_TRANSFER";

struct DeterministicTuiToolsCandidates {
    checked: Vec<CheckedSurfaceEdit>,
    rejected_attempts: Vec<surface_attempt::Evidence>,
}

pub(crate) struct SelectionSealMaterial {
    procedure: ProcedureRef,
    scope: SelectionScope,
    selected_candidate: SubjectRef,
    selected_occurrence_id: Option<CandidateOccurrenceId>,
    selected_membership_id: Option<CandidateMembershipId>,
    considered: Vec<EvaluationPayload>,
    considered_sources: Vec<TraversalCandidateSource>,
    projection_failures: Vec<SelectionProjectionFailure>,
    traversal: Option<TraversalEvidence>,
    metrics: crate::successor_selection::metrics::Set,
    selected_from_generation_outcomes: bool,
}

impl SelectionSealMaterial {
    fn selected_payload(&self) -> Result<&EvaluationPayload, PrepareError> {
        let candidate_set = self.candidate_set_commitment()?;
        if let Some(membership_id) = self.selected_membership_id.as_ref() {
            let membership = candidate_set
                .membership_by_membership_id(membership_id)
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: "selected membership is absent from sealed considered set".to_string(),
                })?;
            return self.payload_by_hash(&membership.payload_hash);
        }
        if let Some(occurrence_id) = self.selected_occurrence_id.as_ref() {
            let membership = candidate_set
                .membership_by_occurrence_id(occurrence_id)
                .ok_or_else(|| PrepareError::InvalidBatchSelection {
                    detail: "selected occurrence is absent from sealed considered set".to_string(),
                })?;
            return self.payload_by_hash(&membership.payload_hash);
        }
        self.considered
            .iter()
            .find(|payload| payload.candidate == self.selected_candidate)
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "selected candidate {} is absent from sealed considered set",
                    self.selected_candidate.as_str()
                ),
            })
    }

    fn candidate_set_commitment(&self) -> Result<CandidateSetCommitment, PrepareError> {
        if self.considered_sources.is_empty() {
            CandidateSetCommitment::from_payloads(&self.considered)
        } else {
            CandidateSetCommitment::from_payloads_with_sources(
                &self.considered,
                &self.considered_sources,
            )
        }
        .map_err(|err| PrepareError::InvalidBatchSelection {
            detail: format!("failed to reconstruct sealed considered set: {err}"),
        })
    }

    fn payload_by_hash(
        &self,
        payload_hash: &HistoryHash,
    ) -> Result<&EvaluationPayload, PrepareError> {
        let matches = self
            .considered
            .iter()
            .filter_map(|payload| {
                payload
                    .payload_hash()
                    .ok()
                    .filter(|hash| hash == payload_hash)
                    .map(|_| payload)
            })
            .collect::<Vec<_>>();
        match matches.as_slice() {
            [payload] => Ok(*payload),
            [] => Err(PrepareError::InvalidBatchSelection {
                detail: "selected member has no payload in sealed considered set".to_string(),
            }),
            many => Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "selected member resolves to multiple sealed payloads: count={}",
                    many.len()
                ),
            }),
        }
    }

    pub(crate) fn selected_artifact(&self) -> Result<CandidateArtifact, PrepareError> {
        self.selected_payload()?.artifact.clone().ok_or_else(|| {
            PrepareError::InvalidBatchSelection {
                detail: format!(
                    "selected candidate {} has no sealed Artifact payload for handoff hydration",
                    self.selected_candidate.as_str()
                ),
            }
        })
    }

    pub(crate) fn into_entry(
        self,
        decision: SuccessorDecision,
    ) -> Result<SelectionDecisionEntry, PrepareError> {
        SelectionDecisionEntry::new_with_traversal_identity_metrics(
            self.procedure,
            self.scope,
            Some(self.selected_candidate),
            self.selected_occurrence_id,
            self.selected_membership_id,
            self.considered,
            self.considered_sources,
            self.projection_failures,
            self.traversal,
            self.metrics,
            decision,
        )
        .map_err(|err| PrepareError::InvalidBatchSelection {
            detail: format!("failed to construct selection decision entry: {err}"),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CandidateGenerationConfig {
    Legacy,
    BroadHarnessRequest,
    DeterministicTuiTools,
}

impl CandidateGenerationConfig {
    fn from_command(command: &Prototype1StateCommand) -> Self {
        Self::from_generator(command.candidate_generator)
    }

    fn from_profile_generation(generation: profile::Generation) -> Self {
        Self::from_generator(generation.candidate_generator())
    }

    fn from_generator(generator: Prototype1CandidateGenerator) -> Self {
        match generator {
            Prototype1CandidateGenerator::Legacy => Self::Legacy,
            Prototype1CandidateGenerator::BroadHarnessRequest => Self::BroadHarnessRequest,
            Prototype1CandidateGenerator::DeterministicTuiTools => Self::DeterministicTuiTools,
        }
    }

    fn ensure_live_complete_admitted(self) -> Result<(), PrepareError> {
        match self {
            Self::Legacy => Err(PrepareError::InvalidBatchSelection {
                detail: "prototype1 hard stop before child planning: legacy candidate generation is disabled for live complete runs".to_string(),
            }),
            Self::BroadHarnessRequest => Ok(()),
            Self::DeterministicTuiTools => Ok(()),
        }
    }

    fn validate_received_child_plan(self, children: &[ChildFiles]) -> Result<(), PrepareError> {
        match self {
            Self::Legacy => Ok(()),
            Self::BroadHarnessRequest => {
                for child in children {
                    validate_requested_broad_harness_child(child)?;
                }
                Ok(())
            }
            Self::DeterministicTuiTools => {
                for child in children {
                    validate_requested_tui_surface_child(child)?;
                }
                Ok(())
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Prototype1StateRunShape {
    stop_after: Prototype1StateStopAfter,
    candidate_generation: CandidateGenerationConfig,
    successor_selection: Prototype1SuccessorSelection,
    successor_selection_seed: u64,
    successor_selection_metrics: Prototype1TraversalMetrics,
    successor_oracle_mode: crate::successor_selection::OracleMode,
    successor_oracle_require_evidence: bool,
    successor_metrics_policy: crate::successor_selection::metrics::Policy,
}

impl Prototype1StateRunShape {
    fn from_command(command: &Prototype1StateCommand) -> Self {
        Self {
            stop_after: command.stop_after,
            candidate_generation: CandidateGenerationConfig::from_command(command),
            successor_selection: command.successor_selection,
            successor_selection_seed: command.successor_selection_seed,
            successor_selection_metrics: command.successor_selection_metrics,
            successor_oracle_mode: crate::successor_selection::OracleMode::RecordOnly,
            successor_oracle_require_evidence: true,
            successor_metrics_policy: crate::successor_selection::metrics::Policy::default(),
        }
    }

    fn from_profile(profile: &profile::Prototype1RunProfile) -> Self {
        Self {
            stop_after: profile.execution.state_stop_after(),
            candidate_generation: CandidateGenerationConfig::from_profile_generation(
                profile.generation,
            ),
            successor_selection: profile.selection.successor_selection(),
            successor_selection_seed: profile.selection.seed,
            successor_selection_metrics: profile.selection.traversal_metrics(),
            successor_oracle_mode: profile.selection.oracle_mode(),
            successor_oracle_require_evidence: profile.selection.oracle_require_evidence(),
            successor_metrics_policy: profile.selection.metrics_policy(),
        }
    }

    fn resolve(
        command: &Prototype1StateCommand,
        manifest_path: &Path,
    ) -> Result<Self, PrepareError> {
        if let Some(admitted) = profile::load_admitted_run_profile(manifest_path)? {
            Ok(Self::from_profile(&admitted.profile))
        } else {
            Ok(Self::from_command(command))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
enum CandidateGenerationError {
    #[error(
        "candidate-generator=deterministic-tui-tools target '{}' for edit-surface={surface:?} is missing from the parent checkout",
        path.display()
    )]
    MissingEditSurfaceTarget {
        surface: Prototype1EditSurface,
        path: PathBuf,
    },
    #[error(
        "candidate-generator=deterministic-tui-tools produced {produced} unique checked candidates for edit-surface={surface:?}, fewer than required minimum {min}"
    )]
    InsufficientUniqueProposals {
        surface: Prototype1EditSurface,
        min: usize,
        produced: usize,
    },
    #[error(
        "checked edit for surface {actual:?} cannot be materialized through requested surface {expected:?}"
    )]
    SurfaceMismatch {
        expected: Prototype1EditSurface,
        actual: Prototype1EditSurface,
    },
    #[error(
        "checked edit target '{}' did not match child node target '{}'",
        checked.display(),
        node.display()
    )]
    TargetMismatch { checked: PathBuf, node: PathBuf },
    #[error("failed to persist checked edit-surface evidence: {detail}")]
    EvidenceProjection { detail: String },
    #[error(
        "deterministic tui edit-surface candidate '{node_id}' is missing checked edit-surface evidence"
    )]
    MissingDeterministicEvidence { node_id: String },
    #[error(
        "candidate-generator=broad-harness-request published request '{}' (request_id='{}', request_hash='{}'), prompt '{}', and isolated candidate workspace '{}'; awaiting staged edit result, with submitted-result path reserved at '{}'",
        request_path.display(),
        request_id,
        request_hash,
        prompt_path.display(),
        workspace_path.display(),
        submitted_result_path.display()
    )]
    PendingBroadHarnessRequest {
        request_id: String,
        request_hash: String,
        request_path: PathBuf,
        prompt_path: PathBuf,
        submitted_result_path: PathBuf,
        workspace_path: PathBuf,
    },
}

impl CandidateGenerationError {
    fn into_prepare(self) -> PrepareError {
        PrepareError::InvalidBatchSelection {
            detail: self.to_string(),
        }
    }
}

fn child_files_from_checked_edit(
    campaign_id: &str,
    edit_surface: Prototype1EditSurface,
    mut node: Prototype1NodeRecord,
    checked: &CheckedSurfaceEdit,
    stop_on_error: bool,
) -> Result<ChildFiles, CandidateGenerationError> {
    if checked.surface() != edit_surface {
        return Err(CandidateGenerationError::SurfaceMismatch {
            expected: edit_surface,
            actual: checked.surface(),
        });
    }
    if checked.target_relpath() != node.target_relpath.as_path() {
        return Err(CandidateGenerationError::TargetMismatch {
            checked: checked.target_relpath().to_path_buf(),
            node: node.target_relpath.clone(),
        });
    }

    let patch_id = checked.patch_id().clone();
    let base_artifact_id = checked.base_artifact_id().clone();
    let derived_artifact_id = checked.derived_artifact_id().clone();
    node.operation_target = Some(crate::loop_graph::OperationTarget::Artifact {
        artifact_id: base_artifact_id.clone(),
    });
    node.base_artifact_id = Some(base_artifact_id.clone());
    node.patch_id = Some(patch_id.clone());
    node.derived_artifact_id = Some(derived_artifact_id.clone());

    let resolved = resolved_from_checked_edit(&node, checked);
    let surface = checked
        .surface_evidence(TUI_EDIT_SURFACE_PRODUCER_ID)
        .map_err(|err| CandidateGenerationError::EvidenceProjection {
            detail: err.to_string(),
        })?;
    Ok(ChildFiles::from_resolved(campaign_id, node, resolved, stop_on_error).with_surface(surface))
}

fn resolved_from_checked_edit(
    node: &Prototype1NodeRecord,
    checked: &CheckedSurfaceEdit,
) -> crate::intervention::ResolvedTreatmentBranch {
    let patch_id = checked.patch_id().clone();
    let base_artifact_id = checked.base_artifact_id().clone();
    let derived_artifact_id = checked.derived_artifact_id().clone();

    crate::intervention::ResolvedTreatmentBranch {
        instance_id: node.instance_id.clone(),
        source_state_id: node.source_state_id.clone(),
        parent_branch_id: node.parent_branch_id.clone(),
        target_relpath: checked.target_relpath().to_path_buf(),
        source_content: checked.source_content().to_string(),
        source_content_hash: checked.source_content_hash().to_string(),
        selected_branch_id: Some(node.branch_id.clone()),
        branch: TreatmentBranchNode {
            branch_id: node.branch_id.clone(),
            candidate_id: node.candidate_id.clone(),
            patch_id: Some(patch_id),
            branch_label: format!("tui edit surface {}", node.candidate_id),
            synthesized_spec_id: TUI_EDIT_SURFACE_PRODUCER_ID.to_string(),
            proposed_content: checked.proposed_content().to_string(),
            proposed_content_hash: checked.proposed_content_hash().to_string(),
            generation_target: Some(crate::loop_graph::OperationTarget::Artifact {
                artifact_id: base_artifact_id,
            }),
            generation_coordinate: None,
            status: TreatmentBranchStatus::Selected,
            apply_id: Some(checked.proposal_id().to_string()),
            applied_content_hash: None,
            derived_artifact_id: Some(derived_artifact_id),
        },
    }
}

async fn run_parent_target_selection(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    parent: Parent<Ready>,
    config: CandidateGenerationConfig,
    child_budget: Prototype1ChildBudget,
) -> Result<ParentTargetSelection, PrepareError> {
    match config {
        CandidateGenerationConfig::Legacy => {
            run_legacy_parent_target_selection(campaign_id, manifest_path, repo_root, parent)
                .await
                .map(ParentTargetSelection::ChildPlan)
        }
        CandidateGenerationConfig::BroadHarnessRequest => {
            publish_broad_harness_child_plan_request(manifest_path, repo_root, parent, child_budget)
                .map(ParentTargetSelection::AwaitingHarnessBatch)
        }
        CandidateGenerationConfig::DeterministicTuiTools => {
            publish_deterministic_tui_tools_child_plan(
                campaign_id,
                manifest_path,
                repo_root,
                parent,
                child_budget,
            )
            .map(ParentTargetSelection::ChildPlan)
        }
    }
}

async fn run_legacy_parent_target_selection(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    parent: Parent<Ready>,
) -> Result<ChildPlanReceipt, PrepareError> {
    let parent_identity = parent.identity().clone();
    let root_node = parent.node().clone();
    let prepared_instance =
        parent_identity
            .instance_id()
            .map(str::to_string)
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "parent identity for '{}' is missing instance_id; target selection cannot read node projection files for this value",
                    parent_identity.node_id()
                ),
            })?;
    let campaign = load_existing_prototype1_campaign(campaign_id)?;
    let batch_id = campaign
        .resolved
        .eval
        .batch_prefix
        .clone()
        .unwrap_or_else(|| campaign_id.to_string());
    let batch_manifest = batches_dir()?.join(&batch_id).join("batch.json");
    let input = Prototype1LoopControllerInput {
        stop_after: Prototype1LoopStopAfter::TargetSelection,
        dry_run: true,
        stop_on_error: false,
        protocol_model_id: None,
        protocol_provider: None,
        search_policy: Prototype1SearchPolicy::default(),
        source_campaign: None,
        source_branch_id: Some(parent_identity.branch_id().to_string()),
        source_parent: Some(parent_identity.clone()),
        repo_root: repo_root.to_path_buf(),
        trace_path: prototype1_trace_path(&campaign.manifest_path),
        batch_id,
        batch_manifest,
        prepared_instances: vec![prepared_instance],
        campaign,
    };
    let running_parent = project_node_status(&root_node, Prototype1NodeStatus::Running);
    write_node_projection(&running_parent)?;
    let telemetry = RuntimeTelemetry::parent(&parent_identity, "target_selection");
    telemetry.install_for_chat_requests();
    let report = match run_prototype1_loop_controller(input)
        .instrument(telemetry.span())
        .await
    {
        Ok(report) => report,
        Err(error) => {
            let failed_parent = project_node_status(&root_node, Prototype1NodeStatus::Failed);
            let _ = write_node_projection(&failed_parent);
            return Err(error);
        }
    };
    let expected_generation = parent_identity.generation() + 1;
    let valid_children = report
        .staged_children
        .iter()
        .filter(|child| {
            let node = child.node_record();
            node.generation == expected_generation
                && node.parent_node_id.as_deref() == Some(parent_identity.node_id())
        })
        .count();
    if valid_children == 0 {
        let failed_parent = project_node_status(&root_node, Prototype1NodeStatus::Failed);
        write_node_projection(&failed_parent)?;
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "child plan did not stage any generation {} children for parent '{}'",
                expected_generation,
                parent_identity.node_id()
            ),
        });
    }
    let files = ChildPlanFiles::for_parent(
        manifest_path,
        &parent_identity,
        report.staged_children.clone(),
    );
    let at = files.message_at();
    let open = Open::<ChildPlan>::from_sender(parent, files);
    let (planned, locked) = open
        .lock(at, |at, body| {
            validate_child_plan(&parent_identity, &report, body)?;
            write_child_plan_file(at.path(), body)
        })
        .map_err(|err| {
            let (_parent, source) = err.into_parts();
            source
        })?;
    receive_child_plan(
        campaign_id,
        manifest_path,
        repo_root,
        &parent_identity,
        planned,
        locked,
    )
}

fn publish_broad_harness_child_plan_request(
    manifest_path: &Path,
    repo_root: &Path,
    parent: Parent<Ready>,
    child_budget: Prototype1ChildBudget,
) -> Result<HarnessRequestBatch, PrepareError> {
    let parent_identity = parent.identity().clone();
    let root_node = parent.node().clone();
    let running_parent = project_node_status(&root_node, Prototype1NodeStatus::Running);
    write_node_projection(&running_parent)?;
    let admission_binding = broad_harness_request_admission_binding(&parent, repo_root)?;
    let slot_budget = Prototype1ChildBudget { min: 1, max: 1 };
    let slot_count = (child_budget.max as usize)
        .checked_mul(BROAD_TUI_FRESH_ATTEMPTS_PER_CHILD)
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child budget max {} overflowed fresh attempt allocation",
                child_budget.max
            ),
        })?;
    let mut slots = Vec::with_capacity(slot_count);
    for _ in 0..slot_count {
        let publication = publish_broad_edit_harness_request(
            manifest_path,
            repo_root,
            &parent_identity,
            slot_budget,
            admission_binding.clone(),
        )?;
        slots.push(HarnessRequestSlot {
            request_path: publication.request_path,
            published: publication.published,
        });
    }
    let first_slot = slots
        .first()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "broad harness child budget did not allocate any request slots".to_string(),
        })?;
    let awaiting_parent = parent.awaiting_harness_plan_for_request((&first_slot.published).into());
    debug_assert_eq!(
        awaiting_parent.harness_request().request_id(),
        first_slot.published.request_id()
    );
    debug_assert_eq!(
        awaiting_parent.harness_request().request_hash(),
        first_slot.published.request_hash()
    );
    Ok(HarnessRequestBatch {
        parent: awaiting_parent,
        slots,
        child_budget,
    })
}

fn broad_harness_admission_for_parent<S>(
    parent: &Parent<S>,
    repo_root: &Path,
) -> Result<EditSurfaceAdmission, PrepareError> {
    let artifact_id = if let Some(artifact_id) = parent.node().derived_artifact_id.clone() {
        artifact_id
    } else {
        GitWorktreeBackend
            .artifact_id_for_head(repo_root)
            .map_err(|source| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "could not derive active artifact id for broad-harness parent '{}': {source}",
                    parent.node().node_id
                ),
            })?
    };
    Ok(EditSurfaceAdmission::new(
        crate::loop_graph::Coordinate {
            runtime_id: *parent.runtime_id(),
            target: crate::loop_graph::OperationTarget::Artifact { artifact_id },
        },
        crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId::new(
            "workspace except ploke-eval",
        ),
    ))
}

fn try_admit_request_result(
    repo_root: &Path,
    parent: &Parent<AwaitingHarnessPlan>,
    slot: &HarnessRequestSlot,
    executor: Option<&transaction::Executor>,
) -> Result<Option<AdmittedBroadHarnessResult>, PrepareError> {
    let result_path = slot.published.submitted_result_path();
    let backend = GitWorktreeBackend;
    let admission = broad_harness_admission_for_parent(parent, repo_root)?;
    let mut last_rejection = None;
    for attempt in 1..=BROAD_TUI_ATTEMPT_LIMIT {
        if !result_path.exists() {
            info!(
                target: EXECUTION_DEBUG_TARGET,
                request_id = %slot.published.request_id(),
                request_hash = %slot.published.request_hash(),
                attempt,
                max_attempts = BROAD_TUI_ATTEMPT_LIMIT,
                submitted_result_path = %result_path.display(),
                "broad headless-tui continuation hook found no submitted result yet"
            );
            continue;
        }

        let bytes =
            fs::read(result_path).map_err(|source| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "could not read submitted broad harness result '{}': {source}",
                    result_path.display()
                ),
            })?;
        let submitted =
            serde_json::from_slice::<SubmittedBroadHarnessResult>(&bytes).map_err(|source| {
                PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "could not decode submitted broad harness result '{}': {source}",
                        result_path.display()
                    ),
                }
            })?;

        match backend.admit_submitted_broad_harness_result(
            repo_root,
            admission.clone(),
            &slot.published,
            &submitted,
        ) {
            Ok(admitted) => {
                let admitted = if let Some(executor) = executor {
                    admitted.with_executor(executor.clone())
                } else {
                    admitted
                };
                info!(
                    target: EXECUTION_DEBUG_TARGET,
                    request_id = %admitted.request_id(),
                    request_hash = %admitted.request_hash(),
                    attempt,
                    changed_paths = admitted.changed_paths().len(),
                    workspace = %admitted.workspace_root().display(),
                    "admitted broad harness submitted result at complete-mode continuation hook"
                );
                return Ok(Some(admitted));
            }
            Err(source) => {
                let detail = source.to_string();
                warn!(
                    target: EXECUTION_DEBUG_TARGET,
                    request_id = %slot.published.request_id(),
                    request_hash = %slot.published.request_hash(),
                    attempt,
                    max_attempts = BROAD_TUI_ATTEMPT_LIMIT,
                    error = %detail,
                    "rejected broad harness submitted result at complete-mode continuation hook"
                );
                last_rejection = Some(detail);
            }
        }
    }

    if let Some(detail) = last_rejection {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad-harness-request submitted result failed admission after {BROAD_TUI_ATTEMPT_LIMIT} attempt(s): {detail}"
            ),
        });
    }

    Ok(None)
}

async fn run_broad_headless_tui_attempt(
    slot: &HarnessRequestSlot,
) -> Result<Option<transaction::Executor>, PrepareError> {
    let options = BroadTuiAttemptOptions::for_parent_patcher_defaults(None, None)?;
    run_broad_headless_tui_attempt_with_options(slot, &options).await
}

fn effective_broad_tui_max_attempts(
    contract: &crate::cli::prototype1_state::edit_surface::harness_request::contract::Bundle,
    options: &BroadTuiAttemptOptions,
) -> u32 {
    options.max_attempts.unwrap_or_else(|| {
        contract
            .attempt
            .max_attempts
            .max(BROAD_TUI_ATTEMPT_LIMIT as u32)
    })
}

fn effective_broad_tui_timeout_secs(
    contract: &crate::cli::prototype1_state::edit_surface::harness_request::contract::Bundle,
    options: &BroadTuiAttemptOptions,
) -> u64 {
    options.timeout_secs.unwrap_or_else(|| {
        contract
            .attempt
            .timeout
            .turn_seconds
            .max(Duration::from_secs(60).as_secs())
    })
}

async fn run_broad_headless_tui_attempt_with_options(
    slot: &HarnessRequestSlot,
    options: &BroadTuiAttemptOptions,
) -> Result<Option<transaction::Executor>, PrepareError> {
    let backend = GitWorktreeBackend;
    let repo_root = slot.published.request().workspace.source_repository_path();
    backend
        .prepare_broad_harness_workspace(repo_root, &slot.published)
        .map_err(|source| PrepareError::InvalidBatchSelection {
            detail: format!("failed to prepare broad headless-tui workspace: {source}"),
        })?;

    let prompt = fs::read_to_string(slot.published.prompt_path()).map_err(|source| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "could not read broad harness prompt '{}': {source}",
                slot.published.prompt_path().display()
            ),
        }
    })?;
    let contract = &slot.published.request().contract;
    let max_attempts = effective_broad_tui_max_attempts(contract, options);
    let timeout_secs = effective_broad_tui_timeout_secs(contract, options);
    let budget = tui_adapter::Budget::new(max_attempts, timeout_secs).map_err(|source| {
        PrepareError::InvalidBatchSelection {
            detail: format!("invalid broad headless-tui attempt budget: {source}"),
        }
    })?;
    let use_stash_transfer = stash_transfer_enabled();
    if use_stash_transfer {
        match backend
            .validate_tui_attempt(repo_root, &slot.published)
            .map_err(|source| PrepareError::InvalidBatchSelection {
                detail: format!("failed to preflight stash-transfer workspace: {source}"),
            })? {
            TuiAttemptOutcome::Rejected(AttemptRejection::NoChange { .. }) => {}
            outcome => {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "stash-transfer broad headless-tui preflight expected clean source and unchanged candidate, got {outcome:?}"
                    ),
                });
            }
        }
    }
    let tui_workspace = if use_stash_transfer {
        repo_root
    } else {
        slot.published.workspace_path()
    };

    let run = tui_adapter::run_headless_with_model(
        tui_workspace,
        &prompt,
        budget,
        slot.published.request().edit_policy,
        &slot.published.request().evidence_roots,
        options.model().cloned(),
    )
    .await
    .map_err(|source| PrepareError::InvalidBatchSelection {
        detail: format!("broad headless-tui attempt failed: {source}"),
    })?;

    let terminal = run
        .terminal()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: "broad headless-tui attempt ended without a terminal outcome".to_string(),
        })?;
    write_broad_headless_tui_diagnostics(slot, &run)?;
    finish_broad_headless_tui_attempt(
        &backend,
        slot,
        repo_root,
        use_stash_transfer,
        &run,
        terminal,
    )
}

fn finish_broad_headless_tui_attempt(
    backend: &GitWorktreeBackend,
    slot: &HarnessRequestSlot,
    repo_root: &Path,
    use_stash_transfer: bool,
    run: &tui_adapter::HeadlessRun,
    terminal: &tui_adapter::HeadlessTerminal,
) -> Result<Option<transaction::Executor>, PrepareError> {
    match terminal {
        tui_adapter::HeadlessTerminal::Applied {
            proposal_id,
            applied_proposal_ids: _,
            request_id,
            changed_paths,
        } => {
            let changed_paths = candidate_paths_for_tui(
                backend,
                slot,
                repo_root,
                changed_paths,
                use_stash_transfer,
            )?;
            write_headless_tui_submission(slot, &changed_paths)?;
            Ok(Some(transaction::Executor::new(
                Some(request_id.to_string()),
                Some(proposal_id.to_string()),
                None,
            )))
        }
        tui_adapter::HeadlessTerminal::Exhausted { attempts, last } => {
            Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "headless ploke-tui exhausted {attempts} attempt(s) without an admissible edit: {last}"
                ),
            })
        }
        tui_adapter::HeadlessTerminal::CompletedWithoutEdit { outcome, summary } => {
            Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "headless ploke-tui completed without staging an edit: outcome={outcome}; {summary}"
                ),
            })
        }
        tui_adapter::HeadlessTerminal::ToolFailed { error } => {
            Err(PrepareError::InvalidBatchSelection {
                detail: format!("headless ploke-tui tool failed: {error}"),
            })
        }
        tui_adapter::HeadlessTerminal::NoEdit => Err(PrepareError::InvalidBatchSelection {
            detail: "headless ploke-tui produced no edit".to_string(),
        }),
        tui_adapter::HeadlessTerminal::ContextUnavailable { reason } => {
            Err(PrepareError::InvalidBatchSelection {
                detail: format!("headless ploke-tui prompt context unavailable: {reason}"),
            })
        }
        tui_adapter::HeadlessTerminal::ProviderUnavailable { reason } => {
            Err(PrepareError::InvalidBatchSelection {
                detail: format!("headless ploke-tui provider unavailable: {reason}"),
            })
        }
        tui_adapter::HeadlessTerminal::TimedOut { secs } => match run.applied_edit() {
            Some(applied) => {
                let changed_paths = candidate_paths_for_tui(
                    backend,
                    slot,
                    repo_root,
                    applied.changed_paths(),
                    use_stash_transfer,
                )?;
                write_headless_tui_submission(slot, &changed_paths)?;
                Ok(Some(transaction::Executor::new(
                    None,
                    Some(applied.proposal_id().to_string()),
                    None,
                )))
            }
            None => Err(PrepareError::InvalidBatchSelection {
                detail: format!("headless ploke-tui timed out after {secs} seconds"),
            }),
        },
    }
}

fn candidate_paths_for_tui(
    backend: &GitWorktreeBackend,
    slot: &HarnessRequestSlot,
    repo_root: &Path,
    changed_paths: &[PathBuf],
    use_stash_transfer: bool,
) -> Result<Vec<PathBuf>, PrepareError> {
    if use_stash_transfer {
        let relpaths = backend
            .stash_to_workspace(
                repo_root,
                slot.published.workspace_path(),
                changed_paths,
                &format!(
                    "prototype1 broad harness request {}",
                    slot.published.request_id()
                ),
            )
            .map_err(|source| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "failed to transfer headless TUI stash to candidate workspace: {source}"
                ),
            })?;
        Ok(relpaths
            .into_iter()
            .map(|path| slot.published.workspace_path().join(path))
            .collect())
    } else {
        Ok(changed_paths.to_vec())
    }
}

fn write_headless_tui_submission(
    slot: &HarnessRequestSlot,
    changed_paths: &[PathBuf],
) -> Result<(), PrepareError> {
    let changed_files = changed_paths
        .iter()
        .map(|path| SubmittedFileChange {
            workspace_relpath: path
                .strip_prefix(slot.published.workspace_path())
                .map(Path::to_path_buf)
                .unwrap_or_else(|_| path.clone()),
            summary: "Changed by headless ploke-tui adapter attempt.".to_string(),
        })
        .collect::<Vec<_>>();
    let return_evidence = SubmittedHarnessReturnEvidence {
        authority_boundary: slot
            .published
            .request()
            .return_evidence
            .authority_boundary
            .clone(),
        change_summary: SubmittedChangeSummary { changed_files },
        guiding_evidence: slot
            .published
            .request()
            .evidence_roots
            .iter()
            .take(6)
            .map(|root| SubmittedEvidenceCitation {
                kind: root.kind,
                location: root.location.clone(),
                summary: "Available to the headless TUI attempt as request evidence.".to_string(),
            })
            .collect(),
        rationale: SubmittedImprovementRationale {
            hypothesis:
                "Headless ploke-tui produced a bounded self-edit for descendant evaluation."
                    .to_string(),
            expected_descendant_effect:
                "ploke-eval will compile and evaluate the admitted child artifact.".to_string(),
        },
        checks: slot
            .published
            .request()
            .contract
            .validation
            .commands
            .iter()
            .map(|command| SubmittedCheckRecommendation {
                label: command.label.clone(),
                command: std::iter::once(command.program.as_str())
                    .chain(command.args.iter().map(String::as_str))
                    .collect::<Vec<_>>()
                    .join(" "),
                success_signal: command.success.clone(),
            })
            .collect(),
    };
    let submitted =
        SubmittedBroadHarnessResult::bind(&slot.published, return_evidence).map_err(|source| {
            PrepareError::InvalidBatchSelection {
                detail: format!("failed to bind headless TUI submitted result: {source:?}"),
            }
        })?;
    if let Some(parent) = slot.published.submitted_result_path().parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    write_json_file_pretty(slot.published.submitted_result_path(), &submitted)
}

pub(crate) async fn run_broad_harness_attempt_from_request_path(
    request_path: PathBuf,
    options: BroadTuiAttemptOptions,
) -> Result<BroadHarnessAttemptProjection, PrepareError> {
    let request_bytes =
        fs::read(&request_path).map_err(|source| PrepareError::InvalidBatchSelection {
            detail: format!(
                "could not read published broad harness request '{}': {source}",
                request_path.display()
            ),
        })?;
    let published = serde_json::from_slice::<
        crate::cli::prototype1_state::edit_surface::harness_request::PublishedBroadHarnessRequest,
    >(&request_bytes)
    .map_err(|source| PrepareError::InvalidBatchSelection {
        detail: format!(
            "could not decode published broad harness request '{}': {source}",
            request_path.display()
        ),
    })?;
    let slot = HarnessRequestSlot {
        request_path: request_path.clone(),
        published,
    };
    run_broad_harness_attempt_slot(&slot, &options).await
}

async fn run_broad_harness_attempt_slot(
    slot: &HarnessRequestSlot,
    options: &BroadTuiAttemptOptions,
) -> Result<BroadHarnessAttemptProjection, PrepareError> {
    let contract = &slot.published.request().contract;
    let max_attempts = effective_broad_tui_max_attempts(contract, options);
    let timeout_secs = effective_broad_tui_timeout_secs(contract, options);
    let started = Instant::now();
    let executor = run_broad_headless_tui_attempt_with_options(slot, options).await?;
    let elapsed_ms = started.elapsed().as_millis();
    let outcome = GitWorktreeBackend
        .validate_tui_attempt(
            slot.published.request().workspace.source_repository_path(),
            &slot.published,
        )
        .map_err(|source| PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad headless-TUI workspace validation errored for '{}': {source}",
                slot.published.workspace_path().display()
            ),
        })?;
    let changed_paths = match outcome {
        TuiAttemptOutcome::Accepted(diff) => diff.changed_paths().to_vec(),
        TuiAttemptOutcome::Rejected(rejection) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "broad headless-TUI workspace rejected after executor {:?}: {:?}",
                    executor, rejection
                ),
            });
        }
    };
    Ok(BroadHarnessAttemptProjection {
        request_path: slot.request_path.clone(),
        request_id: slot.published.request_id().to_string(),
        workspace_path: slot.published.workspace_path().to_path_buf(),
        submitted_result_path: slot.published.submitted_result_path().to_path_buf(),
        diagnostics_path: broad_headless_tui_diagnostics_path(
            slot.published.submitted_result_path(),
        ),
        model_id: options.model_label(),
        provider: options.provider_label(),
        max_attempts,
        timeout_secs,
        elapsed_ms,
        status: "applied".to_string(),
        executor_run_id: executor
            .as_ref()
            .and_then(|executor| executor.run_id().map(std::string::ToString::to_string)),
        executor_attempt_id: executor
            .as_ref()
            .and_then(|executor| executor.attempt_id().map(std::string::ToString::to_string)),
        changed_paths,
        error: None,
    })
}

pub(crate) async fn run_broad_harness_attempt_sweep(
    lanes: Vec<(PathBuf, BroadTuiAttemptOptions)>,
    parallel: usize,
) -> Result<Vec<BroadHarnessAttemptProjection>, PrepareError> {
    if parallel == 0 {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "broad harness sweep --parallel must be greater than zero".to_string(),
        });
    }

    let mut pending = lanes.into_iter();
    let mut running = tokio::task::JoinSet::new();
    let mut finished = Vec::new();

    loop {
        while running.len() < parallel {
            let Some((request_path, options)) = pending.next() else {
                break;
            };
            running.spawn(async move {
                let model_id = options.model_label();
                let provider = options.provider_label();
                let max_attempts = options.max_attempts.unwrap_or(0);
                let timeout_secs = options.timeout_secs.unwrap_or(0);
                let started = Instant::now();
                match run_broad_harness_attempt_from_request_path(request_path.clone(), options)
                    .await
                {
                    Ok(row) => row,
                    Err(source) => BroadHarnessAttemptProjection {
                        request_id: request_path
                            .file_stem()
                            .map(|stem| stem.to_string_lossy().to_string())
                            .unwrap_or_else(|| "<unknown>".to_string()),
                        workspace_path: PathBuf::new(),
                        submitted_result_path: PathBuf::new(),
                        diagnostics_path: PathBuf::new(),
                        request_path,
                        model_id,
                        provider,
                        max_attempts,
                        timeout_secs,
                        elapsed_ms: started.elapsed().as_millis(),
                        status: "failed".to_string(),
                        executor_run_id: None,
                        executor_attempt_id: None,
                        changed_paths: Vec::new(),
                        error: Some(source.to_string()),
                    },
                }
            });
        }

        if running.is_empty() {
            break;
        }

        match running.join_next().await {
            Some(Ok(row)) => finished.push(row),
            Some(Err(source)) => {
                finished.push(BroadHarnessAttemptProjection {
                    request_path: PathBuf::new(),
                    request_id: "<join-error>".to_string(),
                    workspace_path: PathBuf::new(),
                    submitted_result_path: PathBuf::new(),
                    diagnostics_path: PathBuf::new(),
                    model_id: None,
                    provider: None,
                    max_attempts: 0,
                    timeout_secs: 0,
                    elapsed_ms: 0,
                    status: "failed".to_string(),
                    executor_run_id: None,
                    executor_attempt_id: None,
                    changed_paths: Vec::new(),
                    error: Some(source.to_string()),
                });
            }
            None => break,
        }
    }

    Ok(finished)
}

fn stash_transfer_enabled() -> bool {
    std::env::var_os(BROAD_TUI_STASH_TRANSFER_ENV)
        .and_then(|value| value.into_string().ok())
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !matches!(value.as_str(), "" | "0" | "false" | "off" | "no")
        })
        .unwrap_or(false)
}

fn write_broad_headless_tui_diagnostics(
    slot: &HarnessRequestSlot,
    run: &tui_adapter::HeadlessRun,
) -> Result<(), PrepareError> {
    let path = broad_headless_tui_diagnostics_path(slot.published.submitted_result_path());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    write_json_file_pretty(&path, &run.evidence())
}

fn broad_headless_tui_diagnostics_path(submitted_result_path: &Path) -> PathBuf {
    submitted_result_path.with_extension("headless-tui.json")
}

fn broad_harness_child_from_admitted(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    parent_identity: &ParentIdentity,
    parent_runtime_id: crate::loop_graph::RuntimeId,
    admitted: &AdmittedBroadHarnessResult,
    slot_index: usize,
) -> Result<ChildFiles, PrepareError> {
    if admitted.changed_paths().is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "broad harness admitted transaction had no changed paths".to_string(),
        });
    }
    // Legacy child-plan payloads still carry a file-level treatment branch. For
    // broad transactions this is only a projection anchor; C1 materialization
    // consumes the request-bound harness evidence below.
    let target_relpath = admitted.changed_paths()[0].clone();
    let source_content = fs::read_to_string(repo_root.join(&target_relpath)).map_err(|source| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "could not read source file '{}' for broad child plan: {source}",
                repo_root.join(&target_relpath).display()
            ),
        }
    })?;
    let proposed_content = fs::read_to_string(admitted.workspace_root().join(&target_relpath))
        .map_err(|source| PrepareError::InvalidBatchSelection {
            detail: format!(
                "could not read candidate file '{}' for broad child plan: {source}",
                admitted.workspace_root().join(&target_relpath).display()
            ),
        })?;
    let expected_generation = parent_identity.generation() + 1;
    let candidate_id = format!(
        "broad-harness-g{}-{:02}",
        expected_generation,
        slot_index + 1
    );
    let branch_id =
        treatment_branch_id(&parent_identity.branch_id(), &target_relpath, &candidate_id);
    let source_content_hash = format!("{:x}", Sha256::digest(source_content.as_bytes()));
    let proposed_content_hash = format!("{:x}", Sha256::digest(proposed_content.as_bytes()));
    let resolved = crate::intervention::ResolvedTreatmentBranch {
        instance_id: parent_runtime_id.to_string(),
        source_state_id: parent_identity.branch_id().to_string(),
        parent_branch_id: Some(parent_identity.branch_id().to_string()),
        target_relpath: target_relpath.clone(),
        source_content,
        source_content_hash,
        selected_branch_id: Some(branch_id.clone()),
        branch: TreatmentBranchNode {
            branch_id: branch_id.clone(),
            candidate_id,
            patch_id: Some(crate::loop_graph::PatchId::new(format!(
                "broad-harness:{}",
                admitted.request_id()
            ))),
            branch_label: format!("broad harness edit {}", admitted.request_id()),
            synthesized_spec_id: "prototype1:broad-headless-tui-adapter-v1".to_string(),
            proposed_content,
            proposed_content_hash,
            generation_target: Some(crate::loop_graph::OperationTarget::Artifact {
                artifact_id: admitted.base_artifact_id().clone(),
            }),
            generation_coordinate: Some(admitted.coordinate().clone()),
            status: TreatmentBranchStatus::Selected,
            apply_id: Some(admitted.request_id().to_string()),
            applied_content_hash: None,
            derived_artifact_id: Some(admitted.derived_artifact_id().clone()),
        },
    };
    let (node, _request) = write_treatment_evaluation_projection(
        campaign_id,
        manifest_path,
        &resolved,
        expected_generation,
        Some(parent_identity.node_id()),
        repo_root,
        false,
    )?;
    let evidence = admitted.child_evidence();
    Ok(
        ChildFiles::from_resolved(campaign_id, node, resolved, false)
            .with_harness_evidence(evidence),
    )
}

fn publish_broad_harness_child_plan_from_admitted_batch(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    batch: HarnessRequestBatch,
    admitted: Vec<AdmittedBroadHarnessResult>,
) -> Result<ChildPlanReceipt, PrepareError> {
    if admitted.len() < batch.child_budget.min as usize {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness admitted {} child transaction(s), fewer than required minimum {}",
                admitted.len(),
                batch.child_budget.min
            ),
        });
    }
    let parent_identity = batch.parent.identity().clone();
    let parent_runtime_id = *batch.parent.runtime_id();
    let children = admitted
        .iter()
        .enumerate()
        .map(|(index, admitted)| {
            broad_harness_child_from_admitted(
                campaign_id,
                manifest_path,
                repo_root,
                &parent_identity,
                parent_runtime_id,
                admitted,
                index,
            )
        })
        .collect::<Result<Vec<_>, PrepareError>>()?;
    let files = ChildPlanFiles::for_parent(manifest_path, &parent_identity, children);
    let at = files.message_at();
    let ready_parent = batch.parent.accept_harness_plan();
    let open = Open::<ChildPlan>::from_sender(ready_parent, files);
    let (planned, locked) = open
        .lock(at, |at, body| {
            validate_and_write_broad_harness_child_plan(&parent_identity, at.path(), body)
        })
        .map_err(|err| {
            let (_parent, source) = err.into_parts();
            source
        })?;
    receive_child_plan(
        campaign_id,
        manifest_path,
        repo_root,
        &parent_identity,
        planned,
        locked,
    )
}

fn publish_broad_harness_child_plan_from_admitted(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    receipt: HarnessRequestReceipt,
    admitted: AdmittedBroadHarnessResult,
) -> Result<ChildPlanReceipt, PrepareError> {
    publish_broad_harness_child_plan_from_admitted_batch(
        campaign_id,
        manifest_path,
        repo_root,
        HarnessRequestBatch {
            parent: receipt.parent,
            slots: vec![HarnessRequestSlot {
                request_path: receipt.request_path,
                published: receipt.published,
            }],
            child_budget: Prototype1ChildBudget { min: 1, max: 1 },
        },
        vec![admitted],
    )
}

const TUI_EDIT_SURFACE_PRODUCER_ID: &str = "prototype1:tui-edit-surface:deterministic-v1";
const TUI_EDIT_SURFACE_POLICY_ID: &str = "surface-policy:tool-surface-v1";

fn publish_deterministic_tui_tools_child_plan(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    parent: Parent<Ready>,
    child_budget: Prototype1ChildBudget,
) -> Result<ChildPlanReceipt, PrepareError> {
    let edit_surface = Prototype1EditSurface::PlokeTuiTools;
    let parent_identity = parent.identity().clone();
    let root_node = parent.node().clone();
    let running_parent = project_node_status(&root_node, Prototype1NodeStatus::Running);
    write_node_projection(&running_parent)?;

    let generated = produce_deterministic_tui_tools_candidates(repo_root, &parent, child_budget)?;
    let expected_generation = parent_identity.generation() + 1;
    let mut children = Vec::with_capacity(generated.checked.len());

    for (index, checked) in generated.checked.iter().enumerate() {
        let candidate_id = format!("tui-edit-surface-g{}-{:02}", expected_generation, index + 1);
        let branch_id = treatment_branch_id(
            &parent_identity.branch_id(),
            checked.target_relpath(),
            &candidate_id,
        );
        let provisional_node = Prototype1NodeRecord {
            schema_version: PROTOTYPE1_TREATMENT_NODE_SCHEMA_VERSION.to_string(),
            node_id: prototype1_node_id(&branch_id, expected_generation),
            parent_node_id: Some(parent_identity.node_id().to_string()),
            generation: expected_generation,
            instance_id: parent.runtime_id().to_string(),
            source_state_id: parent_identity.branch_id().to_string(),
            operation_target: None,
            base_artifact_id: None,
            patch_id: None,
            derived_artifact_id: None,
            parent_branch_id: Some(parent_identity.branch_id().to_string()),
            branch_id,
            candidate_id,
            target_relpath: checked.target_relpath().to_path_buf(),
            node_dir: PathBuf::new(),
            workspace_root: PathBuf::new(),
            binary_path: PathBuf::new(),
            runner_request_path: PathBuf::new(),
            runner_result_path: PathBuf::new(),
            status: Prototype1NodeStatus::Planned,
            created_at: String::new(),
            updated_at: String::new(),
        };
        let resolved = resolved_from_checked_edit(&provisional_node, checked);
        let (node, _request) = write_treatment_evaluation_projection(
            campaign_id,
            manifest_path,
            &resolved,
            expected_generation,
            Some(parent_identity.node_id()),
            repo_root,
            false,
        )?;
        children.push(
            child_files_from_checked_edit(campaign_id, edit_surface, node, checked, false)
                .map_err(CandidateGenerationError::into_prepare)?,
        );
    }

    if children.len() < child_budget.min as usize {
        persist_rejected_surface_attempt_child_plan(
            manifest_path,
            parent,
            generated.rejected_attempts.clone(),
        )?;
        let failed_parent = project_node_status(&root_node, Prototype1NodeStatus::Failed);
        write_node_projection(&failed_parent)?;
        return Err(CandidateGenerationError::InsufficientUniqueProposals {
            surface: edit_surface,
            min: child_budget.min as usize,
            produced: children.len(),
        }
        .into_prepare());
    }

    let files = ChildPlanFiles::for_parent(manifest_path, &parent_identity, children)
        .with_rejected_surface_attempts(generated.rejected_attempts.clone());
    let at = files.message_at();
    let open = Open::<ChildPlan>::from_sender(parent, files);
    let (planned, locked) = open
        .lock(at, |at, body| {
            validate_and_write_tui_child_plan(&parent_identity, at.path(), body)
        })
        .map_err(|err| {
            let (_parent, source) = err.into_parts();
            source
        })?;
    let receipt = receive_child_plan(
        campaign_id,
        manifest_path,
        repo_root,
        &parent_identity,
        planned,
        locked,
    )?;
    Ok(receipt)
}

struct BroadHarnessRequestPublication {
    request_path: PathBuf,
    published:
        crate::cli::prototype1_state::edit_surface::harness_request::PublishedBroadHarnessRequest,
}

fn publish_broad_edit_harness_request(
    manifest_path: &Path,
    repo_root: &Path,
    parent: &ParentIdentity,
    child_budget: Prototype1ChildBudget,
    admission_binding: crate::cli::prototype1_state::edit_surface::harness_request::RequestAdmissionBinding,
) -> Result<BroadHarnessRequestPublication, PrepareError> {
    let prototype_root = prototype1_campaign_root(manifest_path);
    let request_dir = prototype_root.join("messages/edit-harness-request");
    let request_path = request_dir.join(format!("{}.json", parent.node_id()));
    let prompt_path = request_dir.join(format!("{}.md", parent.node_id()));
    let submitted_result_path = prototype_root
        .join("messages/edit-harness-result")
        .join(format!("{}.json", parent.node_id()));
    let published =
        crate::cli::prototype1_state::edit_surface::harness_request::PublishedBroadHarnessRequest::prototype1_workspace(
            parent.node_id().to_string(),
            repo_root.to_path_buf(),
            crate::cli::prototype1_state::edit_surface::harness_request::HarnessChildBudget {
                min_children: child_budget.min,
                max_children: child_budget.max,
            },
            &prototype_root,
            request_path,
            prompt_path.clone(),
            submitted_result_path,
            admission_binding,
        );
    let request_path = published.request_path().to_path_buf();
    if let Some(parent) = request_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    write_json_file_pretty(&request_path, &published)?;
    fs::write(published.prompt_path(), published.request().render_prompt()).map_err(|source| {
        PrepareError::WriteManifest {
            path: published.prompt_path().to_path_buf(),
            source,
        }
    })?;
    Ok(BroadHarnessRequestPublication {
        request_path,
        published,
    })
}

fn broad_harness_request_admission_binding(
    parent: &Parent<Ready>,
    repo_root: &Path,
) -> Result<
    crate::cli::prototype1_state::edit_surface::harness_request::RequestAdmissionBinding,
    PrepareError,
> {
    let admission = broad_harness_admission_for_parent(parent, repo_root)?;
    crate::cli::prototype1_state::edit_surface::harness_request::RequestAdmissionBinding::from_admission(
        &admission,
    )
    .map_err(|source| PrepareError::InvalidBatchSelection {
        detail: format!(
            "could not derive broad-harness request admission binding for parent '{}': {}",
            parent.identity().node_id(),
            format!("{source:?}")
        ),
    })
}

fn persist_rejected_surface_attempt_child_plan(
    manifest_path: &Path,
    parent: Parent<Ready>,
    rejected_surface_attempts: Vec<surface_attempt::Evidence>,
) -> Result<(), PrepareError> {
    let parent_identity = parent.identity().clone();
    let files = ChildPlanFiles::for_parent(manifest_path, &parent_identity, Vec::new())
        .with_rejected_surface_attempts(rejected_surface_attempts);
    let at = files.message_at();
    let open = Open::<ChildPlan>::from_sender(parent, files);
    let _ = open
        .lock(at, |at, body| {
            validate_and_write_tui_child_plan(&parent_identity, at.path(), body)
        })
        .map_err(|err| {
            let (_parent, source) = err.into_parts();
            source
        })?;
    Ok(())
}

fn validate_and_write_tui_child_plan(
    parent: &ParentIdentity,
    path: &Path,
    body: &ChildPlanFiles,
) -> Result<(), PrepareError> {
    let expected_generation = parent.generation() + 1;
    if body.parent_node_id() != parent.node_id() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "tui edit-surface child plan recipient '{}' did not match parent '{}'",
                body.parent_node_id(),
                parent.node_id()
            ),
        });
    }
    if body.child_generation() != expected_generation {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "tui edit-surface child plan generation {} did not match expected generation {}",
                body.child_generation(),
                expected_generation
            ),
        });
    }
    if body.children().is_empty() && body.rejected_surface_attempts().is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "tui edit-surface child plan cannot be empty without rejected attempt evidence"
                .to_string(),
        });
    }
    for child in body.children() {
        let node = child.node_record();
        if node.generation != expected_generation
            || node.parent_node_id.as_deref() != Some(parent.node_id())
        {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "tui edit-surface child '{}' is not a direct child of parent '{}'",
                    node.node_id,
                    parent.node_id()
                ),
            });
        }
        if child.resolved().branch.synthesized_spec_id != TUI_EDIT_SURFACE_PRODUCER_ID {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "tui edit-surface child '{}' was not produced by the deterministic producer",
                    node.node_id
                ),
            });
        }
        validate_deterministic_surface_evidence(child)?;
    }

    write_child_plan_file(path, body)
}

fn validate_and_write_broad_harness_child_plan(
    parent: &ParentIdentity,
    path: &Path,
    body: &ChildPlanFiles,
) -> Result<(), PrepareError> {
    let expected_generation = parent.generation() + 1;
    if body.parent_node_id() != parent.node_id() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child plan recipient '{}' did not match parent '{}'",
                body.parent_node_id(),
                parent.node_id()
            ),
        });
    }
    if body.child_generation() != expected_generation {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child plan generation {} did not match expected generation {}",
                body.child_generation(),
                expected_generation
            ),
        });
    }
    if body.children().is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "broad harness child plan must contain admitted children".to_string(),
        });
    }
    for child in body.children() {
        validate_requested_broad_harness_child(child)?;
    }
    write_child_plan_file(path, body)
}

fn validate_requested_broad_harness_child(child: &ChildFiles) -> Result<(), PrepareError> {
    let node = child.node_record();
    if child.resolved().branch.synthesized_spec_id != "prototype1:broad-headless-tui-adapter-v1" {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' was not produced by the headless TUI adapter",
                node.node_id
            ),
        });
    }
    if node.derived_artifact_id.is_none() || node.base_artifact_id.is_none() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' is missing base or derived artifact identity",
                node.node_id
            ),
        });
    }
    if child.resolved().target_relpath != node.target_relpath {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' target '{}' did not match resolved target '{}'",
                node.node_id,
                node.target_relpath.display(),
                child.resolved().target_relpath.display()
            ),
        });
    }
    let Some(evidence) = child.harness_evidence() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' is missing request-bound admission evidence",
                node.node_id
            ),
        });
    };
    if evidence.request().request_id() != child.resolved().branch.apply_id.as_deref().unwrap_or("")
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' request evidence did not match branch apply_id",
                node.node_id
            ),
        });
    }
    if Some(evidence.admission_binding().coordinate())
        != child.resolved().branch.generation_coordinate.as_ref()
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' admission coordinate did not match generation coordinate",
                node.node_id
            ),
        });
    }
    if !evidence.changed_paths().contains(&node.target_relpath) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' request evidence does not include target '{}'",
                node.node_id,
                node.target_relpath.display()
            ),
        });
    }
    let Some(workspace) = evidence.workspace() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' is missing admitted workspace evidence",
                node.node_id
            ),
        });
    };
    if workspace.candidate_root.as_os_str().is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' has empty admitted candidate workspace",
                node.node_id
            ),
        });
    }
    let Some(artifact) = evidence.artifact() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' is missing admitted artifact evidence",
                node.node_id
            ),
        });
    };
    if Some(&artifact.base_artifact_id) != node.base_artifact_id.as_ref() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' base artifact evidence did not match node base artifact",
                node.node_id
            ),
        });
    }
    if Some(&artifact.derived_artifact_id) != node.derived_artifact_id.as_ref() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' derived artifact evidence did not match node derived artifact",
                node.node_id
            ),
        });
    }
    if child.resolved().branch.derived_artifact_id.as_ref() != Some(&artifact.derived_artifact_id) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' branch derived artifact did not match admitted artifact evidence",
                node.node_id
            ),
        });
    }
    if evidence.submitted_result_path().as_os_str().is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "broad harness child '{}' has empty submitted-result evidence path",
                node.node_id
            ),
        });
    }
    Ok(())
}

fn validate_deterministic_surface_evidence(child: &ChildFiles) -> Result<(), PrepareError> {
    if child.resolved().branch.synthesized_spec_id != TUI_EDIT_SURFACE_PRODUCER_ID {
        return Ok(());
    }
    let node = child.node_record();
    let Some(surface) = child.surface() else {
        return Err(CandidateGenerationError::MissingDeterministicEvidence {
            node_id: node.node_id.clone(),
        }
        .into_prepare());
    };
    if surface.producer_id != TUI_EDIT_SURFACE_PRODUCER_ID {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried producer evidence '{}'",
                node.node_id, surface.producer_id
            ),
        });
    }
    if !matches!(
        surface.proposal_producer,
        crate::cli::prototype1_state::edit_surface::request_policy::ProposalProducer::NonRouter
    ) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried Router-backed proposal provenance",
                node.node_id
            ),
        });
    }
    validate_surface_evidence_binding(node, child.resolved(), surface)
}

fn validate_requested_tui_surface_child(child: &ChildFiles) -> Result<(), PrepareError> {
    if child.resolved().branch.synthesized_spec_id != TUI_EDIT_SURFACE_PRODUCER_ID {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "candidate-generator=tui-edit-surface cannot use child '{}' produced by '{}'",
                child.node_id(),
                child.resolved().branch.synthesized_spec_id
            ),
        });
    }
    validate_deterministic_surface_evidence(child)
}

fn validate_surface_evidence_binding(
    node: &Prototype1NodeRecord,
    resolved: &crate::intervention::ResolvedTreatmentBranch,
    surface: &SurfaceEvidence,
) -> Result<(), PrepareError> {
    surface
        .verify_integrity()
        .map_err(|source| PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried invalid surface evidence: {}",
                node.node_id, source
            ),
        })?;
    if surface.target_relpath != node.target_relpath
        || surface.target_relpath != resolved.target_relpath
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried evidence for target '{}', node target '{}', resolved target '{}'",
                node.node_id,
                surface.target_relpath.display(),
                node.target_relpath.display(),
                resolved.target_relpath.display()
            ),
        });
    }
    let expected_policy = TUI_EDIT_SURFACE_POLICY_ID;
    if surface.policy != expected_policy {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried policy '{}', expected '{}'",
                node.node_id, surface.policy, expected_policy
            ),
        });
    }
    if surface.source_content_hash != resolved.source_content_hash
        || surface.source_content_hash
            != format!("{:x}", Sha256::digest(resolved.source_content.as_bytes()))
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried source hash '{}', resolved source hash '{}'",
                node.node_id, surface.source_content_hash, resolved.source_content_hash
            ),
        });
    }
    if surface.proposed_content_hash != resolved.branch.proposed_content_hash
        || surface.proposed_content_hash
            != format!(
                "{:x}",
                Sha256::digest(resolved.branch.proposed_content.as_bytes())
            )
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried proposed hash '{}', resolved proposed hash '{}'",
                node.node_id, surface.proposed_content_hash, resolved.branch.proposed_content_hash
            ),
        });
    }
    let expected_generator_surface = GitWorktreeBackend
        .generator_surface_for_surface_touches(
            &surface.target_relpath,
            &resolved.source_content,
            &surface.touches,
        )
        .map_err(|source| PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' could not reconstruct generator_surface provenance: {}",
                node.node_id, source
            ),
        })?;
    let Some(actual_generator_surface) = surface.generator_surface.as_ref() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried no generator_surface provenance",
                node.node_id
            ),
        });
    };
    if actual_generator_surface != &expected_generator_surface {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried generator_surface {:?}, expected {:?}",
                node.node_id, actual_generator_surface, expected_generator_surface
            ),
        });
    }
    require_artifact_id_binding(
        &node.node_id,
        "base_artifact_id",
        node.base_artifact_id.as_ref(),
        &surface.base.artifact_id,
    )?;
    require_artifact_id_binding(
        &node.node_id,
        "derived_artifact_id",
        node.derived_artifact_id.as_ref(),
        &surface.after.artifact_id,
    )?;
    require_artifact_id_binding(
        &node.node_id,
        "branch.derived_artifact_id",
        resolved.branch.derived_artifact_id.as_ref(),
        &surface.after.artifact_id,
    )?;
    require_patch_id_binding(
        &node.node_id,
        "patch_id",
        node.patch_id.as_ref(),
        &surface.patch_id,
    )?;
    require_patch_id_binding(
        &node.node_id,
        "branch.patch_id",
        resolved.branch.patch_id.as_ref(),
        &surface.patch_id,
    )?;
    require_operation_target_binding(
        &node.node_id,
        "operation_target",
        node.operation_target.as_ref(),
        &surface.base.artifact_id,
    )?;
    require_operation_target_binding(
        &node.node_id,
        "branch.generation_target",
        resolved.branch.generation_target.as_ref(),
        &surface.base.artifact_id,
    )?;
    if resolved.branch.apply_id.as_deref() != Some(surface.proposal_id.as_str()) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried proposal '{}', branch apply_id {:?}",
                node.node_id, surface.proposal_id, resolved.branch.apply_id
            ),
        });
    }
    Ok(())
}

fn require_artifact_id_binding(
    node_id: &str,
    field: &str,
    actual: Option<&crate::loop_graph::ArtifactId>,
    expected: &crate::loop_graph::ArtifactId,
) -> Result<(), PrepareError> {
    if actual == Some(expected) {
        return Ok(());
    }
    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "deterministic tui edit-surface child '{}' carried surface Artifact {}, but {} was {:?}",
            node_id, expected, field, actual
        ),
    })
}

fn require_patch_id_binding(
    node_id: &str,
    field: &str,
    actual: Option<&crate::loop_graph::PatchId>,
    expected: &crate::loop_graph::PatchId,
) -> Result<(), PrepareError> {
    if actual == Some(expected) {
        return Ok(());
    }
    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "deterministic tui edit-surface child '{}' carried surface patch {}, but {} was {:?}",
            node_id, expected, field, actual
        ),
    })
}

fn require_operation_target_binding(
    node_id: &str,
    field: &str,
    actual: Option<&crate::loop_graph::OperationTarget>,
    expected: &crate::loop_graph::ArtifactId,
) -> Result<(), PrepareError> {
    match actual {
        Some(crate::loop_graph::OperationTarget::Artifact { artifact_id })
            if artifact_id == expected =>
        {
            Ok(())
        }
        _ => Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "deterministic tui edit-surface child '{}' carried surface base Artifact {}, but {} was {:?}",
                node_id, expected, field, actual
            ),
        }),
    }
}

fn produce_deterministic_tui_tools_candidates(
    repo_root: &Path,
    parent: &Parent<Ready>,
    child_budget: Prototype1ChildBudget,
) -> Result<DeterministicTuiToolsCandidates, PrepareError> {
    let edit_surface = Prototype1EditSurface::PlokeTuiTools;
    let parent_identity = parent.identity();
    if child_budget.min == 0 || child_budget.max == 0 || child_budget.min > child_budget.max {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "invalid tui edit-surface child budget: min={} max={}",
                child_budget.min, child_budget.max
            ),
        });
    }
    let max = child_budget.max as usize;
    let min = child_budget.min as usize;
    let seed = format!(
        "{}:{}",
        parent_identity.node_id(),
        parent_identity.generation()
    );
    let replacements = (0..max).map(|index| {
        format!(
            concat!(
                "\n// prototype1 deterministic scaffold/no-op candidate {}:{}; ",
                "not semantic improvement evidence; next: replace deterministic direct-splice ",
                "generation with live proposal production.\n"
            ),
            seed,
            index + 1
        )
    });
    let proposals = deterministic_surface_proposals(repo_root, &seed, replacements, min, max)?;
    let backend = GitWorktreeBackend;
    let mut checked = Vec::with_capacity(proposals.len());
    let mut rejected_attempts = Vec::new();
    let mut proposed_hashes = BTreeSet::new();

    for proposal in proposals {
        let rejected_attempt = |reason: String, proposal: &EditProposal| {
            let target_relpath = proposal
                .touches
                .first()
                .map(|touch| touch.relpath.clone())
                .unwrap_or_else(|| PathBuf::from("."));
            surface_attempt::Evidence::rejected(
                TUI_EDIT_SURFACE_PRODUCER_ID,
                proposal.proposal_id.clone(),
                proposal.run_id.clone(),
                serde_name(&proposal.surface).to_string(),
                target_relpath,
                reason,
            )
        };

        let admission = match deterministic_tui_edit_surface_admission(
            repo_root,
            *parent.runtime_id(),
            &proposal,
        ) {
            Ok(admission) => admission,
            Err(reason) => {
                rejected_attempts.push(rejected_attempt(reason, &proposal));
                continue;
            }
        };
        let candidate =
            match backend.validate_edit_surface_candidate(repo_root, admission, proposal.clone()) {
                Ok(candidate) => candidate,
                Err(source) => {
                    rejected_attempts.push(rejected_attempt(source.to_string(), &proposal));
                    continue;
                }
            };
        if proposed_hashes.insert(candidate.proposed_content_hash().to_string()) {
            checked.push(candidate);
        }
    }

    if checked.len() < min {
        return Err(CandidateGenerationError::InsufficientUniqueProposals {
            surface: edit_surface,
            min,
            produced: checked.len(),
        }
        .into_prepare());
    }

    Ok(DeterministicTuiToolsCandidates {
        checked,
        rejected_attempts,
    })
}

fn deterministic_tui_edit_surface_admission(
    repo_root: &Path,
    runtime_id: crate::loop_graph::RuntimeId,
    proposal: &EditProposal,
) -> Result<EditSurfaceAdmission, String> {
    let mut paths = proposal
        .touches
        .iter()
        .map(|touch| touch.relpath.clone())
        .collect::<Vec<_>>();
    paths.sort();
    paths.dedup();
    let [target_relpath] = paths.as_slice() else {
        return Err(format!(
            "deterministic tui edit-surface proposal '{}' must target exactly one parent Artifact path, got {}",
            proposal.proposal_id,
            paths.len()
        ));
    };
    let source_path = repo_root.join(target_relpath);
    let source_content = fs::read_to_string(&source_path).map_err(|source| {
        format!(
            "deterministic tui edit-surface proposal '{}' could not read parent Artifact target '{}': {}",
            proposal.proposal_id,
            source_path.display(),
            source
        )
    })?;
    let artifact_id = crate::intervention::text_file_artifact_id(target_relpath, &source_content);
    Ok(EditSurfaceAdmission::new(
        crate::loop_graph::Coordinate {
            runtime_id,
            target: crate::loop_graph::OperationTarget::Artifact { artifact_id },
        },
        crate::cli::prototype1_state::edit_surface::surface::SurfacePolicyId::new(
            TUI_EDIT_SURFACE_POLICY_ID,
        ),
    ))
}

fn deterministic_surface_proposals(
    repo_root: &Path,
    seed: &str,
    replacements: impl IntoIterator<Item = String>,
    min: usize,
    max: usize,
) -> Result<Vec<EditProposal>, PrepareError> {
    let edit_surface = Prototype1EditSurface::PlokeTuiTools;
    let targets = deterministic_surface_targets(repo_root)?;
    let offset = deterministic_target_offset(seed, targets.len());
    let mut proposals = Vec::new();
    let mut proposed_hashes = BTreeSet::new();

    for (index, replacement) in replacements.into_iter().enumerate() {
        if proposals.len() >= max {
            break;
        }
        let relpath = targets[(offset + index) % targets.len()].clone();
        let target = repo_root.join(&relpath);
        let source = fs::read_to_string(&target).map_err(|source| PrepareError::ReadManifest {
            path: target.clone(),
            source,
        })?;
        let base_hash = format!("{:x}", Sha256::digest(source.as_bytes()));
        let start = source.len();
        let replacement = comment_replacement_for(&relpath, replacement);
        let proposed_hash = format!("{:x}", Sha256::digest(format!("{source}{replacement}")));
        if !proposed_hashes.insert(proposed_hash) {
            continue;
        }
        let touches = vec![ProposedTouch {
            target: "direct-splice:eof-comment".to_string(),
            relpath: relpath.clone(),
            start,
            end: start,
            expected_file_hash: base_hash.clone(),
            replacement,
        }];
        let generator_surface = GitWorktreeBackend
            .generator_surface_for_proposed_touches(&relpath, &source, &touches)
            .map_err(|source| {
                CandidateGenerationError::EvidenceProjection {
                    detail: source.to_string(),
                }
                .into_prepare()
            })?;
        proposals.push(EditProposal {
            surface: edit_surface,
            proposal_id: format!("tui-edit-surface-proposal-{:02}", index + 1),
            run_id: format!("tui-edit-surface-run-{:02}", index + 1),
            proposal_producer:
                crate::cli::prototype1_state::edit_surface::request_policy::ProposalProducer::NonRouter,
            generator_surface,
            reported_after_file_hash: None,
            touches,
        });
    }

    if proposals.len() < min {
        return Err(CandidateGenerationError::InsufficientUniqueProposals {
            surface: edit_surface,
            min,
            produced: proposals.len(),
        }
        .into_prepare());
    }

    Ok(proposals)
}

fn deterministic_surface_targets(repo_root: &Path) -> Result<Vec<PathBuf>, PrepareError> {
    let edit_surface = Prototype1EditSurface::PlokeTuiTools;
    let mut targets = edit_surface_paths(repo_root, edit_surface)
        .map_err(|source| {
            CandidateGenerationError::EvidenceProjection {
                detail: source.to_string(),
            }
            .into_prepare()
        })?
        .into_iter()
        .filter(|path| supported_text_surface_target(path))
        .collect::<Vec<_>>();
    targets.sort();
    targets.dedup();
    if targets.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "candidate-generator=deterministic-tui-tools found no writable text targets for edit-surface={edit_surface:?}; protected root '{}'",
                EVAL_CORE_SURFACE_ROOT
            ),
        });
    }
    Ok(targets)
}

fn supported_text_surface_target(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some("rs" | "md" | "toml" | "txt")
    )
}

fn deterministic_target_offset(seed: &str, len: usize) -> usize {
    debug_assert!(len > 0);
    let digest = Sha256::digest(seed.as_bytes());
    let mut bytes = [0_u8; 8];
    bytes.copy_from_slice(&digest[..8]);
    (u64::from_be_bytes(bytes) as usize) % len
}

fn comment_replacement_for(relpath: &Path, body: String) -> String {
    match relpath.extension().and_then(|ext| ext.to_str()) {
        Some("md") => format!("\n<!-- {} -->\n", body.trim()),
        Some("toml" | "txt") => format!("\n# {}\n", body.trim()),
        _ => body,
    }
}

fn receive_existing_child_plan(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    parent: Parent<Ready>,
) -> Result<ChildPlanReceipt, PrepareError> {
    let parent_identity = parent.identity().clone();
    let at = crate::cli::prototype1_state::inner::At::<ChildPlanFile>::resolve((
        manifest_path.to_path_buf(),
        parent_identity.node_id().to_string(),
    ));
    let locked = Locked::<ChildPlan>::from_box(at, read_child_plan_message).map_err(|err| {
        let (_at, source) = err.into_parts();
        source
    })?;
    let planned = parent.planned_from_locked_child_plan();
    receive_child_plan(
        campaign_id,
        manifest_path,
        repo_root,
        &parent_identity,
        planned,
        locked,
    )
}

fn receive_child_plan(
    _campaign_id: &str,
    _manifest_path: &Path,
    _repo_root: &Path,
    parent_identity: &ParentIdentity,
    planned: Parent<Planned>,
    locked: Locked<ChildPlan>,
) -> Result<ChildPlanReceipt, PrepareError> {
    if planned.identity().node_id() != parent_identity.node_id() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "planned parent '{}' did not match active parent '{}'",
                planned.identity().node_id(),
                parent_identity.node_id()
            ),
        });
    }
    let (parent, plan) = locked.unlock(planned).map_err(|err| {
        let (_failed, source) = err.into_parts();
        PrepareError::InvalidBatchSelection {
            detail: source.to_string(),
        }
    })?;
    let rejected_surface_attempts = plan.body().rejected_surface_attempts().to_vec();
    Ok(ChildPlanReceipt {
        parent,
        plan,
        rejected_surface_attempts,
    })
}

fn write_child_plan_file(path: &Path, body: &ChildPlanFiles) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    write_json_file_pretty(path, body)
}

fn read_child_plan_message(
    path: &crate::cli::prototype1_state::inner::At<ChildPlanFile>,
) -> Result<ChildPlanFiles, PrepareError> {
    let bytes = fs::read(path.path()).map_err(|source| PrepareError::ReadManifest {
        path: path.path().to_path_buf(),
        source,
    })?;
    let body = serde_json::from_slice::<ChildPlanFiles>(&bytes).map_err(|source| {
        PrepareError::InvalidBatchSelection {
            detail: format!(
                "could not decode child plan message '{}': {source}",
                path.path().display()
            ),
        }
    })?;
    Ok(body)
}

fn validate_received_child_plan(
    parent: &ParentIdentity,
    plan: &Received<ChildPlan>,
    node_id: &str,
) -> Result<(), PrepareError> {
    let files = plan.body();
    if files.parent_node_id() != parent.node_id() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "received child plan recipient '{}' did not match parent '{}'",
                files.parent_node_id(),
                parent.node_id()
            ),
        });
    }
    if !files.contains_child(node_id) {
        let listed = files
            .children()
            .iter()
            .map(|child| child.node_id())
            .collect::<Vec<_>>()
            .join(", ");
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "node '{node_id}' was not included in received child plan for parent '{}'; planned children: {listed}",
                parent.node_id()
            ),
        });
    }
    Ok(())
}

fn validate_child_plan(
    parent: &ParentIdentity,
    report: &Prototype1LoopReport,
    files: &ChildPlanFiles,
) -> Result<(), PrepareError> {
    // Same direct-child lineage policy as resolve_next_child.
    let expected_generation = parent.generation() + 1;
    if files.parent_node_id() != parent.node_id() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "child plan recipient '{}' did not match parent '{}'",
                files.parent_node_id(),
                parent.node_id()
            ),
        });
    }
    if files.child_generation() != expected_generation {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "child plan generation {} did not match expected generation {}",
                files.child_generation(),
                expected_generation
            ),
        });
    }
    let valid_children = report
        .staged_children
        .iter()
        .filter(|child| {
            let node = child.node_record();
            node.generation == expected_generation
                && node.parent_node_id.as_deref() == Some(parent.node_id())
        })
        .count();

    if valid_children == 0 {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "child plan did not stage any generation {} children for parent '{}'",
                expected_generation,
                parent.node_id()
            ),
        });
    }

    let planned = files
        .children()
        .iter()
        .map(|child| child.node_id())
        .collect::<BTreeSet<_>>();
    let staged = report
        .staged_children
        .iter()
        .filter(|child| {
            let node = child.node_record();
            node.generation == expected_generation
                && node.parent_node_id.as_deref() == Some(parent.node_id())
        })
        .map(|child| child.node_id())
        .collect::<BTreeSet<_>>();
    if planned != staged {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "child plan children did not match staged children for parent '{}': planned={:?} staged={:?}",
                parent.node_id(),
                planned,
                staged
            ),
        });
    }

    Ok(())
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
    source_parent: Option<ParentIdentity>,
    repo_root: PathBuf,
    trace_path: PathBuf,
    batch_id: String,
    batch_manifest: PathBuf,
    prepared_instances: Vec<String>,
    campaign: Prototype1LoopCampaign,
}

impl Prototype1LoopControllerInput {
    fn from_command(command: &Prototype1LoopCommand) -> Result<Self, PrepareError> {
        if command.stop_after >= Prototype1LoopStopAfter::Compare && !command.dry_run {
            return Err(PrepareError::InvalidBatchSelection {
                detail:
                    "loop prototype1 --stop-after compare was removed from active execution; run typed child evaluation through `loop prototype1-state` instead, or stop the legacy wrapper at --stop-after intervention-apply"
                        .to_string(),
            });
        }
        let operator_profile = command
            .profile
            .as_deref()
            .map(profile::load_operator_profile)
            .transpose()?;
        let profile_ref = operator_profile.as_ref().map(|profile| &profile.profile);
        let (batch_manifest, prepared_batch) =
            prepare_or_load_prototype1_batch(command, profile_ref)?;
        let campaign = prepare_prototype1_loop_campaign(command, &prepared_batch, profile_ref)?;
        let admitted_profile = operator_profile
            .as_ref()
            .map(|profile| profile::admit_run_profile(&campaign.manifest_path, profile))
            .transpose()?;
        let trace_path = prototype1_trace_path(&campaign.manifest_path);
        let repo_root = std::env::current_dir().map_err(|source| PrepareError::ReadManifest {
            path: PathBuf::from("."),
            source,
        })?;
        let search_policy = if let Some(profile) = admitted_profile.as_ref() {
            profile.profile.search_policy()
        } else {
            search_policy_from_command(command)?
        };

        Ok(Self {
            stop_after: command.stop_after,
            dry_run: command.dry_run,
            stop_on_error: command.stop_on_error,
            protocol_model_id: command.protocol_model_id.clone(),
            protocol_provider: command.protocol_provider.clone(),
            search_policy,
            source_campaign: command.source_campaign.clone(),
            source_branch_id: command.source_branch_id.clone(),
            source_parent: None,
            repo_root,
            trace_path,
            batch_id: prepared_batch.batch_id.clone(),
            batch_manifest,
            prepared_instances: prepared_batch.instances.clone(),
            campaign,
        })
    }
}

fn search_policy_from_command(
    command: &Prototype1LoopCommand,
) -> Result<Prototype1SearchPolicy, PrepareError> {
    Ok(Prototype1SearchPolicy {
        max_generations: command.max_generations,
        max_total_nodes: command.max_total_nodes,
        child_budget: child_budget_from_command(command)?,
        child_schedule_mode: child_schedule_mode_from_command(command),
        stop_on_first_keep: command.stop_on_first_keep,
        require_keep_for_continuation: command.require_keep_for_continuation,
        explore_from_rejected: command.explore_from_rejected,
    })
}

fn child_budget_from_command(
    command: &Prototype1LoopCommand,
) -> Result<Prototype1ChildBudget, PrepareError> {
    if command.min_children == 0 || command.max_children == 0 {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "child budget values must be nonzero".to_string(),
        });
    }
    if command.min_children > command.max_children {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "--min-children {} cannot exceed --max-children {}",
                command.min_children, command.max_children
            ),
        });
    }
    Ok(Prototype1ChildBudget {
        min: command.min_children,
        max: command.max_children,
    })
}

fn child_schedule_mode_from_command(
    command: &Prototype1LoopCommand,
) -> Prototype1ChildScheduleMode {
    match command.child_schedule_mode {
        CliChildScheduleMode::FullBatch => Prototype1ChildScheduleMode::FullBatch,
        CliChildScheduleMode::AdaptiveBatch => Prototype1ChildScheduleMode::AdaptiveBatch,
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

    let mut baseline_instances = Vec::new();
    let mut selected_targets = Vec::new();
    let mut staged_nodes = Vec::new();
    let mut staged_children = Vec::new();
    let branch_evaluations = Vec::new();
    let selected_next_branch_id = None;
    let continuation_decision = None;
    let mut protocol_failures = Vec::new();
    let mut protocol_task_instances = Vec::new();
    let intervention_repo_root = input.repo_root;

    if input.source_campaign.is_some() {
        return Err(PrepareError::InvalidBatchSelection {
            detail:
                "loop prototype1 source-branch continuation was removed from active execution: successor continuity must arrive through History/channel/artifact handoff"
                    .to_string(),
        });
    }

    let mut eval_policy = campaign.resolved.eval.clone();
    if input.stop_on_error {
        eval_policy.stop_on_error = true;
    }
    let eval_report = {
        let _scope = TimingTrace::scope("loop.prototype1.advance_eval_closure");
        advance_eval_closure(&campaign.resolved, &eval_policy, false, None).await?
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
                        let resolved_branches = resolved_treatment_branches_from_synthesis(
                            &row.instance_id,
                            &synthesis,
                            Some(candidate.candidate_id.as_str()),
                            input.source_branch_id.as_deref(),
                        );
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
                        let generation = input
                            .source_parent
                            .as_ref()
                            .map(|parent| parent.generation() + 1)
                            .unwrap_or(1);
                        let parent_node_id =
                            input.source_parent.as_ref().map(|parent| parent.node_id());
                        for resolved in resolved_branches
                            .into_iter()
                            .take(search_policy.child_budget.max as usize)
                        {
                            if staged_nodes.len() as u32 >= search_policy.max_total_nodes {
                                break;
                            }
                            let (node, _) = write_treatment_evaluation_projection(
                                &campaign.campaign_id,
                                &campaign.manifest_path,
                                &resolved,
                                generation,
                                parent_node_id,
                                &intervention_repo_root,
                                input.stop_on_error,
                            )?;
                            staged_children.push(ChildFiles::from_resolved(
                                &campaign.campaign_id,
                                node.clone(),
                                resolved,
                                input.stop_on_error,
                            ));
                            staged_nodes.push(node);
                        }
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

    if input.stop_after >= Prototype1LoopStopAfter::Compare && !input.dry_run {
        return Err(PrepareError::InvalidBatchSelection {
            detail:
                "loop prototype1 compare was removed from active execution: child evaluation must run through the typed prototype1-state channel/History path"
                    .to_string(),
        });
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
        staged_children,
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
    run_profile: Option<&profile::Prototype1RunProfile>,
) -> Result<(PathBuf, PreparedMsbBatch), PrepareError> {
    if command.batch.is_some() || command.batch_id.is_some() {
        return load_prepared_batch_for_loop(resolve_batch_manifest(
            command.batch.clone(),
            command.batch_id.clone(),
        )?);
    }

    let dataset_key = command
        .dataset_key
        .clone()
        .or_else(|| run_profile.and_then(|profile| profile.target.dataset_key.clone()));
    let instance_ids = if command.instance.is_empty() {
        run_profile
            .map(|profile| profile.target.eval_instances())
            .unwrap_or_default()
    } else {
        command.instance.clone()
    };
    let batch_id = command.prepare_batch_id.clone().unwrap_or_else(|| {
        default_batch_id(
            dataset_key.as_deref(),
            command.dataset.as_ref(),
            command.all,
            &instance_ids,
            &command.specific,
        )
    });
    let prepared = PrepareMsbBatchRequest {
        dataset_file: command.dataset.clone(),
        dataset_key,
        batch_id,
        select_all: command.all,
        instance_ids,
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

#[derive(Debug, Clone, PartialEq, Eq)]
struct TerminalState {
    reason: &'static str,
    detail: String,
}

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

fn run_metric_slice(
    campaign_id: &str,
    manifest_path: &Path,
    command: &Prototype1MetricsCommand,
) -> Result<(), PrepareError> {
    crate::cli::prototype1_state::metrics::run(
        campaign_id,
        manifest_path,
        crate::cli::prototype1_state::metrics::MetricRequest {
            rows: command.rows,
            generation: command.generation,
            view: command.view,
            format: command.format,
        },
    )
}

fn run_child_evidence(
    campaign_id: &str,
    manifest_path: &Path,
    command: &Prototype1ChildEvidenceCommand,
) -> Result<(), PrepareError> {
    crate::cli::prototype1_state::history_preview::run_child_evidence(
        campaign_id,
        manifest_path,
        command.format,
    )
}

fn run_score_report(
    campaign_id: &str,
    manifest_path: &Path,
    command: &Prototype1ScoreCommand,
) -> Result<(), PrepareError> {
    crate::cli::prototype1_state::score::run(
        campaign_id,
        manifest_path,
        crate::cli::prototype1_state::score::ScoreRequest {
            rows: command.rows,
            generation: command.generation,
            format: command.format,
        },
    )
}

fn run_score_selection_review(
    campaign_id: &str,
    manifest_path: &Path,
    command: &Prototype1ScoreCommand,
) -> Result<(), PrepareError> {
    crate::cli::prototype1_state::score::run_selection_review(
        campaign_id,
        manifest_path,
        crate::cli::prototype1_state::score::ScoreSelectionReviewRequest {
            rows: command.rows,
            generation: command.generation,
            format: command.format,
        },
    )
}

fn run_selection_show(
    campaign_id: &str,
    manifest_path: &Path,
    command: &Prototype1SelectionShowCommand,
) -> Result<(), PrepareError> {
    crate::cli::prototype1_state::history_preview::run_selection_show(
        campaign_id,
        manifest_path,
        crate::cli::prototype1_state::history_preview::SelectionShowRequest {
            row: command.row,
            replay: command.replay,
            format: command.format,
        },
    )
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
            volatility: "append-only typed record stream; appended as branches are synthesized, selected, applied, and compared",
            description: "Branch registry snapshots and parent comparison summaries.",
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

#[derive(Debug, Clone, Serialize)]
struct Report {
    schema_version: &'static str,
    generated_at: String,
    campaign_id: String,
    prototype_root: PathBuf,
}

#[derive(Debug, Clone, Serialize)]
struct NodeEvidence {
    node_id: String,
    generation: u32,
    parent_node_id: Option<String>,
    branch_id: String,
    status: String,
    created_at: String,
    updated_at: String,
    runner_result_exists: bool,
    runner_result_recorded_at: Option<String>,
    runner_result_disposition: Option<String>,
    runs: Vec<RunArtifacts>,
    streams: Vec<StreamEvidence>,
    traces: Vec<TurnTrace>,
    observation_steps: Vec<ObservationStep>,
    provider_http: Vec<ProviderHttpEvent>,
}

#[derive(Debug, Clone, Serialize)]
struct RunArtifacts {
    record_path: PathBuf,
    #[serde(skip_serializing)]
    record: RunRecord,
    responses: Option<ResponseSidecar>,
}

#[derive(Debug, Clone, Serialize)]
struct ResponseSidecar {
    path: PathBuf,
    #[serde(skip_serializing)]
    records: Vec<RawFullResponseRecord>,
}

#[derive(Debug, Clone, Serialize)]
struct StreamEvidence {
    runtime_id: String,
    stderr_path: PathBuf,
    stderr_modified_at: Option<String>,
    branch_eval_started_at: Option<String>,
    branch_eval_seconds: Option<f64>,
    http_body_timeouts: usize,
    http_body_timeout_seconds: f64,
    chat_retries: usize,
    chat_backoff_seconds: f64,
    llm_errors: usize,
}

#[derive(Debug, Clone, Serialize)]
struct TurnTrace {
    path: PathBuf,
    modified_at: Option<String>,
    event_counts: BTreeMap<String, usize>,
    tool_completed: usize,
    tool_failed: usize,
    tool_latency_ms_total: u64,
    tool_latency_ms_max: u64,
}

#[derive(Debug, Clone, Serialize)]
struct ObservationStep {
    timestamp: Option<String>,
    node_id: Option<String>,
    generation: Option<u64>,
    step: String,
    outcome: Option<String>,
    duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
struct ProviderHttpEvent {
    source: PathBuf,
    timestamp: Option<String>,
    campaign_id: Option<String>,
    node_id: Option<String>,
    branch_id: Option<String>,
    generation: Option<u64>,
    runtime_id: Option<String>,
    role: Option<String>,
    runtime_phase: Option<String>,
    request_id: u64,
    attempt: u32,
    max_attempts: Option<u32>,
    event: String,
    phase: Option<String>,
    status: Option<u16>,
    elapsed_ms: Option<u64>,
    backoff_ms: Option<u64>,
    model: Option<String>,
    request_bytes: Option<u64>,
    response_bytes: Option<u64>,
    is_timeout: Option<bool>,
    failure: Option<String>,
    provider_attempt: Option<ProviderAttempt>,
    #[serde(skip_serializing)]
    observation: ProviderHttpObservation,
}

#[derive(Debug, Clone)]
enum ProviderHttpObservation {
    AttemptTimeline(ProviderAttempt),
    MalformedProviderAttempt(ProviderAttemptParseFailureProjection),
    RequestError(ProviderRequestErrorProjection),
    StatusError(ProviderStatusErrorProjection),
    RetryScheduled(ProviderRetryScheduledProjection),
    RetrySuppressed(ProviderRetrySuppressedProjection),
    Other(ProviderOtherObservation),
}

#[derive(Debug, Clone)]
struct ProviderRequestErrorProjection {
    elapsed_ms: Option<u64>,
    status: Option<u16>,
    failure: Option<String>,
    body_failure: Option<HttpBodyFailure>,
    is_timeout: bool,
}

#[derive(Debug, Clone)]
struct ProviderStatusErrorProjection {
    elapsed_ms: Option<u64>,
    status: u16,
}

#[derive(Debug, Clone)]
struct ProviderRetryScheduledProjection {
    elapsed_ms: Option<u64>,
    backoff_ms: Option<u64>,
    status: Option<u16>,
}

#[derive(Debug, Clone)]
struct ProviderRetrySuppressedProjection {
    elapsed_ms: Option<u64>,
    status: Option<u16>,
    body_failure: Option<HttpBodyFailure>,
}

#[derive(Debug, Clone)]
struct ProviderOtherObservation {
    event: String,
    elapsed_ms: Option<u64>,
    backoff_ms: Option<u64>,
    status: Option<u16>,
    is_timeout: bool,
}

#[derive(Debug, Clone)]
struct ProviderAttemptParseFailureProjection {
    raw: String,
    error: String,
}

#[derive(Debug, Clone, Deserialize)]
struct ObservationTraceRecord {
    timestamp: Option<String>,
    target: Option<String>,
    event: Option<String>,
    phase: Option<String>,
    message: Option<String>,
    outcome: Option<String>,
    #[serde(default, deserialize_with = "optional_u64")]
    duration_ms: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64")]
    request_id: Option<u64>,
    #[serde(default, deserialize_with = "optional_u32")]
    attempt: Option<u32>,
    #[serde(default, deserialize_with = "optional_u32")]
    max_attempts: Option<u32>,
    #[serde(default, deserialize_with = "optional_u16")]
    status: Option<u16>,
    #[serde(default, deserialize_with = "optional_u64")]
    elapsed_ms: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64")]
    backoff_ms: Option<u64>,
    model: Option<String>,
    #[serde(default, deserialize_with = "optional_u64")]
    request_bytes: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64")]
    response_bytes: Option<u64>,
    is_timeout: Option<bool>,
    failure: Option<String>,
    url: Option<String>,
    receive_phase: Option<String>,
    raw_error: Option<String>,
    reason: Option<String>,
    #[serde(default, deserialize_with = "optional_u64")]
    retry_after_ms: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64")]
    started_at_ms: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64")]
    request_sent_ms: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64")]
    headers_received_ms: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64")]
    output_started_ms: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64")]
    output_progress_ms: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64")]
    output_completed_ms: Option<u64>,
    #[serde(default, deserialize_with = "optional_u64")]
    failed_ms: Option<u64>,
    #[serde(default)]
    failure_phase: Option<ploke_llm::ProviderFailurePhase>,
    #[serde(default, deserialize_with = "optional_http_body_failure")]
    body_failure: Option<HttpBodyFailure>,
    #[serde(default)]
    retry_decision: Option<ploke_llm::ProviderRetryDecision>,
    span: Option<ObservationTraceSpan>,
    #[serde(default)]
    spans: Vec<ObservationTraceSpan>,
    #[serde(default, deserialize_with = "optional_provider_attempt_compatibility")]
    provider_attempt: Option<ProviderAttemptCompatibility>,
}

#[derive(Debug, Clone, Deserialize)]
struct ObservationTraceSpan {
    name: Option<String>,
    campaign_id: Option<String>,
    campaign: Option<String>,
    node_id: Option<String>,
    branch_id: Option<String>,
    #[serde(default, deserialize_with = "optional_u64")]
    generation: Option<u64>,
    runtime_id: Option<String>,
    role: Option<String>,
    runtime_phase: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum ProviderAttemptProjection {
    JsonString(String),
    Record(ProviderAttempt),
}

#[derive(Debug, Clone)]
enum ProviderAttemptCompatibility {
    Parsed(ProviderAttempt),
    MalformedLegacy(ProviderAttemptParseFailureProjection),
}

impl ProviderAttemptCompatibility {
    fn from_legacy_string(raw: String) -> Self {
        match serde_json::from_str::<ProviderAttempt>(&raw) {
            Ok(attempt) => Self::Parsed(attempt),
            Err(error) => Self::MalformedLegacy(ProviderAttemptParseFailureProjection {
                raw,
                error: error.to_string(),
            }),
        }
    }

    fn parsed(&self) -> Option<&ProviderAttempt> {
        match self {
            Self::Parsed(attempt) => Some(attempt),
            Self::MalformedLegacy(_) => None,
        }
    }

    fn parse_failure(&self) -> Option<&ProviderAttemptParseFailureProjection> {
        match self {
            Self::Parsed(_) => None,
            Self::MalformedLegacy(failure) => Some(failure),
        }
    }
}

impl ProviderHttpObservation {
    fn from_record(record: &ObservationTraceRecord) -> Self {
        match record.event.as_deref() {
            Some("provider_attempt") => provider_attempt_from_record(record)
                .map(Self::AttemptTimeline)
                .or_else(|| {
                    record
                        .provider_attempt
                        .as_ref()
                        .and_then(ProviderAttemptCompatibility::parse_failure)
                        .cloned()
                        .map(Self::MalformedProviderAttempt)
                })
                .unwrap_or_else(|| {
                    Self::Other(ProviderOtherObservation::from_record(
                        record,
                        "provider_attempt",
                    ))
                }),
            Some("chat_http_request_error") => Self::RequestError(ProviderRequestErrorProjection {
                elapsed_ms: record.elapsed_ms,
                status: record.status,
                failure: record.failure.clone(),
                body_failure: record.body_failure.clone(),
                is_timeout: record.is_timeout.unwrap_or(false),
            }),
            Some("chat_http_response_error_status") => record
                .status
                .map(|status| {
                    Self::StatusError(ProviderStatusErrorProjection {
                        elapsed_ms: record.elapsed_ms,
                        status,
                    })
                })
                .unwrap_or_else(|| {
                    Self::Other(ProviderOtherObservation::from_record(
                        record,
                        "chat_http_response_error_status",
                    ))
                }),
            Some("chat_http_retry_scheduled") => {
                Self::RetryScheduled(ProviderRetryScheduledProjection {
                    elapsed_ms: record.elapsed_ms,
                    backoff_ms: record.backoff_ms,
                    status: record.status,
                })
            }
            Some("chat_http_retry_suppressed") => {
                Self::RetrySuppressed(ProviderRetrySuppressedProjection {
                    elapsed_ms: record.elapsed_ms,
                    status: record.status,
                    body_failure: record.body_failure.clone(),
                })
            }
            Some(event) => Self::Other(ProviderOtherObservation::from_record(record, event)),
            None => Self::Other(ProviderOtherObservation::from_record(record, "")),
        }
    }

    fn event_name(&self) -> &str {
        match self {
            Self::AttemptTimeline(_) => "provider_attempt",
            Self::MalformedProviderAttempt(_) => "provider_attempt_parse_failure",
            Self::RequestError(_) => "chat_http_request_error",
            Self::StatusError(_) => "chat_http_response_error_status",
            Self::RetryScheduled(_) => "chat_http_retry_scheduled",
            Self::RetrySuppressed(_) => "chat_http_retry_suppressed",
            Self::Other(record) => record.event.as_str(),
        }
    }

    fn provider_attempt(&self) -> Option<&ProviderAttempt> {
        match self {
            Self::AttemptTimeline(attempt) => Some(attempt),
            _ => None,
        }
    }

    fn failure(&self) -> Option<String> {
        match self {
            Self::MalformedProviderAttempt(failure) => Some(format!(
                "malformed_legacy_provider_attempt: {}; raw_bytes={}",
                failure.error,
                failure.raw.len()
            )),
            _ => None,
        }
    }

    fn apply_to_attempt(&self, attempt: &mut ProviderHttpAttempt) {
        match self {
            Self::AttemptTimeline(provider_attempt) => {
                if let Some(elapsed_ms) = provider_attempt_elapsed_ms(provider_attempt) {
                    attempt.elapsed_ms = Some(attempt.elapsed_ms.unwrap_or(0).max(elapsed_ms));
                }
                if let Some(backoff_ms) = provider_attempt
                    .backoff
                    .map(|duration| duration.as_millis() as u64)
                {
                    attempt.backoff_ms = attempt.backoff_ms.max(backoff_ms);
                }
                attempt.status = provider_attempt.status.or(attempt.status);
                attempt.has_error |= provider_attempt.outcome == ProviderAttemptOutcome::Failed;
                attempt.is_timeout |=
                    provider_attempt.body_failure == Some(HttpBodyFailure::Timeout);
            }
            Self::MalformedProviderAttempt(_) => {
                attempt.has_error = true;
            }
            Self::RequestError(record) => {
                apply_elapsed(attempt, record.elapsed_ms);
                attempt.status = record.status.or(attempt.status);
                attempt.has_error = true;
                attempt.is_timeout |=
                    record.is_timeout || record.body_failure == Some(HttpBodyFailure::Timeout);
                if record.failure.as_deref() == Some("timeout") {
                    attempt.is_timeout = true;
                }
            }
            Self::StatusError(record) => {
                apply_elapsed(attempt, record.elapsed_ms);
                attempt.status = Some(record.status).or(attempt.status);
                attempt.has_error = true;
            }
            Self::RetryScheduled(record) => {
                apply_elapsed(attempt, record.elapsed_ms);
                if let Some(backoff_ms) = record.backoff_ms {
                    attempt.backoff_ms = attempt.backoff_ms.saturating_add(backoff_ms);
                }
                attempt.status = record.status.or(attempt.status);
            }
            Self::RetrySuppressed(record) => {
                apply_elapsed(attempt, record.elapsed_ms);
                attempt.status = record.status.or(attempt.status);
                attempt.has_error = true;
                attempt.is_timeout |= record.body_failure == Some(HttpBodyFailure::Timeout);
            }
            Self::Other(record) => {
                apply_elapsed(attempt, record.elapsed_ms);
                if let Some(backoff_ms) = record.backoff_ms {
                    attempt.backoff_ms = attempt.backoff_ms.saturating_add(backoff_ms);
                }
                attempt.status = record.status.or(attempt.status);
                attempt.is_timeout |= record.is_timeout;
            }
        }
    }
}

impl ProviderOtherObservation {
    fn from_record(record: &ObservationTraceRecord, event: &str) -> Self {
        Self {
            event: event.to_string(),
            elapsed_ms: record.elapsed_ms,
            backoff_ms: record.backoff_ms,
            status: record.status,
            is_timeout: record.is_timeout.unwrap_or(false),
        }
    }
}

fn apply_elapsed(attempt: &mut ProviderHttpAttempt, elapsed_ms: Option<u64>) {
    if let Some(elapsed_ms) = elapsed_ms {
        attempt.elapsed_ms = Some(attempt.elapsed_ms.unwrap_or(0).max(elapsed_ms));
    }
}

fn provider_attempt_from_record(record: &ObservationTraceRecord) -> Option<ProviderAttempt> {
    if let Some(attempt) = record
        .provider_attempt
        .as_ref()
        .and_then(ProviderAttemptCompatibility::parsed)
        .cloned()
    {
        return Some(attempt);
    }
    let timeline = ProviderAttemptTimeline {
        request_id: record.request_id?,
        attempt: record.attempt.unwrap_or(1),
        max_attempts: record.max_attempts?,
        started_at_ms: record.started_at_ms.unwrap_or(0),
        request_sent_ms: record.request_sent_ms,
        headers_received_ms: record.headers_received_ms,
        output_started_ms: record.output_started_ms,
        output_progress_ms: record.output_progress_ms,
        output_completed_ms: record.output_completed_ms,
        failed_ms: record.failed_ms,
        status: record.status,
        response_bytes: record.response_bytes.map(|value| value as usize),
        outcome: record
            .outcome
            .as_deref()
            .and_then(parse_provider_attempt_outcome)?,
        failure_phase: record.failure_phase,
        body_failure: record.body_failure.clone(),
        retry_decision: record
            .retry_decision
            .unwrap_or(ploke_llm::ProviderRetryDecision::None),
        backoff_ms: record.backoff_ms,
    };
    Some(timeline.into_attempt())
}

fn parse_provider_attempt_outcome(value: &str) -> Option<ProviderAttemptOutcome> {
    match value {
        "completed" => Some(ProviderAttemptOutcome::Completed),
        "failed" => Some(ProviderAttemptOutcome::Failed),
        _ => None,
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum U64Projection {
    Number(u64),
    String(String),
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProviderAttemptEventKey {
    source: PathBuf,
    timestamp: Option<String>,
    request_id: u64,
    attempt: u32,
}

fn print_prototype1_monitor_timing(
    campaign_id: &str,
    manifest_path: &Path,
    command: &Prototype1MonitorTimingCommand,
) -> Result<(), PrepareError> {
    if command.watch {
        return watch_prototype1_monitor_timing(campaign_id, manifest_path, command);
    }
    let report = Report::load(campaign_id, manifest_path)?;
    print_timing_report(&report, command)?;
    Ok(())
}

fn watch_prototype1_monitor_timing(
    campaign_id: &str,
    manifest_path: &Path,
    command: &Prototype1MonitorTimingCommand,
) -> Result<(), PrepareError> {
    let prototype_root = prototype1_campaign_root(manifest_path);
    let interval = Duration::from_millis(command.interval_ms.max(1));
    let mut seen = BTreeSet::new();

    if !command.phone {
        println!("watching provider attempts");
        println!("campaign_id: {campaign_id}");
        println!("campaign_root: {}", prototype_root.display());
        println!("interval_ms: {}", interval.as_millis());
        println!("press Ctrl-C to stop");
    }
    if matches!(command.format, InspectOutputFormat::Table) && !command.phone {
        println!("{}", provider_attempt_watch_header(command.show_paths));
    }

    let observations = load_observation_evidence(campaign_id);
    for event in observations
        .provider_http
        .iter()
        .filter(|event| should_watch_provider_attempt(event, command))
    {
        seen.insert(provider_attempt_event_key(event));
        print_provider_attempt_event(event, command)?;
    }
    std::io::stdout().flush().ok();

    loop {
        let observations = load_observation_evidence(campaign_id);
        for event in observations
            .provider_http
            .iter()
            .filter(|event| should_watch_provider_attempt(event, command))
        {
            let key = provider_attempt_event_key(event);
            if seen.insert(key) {
                print_provider_attempt_event(event, command)?;
            }
        }

        let snapshot = collect_prototype1_monitor_snapshot(&prototype_root, None)?;
        if let Some(terminal) = terminal_state(manifest_path, &prototype_root, &snapshot) {
            if command.phone {
                print_phone_terminal_state(&terminal);
            } else {
                print_terminal_state(&terminal);
            }
            return Ok(());
        }

        thread::sleep(interval);
    }
}

fn should_watch_provider_attempt(
    event: &ProviderHttpEvent,
    command: &Prototype1MonitorTimingCommand,
) -> bool {
    event.provider_attempt.is_some()
        && command
            .node
            .as_deref()
            .is_none_or(|node| event.node_id.as_deref() == Some(node))
}

fn provider_attempt_event_key(event: &ProviderHttpEvent) -> ProviderAttemptEventKey {
    ProviderAttemptEventKey {
        source: event.source.clone(),
        timestamp: event.timestamp.clone(),
        request_id: event.request_id,
        attempt: event.attempt,
    }
}

fn print_provider_attempt_event(
    event: &ProviderHttpEvent,
    command: &Prototype1MonitorTimingCommand,
) -> Result<(), PrepareError> {
    if event.provider_attempt.is_none() {
        return Ok(());
    }
    if matches!(command.format, InspectOutputFormat::Json) {
        println!(
            "{}",
            serde_json::to_string(event).map_err(PrepareError::Serialize)?
        );
        return Ok(());
    }

    let row = if command.phone {
        provider_attempt_phone_row(event)
    } else {
        provider_attempt_watch_row(event, command.show_paths)
    };
    if let Some(line) = row {
        println!("{line}");
    }
    Ok(())
}

fn provider_attempt_watch_header(show_paths: bool) -> String {
    let mut line = format!(
        "{:<30} {:>3} {:<12} {:<12} {:>7} {:>7} {:>9} {:>9} {:<10} {:<12} {:>6} {:<12}",
        "time",
        "gen",
        "node",
        "branch",
        "request",
        "attempt",
        "elapsed",
        "backoff",
        "outcome",
        "retry",
        "status",
        "body"
    );
    if show_paths {
        line.push_str(" source");
    }
    line
}

fn provider_attempt_watch_row(event: &ProviderHttpEvent, show_paths: bool) -> Option<String> {
    let attempt = event.provider_attempt.as_ref()?;
    let timestamp = event.timestamp.as_deref().unwrap_or("-");
    let generation = event
        .generation
        .map(|generation| generation.to_string())
        .unwrap_or_else(|| "-".to_string());
    let branch = event
        .branch_id
        .as_deref()
        .map(short_id)
        .unwrap_or("-")
        .to_string();
    let node = event
        .node_id
        .as_deref()
        .map(short_id)
        .unwrap_or("-")
        .to_string();
    let max_attempts = event
        .max_attempts
        .or(Some(attempt.max_attempts))
        .map(|max_attempts| max_attempts.to_string())
        .unwrap_or_else(|| "-".to_string());
    let elapsed = provider_attempt_elapsed_ms(attempt)
        .map(format_duration)
        .unwrap_or_else(|| "-".to_string());
    let backoff = attempt
        .backoff
        .map(|duration| format_duration(duration.as_millis() as u64))
        .unwrap_or_else(|| "-".to_string());
    let status = attempt
        .status
        .map(|status| status.to_string())
        .unwrap_or_else(|| "-".to_string());
    let body = attempt
        .body_failure
        .as_ref()
        .map(serde_name)
        .unwrap_or_else(|| "-".to_string());

    let mut line = format!(
        "{:<30} {:>3} {:<12} {:<12} {:>7} {:>7} {:>9} {:>9} {:<10} {:<12} {:>6} {:<12}",
        fit_cell(timestamp, 30),
        fit_cell(&generation, 3),
        fit_cell(&node, 12),
        fit_cell(&branch, 12),
        attempt.request_id,
        format!("{}/{}", attempt.attempt, max_attempts),
        fit_cell(&elapsed, 9),
        fit_cell(&backoff, 9),
        fit_cell(&serde_name(&attempt.outcome), 10),
        fit_cell(&serde_name(&attempt.retry_decision), 12),
        fit_cell(&status, 6),
        fit_cell(&body, 12),
    );
    if show_paths {
        line.push_str(&format!(" source={}", event.source.display()));
    }
    Some(line)
}

fn provider_attempt_phone_row(event: &ProviderHttpEvent) -> Option<String> {
    let attempt = event.provider_attempt.as_ref()?;
    let time = phone_time(event.timestamp.as_deref());
    let generation = event
        .generation
        .map(|generation| generation.min(999).to_string())
        .unwrap_or_else(|| "-".to_string());
    let node = phone_node(event.node_id.as_deref());
    let elapsed = provider_attempt_elapsed_ms(attempt)
        .map(phone_duration)
        .unwrap_or_else(|| "-".to_string());
    let code = phone_outcome_code(attempt);

    Some(format!(
        "{time} g{:>3} {node:<5} r{:>3}/{:<1} {:>4} {code:<2}",
        generation, attempt.request_id, attempt.attempt, elapsed
    ))
}

fn phone_time(timestamp: Option<&str>) -> String {
    timestamp
        .and_then(|timestamp| DateTime::parse_from_rfc3339(timestamp).ok())
        .map(|timestamp| timestamp.with_timezone(&Utc).format("%H:%M").to_string())
        .unwrap_or_else(|| "--:--".to_string())
}

fn phone_node(node_id: Option<&str>) -> String {
    let Some(node_id) = node_id else {
        return "n----".to_string();
    };
    let body = node_id.strip_prefix("node-").unwrap_or(node_id);
    let short = body.chars().take(4).collect::<String>();
    format!("n{short}")
}

fn phone_duration(elapsed_ms: u64) -> String {
    let seconds = ((elapsed_ms + 500) / 1000).min(999);
    if elapsed_ms >= 999_500 {
        "999+".to_string()
    } else {
        format!("{seconds}s")
    }
}

fn phone_outcome_code(attempt: &ProviderAttempt) -> &'static str {
    if attempt.body_failure == Some(HttpBodyFailure::Timeout) {
        return "TO";
    }
    if attempt.outcome == ProviderAttemptOutcome::Completed {
        return "OK";
    }
    if attempt
        .status
        .is_some_and(|status| !(200..300).contains(&status))
    {
        return "HT";
    }
    if attempt.retry_decision == ploke_llm::ProviderRetryDecision::Exhausted {
        return "EX";
    }
    "ER"
}

fn print_phone_terminal_state(terminal: &TerminalState) {
    let reason = match terminal.reason {
        "node_failed" => "node_fail",
        "parent_process_exited" => "parent_exit",
        other => other,
    };
    println!(
        "{} term {}",
        Utc::now().format("%H:%M"),
        fit_cell(reason, 22)
    );
}

fn print_timing_report(
    report: &Report,
    command: &Prototype1MonitorTimingCommand,
) -> Result<(), PrepareError> {
    match command.format {
        InspectOutputFormat::Json => {
            println!(
                "{}",
                serde_json::to_string_pretty(report).map_err(PrepareError::Serialize)?
            );
        }
        InspectOutputFormat::Table => report.print(command.depth, command.show_paths),
    }
    Ok(())
}

impl Report {
    fn load(campaign_id: &str, manifest_path: &Path) -> Result<Self, PrepareError> {
        let prototype_root = prototype1_campaign_root(manifest_path);

        Ok(Self {
            schema_version: "prototype1-monitor-timing.v2",
            generated_at: Utc::now().to_rfc3339(),
            campaign_id: campaign_id.to_string(),
            prototype_root,
        })
    }

    fn print(&self, depth: Depth, show_paths: bool) {
        println!("prototype1 timing");
        println!("{}", "-".repeat(40));
        println!("schema_version: {}", self.schema_version);
        println!("generated_at: {}", self.generated_at);
        println!("campaign_id: {}", self.campaign_id);
        println!("prototype_root: {}", self.prototype_root.display());
        println!("depth: {:?}", depth);
        println!();

        self.forest(depth, show_paths).render(depth);
    }

    fn forest(&self, _depth: Depth, _show_paths: bool) -> Forest {
        let roots = Vec::new();
        let omitted_roots = 0usize;
        Forest {
            roots,
            omitted_roots,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct Field {
    label: &'static str,
    value: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case", tag = "state")]
enum Elapsed {
    Measured { ms: u64 },
    Missing { reason: &'static str },
}

#[derive(Debug, Clone, Serialize)]
struct Span {
    name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    elapsed: Option<Elapsed>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    fields: Vec<Field>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    children: Vec<Span>,
}

#[derive(Debug, Clone, Serialize)]
struct Forest {
    roots: Vec<Span>,
    omitted_roots: usize,
}

impl Field {
    fn new(label: &'static str, value: impl ToString) -> Self {
        Self {
            label,
            value: value.to_string(),
        }
    }
}

impl Span {
    fn new(name: impl Into<String>, elapsed_ms: Option<u64>) -> Self {
        Self {
            name: name.into(),
            elapsed: elapsed_ms.map(|ms| Elapsed::Measured { ms }),
            fields: Vec::new(),
            children: Vec::new(),
        }
    }

    fn missing_elapsed(mut self, reason: &'static str) -> Self {
        self.elapsed = Some(Elapsed::Missing { reason });
        self
    }

    fn field(mut self, label: &'static str, value: impl ToString) -> Self {
        self.fields.push(Field::new(label, value));
        self
    }

    fn child(mut self, child: Span) -> Self {
        self.children.push(child);
        self
    }

    fn children<I>(mut self, children: I) -> Self
    where
        I: IntoIterator<Item = Span>,
    {
        self.children.extend(children);
        self
    }

    fn self_ms(&self) -> Option<u64> {
        self.elapsed_ms().map(|elapsed| {
            let child_total = self.children.iter().filter_map(Span::elapsed_ms).sum();
            elapsed.saturating_sub(child_total)
        })
    }

    fn elapsed_ms(&self) -> Option<u64> {
        match &self.elapsed {
            Some(Elapsed::Measured { ms }) => Some(*ms),
            Some(Elapsed::Missing { .. }) | None => None,
        }
    }

    fn missing_elapsed_reason(&self) -> Option<&'static str> {
        match &self.elapsed {
            Some(Elapsed::Missing { reason }) => Some(*reason),
            Some(Elapsed::Measured { .. }) | None => None,
        }
    }
}

impl Forest {
    fn render(&self, depth: Depth) {
        let depth_remaining = match depth {
            Depth::Node => Some(0),
            Depth::Phase => Some(4),
            Depth::Turn => Some(5),
            Depth::Call => None,
        };
        let global_ms: u64 = self.roots.iter().filter_map(Span::elapsed_ms).sum();
        let budget = Budget::new(&self.roots, depth_remaining);

        println!("{}", "═".repeat(budget.display_width));
        for root in &self.roots {
            print_span(root, global_ms, "", "", depth_remaining, budget);
        }

        print_containment_check(&self.roots, budget.display_width);
        print_span_stats(&self.roots, budget.display_width);
        print_missing_elapsed(&self.roots, budget.display_width);

        println!("{}", "─".repeat(budget.display_width));
        println!(
            "{:<width$} {:>8}            {:>6}",
            fit_cell("Observed total", budget.name_col),
            format_duration(global_ms),
            pct_text(global_ms, global_ms),
            width = budget.name_col,
        );
        if self.omitted_roots > 0 {
            println!("Omitted nodes_without_timing={}", self.omitted_roots);
        }
        println!("{}", "═".repeat(budget.display_width));
    }
}

#[derive(Debug, Clone, Copy)]
struct Budget {
    display_width: usize,
    name_col: usize,
}

impl Budget {
    const DEFAULT_WIDTH: usize = 120;
    const MIN_WIDTH: usize = 80;
    const NUMBER_COLS: usize = 30;
    const MIN_NAME_COL: usize = 28;

    fn new(roots: &[Span], depth_remaining: Option<usize>) -> Self {
        let width_limit = terminal_width()
            .unwrap_or(Self::DEFAULT_WIDTH)
            .clamp(Self::MIN_WIDTH, Self::DEFAULT_WIDTH);
        let name_limit = width_limit.saturating_sub(Self::NUMBER_COLS);
        let natural_name_col = roots
            .iter()
            .map(|root| structural_width(root, 0, depth_remaining))
            .max()
            .unwrap_or(0)
            .max(Self::MIN_NAME_COL)
            + 2;
        let name_col = natural_name_col.min(name_limit);

        Self {
            display_width: name_col + Self::NUMBER_COLS,
            name_col,
        }
    }
}

fn terminal_width() -> Option<usize> {
    std::env::var("COLUMNS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|width| *width > 0)
}

fn fit_cell(value: &str, width: usize) -> String {
    let len = value.chars().count();
    if len <= width {
        return value.to_string();
    }
    if width <= 3 {
        return ".".repeat(width);
    }

    let mut out = value
        .chars()
        .take(width.saturating_sub(3))
        .collect::<String>();
    out.push_str("...");
    out
}

fn structural_width(span: &Span, depth: usize, depth_remaining: Option<usize>) -> usize {
    let prefix_width = if depth == 0 { 0 } else { 3 * depth };
    let width = prefix_width + span.name.chars().count();
    if depth_remaining == Some(0) || (span.name == "responses" && depth_remaining.is_some()) {
        return width;
    }

    let next_depth = depth_remaining.map(|depth| depth.saturating_sub(1));
    span.children
        .iter()
        .map(|child| structural_width(child, depth + 1, next_depth))
        .fold(width, usize::max)
}

fn print_span(
    span: &Span,
    global_ms: u64,
    prefix: &str,
    connector: &str,
    depth_remaining: Option<usize>,
    budget: Budget,
) {
    let label = fit_cell(
        &format!("{prefix}{connector}{}", span.name),
        budget.name_col,
    );
    let elapsed = match &span.elapsed {
        Some(Elapsed::Measured { ms }) => format_duration(*ms),
        Some(Elapsed::Missing { .. }) => "missing".to_string(),
        None => "-".to_string(),
    };
    let self_part = if span.children.is_empty() {
        "           ".to_string()
    } else {
        span.self_ms()
            .map(|self_ms| format!("{:>11}", format!("({})", format_duration(self_ms))))
            .unwrap_or_else(|| "           ".to_string())
    };
    let pct = span
        .elapsed_ms()
        .map(|elapsed| pct_text(elapsed, global_ms))
        .unwrap_or_else(|| "-".to_string());
    println!(
        "{:<width$} {:>8}{} {:>6}",
        label,
        elapsed,
        self_part,
        pct,
        width = budget.name_col,
    );

    if depth_remaining == Some(0) || (span.name == "responses" && depth_remaining.is_some()) {
        return;
    }
    let next_depth = depth_remaining.map(|depth| depth.saturating_sub(1));
    let child_prefix = if connector.is_empty() {
        prefix.to_string()
    } else if connector.starts_with('└') {
        format!("{prefix}   ")
    } else {
        format!("{prefix}│  ")
    };
    let count = span.children.len();
    for (index, child) in span.children.iter().enumerate() {
        let child_connector = if index + 1 == count {
            "└─ "
        } else {
            "├─ "
        };
        print_span(
            child,
            global_ms,
            &child_prefix,
            child_connector,
            next_depth,
            budget,
        );
    }
}

fn print_containment_check(roots: &[Span], display_width: usize) {
    let rows = roots
        .iter()
        .filter_map(run_containment_row)
        .collect::<Vec<_>>();
    if rows.is_empty() {
        return;
    }
    print_section("containment", display_width);
    println!("  {:<18} {:>9} {:>9} {:>9}", "node", "wait", "run", "delta");
    for row in rows {
        println!(
            "  {:<18} {:>9} {:>9} {:>9}",
            row.node,
            format_duration(row.wait_ms),
            format_duration(row.run_ms),
            format_duration_signed(row.wait_ms as i128 - row.run_ms as i128),
        );
    }
}

fn print_span_stats(roots: &[Span], display_width: usize) {
    let mut groups: BTreeMap<&'static str, Vec<u64>> = BTreeMap::new();
    for root in roots {
        collect_span_stat(root, &mut groups);
    }
    if groups.is_empty() {
        return;
    }
    print_section("stats", display_width);
    println!(
        "  {:<8} {:>2} {:>9} {:>9} {:>9} {:>9}",
        "span", "n", "min", "avg", "max", "std"
    );
    for (name, values) in groups {
        if let Some(stats) = DurationStats::new(&values) {
            println!(
                "  {:<8} {:>2} {:>9} {:>9} {:>9} {:>9}",
                name,
                values.len(),
                format_duration(stats.min),
                format_duration(stats.avg),
                format_duration(stats.max),
                format_duration(stats.std),
            );
        }
    }
}

fn print_missing_elapsed(roots: &[Span], display_width: usize) {
    let mut groups: BTreeMap<&'static str, usize> = BTreeMap::new();
    for root in roots {
        collect_missing_elapsed(root, &mut groups);
    }
    if groups.is_empty() {
        return;
    }
    print_section("missing duration evidence", display_width);
    for (reason, count) in groups {
        println!("  {:<28} {:>4}", reason, count);
    }
}

fn print_section(title: &str, display_width: usize) {
    let label = format!("── {title} ");
    println!(
        "{label}{}",
        "─".repeat(display_width.saturating_sub(label.chars().count()))
    );
}

fn collect_span_stat(span: &Span, groups: &mut BTreeMap<&'static str, Vec<u64>>) {
    let key = match span.name.as_str() {
        "build" => Some("build"),
        "observe" => Some("observe"),
        "run" => Some("run"),
        "agent" => Some("agent"),
        _ => None,
    };
    if let (Some(key), Some(elapsed_ms)) = (key, span.elapsed_ms()) {
        groups.entry(key).or_default().push(elapsed_ms);
    }
    for child in &span.children {
        collect_span_stat(child, groups);
    }
}

fn collect_missing_elapsed(span: &Span, groups: &mut BTreeMap<&'static str, usize>) {
    if let Some(reason) = span.missing_elapsed_reason() {
        *groups.entry(reason).or_default() += 1;
    }
    for child in &span.children {
        collect_missing_elapsed(child, groups);
    }
}

struct ContainmentRow {
    node: String,
    wait_ms: u64,
    run_ms: u64,
}

fn run_containment_row(root: &Span) -> Option<ContainmentRow> {
    let observe = root.children.iter().find(|child| child.name == "observe")?;
    let wait = observe
        .children
        .iter()
        .find(|child| child.name == "wait_for_result")?;
    let run = wait.children.iter().find(|child| child.name == "run")?;
    Some(ContainmentRow {
        node: root.name.clone(),
        wait_ms: wait.elapsed_ms()?,
        run_ms: run.elapsed_ms()?,
    })
}

struct DurationStats {
    min: u64,
    avg: u64,
    max: u64,
    std: u64,
}

impl DurationStats {
    fn new(values: &[u64]) -> Option<Self> {
        let min = *values.iter().min()?;
        let max = *values.iter().max()?;
        let sum: u64 = values.iter().sum();
        let avg = sum / values.len() as u64;
        let variance = values
            .iter()
            .map(|value| {
                let diff = *value as f64 - avg as f64;
                diff * diff
            })
            .sum::<f64>()
            / values.len() as f64;
        Some(Self {
            min,
            avg,
            max,
            std: variance.sqrt() as u64,
        })
    }
}

fn pct_text(elapsed_ms: u64, global_ms: u64) -> String {
    if global_ms == 0 {
        "-".to_string()
    } else {
        format!("{:>5.1}%", elapsed_ms as f64 / global_ms as f64 * 100.0)
    }
}

fn format_duration(ms: u64) -> String {
    if ms >= 1000 {
        format!("{:.3}s", ms as f64 / 1000.0)
    } else {
        format!("{ms}ms")
    }
}

fn format_duration_signed(ms: i128) -> String {
    let sign = if ms >= 0 { "+" } else { "-" };
    format!("{sign}{}", format_duration(ms.unsigned_abs() as u64))
}

impl NodeEvidence {
    fn span(&self, depth: Depth, show_paths: bool) -> Span {
        let mut span = Span::new(
            format!("gen{} {}", self.generation, short_id(&self.node_id)),
            self.elapsed_ms(),
        )
        .field("status", self.status.clone())
        .field("branch", short_id(&self.branch_id));

        if let (Some(tools), Some(failed)) = (self.tool_count(), self.failed_tool_count()) {
            span = span.field("tools", format!("{tools}/{failed}"));
        }
        if let Some(tokens) = self.response_token_count() {
            span = span.field("tokens", format_token_count(tokens));
        }
        if self
            .streams
            .iter()
            .any(|stream| stream.http_body_timeouts > 0)
        {
            span = span.field("http_timeouts", self.http_body_timeouts());
        }
        if self.streams.iter().any(|stream| stream.chat_retries > 0) {
            span = span.field("chat_retries", self.chat_retries());
        }
        if !self.provider_http.is_empty() {
            let requests = self.provider_http_requests();
            span = span.field("provider_requests", requests.len()).field(
                "provider_http",
                format_duration(provider_http_total_ms(&requests)),
            );
        }
        if show_paths {
            span = span
                .field("branch_full", self.branch_id.clone())
                .field("runner_result", yes_no(self.runner_result_exists));
        }

        span.children(self.phase_spans(depth, show_paths))
    }

    fn elapsed_ms(&self) -> Option<u64> {
        let mut found = false;
        let total = self
            .phase_spans(Depth::Phase, false)
            .into_iter()
            .filter_map(|span| span.elapsed_ms())
            .inspect(|_| found = true)
            .sum();
        found.then_some(total)
    }

    fn observation_duration_by_prefix(&self, prefix: &str) -> Option<u64> {
        let mut found = false;
        let total = self
            .observation_steps
            .iter()
            .filter(|step| step.step.starts_with(prefix))
            .inspect(|_| found = true)
            .map(|step| step.duration_ms)
            .sum();
        found.then_some(total)
    }

    fn observation_duration_by_name(&self, name: &str) -> Option<u64> {
        let mut found = false;
        let total = self
            .observation_steps
            .iter()
            .filter(|step| step.step == name)
            .inspect(|_| found = true)
            .map(|step| step.duration_ms)
            .sum();
        found.then_some(total)
    }

    fn http_body_timeouts(&self) -> usize {
        self.streams
            .iter()
            .map(|stream| stream.http_body_timeouts)
            .sum()
    }

    fn chat_retries(&self) -> usize {
        self.streams.iter().map(|stream| stream.chat_retries).sum()
    }

    fn tool_count(&self) -> Option<usize> {
        if !self.runs.is_empty() {
            return Some(
                self.runs
                    .iter()
                    .map(|run| run_tool_count(&run.record))
                    .sum(),
            );
        }
        if self.traces.is_empty() {
            None
        } else {
            Some(
                self.traces
                    .iter()
                    .map(|trace| trace.tool_completed + trace.tool_failed)
                    .sum(),
            )
        }
    }

    fn failed_tool_count(&self) -> Option<usize> {
        if !self.runs.is_empty() {
            return Some(
                self.runs
                    .iter()
                    .map(|run| run_failed_tool_count(&run.record))
                    .sum(),
            );
        }
        if self.traces.is_empty() {
            None
        } else {
            Some(self.traces.iter().map(|trace| trace.tool_failed).sum())
        }
    }

    fn response_token_count(&self) -> Option<u32> {
        let mut found = false;
        let total = self
            .runs
            .iter()
            .filter_map(|run| run.responses.as_ref())
            .inspect(|_| found = true)
            .map(ResponseSidecar::token_count)
            .sum();
        found.then_some(total)
    }

    fn phase_spans(&self, depth: Depth, show_paths: bool) -> Vec<Span> {
        let mut spans = Vec::new();
        spans.extend(self.phase(
            "build",
            "prototype1.child.build.",
            &[
                ("check", "prototype1.child.build.cargo_check"),
                ("cargo", "prototype1.child.build.cargo_build"),
                ("promote", "prototype1.child.build.promote_binary"),
            ],
        ));
        spans.extend(self.observe_span(depth, show_paths));
        spans.extend(self.phase(
            "parent",
            "prototype1.parent.",
            &[
                ("genesis", "prototype1.parent.startup.genesis"),
                ("predecessor", "prototype1.parent.startup.predecessor"),
                ("select", "prototype1.parent.select_successor"),
                ("checkout", "prototype1.parent.checkout.active"),
            ],
        ));
        spans.extend(self.phase(
            "history",
            "prototype1.history.",
            &[
                ("seal", "prototype1.history.seal"),
                ("append", "prototype1.history.append"),
            ],
        ));
        spans.extend(self.phase(
            "successor",
            "prototype1.successor.",
            &[
                ("spawn", "prototype1.successor.spawn"),
                ("ready", "prototype1.successor.ready_wait"),
            ],
        ));
        spans.extend(self.phase(
            "cleanup",
            "prototype1.cleanup.",
            &[
                ("binary", "prototype1.cleanup.binary"),
                ("target", "prototype1.cleanup.target"),
            ],
        ));
        spans
    }

    fn has_timing_evidence(&self) -> bool {
        !self.runs.is_empty() || !self.phase_spans(Depth::Phase, false).is_empty()
    }

    fn should_expand(&self) -> bool {
        self.status != "planned" && self.has_timing_evidence()
    }

    fn observe_span(&self, depth: Depth, show_paths: bool) -> Option<Span> {
        let mut observe = Span::new(
            "observe",
            self.observation_duration_by_prefix("prototype1.child.observe."),
        );
        let mut wait = self
            .observation_duration_by_name("prototype1.child.observe.wait_for_result")
            .map(|duration| Span::new("wait_for_result", Some(duration)));
        let provider_span = self.provider_http_span(depth, show_paths);
        for (index, run) in self.runs.iter().enumerate() {
            let provider_span = (index == 0).then(|| provider_span.clone()).flatten();
            if let Some(wait) = &mut wait {
                wait.children
                    .push(run.span(depth, show_paths, provider_span));
            } else {
                observe
                    .children
                    .push(run.span(depth, show_paths, provider_span));
            }
        }
        if self.runs.is_empty() {
            if let Some(provider_span) = provider_span {
                if let Some(wait) = &mut wait {
                    wait.children.push(provider_span);
                } else {
                    observe.children.push(provider_span);
                }
            }
        }
        if let Some(wait) = wait {
            observe.children.push(wait);
        }
        for (name, step) in [
            (
                "load_runner_result",
                "prototype1.child.observe.load_runner_result",
            ),
            (
                "load_evaluation_report",
                "prototype1.child.observe.load_evaluation_report",
            ),
        ] {
            if let Some(duration) = self.observation_duration_by_name(step) {
                observe.children.push(Span::new(name, Some(duration)));
            }
        }
        (!observe.children.is_empty() || observe.elapsed.is_some()).then_some(observe)
    }

    fn provider_http_requests(&self) -> Vec<ProviderHttpRequest> {
        provider_http_requests(&self.provider_http)
    }

    fn provider_http_span(&self, depth: Depth, show_paths: bool) -> Option<Span> {
        let requests = self.provider_http_requests();
        if requests.is_empty() {
            return self.response_token_count().is_some().then(|| {
                Span::new("provider_http", None).missing_elapsed("chat_http_not_recorded")
            });
        }

        let retries: usize = requests.iter().map(ProviderHttpRequest::retry_count).sum();
        let errors: usize = requests.iter().map(ProviderHttpRequest::error_count).sum();
        let backoff_ms: u64 = requests.iter().map(ProviderHttpRequest::backoff_ms).sum();
        let total_ms = provider_http_total_ms(&requests);
        let name = format!(
            "provider_http requests={} retries={} backoff={}",
            requests.len(),
            retries,
            format_duration(backoff_ms)
        );
        let mut span = Span::new(name, Some(total_ms))
            .field("errors", errors)
            .field("timeouts", provider_http_timeout_count(&requests));
        if show_paths {
            span = span.field("sources", provider_http_source_count(&requests));
        }
        if matches!(depth, Depth::Turn | Depth::Call) {
            span = span.children(
                requests
                    .iter()
                    .map(|request| request.span(depth, show_paths)),
            );
        }
        Some(span)
    }

    fn phase(
        &self,
        name: &'static str,
        prefix: &str,
        children: &[(&'static str, &str)],
    ) -> Option<Span> {
        let span = Span::new(name, self.observation_duration_by_prefix(prefix)).children(
            children.iter().filter_map(|(label, step)| {
                self.observation_duration_by_name(step)
                    .map(|duration| Span::new(*label, Some(duration)))
            }),
        );
        (!span.children.is_empty() || span.elapsed.is_some()).then_some(span)
    }
}

impl ResponseSidecar {
    fn token_count(&self) -> u32 {
        self.records
            .iter()
            .filter_map(|record| record.response().usage.as_ref())
            .map(|usage| usage.total_tokens)
            .sum()
    }

    fn span(&self, show_paths: bool, _include_records: bool) -> Span {
        let mut span = Span::new("responses", None)
            .field("count", self.records.len())
            .field("tokens", format_token_count(self.token_count()));
        if show_paths {
            span = span.field("path", self.path.display());
        }
        span.children(self.records.iter().map(response_span))
    }
}

impl RunArtifacts {
    fn span(&self, depth: Depth, show_paths: bool, provider_http: Option<Span>) -> Span {
        let timing = self.record.timing.as_ref();
        let mut span = Span::new(
            "run",
            timing.map(|timing| seconds_to_millis(timing.total_wall_clock_secs)),
        )
        .field("turns", self.record.phases.agent_turns.len())
        .field("tools", run_tool_count(&self.record))
        .field("failed_tools", run_failed_tool_count(&self.record));

        if let Some(responses) = &self.responses {
            span = span
                .field("responses", responses.records.len())
                .field("tokens", format_token_count(responses.token_count()));
        }
        if show_paths {
            span = span.field("record", self.record_path.display());
        }

        if let Some(agent_ms) = timing
            .and_then(|timing| timing.agent_wall_clock_secs)
            .map(seconds_to_millis)
        {
            span = span.child(
                Span::new("agent", Some(agent_ms))
                    .children(self.record.phases.agent_turns.iter().map(turn_span)),
            );
        } else {
            span = span.children(self.record.phases.agent_turns.iter().map(turn_span));
        }

        if let Some(responses) = &self.responses {
            span = span.child(responses.span(show_paths, matches!(depth, Depth::Call)));
        }
        if let Some(provider_http) = provider_http {
            span = span.child(provider_http);
        } else if self.responses.is_some() {
            span = span
                .child(Span::new("provider_http", None).missing_elapsed("chat_http_not_recorded"));
        }

        span
    }
}

fn turn_span(turn: &crate::record::TurnRecord) -> Span {
    let span = Span::new(
        format!("turn {}", turn.turn_number),
        turn_duration_ms(turn).filter(|duration| *duration > 0),
    );
    let span = if span.elapsed.is_none() {
        span.missing_elapsed("turn_duration_not_recorded")
    } else {
        span
    };
    span.field("outcome", turn_outcome_label(&turn.outcome))
        .field("tools", turn.tool_calls.len())
        .field("failed_tools", turn_failed_tool_count(turn))
        .children(turn.tool_calls.iter().map(tool_span))
}

fn tool_span(call: &crate::record::ToolExecutionRecord) -> Span {
    Span::new(format!("tool {}", call.request.tool), Some(call.latency_ms))
        .field(
            "status",
            match &call.result {
                crate::record::ToolResult::Completed(_) => "completed",
                crate::record::ToolResult::Failed(_) => "failed",
            },
        )
        .field("call_id", short_id(&call.request.call_id))
}

fn response_span(record: &RawFullResponseRecord) -> Span {
    let response = record.response();
    let usage = response.usage.as_ref();
    let mut span = Span::new(format!("response {}", record.response_index()), None)
        .missing_elapsed("provider_latency_not_recorded")
        .field(
            "finish",
            response
                .choices
                .first()
                .and_then(|choice| choice.finish_reason.as_ref())
                .map(serde_name)
                .unwrap_or_else(|| "-".to_string()),
        );
    if let Some(usage) = usage {
        span = span.field("tokens", format_token_count(usage.total_tokens));
    }
    if !response.model.is_empty() {
        span = span.field("model", response.model.clone());
    }
    if let Some(provider) = &response.provider {
        span = span.field("provider", format!("{provider:?}"));
    }
    span
}

#[derive(Debug, Clone)]
struct ProviderHttpRequest {
    source: PathBuf,
    request_id: u64,
    max_attempts: Option<u32>,
    model: Option<String>,
    status: Option<u16>,
    attempts: BTreeMap<u32, ProviderHttpAttempt>,
}

#[derive(Debug, Clone, Default)]
struct ProviderHttpAttempt {
    elapsed_ms: Option<u64>,
    backoff_ms: u64,
    status: Option<u16>,
    events: BTreeSet<String>,
    has_error: bool,
    is_timeout: bool,
}

impl ProviderHttpRequest {
    fn elapsed_ms(&self) -> u64 {
        self.attempts
            .values()
            .map(ProviderHttpAttempt::wall_ms)
            .sum()
    }

    fn retry_count(&self) -> usize {
        self.attempts
            .values()
            .filter(|attempt| attempt.events.contains("chat_http_retry_scheduled"))
            .count()
    }

    fn error_count(&self) -> usize {
        self.attempts
            .values()
            .filter(|attempt| attempt.has_error)
            .count()
    }

    fn timeout_count(&self) -> usize {
        self.attempts
            .values()
            .filter(|attempt| attempt.is_timeout)
            .count()
    }

    fn backoff_ms(&self) -> u64 {
        self.attempts
            .values()
            .map(|attempt| attempt.backoff_ms)
            .sum()
    }

    fn span(&self, depth: Depth, show_paths: bool) -> Span {
        let retries = self.retry_count();
        let mut name = format!("request {}", self.request_id);
        if retries > 0 {
            name.push_str(&format!(" retries={retries}"));
        }
        if let Some(status) = self.status {
            name.push_str(&format!(" status={status}"));
        }
        let mut span = Span::new(name, Some(self.elapsed_ms()));
        if let Some(model) = &self.model {
            span = span.field("model", model);
        }
        if let Some(max_attempts) = self.max_attempts {
            span = span.field("max_attempts", max_attempts);
        }
        if show_paths {
            span = span.field("source", self.source.display());
        }
        if matches!(depth, Depth::Call) {
            span = span.children(
                self.attempts
                    .iter()
                    .map(|(attempt, evidence)| evidence.span(*attempt)),
            );
        }
        span
    }
}

impl ProviderHttpAttempt {
    fn wall_ms(&self) -> u64 {
        self.elapsed_ms.unwrap_or(0) + self.backoff_ms
    }

    fn span(&self, attempt: u32) -> Span {
        let mut name = format!("attempt {attempt}");
        if self.events.contains("chat_http_retry_scheduled") {
            name.push_str(" retry");
        }
        if self.has_error {
            name.push_str(" error");
        }
        if self.is_timeout {
            name.push_str(" timeout");
        }
        if let Some(status) = self.status {
            name.push_str(&format!(" status={status}"));
        }

        let mut children = Vec::new();
        if let Some(elapsed_ms) = self.elapsed_ms {
            children.push(Span::new("http", Some(elapsed_ms)));
        }
        if self.backoff_ms > 0 {
            children.push(Span::new("backoff", Some(self.backoff_ms)));
        }
        Span::new(name, Some(self.wall_ms())).children(children)
    }
}

fn provider_http_requests(events: &[ProviderHttpEvent]) -> Vec<ProviderHttpRequest> {
    let mut requests: BTreeMap<(PathBuf, u64), ProviderHttpRequest> = BTreeMap::new();
    for event in events {
        let request = requests
            .entry((event.source.clone(), event.request_id))
            .or_insert_with(|| ProviderHttpRequest {
                source: event.source.clone(),
                request_id: event.request_id,
                max_attempts: event.max_attempts,
                model: event.model.clone(),
                status: event.status,
                attempts: BTreeMap::new(),
            });
        request.max_attempts = request.max_attempts.or(event.max_attempts);
        if request.model.is_none() {
            request.model = event.model.clone();
        }
        request.status = event.status.or(request.status);

        let attempt = request.attempts.entry(event.attempt).or_default();
        attempt
            .events
            .insert(event.observation.event_name().to_string());
        event.observation.apply_to_attempt(attempt);
    }

    requests.into_values().collect()
}

fn provider_attempt_elapsed_ms(attempt: &ProviderAttempt) -> Option<u64> {
    attempt
        .failed
        .or(attempt.output_completed)
        .or(attempt.headers_received)
        .map(|duration| duration.as_millis() as u64)
}

fn provider_http_total_ms(requests: &[ProviderHttpRequest]) -> u64 {
    requests.iter().map(ProviderHttpRequest::elapsed_ms).sum()
}

fn provider_http_timeout_count(requests: &[ProviderHttpRequest]) -> usize {
    requests
        .iter()
        .map(ProviderHttpRequest::timeout_count)
        .sum()
}

fn provider_http_source_count(requests: &[ProviderHttpRequest]) -> usize {
    requests
        .iter()
        .map(|request| &request.source)
        .collect::<BTreeSet<_>>()
        .len()
}

fn seconds_to_millis(seconds: f64) -> u64 {
    (seconds.max(0.0) * 1000.0).round() as u64
}

fn load_run_artifacts(
    campaign_id: &str,
    branch_id: &str,
) -> Result<Vec<RunArtifacts>, PrepareError> {
    let Some(root) = home_dir().map(|home| {
        home.join(".ploke-eval")
            .join("instances")
            .join("prototype1")
            .join(campaign_id)
            .join("treatments")
            .join(branch_id)
    }) else {
        return Ok(Vec::new());
    };
    if !root.exists() {
        return Ok(Vec::new());
    }

    let mut paths = Vec::new();
    collect_named_files(&root, "record.json.gz", &mut paths);
    paths.sort();
    paths
        .into_iter()
        .map(load_run_artifact)
        .collect::<Result<Vec<_>, _>>()
}

fn load_run_artifact(record_path: PathBuf) -> Result<RunArtifacts, PrepareError> {
    let record =
        read_compressed_record(&record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.clone(),
            source,
        })?;
    let run_dir = record_path
        .parent()
        .ok_or_else(|| PrepareError::MissingRunManifest(record_path.clone()))?;
    let response_path = run_dir.join(FULL_RESPONSE_TRACE_FILE);
    let responses = if response_path.exists() {
        Some(ResponseSidecar {
            records: load_run_responses(&response_path)?,
            path: response_path,
        })
    } else {
        None
    };

    Ok(RunArtifacts {
        record_path,
        record,
        responses,
    })
}

fn load_run_responses(path: &Path) -> Result<Vec<RawFullResponseRecord>, PrepareError> {
    let text = fs::read_to_string(path).map_err(|source| PrepareError::ReadManifest {
        path: path.to_path_buf(),
        source,
    })?;
    let mut responses = Vec::new();
    for line in text.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        responses.push(serde_json::from_str(trimmed).map_err(|source| {
            PrepareError::ParseManifest {
                path: path.to_path_buf(),
                source,
            }
        })?);
    }
    responses.sort_by_key(|record: &RawFullResponseRecord| record.response_index());
    Ok(responses)
}

fn turn_outcome_label(outcome: &crate::record::TurnOutcome) -> String {
    match outcome {
        crate::record::TurnOutcome::ToolCalls { count } => format!("tool_calls:{count}"),
        crate::record::TurnOutcome::Content => "content".to_string(),
        crate::record::TurnOutcome::Error { .. } => "error".to_string(),
        crate::record::TurnOutcome::Timeout { elapsed_secs } => {
            format!("timeout:{elapsed_secs}s")
        }
    }
}

fn duration_between_ms(started_at: &str, ended_at: &str) -> Option<u64> {
    let started = DateTime::parse_from_rfc3339(started_at).ok()?;
    let ended = DateTime::parse_from_rfc3339(ended_at).ok()?;
    let ms = ended.signed_duration_since(started).num_milliseconds();
    u64::try_from(ms.max(0)).ok()
}

fn turn_duration_ms(turn: &crate::record::TurnRecord) -> Option<u64> {
    duration_between_ms(&turn.started_at, &turn.ended_at)
}

fn run_tool_count(record: &RunRecord) -> usize {
    record
        .phases
        .agent_turns
        .iter()
        .map(|turn| turn.tool_calls.len())
        .sum()
}

fn run_failed_tool_count(record: &RunRecord) -> usize {
    record
        .phases
        .agent_turns
        .iter()
        .flat_map(|turn| &turn.tool_calls)
        .filter(|call| matches!(call.result, crate::record::ToolResult::Failed(_)))
        .count()
}

fn turn_failed_tool_count(turn: &crate::record::TurnRecord) -> usize {
    turn.tool_calls
        .iter()
        .filter(|call| matches!(call.result, crate::record::ToolResult::Failed(_)))
        .count()
}

fn load_stream_timings(node_dir: &Path) -> Result<Vec<StreamEvidence>, PrepareError> {
    let streams_dir = node_dir.join("streams");
    let mut timings = Vec::new();
    if !streams_dir.exists() {
        return Ok(timings);
    }
    for entry in fs::read_dir(&streams_dir).map_err(|source| PrepareError::ReadManifest {
        path: streams_dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: streams_dir.clone(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let runtime_id = path
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("(unknown)")
            .to_string();
        let stderr_path = path.join("stderr.log");
        if stderr_path.is_file() {
            timings.push(parse_stderr_timing(runtime_id, stderr_path)?);
        }
    }
    timings.sort_by(|left, right| left.runtime_id.cmp(&right.runtime_id));
    Ok(timings)
}

fn parse_stderr_timing(
    runtime_id: String,
    stderr_path: PathBuf,
) -> Result<StreamEvidence, PrepareError> {
    let text = fs::read_to_string(&stderr_path).map_err(|source| PrepareError::ReadManifest {
        path: stderr_path.clone(),
        source,
    })?;
    let ansi = regex::Regex::new(r"\x1b\[[0-9;]*m").expect("valid ANSI regex");
    let elapsed = regex::Regex::new(r"elapsed_ms=([0-9]+)").expect("valid elapsed regex");
    let backoff = regex::Regex::new(r"retry_delay_secs=([0-9.]+)").expect("valid backoff regex");
    let branch_eval_start = regex::Regex::new(
        r"^([0-9]{2}:[0-9]{2}:[0-9]{2}) loop\.prototype1_branch\.evaluate\.[^ ]+\.start",
    )
    .expect("valid branch eval start regex");
    let branch_eval_end =
        regex::Regex::new(r"loop\.prototype1_branch\.evaluate\.[^ ]+\.end \+([0-9.]+)s")
            .expect("valid branch eval end regex");

    let mut branch_eval_started_at = None;
    let mut branch_eval_seconds = None;
    let mut http_body_timeouts = 0;
    let mut http_body_timeout_seconds = 0.0;
    let mut chat_retries = 0;
    let mut chat_backoff_seconds = 0.0;
    let mut llm_errors = 0;

    for raw_line in text.lines() {
        let line = ansi.replace_all(raw_line, "");
        if branch_eval_started_at.is_none() {
            if let Some(captures) = branch_eval_start.captures(&line) {
                branch_eval_started_at = captures.get(1).map(|value| value.as_str().to_string());
            }
        }
        if let Some(captures) = branch_eval_end.captures(&line) {
            branch_eval_seconds = captures
                .get(1)
                .and_then(|value| value.as_str().parse::<f64>().ok());
        }
        if line.contains("chat_http_request_error")
            && line.contains("body_failure")
            && line.contains("timeout")
            && line.contains("status=200")
        {
            http_body_timeouts += 1;
            if let Some(captures) = elapsed.captures(&line) {
                if let Some(ms) = captures
                    .get(1)
                    .and_then(|value| value.as_str().parse::<u64>().ok())
                {
                    http_body_timeout_seconds += ms as f64 / 1000.0;
                }
            }
        }
        if line.contains("chat_step failed; retrying") {
            chat_retries += 1;
            if let Some(captures) = backoff.captures(&line) {
                if let Some(seconds) = captures
                    .get(1)
                    .and_then(|value| value.as_str().parse::<f64>().ok())
                {
                    chat_backoff_seconds += seconds;
                }
            }
        }
        if line.contains("LLM request ended with error") {
            llm_errors += 1;
        }
    }

    Ok(StreamEvidence {
        runtime_id,
        stderr_modified_at: file_modified_at(&stderr_path),
        stderr_path,
        branch_eval_started_at,
        branch_eval_seconds,
        http_body_timeouts,
        http_body_timeout_seconds,
        chat_retries,
        chat_backoff_seconds,
        llm_errors,
    })
}

fn find_agent_turn_traces(campaign_id: &str) -> Vec<TurnTrace> {
    let Some(root) = home_dir().map(|home| {
        home.join(".ploke-eval")
            .join("instances")
            .join("prototype1")
            .join(campaign_id)
    }) else {
        return Vec::new();
    };
    let mut paths = Vec::new();
    collect_named_files(&root, "agent-turn-trace.json", &mut paths);
    let mut traces = paths
        .into_iter()
        .filter_map(|path| parse_turn_trace(path).ok())
        .collect::<Vec<_>>();
    traces.sort_by(|left, right| left.path.cmp(&right.path));
    traces
}

fn parse_turn_trace(path: PathBuf) -> Result<TurnTrace, PrepareError> {
    let text = fs::read_to_string(&path).map_err(|source| PrepareError::ReadManifest {
        path: path.clone(),
        source,
    })?;
    let trace: AgentTurnTraceProjection =
        serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
            path: path.clone(),
            source,
        })?;
    let mut event_counts = BTreeMap::new();
    let mut tool_completed = 0;
    let mut tool_failed = 0;
    let mut tool_latency_ms_total = 0;
    let mut tool_latency_ms_max = 0;

    for event in &trace.events {
        *event_counts.entry(event.kind().to_string()).or_insert(0) += 1;
        if event.is_tool_completed() {
            tool_completed += 1;
        } else if event.is_tool_failed() {
            tool_failed += 1;
        }
        if let Some(latency) = event.tool_latency_ms() {
            tool_latency_ms_total += latency;
            tool_latency_ms_max = tool_latency_ms_max.max(latency);
        }
    }

    Ok(TurnTrace {
        modified_at: file_modified_at(&path),
        path,
        event_counts,
        tool_completed,
        tool_failed,
        tool_latency_ms_total,
        tool_latency_ms_max,
    })
}

#[derive(Debug, Default)]
struct ObservationEvidence {
    steps: Vec<ObservationStep>,
    provider_http: Vec<ProviderHttpEvent>,
}

fn load_observation_evidence(campaign_id: &str) -> ObservationEvidence {
    let Some(log_root) = home_dir().map(|home| home.join(".ploke-eval").join("logs")) else {
        return ObservationEvidence::default();
    };
    let mut paths = Vec::new();
    collect_observation_logs(&log_root, &mut paths);
    let mut evidence = ObservationEvidence::default();
    for path in paths {
        parse_observation_log(campaign_id, &path, &mut evidence);
    }
    evidence
        .steps
        .sort_by(|left, right| left.timestamp.cmp(&right.timestamp));
    evidence
        .provider_http
        .sort_by(|left, right| left.timestamp.cmp(&right.timestamp));
    evidence
}

fn parse_observation_log(campaign_id: &str, path: &Path, evidence: &mut ObservationEvidence) {
    let Ok(text) = fs::read_to_string(path) else {
        return;
    };
    let file_mentions_campaign = observation_log_mentions_campaign(&text, campaign_id);
    for line in text.lines() {
        let Ok(record) = serde_json::from_str::<ObservationTraceRecord>(line) else {
            continue;
        };
        let mut event_campaign = trace_string_field(&record, "campaign_id")
            .or_else(|| trace_string_field(&record, "campaign"));
        if event_campaign.is_none() && file_mentions_campaign {
            event_campaign = Some(campaign_id.to_string());
        }
        if event_campaign.as_deref() != Some(campaign_id) {
            continue;
        }
        if record.target.as_deref() == Some("chat_http")
            || record
                .event
                .as_deref()
                .is_some_and(|event| event.starts_with("chat_http_"))
        {
            if let Some(event) = provider_http_event(path, &record, event_campaign) {
                evidence.provider_http.push(event);
            }
            continue;
        }
        let Some(duration_ms) = record.duration_ms else {
            continue;
        };
        let step = record
            .span
            .as_ref()
            .and_then(|span| span_string_field(span, "name"))
            .or_else(|| record.phase.clone())
            .or_else(|| record.message.clone())
            .unwrap_or_else(|| "(unnamed)".to_string());
        evidence.steps.push(ObservationStep {
            timestamp: record.timestamp.clone(),
            node_id: trace_string_field(&record, "node_id"),
            generation: trace_u64_field(&record, "generation"),
            step,
            outcome: record.outcome.clone(),
            duration_ms,
        });
    }
}

fn observation_log_mentions_campaign(text: &str, campaign_id: &str) -> bool {
    // TEMPORARY 2026-05-05: this only preserves older Prototype 1 provider
    // attempt logs that lack span-scoped campaign fields. It can misattribute
    // unscoped provider attempts if one log file contains multiple campaigns.
    // Task stack: `prototype1-remove-unscoped-provider-attribution-bridge`.
    text.lines().any(|line| {
        serde_json::from_str::<ObservationTraceRecord>(line)
            .ok()
            .and_then(|record| {
                trace_string_field(&record, "campaign_id")
                    .or_else(|| trace_string_field(&record, "campaign"))
            })
            .as_deref()
            == Some(campaign_id)
    })
}

fn provider_http_event(
    path: &Path,
    value: &ObservationTraceRecord,
    campaign_id: Option<String>,
) -> Option<ProviderHttpEvent> {
    let observation = ProviderHttpObservation::from_record(value);
    let provider_attempt = observation.provider_attempt().cloned();
    Some(ProviderHttpEvent {
        source: path.to_path_buf(),
        timestamp: value.timestamp.clone(),
        campaign_id,
        node_id: trace_string_field(value, "node_id"),
        branch_id: trace_string_field(value, "branch_id"),
        generation: trace_u64_field(value, "generation"),
        runtime_id: trace_string_field(value, "runtime_id"),
        role: trace_string_field(value, "role"),
        runtime_phase: trace_string_field(value, "runtime_phase"),
        request_id: value
            .request_id
            .or_else(|| provider_attempt.as_ref().map(|attempt| attempt.request_id))?,
        attempt: value
            .attempt
            .or_else(|| provider_attempt.as_ref().map(|attempt| attempt.attempt))
            .unwrap_or(1),
        max_attempts: value.max_attempts.or_else(|| {
            provider_attempt
                .as_ref()
                .map(|attempt| attempt.max_attempts)
        }),
        event: observation.event_name().to_string(),
        phase: value.phase.clone(),
        status: value.status,
        elapsed_ms: value.elapsed_ms,
        backoff_ms: value.backoff_ms,
        model: value.model.clone(),
        request_bytes: value.request_bytes,
        response_bytes: value.response_bytes,
        is_timeout: value.is_timeout,
        failure: value.failure.clone().or_else(|| observation.failure()),
        provider_attempt,
        observation,
    })
}

fn optional_provider_attempt_compatibility<'de, D>(
    deserializer: D,
) -> Result<Option<ProviderAttemptCompatibility>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    let Some(projection) = Option::<ProviderAttemptProjection>::deserialize(deserializer)? else {
        return Ok(None);
    };
    match projection {
        ProviderAttemptProjection::Record(record) => {
            Ok(Some(ProviderAttemptCompatibility::Parsed(record)))
        }
        ProviderAttemptProjection::JsonString(text) => {
            Ok(Some(ProviderAttemptCompatibility::from_legacy_string(text)))
        }
    }
}

fn optional_http_body_failure<'de, D>(deserializer: D) -> Result<Option<HttpBodyFailure>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(
        match Option::<HttpBodyFailureProjection>::deserialize(deserializer)? {
            Some(HttpBodyFailureProjection::Name(value)) => match value.as_str() {
                "timeout" | "Timeout" => Some(HttpBodyFailure::Timeout),
                "read_failed" | "ReadFailed" => Some(HttpBodyFailure::ReadFailed),
                "decode_failed" | "DecodeFailed" => Some(HttpBodyFailure::DecodeFailed),
                _ => None,
            },
            Some(HttpBodyFailureProjection::Ignored(_)) | None => None,
        },
    )
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum HttpBodyFailureProjection {
    Name(String),
    Ignored(serde::de::IgnoredAny),
}

fn optional_u64<'de, D>(deserializer: D) -> Result<Option<u64>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(match Option::<U64Projection>::deserialize(deserializer)? {
        Some(U64Projection::Number(value)) => Some(value),
        Some(U64Projection::String(value)) => value.parse().ok(),
        None => None,
    })
}

fn optional_u32<'de, D>(deserializer: D) -> Result<Option<u32>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(optional_u64(deserializer)?.and_then(|value| u32::try_from(value).ok()))
}

fn optional_u16<'de, D>(deserializer: D) -> Result<Option<u16>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    Ok(optional_u64(deserializer)?.and_then(|value| u16::try_from(value).ok()))
}

fn trace_string_field(value: &ObservationTraceRecord, field: &str) -> Option<String> {
    non_empty_top_string(value, field)
        .or_else(|| {
            value
                .span
                .as_ref()
                .and_then(|span| span_string_field(span, field))
        })
        .or_else(|| {
            value
                .spans
                .iter()
                .rev()
                .find_map(|span| span_string_field(span, field))
        })
}

fn trace_u64_field(value: &ObservationTraceRecord, field: &str) -> Option<u64> {
    top_u64_field(value, field)
        .or_else(|| {
            value
                .span
                .as_ref()
                .and_then(|span| span_u64_field(span, field))
        })
        .or_else(|| {
            value
                .spans
                .iter()
                .rev()
                .find_map(|span| span_u64_field(span, field))
        })
}

fn non_empty_top_string(value: &ObservationTraceRecord, field: &str) -> Option<String> {
    match field {
        "timestamp" => value.timestamp.as_deref(),
        "target" => value.target.as_deref(),
        "event" => value.event.as_deref(),
        "phase" => value.phase.as_deref(),
        "message" => value.message.as_deref(),
        "outcome" => value.outcome.as_deref(),
        "model" => value.model.as_deref(),
        "failure" => value.failure.as_deref(),
        _ => None,
    }
    .filter(|value| !value.is_empty())
    .map(ToString::to_string)
}

fn top_u64_field(value: &ObservationTraceRecord, field: &str) -> Option<u64> {
    match field {
        "duration_ms" => value.duration_ms,
        "request_id" => value.request_id,
        "attempt" => value.attempt.map(u64::from),
        "max_attempts" => value.max_attempts.map(u64::from),
        "status" => value.status.map(u64::from),
        "elapsed_ms" => value.elapsed_ms,
        "backoff_ms" => value.backoff_ms,
        "request_bytes" => value.request_bytes,
        "response_bytes" => value.response_bytes,
        _ => None,
    }
}

fn span_string_field(value: &ObservationTraceSpan, field: &str) -> Option<String> {
    match field {
        "name" => value.name.as_deref(),
        "campaign_id" => value.campaign_id.as_deref(),
        "campaign" => value.campaign.as_deref(),
        "node_id" => value.node_id.as_deref(),
        "branch_id" => value.branch_id.as_deref(),
        "runtime_id" => value.runtime_id.as_deref(),
        "role" => value.role.as_deref(),
        "runtime_phase" => value.runtime_phase.as_deref(),
        _ => None,
    }
    .filter(|value| !value.is_empty())
    .map(ToString::to_string)
}

fn span_u64_field(value: &ObservationTraceSpan, field: &str) -> Option<u64> {
    match field {
        "generation" => value.generation,
        _ => None,
    }
}

fn collect_named_files(root: &Path, name: &str, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_named_files(&path, name, paths);
        } else if path.file_name().and_then(|value| value.to_str()) == Some(name) {
            paths.push(path);
        }
    }
}

fn collect_observation_logs(root: &Path, paths: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
            continue;
        };
        if path.is_file() && name.starts_with("prototype1_observation_") && name.ends_with(".jsonl")
        {
            paths.push(path);
        }
    }
}

fn path_mentions(path: &Path, needle: &str) -> bool {
    path.components().any(|component| {
        component
            .as_os_str()
            .to_str()
            .is_some_and(|part| part == needle)
    })
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

fn file_modified_at(path: &Path) -> Option<String> {
    let modified = fs::metadata(path).ok()?.modified().ok()?;
    Some(DateTime::<Utc>::from(modified).to_rfc3339())
}

fn format_token_count(tokens: u32) -> String {
    if tokens >= 1_000_000 {
        format!("{:.1}m", tokens as f64 / 1_000_000.0)
    } else if tokens >= 1_000 {
        format!("{:.1}k", tokens as f64 / 1_000.0)
    } else {
        tokens.to_string()
    }
}

fn short_id(value: &str) -> &str {
    value.get(..12).unwrap_or(value)
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
    println!("exits on terminal loop state");
    println!("press Ctrl-C to stop early");

    let mut previous =
        collect_prototype1_monitor_snapshot(&prototype_root, identity_path.as_deref())?;
    let mut seen_journal_entries = if command.print_initial {
        0
    } else {
        journal_entry_count(&prototype_root)
    };
    if command.print_initial {
        for (path, entry) in &previous {
            print_prototype1_monitor_event("initial", &prototype_root, path, Some(*entry));
        }
        print_watch_summary(manifest_path, &prototype_root);
        seen_journal_entries = print_new_journal_entries(&prototype_root, seen_journal_entries);
    }

    let mut terminal_seen = terminal_state(manifest_path, &prototype_root, &previous);
    if let Some(terminal) = terminal_seen.as_ref() {
        print_terminal_state(terminal);
        return Ok(());
    }

    loop {
        thread::sleep(interval);
        let current =
            collect_prototype1_monitor_snapshot(&prototype_root, identity_path.as_deref())?;

        let mut semantic_changed = false;
        for (path, entry) in &current {
            match previous.get(path) {
                None => {
                    if !is_transition_journal_path(&prototype_root, path) {
                        print_prototype1_monitor_event(
                            "created",
                            &prototype_root,
                            path,
                            Some(*entry),
                        );
                        semantic_changed |= is_semantic_watch_path(&prototype_root, path);
                    }
                }
                Some(old) if old != entry => {
                    if !is_transition_journal_path(&prototype_root, path) {
                        print_prototype1_monitor_event(
                            "modified",
                            &prototype_root,
                            path,
                            Some(*entry),
                        );
                        semantic_changed |= is_semantic_watch_path(&prototype_root, path);
                    }
                }
                Some(_) => {}
            }
        }
        for path in previous.keys() {
            if !current.contains_key(path) {
                if !is_transition_journal_path(&prototype_root, path) {
                    print_prototype1_monitor_event("removed", &prototype_root, path, None);
                    semantic_changed |= is_semantic_watch_path(&prototype_root, path);
                }
            }
        }

        seen_journal_entries = print_new_journal_entries(&prototype_root, seen_journal_entries);

        if semantic_changed {
            print_watch_summary(manifest_path, &prototype_root);
        }

        let terminal = terminal_state(manifest_path, &prototype_root, &current);
        if terminal_seen.as_ref() != terminal.as_ref() {
            if let Some(terminal) = terminal.as_ref() {
                print_terminal_state(terminal);
                return Ok(());
            }
            terminal_seen = terminal;
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

fn is_semantic_watch_path(prototype_root: &Path, path: &Path) -> bool {
    let Ok(relative) = path.strip_prefix(prototype_root) else {
        return path
            .file_name()
            .and_then(|name| name.to_str())
            .is_some_and(|name| name == "parent_identity.json");
    };
    let parts: Vec<_> = relative
        .components()
        .filter_map(|component| match component {
            std::path::Component::Normal(value) => value.to_str(),
            _ => None,
        })
        .collect();
    matches!(
        parts.as_slice(),
        ["scheduler.json"]
            | ["branches.json"]
            | ["nodes", _, "node.json"]
            | ["nodes", _, "runner-result.json"]
            | ["nodes", _, "successor-ready", _]
            | ["nodes", _, "successor-completion", _]
    )
}

fn is_transition_journal_path(prototype_root: &Path, path: &Path) -> bool {
    path == prototype_root.join("transition-journal.jsonl")
}

fn print_watch_summary(manifest_path: &Path, _prototype_root: &Path) {
    if let Ok(scheduler) =
        load_scheduler_state(manifest_path, OperatorProjectionRead::cli_operator())
    {
        let continuation = scheduler
            .last_continuation_decision
            .as_ref()
            .map(|decision| {
                format!(
                    "{} next_generation={} selected_next_branch_id={}",
                    serde_name(&decision.disposition),
                    decision.next_generation,
                    decision
                        .selected_next_branch_id
                        .as_deref()
                        .unwrap_or("(none)")
                )
            })
            .unwrap_or_else(|| "(none)".to_string());
        println!(
            "  scheduler: nodes={} frontier={} completed={} failed={} continuation={}",
            scheduler.nodes.len(),
            scheduler.frontier_node_ids.len(),
            scheduler.completed_node_ids.len(),
            scheduler.failed_node_ids.len(),
            continuation
        );
    }
}

fn journal_entry_count(prototype_root: &Path) -> usize {
    let journal_path = prototype_root.join("transition-journal.jsonl");
    let journal = PrototypeJournal::new(&journal_path);
    journal
        .load_entries()
        .map(|entries| entries.len())
        .unwrap_or(0)
}

fn print_new_journal_entries(prototype_root: &Path, seen: usize) -> usize {
    let journal_path = prototype_root.join("transition-journal.jsonl");
    let journal = PrototypeJournal::new(&journal_path);
    let Ok(entries) = journal.load_entries() else {
        return seen;
    };
    for entry in entries.iter().skip(seen) {
        print_journal_transition(entry);
    }
    entries.len()
}

fn print_journal_transition(entry: &JournalEntry) {
    println!("journal: {}", journal_entry_summary(entry));
}

fn journal_entry_summary(entry: &JournalEntry) -> String {
    match entry {
        JournalEntry::ParentStarted(entry) => format!(
            "{} campaign={} parent={} generation={} pid={}",
            "parent_started",
            entry.campaign_id,
            entry.parent_identity.parent_id(),
            entry.parent_identity.generation(),
            entry.pid
        ),
        JournalEntry::Resource(entry) => format!(
            "{} subject={:?} phase={:?} parent={} node={} generation={} status={:?} bytes={} path={}",
            "resource",
            entry.subject,
            entry.phase,
            entry.parent_id,
            entry.node_id,
            entry.generation,
            entry.status,
            entry
                .bytes
                .map(|bytes| bytes.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry.path.display()
        ),
        JournalEntry::ChildArtifactCommitted(entry) => format!(
            "{} node={} generation={} branch={} target_commit={} identity_commit={}",
            "child_artifact_committed",
            entry.node_id,
            entry.generation,
            entry.child_branch,
            entry.target_commit,
            entry.identity_commit.as_deref().unwrap_or("none")
        ),
        JournalEntry::ActiveCheckoutAdvanced(entry) => format!(
            "{} selected_parent={} generation={} branch={} commit={}",
            "active_checkout_advanced",
            entry.selected_parent_identity.parent_id(),
            entry.selected_parent_identity.generation(),
            entry.selected_branch,
            entry.installed_commit
        ),
        JournalEntry::SuccessorHandoff(entry) => format!(
            "{} node={} runtime={} pid={} ready={}",
            "successor_handoff",
            entry.node_id,
            entry.runtime_id,
            entry.pid,
            entry.ready_path.display()
        ),
        JournalEntry::Successor(entry) => format!(
            "{} node={} runtime={} state={:?}",
            entry.entry_kind(),
            entry.node_id,
            entry
                .runtime_id
                .map(|runtime_id| runtime_id.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry.state
        ),
        JournalEntry::MaterializeBranch(entry) => format!(
            "{} node={} generation={} transition={} branch={} current={} proposed={}",
            journal_entry_kind(&JournalEntry::MaterializeBranch(entry.clone())),
            entry.refs.node_id,
            entry.generation,
            entry.transition_id,
            entry.refs.branch_id,
            entry.hashes.current,
            entry.hashes.proposed
        ),
        JournalEntry::BuildChild(entry) => format!(
            "{} node={} generation={} transition={} result={}",
            journal_entry_kind(&JournalEntry::BuildChild(entry.clone())),
            entry.refs.node_id,
            entry.generation,
            entry.transition_id,
            entry
                .result
                .as_ref()
                .map(|result| format!("{result:?}"))
                .unwrap_or_else(|| "(pending)".to_string())
        ),
        JournalEntry::SpawnChild(entry) => format!(
            "{} node={} generation={} runtime={} child_pid={} result={}",
            journal_entry_kind(&JournalEntry::SpawnChild(entry.clone())),
            entry.refs.node_id,
            entry.generation,
            entry.runtime_id,
            entry
                .child_pid
                .map(|pid| pid.to_string())
                .unwrap_or_else(|| "-".to_string()),
            entry
                .result
                .as_ref()
                .map(|result| format!("{result:?}"))
                .unwrap_or_else(|| "(pending)".to_string())
        ),
        JournalEntry::Child(entry) => entry.entry_kind().to_string(),
        JournalEntry::ChildReady(entry) => format!(
            "{} node={} generation={} runtime={} pid={}",
            journal_entry_kind(&JournalEntry::ChildReady(entry.clone())),
            entry.refs.node_id,
            entry.generation,
            entry.runtime_id,
            entry.pid
        ),
        JournalEntry::ObserveChild(entry) => format!(
            "{} node={} generation={} runtime={} transition={} result={}",
            journal_entry_kind(&JournalEntry::ObserveChild(entry.clone())),
            entry.refs.node_id,
            entry.generation,
            entry.runtime_id,
            entry.transition_id,
            entry
                .result
                .as_ref()
                .map(|result| format!("{result:?}"))
                .unwrap_or_else(|| "(pending)".to_string())
        ),
    }
}

fn journal_entry_kind(entry: &JournalEntry) -> &'static str {
    match entry {
        JournalEntry::ParentStarted(_) => "parent_started",
        JournalEntry::Resource(_) => "resource",
        JournalEntry::ChildArtifactCommitted(_) => "child_artifact_committed",
        JournalEntry::ActiveCheckoutAdvanced(_) => "active_checkout_advanced",
        JournalEntry::SuccessorHandoff(_) => "successor_handoff",
        JournalEntry::Successor(entry) => entry.entry_kind(),
        JournalEntry::MaterializeBranch(entry) => match entry.phase {
            crate::intervention::CommitPhase::Before => "materialize_branch:before",
            crate::intervention::CommitPhase::After => "materialize_branch:after",
        },
        JournalEntry::BuildChild(entry) => match entry.phase {
            crate::intervention::CommitPhase::Before => "build_child:before",
            crate::intervention::CommitPhase::After => "build_child:after",
        },
        JournalEntry::SpawnChild(entry) => match entry.phase {
            crate::cli::prototype1_state::journal::SpawnPhase::Starting => "spawn_child:starting",
            crate::cli::prototype1_state::journal::SpawnPhase::Spawned => "spawn_child:spawned",
            crate::cli::prototype1_state::journal::SpawnPhase::Observed => "spawn_child:observed",
        },
        JournalEntry::Child(entry) => entry.entry_kind(),
        JournalEntry::ChildReady(_) => "child_ready",
        JournalEntry::ObserveChild(entry) => match entry.phase {
            crate::intervention::CommitPhase::Before => "observe_child:before",
            crate::intervention::CommitPhase::After => "observe_child:after",
        },
    }
}

fn terminal_state(
    manifest_path: &Path,
    prototype_root: &Path,
    snapshot: &BTreeMap<PathBuf, Prototype1MonitorSnapshotEntry>,
) -> Option<TerminalState> {
    let scheduler =
        load_scheduler_state(manifest_path, OperatorProjectionRead::cli_operator()).ok();
    if let Some(scheduler) = scheduler.as_ref() {
        if let Some(decision) = scheduler.last_continuation_decision.as_ref() {
            if !decision.disposition.allows_successor() {
                return Some(TerminalState {
                    reason: "scheduler_stopped",
                    detail: format!(
                        "{} next_generation={} total_nodes_after_continue={} selected_next_branch_id={}",
                        serde_name(&decision.disposition),
                        decision.next_generation,
                        decision.total_nodes_after_continue,
                        decision
                            .selected_next_branch_id
                            .as_deref()
                            .unwrap_or("(none)")
                    ),
                });
            }
        }

        let mut failed = BTreeSet::new();
        failed.extend(scheduler.failed_node_ids.iter().cloned());
        failed.extend(
            scheduler
                .nodes
                .iter()
                .filter(|node| node.status == Prototype1NodeStatus::Failed)
                .map(|node| node.node_id.clone()),
        );
        if !failed.is_empty() {
            return Some(TerminalState {
                reason: "node_failed",
                detail: failed.into_iter().collect::<Vec<_>>().join(", "),
            });
        }
    }

    if let Some(terminal) = successor_completion_state(prototype_root, snapshot) {
        return Some(terminal);
    }

    if let Some(terminal) = stalled_materialization_state(prototype_root, snapshot) {
        return Some(terminal);
    }

    exited_parent_state(prototype_root, snapshot)
}

fn successor_completion_state(
    prototype_root: &Path,
    snapshot: &BTreeMap<PathBuf, Prototype1MonitorSnapshotEntry>,
) -> Option<TerminalState> {
    for path in snapshot.keys() {
        let Ok(relative) = path.strip_prefix(prototype_root) else {
            continue;
        };
        let parts: Vec<_> = relative
            .components()
            .filter_map(|component| match component {
                std::path::Component::Normal(value) => value.to_str(),
                _ => None,
            })
            .collect();
        if !matches!(parts.as_slice(), ["nodes", _, "successor-completion", _]) {
            continue;
        }
        let Ok(text) = fs::read_to_string(path) else {
            continue;
        };
        let Ok(value) = serde_json::from_str::<serde_json::Value>(&text) else {
            continue;
        };
        let status = value
            .get("status")
            .and_then(|status| status.as_str())
            .unwrap_or("(unknown)");
        if status == "failed" {
            return Some(TerminalState {
                reason: "successor_failed",
                detail: format!(
                    "{} status=failed detail={}",
                    prototype1_monitor_display_path(prototype_root, path),
                    value
                        .get("detail")
                        .and_then(|detail| detail.as_str())
                        .unwrap_or("(none)")
                ),
            });
        }
    }
    None
}

fn stalled_materialization_state(
    prototype_root: &Path,
    snapshot: &BTreeMap<PathBuf, Prototype1MonitorSnapshotEntry>,
) -> Option<TerminalState> {
    if !snapshot_quiet_for(snapshot, Duration::from_secs(2)) {
        return None;
    }

    let journal_path = prototype_root.join("transition-journal.jsonl");
    let journal = PrototypeJournal::new(journal_path);
    let Ok(materialize) = journal.replay_materialize_branch() else {
        return None;
    };
    let pending = materialize.iter().find(|entry| {
        matches!(
            entry.outcome,
            crate::cli::prototype1_state::journal::MaterializeBranchOutcome::Pending { .. }
        )
    })?;

    Some(TerminalState {
        reason: "materialize_incomplete",
        detail: format!(
            "node_id={} branch_id={} has materialize_branch:before without after and no monitor file changed for 2000ms",
            pending.before.refs.node_id, pending.before.refs.branch_id
        ),
    })
}

fn exited_parent_state(
    prototype_root: &Path,
    snapshot: &BTreeMap<PathBuf, Prototype1MonitorSnapshotEntry>,
) -> Option<TerminalState> {
    if !snapshot_quiet_for(snapshot, Duration::from_secs(2)) {
        return None;
    }

    let journal_path = prototype_root.join("transition-journal.jsonl");
    let journal = PrototypeJournal::new(journal_path);
    let Ok(entries) = journal.load_entries() else {
        return None;
    };
    let Some((pid, node_id)) = entries.iter().rev().find_map(|entry| match entry {
        JournalEntry::ParentStarted(entry) => {
            Some((entry.pid, entry.parent_identity.node_id().to_string()))
        }
        _ => None,
    }) else {
        return None;
    };

    if pid_alive(pid) {
        return None;
    }

    Some(TerminalState {
        reason: "parent_process_exited",
        detail: format!(
            "latest parent pid={} for node_id={} is no longer visible and no monitor file changed for 2000ms",
            pid, node_id
        ),
    })
}

fn snapshot_quiet_for(
    snapshot: &BTreeMap<PathBuf, Prototype1MonitorSnapshotEntry>,
    duration: Duration,
) -> bool {
    let Some(latest) = snapshot.values().filter_map(|entry| entry.modified).max() else {
        return false;
    };
    SystemTime::now()
        .duration_since(latest)
        .is_ok_and(|elapsed| elapsed >= duration)
}

fn pid_alive(pid: u32) -> bool {
    Path::new("/proc").join(pid.to_string()).exists()
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

fn print_terminal_state(terminal: &TerminalState) {
    let timestamp = Utc::now().to_rfc3339();
    println!(
        "{timestamp} terminal reason={} {}",
        terminal.reason, terminal.detail
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
        ["branches.json"] => "append-only branch registry and parent comparison stream",
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

fn current_dir_as_repo_root() -> Result<PathBuf, PrepareError> {
    std::env::current_dir().map_err(|source| PrepareError::ReadManifest {
        path: PathBuf::from("."),
        source,
    })
}

fn infer_campaign_from_parent_identity(repo_root: &Path) -> Result<Option<String>, PrepareError> {
    load_parent_identity_optional(repo_root)
        .map(|identity| identity.map(|identity| identity.campaign_id().to_string()))
}

fn record_active_prototype1_monitor_target(campaign_id: &str, repo_root: &Path) {
    let target = ActivePrototype1MonitorTarget {
        campaign_id: campaign_id.to_string(),
        repo_root: repo_root.to_path_buf(),
    };
    if let Err(error) = save_active_prototype1_monitor_target(&target) {
        warn!(%campaign_id, repo_root = %repo_root.display(), error = %error, "failed to cache active Prototype 1 monitor target");
    }
}

fn resolve_prototype1_state_campaign(
    command: &Prototype1StateCommand,
    repo_root: &Path,
) -> Result<String, PrepareError> {
    if let Some(campaign) = command.campaign.as_ref() {
        return Ok(campaign.clone());
    }
    if let Some(campaign) = infer_campaign_from_parent_identity(repo_root)? {
        return Ok(campaign);
    }

    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "--campaign was omitted and no parent identity exists at '{}'. Active Prototype 1 execution does not read selection.json; pass --campaign explicitly or run from a checkout with parent identity.",
            crate::cli::prototype1_state::identity::parent_identity_path(repo_root).display()
        ),
    })
}

fn resolve_history_campaign(
    command: &HistoryCommand,
    repo_root: &Path,
) -> Result<String, PrepareError> {
    if let Some(campaign) = command.campaign.as_ref() {
        return Ok(campaign.clone());
    }
    if let Some(campaign) = infer_campaign_from_parent_identity(repo_root)? {
        return Ok(campaign);
    }
    if let Some(campaign) = load_active_selection(OperatorProjectionRead::cli_operator())?.campaign
    {
        return Ok(campaign);
    }
    if let Some(campaign) = latest_prototype1_campaign()? {
        return Ok(campaign);
    }

    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "--campaign was omitted, no parent identity exists at '{}', no active campaign is selected, and no Prototype 1 campaign was found under '{}'. Run `ploke-eval select campaign <campaign>` or pass --campaign.",
            crate::cli::prototype1_state::identity::parent_identity_path(repo_root).display(),
            crate::layout::campaigns_dir()?.display()
        ),
    })
}

fn latest_prototype1_campaign() -> Result<Option<String>, PrepareError> {
    let root = crate::layout::campaigns_dir()?;
    if !root.exists() {
        return Ok(None);
    }

    let mut latest = None::<(SystemTime, String)>;
    for entry in fs::read_dir(&root).map_err(|source| PrepareError::ReadCampaignManifest {
        path: root.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| PrepareError::ReadCampaignManifest {
            path: root.clone(),
            source,
        })?;
        let path = entry.path();
        if !path.is_dir()
            || !path.join("campaign.json").exists()
            || !path.join("prototype1").exists()
        {
            continue;
        }
        let modified = path
            .metadata()
            .and_then(|metadata| metadata.modified())
            .unwrap_or(SystemTime::UNIX_EPOCH);
        let campaign_id = entry.file_name().to_string_lossy().to_string();
        if latest
            .as_ref()
            .map(|(current, _)| modified > *current)
            .unwrap_or(true)
        {
            latest = Some((modified, campaign_id));
        }
    }

    Ok(latest.map(|(_, campaign_id)| campaign_id))
}

fn resolve_prototype1_candidate_node_id(
    command: &Prototype1StateCommand,
    _campaign_id: &str,
    _manifest_path: &Path,
    required_generation: Option<u32>,
    _parent_node_id: Option<&str>,
    purpose: &str,
) -> Result<String, PrepareError> {
    if let Some(node_id) = command.node_id.as_ref() {
        return Ok(node_id.clone());
    }

    let generation_detail = required_generation
        .map(|generation| format!(" generation {generation}"))
        .unwrap_or_default();
    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "could not infer --node-id for {purpose}: active Prototype 1 execution no longer reads scheduler.json, branches.json, or selection.json to discover runnable{generation_detail} nodes; pass --node-id explicitly or use a typed child plan/History-backed path"
        ),
    })
}

fn resolve_initial_parent_node_id(
    command: &Prototype1StateCommand,
    campaign_id: &str,
    manifest_path: &Path,
) -> Result<String, PrepareError> {
    resolve_prototype1_candidate_node_id(
        command,
        campaign_id,
        manifest_path,
        Some(0),
        None,
        "initial parent identity",
    )
}

async fn resolve_child_plan(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    parent: Parent<Ready>,
    candidate_generation: CandidateGenerationConfig,
    selected_node_id: Option<&str>,
    child_budget: Prototype1ChildBudget,
) -> Result<PlannedChildren, PrepareError> {
    let parent_identity = parent.identity().clone();
    info!(
        target: EXECUTION_DEBUG_TARGET,
        role = "parent",
        authority = "parent_broadcast_channel",
        transition = "Parent<Ready>->ChildPlan",
        campaign = %campaign_id,
        parent_node_id = %parent_identity.node_id(),
        generation = parent_identity.generation(),
        "resolving parent child-plan authority"
    );
    // Prototype 1 currently enforces direct-child lineage: Parent k may only
    // materialize candidates produced as generation k + 1.
    let required_generation = parent_identity.generation() + 1;
    let plan_at = crate::cli::prototype1_state::inner::At::<ChildPlanFile>::resolve((
        manifest_path.to_path_buf(),
        parent_identity.node_id().to_string(),
    ));
    let receipt = if plan_at.path().exists() {
        receive_existing_child_plan(campaign_id, manifest_path, repo_root, parent)?
    } else {
        match run_parent_target_selection(
            campaign_id,
            manifest_path,
            repo_root,
            parent,
            candidate_generation,
            child_budget,
        )
        .await?
        {
            ParentTargetSelection::ChildPlan(receipt) => receipt,
            ParentTargetSelection::AwaitingHarnessBatch(batch) => {
                let mut admitted = Vec::with_capacity(batch.slots.len());
                for slot in &batch.slots {
                    if admitted.len() >= batch.child_budget.max as usize {
                        break;
                    }
                    let mut executor = None;
                    if !slot.published.submitted_result_path().exists() {
                        match run_broad_headless_tui_attempt(slot).await {
                            Ok(value) => {
                                executor = value;
                            }
                            Err(source) => {
                                warn!(
                                    target: EXECUTION_DEBUG_TARGET,
                                    request_id = %slot.published.request_id(),
                                    request_hash = %slot.published.request_hash(),
                                    admitted = admitted.len(),
                                    required_min = batch.child_budget.min,
                                    configured_max = batch.child_budget.max,
                                    error = %source,
                                    "broad headless-tui slot attempt did not produce an admissible edit; trying next fresh slot"
                                );
                                continue;
                            }
                        }
                    }
                    match try_admit_request_result(
                        repo_root,
                        &batch.parent,
                        slot,
                        executor.as_ref(),
                    ) {
                        Ok(Some(transaction)) => admitted.push(transaction),
                        Ok(None) => {
                            warn!(
                                target: EXECUTION_DEBUG_TARGET,
                                request_id = %slot.published.request_id(),
                                request_hash = %slot.published.request_hash(),
                                admitted = admitted.len(),
                                required_min = batch.child_budget.min,
                                configured_max = batch.child_budget.max,
                                submitted_result_path = %slot.published.submitted_result_path().display(),
                                "broad headless-tui slot had no submitted result; trying next fresh slot"
                            );
                        }
                        Err(source) => {
                            warn!(
                                target: EXECUTION_DEBUG_TARGET,
                                request_id = %slot.published.request_id(),
                                request_hash = %slot.published.request_hash(),
                                admitted = admitted.len(),
                                required_min = batch.child_budget.min,
                                configured_max = batch.child_budget.max,
                                error = %source,
                                "broad headless-tui slot result failed admission; trying next fresh slot"
                            );
                        }
                    }
                }
                publish_broad_harness_child_plan_from_admitted_batch(
                    campaign_id,
                    manifest_path,
                    repo_root,
                    batch,
                    admitted,
                )?
            }
        }
    };
    let children = receipt
        .plan
        .body()
        .children()
        .iter()
        .cloned()
        .collect::<Vec<_>>();
    candidate_generation.validate_received_child_plan(&children)?;

    let children = if let Some(node_id) = selected_node_id {
        let candidate = children
            .iter()
            .find(|child| child.node_id() == node_id)
            .cloned()
            .ok_or_else(|| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "--node-id '{}' is not present in the received child plan for active parent '{}'",
                    node_id, parent_identity.parent_id()
                ),
            })?;
        let node = candidate.node_record();
        if node.generation != required_generation {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "--node-id '{}' is generation {}, but parent '{}' can only materialize generation {} candidates",
                    node_id,
                    node.generation,
                    parent_identity.parent_id(),
                    required_generation
                ),
            });
        }
        if node.parent_node_id.as_deref() != Some(parent_identity.node_id()) {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "--node-id '{}' is not a child of active parent '{}'",
                    node_id,
                    parent_identity.parent_id()
                ),
            });
        }
        vec![candidate]
    } else {
        children
    };

    for child in &children {
        validate_received_child_plan(&parent_identity, &receipt.plan, child.node_id())?;
    }
    info!(
        target: EXECUTION_DEBUG_TARGET,
        role = "parent",
        authority = "parent_broadcast_channel",
        transition = "ChildPlan->Parent<Selectable>",
        campaign = %campaign_id,
        parent_id = %parent_identity.parent_id(),
        parent_node_id = %parent_identity.node_id(),
        generation = parent_identity.generation(),
        selected_children = children.len(),
        plan_children = receipt.plan.body().children().len(),
        "validated child-plan membership for active parent"
    );
    Ok(PlannedChildren {
        parent: receipt.parent,
        plan: receipt.plan,
        children,
        rejected_surface_attempts: receipt.rejected_surface_attempts,
    })
}

pub(crate) async fn resolve_profile_child_plan(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    parent: Parent<Ready>,
    run_profile: &profile::Prototype1RunProfile,
    child_budget: Prototype1ChildBudget,
) -> Result<PlannedChildren, PrepareError> {
    resolve_child_plan(
        campaign_id,
        manifest_path,
        repo_root,
        parent,
        CandidateGenerationConfig::from_profile_generation(run_profile.generation),
        None,
        child_budget,
    )
    .await
}

pub(crate) fn compare_observed_child_treatment(
    campaign_id: &str,
    manifest_path: &Path,
    parent_baseline: &CompleteBaseline,
    resolved: &crate::intervention::ResolvedTreatmentBranch,
    treatment: &Prototype1TreatmentEvidence,
    branch_log_gate: &Mutex<()>,
) -> Result<Prototype1BranchEvaluationReport, PrepareError> {
    let branch_id = resolved.branch.branch_id.as_str();
    if treatment.baseline_campaign_id != campaign_id || treatment.branch_id != branch_id {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "child treatment evidence does not match active parent comparison: expected campaign={} branch={}, got campaign={} branch={}",
                campaign_id, branch_id, treatment.baseline_campaign_id, treatment.branch_id
            ),
        });
    }
    let branch_registry_path = prototype1_branch_registry_path(manifest_path);
    let evaluation_artifact_path = prototype1_branch_evaluation_path(manifest_path, branch_id);
    let report = build_prototype1_branch_evaluation_report(
        campaign_id,
        branch_id,
        &branch_registry_path,
        &evaluation_artifact_path,
        parent_baseline,
        treatment,
    )?;
    write_json_file_pretty(&evaluation_artifact_path, &report)?;

    let rejected_instances = report
        .compared_instances
        .iter()
        .filter(|row| {
            row.evaluation
                .as_ref()
                .is_some_and(|evaluation| evaluation.disposition == BranchDisposition::Reject)
                || row.status != "compared"
        })
        .count();
    let summary = branch_log::ComparisonSummary {
        baseline_campaign_id: campaign_id.to_string(),
        treatment_campaign_id: report.treatment_campaign_id.clone(),
        compared_instances: report.compared_instances.len(),
        rejected_instances,
        overall_disposition: report.overall_disposition.clone(),
        evaluated_at: Utc::now().to_rfc3339(),
    };
    let _guard = branch_log_gate
        .lock()
        .map_err(|_| PrepareError::InvalidBatchSelection {
            detail: "parent branch comparison log lock was poisoned".to_string(),
        })?;
    branch_log::record_parent_comparison(campaign_id, manifest_path, resolved, summary)?;

    Ok(report)
}

pub(crate) fn run_planned_child(
    campaign_id: String,
    manifest_path: PathBuf,
    repo_root: PathBuf,
    journal_path: PathBuf,
    parent_identity: ParentIdentity,
    parent_baseline: CompleteBaseline,
    branch_log_gate: Arc<Mutex<()>>,
    stop_after: Prototype1StateStopAfter,
    plan_index: usize,
    child: ChildFiles,
) -> Result<PlannedChildOutcome, PrepareError> {
    let mut journal = PrototypeJournal::new(journal_path);
    let node = child.node_record().clone();
    let request = child.runner_request().clone();
    let resolved = child.resolved().clone();
    let surface = child.surface().cloned();
    let harness = child.harness_evidence().cloned();
    let node_id = node.node_id.clone();
    let child_path_span = tracing::info_span!(
        target: EXECUTION_DEBUG_TARGET,
        "prototype1.parent.child_path",
        role = "parent",
        authority = "artifact_backend+child_channel",
        phase = "child_path",
        transition = "Parent<Selectable>->ChildRuntime",
        campaign = %campaign_id,
        child_node_id = %node.node_id,
        generation = node.generation,
        branch_id = %node.branch_id,
        candidate_id = %node.candidate_id,
        plan_index = plan_index,
    );
    let _child_path_entered = child_path_span.enter();
    info!(
        target: EXECUTION_DEBUG_TARGET,
        role = "parent",
        authority = "parent_broadcast_channel",
        transition = "ChildFiles->C1",
        campaign = %campaign_id,
        node_id = %node_id,
        branch_id = %node.branch_id,
        candidate_id = %node.candidate_id,
        generation = node.generation,
        "starting planned child state path from broadcast payload"
    );
    let c1 = C1::from_child_plan(
        campaign_id.clone(),
        manifest_path.clone(),
        node,
        request,
        resolved,
        repo_root.clone(),
    )
    .map_err(|err| {
        prototype1_state_transition_error("prototype1_state_load_c1", err.to_string())
    })?;
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        campaign = %campaign_id,
        candidate_node_id = %node_id,
        "resolved prototype1 candidate for parent turn"
    );

    let materialized = if let Some(evidence) = harness.as_ref() {
        MaterializeBranch::new().transition_with_harness(c1, evidence, &mut journal)
    } else {
        MaterializeBranch::new().transition(c1, &mut journal)
    };
    let c2 = match materialized.map_err(|err| {
        prototype1_state_transition_error("prototype1_state_materialize", format!("{err:?}"))
    })? {
        Outcome::Advanced(next) => {
            info!(
                target: EXECUTION_DEBUG_TARGET,
                role = "parent",
                authority = "artifact_backend",
                transition = "C1->C2",
                campaign = %campaign_id,
                node_id = %node_id,
                branch_id = %next.node().branch_id,
                generation = next.node().generation,
                workspace_root = %next.artifact.repo_root.display(),
                "materialized child Artifact"
            );
            debug!(
                target: EXECUTION_DEBUG_TARGET,
                campaign = %campaign_id,
                candidate_node_id = %node_id,
                workspace_root = %next.artifact.repo_root.display(),
                "materialize completed"
            );
            next
        }
        Outcome::Rejected(never) => match never {},
    };
    let mut report_node = c2.node().clone();
    let mut report_resolved = c2.resolved().clone();

    let mut child_runtime = None;
    let mut evaluation_report = None;
    let mut selection_input = None;
    let mut artifact_surface = harness
        .as_ref()
        .map(|evidence| evidence.artifact_surface().clone());
    let outcome = if stop_after == Prototype1StateStopAfter::Materialize {
        "materialized".to_string()
    } else {
        let _surface = validate_child_surface(
            &repo_root,
            &c2.artifact.repo_root,
            "prototype1_state_child_surface_commitment_before_build",
        )?;
        match BuildChild::new()
            .transition(c2, &mut journal)
            .map_err(|err| {
                prototype1_state_transition_error("prototype1_state_build", format!("{err:?}"))
            })? {
            Outcome::Rejected(rejected) => {
                debug!(
                    target: EXECUTION_DEBUG_TARGET,
                    campaign = %campaign_id,
                    candidate_node_id = %node_id,
                    rejected = ?rejected,
                    "build rejected"
                );
                format!("build_rejected:{rejected:?}")
            }
            Outcome::Advanced(c3) => {
                report_node = c3.node().clone();
                report_resolved = c3.resolved().clone();
                artifact_surface = Some(persist_prototype1_buildable_child_artifact(
                    &campaign_id,
                    &manifest_path,
                    &repo_root,
                    &parent_identity,
                    c3.node(),
                    c3.resolved(),
                )?);
                info!(
                    target: EXECUTION_DEBUG_TARGET,
                    role = "parent",
                    authority = "artifact_backend",
                    transition = "C2->C3",
                    campaign = %campaign_id,
                    node_id = %node_id,
                    branch_id = %c3.node().branch_id,
                    generation = c3.node().generation,
                    binary_path = %c3.binary.child_path.display(),
                    "compiled child Runtime binary"
                );
                debug!(
                    target: EXECUTION_DEBUG_TARGET,
                    campaign = %campaign_id,
                    candidate_node_id = %node_id,
                    binary_path = %c3.binary.child_path.display(),
                    "build completed"
                );
                if stop_after == Prototype1StateStopAfter::Build {
                    "built".to_string()
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
                                campaign = %campaign_id,
                                candidate_node_id = %node_id,
                                rejected = ?rejected,
                                "spawn rejected"
                            );
                            format!("spawn_rejected:{rejected:?}")
                        }
                        Outcome::Advanced(c4) => {
                            report_node = c4.node().clone();
                            report_resolved = c4.resolved().clone();
                            child_runtime = c4.binary.child_runtime.map(
                                |id: crate::cli::prototype1_state::event::RuntimeId| id.to_string(),
                            );
                            info!(
                                target: EXECUTION_DEBUG_TARGET,
                                role = "parent",
                                authority = "child_runtime_channel",
                                transition = "C3->C4",
                                campaign = %campaign_id,
                                node_id = %node_id,
                                branch_id = %c4.node().branch_id,
                                generation = c4.node().generation,
                                child_runtime = ?child_runtime,
                                "spawned child Runtime and observed ready acknowledgement"
                            );
                            debug!(
                                target: EXECUTION_DEBUG_TARGET,
                                campaign = %campaign_id,
                                candidate_node_id = %node_id,
                                child_runtime = ?child_runtime,
                                "spawn completed"
                            );
                            if stop_after == Prototype1StateStopAfter::Spawn {
                                "spawned".to_string()
                            } else {
                                match ObserveChild::new().transition(c4, &mut journal).map_err(
                                    |err| {
                                        prototype1_state_transition_error(
                                            "prototype1_state_complete",
                                            format!("{err:?}"),
                                        )
                                    },
                                )? {
                                    Outcome::Rejected(rejected) => {
                                        debug!(
                                            target: EXECUTION_DEBUG_TARGET,
                                            campaign = %campaign_id,
                                            candidate_node_id = %node_id,
                                            rejected = ?rejected,
                                            "child completion rejected"
                                        );
                                        format!("completion_rejected:{rejected:?}")
                                    }
                                    Outcome::Advanced(c5) => {
                                        report_node = c5.base.node().clone();
                                        report_resolved = c5.base.resolved().clone();
                                        if let ObservedChild::Succeeded(successful) = &c5.observed {
                                            let report = compare_observed_child_treatment(
                                                &campaign_id,
                                                &manifest_path,
                                                &parent_baseline,
                                                c5.base.resolved(),
                                                &successful.treatment,
                                                &branch_log_gate,
                                            )?;
                                            info!(
                                                target: EXECUTION_DEBUG_TARGET,
                                                role = "parent",
                                                authority = "parent_baseline+treatment_evidence",
                                                transition = "C5->ParentCompared",
                                                campaign = %campaign_id,
                                                node_id = %node_id,
                                                branch_id = %c5.base.node().branch_id,
                                                generation = c5.base.node().generation,
                                                disposition = ?report.overall_disposition,
                                                "compared child treatment evidence against parent baseline"
                                            );
                                            debug!(
                                                target: EXECUTION_DEBUG_TARGET,
                                                campaign = %campaign_id,
                                                candidate_node_id = %node_id,
                                                disposition = ?report.overall_disposition,
                                                "parent comparison completed"
                                            );
                                            evaluation_report = Some(report.clone());
                                            selection_input =
                                                Some(selection_input_from_child_report(
                                                    c5.base.node(),
                                                    &report,
                                                ));
                                            format!("completed:{:?}", report.overall_disposition)
                                        } else {
                                            info!(
                                                target: EXECUTION_DEBUG_TARGET,
                                                role = "parent",
                                                authority = "child_runtime_channel",
                                                transition = "C4->C5",
                                                campaign = %campaign_id,
                                                node_id = %node_id,
                                                branch_id = %c5.base.node().branch_id,
                                                generation = c5.base.node().generation,
                                                "observed failed child terminal result through runtime channel"
                                            );
                                            "completed:Reject".to_string()
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
    if stop_after == Prototype1StateStopAfter::Complete {
        cleanup_prototype1_child_build_products(&manifest_path, &campaign_id, &report_node)?;
    }

    Ok(PlannedChildOutcome {
        plan_index,
        node: report_node.clone(),
        node_id,
        outcome,
        node_status: report_node.status,
        workspace_root: report_node.workspace_root,
        binary_path: report_node.binary_path,
        resolved: report_resolved,
        child_runtime,
        evaluation_report,
        selection_input,
        surface,
        artifact_surface,
        // Rejected attempts from proposal validation are tracked separately
        // and projected as payload-only candidates.
    })
}

async fn run_child_fanout(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    journal_path: &Path,
    parent_identity: &ParentIdentity,
    parent_baseline: &CompleteBaseline,
    stop_after: Prototype1StateStopAfter,
    child_schedule_mode: Prototype1ChildScheduleMode,
    child_budget: Prototype1ChildBudget,
    plan_index_offset: usize,
    children: Vec<ChildFiles>,
) -> Result<Vec<PlannedChildOutcome>, PrepareError> {
    if children.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "child plan contained no runnable child nodes".to_string(),
        });
    }

    let fanout_width = child_schedule_mode.fanout_width(child_budget, children.len());
    info!(
        target: EXECUTION_DEBUG_TARGET,
        role = "parent",
        authority = "parent_scheduler_policy",
        transition = "ChildPlan->ChildFanout",
        campaign = %campaign_id,
        planned_children = children.len(),
        schedule_mode = %serde_name(&child_schedule_mode),
        fanout_width = fanout_width,
        plan_index_offset = plan_index_offset,
        budget_min = child_budget.min,
        budget_max = child_budget.max,
        stop_after = ?stop_after,
        "starting parent child fanout"
    );
    if children.len() < child_budget.min as usize {
        warn!(
            target: EXECUTION_DEBUG_TARGET,
            campaign = %campaign_id,
            planned_children = children.len(),
            configured_min_children = child_budget.min,
            "child plan has fewer nodes than configured fanout width"
        );
    }

    let mut completed = Vec::new();
    let mut next = 0usize;
    let branch_log_gate = Arc::new(Mutex::new(()));
    while next < children.len() {
        let end = usize::min(next + fanout_width, children.len());
        let mut join_set = tokio::task::JoinSet::new();
        for (offset, child) in children[next..end].iter().cloned().enumerate() {
            let plan_index = plan_index_offset + next + offset;
            let campaign_id = campaign_id.to_string();
            let manifest_path = manifest_path.to_path_buf();
            let repo_root = repo_root.to_path_buf();
            let journal_path = journal_path.to_path_buf();
            let parent_identity = parent_identity.clone();
            let parent_baseline = parent_baseline.clone();
            let branch_log_gate = branch_log_gate.clone();
            join_set.spawn_blocking(move || {
                run_planned_child(
                    campaign_id,
                    manifest_path,
                    repo_root,
                    journal_path,
                    parent_identity,
                    parent_baseline,
                    branch_log_gate,
                    stop_after,
                    plan_index,
                    child,
                )
            });
        }

        let mut first_error = None;
        while let Some(result) = join_set.join_next().await {
            match result {
                Ok(Ok(outcome)) => completed.push(outcome),
                Ok(Err(err)) => {
                    error!(
                        target: EXECUTION_DEBUG_TARGET,
                        error = %err,
                        "planned child task failed"
                    );
                    if first_error.is_none() {
                        first_error = Some(err);
                    }
                }
                Err(err) => {
                    if first_error.is_none() {
                        first_error = Some(PrepareError::InvalidBatchSelection {
                            detail: format!("planned child task failed to join: {err}"),
                        });
                    }
                }
            }
        }
        if let Some(err) = first_error {
            return Err(err);
        }

        completed.sort_by_key(|outcome| outcome.plan_index);
        if stop_after != Prototype1StateStopAfter::Complete {
            break;
        }
        next = end;
    }

    completed.sort_by_key(|outcome| outcome.plan_index);
    info!(
        target: EXECUTION_DEBUG_TARGET,
        role = "parent",
        authority = "child_runtime_channel",
        transition = "ChildFanout->ChildOutcomes",
        campaign = %campaign_id,
        completed_children = completed.len(),
        "completed parent child fanout"
    );
    Ok(completed)
}

async fn run_adaptive_child_fanout(
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
    journal_path: &Path,
    parent_identity: &ParentIdentity,
    parent_baseline: &CompleteBaseline,
    child_budget: Prototype1ChildBudget,
    children: Vec<ChildFiles>,
    rejected_surface_attempts: &[surface_attempt::Evidence],
    selection_seed: u64,
    selection_strategy: ActiveSelectionStrategy,
) -> Result<
    (
        Vec<PlannedChildOutcome>,
        Option<(SuccessorDecision, SelectionSealMaterial)>,
    ),
    PrepareError,
> {
    if children.is_empty() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "child plan contained no runnable child nodes".to_string(),
        });
    }

    let fanout_width =
        Prototype1ChildScheduleMode::AdaptiveBatch.fanout_width(child_budget, children.len());
    let mut completed = Vec::new();
    let mut selection = None;
    let mut next = 0usize;
    while next < children.len() {
        let end = usize::min(next + fanout_width, children.len());
        let batch = children[next..end].to_vec();
        let mut batch_outcomes = run_child_fanout(
            campaign_id,
            manifest_path,
            repo_root,
            journal_path,
            parent_identity,
            parent_baseline,
            Prototype1StateStopAfter::Complete,
            Prototype1ChildScheduleMode::FullBatch,
            child_budget,
            next,
            batch,
        )
        .await?;
        completed.append(&mut batch_outcomes);
        completed.sort_by_key(|outcome| outcome.plan_index);

        let parent_selection = ParentSelection::new(
            manifest_path,
            parent_identity,
            &completed,
            rejected_surface_attempts,
        );
        selection = parent_selection.select_successor(selection_seed, selection_strategy)?;
        if adaptive_selection_accepts_successor(&selection) {
            break;
        }
        next = end;
    }

    Ok((completed, selection))
}

fn adaptive_selection_accepts_successor(
    selection: &Option<(SuccessorDecision, SelectionSealMaterial)>,
) -> bool {
    selection.as_ref().is_some_and(|(decision, _)| {
        matches!(
            decision.outcome,
            SuccessorOutcome::Accepted | SuccessorOutcome::ExploreFrom
        )
    })
}

pub(crate) fn live_successor_continuation_decision(
    campaign_manifest_path: &Path,
    parent_identity: &ParentIdentity,
    policy: &Prototype1SearchPolicy,
    decision: &SuccessorDecision,
    material: &SelectionSealMaterial,
    selected_node: &Prototype1NodeRecord,
) -> Result<Prototype1ContinuationDecision, PrepareError> {
    let total_nodes_after_continue = persisted_prototype1_node_count(campaign_manifest_path)?;
    let traversal = historical_traversal_guard(campaign_manifest_path)?;
    let selected_rejected = decision
        .selected_branch_disposition()
        .is_some_and(|value| value != "keep");
    let explore_from_rejected = policy.explore_from_rejected && selected_rejected;
    let expected_generation = parent_identity.generation().saturating_add(1);
    let direct_child = material.selected_from_generation_outcomes
        && selected_node.parent_node_id.as_deref() == Some(parent_identity.node_id())
        && selected_node.generation == expected_generation;

    let disposition = if decision.selected_branch_id.is_none() {
        Prototype1ContinuationDisposition::StopNoSelectedBranch
    } else if !material.selected_from_generation_outcomes {
        if traversal.parent_turns_started >= policy.max_generations.saturating_add(1) {
            Prototype1ContinuationDisposition::StopHistoricalTraversalBudget
        } else if policy.require_keep_for_continuation
            && selected_rejected
            && !explore_from_rejected
        {
            Prototype1ContinuationDisposition::StopSelectedBranchRejected
        } else if policy.stop_on_first_keep
            && decision
                .selected_branch_disposition()
                .is_some_and(|value| value == "keep")
        {
            Prototype1ContinuationDisposition::StopOnFirstKeepSatisfied
        } else if selected_node.generation > policy.max_generations {
            Prototype1ContinuationDisposition::StopMaxGenerations
        } else if total_nodes_after_continue >= policy.max_total_nodes {
            Prototype1ContinuationDisposition::StopMaxTotalNodes
        } else if explore_from_rejected {
            Prototype1ContinuationDisposition::ContinueExploreFromRejected
        } else {
            Prototype1ContinuationDisposition::ContinueHistoricalTraversal
        }
    } else if !direct_child {
        Prototype1ContinuationDisposition::StopNonDirectChildSelection
    } else if policy.require_keep_for_continuation && selected_rejected && !explore_from_rejected {
        Prototype1ContinuationDisposition::StopSelectedBranchRejected
    } else if policy.stop_on_first_keep
        && decision
            .selected_branch_disposition()
            .is_some_and(|value| value == "keep")
    {
        Prototype1ContinuationDisposition::StopOnFirstKeepSatisfied
    } else if selected_node.generation > policy.max_generations {
        Prototype1ContinuationDisposition::StopMaxGenerations
    } else if total_nodes_after_continue >= policy.max_total_nodes {
        Prototype1ContinuationDisposition::StopMaxTotalNodes
    } else if explore_from_rejected {
        Prototype1ContinuationDisposition::ContinueExploreFromRejected
    } else {
        Prototype1ContinuationDisposition::ContinueReady
    };

    Ok(Prototype1ContinuationDecision {
        disposition,
        selected_next_branch_id: decision.selected_branch_id.clone(),
        selected_branch_disposition: decision
            .selected_branch_disposition()
            .map(ToOwned::to_owned),
        next_generation: selected_node.generation,
        total_nodes_after_continue,
    })
}

struct HistoricalTraversalGuard {
    parent_turns_started: u32,
}

fn historical_traversal_guard(
    campaign_manifest_path: &Path,
) -> Result<HistoricalTraversalGuard, PrepareError> {
    let journal = PrototypeJournal::new(prototype1_transition_journal_path(campaign_manifest_path));
    let entries = journal.load_entries().map_err(|err| {
        prototype1_state_transition_error("prototype1_history_traversal_guard", err.to_string())
    })?;
    let mut parent_turns_started = 0u32;
    for entry in entries {
        match entry {
            JournalEntry::ParentStarted(entry) => {
                parent_turns_started = parent_turns_started.saturating_add(1);
            }
            _ => {}
        }
    }
    if parent_turns_started == 0 {
        parent_turns_started = 1;
    }
    Ok(HistoricalTraversalGuard {
        parent_turns_started,
    })
}

fn persisted_prototype1_node_count(campaign_manifest_path: &Path) -> Result<u32, PrepareError> {
    let nodes_dir = prototype1_nodes_dir(campaign_manifest_path);
    let entries = match fs::read_dir(&nodes_dir) {
        Ok(entries) => entries,
        Err(source) if source.kind() == io::ErrorKind::NotFound => return Ok(0),
        Err(source) => {
            return Err(PrepareError::ReadManifest {
                path: nodes_dir,
                source,
            });
        }
    };
    let mut count = 0u32;
    for entry in entries {
        let entry = entry.map_err(|source| PrepareError::ReadManifest {
            path: nodes_dir.clone(),
            source,
        })?;
        if !entry
            .file_type()
            .map_err(|source| PrepareError::ReadManifest {
                path: entry.path(),
                source,
            })?
            .is_dir()
        {
            continue;
        }
        let node_id = entry.file_name().to_string_lossy().into_owned();
        let record_path = entry.path().join("node.json");
        if !record_path.exists() {
            continue;
        }
        load_node_record(
            campaign_manifest_path,
            &node_id,
            OperatorProjectionRead::cli_operator(),
        )?;
        count = count.saturating_add(1);
    }
    Ok(count)
}

fn reserve_complete_child_budget(
    policy: &Prototype1SearchPolicy,
    current_node_count: u32,
) -> Result<Prototype1ChildBudget, PrepareError> {
    if current_node_count >= policy.max_total_nodes {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1 hard stop before child planning: persisted node count {} has reached max_total_nodes {}",
                current_node_count, policy.max_total_nodes
            ),
        });
    }
    let remaining_node_slots = policy.max_total_nodes.saturating_sub(current_node_count);
    if remaining_node_slots < policy.child_budget.min {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "prototype1 hard stop before child planning: remaining node slots {} are below required min children {}",
                remaining_node_slots, policy.child_budget.min
            ),
        });
    }
    Ok(Prototype1ChildBudget {
        min: policy.child_budget.min,
        max: policy.child_budget.max.min(remaining_node_slots),
    })
}

pub(crate) fn reserve_profile_child_budget(
    policy: &Prototype1SearchPolicy,
    current_node_count: u32,
) -> Result<Prototype1ChildBudget, PrepareError> {
    reserve_complete_child_budget(policy, current_node_count)
}

struct GenerationCandidateProjection {
    considered: Vec<EvaluationPayload>,
    projection_failures: Vec<SelectionProjectionFailure>,
}

struct ParentSelection<'a> {
    manifest_path: &'a Path,
    parent_identity: &'a ParentIdentity,
    child_outcomes: &'a [PlannedChildOutcome],
    rejected_surface_attempts: &'a [surface_attempt::Evidence],
}

#[derive(Debug, Clone, Copy)]
enum SelectionCandidateScope {
    CurrentGeneration,
    AllAdmittedHistory,
}

#[derive(Debug, Clone, Copy)]
struct ActiveSelectionStrategy {
    candidate_scope: SelectionCandidateScope,
    traversal: StrategyKind,
    metrics_policy: crate::successor_selection::metrics::Policy,
}

impl Prototype1SuccessorSelection {
    fn active_strategy(
        self,
        metrics: crate::metric::Inputs,
        oracle: crate::successor_selection::OracleMode,
        require_evidence: bool,
        metrics_policy: crate::successor_selection::metrics::Policy,
    ) -> ActiveSelectionStrategy {
        match self {
            Prototype1SuccessorSelection::GenerationLocal => ActiveSelectionStrategy {
                candidate_scope: SelectionCandidateScope::CurrentGeneration,
                traversal: StrategyKind::score_child_prop()
                    .with_metrics(metrics)
                    .with_oracle_policy(oracle, require_evidence),
                metrics_policy,
            },
            Prototype1SuccessorSelection::HistoryFrontierMax => ActiveSelectionStrategy {
                candidate_scope: SelectionCandidateScope::AllAdmittedHistory,
                traversal: StrategyKind::default()
                    .with_metrics(metrics)
                    .with_oracle_policy(oracle, require_evidence),
                metrics_policy,
            },
            Prototype1SuccessorSelection::HistoryScoreChildProp => ActiveSelectionStrategy {
                candidate_scope: SelectionCandidateScope::AllAdmittedHistory,
                traversal: StrategyKind::score_child_prop()
                    .with_metrics(metrics)
                    .with_oracle_policy(oracle, require_evidence),
                metrics_policy,
            },
        }
    }
}

pub(crate) fn select_successor_for_profile(
    manifest_path: &Path,
    parent_identity: &ParentIdentity,
    child_outcomes: &[PlannedChildOutcome],
    rejected_surface_attempts: &[surface_attempt::Evidence],
    run_profile: &profile::Prototype1RunProfile,
) -> Result<Option<(SuccessorDecision, SelectionSealMaterial)>, PrepareError> {
    let metric_inputs = traversal_metric_inputs(run_profile.selection.traversal_metrics());
    let strategy = run_profile.selection.successor_selection().active_strategy(
        metric_inputs,
        run_profile.selection.oracle_mode(),
        run_profile.selection.oracle_require_evidence(),
        run_profile.selection.metrics_policy(),
    );
    ParentSelection::new(
        manifest_path,
        parent_identity,
        child_outcomes,
        rejected_surface_attempts,
    )
    .select_successor(run_profile.selection.seed, strategy)
}

#[instrument(
    target = EXECUTION_DEBUG_TARGET,
    level = "debug",
    skip(outcome),
    fields(
        evidence_boundary = "child_outcome_channel",
        node_id = %outcome.node_id,
        branch_id = %outcome.node.branch_id,
        generation = outcome.node.generation,
        plan_index = outcome.plan_index,
        runtime_id = ?outcome.child_runtime,
        has_selection_input = outcome.selection_input.is_some(),
        has_evaluation_report = outcome.evaluation_report.is_some(),
    )
)]
fn current_generation_candidate_evidence(
    outcome: &PlannedChildOutcome,
) -> Result<SealedCandidateEvidence, PrepareError> {
    let mut child_diagnostics = Vec::new();
    if outcome.selection_input.is_some() && outcome.evaluation_report.is_none() {
        child_diagnostics.push(
            "current_generation_candidate: selection input exists without parent comparison report"
                .to_string(),
        );
    }

    let evidence = SealedCandidateEvidence {
        schema_version: 3,
        coordinate: CandidateCoordinate {
            node_id: outcome.node_id.clone(),
            parent_node_id: outcome.node.parent_node_id.clone(),
            branch_id: Some(outcome.node.branch_id.clone()),
            generation: Some(outcome.node.generation),
            plan_index: Some(outcome.plan_index as u32),
            primary_runtime_id: outcome.child_runtime.clone(),
        },
        lifecycle: CandidateLifecycle {
            planner_outcome: outcome.outcome.clone(),
            node_status: format!("{:?}", outcome.node_status),
        },
        evaluations: outcome
            .evaluation_report
            .as_ref()
            .map(current_generation_evaluation_evidence)
            .transpose()?
            .into_iter()
            .collect(),
        runtimes: outcome
            .child_runtime
            .as_ref()
            .map(|runtime_id| SealedRuntimeEvidence {
                runtime_id: runtime_id.clone(),
                document_citations: Vec::new(),
                journal_citations: Vec::new(),
            })
            .into_iter()
            .collect(),
        branches: vec![SealedBranchEvidence {
            branch_id: outcome.resolved.branch.branch_id.clone(),
            candidate_id: Some(outcome.resolved.branch.candidate_id.clone()),
            source_state_id: Some(outcome.resolved.source_state_id.clone()),
            branch_evidence_citations: Vec::new(),
        }],
        extra_document_citations: Vec::new(),
        extra_journal_citations: Vec::new(),
        child_diagnostics,
    };
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        evidence_boundary = "child_outcome_channel",
        node_id = %evidence.coordinate.node_id,
        branch_id = ?evidence.coordinate.branch_id,
        generation = ?evidence.coordinate.generation,
        runtime_id = ?evidence.coordinate.primary_runtime_id,
        evaluation_count = evidence.evaluations.len(),
        branch_count = evidence.branches.len(),
        "sealed current-generation candidate evidence from typed child outcome"
    );
    Ok(evidence)
}

#[instrument(
    target = EXECUTION_DEBUG_TARGET,
    level = "debug",
    skip(report),
    fields(
        evidence_boundary = "parent_compared_treatment_evidence",
        branch_id = %report.branch_id,
        treatment_campaign_id = %report.treatment_campaign_id,
        compared_instances = report.compared_instances.len(),
    )
)]
fn current_generation_evaluation_evidence(
    report: &Prototype1BranchEvaluationReport,
) -> Result<SealedEvaluationEvidence, PrepareError> {
    let report_hash = HistoryHash::of_domain_json(
        "prototype1.history.current_generation_evaluation_report.v1",
        report,
    )
    .map_err(|err| PrepareError::InvalidBatchSelection {
        detail: format!("failed to hash current-generation evaluation report: {err}"),
    })?;
    let evaluator_identity = report
        .evaluator_identity
        .as_ref()
        .map(seal_evaluator_identity_for_history);
    let eval_set_identity = report
        .eval_set_identity
        .as_ref()
        .map(seal_eval_set_identity_for_history);

    Ok(SealedEvaluationEvidence {
        branch_id: report.branch_id.clone(),
        evaluation_procedure_id: report.evaluation_procedure_id.clone(),
        evaluator_identity,
        eval_set_identity,
        evaluation_artifact_citation: Some(SealedEvidenceCitation {
            ref_id: format!(
                "opaque_evaluation_artifact:{}",
                report.evaluation_artifact_path.display()
            ),
            content_hash: None,
            record_name: None,
        }),
        overall_disposition: Some(serde_name(&report.overall_disposition).to_string()),
        primary_report_citation: SealedEvidenceCitation {
            ref_id: format!(
                "inline:child-channel:evaluation-report:{}",
                report.branch_id
            ),
            content_hash: Some(report_hash),
            record_name: Some("prototype1_branch_evaluation_report".to_string()),
        },
        compared_runs: report
            .compared_instances
            .iter()
            .map(current_generation_compared_run_evidence)
            .collect(),
    })
}

fn seal_evaluator_identity_for_history(
    identity: &Prototype1EvaluatorIdentity,
) -> SealedEvaluatorIdentity {
    SealedEvaluatorIdentity {
        id: identity.id.clone(),
        version: identity.version.clone(),
    }
}

fn seal_eval_set_identity_for_history(
    identity: &Prototype1EvalSetIdentity,
) -> SealedEvalSetIdentity {
    SealedEvalSetIdentity {
        id: identity.id.clone(),
        kind: identity.kind.clone(),
        authority: identity.authority.clone(),
        explicit: identity.explicit,
        benchmark_family: Some(serde_name(&identity.benchmark_family)),
        dataset_source_count: identity.dataset_sources.len(),
        instance_ids: identity.instance_ids.clone(),
        missing_treatment_instance_ids: identity.missing_treatment_instance_ids.clone(),
        note: identity.note.clone(),
    }
}

fn current_generation_compared_run_evidence(
    row: &Prototype1ComparedInstanceReport,
) -> SealedComparedRunEvidence {
    let mut diagnostics = Vec::new();
    let baseline_protocol = current_generation_protocol(
        "baseline",
        row.baseline_record_path.as_deref(),
        &mut diagnostics,
    );
    let treatment_protocol = current_generation_protocol(
        "treatment",
        row.treatment_record_path.as_deref(),
        &mut diagnostics,
    );
    SealedComparedRunEvidence {
        instance_id: Some(row.instance_id.clone()),
        status: Some(row.status.clone()),
        baseline_citation: row.baseline_registration_path.as_ref().map(|path| {
            SealedEvidenceCitation {
                ref_id: format!("opaque_registration:{}", path.display()),
                content_hash: None,
                record_name: Some("run_registration".to_string()),
            }
        }),
        treatment_citation: row.treatment_registration_path.as_ref().map(|path| {
            SealedEvidenceCitation {
                ref_id: format!("opaque_registration:{}", path.display()),
                content_hash: None,
                record_name: Some("run_registration".to_string()),
            }
        }),
        baseline_metrics: row.baseline_metrics.clone(),
        treatment_metrics: row.treatment_metrics.clone(),
        baseline_protocol,
        treatment_protocol,
        oracle_evaluation: row.oracle_evaluation.clone(),
        diagnostics,
        baseline_run: None,
        treatment_run: None,
    }
}

fn current_generation_protocol(
    arm: &'static str,
    record_path: Option<&Path>,
    diagnostics: &mut Vec<String>,
) -> Option<crate::metric::Protocol> {
    let Some(record_path) = record_path else {
        diagnostics.push(format!("{arm}_protocol:record_path_missing"));
        return None;
    };
    match load_protocol_aggregate(record_path) {
        Ok(aggregate) => Some(crate::metric::Protocol::from(&aggregate)),
        Err(error) => {
            diagnostics.push(format!("{arm}_protocol:aggregate_unavailable:{error}"));
            None
        }
    }
}

impl<'a> ParentSelection<'a> {
    fn new(
        manifest_path: &'a Path,
        parent_identity: &'a ParentIdentity,
        child_outcomes: &'a [PlannedChildOutcome],
        rejected_surface_attempts: &'a [surface_attempt::Evidence],
    ) -> Self {
        Self {
            manifest_path,
            parent_identity,
            child_outcomes,
            rejected_surface_attempts,
        }
    }

    #[instrument(
        target = EXECUTION_DEBUG_TARGET,
        level = "debug",
        skip(self),
        fields(
            evidence_boundary = "child_outcome_channel",
            parent_node_id = %self.parent_identity.node_id(),
            generation = self.parent_identity.generation() + 1,
            child_count = self.child_outcomes.len(),
        )
    )]
    fn current_generation_candidates(&self) -> Result<GenerationCandidateProjection, PrepareError> {
        let mut projection_failures = Vec::new();
        let mut considered = Vec::new();
        for outcome in self.child_outcomes {
            let candidate = SubjectRef::new(format!(
                "candidate:{}:plan_index={}",
                outcome.node_id, outcome.plan_index
            ));
            let procedure = ProcedureRef::new(crate::successor_selection::PROCEDURE_ID);
            let sealed_body = current_generation_candidate_evidence(outcome)?;
            let artifact = candidate_artifact_from_outcome(outcome)?;
            let mut builder = EvaluationPayload::builder(candidate.clone(), procedure)
                .sealed_candidate_evidence(sealed_body)
                .candidate_artifact(artifact);
            if let Some(surface) = outcome.surface.as_ref() {
                builder = builder.surface_attempt_evidence(surface_attempt_from_surface(surface));
            }

            if let Some(input) = outcome.selection_input.as_ref() {
                builder = builder.selection_input(input.clone()).map_err(|err| {
                    PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "failed to build selection input payload for node_id={}: {err}",
                            outcome.node_id
                        ),
                    }
                })?;
            } else {
                let failure = SelectionProjectionFailure::committed(
                    SelectionProjectionFailureKind::MissingSelectionInput,
                    Some(candidate.clone()),
                    Some(format!(
                        "missing_selection_input: child_fanout_outcome={:?}",
                        outcome.outcome
                    )),
                )
                .map_err(|err| PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "failed to commit selection projection failure id for node_id={}: {err}",
                        outcome.node_id
                    ),
                })?;
                projection_failures.push(failure.clone());
                builder = builder.projection_failure(failure);
            }

            considered.push(builder.build());
        }
        for (index, surface_attempt) in self.rejected_surface_attempts.iter().enumerate() {
            let candidate = SubjectRef::new(format!(
                "candidate:rejected_surface_attempt:{}:proposal_id={}",
                index + 1,
                surface_attempt.proposal_id
            ));
            let procedure = ProcedureRef::new(crate::successor_selection::PROCEDURE_ID);
            let failure = SelectionProjectionFailure::committed(
                SelectionProjectionFailureKind::MissingSelectionInput,
                Some(candidate.clone()),
                Some("missing_selection_input: rejected_surface_attempt_without_child_runtime".to_string()),
            )
            .map_err(|err| PrepareError::InvalidBatchSelection {
                detail: format!(
                    "failed to commit selection projection failure id for rejected surface attempt proposal_id={}: {err}",
                    surface_attempt.proposal_id
                ),
            })?;
            projection_failures.push(failure.clone());
            let payload = EvaluationPayload::builder(candidate, procedure)
                .surface_attempt_evidence(surface_attempt.clone())
                .projection_failure(failure)
                .build();
            considered.push(payload);
        }

        debug!(
            target: EXECUTION_DEBUG_TARGET,
            evidence_boundary = "child_outcome_channel",
            parent_node_id = %self.parent_identity.node_id(),
            considered_count = considered.len(),
            projection_failure_count = projection_failures.len(),
            "assembled current-generation selection payloads from typed child outcomes"
        );
        Ok(GenerationCandidateProjection {
            considered,
            projection_failures,
        })
    }

    fn select_successor(
        &self,
        seed: u64,
        strategy: ActiveSelectionStrategy,
    ) -> Result<Option<(SuccessorDecision, SelectionSealMaterial)>, PrepareError> {
        let current_scope =
            <Self as ScopeFor<Generation>>::scope_for(self, self.parent_identity.generation() + 1)
                .into_selection_scope();
        let scope = match strategy.candidate_scope {
            SelectionCandidateScope::CurrentGeneration => current_scope.clone(),
            SelectionCandidateScope::AllAdmittedHistory => {
                SelectionScope::all_admitted_candidates()
            }
        };
        let history = History::for_campaign_manifest(self.manifest_path);
        let candidates =
            history
                .candidates(&scope)
                .map_err(|err| PrepareError::InvalidBatchSelection {
                    detail: format!("failed to load History traversal candidates: {err}"),
                })?;
        let current = self.current_generation_candidates()?;
        let traversal_candidates = traversal_selection::Candidates::from_history(candidates)
            .with_current_generation(current_scope, current.considered)
            .map_err(|err| PrepareError::InvalidBatchSelection {
                detail: format!("failed to add current generation traversal candidates: {err}"),
            })?;
        let Some(selection) = traversal_selection::select_with_policy(
            traversal_candidates,
            seed,
            strategy.traversal,
            strategy.metrics_policy,
        )
        .map_err(|err| PrepareError::InvalidBatchSelection {
            detail: format!("failed to decide History traversal successor: {err}"),
        })?
        else {
            return Ok(None);
        };
        let selected_occurrence_id = selection.selected_occurrence_id();
        let selected_membership_id = selection.selected_membership_id();
        let mut projection_failures = current.projection_failures;
        projection_failures.extend(selection.projection_failures);
        let material = SelectionSealMaterial {
            procedure: ProcedureRef::new(
                crate::successor_selection::HISTORY_TRAVERSAL_PROCEDURE_ID,
            ),
            scope,
            selected_candidate: selection.selected_payload.candidate.clone(),
            selected_occurrence_id,
            selected_membership_id,
            considered: selection.considered,
            considered_sources: selection.considered_sources,
            projection_failures,
            traversal: Some(TraversalEvidence {
                seed,
                strategy: strategy.traversal,
                selected_source: Some(if selection.selected_from_current_generation {
                    TraversalCandidateSource::CurrentGeneration
                } else {
                    TraversalCandidateSource::History
                }),
            }),
            metrics: selection.metrics,
            selected_from_generation_outcomes: selection.selected_from_current_generation,
        };
        Ok(Some((selection.decision, material)))
    }
}

fn surface_attempt_from_surface(surface: &SurfaceEvidence) -> surface_attempt::Evidence {
    surface_attempt::Evidence::applied(
        surface.producer_id.clone(),
        surface.proposal_id.clone(),
        surface.run_id.clone(),
        surface.policy.clone(),
        surface.target_relpath.clone(),
    )
}

impl ScopeFor<Generation> for ParentSelection<'_> {
    type Coordinate = u32;

    fn scope_for(&self, generation: Self::Coordinate) -> Scope<Generation> {
        Scope::<Generation>::local(&self.parent_identity.node_id(), generation)
    }
}

fn candidate_artifact_from_outcome(
    outcome: &PlannedChildOutcome,
) -> Result<CandidateArtifact, PrepareError> {
    let mut artifact = CandidateArtifact::new(outcome.node.clone(), outcome.resolved.clone());
    if let Some(surface) = outcome.artifact_surface.clone() {
        artifact = artifact.with_artifact_surface(surface);
    }
    match outcome.surface.clone() {
        Some(surface) => {
            validate_surface_evidence_binding(&outcome.node, &outcome.resolved, &surface)?;
            Ok(artifact.with_surface(surface))
        }
        None if outcome.resolved.branch.synthesized_spec_id == TUI_EDIT_SURFACE_PRODUCER_ID => {
            Err(CandidateGenerationError::MissingDeterministicEvidence {
                node_id: outcome.node_id.clone(),
            }
            .into_prepare())
        }
        None => Ok(artifact),
    }
}

#[instrument(
    target = EXECUTION_DEBUG_TARGET,
    level = "debug",
    skip(decision, material),
    fields(
        selected_candidate = %material.selected_candidate.as_str(),
        candidate_node_id = %decision.candidate_node_id,
        selected_branch_id = ?decision.selected_branch_id,
        selected_from_generation_outcomes = material.selected_from_generation_outcomes,
    )
)]
pub(crate) fn select_artifact_for_handoff(
    decision: &SuccessorDecision,
    material: &SelectionSealMaterial,
) -> Result<state_selection::Selection<state_selection::Artifact>, PrepareError> {
    let artifact = material.selected_artifact()?;
    let node = artifact.node().clone();
    let resolved = artifact.resolved().clone();
    if resolved.branch.synthesized_spec_id == TUI_EDIT_SURFACE_PRODUCER_ID
        && artifact.surface.is_none()
    {
        return Err(CandidateGenerationError::MissingDeterministicEvidence {
            node_id: node.node_id.clone(),
        }
        .into_prepare());
    }
    if let Some(surface) = artifact.surface.as_ref() {
        validate_surface_evidence_binding(&node, &resolved, surface)?;
    }
    let Some(artifact_surface) = artifact.artifact_surface.clone() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selected candidate {} has no admitted artifact surface measurement for handoff",
                material.selected_candidate.as_str()
            ),
        });
    };
    if node.node_id != decision.candidate_node_id {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selected candidate node mismatch before handoff: node_record={}, decision={}",
                node.node_id, decision.candidate_node_id
            ),
        });
    }
    let Some(selected_branch_id) = decision.selected_branch_id.as_deref() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: "successor handoff requires decision.selected_branch_id".to_string(),
        });
    };
    if node.branch_id != selected_branch_id {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selected historical candidate branch cannot be resolved safely: node_record={}, decision={}",
                node.branch_id, selected_branch_id
            ),
        });
    }

    let selected_payload = material.selected_payload()?;
    let Some(sealed) = selected_payload.sealed_evidence.as_ref() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selected candidate {} has no sealed artifact evidence",
                material.selected_candidate.as_str()
            ),
        });
    };
    if let Some(generation) = sealed.coordinate.generation
        && generation != node.generation
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selected sealed payload generation mismatch before handoff: payload={}, node_record={}",
                generation, node.generation
            ),
        });
    }
    if sealed.coordinate.node_id != node.node_id {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selected sealed payload node mismatch before handoff: payload={}, node_record={}",
                sealed.coordinate.node_id, node.node_id
            ),
        });
    }
    if sealed.coordinate.branch_id.as_deref() != Some(selected_branch_id) {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selected sealed payload branch mismatch before handoff: payload={:?}, decision={}",
                sealed.coordinate.branch_id, selected_branch_id
            ),
        });
    }
    if !material.selected_from_generation_outcomes && sealed.coordinate.primary_runtime_id.is_none()
    {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "selected historical candidate {} lacks sealed runtime identity for rehydration",
                material.selected_candidate.as_str()
            ),
        });
    }
    let primary_runtime_id = sealed.coordinate.primary_runtime_id.clone();

    let source = if material.selected_from_generation_outcomes {
        state_selection::Source::CurrentGeneration
    } else {
        state_selection::Source::History
    };
    debug!(
        target: EXECUTION_DEBUG_TARGET,
        selected_candidate = %material.selected_candidate.as_str(),
        node_id = %node.node_id,
        branch_id = %node.branch_id,
        source = ?source,
        primary_runtime_id = ?primary_runtime_id,
        "hydrated selected Artifact payload for successor handoff"
    );
    state_selection::Selection::artifact(
        node,
        material.selected_candidate.clone(),
        resolved,
        artifact_surface,
        source,
        primary_runtime_id,
    )
    .map_err(|err| PrepareError::InvalidBatchSelection {
        detail: err.to_string(),
    })
}

fn outcome_for_report<'a>(
    outcomes: &'a [PlannedChildOutcome],
    selected_node_id: Option<&str>,
) -> Option<&'a PlannedChildOutcome> {
    selected_node_id
        .and_then(|node_id| outcomes.iter().find(|outcome| outcome.node_id == node_id))
        .or_else(|| outcomes.last())
}

fn initialize_prototype1_parent_identity(
    command: &Prototype1StateCommand,
    campaign_id: &str,
    manifest_path: &Path,
    repo_root: &Path,
) -> Result<ParentIdentity, PrepareError> {
    let node_id = resolve_initial_parent_node_id(command, campaign_id, manifest_path)?;
    let Some(branch) = command.identity_branch.as_deref() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail:
                "--init-parent-identity requires --identity-branch so gen0 starts on a fresh branch"
                    .to_string(),
        });
    };
    let Some(instance_id) = command.identity_instance.as_deref() else {
        return Err(PrepareError::InvalidBatchSelection {
            detail:
                "--init-parent-identity requires --identity-instance so gen0 target selection does not read node projection files"
                    .to_string(),
        });
    };
    let expected_node_id = prototype1_node_id(branch, 0);
    if node_id != expected_node_id {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "--init-parent-identity node '{}' does not match deterministic generation-0 node '{}' for identity branch '{}'",
                node_id, expected_node_id, branch
            ),
        });
    }
    let backend = GitWorktreeBackend;
    backend
        .checkout_fresh_parent_branch(repo_root, branch)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_parent_identity_branch",
            detail: source.to_string(),
        })?;
    let identity = ParentIdentity::root_bootstrap(
        campaign_id.to_string(),
        node_id,
        instance_id.to_string(),
        branch.to_string(),
        Some(branch.to_string()),
    );
    write_parent_identity(repo_root, &identity)?;
    let message = parent_identity_commit_message(&identity);
    backend
        .persist_active_checkout_files(repo_root, &[parent_identity_relpath()], &message)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_parent_identity_commit",
            detail: source.to_string(),
        })?;
    backend
        .validate_parent_checkout(repo_root, &identity)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_parent_checkout",
            detail: source.to_string(),
        })?;
    Ok(identity)
}

fn resolve_prototype1_parent_identity(
    campaign_id: &str,
    repo_root: &Path,
) -> Result<ParentIdentity, PrepareError> {
    if let Some(identity) = load_parent_identity_optional(repo_root)? {
        identity.validate_for_command(campaign_id, None)?;
        return Ok(identity);
    }

    Err(PrepareError::InvalidBatchSelection {
        detail: format!(
            "prototype1-state requires parent identity at '{}'; run prototype1-setup or --init-parent-identity first",
            repo_root.join(parent_identity_relpath()).display()
        ),
    })
}

fn acknowledge_prototype1_state_handoff(
    command: &Prototype1StateCommand,
    campaign_id: &str,
    parent: Parent<Unchecked>,
    manifest_path: &Path,
    repo_root: &Path,
) -> Result<(Parent<Ready>, Option<SuccessorInvocation>), PrepareError> {
    let Some(invocation_path) = command.handoff_invocation.as_deref() else {
        let backend = GitWorktreeBackend;
        let parent = parent.check(
            &backend,
            manifest_path,
            Check {
                campaign_id,
                active_root: repo_root,
            },
        )?;
        let startup = Startup::<Genesis>::from_history(parent.identity(), manifest_path)?;
        return Ok((parent.ready(startup)?, None));
    };
    let invocation = match invocation::load_executable(invocation_path)? {
        InvocationAuthority::Successor(invocation) => invocation,
        InvocationAuthority::Child(_) => {
            return Err(PrepareError::InvalidBatchSelection {
                detail: format!(
                    "handoff invocation '{}' is a child invocation, expected successor",
                    invocation_path.display()
                ),
            });
        }
    };
    let identity = parent.identity();

    if invocation.campaign_id() != campaign_id {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff invocation campaign '{}' does not match command campaign '{}'",
                invocation.campaign_id(),
                campaign_id
            ),
        });
    }
    if invocation.node_id() != identity.node_id() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "handoff invocation node '{}' does not match parent identity node '{}'",
                invocation.node_id(),
                identity.node_id()
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

    let sealed_identity = validate_prototype1_successor_continuation(&invocation, manifest_path)?;
    if &sealed_identity != identity {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "sealed successor parent identity for node '{}' does not match loaded parent identity",
                invocation.node_id()
            ),
        });
    }
    let startup = Startup::<Predecessor>::from_history(identity, manifest_path, repo_root)?;
    let parent = parent.ready_from_predecessor_startup(startup)?;
    let ready = record_prototype1_successor_ready(&invocation)?;
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
    Ok((parent, Some(invocation)))
}

fn record_failed_successor_turn(invocation_path: &Path, error: &PrepareError) {
    let Ok(InvocationAuthority::Successor(invocation)) =
        invocation::load_executable(invocation_path)
    else {
        return;
    };
    let Ok(manifest_path) = campaign_manifest_path(invocation.campaign_id()) else {
        return;
    };
    if let Err(record_error) = record_prototype1_successor_completion(
        &invocation,
        &manifest_path,
        SuccessorCompletionStatus::Failed,
        None,
        Some(format!("prototype1-state successor failed: {error}")),
    ) {
        eprintln!(
            "failed to record successor failure for '{}': {record_error}",
            invocation_path.display()
        );
    }
}

fn directory_size_bytes(path: &Path) -> io::Result<u64> {
    let metadata = fs::symlink_metadata(path)?;
    if metadata.is_file() {
        return Ok(metadata.len());
    }
    if !metadata.is_dir() {
        return Ok(0);
    }

    let mut bytes = 0_u64;
    for entry in fs::read_dir(path)? {
        let entry = entry?;
        bytes = bytes.saturating_add(directory_size_bytes(&entry.path())?);
    }
    Ok(bytes)
}

fn append_parent_target_sample(
    journal: &mut PrototypeJournal,
    campaign_id: &str,
    parent_identity: &ParentIdentity,
    runtime_id: Option<crate::cli::prototype1_state::event::RuntimeId>,
    repo_root: &Path,
    phase: journal::resource::Phase,
) {
    let path = repo_root.join("target");
    let measurement = match directory_size_bytes(&path) {
        Ok(bytes) => (journal::resource::Status::Measured, Some(bytes), None),
        Err(source) if source.kind() == io::ErrorKind::NotFound => {
            (journal::resource::Status::Missing, None, None)
        }
        Err(source) => (
            journal::resource::Status::Failed,
            None,
            Some(source.to_string()),
        ),
    };
    let sample = journal::resource::Sample {
        recorded_at: RecordedAt::now(),
        campaign_id: campaign_id.to_string(),
        parent_id: parent_identity.parent_id().to_string(),
        node_id: parent_identity.node_id().to_string(),
        generation: parent_identity.generation(),
        runtime_id,
        subject: journal::resource::Subject::CargoTarget,
        phase,
        path,
        status: measurement.0,
        bytes: measurement.1,
        error: measurement.2,
    };

    info!(
        target: EXECUTION_DEBUG_TARGET,
        event = "prototype1_resource_sample",
        campaign = %sample.campaign_id,
        parent_id = %sample.parent_id,
        node_id = %sample.node_id,
        generation = sample.generation,
        runtime_id = ?sample.runtime_id,
        subject = ?sample.subject,
        phase = ?sample.phase,
        status = ?sample.status,
        bytes = ?sample.bytes,
        path = %sample.path.display(),
        "recorded prototype1 resource sample"
    );

    if let Err(source) = journal.append(JournalEntry::Resource(sample)) {
        warn!(
            target: EXECUTION_DEBUG_TARGET,
            event = "prototype1_resource_sample_record_failed",
            campaign = %campaign_id,
            parent_id = %parent_identity.parent_id(),
            node_id = %parent_identity.node_id(),
            generation = parent_identity.generation(),
            error = %source,
            "failed to append prototype1 resource sample"
        );
    }
}

impl Prototype1StateCommand {
    #[cfg(feature = "demo")]
    fn successor_handoff_mode(&self) -> SuccessorHandoffMode {
        SuccessorHandoffMode::Exec
    }

    #[cfg(not(feature = "demo"))]
    fn successor_handoff_mode(&self) -> SuccessorHandoffMode {
        SuccessorHandoffMode::Detached
    }

    #[instrument(
        target = "ploke_exec",
        level = "debug",
        skip(self),
        fields(phase = "prototype1_state")
    )]
    pub async fn run(self) -> Result<(), PrepareError> {
        let handoff_invocation = self.handoff_invocation.clone();
        let result = self.run_turn().await;
        if let Err(error) = &result
            && let Some(invocation_path) = handoff_invocation.as_deref()
        {
            record_failed_successor_turn(invocation_path, error);
        }
        result
    }

    async fn run_turn(self) -> Result<(), PrepareError> {
        let repo_root = if let Some(path) = self.repo_root.clone() {
            path
        } else {
            current_dir_as_repo_root()?
        };
        let campaign_id = resolve_prototype1_state_campaign(&self, &repo_root)?;
        record_active_prototype1_monitor_target(&campaign_id, &repo_root);
        let manifest_path = campaign_manifest_path(&campaign_id)?;
        let run_shape = Prototype1StateRunShape::resolve(&self, &manifest_path)?;
        let resolved_campaign =
            resolve_campaign_config(&campaign_id, &CampaignOverrides::default())?;
        ensure_prototype1_baseline_closure_state(&resolved_campaign)?;
        let journal_path = prototype1_transition_journal_path(&manifest_path);
        let mut journal = PrototypeJournal::new(journal_path.clone());
        let turn_span = tracing::info_span!(
            target: EXECUTION_DEBUG_TARGET,
            "prototype1.parent.turn",
            role = "parent",
            phase = "parent_turn",
            campaign = %campaign_id,
        );
        let _turn_entered = turn_span.enter();

        if self.init_parent_identity {
            let identity = initialize_prototype1_parent_identity(
                &self,
                &campaign_id,
                &manifest_path,
                &repo_root,
            )?;
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

        let parent_identity = if let Some(invocation_path) = self.handoff_invocation.as_deref() {
            match invocation::load_executable(invocation_path)? {
                InvocationAuthority::Successor(invocation) => {
                    validate_prototype1_successor_continuation(&invocation, &manifest_path)?
                }
                InvocationAuthority::Child(_) => {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "handoff invocation '{}' is a child invocation, expected successor",
                            invocation_path.display()
                        ),
                    });
                }
            }
        } else {
            resolve_prototype1_parent_identity(&campaign_id, &repo_root)?
        };
        info!(
            target: EXECUTION_DEBUG_TARGET,
            role = "parent",
            authority = if self.handoff_invocation.is_some() { "successor_invocation" } else { "artifact_identity" },
            transition = if self.handoff_invocation.is_some() { "SuccessorInvocation->ParentIdentity" } else { "active_checkout->ParentIdentity" },
            campaign = %campaign_id,
            parent_id = %parent_identity.parent_id(),
            node_id = %parent_identity.node_id(),
            generation = parent_identity.generation(),
            branch_id = %parent_identity.branch_id(),
            "resolved active parent identity"
        );
        let parent = if let Some(invocation_path) = self.handoff_invocation.as_deref() {
            let runtime_id = match invocation::load_executable(invocation_path)? {
                InvocationAuthority::Successor(invocation) => {
                    if invocation.campaign_id() != campaign_id {
                        return Err(PrepareError::InvalidBatchSelection {
                            detail: format!(
                                "handoff invocation campaign '{}' does not match command campaign '{}'",
                                invocation.campaign_id(),
                                campaign_id
                            ),
                        });
                    }
                    if invocation.node_id() != parent_identity.node_id() {
                        return Err(PrepareError::InvalidBatchSelection {
                            detail: format!(
                                "handoff invocation node '{}' does not match parent identity node '{}'",
                                invocation.node_id(),
                                parent_identity.node_id()
                            ),
                        });
                    }
                    invocation.runtime_id()
                }
                InvocationAuthority::Child(_) => {
                    return Err(PrepareError::InvalidBatchSelection {
                        detail: format!(
                            "handoff invocation '{}' is a child invocation, expected successor",
                            invocation_path.display()
                        ),
                    });
                }
            };
            Parent::<Unchecked>::load_with_runtime_id(&manifest_path, parent_identity, runtime_id)?
        } else {
            Parent::<Unchecked>::load(&manifest_path, parent_identity)?
        };
        let (parent, handoff_invocation) = acknowledge_prototype1_state_handoff(
            &self,
            &campaign_id,
            parent,
            &manifest_path,
            &repo_root,
        )?;
        let parent_identity = parent.identity().clone();
        info!(
            target: EXECUTION_DEBUG_TARGET,
            role = "parent",
            authority = "history_startup",
            transition = "Parent<Checked>->Parent<Ready>",
            campaign = %campaign_id,
            parent_id = %parent_identity.parent_id(),
            node_id = %parent_identity.node_id(),
            generation = parent_identity.generation(),
            branch_id = %parent_identity.branch_id(),
            handoff_runtime_id = ?handoff_invocation.as_ref().map(|invocation| invocation.runtime_id()),
            "parent entered ready state for active turn"
        );
        journal
            .append(JournalEntry::ParentStarted(ParentStartedEntry {
                recorded_at: RecordedAt::now(),
                campaign_id: campaign_id.clone(),
                parent_identity: parent_identity.clone(),
                repo_root: repo_root.clone(),
                handoff_runtime_id: handoff_invocation
                    .as_ref()
                    .map(|invocation| invocation.runtime_id()),
                pid: std::process::id(),
            }))
            .map_err(|err| {
                prototype1_state_transition_error("prototype1_parent_start", err.to_string())
            })?;
        append_parent_target_sample(
            &mut journal,
            &campaign_id,
            &parent_identity,
            handoff_invocation
                .as_ref()
                .map(|invocation| invocation.runtime_id()),
            &repo_root,
            journal::resource::Phase::ParentStart,
        );

        debug!(
            target: EXECUTION_DEBUG_TARGET,
            campaign = %campaign_id,
            parent_id = %parent_identity.parent_id(),
            generation = parent_identity.generation(),
            repo_root = %repo_root.display(),
            journal_path = %journal_path.display(),
            "starting typed prototype1 parent turn"
        );
        let parent_baseline = establish_parent_baseline(
            &campaign_id,
            &resolved_campaign,
            &manifest_path,
            &parent_identity,
        )
        .await?;
        let complete_search_policy = if run_shape.stop_after == Prototype1StateStopAfter::Complete {
            Some(
                if let Some(admitted) = profile::load_admitted_run_profile(&manifest_path)? {
                    admitted.profile.search_policy()
                } else {
                    load_scheduler_state(&manifest_path, OperatorProjectionRead::cli_operator())?
                        .policy
                },
            )
        } else {
            None
        };
        if run_shape.stop_after == Prototype1StateStopAfter::Complete {
            run_shape
                .candidate_generation
                .ensure_live_complete_admitted()?;
        }
        let plan_child_budget = if let Some(policy) = complete_search_policy.as_ref() {
            let current_node_count = persisted_prototype1_node_count(&manifest_path)?;
            if parent_identity.generation() >= policy.max_generations {
                return Err(PrepareError::InvalidBatchSelection {
                    detail: format!(
                        "prototype1 hard stop before child planning: parent generation {} has reached max_generations {}",
                        parent_identity.generation(),
                        policy.max_generations
                    ),
                });
            }
            reserve_complete_child_budget(policy, current_node_count)?
        } else {
            Prototype1ChildBudget { min: 1, max: 1 }
        };
        let planned_children = resolve_child_plan(
            &campaign_id,
            &manifest_path,
            &repo_root,
            parent,
            run_shape.candidate_generation,
            self.node_id.as_deref(),
            plan_child_budget,
        )
        .await?;
        let PlannedChildren {
            parent,
            plan,
            mut children,
            rejected_surface_attempts,
        } = planned_children;
        let planned_child_count = plan.body().children().len();
        let (mut child_budget, mut child_schedule_mode) =
            if let Some(policy) = complete_search_policy.as_ref() {
                (plan_child_budget, policy.child_schedule_mode)
            } else {
                // Non-Complete modes intentionally run one child as a debug/inspection slice.
                (
                    Prototype1ChildBudget { min: 1, max: 1 },
                    Prototype1ChildScheduleMode::AdaptiveBatch,
                )
            };
        if run_shape.stop_after != Prototype1StateStopAfter::Complete && self.node_id.is_some() {
            // Non-Complete + explicit node id is a single-node debug path.
            child_budget = Prototype1ChildBudget { min: 1, max: 1 };
            child_schedule_mode = Prototype1ChildScheduleMode::AdaptiveBatch;
        }
        if self.node_id.is_none() {
            children.truncate(child_budget.max as usize);
        }

        let metric_inputs = traversal_metric_inputs(run_shape.successor_selection_metrics);
        let selection_strategy = run_shape.successor_selection.active_strategy(
            metric_inputs,
            run_shape.successor_oracle_mode,
            run_shape.successor_oracle_require_evidence,
            run_shape.successor_metrics_policy,
        );
        let rejected_only_plan = run_shape.stop_after == Prototype1StateStopAfter::Complete
            && children.is_empty()
            && !rejected_surface_attempts.is_empty();
        let (child_outcomes, selection, rejected_attempt_payloads) = if rejected_only_plan {
            let projection = ParentSelection::new(
                &manifest_path,
                &parent_identity,
                &[],
                &rejected_surface_attempts,
            )
            .current_generation_candidates()?;
            (Vec::new(), None, Some(projection.considered.len()))
        } else if run_shape.stop_after == Prototype1StateStopAfter::Complete
            && child_schedule_mode == Prototype1ChildScheduleMode::AdaptiveBatch
        {
            let (outcomes, selection) = run_adaptive_child_fanout(
                &campaign_id,
                &manifest_path,
                &repo_root,
                &journal_path,
                &parent_identity,
                &parent_baseline,
                child_budget,
                children,
                &rejected_surface_attempts,
                run_shape.successor_selection_seed,
                selection_strategy,
            )
            .await?;
            (outcomes, selection, None)
        } else {
            let child_outcomes = run_child_fanout(
                &campaign_id,
                &manifest_path,
                &repo_root,
                &journal_path,
                &parent_identity,
                &parent_baseline,
                run_shape.stop_after,
                child_schedule_mode,
                child_budget,
                0,
                children,
            )
            .await?;
            let parent_selection = ParentSelection::new(
                &manifest_path,
                &parent_identity,
                &child_outcomes,
                &rejected_surface_attempts,
            );
            let selection = if run_shape.stop_after == Prototype1StateStopAfter::Complete {
                parent_selection
                    .select_successor(run_shape.successor_selection_seed, selection_strategy)?
            } else {
                None
            };
            (child_outcomes, selection, None)
        };
        let fallback_node = parent.node().clone();
        let report_child = if rejected_attempt_payloads.is_some() {
            None
        } else {
            let selected_node_id = selection
                .as_ref()
                .map(|(decision, _)| decision.candidate_node_id.as_str());
            outcome_for_report(&child_outcomes, selected_node_id)
        };
        let (mut outcome, child_runtime) = if let Some(payloads) = rejected_attempt_payloads {
            (
                format!(
                    "rejected_surface_attempts_only;children_ran=0;children_planned={};rejected_attempt_payloads={payloads}",
                    planned_child_count
                ),
                None,
            )
        } else {
            let report_child =
                report_child
                    .as_ref()
                    .ok_or_else(|| PrepareError::InvalidBatchSelection {
                        detail: "child fanout completed without any child outcome".to_string(),
                    })?;
            (
                format!(
                    "{};children_ran={};children_planned={}",
                    report_child.outcome,
                    child_outcomes.len(),
                    planned_child_count
                ),
                report_child.child_runtime.clone(),
            )
        };
        let mut successor_runtime = None;
        let mut successor_pid = None;
        let mut successor_ready_path = None;

        if let Some((selection_decision, selection_material)) = selection {
            let material = selection_material;
            let artifact = material.selected_artifact()?;
            let node = artifact.node().clone();
            let search_policy =
                complete_search_policy
                    .as_ref()
                    .ok_or_else(|| PrepareError::InvalidBatchSelection {
                        detail:
                            "successor selection reached handoff without an admitted or scheduler search policy"
                                .to_string(),
                    })?;
            let decision = live_successor_continuation_decision(
                &manifest_path,
                &parent_identity,
                search_policy,
                &selection_decision,
                &material,
                &node,
            )?;
            let handoff = if decision.disposition.allows_successor() {
                let selected_artifact =
                    select_artifact_for_handoff(&selection_decision, &material)?;
                let selection_entry = material.into_entry(selection_decision.clone())?;
                Some((selected_artifact, selection_entry))
            } else {
                None
            };
            observe::Step::start(observe::span!(
                "prototype1.parent.select_successor",
                campaign_id = %campaign_id,
                node_id = %node.node_id,
                generation = node.generation,
                selection_procedure = %selection_decision.procedure_id,
                selection_outcome = ?selection_decision.outcome,
                disposition = ?decision.disposition,
                selected_next_branch_id = ?decision.selected_next_branch_id,
                next_generation = decision.next_generation,
                total_nodes_after_continue = decision.total_nodes_after_continue,
            ))
            .success();
            journal
                .append(JournalEntry::Successor(
                    SuccessorRecord::selected_with_decision(
                        campaign_id.clone(),
                        node.node_id.clone(),
                        decision.clone(),
                        selection_decision.clone(),
                    ),
                ))
                .map_err(|err| {
                    prototype1_state_transition_error(
                        "prototype1_successor_selection",
                        err.to_string(),
                    )
                })?;
            outcome.push_str(&format!(
                ";selection={:?};successor={}",
                selection_decision.outcome, selection_decision.candidate_node_id
            ));
            if let Some((selected_artifact, selection_entry)) = handoff {
                match spawn_and_handoff_prototype1_successor(
                    &campaign_id,
                    selected_artifact,
                    &repo_root,
                    parent,
                    selection_entry,
                    self.successor_handoff_mode(),
                )? {
                    (_retired, Some(successor)) => {
                        successor_runtime = Some(successor.runtime_id.to_string());
                        successor_pid = Some(successor.pid);
                        successor_ready_path = Some(successor.ready_path);
                        outcome.push_str(";successor_handoff=acknowledged");
                    }
                    (_retired, None) => {
                        outcome.push_str(";successor_handoff=timed_out");
                    }
                }
            } else {
                journal
                    .append(JournalEntry::Successor(SuccessorRecord::stopped(
                        campaign_id.clone(),
                        node.node_id.clone(),
                        decision.clone(),
                        selection_decision.clone(),
                    )))
                    .map_err(|err| {
                        prototype1_state_transition_error(
                            "prototype1_successor_stopped",
                            err.to_string(),
                        )
                    })?;
                outcome.push_str(&format!(
                    ";successor_handoff=skipped:{:?}",
                    decision.disposition
                ));
            }
        } else if run_shape.stop_after == Prototype1StateStopAfter::Complete {
            outcome.push_str(";selection=none");
        }

        append_parent_target_sample(
            &mut journal,
            &campaign_id,
            &parent_identity,
            handoff_invocation
                .as_ref()
                .map(|invocation| invocation.runtime_id()),
            &repo_root,
            journal::resource::Phase::ParentComplete,
        );
        let report = Prototype1StateReport {
            campaign_id,
            node_id: report_child
                .as_ref()
                .map(|child| child.node_id.clone())
                .unwrap_or_else(|| fallback_node.node_id.clone()),
            repo_root,
            journal_path,
            stop_after: run_shape.stop_after,
            outcome,
            node_status: report_child
                .as_ref()
                .map(|child| child.node_status)
                .unwrap_or(fallback_node.status),
            workspace_root: report_child
                .as_ref()
                .map(|child| child.workspace_root.clone())
                .unwrap_or_else(|| fallback_node.workspace_root.clone()),
            binary_path: report_child
                .as_ref()
                .map(|child| child.binary_path.clone())
                .unwrap_or_else(|| fallback_node.binary_path.clone()),
            child_runtime,
            successor_runtime,
            successor_pid,
            successor_ready_path,
        };

        #[cfg(not(feature = "demo"))]
        {
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
        }
        #[cfg(feature = "demo")]
        let _ = report;
        if let Some(invocation) = handoff_invocation {
            let _ = record_prototype1_successor_completion(
                &invocation,
                &manifest_path,
                SuccessorCompletionStatus::Succeeded,
                None,
                None,
            )?;
        }
        Ok(())
    }
}

fn traversal_metric_inputs(input: Prototype1TraversalMetrics) -> crate::metric::Inputs {
    match input {
        Prototype1TraversalMetrics::Operational => crate::metric::Inputs::Operational,
        Prototype1TraversalMetrics::OperationalAndProtocol => {
            crate::metric::Inputs::OperationalAndProtocol
        }
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
    execute_intervention_apply(&input).map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_branch_evaluate_apply",
        detail: source.to_string(),
    })?;
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
    manifest.route_source = Some(baseline.route_source);
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

const PROTOTYPE1_BRANCH_EVALUATOR_ID: &str = "prototype1.branch_evaluation.mechanized";
const PROTOTYPE1_BRANCH_EVALUATOR_VERSION: &str = "v1";
const PROTOTYPE1_CLOSURE_EVAL_SET_KIND: &str = "closure_instance_slice";
const PROTOTYPE1_CLOSURE_EVAL_SET_AUTHORITY: &str = "typed_closure_context";

pub(crate) fn build_prototype1_branch_evaluation_report(
    baseline_campaign_id: &str,
    branch_id: &str,
    branch_registry_path: &Path,
    evaluation_artifact_path: &Path,
    baseline: &CompleteBaseline,
    treatment: &Prototype1TreatmentEvidence,
) -> Result<Prototype1BranchEvaluationReport, PrepareError> {
    let mut treatment_by_instance = BTreeMap::new();
    for row in &treatment.instances {
        treatment_by_instance.insert(row.instance_id.clone(), row);
    }

    let mut compared_instances = Vec::new();
    let mut reasons = Vec::new();

    for row in baseline.instances() {
        let treatment_row = treatment_by_instance.get(&row.instance_id).copied();
        let baseline_registration_path = row.registration_path.clone();
        let treatment_registration_path =
            treatment_row.and_then(|row| row.registration_path.clone());
        let baseline_record_path = Some(row.record_path.clone());
        let treatment_record_path = treatment_row.and_then(|row| row.record_path.clone());

        let baseline_metrics: Option<OperationalRunMetrics> = Some(row.metrics.clone());
        let treatment_metrics = treatment_row.and_then(|row| row.metrics.clone());
        let oracle_evaluation = treatment_row.and_then(|row| row.oracle_evaluation.clone());

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
                (
                    treatment_row
                        .map(|row| row.status.clone())
                        .unwrap_or_else(|| "missing_treatment_record".to_string()),
                    None,
                )
            }
            (None, _) => unreachable!("Baseline<Complete> always carries baseline metrics"),
        };

        compared_instances.push(Prototype1ComparedInstanceReport {
            instance_id: row.instance_id.clone(),
            baseline_registration_path,
            treatment_registration_path,
            baseline_record_path,
            treatment_record_path,
            baseline_metrics,
            treatment_metrics,
            oracle_evaluation,
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
        treatment_campaign_id: treatment.treatment_campaign_id.clone(),
        evaluation_procedure_id: Some(
            crate::cli::prototype1_state::evidence::PROTOTYPE1_BRANCH_EVALUATION_PROCEDURE_ID
                .to_string(),
        ),
        evaluator_identity: Some(Prototype1EvaluatorIdentity {
            id: PROTOTYPE1_BRANCH_EVALUATOR_ID.to_string(),
            version: PROTOTYPE1_BRANCH_EVALUATOR_VERSION.to_string(),
        }),
        eval_set_identity: Some(build_prototype1_eval_set_identity(
            baseline_campaign_id,
            treatment,
            baseline,
            &compared_instances,
        )),
        branch_registry_path: branch_registry_path.to_path_buf(),
        evaluation_artifact_path: evaluation_artifact_path.to_path_buf(),
        treatment_campaign_manifest: treatment.treatment_campaign_manifest.clone(),
        treatment_closure_state_path: treatment.treatment_closure_state_path.clone(),
        overall_disposition,
        reasons,
        compared_instances,
    })
}

pub(crate) fn build_prototype1_treatment_evidence(
    baseline_campaign_id: &str,
    branch_id: &str,
    treatment_campaign: &Prototype1LoopCampaign,
    treatment_state: &crate::closure::ClosureState,
) -> Result<Prototype1TreatmentEvidence, PrepareError> {
    let instances = treatment_state
        .instances
        .iter()
        .map(|row| {
            let metrics = if row.eval_status == ClosureClass::Complete {
                row.artifacts
                    .record_path
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
            let status = if metrics.is_some() {
                "complete".to_string()
            } else if row.eval_status == ClosureClass::Complete {
                "missing_treatment_record".to_string()
            } else {
                serde_name(&row.eval_status).to_string()
            };
            Ok(Prototype1TreatmentInstanceEvidence {
                instance_id: row.instance_id.clone(),
                registration_path: row.artifacts.registration_path.clone(),
                record_path: row.artifacts.record_path.clone(),
                metrics,
                oracle_evaluation: None,
                status,
            })
        })
        .collect::<Result<Vec<_>, PrepareError>>()?;

    Ok(Prototype1TreatmentEvidence {
        baseline_campaign_id: baseline_campaign_id.to_string(),
        branch_id: branch_id.to_string(),
        treatment_campaign_id: treatment_campaign.campaign_id.clone(),
        treatment_campaign_manifest: treatment_campaign.manifest_path.clone(),
        treatment_closure_state_path: treatment_campaign.closure_state_path.clone(),
        eval_policy: treatment_campaign.resolved.eval.clone(),
        benchmark_family: treatment_campaign.resolved.benchmark_family,
        dataset_sources: treatment_campaign.resolved.dataset_sources.clone(),
        instances,
    })
}

fn build_prototype1_eval_set_identity(
    baseline_campaign_id: &str,
    treatment: &Prototype1TreatmentEvidence,
    baseline: &CompleteBaseline,
    compared_instances: &[Prototype1ComparedInstanceReport],
) -> Prototype1EvalSetIdentity {
    let instance_ids = compared_instances
        .iter()
        .map(|instance| instance.instance_id.clone())
        .collect::<Vec<_>>();
    let treatment_instances = treatment
        .instances
        .iter()
        .map(|row| row.instance_id.as_str())
        .collect::<BTreeSet<_>>();
    let missing_treatment_instance_ids = instance_ids
        .iter()
        .filter(|instance_id| !treatment_instances.contains(instance_id.as_str()))
        .cloned()
        .collect::<Vec<_>>();
    let dataset_sources = treatment.dataset_sources.clone();
    let id = prototype1_eval_set_id(
        baseline_campaign_id,
        &treatment.treatment_campaign_id,
        treatment.benchmark_family,
        &dataset_sources,
        &treatment.eval_policy,
        &instance_ids,
    );

    Prototype1EvalSetIdentity {
        id,
        kind: PROTOTYPE1_CLOSURE_EVAL_SET_KIND.to_string(),
        authority: format!(
            "{}:{}",
            PROTOTYPE1_CLOSURE_EVAL_SET_AUTHORITY, baseline.eval_set_id()
        ),
        explicit: true,
        benchmark_family: treatment.benchmark_family,
        dataset_sources,
        eval_policy: treatment.eval_policy.clone(),
        instance_ids,
        missing_treatment_instance_ids,
        note: Some(
            "eval set identity is derived from typed closure/campaign context and the compared baseline closure slice".to_string(),
        ),
    }
}

fn prototype1_eval_set_id(
    baseline_campaign_id: &str,
    treatment_campaign_id: &str,
    benchmark_family: BenchmarkFamily,
    dataset_sources: &[RegistryDatasetSource],
    eval_policy: &EvalCampaignPolicy,
    instance_ids: &[String],
) -> String {
    let mut hasher = Sha256::new();
    hasher.update(PROTOTYPE1_CLOSURE_EVAL_SET_KIND.as_bytes());
    hasher.update(b"\0");
    hasher.update(baseline_campaign_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(treatment_campaign_id.as_bytes());
    hasher.update(b"\0");
    hasher.update(prototype1_benchmark_family_id(benchmark_family).as_bytes());
    hasher.update(b"\0");
    for source in dataset_sources {
        if let Some(key) = source.key.as_deref() {
            hasher.update(key.as_bytes());
        }
        hasher.update(b"\0");
        hasher.update(source.label.as_bytes());
        hasher.update(b"\0");
        hasher.update(source.path.display().to_string().as_bytes());
        hasher.update(b"\0");
        if let Some(url) = source.url.as_deref() {
            hasher.update(url.as_bytes());
        }
        hasher.update(b"\0");
    }
    hasher.update(if eval_policy.include_partial {
        b"1"
    } else {
        b"0"
    });
    hasher.update(b"\0");
    hasher.update(if eval_policy.stop_on_error {
        b"1"
    } else {
        b"0"
    });
    hasher.update(b"\0");
    if let Some(limit) = eval_policy.limit {
        hasher.update(limit.to_string().as_bytes());
    }
    hasher.update(b"\0");
    for label in &eval_policy.include_dataset_labels {
        hasher.update(label.as_bytes());
        hasher.update(b"\0");
    }
    hasher.update(b"\0");
    for label in &eval_policy.exclude_dataset_labels {
        hasher.update(label.as_bytes());
        hasher.update(b"\0");
    }
    hasher.update(b"\0");
    hasher.update(eval_policy.budget.max_turns.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(eval_policy.budget.max_tool_calls.to_string().as_bytes());
    hasher.update(b"\0");
    hasher.update(eval_policy.budget.wall_clock_secs.to_string().as_bytes());
    hasher.update(b"\0");
    if let Some(batch_prefix) = eval_policy.batch_prefix.as_deref() {
        hasher.update(batch_prefix.as_bytes());
    }
    hasher.update(b"\0");
    for instance_id in instance_ids {
        hasher.update(instance_id.as_bytes());
        hasher.update(b"\0");
    }
    format!(
        "prototype1.eval_set.closure_instance_slice.v1:{:x}",
        hasher.finalize()
    )
}

fn prototype1_benchmark_family_id(benchmark_family: BenchmarkFamily) -> &'static str {
    match benchmark_family {
        BenchmarkFamily::MultiSweBenchRust => "multi_swe_bench_rust",
    }
}

fn resolve_loop_provider_slug(
    route_source: ModelRouteSource,
    model_id: &ModelId,
    provider: Option<String>,
    phase: &'static str,
) -> Result<Option<String>, PrepareError> {
    if route_source.is_direct_google() {
        if let Some(provider) = provider.as_deref()
            && provider != "google"
        {
            return Err(PrepareError::DatabaseSetup {
                phase,
                detail: format!(
                    "direct Google route does not accept OpenRouter provider '{provider}'"
                ),
            });
        }
        return Ok(None);
    }

    match provider {
        Some(provider) => Ok(Some(
            ProviderKey::new(&provider)
                .map_err(|err| PrepareError::DatabaseSetup {
                    phase,
                    detail: err.to_string(),
                })?
                .slug
                .as_str()
                .to_string(),
        )),
        None => {
            Ok(load_provider_for_model(model_id)?
                .map(|provider| provider.slug.as_str().to_string()))
        }
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
    run_profile: Option<&profile::Prototype1RunProfile>,
) -> Result<Prototype1LoopCampaign, PrepareError> {
    let selected_eval_model = resolve_model_for_run(
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
    )?;
    let eval_route_source = command
        .route_source
        .unwrap_or(selected_eval_model.route_source);
    let eval_model = selected_eval_model.id;
    let eval_provider_slug = resolve_loop_provider_slug(
        eval_route_source,
        &eval_model,
        command.provider.clone(),
        "prototype1_loop_provider",
    )?;

    if command.stop_after >= Prototype1LoopStopAfter::BaselineProtocol {
        let protocol_model = resolve_protocol_model_id(command.protocol_model_id.clone())?;
        let protocol_route_source = command.protocol_route_source.unwrap_or(eval_route_source);
        let protocol_provider = resolve_protocol_provider_slug(
            &protocol_model,
            Some(protocol_route_source),
            command.protocol_provider.clone(),
        )?;
        if protocol_model != eval_model
            || protocol_route_source != eval_route_source
            || protocol_provider != eval_provider_slug
        {
            return Err(PrepareError::DatabaseSetup {
                phase: "prototype1_loop_campaign",
                detail: format!(
                    "prototype1 baseline arm now delegates to closure/campaign and currently requires one shared model/route/provider; eval={} {:?} {:?}, protocol={} {:?} {:?}",
                    eval_model,
                    eval_route_source,
                    eval_provider_slug,
                    protocol_model,
                    protocol_route_source,
                    protocol_provider
                ),
            });
        }
    }

    let campaign_id = command.campaign.clone().unwrap_or_else(|| {
        format!(
            "prototype1-{}",
            sanitize_batch_component(&prepared_batch.batch_id)
        )
    });
    let manifest_path = campaign_manifest_path(&campaign_id)?;
    if manifest_path.exists() {
        return Err(PrepareError::InvalidBatchSelection {
            detail: format!(
                "campaign '{}' already has a manifest at '{}'; choose a new --campaign or run the existing campaign",
                campaign_id,
                manifest_path.display()
            ),
        });
    }
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
    manifest.route_source = Some(eval_route_source);
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
    let protocol_policy = run_profile
        .map(profile::Prototype1RunProfile::protocol_policy)
        .unwrap_or_default();
    manifest.protocol = ProtocolCampaignPolicy {
        stop_on_error: command.stop_on_error,
        ..protocol_policy
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
    staged_children: Vec<ChildFiles>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Prototype1TreatmentEvidence {
    pub(crate) baseline_campaign_id: String,
    pub(crate) branch_id: String,
    pub(crate) treatment_campaign_id: String,
    pub(crate) treatment_campaign_manifest: PathBuf,
    pub(crate) treatment_closure_state_path: PathBuf,
    pub(crate) eval_policy: EvalCampaignPolicy,
    pub(crate) benchmark_family: BenchmarkFamily,
    pub(crate) dataset_sources: Vec<RegistryDatasetSource>,
    pub(crate) instances: Vec<Prototype1TreatmentInstanceEvidence>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Prototype1TreatmentInstanceEvidence {
    pub(crate) instance_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) registration_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) record_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) metrics: Option<OperationalRunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) oracle_evaluation: Option<crate::mbe::OracleEvaluation>,
    pub(crate) status: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Prototype1BranchEvaluationReport {
    pub(crate) baseline_campaign_id: String,
    pub(crate) branch_id: String,
    pub(crate) treatment_campaign_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) evaluation_procedure_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) evaluator_identity: Option<Prototype1EvaluatorIdentity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) eval_set_identity: Option<Prototype1EvalSetIdentity>,
    pub(crate) branch_registry_path: PathBuf,
    pub(crate) evaluation_artifact_path: PathBuf,
    pub(crate) treatment_campaign_manifest: PathBuf,
    pub(crate) treatment_closure_state_path: PathBuf,
    pub(crate) overall_disposition: BranchDisposition,
    pub(crate) reasons: Vec<String>,
    pub(crate) compared_instances: Vec<Prototype1ComparedInstanceReport>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Prototype1EvaluatorIdentity {
    pub(crate) id: String,
    pub(crate) version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Prototype1EvalSetIdentity {
    pub(crate) id: String,
    pub(crate) kind: String,
    pub(crate) authority: String,
    pub(crate) explicit: bool,
    pub(crate) benchmark_family: BenchmarkFamily,
    pub(crate) dataset_sources: Vec<RegistryDatasetSource>,
    pub(crate) eval_policy: EvalCampaignPolicy,
    pub(crate) instance_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) missing_treatment_instance_ids: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct Prototype1ComparedInstanceReport {
    pub(crate) instance_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) baseline_registration_path: Option<PathBuf>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) treatment_registration_path: Option<PathBuf>,
    pub(crate) baseline_record_path: Option<PathBuf>,
    pub(crate) treatment_record_path: Option<PathBuf>,
    pub(crate) baseline_metrics: Option<OperationalRunMetrics>,
    pub(crate) treatment_metrics: Option<OperationalRunMetrics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) oracle_evaluation: Option<crate::mbe::OracleEvaluation>,
    pub(crate) evaluation: Option<BranchEvaluationResult>,
    pub(crate) status: String,
}

pub(crate) fn selection_input_from_child_report(
    node: &Prototype1NodeRecord,
    report: &Prototype1BranchEvaluationReport,
) -> SelectionInput {
    let comparisons = report
        .compared_instances
        .iter()
        .map(|instance| RunComparison {
            instance_id: instance.instance_id.clone(),
            parent_metrics: instance.baseline_metrics.clone(),
            child_metrics: instance.treatment_metrics.clone(),
            oracle_evaluation: instance.oracle_evaluation.clone(),
            status: instance.status.clone(),
        })
        .collect();

    SelectionInput::new(
        CandidateRef {
            node_id: node.node_id.clone(),
            branch_id: node.branch_id.clone(),
            generation: node.generation,
        },
        report.overall_disposition.clone(),
        report.evaluation_artifact_path.clone(),
        comparisons,
    )
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
        "search_policy: generations<={} nodes<={} children={}..={} mode={} stop_on_first_keep={} require_keep_for_continuation={} explore_from_rejected={}",
        report.search_policy.max_generations,
        report.search_policy.max_total_nodes,
        report.search_policy.child_budget.min,
        report.search_policy.child_budget.max,
        serde_name(&report.search_policy.child_schedule_mode),
        yes_no(report.search_policy.stop_on_first_keep),
        yes_no(report.search_policy.require_keep_for_continuation),
        yes_no(report.search_policy.explore_from_rejected)
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

fn prototype1_trace_path(campaign_manifest_path: &Path) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1-loop-trace.json")
}
