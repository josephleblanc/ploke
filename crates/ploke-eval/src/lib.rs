pub mod inner;
pub mod prelude;

pub mod algebra;
pub mod branch_evaluation;
pub mod campaign;
pub mod cli;
pub mod closure;
pub mod intervention;
pub mod intervention_issue_aggregate;
pub mod layout;
pub(crate) mod loop_graph;
pub mod mbe;
pub(crate) mod metric;
pub mod model_registry;
pub mod msb;
pub mod operational_metrics;
pub mod projection;
pub mod protocol;
mod protocol_artifacts;
mod protocol_report;
mod protocol_triage_report;
pub mod provider_prefs;
pub mod record;
pub(crate) mod record_emission;
pub mod registry;
pub mod replay;
pub mod run_history;
pub mod run_registry;
pub mod runner;
pub mod selection;
pub mod spec;
pub(crate) mod successor_selection;
pub mod target_registry;
pub mod tracing_setup;

/// Non-secret Google Cloud project identifier used as the default direct
/// Vertex AI route for ploke-eval live tests.
pub const DEFAULT_GOOGLE_PROJECT_ID: &str = "cs-poc-gtxw7jmtfuwfsiauziui9yx";

/// Default Vertex AI location for ploke-eval direct Google live tests.
pub const DEFAULT_GOOGLE_REGION: &str = "global";

#[cfg(test)]
pub(crate) mod test_support {
    use std::{
        ffi::OsString,
        sync::{Mutex, MutexGuard, OnceLock},
    };

    pub(crate) struct EnvGuard {
        _lock: MutexGuard<'static, ()>,
        previous: Vec<(&'static str, Option<OsString>)>,
    }

    impl Drop for EnvGuard {
        fn drop(&mut self) {
            for (key, value) in self.previous.drain(..).rev() {
                match value {
                    Some(value) => unsafe {
                        std::env::set_var(key, value);
                    },
                    None => unsafe {
                        std::env::remove_var(key);
                    },
                }
            }
        }
    }

    pub(crate) fn env_guard_os(values: Vec<(&'static str, OsString)>) -> EnvGuard {
        let lock = env_lock()
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        let previous = values
            .iter()
            .map(|(key, _)| (*key, std::env::var_os(*key)))
            .collect::<Vec<_>>();
        for (key, value) in &values {
            unsafe {
                std::env::set_var(key, value);
            }
        }
        EnvGuard {
            _lock: lock,
            previous,
        }
    }

    pub(crate) fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    pub(crate) fn llm_lock() -> &'static tokio::sync::Mutex<()> {
        static LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| tokio::sync::Mutex::new(()))
    }

    #[cfg(feature = "live_api_tests")]
    pub(crate) fn install_default_google_route_env() {
        static INIT: OnceLock<()> = OnceLock::new();
        INIT.get_or_init(|| unsafe {
            if std::env::var_os("GOOGLE_PROJECT_ID").is_none() {
                std::env::set_var("GOOGLE_PROJECT_ID", crate::DEFAULT_GOOGLE_PROJECT_ID);
            }
            if std::env::var_os("GOOGLE_REGION").is_none() {
                std::env::set_var("GOOGLE_REGION", crate::DEFAULT_GOOGLE_REGION);
            }
        });
    }

    #[cfg(feature = "live_api_tests")]
    pub(crate) async fn live_google_env_or_skip(test_name: &str) -> bool {
        use ploke_llm::Router;
        use ploke_llm::router_only::google::Google;

        install_default_google_route_env();
        let route_config_available = Google::route_config_available().is_ok();
        let auth_config_available =
            Google::auth_config_available().is_ok() && Google::resolve_bearer_token().await.is_ok();
        if route_config_available && auth_config_available {
            return true;
        }

        let missing = match (route_config_available, auth_config_available) {
            (false, false) => "GOOGLE_PROJECT_ID/GOOGLE_REGION route config and Google ADC auth",
            (false, true) => "GOOGLE_PROJECT_ID/GOOGLE_REGION route config",
            (true, false) => "Google ADC auth",
            (true, true) => unreachable!("handled above"),
        };
        let message = format!(
            "skipping {test_name}: direct Google route is configured for this live test, \
             but missing {missing}; prototype1 did not exercise the live Gemini path"
        );
        if strict_live_tests_requested() {
            panic!("{message}; PLOKE_RUN_LIVE_TESTS requested live execution");
        }
        eprintln!("{message}");
        false
    }

    #[cfg(feature = "live_api_tests")]
    pub(crate) fn strict_live_tests_requested() -> bool {
        std::env::var("PLOKE_RUN_LIVE_TESTS")
            .ok()
            .is_some_and(|value| matches!(value.trim(), "1" | "true" | "TRUE" | "yes" | "YES"))
    }

    #[cfg(feature = "live_api_tests")]
    pub(crate) fn live_google_model_id() -> ploke_llm::ModelId {
        let raw = std::env::var("PLOKE_EVAL_LIVE_GOOGLE_MODEL_ID")
            .or_else(|_| std::env::var("PLOKE_EVAL_HEADLESS_TUI_GOOGLE_MODEL_ID"))
            .or_else(|_| std::env::var("PLOKE_LIVE_GOOGLE_CHAT_MODEL"))
            .unwrap_or_else(|_| "google/gemini-3.5-flash".to_string());
        let model = if raw.contains('/') {
            raw
        } else {
            format!("google/{raw}")
        };
        model.parse().expect("live Google model id")
    }

