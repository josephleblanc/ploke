pub mod inner;

pub mod campaign;
pub mod cli;
pub mod closure;
pub mod layout;
pub mod model_registry;
pub mod msb;
pub mod protocol;
mod protocol_artifacts;
mod protocol_report;
mod protocol_triage_report;
pub mod provider_prefs;
pub mod record;
pub mod registry;
pub mod run_history;
pub mod run_registry;
pub mod runner;
pub mod spec;
pub mod target_registry;
pub mod tracing_setup;

pub use campaign::{
    CAMPAIGN_MANIFEST_SCHEMA_VERSION, CampaignManifest, CampaignOverrides, CampaignValidationCheck,
    EvalCampaignPolicy, ProtocolCampaignPolicy, ResolvedCampaignConfig, campaign_manifest_path,
    load_campaign_manifest, render_resolved_campaign_config, resolve_campaign_config,
    save_campaign_manifest, validate_campaign_config,
};
pub use cli::Cli;
pub use closure::{
    CLOSURE_STATE_SCHEMA_VERSION, ClosureClass, ClosureConfig, ClosureState,
    DEFAULT_REQUIRED_PROCEDURES, closure_state_path, load_closure_state, recompute_closure_state,
    render_closure_status,
};
pub use layout::{
    batches_dir, campaigns_dir, datasets_dir, ploke_eval_home, protocol_artifacts_dir_for_run,
    registries_dir, repos_dir, runs_dir, workspace_root_for_key,
};
pub use msb::{PrepareMsbBatchRequest, PrepareMsbSingleRunRequest};
pub use record::{
    BuildResult, ConversationMessage, DbState, LlmResponseRecord, NodeInfo,
    RUN_RECORD_SCHEMA_VERSION, RawFullResponseRecord, ReplayError, ReplayState, RunMetadata,
    RunOutcomeSummary, RunPhases, RunRecord, RunRecordBuilder, TimeTravelMarker,
    ToolExecutionRecord, ToolResult, TurnOutcome, TurnRecord, ValidationPhase,
};
pub use registry::{
    DatasetRegistryEntry, builtin_dataset_registry_entries, builtin_dataset_registry_entry,
};
pub use run_registry::{
    RunArtifactRefs, RunExecutionStatus, RunLifecycle, RunPhaseLifecycle, RunSelectionPreference,
    RunSubmissionStatus, completed_record_paths_for_runs_root, list_registrations_for_instance,
    load_registration_for_record_path, load_registration_for_run_dir, persist_registration,
    preferred_registration_for_instance, register_live_run, storage_roots_for_instance,
    sync_protocol_registration_status,
};
pub use runner::{
    AgentRunArtifactPaths, AgentTurnArtifact, BatchRunArtifactPaths, RunMsbAgentBatchRequest,
    RunMsbAgentSingleRequest, RunMsbBatchRequest,
};
pub use spec::{
    EvalBudget, FrameworkConfig, FrameworkToolConfig, IssueInput, MultiSweBenchSource, OutputMode,
    PrepareSingleRunRequest, PrepareWrite, PreparedCampaignContext, PreparedMsbBatch,
    PreparedSingleRun, RunSource,
};
pub use target_registry::{
    BenchmarkFamily, RegistryDatasetSource, RegistryEntry, RegistryEntryState,
    RegistryRecomputeRequest, TARGET_REGISTRY_SCHEMA_VERSION, TargetRegistry, load_target_registry,
    recompute_target_registry, render_target_registry_status, resolve_registry_dataset_sources,
    target_registry_path,
};

#[cfg(test)]
mod tests;
