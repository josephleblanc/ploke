use serde::Serialize;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

mod args;
mod dispatch;
mod format;
mod handlers;
mod intervention;
mod record;
pub mod types;

pub(crate) use handlers::closure::{
    ProtocolNextStep, advance_eval_closure, advance_protocol_closure,
    execute_protocol_intent_segments_quiet, execute_protocol_tool_call_review_quiet,
    execute_protocol_tool_call_segment_review_quiet, print_protocol_state_table,
    protocol_llm_config, protocol_next_command, protocol_state_for_run,
    run_tool_call_review_with_json_retries, run_tool_call_segment_review_with_json_retries,
};
pub(crate) use handlers::inspect::{
    build_segment_review_subject, build_tool_call_review_subject, build_tool_call_sequence_subject,
    join_indices, serde_name,
};
pub(crate) use handlers::protocol::{persist_issue_detection_for_record, print_issue_case_block};
pub(crate) use handlers::prototype1_support::{TimingTrace, pending_prototype1_stages};
pub(crate) use handlers::run::{resolve_batch_manifest, yes_no};
pub(crate) use intervention::{
    persist_intervention_apply_for_record, persist_intervention_synthesis_for_record,
};
mod protocol_route;
mod prototype1_process;
/// Prototype 1 typed state model and persisted artifact map.
///
/// See `prototype1_state::mod` for the implementation split and the on-disk
/// campaign layout under `~/.ploke-eval/campaigns/<campaign-id>/prototype1/`.
pub(crate) mod prototype1_state;
mod provider;

pub use args::*;

pub(crate) use protocol_route::{
    is_retryable_intent_segmentation_error, resolve_protocol_model_id,
    resolve_protocol_provider_slug, resolve_protocol_route,
    tool_call_intent_segmentation_error_to_prepare, tool_call_review_error_to_prepare,
    tool_call_segment_review_error_to_prepare, truncate_for_table, truncate_middle,
};

pub(crate) use record::{
    has_legacy_instance_root_artifacts, legacy_instance_root_warning, list_attempt_registrations,
    print_record_resolution_footer, print_selection_update, resolve_record_path,
    sanitize_batch_component, selection_with_resolution,
};

#[cfg(test)]
#[path = "cli/tests.rs"]
mod tests;

pub(crate) const PROTOCOL_HTTP_MAX_ATTEMPTS: u32 = 1;
pub(crate) const PROTOCOL_JSON_REVIEW_MAX_ATTEMPTS: usize = 3;
pub(crate) const TOOL_CALL_REVIEW_TIMEOUT_SECS: u64 = ploke_llm::LLM_TIMEOUT_SECS;

use crate::campaign::{CampaignOverrides, campaign_manifest_path, resolve_campaign_config};
use crate::closure::load_closure_state;
use crate::registry::builtin_dataset_registry_entries;
use crate::spec::PrepareError;

