use clap::{ArgAction, ArgGroup, Args, Parser, Subcommand};
use ploke_llm::Router;
use ploke_llm::request::{endpoint::Endpoint, models::ModelRouteSource};
use ploke_llm::router_only::HasEndpoint;
use ploke_llm::router_only::google::Google;
use ploke_llm::router_only::openrouter::{OpenRouter, OpenRouterModelId};
use ploke_llm::{ModelId, ProviderKey, SupportsTools};
use ploke_protocol::procedure::{
    ProcedureDebugEvent, ProcedureDebugEventKind, ProcedureDebugSink, set_procedure_debug_sink,
};
use ploke_protocol::tool_calls::trace::NeighborhoodSource;
use ploke_protocol::tool_calls::{review, segment, trace};
use ploke_protocol::{JsonAdjudicator, JsonLlmConfig, Procedure, ProtocolReasoningPolicy};
use ploke_records::llm_response::{FULL_RESPONSE_TRACE_FILE, RawFullResponseRecord};
use ploke_records::protocol::{InterventionApplyArtifact, InterventionIssueDetectionArtifact};
use ploke_records::tool_contracts::{
    PersistedToolCallArguments, ToolArgumentDecodeError, ToolArgumentParseFailure,
    ToolArgumentsJson, ToolCallArguments,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, VecDeque};
use std::fs;
use std::io::{IsTerminal, Write};
use std::path::{Path, PathBuf};
use std::process::{Command as ProcessCommand, ExitCode};
use std::str::FromStr;
use std::sync::{Arc, OnceLock};
use tokio::sync::Semaphore;
use tokio::task::JoinSet;
use uuid::Uuid;

mod args;
mod dispatch;
mod format;
mod handlers;
pub mod types;

pub(crate) use handlers::campaign::default_campaign_submission_export_path;
pub(crate) use handlers::closure::{
    ClosureAdvanceProtocolReport, ProtocolNextStep, ProtocolRunPlan, ProtocolRunState,
    advance_eval_closure, advance_protocol_closure, advance_protocol_or_block,
    ensure_prepared_runs_under_repo_cache, ensure_repo_cache_clone_preflight,
    execute_protocol_intent_segments_quiet, execute_protocol_tool_call_review_quiet,
    execute_protocol_tool_call_segment_review_quiet, format_protocol_selected_runs,
    format_remaining_protocol_work, is_retryable_local_analysis_review_error,
    print_protocol_state_table, protocol_llm_config, protocol_next_command,
    protocol_report_allows_continue, protocol_report_made_progress, protocol_run_plan,
    protocol_state_for_run, run_tool_call_review_with_json_retries,
    run_tool_call_segment_review_with_json_retries,
};
pub(crate) use handlers::inspect::{
    abbreviate_path_tail, build_segment_review_subject, build_tool_call_review_subject,
    build_tool_call_sequence_subject, extract_argument_string, join_indices, render_messages_json,
    render_messages_table, render_payload_block, render_tool_inputs, render_tool_loop_table,
    serde_name, summarize_failure_reason, summarize_patch_state, summarize_tool_inputs,
    tool_call_next_step_index,
};
pub(crate) use handlers::protocol::{persist_issue_detection_for_record, print_issue_case_block};
pub(crate) use handlers::prototype1_support::{TimingTrace, pending_prototype1_stages};
pub(crate) use handlers::registry::registry_dataset_view;
pub(crate) use handlers::run::{default_batch_id, resolve_batch_manifest, yes_no};
mod prototype1_process;
/// Prototype 1 typed state model and persisted artifact map.
///
/// See `prototype1_state::mod` for the implementation split and the on-disk
/// campaign layout under `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`.
pub(crate) mod prototype1_state;
mod provider;

pub use args::*;

pub(crate) use format::{
    display_context_length, display_price_per_million, extract_model_size, model_size_string,
};

pub(crate) use provider::{
    current_provider_for_model, headless_model_selection,
    headless_model_selection_from_provider_preference, load_parent_patcher_model_selection,
    parse_provider_key, registry_route_source, resolve_provider_model_id,
};

#[cfg(test)]
#[path = "cli/tests.rs"]
mod tests;

pub(crate) const PROTOCOL_HTTP_MAX_ATTEMPTS: u32 = 1;
pub(crate) const PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS: usize = 3;
pub(crate) const TOOL_CALL_REVIEW_TIMEOUT_SECS: u64 = ploke_llm::LLM_TIMEOUT_SECS;