    #[cfg(feature = "live_api_tests")]
    pub(crate) fn write_direct_google_model_config(
        eval_home: &std::path::Path,
        model_id: &ploke_llm::ModelId,
    ) {
        let models_dir = eval_home.join("models");
        std::fs::create_dir_all(&models_dir).expect("create temp model config dir");
        let model = model_id.to_string();
        let name = model
            .rsplit('/')
            .next()
            .unwrap_or(model.as_str())
            .to_string();
        std::fs::write(
            models_dir.join("registry.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "data": [{
                    "id": model,
                    "name": name,
                    "created": 0,
                    "description": "Direct Google live test row",
                    "architecture": {
                        "input_modalities": ["text"],
                        "modality": "text->text",
                        "output_modalities": ["text"],
                        "tokenizer": "Gemini"
                    },
                    "top_provider": {
                        "is_moderated": false,
                        "context_length": null,
                        "max_completion_tokens": null
                    },
                    "pricing": {
                        "prompt": 0.0,
                        "completion": 0.0
                    },
                    "canonical_slug": model,
                    "context_length": 1048576,
                    "hugging_face_id": null,
                    "per_request_limits": null,
                    "supported_parameters": ["tools"],
                    "route_source": "direct_google"
                }]
            }))
            .expect("serialize direct Google registry"),
        )
        .expect("write direct Google registry");
        crate::model_registry::save_active_model(model_id)
            .expect("save direct Google active model");
        crate::model_registry::save_parent_patcher_model(model_id)
            .expect("save direct Google parent patcher model");
    }

    #[cfg(feature = "live_api_tests")]
    pub(crate) fn live_tempdir(prefix: &str) -> tempfile::TempDir {
        let root = std::env::var_os("PLOKE_EVAL_LIVE_TMP")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| {
                std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                    .join("../../target/tmp/live-api-tests")
            });
        std::fs::create_dir_all(&root).expect("create live test temp root");
        // Keep live eval homes canonical. Closure-state reconstruction matches
        // run registrations by storage-root path identity, so a lexical `..`
        // can make a completed treatment run appear missing.
        let root = root
            .canonicalize()
            .expect("canonicalize live test temp root");
        tempfile::Builder::new()
            .prefix(prefix)
            .tempdir_in(root)
            .expect("create live test tempdir")
    }
}

pub use branch_evaluation::{
    BranchDisposition, BranchEvaluationInput, BranchEvaluationResult, evaluate_branch,
};
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
    batches_dir, campaigns_dir, datasets_dir, instances_dir, ploke_eval_home,
    protocol_artifact_read_dirs_for_run, protocol_artifacts_dir_for_run, protocol_dir,
    registries_dir, repos_dir, workspace_root_for_key,
};
pub use mbe::{
    FinalReport as MbeFinalReport, HarnessConfig as MbeHarnessConfig,
    HarnessInvocation as MbeHarnessInvocation, HarnessRun as MbeHarnessRun, Layout as MbeLayout,
    Mode as MbeMode, Options as MbeOptions, OracleEvidence as MbeOracleEvidence,
    Request as MbeRequest, Verdict as MbeVerdict, Workers as MbeWorkers,
    WrittenConfig as WrittenMbeConfig,
};
pub use msb::{PrepareMsbBatchRequest, PrepareMsbSingleRunRequest};
pub use operational_metrics::{OperationalRunMetrics, PatchApplyState};
pub use record::{
    BuildResult, ConversationMessage, DbState, LlmResponseRecord, NodeInfo, PackagingPhase,
    RUN_RECORD_SCHEMA_VERSION, ReplayError, ReplayState, RunMetadata, RunOutcomeSummary, RunPhases,
    RunRecord, RunRecordBuilder, SubmissionArtifactState, TimeTravelMarker, ToolExecutionRecord,
    ToolResult, TurnOutcome, TurnRecord, ValidationPhase,
};
pub use registry::{
    DatasetRegistryEntry, builtin_dataset_registry_entries, builtin_dataset_registry_entry,
};
pub use run_registry::{
    RunArtifactRefs, RunExecutionStatus, RunLifecycle, RunPhaseLifecycle, RunSelectionPreference,
    RunSubmissionStatus, completed_record_paths_for_instances_root,
    list_registrations_for_instance, load_registration_for_record_path,
    load_registration_for_run_dir, persist_registration, preferred_registration_for_instance,
    register_live_run, storage_roots_for_instance, sync_protocol_registration_status,
};
pub use runner::{
    AgentRunArtifactPaths, AgentTurnArtifact, BatchRunArtifactPaths, RunMsbAgentBatchRequest,
    RunMsbAgentSingleRequest, RunMsbBatchRequest,
};
pub use selection::{
    ActiveSelection, ActiveSelectionSlot, clear_active_selection, load_active_selection,
    render_selection_warnings, save_active_selection, unset_active_selection_slot,
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
