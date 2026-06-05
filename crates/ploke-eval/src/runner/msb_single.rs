use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::Database;
use ploke_db::bm25_index::bm25_service::Bm25Status;
use ploke_db::multi_embedding::db_ext::EmbeddingExt;
use ploke_embed::config::{OpenRouterConfig, TruncatePolicy};
use ploke_embed::indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexingStatus};
use ploke_embed::providers::openrouter::OpenRouterBackend;
use ploke_llm::embeddings::{
    EmbClientConfig, EmbeddingInput, EmbeddingRequest, HasDims, HasEmbeddings,
};
use ploke_llm::manager::RequestMessage;
use ploke_llm::request::{endpoint::Endpoint, models::ResponseItem};
use ploke_llm::router_only::{
    HasEndpoint,
    openrouter::{
        EmbeddingProviderPrefs, OpenRouter, OpenRouterModelId, ProviderPreferences,
        embed::OpenRouterEmbeddingFields,
    },
};
use ploke_llm::{LlmRoute, ModelId, ProviderKey, ProviderSlug, SupportsTools};
use ploke_records::agent_turn::{
    AgentTurnArtifactRecord as PersistedAgentTurnArtifactRecord, AgentTurnSummaryRecord,
    AgentTurnTraceRecord, ExpectedFileChangeRecord as PersistedExpectedFileChangeRecord,
    FinishReasonRecord, FunctionCallMarker, LlmMetadataRecord as PersistedLlmMetadataRecord,
    LlmResponseRecord as PersistedLlmResponseRecord,
    MessageSnapshotRecord as PersistedMessageSnapshotRecord, ObservedTurnEventRecord,
    PatchArtifactRecord as PersistedPatchArtifactRecord,
    PerformanceMetricsRecord as PersistedPerformanceMetricsRecord,
    ProposalSnapshotRecord as PersistedProposalSnapshotRecord, ProviderFunctionCallRecord,
    ProviderToolCallRecord, RequestMessageRecord as PersistedRequestMessageRecord,
    RequestRoleRecord, TokenUsageRecord as PersistedTokenUsageRecord,
    ToolCompletedRecord as PersistedToolCompletedRecord, ToolErrorCodeRecord, ToolErrorWireRecord,
    ToolFailedRecord as PersistedToolFailedRecord, ToolLlmErrorPayloadRecord,
    ToolRequestRecord as PersistedToolRequestRecord, ToolRetryContextFieldRecord,
    ToolRetryContextRecord, ToolRetryContextValueRecord, ToolUiFieldRecord, ToolUiPayloadRecord,
    ToolVerbosityRecord, TurnFinishedRecord as PersistedTurnFinishedRecord,
};
use ploke_records::evaluation::{
    BENCHMARK_PATCH_PROJECTION_SCHEMA_V1, BenchmarkCheckoutRef, BenchmarkPatchProjectionRecord,
    MultiSweBenchTarget, PatchProjectionCheck, PatchProjectionCheckState, RunArtifactRef,
    SubmissionPatchRef,
};
use ploke_records::record::ToRecord;
use ploke_records::tool_contracts::{PersistedToolCallArguments, ToolCallArguments};
use ploke_tui::AppEvent;
use ploke_tui::app::App;
use ploke_tui::app::commands::harness::TestAppAccessor;
use ploke_tui::app::commands::harness::{TestRuntime, TestRuntimeActorGuard};
use ploke_tui::app::view::components::model_browser::tool_capable_provider_key;
use ploke_tui::app_state::AppState;
use ploke_tui::app_state::core::ParseFailure;
use ploke_tui::app_state::core::{DiffPreview, EditProposalStatus, RuntimeConfig};
use ploke_tui::app_state::events::SystemEvent;
use ploke_tui::llm::{ChatEvt, LlmEvent};
use ploke_tui::parser::{resolve_index_target, run_parse_resolved};
use ploke_tui::user_config::{
    ChatPolicy, ChatTimeoutStrategy, RetrievalStrategyUser, ToolLoopMode,
};
use ploke_tui::utils::parse_errors::FlattenedParserDiagnostic;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tokio::sync::broadcast;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Instant, sleep};
use tracing::{info, warn};
use uuid::Uuid;

use crate::LlmResponseRecord;
use crate::inner::core::{RegisteredRunRole, RunIntent};
use crate::inner::registry::{RunLifecyclePhase, RunPhaseStatus, RunRegistration};
use crate::layout;
use crate::model_registry::resolve_model_for_run;
use crate::provider_prefs::load_provider_for_model;
use crate::record::{
    CrateIndexStatus, IndexedCrateSummary, PackagingPhase, ParseErrorSummary, ParseFailureRecord,
    RunRecord, RunRecordBuilder, RunTimingSummary, SetupPhase, SubmissionArtifactState,
    write_compressed_record,
};
use crate::run_history::record_last_run;
use crate::run_registry::{persist_registration, register_live_run, storage_roots_for_instance};
use crate::spec::{PrepareError, PreparedMsbBatch, PreparedSingleRun, RunSource};
use crate::tracing_setup::current_full_response_log_path;

use super::artifacts::*;
use super::*;