use crate::campaign::{
    CampaignManifest, CampaignOverrides, CampaignValidationCheck, EvalCampaignPolicy,
    ProtocolCampaignPolicy, ResolvedCampaignConfig, adopt_campaign_manifest_from_closure_state,
    adopt_campaign_manifest_from_registry, apply_campaign_overrides, campaign_closure_state_path,
    campaign_manifest_path, dataset_files_from_sources, dataset_keys_from_sources,
    default_protocol_max_tokens, list_campaigns, render_resolved_campaign_config,
    resolve_campaign_config, save_campaign_manifest, validate_campaign_config,
};
use crate::closure::{
    ClosureClass, ClosureRecomputeRequest, closure_state_path, load_closure_state,
    recompute_closure_state, render_closure_status,
};
use crate::inner::registry::RunRegistration;
use crate::intervention::{
    INTERVENTION_APPLY_PROCEDURE, INTERVENTION_ISSUE_DETECTION_PROCEDURE,
    INTERVENTION_SYNTHESIS_PROCEDURE, InterventionApplyInput, InterventionApplyOutput,
    InterventionSynthesisInput, IssueCase, IssueDetectionInput, IssueDetectionOutput,
    detect_issue_cases, execute_intervention_apply, issue_detection_artifact_input,
    operation_target_artifact_id, select_primary_issue, synthesize_intervention_with_llm,
};
use crate::intervention_issue_aggregate::{
    IssueDetectionAggregate, IssueDetectionAggregateError, load_issue_detection_aggregate,
};
use crate::layout::{
    active_model_file, batches_dir, cache_dir, datasets_dir, instances_dir, model_registry_file,
    models_dir, parent_patcher_model_file, repos_dir, starting_db_cache_dir,
    workspace_root_for_key,
};
use crate::model_registry::{
    find_models, load_active_model, load_model_registry, load_parent_patcher_model,
    refresh_model_registry, registry_has_model, save_active_model, save_parent_patcher_model,
};
use crate::msb::{PrepareMsbBatchRequest, PrepareMsbSingleRunRequest};
use crate::projection::OperatorProjectionRead;
use crate::protocol::protocol_aggregate::{
    ProtocolAggregate, ProtocolAggregateError, ProtocolCallReviewRow, load_protocol_aggregate,
};
use crate::protocol_artifacts::{
    StoredProtocolArtifactFile, list_protocol_artifact_load_results, list_protocol_artifacts,
    load_protocol_artifact, protocol_artifact_preview, protocol_artifact_summary,
    write_protocol_artifact,
};
use crate::protocol_report::{
    ProtocolAggregateCallIssueRow, ProtocolAggregateCoverage, ProtocolAggregateReport,
    ProtocolAggregateSegmentRow, ProtocolColorProfile, ProtocolReportRenderOptions,
    render_protocol_aggregate_report_with_options,
};
use crate::protocol_triage_report::{
    ProtocolCampaignCountRow, ProtocolCampaignEvidence, ProtocolCampaignExemplarRow,
    ProtocolCampaignFamilyRow, ProtocolCampaignSummary, ProtocolCampaignTriageReport,
    render_protocol_campaign_triage_report, sort_count_rows,
};
use crate::provider_prefs::{
    clear_provider_for_model, load_provider_for_model, set_provider_for_model,
};
use crate::record::read_compressed_record;
use crate::registry::{builtin_dataset_registry_entries, builtin_dataset_registry_entry};
use crate::run_history::{
    RunDirPreference, list_finished_record_paths_in_instances_root, preferred_run_dir_for_instance,
    print_assistant_messages_from_record_path,
};
use crate::run_registry::list_registrations_for_instance;
use crate::runner::{
    BatchRunArtifactPaths, BatchRunSummary, MultiSweBenchSubmissionRecord, ReplayMsbBatchRequest,
    RunMsbAgentBatchRequest, RunMsbAgentSingleRequest, RunMsbBatchRequest, RunMsbSingleRequest,
    resolve_route_for_model,
};
use crate::selection::{
    ActiveSelection, ActiveSelectionSlot, clear_active_selection, load_active_selection,
    load_active_selection_at, render_selection_warnings, save_active_selection,
    unset_active_selection_slot,
};
use crate::spec::{
    EvalBudget, IssueInput, OutputMode, PrepareError, PrepareSingleRunRequest, PrepareWrite,
    PreparedCampaignContext,
};
use crate::target_registry::{
    BenchmarkFamily, RegistryEntry, RegistryRecomputeRequest, TargetRegistry, load_target_registry,
    recompute_target_registry, render_target_registry_status, target_registry_path,
};