#[cfg(test)]
use crate::campaign::{
    CampaignManifest, CampaignValidationCheck, EvalCampaignPolicy, ProtocolCampaignPolicy,
    ResolvedCampaignConfig, adopt_campaign_manifest_from_closure_state,
    adopt_campaign_manifest_from_registry, apply_campaign_overrides, campaign_closure_state_path,
    dataset_files_from_sources, dataset_keys_from_sources, default_protocol_max_tokens,
    list_campaigns, render_resolved_campaign_config, save_campaign_manifest,
    validate_campaign_config,
};
#[cfg(test)]
use crate::closure::{
    ClosureClass, ClosureRecomputeRequest, closure_state_path, recompute_closure_state,
    render_closure_status,
};
#[cfg(test)]
use crate::inner::registry::RunRegistration;
#[cfg(test)]
use crate::intervention::{
    INTERVENTION_APPLY_PROCEDURE, INTERVENTION_ISSUE_DETECTION_PROCEDURE,
    INTERVENTION_SYNTHESIS_PROCEDURE, InterventionApplyInput, InterventionApplyOutput,
    InterventionSynthesisInput, IssueCase, IssueDetectionInput, IssueDetectionOutput,
    detect_issue_cases, execute_intervention_apply, issue_detection_artifact_input,
    operation_target_artifact_id, select_primary_issue, synthesize_intervention_with_llm,
};
#[cfg(test)]
use crate::intervention_issue_aggregate::{
    IssueDetectionAggregate, IssueDetectionAggregateError, load_issue_detection_aggregate,
};
#[cfg(test)]
use crate::model_registry::load_active_model;
#[cfg(test)]
use crate::msb::{PrepareMsbBatchRequest, PrepareMsbSingleRunRequest};
#[cfg(test)]
use crate::projection::OperatorProjectionRead;
#[cfg(test)]
use crate::protocol::protocol_aggregate::{
    ProtocolAggregate, ProtocolAggregateError, ProtocolCallReviewRow, load_protocol_aggregate,
};
#[cfg(test)]
use crate::protocol_artifacts::{
    StoredProtocolArtifactFile, list_protocol_artifact_load_results, list_protocol_artifacts,
    load_protocol_artifact, protocol_artifact_preview, protocol_artifact_summary,
    write_protocol_artifact,
};
#[cfg(test)]
use crate::protocol_report::{
    ProtocolAggregateCallIssueRow, ProtocolAggregateCoverage, ProtocolAggregateReport,
    ProtocolAggregateSegmentRow, ProtocolColorProfile, ProtocolReportRenderOptions,
    render_protocol_aggregate_report_with_options,
};
#[cfg(test)]
use crate::protocol_triage_report::{
    ProtocolCampaignCountRow, ProtocolCampaignEvidence, ProtocolCampaignExemplarRow,
    ProtocolCampaignFamilyRow, ProtocolCampaignSummary, ProtocolCampaignTriageReport,
    render_protocol_campaign_triage_report, sort_count_rows,
};
#[cfg(test)]
use crate::provider_prefs::{
    clear_provider_for_model, load_provider_for_model, set_provider_for_model,
};
#[cfg(test)]
use crate::record::read_compressed_record;
#[cfg(test)]
use crate::registry::builtin_dataset_registry_entry;
#[cfg(test)]
use crate::run_history::{
    RunDirPreference, list_finished_record_paths_in_instances_root, preferred_run_dir_for_instance,
    print_assistant_messages_from_record_path,
};
#[cfg(test)]
use crate::run_registry::list_registrations_for_instance;
#[cfg(test)]
use crate::runner::{
    BatchRunArtifactPaths, BatchRunSummary, MultiSweBenchSubmissionRecord, ReplayMsbBatchRequest,
    RunMsbAgentBatchRequest, RunMsbAgentSingleRequest, RunMsbBatchRequest, RunMsbSingleRequest,
    resolve_route_for_model,
};
#[cfg(test)]
use crate::selection::{
    ActiveSelection, ActiveSelectionSlot, clear_active_selection, load_active_selection,
    load_active_selection_at, render_selection_warnings, save_active_selection,
    unset_active_selection_slot,
};
#[cfg(test)]
use crate::spec::{
    EvalBudget, IssueInput, OutputMode, PrepareSingleRunRequest, PrepareWrite,
    PreparedCampaignContext,
};
#[cfg(test)]
use crate::target_registry::{
    BenchmarkFamily, RegistryEntry, RegistryRecomputeRequest, TargetRegistry, load_target_registry,
    recompute_target_registry, render_target_registry_status, target_registry_path,
};

#[cfg(test)]
pub(crate) use format::{
    display_context_length, display_price_per_million, extract_model_size, model_size_string,
};
#[cfg(test)]
pub(crate) use handlers::campaign::default_campaign_submission_export_path;
#[cfg(test)]
pub(crate) use handlers::closure::{
    ClosureAdvanceProtocolReport, ProtocolRunPlan, advance_protocol_or_block,
    ensure_prepared_runs_under_repo_cache, ensure_repo_cache_clone_preflight,
    format_protocol_selected_runs, format_remaining_protocol_work,
    is_retryable_local_analysis_review_error, protocol_report_allows_continue,
    protocol_report_made_progress, protocol_run_plan,
};
#[cfg(test)]
pub(crate) use handlers::inspect::{
    abbreviate_path_tail, extract_argument_string, render_messages_json, render_messages_table,
    render_payload_block, render_tool_inputs, render_tool_loop_table, summarize_failure_reason,
    summarize_patch_state, summarize_tool_inputs, tool_call_next_step_index,
};
#[cfg(test)]
pub(crate) use handlers::registry::registry_dataset_view;
#[cfg(test)]
pub(crate) use handlers::run::default_batch_id;
#[cfg(test)]
pub(crate) use provider::{
    current_provider_for_model, headless_model_selection,
    headless_model_selection_from_provider_preference, load_parent_patcher_model_selection,
    parse_provider_key, registry_route_source, resolve_provider_model_id,
};
#[cfg(test)]
pub(crate) use record::{RecordResolution, resolve_record_path_from_eval_home};

#[cfg(test)]
use clap::Parser;
#[cfg(test)]
use ploke_llm::request::models::ModelRouteSource;
#[cfg(test)]
use ploke_llm::{ModelId, ProviderKey};
#[cfg(test)]
use ploke_protocol::ProtocolReasoningPolicy;
#[cfg(test)]
use ploke_protocol::tool_calls::review;
#[cfg(test)]
use ploke_records::tool_contracts::ToolArgumentsJson;
#[cfg(test)]
use serde::Deserialize;
#[cfg(test)]
use std::collections::BTreeMap;
#[cfg(test)]
use std::sync::Mutex;

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