impl RunMsbSingleRequest {
    pub async fn run(self) -> Result<RunArtifactPaths, PrepareError> {
        let run_arm = RunArm::shell_only_control();
        let setup_start_time = chrono::Utc::now();
        let run_start_instant = Instant::now();
        let (manifest_path, prepared) = load_prepared_run(self.run_manifest)?;
        let embedding_selection = resolve_eval_embedding_selection(None, None).await?;
        let requested_model = parse_requested_model_id(self.model_id.as_deref())?;
        let selected_model =
            resolve_model_for_run(requested_model.as_ref(), self.use_default_model)?;
        let selected_model_id = selected_model.id.clone();
        let preferred_provider =
            load_provider_preference_for_selected_model(&selected_model, self.provider.as_ref())?;
        let requested_provider = provider_request_for_selected_model(
            &selected_model,
            self.provider.as_ref(),
            preferred_provider.as_ref(),
        );
        let route = resolve_route_for_model(&selected_model, requested_provider).await?;
        let selected_provider = route.selected_provider_slug();
        let selected_endpoint = selected_endpoint_provenance(&route);

        fs::create_dir_all(&prepared.output_dir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: prepared.output_dir.clone(),
                source,
            }
        })?;
        let run_output_dir = allocate_run_output_dir(&prepared.output_dir, &run_arm)?;
        fs::create_dir_all(&run_output_dir).map_err(|source| PrepareError::CreateOutputDir {
            path: run_output_dir.clone(),
            source,
        })?;

        let execution_log_path = run_output_dir.join("execution-log.json");
        let repo_state_path = run_output_dir.join("repo-state.json");
        let indexing_status_path = run_output_dir.join("indexing-status.json");
        let parse_failure_path = run_output_dir.join("parse-failure.json");
        let snapshot_status_path = run_output_dir.join("snapshot-status.json");
        let indexing_checkpoint_db = run_output_dir.join("indexing-checkpoint.db");
        let indexing_failure_db = run_output_dir.join("indexing-failure.db");
        let record_path = run_output_dir.join("record.json.gz");

        let mut run_record = RunRecord::new(&prepared, run_arm.clone());
        run_record.metadata.agent.model_id = Some(selected_model_id.clone());
        run_record.metadata.agent.provider = Some(selected_provider.clone());
        run_record.metadata.agent.selected_endpoint = selected_endpoint.clone();
        let mut registration = register_run_attempt(
            &prepared,
            &manifest_path,
            &run_arm,
            &run_output_dir,
            &selected_model_id,
            &selected_provider,
            self.batch_id.clone(),
        )?;
        registration.mark_execution_started(Some("run setup started".to_string()));
        persist_registration(&registration)?;

        let result: Result<RunArtifactPaths, PrepareError> = async {
            let mut steps = vec!["load_manifest".to_string()];
            checkout_repo_to_base(&prepared.repo_root, prepared.base_sha.as_deref())?;
            steps.push("checkout_base_sha".to_string());

            let repo_state = RepoStateArtifact {
                repo_root: prepared.repo_root.clone(),
                requested_base_sha: prepared.base_sha.clone(),
                checked_out_head_sha: git_stdout(
                    &prepared.repo_root,
                    &["rev-parse", "HEAD"],
                    "git rev-parse HEAD",
                )?
                .map(|s| s.trim().to_string()),
                git_status_porcelain: git_stdout(
                    &prepared.repo_root,
                    &["status", "--short"],
                    "git status --short",
                )?
                .unwrap_or_default(),
            };
            write_json(&repo_state_path, &repo_state)?;
            steps.push("write_repo_state".to_string());

            let cached_starting_db = match load_cached_starting_db(&prepared, &embedding_selection) {
                Ok(cached) => cached,
                Err(err) => {
                    warn!(error = %err, "runner phase: starting db cache lookup failed; falling back to fresh indexing");
                    None
                }
            };
            let mut using_cached_starting_db = false;
            let runtime_db = if let Some(cache_paths) = cached_starting_db.as_ref() {
                match Database::create_new_backup_default(&cache_paths.snapshot).await {
                    Ok(db) => {
                        info!(
                            snapshot = %cache_paths.snapshot.display(),
                            "runner phase: restoring cached starting db snapshot"
                        );
                        using_cached_starting_db = true;
                        steps.push("restore_cached_starting_db".to_string());
                        Arc::new(db)
                    }
                    Err(err) => {
                        warn!(
                            snapshot = %cache_paths.snapshot.display(),
                            error = %err,
                            "runner phase: cached starting db restore failed; falling back to fresh indexing"
                        );
                        let db = init_runtime_db()?;
                        steps.push("init_runtime_db".to_string());
                        db
                    }
                }
            } else {
                let db = init_runtime_db()?;
                steps.push("init_runtime_db".to_string());
                db
            };

            let config_home = run_output_dir.join("config");
            fs::create_dir_all(&config_home).map_err(|source| PrepareError::CreateOutputDir {
                path: config_home.clone(),
                source,
            })?;
            let _config_guard = XdgConfigHomeGuard::set_to(&config_home);
            steps.push("sandbox_config_home".to_string());

            steps.push("embedding_model_preflight".to_string());

            let embedding_processor = eval_embedding_processor(&embedding_selection)?;
            steps.push("init_codestral_embedder".to_string());

            let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, embedding_processor)
                .spawn_file_manager()
                .spawn_state_manager()
                .spawn_event_bus();
            let events = runtime
                .events_builder()
                .build_event_bus_only()
                .event_bus_events;
            let mut realtime_rx = events.realtime_tx_rx;
            let mut background_rx = events.background_tx_rx;
            let mut index_rx =
                Arc::try_unwrap(events.index_tx_rx).map_err(|_| PrepareError::DatabaseSetup {
                    phase: "subscribe_index_status",
                    detail: "index receiver unexpectedly shared".to_string(),
                })?;
            let state = runtime.state_arc();
            {
                let mut cfg = state.config.write().await;
                configure_eval_model_runtime(&mut cfg, &route);
            }
            info!("runner phase: inspect active embedding set before activation");
            let currently_active_set: EmbeddingSet = runtime_db
                .with_active_set(|set| set.clone())
                .expect("active embedding set");
            info!(
                ?currently_active_set,
                "active embedding set before activation"
            );

            activate_eval_embedding_runtime(&state, &embedding_selection)?;

            let currently_active_set: EmbeddingSet = runtime_db
                .with_active_set(|set| set.clone())
                .expect("active embedding set");
            info!(
                ?currently_active_set,
                "active embedding set after activation"
            );
            steps.push("activate_codestral_embedding_set".to_string());
            let mut app = runtime
                .into_app_with_state_pwd(prepared.repo_root.clone())
                .await;
            steps.push("bootstrap_headless_runtime".to_string());

            if using_cached_starting_db {
                info!(
                    task_id = %prepared.task_id,
                    repo_root = %prepared.repo_root.display(),
                    "runner phase: cached starting db present, skipping indexing"
                );
                seed_loaded_workspace_from_repo(&state, &prepared).await?;
                steps.push("seed_loaded_workspace_from_repo".to_string());
                app.pump_pending_events().await;
                steps.push("pump_post_cached_workspace_events".to_string());
            } else {
                info!(task_id = %prepared.task_id, repo_root = %prepared.repo_root.display(), "runner phase: start indexing");
                app.run_command_text("/index").await;
                steps.push("run_index_command".to_string());

                info!("runner phase: waiting for indexing completion");
                let mut last_index_progress = None;
                if let Err(err) = wait_for_indexing_completion(
                    &mut app,
                    &mut realtime_rx,
                    &mut background_rx,
                    &mut index_rx,
                    Arc::clone(&state.db),
                    indexing_checkpoint_db.clone(),
                    indexing_failure_db.clone(),
                    self.index_debug_snapshots,
                    &mut last_index_progress,
                )
                .await
                {
                    persist_indexing_failure_status(
                        &indexing_status_path,
                        &err,
                        last_index_progress,
                    );
                    persist_parse_failure_artifact(&state, &parse_failure_path).await;
                    return Err(err);
                }
                steps.push("indexing_completed".to_string());
                info!("runner phase: indexing completed");
                app.pump_pending_events().await;
                steps.push("pump_post_index_events".to_string());
            }

            let indexing_status = IndexingStatusArtifact {
                status: "completed".to_string(),
                detail: if using_cached_starting_db {
                    "Loaded cached starting db snapshot and skipped reindexing.".to_string()
                } else {
                    "Indexing completed through the full app command path.".to_string()
                },
                last_progress: None,
            };
            write_json(&indexing_status_path, &indexing_status)?;
            steps.push("write_indexing_status".to_string());

            let setup_phase = build_setup_phase(
                &state.db,
                &repo_state,
                &indexing_status,
                setup_start_time,
                using_cached_starting_db,
                &parse_failure_path,
            )
            .await?;
            run_record.phases.setup = Some(setup_phase);
            steps.push("populate_setup_phase".to_string());
            registration.update_phase(
                RunLifecyclePhase::Setup,
                RunPhaseStatus::Completed,
                Some(indexing_status.detail.clone()),
            );
            registration.update_phase(
                RunLifecyclePhase::Patching,
                RunPhaseStatus::Skipped,
                Some("setup-only control run".to_string()),
            );
            persist_registration(&registration)?;

            persist_db_snapshot(
                Arc::clone(&state.db),
                indexing_checkpoint_db.clone(),
                "starting snapshot checkpoint",
            )
            .await?;
            steps.push("write_indexing_checkpoint".to_string());

            if !using_cached_starting_db {
                if let Err(err) = persist_starting_db_cache(
                    &prepared,
                    &embedding_selection,
                    &indexing_checkpoint_db,
                )
                .await
                {
                    warn!(
                        snapshot = %indexing_checkpoint_db.display(),
                        error = %err,
                        "runner phase: failed to refresh starting db cache"
                    );
                }
                steps.push("refresh_starting_db_cache".to_string());
            }

            let setup_wall_clock_secs = Some(run_start_instant.elapsed().as_secs_f64());

            let snapshot_file = run_output_dir.join("final-snapshot.db");
            info!(
                snapshot = %snapshot_file.display(),
                "runner phase: persisting final eval snapshot"
            );
            persist_db_snapshot(
                Arc::clone(&state.db),
                snapshot_file.clone(),
                "final snapshot",
            )
            .await?;
            steps.push("snapshot_completed".to_string());
            info!(snapshot_file = %snapshot_file.display(), "runner phase: snapshot completed");

            let snapshot_status = SnapshotStatusArtifact {
                status: "completed".to_string(),
                snapshot_file: Some(snapshot_file.clone()),
                registry_file: snapshot_file.clone(),
                config_home,
            };
            write_json(&snapshot_status_path, &snapshot_status)?;
            steps.push("write_snapshot_status".to_string());

            registration.update_phase(
                RunLifecyclePhase::Packaging,
                RunPhaseStatus::InProgress,
                Some("writing benchmark submission artifact".to_string()),
            );
            persist_registration(&registration)?;

            let packaging_started_at = chrono::Utc::now().to_rfc3339();
            let msb_submission_artifact = write_msb_submission_artifact(
                &prepared,
                &run_arm,
                &run_output_dir,
                &manifest_path,
                &record_path,
                None,
            )?;
            if let Some(submission) = msb_submission_artifact.as_ref() {
                steps.push("write_msb_submission".to_string());
                steps.push("write_benchmark_patch_projection".to_string());
                registration.artifacts.patch_projection =
                    Some(submission.patch_projection_path.clone());
                registration.update_submission_status(Some(&submission.fix_patch));
                registration.update_phase(
                    RunLifecyclePhase::Packaging,
                    RunPhaseStatus::Completed,
                    Some("submission artifact written".to_string()),
                );
            } else {
                registration.update_submission_status(None);
                registration.update_phase(
                    RunLifecyclePhase::Packaging,
                    RunPhaseStatus::Skipped,
                    Some("no benchmark packaging for this run".to_string()),
                );
            }
            run_record.phases.packaging = Some(PackagingPhase {
                started_at: packaging_started_at,
                ended_at: chrono::Utc::now().to_rfc3339(),
                submission_artifact_state: match msb_submission_artifact.as_ref() {
                    Some(submission) if submission.fix_patch.trim().is_empty() => {
                        SubmissionArtifactState::Empty
                    }
                    Some(_) => SubmissionArtifactState::Nonempty,
                    None => SubmissionArtifactState::NotApplicable,
                },
                msb_submission_path: msb_submission_artifact
                    .as_ref()
                    .map(|artifact| artifact.path.clone()),
                patch_projection_path: msb_submission_artifact
                    .as_ref()
                    .map(|artifact| artifact.patch_projection_path.clone()),
                patch_projection_check_state: msb_submission_artifact
                    .as_ref()
                    .map(|artifact| artifact.patch_projection_check_state)
                    .unwrap_or(PatchProjectionCheckState::NotApplicable),
            });
            registration.update_phase(
                RunLifecyclePhase::Validation,
                RunPhaseStatus::Skipped,
                Some("validation not executed in setup-only run".to_string()),
            );
            persist_registration(&registration)?;

            finalize_run_timing(
                &mut run_record,
                setup_start_time,
                run_start_instant,
                setup_wall_clock_secs,
                None,
            );

            let execution_log = ExecutionLog {
                task_id: prepared.task_id.clone(),
                run_arm: run_arm.clone(),
                repo_root: prepared.repo_root.clone(),
                output_dir: run_output_dir.clone(),
                selected_model: selected_model_id.clone(),
                selected_provider: Some(selected_provider.clone()),
                selected_endpoint: selected_endpoint.clone(),
                full_response_trace: None,
                steps,
            };
            write_json(&execution_log_path, &execution_log)?;
            info!(
                execution_log = %execution_log_path.display(),
                repo_state = %repo_state_path.display(),
                indexing_status = %indexing_status_path.display(),
                indexing_checkpoint_db = %indexing_checkpoint_db.display(),
                indexing_failure_db = %indexing_failure_db.display(),
                snapshot_status = %snapshot_status_path.display(),
                msb_submission = msb_submission_artifact.as_ref().map(|artifact| artifact.path.display().to_string()),
                "runner phase: wrote run artifacts"
            );
            record_last_run(&execution_log.output_dir)?;

            if let Err(e) = write_compressed_record(&record_path, &run_record) {
                warn!(
                    path = %record_path.display(),
                    error = %e,
                    "runner phase: failed to write compressed run record"
                );
            } else {
                info!(path = %record_path.display(), "runner phase: wrote compressed run record");
            }

            Ok(RunArtifactPaths {
                run_manifest: manifest_path.clone(),
                execution_log: execution_log_path.clone(),
                repo_state: repo_state_path.clone(),
                indexing_status: indexing_status_path.clone(),
                indexing_checkpoint_db: indexing_checkpoint_db.clone(),
                indexing_failure_db: indexing_failure_db.clone(),
                snapshot_status: snapshot_status_path.clone(),
                msb_submission: msb_submission_artifact.map(|artifact| artifact.path),
                patch_projection: registration.artifacts.patch_projection.clone(),
                validation_audit: None,
                record_path: Some(record_path.clone()),
                full_response_trace: None,
            })
        }
        .await;

        match result {
            Ok(artifacts) => {
                registration.mark_completed();
                persist_registration(&registration)?;
                Ok(artifacts)
            }
            Err(err) => {
                registration.mark_failed(err.to_string());
                persist_registration(&registration)?;
                Err(err)
            }
        }
    }
}