impl Cli {
    pub async fn run(self) -> ExitCode {
        dispatch::run(self).await
    }
}

pub(crate) fn print_builtin_dataset_entries() {
    for entry in builtin_dataset_registry_entries() {
        println!("{}\t{}\t{}", entry.key, entry.language, entry.url);
    }
}

async fn persist_intervention_synthesis_for_record(
    record_path: &Path,
    issue: IssueCase,
    source_state_id: String,
    model_id: Option<String>,
    provider: Option<String>,
) -> Result<crate::intervention::InterventionSynthesisOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let target_relpath = PathBuf::from(issue.target_tool.description_artifact_relpath());
    let source_content =
        fs::read_to_string(&target_relpath).map_err(|source| PrepareError::ReadManifest {
            path: target_relpath.clone(),
            source,
        })?;
    let input = InterventionSynthesisInput {
        issue,
        source_state_id,
        source_content,
        // The generic CLI path does not yet know the durable Artifact target
        // for this record. Downstream layers will preserve a fallback text-file
        // surface id, but future backend-aware callers should pass an
        // OperationTarget here instead of relying on that fallback.
        operation_target: None,
    };
    let cfg = protocol_llm_config(
        model_id,
        None,
        provider,
        120,
        PROTOCOL_HTTP_MAX_ATTEMPTS,
        3200,
        ProtocolReasoningPolicy::default(),
    )?;
    let run = synthesize_intervention_with_llm(input.clone(), cfg.clone())
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "intervention_synthesis",
            detail: err.to_string(),
        })?;
    write_protocol_artifact(
        record_path,
        INTERVENTION_SYNTHESIS_PROCEDURE,
        &subject_id,
        Some(cfg.model_id.as_str()),
        cfg.provider_slug.as_deref(),
        &input,
        &run.output,
        &run.artifact,
    )?;
    Ok(run.output)
}

fn persist_intervention_apply_for_record(
    record_path: &Path,
    synthesis: &crate::intervention::InterventionSynthesisOutput,
    candidate_id: &str,
    repo_root: &Path,
) -> Result<InterventionApplyOutput, PrepareError> {
    let record =
        read_compressed_record(record_path).map_err(|source| PrepareError::ReadManifest {
            path: record_path.to_path_buf(),
            source,
        })?;
    let subject_id = record.metadata.benchmark.instance_id.clone();
    let candidate = synthesis
        .candidate_set
        .candidates
        .iter()
        .find(|candidate| candidate.candidate_id == candidate_id)
        .cloned()
        .ok_or_else(|| PrepareError::InvalidBatchSelection {
            detail: format!(
                "intervention apply candidate '{}' not found for subject '{}'",
                candidate_id, subject_id
            ),
        })?;
    let base_artifact_id = synthesis
        .candidate_set
        .operation_target
        .as_ref()
        .and_then(operation_target_artifact_id)
        .cloned();
    let patch_id = candidate.patch_id.clone();
    let input = InterventionApplyInput {
        source_state_id: synthesis.candidate_set.source_state_id.clone(),
        candidate,
        target_relpath: synthesis.candidate_set.target_relpath.clone(),
        expected_source_content: synthesis.candidate_set.source_content.clone(),
        repo_root: repo_root.to_path_buf(),
        base_artifact_id,
        patch_id,
    };
    let output =
        execute_intervention_apply(&input).map_err(|source| PrepareError::DatabaseSetup {
            phase: "intervention_apply",
            detail: source.to_string(),
        })?;
    let artifact = InterventionApplyArtifact(&output);
    write_protocol_artifact(
        record_path,
        INTERVENTION_APPLY_PROCEDURE,
        &subject_id,
        None,
        None,
        &input,
        &output,
        &artifact,
    )?;
    Ok(output)
}