impl RunMsbAgentSingleRequest {
    pub async fn run(self) -> Result<AgentRunArtifactPaths, PrepareError> {
        let setup_start_time = chrono::Utc::now();
        let run_start_instant = Instant::now();
        let run_arm = RunArm::structured_current_policy_treatment();
        let (manifest_path, prepared) = load_prepared_run(self.run_manifest)?;
        let embedding_selection = resolve_eval_embedding_selection(
            self.embedding_model_id.as_deref(),
            self.embedding_provider.as_ref(),
        )
        .await?;
        let requested_model = parse_requested_model_id(self.model_id.as_deref())?;
        let selected_model =
            resolve_model_for_run(requested_model.as_ref(), self.use_default_model)?;
        let selected_model_id = selected_model.id.clone();
        let preferred_provider =
            load_provider_preference_for_selected_model(&selected_model, self.provider.as_ref())?;
        let requested_provider = provider_request_for_selected_model(
            &selected_model,
            self.provider.as_ref(),
            preferred_provider.as_ref(),
        );
        let route = resolve_route_for_model(&selected_model, requested_provider).await?;
        let selected_provider = route.selected_provider_slug();
        let selected_endpoint = selected_endpoint_provenance(&route);

        fs::create_dir_all(&prepared.output_dir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: prepared.output_dir.clone(),
                source,
            }
        })?;
        let run_output_dir = allocate_run_output_dir(&prepared.output_dir, &run_arm)?;
        fs::create_dir_all(&run_output_dir).map_err(|source| PrepareError::CreateOutputDir {
            path: run_output_dir.clone(),
            source,
        })?;

        let execution_log_path = run_output_dir.join("execution-log.json");
        let repo_state_path = run_output_dir.join("repo-state.json");
        let indexing_status_path = run_output_dir.join("indexing-status.json");
        let parse_failure_path = run_output_dir.join("parse-failure.json");
        let snapshot_status_path = run_output_dir.join("snapshot-status.json");
        let indexing_checkpoint_db = run_output_dir.join("indexing-checkpoint.db");
        let indexing_failure_db = run_output_dir.join("indexing-failure.db");
        let turn_trace_path = run_output_dir.join("agent-turn-trace.json");
        let turn_summary_path = run_output_dir.join("agent-turn-summary.json");
        let validation_audit_path = run_output_dir.join(VALIDATION_AUDIT_FILE);
        let full_response_trace_path = run_output_dir.join("llm-full-responses.jsonl");
        let record_path = run_output_dir.join("record.json.gz");
        let full_response_trace_source = current_full_response_log_path().map(Path::to_path_buf);
        let full_response_trace_start = match full_response_trace_source.as_ref() {
            Some(path) => Some(current_trace_file_offset(path)?),
            None => None,
        };

        let mut run_record = RunRecord::new(&prepared, run_arm.clone());
        run_record.metadata.agent.model_id = Some(selected_model_id.clone());
        run_record.metadata.agent.provider = Some(selected_provider.clone());
        run_record.metadata.agent.selected_endpoint = selected_endpoint.clone();
        let mut registration = register_run_attempt(
            &prepared,
            &manifest_path,
            &run_arm,
            &run_output_dir,
            &selected_model_id,
            &selected_provider,
            self.batch_id.clone(),
        )?;
        registration.artifacts.turn_trace = Some(turn_trace_path.clone());
        registration.artifacts.turn_summary = Some(turn_summary_path.clone());
        registration.artifacts.validation_audit = Some(validation_audit_path.clone());
        registration.artifacts.full_response_trace = Some(full_response_trace_path.clone());
        registration.mark_execution_started(Some("run setup started".to_string()));
        persist_registration(&registration)?;

        let result: Result<AgentRunArtifactPaths, PrepareError> = async {
            let mut steps = vec!["load_manifest".to_string()];
            checkout_repo_to_base(&prepared.repo_root, prepared.base_sha.as_deref())?;
            steps.push("checkout_base_sha".to_string());
            let expected_file_baselines =
                snapshot_expected_files(&prepared.repo_root, &expected_patch_files(&prepared))?;
            steps.push("snapshot_expected_files_before_turn".to_string());

            let repo_state = RepoStateArtifact {
                repo_root: prepared.repo_root.clone(),
                requested_base_sha: prepared.base_sha.clone(),
                checked_out_head_sha: git_stdout(
                    &prepared.repo_root,
                    &["rev-parse", "HEAD"],
                    "git rev-parse HEAD",
                )?
                .map(|s| s.trim().to_string()),
                git_status_porcelain: git_stdout(
                    &prepared.repo_root,
                    &["status", "--short"],
                    "git status --short",
                )?
                .unwrap_or_default(),
            };
            write_json(&repo_state_path, &repo_state)?;
            steps.push("write_repo_state".to_string());

            let cached_starting_db = match load_cached_starting_db(&prepared, &embedding_selection) {
                Ok(cached) => cached,
                Err(err) => {
                    warn!(error = %err, "runner phase: starting db cache lookup failed; falling back to fresh indexing");
                    None
                }
            };
            let mut using_cached_starting_db = false;
            let runtime_db = if let Some(cache_paths) = cached_starting_db.as_ref() {
                match Database::create_new_backup_default(&cache_paths.snapshot).await {
                    Ok(db) => {
                        info!(
                            snapshot = %cache_paths.snapshot.display(),
                            "runner phase: restoring cached starting db snapshot"
                        );
                        using_cached_starting_db = true;
                        steps.push("restore_cached_starting_db".to_string());
                        Arc::new(db)
                    }
                    Err(err) => {
                        warn!(
                            snapshot = %cache_paths.snapshot.display(),
                            error = %err,
                            "runner phase: cached starting db restore failed; falling back to fresh indexing"
                        );
                        let db = init_runtime_db()?;
                        steps.push("init_runtime_db".to_string());
                        db
                    }
                }
            } else {
                let db = init_runtime_db()?;
                steps.push("init_runtime_db".to_string());
                db
            };

            let config_home = run_output_dir.join("config");
            fs::create_dir_all(&config_home).map_err(|source| PrepareError::CreateOutputDir {
                path: config_home.clone(),
                source,
            })?;
            let _config_guard = XdgConfigHomeGuard::set_to(&config_home);
            steps.push("sandbox_config_home".to_string());

            steps.push("embedding_model_preflight".to_string());

            let embedding_processor = eval_embedding_processor(&embedding_selection)?;
            steps.push("init_codestral_embedder".to_string());

            let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, embedding_processor)
                .spawn_file_manager()
                .spawn_state_manager()
                .spawn_event_bus()
                .spawn_llm_manager()
                .spawn_observability();
            let events = runtime.events_builder().build_all();
            let mut realtime_rx = events.event_bus_events.realtime_tx_rx;
            let mut background_rx = events.event_bus_events.background_tx_rx;
            let mut index_rx =
                Arc::try_unwrap(events.event_bus_events.index_tx_rx).map_err(|_| {
                    PrepareError::DatabaseSetup {
                        phase: "subscribe_index_status",
                        detail: "index receiver unexpectedly shared".to_string(),
                    }
                })?;
            let mut debug_rx =
                events
                    .app_actor_events
                    .debug_string_rx
                    .ok_or_else(|| PrepareError::DatabaseSetup {
                        phase: "subscribe_debug_string",
                        detail: "missing debug string receiver".to_string(),
                    })?;
            let state = runtime.state_arc();
            {
                let mut cfg = state.config.write().await;
                configure_headless_benchmark_chat(&mut cfg, &route);
            }
            info!("runner phase: inspect active embedding set before activation");
            let currently_active_set: EmbeddingSet = runtime_db
                .with_active_set(|set| set.clone())
                .expect("active embedding set");
            info!(
                ?currently_active_set,
                "active embedding set before activation"
            );

            activate_eval_embedding_runtime(&state, &embedding_selection)?;

            #[cfg(feature = "demo")]
            let _terminal_app = runtime
                .spawn_terminal_app(prepared.repo_root.clone())
                .await;

            let currently_active_set: EmbeddingSet = runtime_db
                .with_active_set(|set| set.clone())
                .expect("active embedding set");
            info!(
                ?currently_active_set,
                "active embedding set after activation"
            );
            steps.push("activate_codestral_embedding_set".to_string());
            let mut app = runtime
                .into_app_with_state_pwd(prepared.repo_root.clone())
                .await;
            steps.push("bootstrap_headless_runtime".to_string());

            if using_cached_starting_db {
                info!(
                    task_id = %prepared.task_id,
                    repo_root = %prepared.repo_root.display(),
                    "runner phase: cached starting db present, skipping indexing"
                );
                seed_loaded_workspace_from_repo(&state, &prepared).await?;
                steps.push("seed_loaded_workspace_from_repo".to_string());
                app.pump_pending_events().await;
                steps.push("pump_post_cached_workspace_events".to_string());
            } else {
                info!(task_id = %prepared.task_id, repo_root = %prepared.repo_root.display(), "runner phase: start indexing");
                app.run_command_text("/index").await;
                steps.push("run_index_command".to_string());

                info!("runner phase: waiting for indexing completion");
                let mut last_index_progress = None;
                if let Err(err) = wait_for_indexing_completion(
                    &mut app,
                    &mut realtime_rx,
                    &mut background_rx,
                    &mut index_rx,
                    Arc::clone(&state.db),
                    indexing_checkpoint_db.clone(),
                    indexing_failure_db.clone(),
                    self.index_debug_snapshots,
                    &mut last_index_progress,
                )
                .await
                {
                    persist_indexing_failure_status(
                        &indexing_status_path,
                        &err,
                        last_index_progress,
                    );
                    persist_parse_failure_artifact(&state, &parse_failure_path).await;
                    return Err(err);
                }
                steps.push("indexing_completed".to_string());
                info!("runner phase: indexing completed");
                app.pump_pending_events().await;
                steps.push("pump_post_index_events".to_string());
            }

            let indexing_status = IndexingStatusArtifact {
                status: "completed".to_string(),
                detail: if using_cached_starting_db {
                    "Loaded cached starting db snapshot and skipped reindexing.".to_string()
                } else {
                    "Indexing completed through the full app command path.".to_string()
                },
                last_progress: None,
            };
            write_json(&indexing_status_path, &indexing_status)?;
            steps.push("write_indexing_status".to_string());

            persist_db_snapshot(
                Arc::clone(&state.db),
                indexing_checkpoint_db.clone(),
                "starting snapshot checkpoint",
            )
            .await?;
            steps.push("write_indexing_checkpoint".to_string());

            if !using_cached_starting_db {
                if let Err(err) = persist_starting_db_cache(
                    &prepared,
                    &embedding_selection,
                    &indexing_checkpoint_db,
                )
                .await
                {
                    warn!(
                        snapshot = %indexing_checkpoint_db.display(),
                        error = %err,
                        "runner phase: failed to refresh starting db cache"
                    );
                }
                steps.push("refresh_starting_db_cache".to_string());
            }

            let setup_phase = build_setup_phase(
                &state.db,
                &repo_state,
                &indexing_status,
                setup_start_time,
                using_cached_starting_db,
                &parse_failure_path,
            )
            .await?;
            run_record.phases.setup = Some(setup_phase);
            steps.push("populate_setup_phase".to_string());
            registration.update_phase(
                RunLifecyclePhase::Setup,
                RunPhaseStatus::Completed,
                Some(indexing_status.detail.clone()),
            );
            registration.update_phase(
                RunLifecyclePhase::Patching,
                RunPhaseStatus::InProgress,
                Some("agent inquiry running".to_string()),
            );
            persist_registration(&registration)?;
            let setup_wall_clock_secs = Some(run_start_instant.elapsed().as_secs_f64());

            let agent_execution_start = Instant::now();
            let turn_artifact = run_benchmark_turn(
                &prepared,
                &state,
                &mut app,
                &mut debug_rx,
                &mut realtime_rx,
                &mut background_rx,
                &turn_trace_path,
                selected_model_id.clone(),
                &expected_file_baselines,
            )
            .await?;
            write_agent_turn_summary(&turn_summary_path, &turn_artifact)?;
            let validation_audit = build_agent_validation_audit(&prepared, &turn_artifact);
            write_json(&validation_audit_path, &validation_audit)?;
            steps.push("benchmark_turn_completed".to_string());
            steps.push("write_validation_audit".to_string());
            let full_response_trace = match (
                full_response_trace_source.as_ref(),
                full_response_trace_start,
            ) {
                (Some(source_path), Some(start_offset)) => {
                    if persist_full_response_trace_slice(
                        source_path,
                        start_offset,
                        &full_response_trace_path,
                    )
                    .await?
                    {
                        steps.push("persist_full_response_trace".to_string());
                        Some(full_response_trace_path.clone())
                    } else {
                        None
                    }
                }
                _ => None,
            };
            let agent_wall_clock_secs = Some(agent_execution_start.elapsed().as_secs_f64());

            let db_timestamp =
                state
                    .db
                    .current_validity_micros()
                    .map_err(|e| PrepareError::DatabaseSetup {
                        phase: "get_db_timestamp",
                        detail: format!("Failed to get Cozo timestamp: {}", e),
                    })?;
            let patch_artifact = turn_artifact.patch_artifact.clone();
            run_record.mark_time_travel(1, db_timestamp, "turn_complete");
            run_record.add_turn_from_artifact(turn_artifact, db_timestamp);
            registration.update_phase(
                RunLifecyclePhase::Patching,
                RunPhaseStatus::Completed,
                Some("agent benchmark turn completed".to_string()),
            );
            registration.artifacts.full_response_trace = full_response_trace.clone();
            persist_registration(&registration)?;

            let snapshot_file = run_output_dir.join("final-snapshot.db");
            info!(
                snapshot = %snapshot_file.display(),
                "runner phase: persisting final eval snapshot"
            );
            persist_db_snapshot(
                Arc::clone(&state.db),
                snapshot_file.clone(),
                "final snapshot",
            )
            .await?;
            steps.push("snapshot_completed".to_string());
            info!(snapshot_file = %snapshot_file.display(), "runner phase: snapshot completed");

            let snapshot_status = SnapshotStatusArtifact {
                status: "completed".to_string(),
                snapshot_file: Some(snapshot_file.clone()),
                registry_file: snapshot_file.clone(),
                config_home,
            };
            write_json(&snapshot_status_path, &snapshot_status)?;
            steps.push("write_snapshot_status".to_string());

            registration.update_phase(
                RunLifecyclePhase::Packaging,
                RunPhaseStatus::InProgress,
                Some("writing benchmark submission artifact".to_string()),
            );
            persist_registration(&registration)?;

            let packaging_started_at = chrono::Utc::now().to_rfc3339();
            let msb_submission_artifact = match write_msb_submission_artifact(
                &prepared,
                &run_arm,
                &run_output_dir,
                &manifest_path,
                &record_path,
                Some(&patch_artifact),
            ) {
                Ok(artifact) => artifact,
                Err(err) => {
                    let detail = err.to_string();
                    steps.push("write_msb_submission_failed".to_string());
                    record_packaging_failure(
                        &mut run_record,
                        &mut registration,
                        packaging_started_at,
                        detail.clone(),
                    );
                    persist_registration(&registration)?;

                    finalize_run_timing(
                        &mut run_record,
                        setup_start_time,
                        run_start_instant,
                        setup_wall_clock_secs,
                        agent_wall_clock_secs,
                    );

                    let execution_log = ExecutionLog {
                        task_id: prepared.task_id.clone(),
                        run_arm: run_arm.clone(),
                        repo_root: prepared.repo_root.clone(),
                        output_dir: run_output_dir.clone(),
                        selected_model: selected_model_id.clone(),
                        selected_provider: Some(selected_provider.clone()),
                        selected_endpoint: selected_endpoint.clone(),
                        full_response_trace: full_response_trace.clone(),
                        steps,
                    };
                    if let Err(write_err) = write_json(&execution_log_path, &execution_log) {
                        warn!(
                            path = %execution_log_path.display(),
                            error = %write_err,
                            "runner phase: failed to write packaging-failure execution log"
                        );
                    } else if let Err(write_err) = record_last_run(&execution_log.output_dir) {
                        warn!(
                            output_dir = %execution_log.output_dir.display(),
                            error = %write_err,
                            "runner phase: failed to record packaging-failure last run"
                        );
                    }

                    if let Err(write_err) = write_compressed_record(&record_path, &run_record) {
                        warn!(
                            path = %record_path.display(),
                            error = %write_err,
                            "runner phase: failed to write packaging-failure compressed run record"
                        );
                    }

                    return Err(err);
                }
            };
            if let Some(submission) = msb_submission_artifact.as_ref() {
                steps.push("write_msb_submission".to_string());
                steps.push("write_benchmark_patch_projection".to_string());
                registration.artifacts.patch_projection =
                    Some(submission.patch_projection_path.clone());
                registration.update_submission_status(Some(&submission.fix_patch));
                registration.update_phase(
                    RunLifecyclePhase::Packaging,
                    RunPhaseStatus::Completed,
                    Some("submission artifact written".to_string()),
                );
            } else {
                registration.update_submission_status(None);
                registration.update_phase(
                    RunLifecyclePhase::Packaging,
                    RunPhaseStatus::Skipped,
                    Some("no benchmark packaging for this run".to_string()),
                );
            }
            run_record.phases.packaging = Some(PackagingPhase {
                started_at: packaging_started_at,
                ended_at: chrono::Utc::now().to_rfc3339(),
                submission_artifact_state: match msb_submission_artifact.as_ref() {
                    Some(submission) if submission.fix_patch.trim().is_empty() => {
                        SubmissionArtifactState::Empty
                    }
                    Some(_) => SubmissionArtifactState::Nonempty,
                    None => SubmissionArtifactState::NotApplicable,
                },
                msb_submission_path: msb_submission_artifact
                    .as_ref()
                    .map(|artifact| artifact.path.clone()),
                patch_projection_path: msb_submission_artifact
                    .as_ref()
                    .map(|artifact| artifact.patch_projection_path.clone()),
                patch_projection_check_state: msb_submission_artifact
                    .as_ref()
                    .map(|artifact| artifact.patch_projection_check_state)
                    .unwrap_or(PatchProjectionCheckState::NotApplicable),
            });
            registration.update_phase(
                RunLifecyclePhase::Validation,
                RunPhaseStatus::Skipped,
                Some(format!(
                    "runner validation not executed; agent tool validation audit written to {}",
                    validation_audit_path.display()
                )),
            );
            persist_registration(&registration)?;

            finalize_run_timing(
                &mut run_record,
                setup_start_time,
                run_start_instant,
                setup_wall_clock_secs,
                agent_wall_clock_secs,
            );

            let execution_log = ExecutionLog {
                task_id: prepared.task_id.clone(),
                run_arm: run_arm.clone(),
                repo_root: prepared.repo_root.clone(),
                output_dir: run_output_dir.clone(),
                selected_model: selected_model_id.clone(),
                selected_provider: Some(selected_provider.clone()),
                selected_endpoint: selected_endpoint.clone(),
                full_response_trace: full_response_trace.clone(),
                steps,
            };
            write_json(&execution_log_path, &execution_log)?;
            info!(
                execution_log = %execution_log_path.display(),
                repo_state = %repo_state_path.display(),
                indexing_status = %indexing_status_path.display(),
                indexing_checkpoint_db = %indexing_checkpoint_db.display(),
                indexing_failure_db = %indexing_failure_db.display(),
                snapshot_status = %snapshot_status_path.display(),
                turn_trace = %turn_trace_path.display(),
                turn_summary = %turn_summary_path.display(),
                full_response_trace = full_response_trace.as_ref().map(|p| p.display().to_string()),
                msb_submission = msb_submission_artifact.as_ref().map(|artifact| artifact.path.display().to_string()),
                "runner phase: wrote run artifacts"
            );
            record_last_run(&execution_log.output_dir)?;

            if let Err(e) = write_compressed_record(&record_path, &run_record) {
                warn!(
                    path = %record_path.display(),
                    error = %e,
                    "runner phase: failed to write compressed run record"
                );
            } else {
                info!(path = %record_path.display(), "runner phase: wrote compressed run record");
            }

            Ok(AgentRunArtifactPaths {
                base: RunArtifactPaths {
                    run_manifest: manifest_path.clone(),
                    execution_log: execution_log_path.clone(),
                    repo_state: repo_state_path.clone(),
                    indexing_status: indexing_status_path.clone(),
                    indexing_checkpoint_db: indexing_checkpoint_db.clone(),
                    indexing_failure_db: indexing_failure_db.clone(),
                    snapshot_status: snapshot_status_path.clone(),
                    msb_submission: msb_submission_artifact.map(|artifact| artifact.path),
                    patch_projection: registration.artifacts.patch_projection.clone(),
                    validation_audit: Some(validation_audit_path.clone()),
                    record_path: Some(record_path.clone()),
                    full_response_trace,
                },
                turn_trace: turn_trace_path.clone(),
                turn_summary: turn_summary_path.clone(),
            })
        }
        .await;

        match result {
            Ok(artifacts) => {
                registration.mark_completed();
                persist_registration(&registration)?;
                Ok(artifacts)
            }
            Err(err) => {
                registration.mark_failed(err.to_string());
                persist_registration(&registration)?;
                Err(err)
            }
        }
    }
}
pub(crate) async fn run_benchmark_turn(
    prepared: &PreparedSingleRun,
    state: &Arc<AppState>,
    app: &mut App,
    debug_rx: &mut mpsc::Receiver<ploke_tui::app::commands::harness::DebugStateCommand>,
    realtime_rx: &mut broadcast::Receiver<AppEvent>,
    background_rx: &mut broadcast::Receiver<AppEvent>,
    trace_path: &Path,
    selected_model: ModelId,
    expected_file_baselines: &[ExpectedFileBaseline],
) -> Result<AgentTurnArtifact, PrepareError> {
    let deadline = Instant::now() + Duration::from_secs(prepared.budget.wall_clock_secs as u64);
    let issue_prompt = build_agent_issue_prompt(prepared);
    let user_message_id = submit_benchmark_prompt(app, &issue_prompt).await?;
    let mut artifact = AgentTurnArtifact {
        task_id: prepared.task_id.clone(),
        selected_model,
        issue_prompt,
        user_message_id,
        events: Vec::new(),
        prompt_debug: None,
        terminal_record: None,
        final_assistant_message: None,
        patch_artifact: PatchArtifact {
            edit_proposals: Vec::new(),
            create_proposals: Vec::new(),
            applied: false,
            all_proposals_applied: false,
            expected_file_changes: Vec::new(),
            any_expected_file_changed: false,
            all_expected_files_changed: false,
        },
        llm_prompt: Vec::new(),
        llm_response: None,
    };
    let mut tool_request_started_at: HashMap<String, Instant> = HashMap::new();
    write_agent_turn_trace(trace_path, &artifact)?;

    loop {
        app.pump_pending_events().await;

        if artifact.terminal_record.is_some() {
            // Keep draining debug_rx here alongside the app event streams.
            // In the RuntimeHarness relay, debug emission is coupled to
            // forwarding/proxying StateCommand traffic, including delicate
            // oneshot relay cases. If we stop draining this receiver as
            // soon as ChatTurnFinished arrives, a paired oneshot can remain
            // stuck waiting forever even though the turn is otherwise
            // terminal. See crates/ploke-tui/src/app/commands/unit_tests/harness.rs
            // ("Key Design: The Relay Pattern" and RelayStateCmd::run_relay).
            drain_post_terminal_events(
                &mut artifact,
                state,
                debug_rx,
                realtime_rx,
                background_rx,
                trace_path,
                &mut tool_request_started_at,
                Duration::from_millis(FINAL_RESPONSE_GRACE_MILLIS),
            )
            .await?;
            break;
        }

        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            artifact.patch_artifact =
                collect_patch_artifact_with_expected(state, expected_file_baselines).await?;
            write_agent_turn_trace(trace_path, &artifact)?;
            return Err(PrepareError::Timeout {
                phase: "benchmark_turn",
                secs: prepared.budget.wall_clock_secs as u64,
            });
        }

        let wait_for = remaining.min(Duration::from_millis(250));
        tokio::select! {
            debug = debug_rx.recv() => {
                match debug {
                    Some(debug) => {
                        artifact.events.push(ObservedTurnEvent::DebugCommand(debug.as_str().to_string()));
                        write_agent_turn_trace(trace_path, &artifact)?;
                    }
                    None => {
                        return Err(PrepareError::EventStreamClosed { phase: "benchmark_turn_debug" });
                    }
                }
            }
            realtime = realtime_rx.recv() => {
                match realtime {
                    Ok(event) => {
                        handle_benchmark_event(
                            &mut artifact,
                            state,
                            event,
                            &mut tool_request_started_at,
                        )
                        .await;
                        write_agent_turn_trace(trace_path, &artifact)?;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PrepareError::EventStreamClosed { phase: "benchmark_turn_realtime" });
                    }
                }
            }
            background = background_rx.recv() => {
                match background {
                    Ok(event) => {
                        handle_benchmark_event(
                            &mut artifact,
                            state,
                            event,
                            &mut tool_request_started_at,
                        )
                        .await;
                        write_agent_turn_trace(trace_path, &artifact)?;
                    }
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PrepareError::EventStreamClosed { phase: "benchmark_turn_background" });
                    }
                }
            }
            _ = sleep(wait_for) => {
                continue;
            }
        }
    }

    app.pump_pending_events().await;
    artifact.final_assistant_message = None;
    artifact.patch_artifact =
        collect_patch_artifact_with_expected(state, expected_file_baselines).await?;

    // Note: llm_prompt and llm_response are now captured via events in handle_benchmark_event
    // This avoids the need for mutable state access and TTL mutation side effects

    write_agent_turn_trace(trace_path, &artifact)?;
    Ok(artifact)
}

pub(crate) async fn drain_post_terminal_events(
    artifact: &mut AgentTurnArtifact,
    state: &Arc<AppState>,
    debug_rx: &mut mpsc::Receiver<ploke_tui::app::commands::harness::DebugStateCommand>,
    realtime_rx: &mut broadcast::Receiver<AppEvent>,
    background_rx: &mut broadcast::Receiver<AppEvent>,
    trace_path: &Path,
    tool_request_started_at: &mut HashMap<String, Instant>,
    grace_period: Duration,
) -> Result<(), PrepareError> {
    let deadline = Instant::now() + grace_period;

    loop {
        let mut observed_event = false;

        loop {
            match debug_rx.try_recv() {
                Ok(debug) => {
                    artifact
                        .events
                        .push(ObservedTurnEvent::DebugCommand(debug.as_str().to_string()));
                    observed_event = true;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => break,
            }
        }

        loop {
            match realtime_rx.try_recv() {
                Ok(event) => {
                    handle_benchmark_event(artifact, state, event, tool_request_started_at).await;
                    observed_event = true;
                }
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Closed) => break,
                Err(broadcast::error::TryRecvError::Lagged(_)) => {
                    observed_event = true;
                    continue;
                }
            }
        }

        loop {
            match background_rx.try_recv() {
                Ok(event) => {
                    handle_benchmark_event(artifact, state, event, tool_request_started_at).await;
                    observed_event = true;
                }
                Err(broadcast::error::TryRecvError::Empty) => break,
                Err(broadcast::error::TryRecvError::Closed) => break,
                Err(broadcast::error::TryRecvError::Lagged(_)) => {
                    observed_event = true;
                    continue;
                }
            }
        }

        if observed_event {
            write_agent_turn_trace(trace_path, &artifact)?;
        }

        if Instant::now() >= deadline {
            break;
        }

        if artifact.llm_response.is_some() && !observed_event {
            break;
        }

        if !observed_event {
            sleep(Duration::from_millis(25)).await;
        }
    }

    Ok(())
}