pub(crate) fn write_json_file_pretty<T: Serialize>(
    path: &Path,
    value: &T,
) -> Result<(), PrepareError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let bytes = serde_json::to_vec_pretty(value).map_err(PrepareError::Serialize)?;
    fs::write(path, bytes).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn resolve_protocol_model_id(model_id: Option<String>) -> Result<ModelId, PrepareError> {
    match model_id {
        Some(model_id) => {
            model_id
                .parse()
                .map_err(|err: ploke_llm::IdError| PrepareError::DatabaseSetup {
                    phase: "protocol_model_id",
                    detail: err.to_string(),
                })
        }
        // Temporary split config: eval/protocol defaults still read the active
        // model selection here, while broad parent patching reads
        // `load_parent_patcher_model_selection()` below. Collapse both onto the
        // admitted profile/campaign config once that plumbing exists.
        None => load_active_model().map(|selection| selection.model_id),
    }
}

fn resolve_protocol_provider_slug(
    model_id: &ModelId,
    route_source: Option<ModelRouteSource>,
    provider: Option<String>,
) -> Result<Option<String>, PrepareError> {
    resolve_protocol_route(model_id, route_source, provider).map(|(_, provider_slug)| provider_slug)
}

pub(crate) fn resolve_protocol_route(
    model_id: &ModelId,
    route_source: Option<ModelRouteSource>,
    provider: Option<String>,
) -> Result<(ModelRouteSource, Option<String>), PrepareError> {
    if let Some(route_source) = route_source {
        return match route_source {
            ModelRouteSource::DirectGoogle => {
                if let Some(provider) = provider.as_deref()
                    && provider != "google"
                {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "protocol_route",
                        detail: format!(
                            "direct Google route does not accept OpenRouter provider '{provider}'"
                        ),
                    });
                }
                Ok((ModelRouteSource::DirectGoogle, None))
            }
            ModelRouteSource::OpenRouter => {
                let provider_slug = provider
                    .map(|provider| {
                        ProviderKey::new(&provider).map_err(|err| PrepareError::DatabaseSetup {
                            phase: "protocol_provider_slug",
                            detail: err.to_string(),
                        })
                    })
                    .transpose()?
                    .map(|provider| provider.slug.as_str().to_string());
                Ok((ModelRouteSource::OpenRouter, provider_slug))
            }
        };
    }

    if let Some(provider) = provider {
        let parsed = ProviderKey::new(&provider).map_err(|err| PrepareError::DatabaseSetup {
            phase: "protocol_provider_slug",
            detail: err.to_string(),
        })?;
        if registry_route_source(model_id)?.is_some_and(|source| source.is_direct_google()) {
            if parsed.slug.as_str() == "google" {
                return Ok((ModelRouteSource::DirectGoogle, None));
            }
            return Err(PrepareError::DatabaseSetup {
                phase: "protocol_route",
                detail: format!(
                    "direct Google model '{model_id}' does not accept OpenRouter provider '{}'",
                    parsed.slug.as_str()
                ),
            });
        }
        return Ok((
            ModelRouteSource::OpenRouter,
            Some(parsed.slug.as_str().to_string()),
        ));
    }

    if registry_route_source(model_id)?.is_some_and(|source| source.is_direct_google()) {
        return Ok((ModelRouteSource::DirectGoogle, None));
    }

    let provider = load_provider_for_model(model_id)?;
    Ok((
        ModelRouteSource::OpenRouter,
        provider.map(|provider| provider.slug.as_str().to_string()),
    ))
}

pub(crate) fn tool_call_review_error_to_prepare(err: review::ToolCallReviewError) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_review",
        detail: err.to_string(),
    }
}

pub(crate) fn tool_call_intent_segmentation_error_to_prepare(
    err: segment::IntentSegmentationError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_intent_segmentation",
        detail: err.to_string(),
    }
}

pub(crate) fn is_retryable_intent_segmentation_error(
    err: &segment::IntentSegmentationError,
) -> bool {
    match err {
        segment::IntentSegmentationError::Second(ploke_protocol::MergeError::Branches(
            ploke_protocol::FanOutError::Right(llm_error),
        )) if llm_error.is_truncated_json_parse() => true,
        segment::IntentSegmentationError::Second(ploke_protocol::MergeError::Join(
            segment::NormalizeSegmentsError::Overlap { .. }
            | segment::NormalizeSegmentsError::InvalidRange { .. }
            | segment::NormalizeSegmentsError::MissingLabel { .. }
            | segment::NormalizeSegmentsError::AmbiguousWithLabel { .. },
        )) => true,
        _ => false,
    }
}

pub(crate) fn tool_call_segment_review_error_to_prepare(
    err: review::ToolCallSegmentReviewError,
) -> PrepareError {
    PrepareError::DatabaseSetup {
        phase: "protocol_tool_call_segment_review",
        detail: err.to_string(),
    }
}

pub(crate) fn truncate_for_table(text: &str, max_len: usize) -> String {
    if text.chars().count() <= max_len {
        text.to_string()
    } else {
        format!(
            "{}...",
            text.chars()
                .take(max_len.saturating_sub(3))
                .collect::<String>()
        )
    }
}

pub(crate) fn truncate_middle(text: &str, max_len: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= max_len {
        return text.to_string();
    }
    if max_len <= 3 {
        return ".".repeat(max_len);
    }
    let front = (max_len - 3) / 2;
    let back = max_len - 3 - front;
    format!(
        "{}...{}",
        chars[..front].iter().collect::<String>(),
        chars[chars.len() - back..].iter().collect::<String>()
    )
}

struct RecordResolution {
    record_path: PathBuf,
    footer: Option<String>,
    warnings: Vec<String>,
}

pub(crate) fn has_legacy_instance_root_artifacts(instances_root: &Path, instance_id: &str) -> bool {
    let instance_root = instances_root.join(instance_id);
    [
        "record.json.gz",
        "execution-log.json",
        "repo-state.json",
        "indexing-status.json",
        "snapshot-status.json",
        "final-snapshot.db",
        "agent-turn-summary.json",
        "agent-turn-trace.json",
        "llm-full-responses.jsonl",
        "multi-swe-bench-submission.jsonl",
        "protocol-artifacts",
    ]
    .iter()
    .any(|name| instance_root.join(name).exists())
}

pub(crate) fn legacy_instance_root_warning(instance_id: &str) -> String {
    format!(
        "instance {instance_id} still has legacy top-level run artifacts under ~/.ploke-eval/instances/{instance_id}; authoritative attempt data lives under runs/run-* and is selected from registrations"
    )
}

pub(crate) fn resolve_record_path(
    record: Option<PathBuf>,
    instance: Option<String>,
    attempt: Option<u32>,
) -> Result<RecordResolution, PrepareError> {
    resolve_record_path_from_eval_home(record, instance, attempt, crate::layout::ploke_eval_home()?)
}

fn resolve_record_path_from_eval_home(
    record: Option<PathBuf>,
    instance: Option<String>,
    attempt: Option<u32>,
    eval_home: PathBuf,
) -> Result<RecordResolution, PrepareError> {
    let instances_root = crate::layout::instances_dir()?;
    let selection = load_active_selection_at(&eval_home, OperatorProjectionRead::cli_operator())?;
    match (record, instance) {
        (Some(path), None) => Ok(RecordResolution {
            record_path: path,
            footer: None,
            warnings: Vec::new(),
        }),
        (None, explicit_instance) => {
            let resolved_instance = explicit_instance.or_else(|| selection.instance.clone());
            let (resolved_attempt, attempt_warning) =
                resolve_attempt_override(&selection, resolved_instance.as_deref(), attempt);
            if let Some(instance_id) = resolved_instance {
                let mut warnings = render_selection_warnings(&selection_with_resolution(
                    &selection,
                    Some(&instance_id),
                    resolved_attempt,
                ));
                if let Some(warning) = attempt_warning {
                    warnings.push(warning);
                }
                return resolve_instance_record_path(
                    &instances_root,
                    &instance_id,
                    resolved_attempt,
                    warnings,
                );
            }
            if resolved_attempt.is_some() {
                return Err(PrepareError::DatabaseSetup {
                    phase: "resolve_record_path",
                    detail: "an active attempt selection requires an active or explicit instance"
                        .to_string(),
                });
            }
            let last_run = crate::run_history::load_last_run_at(&eval_home)?;
            let record_path = last_run.run_dir.join("record.json.gz");
            let footer =
                match crate::run_registry::load_registration_for_run_dir(&last_run.run_dir)? {
                    Some(registration) => {
                        let attempt = attempt_number_for_registration(
                            &instances_root,
                            &registration.frozen_spec.task_id,
                            &registration.run_id,
                        )?;
                        Some(format!(
                            "resolved run: {} attempt {} (latest)",
                            registration.frozen_spec.task_id, attempt
                        ))
                    }
                    None => Some("resolved run: most recent completed run".to_string()),
                };
            Ok(RecordResolution {
                record_path,
                footer,
                warnings: render_selection_warnings(&selection),
            })
        }
        (Some(_), Some(_)) => Err(PrepareError::MissingRunManifest(
            instances_root.join("<instance>/runs/run-*/record.json.gz"),
        )),
    }
}