pub(crate) async fn handle_benchmark_event(
    artifact: &mut AgentTurnArtifact,
    state: &Arc<AppState>,
    event: AppEvent,
    tool_request_started_at: &mut HashMap<String, Instant>,
) {
    match event {
        AppEvent::Llm(llm_event) => {
            // Capture structured LLM events for RunRecord (Phase 1C/1D)
            match &llm_event {
                LlmEvent::ChatCompletion(ChatEvt::PromptConstructed {
                    formatted_prompt, ..
                }) => {
                    // Capture the exact prompt sent to the LLM (what LLM sees)
                    artifact.llm_prompt = formatted_prompt.clone();
                    artifact.prompt_debug = Some(format!("{:?}", llm_event));
                    // Still log as debug string for other events
                    let rendered = format!("{:?}", llm_event);
                    artifact.events.push(ObservedTurnEvent::LlmEvent(rendered));
                }
                LlmEvent::ChatCompletion(ChatEvt::Response {
                    content,
                    model,
                    metadata,
                    usage,
                    ..
                }) => {
                    // Capture the LLM's response content (for backward compat)
                    artifact.llm_response = Some(content.clone());

                    // Phase 1D: Structured LLM response capture
                    let record = LlmResponseRecord {
                        content: content.clone(),
                        model: model.clone(),
                        usage: Some(ploke_llm::response::TokenUsage {
                            prompt_tokens: usage.prompt_tokens,
                            completion_tokens: usage.completion_tokens,
                            total_tokens: usage.total_tokens,
                        }),
                        finish_reason: Some(metadata.finish_reason.clone()),
                        metadata: Some(metadata.clone()),
                    };
                    artifact.events.push(ObservedTurnEvent::LlmResponse(record));
                }
                _ => {
                    // Other LLM events: log as debug string
                    let rendered = format!("{:?}", llm_event);
                    artifact.events.push(ObservedTurnEvent::LlmEvent(rendered));
                }
            }
        }
        AppEvent::System(SystemEvent::ToolCallRequested {
            request_id,
            parent_id,
            tool_call,
        }) => {
            tool_request_started_at.insert(tool_call.call_id.to_string(), Instant::now());
            let record = ToolRequestRecord {
                request_id: request_id.to_string(),
                parent_id: parent_id.to_string(),
                call_id: tool_call.call_id.to_string(),
                tool: tool_call.function.name.as_str().to_string(),
                arguments: tool_call.function.arguments.clone().into(),
            };
            artifact
                .events
                .push(ObservedTurnEvent::ToolRequested(record));
        }
        AppEvent::System(SystemEvent::ToolCallCompleted {
            request_id,
            parent_id,
            call_id,
            content,
            ui_payload,
        }) => {
            let tool = artifact
                .events
                .iter()
                .rev()
                .find_map(|ev| match ev {
                    ObservedTurnEvent::ToolRequested(req) if req.call_id == call_id.to_string() => {
                        Some(req.tool.clone())
                    }
                    _ => None,
                })
                .unwrap_or_else(|| "unknown".to_string());
            let record = ToolCompletedRecord {
                request_id: request_id.to_string(),
                parent_id: parent_id.to_string(),
                call_id: call_id.to_string(),
                tool,
                content,
                ui_payload,
                latency_ms: tool_request_started_at
                    .remove(call_id.as_ref())
                    .map(|started_at| started_at.elapsed().as_millis() as u64)
                    .unwrap_or(0),
            };
            artifact
                .events
                .push(ObservedTurnEvent::ToolCompleted(record));
        }
        AppEvent::System(SystemEvent::ToolCallFailed {
            request_id,
            parent_id,
            call_id,
            error,
            ui_payload,
        }) => {
            let tool = artifact.events.iter().rev().find_map(|ev| match ev {
                ObservedTurnEvent::ToolRequested(req) if req.call_id == call_id.to_string() => {
                    Some(req.tool.clone())
                }
                _ => None,
            });
            let record = ToolFailedRecord {
                request_id: request_id.to_string(),
                parent_id: parent_id.to_string(),
                call_id: call_id.to_string(),
                tool,
                error,
                ui_payload,
                latency_ms: tool_request_started_at
                    .remove(call_id.as_ref())
                    .map(|started_at| started_at.elapsed().as_millis() as u64)
                    .unwrap_or(0),
            };
            artifact.events.push(ObservedTurnEvent::ToolFailed(record));
        }
        AppEvent::MessageUpdated(message_event) => {
            if let Some(snapshot) = snapshot_message(state, message_event) {
                artifact
                    .events
                    .push(ObservedTurnEvent::MessageUpdated(snapshot));
            }
        }
        AppEvent::System(SystemEvent::ChatTurnFinished {
            session_id,
            request_id,
            parent_id,
            assistant_message_id,
            outcome,
            error_id,
            summary,
            attempts,
            ..
        }) => {
            let record = TurnFinishedRecord {
                session_id: session_id.to_string(),
                request_id: request_id.to_string(),
                parent_id: parent_id.to_string(),
                assistant_message_id: assistant_message_id.to_string(),
                outcome,
                error_id: error_id.map(|id| id.to_string()),
                summary,
                attempts,
            };
            artifact.terminal_record = Some(record.clone());
            artifact
                .events
                .push(ObservedTurnEvent::TurnFinished(record));
        }
        _ => {}
    }
}

pub(crate) async fn submit_benchmark_prompt(
    app: &mut App,
    prompt: &str,
) -> Result<String, PrepareError> {
    let state_cmd_tx = app.state_cmd_tx();
    let user_message_id = ploke_core::PROJECT_NAMESPACE_UUID;
    let (completion_tx, completion_rx) = oneshot::channel();
    let (scan_tx, scan_rx) = oneshot::channel();

    state_cmd_tx
        .send(
            ploke_tui::app_state::commands::StateCommand::AddUserMessage {
                content: prompt.to_string(),
                new_user_msg_id: user_message_id,
                completion_tx,
            },
        )
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "submit_benchmark_prompt_add_user",
            detail: err.to_string(),
        })?;
    state_cmd_tx
        .send(ploke_tui::app_state::commands::StateCommand::ScanForChange { scan_tx })
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "submit_benchmark_prompt_scan",
            detail: err.to_string(),
        })?;
    state_cmd_tx
        .send(ploke_tui::app_state::commands::StateCommand::EmbedMessage {
            new_msg_id: user_message_id,
            completion_rx,
            scan_rx,
        })
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "submit_benchmark_prompt_embed",
            detail: err.to_string(),
        })?;

    Ok(user_message_id.to_string())
}