fn resolve_attempt_override(
    selection: &ActiveSelection,
    resolved_instance: Option<&str>,
    explicit_attempt: Option<u32>,
) -> (Option<u32>, Option<String>) {
    if explicit_attempt.is_some() {
        return (explicit_attempt, None);
    }
    let Some(selected_attempt) = selection.attempt else {
        return (None, None);
    };
    let Some(selected_instance) = selection.instance.as_deref() else {
        return (
            None,
            Some("selected attempt was ignored because no selected instance is active".to_string()),
        );
    };
    match resolved_instance {
        Some(instance) if instance == selected_instance => (Some(selected_attempt), None),
        Some(instance) => (
            None,
            Some(format!(
                "selected attempt {} for instance {} was ignored because instance {} was requested",
                selected_attempt, selected_instance, instance
            )),
        ),
        None => (None, None),
    }
}

pub(crate) fn selection_with_resolution(
    selection: &ActiveSelection,
    instance: Option<&str>,
    attempt: Option<u32>,
) -> ActiveSelection {
    let mut resolved = selection.clone();
    if let Some(instance) = instance {
        resolved.instance = Some(instance.to_string());
        resolved.attempt = attempt;
    }
    resolved
}

fn resolve_instance_record_path(
    instances_root: &Path,
    instance_id: &str,
    attempt: Option<u32>,
    mut warnings: Vec<String>,
) -> Result<RecordResolution, PrepareError> {
    let registrations = list_attempt_registrations(instances_root, instance_id)?;
    if !registrations.is_empty() {
        if has_legacy_instance_root_artifacts(instances_root, instance_id) {
            warnings.push(legacy_instance_root_warning(instance_id));
        }
        let selected_index = match attempt {
            Some(number) if number > 0 => {
                let index = (number - 1) as usize;
                if index >= registrations.len() {
                    return Err(PrepareError::DatabaseSetup {
                        phase: "resolve_record_path",
                        detail: format!(
                            "instance {instance_id} has {} attempt(s); attempt {} is out of range",
                            registrations.len(),
                            number
                        ),
                    });
                }
                index
            }
            Some(_) => {
                return Err(PrepareError::DatabaseSetup {
                    phase: "resolve_record_path",
                    detail: "attempt numbers are 1-based".to_string(),
                });
            }
            None => registrations.len() - 1,
        };
        let selected = &registrations[selected_index];
        return Ok(RecordResolution {
            record_path: selected.artifacts.record_path.clone(),
            footer: attempt.is_none().then(|| {
                format!(
                    "resolved run: {} attempt {} (latest)",
                    instance_id,
                    selected_index + 1
                )
            }),
            warnings,
        });
    }

    if attempt.is_some() {
        return Err(PrepareError::DatabaseSetup {
            phase: "resolve_record_path",
            detail: format!(
                "instance {instance_id} has no registered attempts; cannot resolve a numbered attempt"
            ),
        });
    }

    Err(PrepareError::DatabaseSetup {
        phase: "resolve_record_path",
        detail: format!(
            "instance {instance_id} has no registered attempts; legacy instance-root artifacts are no longer used"
        ),
    })
}

pub(crate) fn list_attempt_registrations(
    instances_root: &Path,
    instance_id: &str,
) -> Result<Vec<RunRegistration>, PrepareError> {
    let mut registrations = list_registrations_for_instance(instances_root, instance_id)?;
    registrations.sort_by(|left, right| {
        run_registration_sort_key(left).cmp(&run_registration_sort_key(right))
    });
    Ok(registrations)
}

fn attempt_number_for_registration(
    instances_root: &Path,
    instance_id: &str,
    run_id: &str,
) -> Result<usize, PrepareError> {
    let registrations = list_attempt_registrations(instances_root, instance_id)?;
    registrations
        .iter()
        .position(|registration| registration.run_id == run_id)
        .map(|index| index + 1)
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "resolve_record_path",
            detail: format!("run {run_id} is not registered under instance {instance_id}"),
        })
}

fn run_registration_sort_key(registration: &RunRegistration) -> (String, String) {
    (
        registration
            .lifecycle
            .finished_at
            .clone()
            .unwrap_or_else(|| registration.lifecycle.updated_at.clone()),
        registration.run_id.clone(),
    )
}

pub(crate) fn print_record_resolution_footer(resolution: &RecordResolution) {
    let _ = std::io::stdout().flush();
    for warning in &resolution.warnings {
        eprintln!("warning: {warning}");
    }
    if let Some(footer) = &resolution.footer {
        eprintln!("{footer}");
    }
}

pub(crate) fn print_selection_update(selection: &ActiveSelection) {
    println!(
        "campaign: {}",
        selection.campaign.as_deref().unwrap_or("(none)")
    );
    println!("batch: {}", selection.batch.as_deref().unwrap_or("(none)"));
    println!(
        "instance: {}",
        selection.instance.as_deref().unwrap_or("(none)")
    );
    println!(
        "attempt: {}",
        selection
            .attempt
            .map(|attempt| attempt.to_string())
            .unwrap_or_else(|| "(latest)".to_string())
    );
    for warning in render_selection_warnings(selection) {
        println!("warning: {warning}");
    }
}

pub(crate) fn sanitize_batch_component(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut last_was_dash = false;
    for ch in input.chars() {
        let normalized = if ch.is_ascii_alphanumeric() { ch } else { '-' };
        if normalized == '-' {
            if last_was_dash {
                continue;
            }
            last_was_dash = true;
            out.push('-');
        } else {
            last_was_dash = false;
            out.push(normalized.to_ascii_lowercase());
        }
    }
    let trimmed = out.trim_matches('-');
    if trimmed.is_empty() {
        "batch".to_string()
    } else {
        trimmed.to_string()
    }
}

fn run_doctor() -> Result<(), PrepareError> {
    let mut ok = 0usize;
    let mut warn = 0usize;
    let mut note = 0usize;

    println!("ploke-eval doctor");
    println!();

    let home = crate::layout::ploke_eval_home()?;
    println!("home: {}", home.display());

    let builtins = builtin_dataset_registry_entries();
    println!(
        "built-in datasets: {}{}",
        builtins.len(),
        if builtins.is_empty() {
            String::new()
        } else {
            format!(
                " ({})",
                builtins
                    .iter()
                    .map(|entry| entry.key)
                    .collect::<Vec<_>>()
                    .join(", ")
            )
        }
    );

    check_dir(
        "datasets dir",
        datasets_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "models dir",
        models_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "repo cache dir",
        repos_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "run artifacts dir",
        instances_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Warn,
    );
    check_dir(
        "batch artifacts dir",
        batches_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );
    check_dir(
        "starting-db cache dir",
        starting_db_cache_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );
    check_dir(
        "cache dir",
        cache_dir()?,
        &mut ok,
        &mut warn,
        &mut note,
        MissingDirStatus::Note,
    );

    match load_model_registry() {
        Ok(registry) => {
            ok += 1;
            println!(
                "[ok] model registry: {} models ({})",
                registry.data.len(),
                model_registry_file()?.display()
            );
        }
        Err(PrepareError::MissingModelRegistry(path)) => {
            warn += 1;
            println!("[warn] model registry: missing ({})", path.display());
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model list",
            ]);
        }
        Err(err) => return Err(err),
    }

    match load_active_model() {
        Ok(active) => match load_model_registry() {
            Ok(registry) => {
                if registry_has_model(&registry, &active.model_id) {
                    ok += 1;
                    println!(
                        "[ok] active model: {} ({})",
                        active.model_id,
                        active_model_file()?.display()
                    );
                } else {
                    warn += 1;
                    println!(
                        "[warn] active model: {} is not present in the current registry ({})",
                        active.model_id,
                        active_model_file()?.display()
                    );
                    print_advice(&[
                        "cargo run -p ploke-eval -- model refresh",
                        "cargo run -p ploke-eval -- model set <model_id>",
                    ]);
                }
            }
            Err(PrepareError::MissingModelRegistry(_)) => {
                warn += 1;
                println!(
                    "[warn] active model: {} ({})",
                    active.model_id,
                    active_model_file()?.display()
                );
            }
            Err(err) => return Err(err),
        },
        Err(PrepareError::MissingActiveModel(path)) => {
            warn += 1;
            println!("[warn] active model: missing ({})", path.display());
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model set <model_id>",
            ]);
        }
        Err(err) => return Err(err),
    }

    match load_parent_patcher_model() {
        Ok(selected) => match load_model_registry() {
            Ok(registry) => {
                if registry_has_model(&registry, &selected.model_id) {
                    ok += 1;
                    println!(
                        "[ok] parent patcher model: {} ({})",
                        selected.model_id,
                        parent_patcher_model_file()?.display()
                    );
                } else {
                    warn += 1;
                    println!(
                        "[warn] parent patcher model: {} is not present in the current registry ({})",
                        selected.model_id,
                        parent_patcher_model_file()?.display()
                    );
                    print_advice(&[
                        "cargo run -p ploke-eval -- model refresh",
                        "cargo run -p ploke-eval -- model parent-patcher set <model_id>",
                    ]);
                }
            }
            Err(PrepareError::MissingModelRegistry(_)) => {
                warn += 1;
                println!(
                    "[warn] parent patcher model: {} ({})",
                    selected.model_id,
                    parent_patcher_model_file()?.display()
                );
            }
            Err(err) => return Err(err),
        },
        Err(PrepareError::MissingParentPatcherModel(path)) => {
            warn += 1;
            println!(
                "[warn] parent patcher model: missing ({}); broad harness will fall back to the active model",
                path.display()
            );
            print_advice(&[
                "cargo run -p ploke-eval -- model refresh",
                "cargo run -p ploke-eval -- model parent-patcher set <model_id>",
            ]);
        }
        Err(err) => return Err(err),
    }

    match OpenRouter::resolve_api_key() {
        Ok(_) => {
            ok += 1;
            println!("[ok] OpenRouter API key: present");
        }
        Err(err) => {
            warn += 1;
            println!("[warn] OpenRouter API key: unavailable ({err})");
        }
    }

    match Google::route_config_available() {
        Ok(_) => {
            ok += 1;
            println!("[ok] Google route config: GOOGLE_PROJECT_ID and GOOGLE_REGION available");
        }
        Err(err) => {
            note += 1;
            println!("[note] Google route config: unavailable ({err})");
        }
    }

    match Google::auth_config_available() {
        Ok(_) => {
            ok += 1;
            println!("[ok] Google auth: ADC or explicit bearer token available");
        }
        Err(err) => {
            note += 1;
            println!("[note] Google auth: unavailable ({err})");
        }
    }

    match std::process::Command::new("git").arg("--version").output() {
        Ok(output) if output.status.success() => {
            ok += 1;
            let version = String::from_utf8_lossy(&output.stdout).trim().to_string();
            println!("[ok] git: {version}");
        }
        Ok(output) => {
            warn += 1;
            println!(
                "[warn] git: command exited with status {}",
                output.status.code().unwrap_or(-1)
            );
        }
        Err(err) => {
            warn += 1;
            println!("[warn] git: unavailable ({err})");
        }
    }

    println!();
    println!(
        "summary: {ok} ok, {warn} warning{}, {note} note{}",
        if warn == 1 { "" } else { "s" },
        if note == 1 { "" } else { "s" }
    );
    Ok(())
}

#[derive(Clone, Copy)]
enum MissingDirStatus {
    Warn,
    Note,
}

fn check_dir(
    label: &str,
    path: PathBuf,
    ok: &mut usize,
    warn: &mut usize,
    note: &mut usize,
    missing_status: MissingDirStatus,
) {
    if path.exists() {
        if path.is_dir() {
            *ok += 1;
            println!("[ok] {label}: {}", path.display());
        } else {
            *warn += 1;
            println!(
                "[warn] {label}: exists but is not a directory ({})",
                path.display()
            );
        }
    } else {
        match missing_status {
            MissingDirStatus::Warn => {
                *warn += 1;
                println!("[warn] {label}: missing ({})", path.display());
            }
            MissingDirStatus::Note => {
                *note += 1;
                println!("[note] {label}: missing ({})", path.display());
            }
        }
    }
}

fn print_advice(lines: &[&str]) {
    for (idx, line) in lines.iter().enumerate() {
        if idx == 0 {
            println!("  next: {line}");
        } else {
            println!("  then: {line}");
        }
    }
}
