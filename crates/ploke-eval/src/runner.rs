use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::Database;
use ploke_db::multi_embedding::db_ext::EmbeddingExt;
use ploke_embed::config::{OpenRouterConfig, TruncatePolicy};
use ploke_embed::indexer::{
    EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexingStatus,
};
use ploke_embed::providers::openrouter::OpenRouterBackend;
use ploke_tui::AppEvent;
use ploke_tui::app::App;
use ploke_tui::app::commands::harness::TestRuntime;
use ploke_tui::app_state::AppState;
use ploke_tui::parser::{resolve_index_target, run_parse_resolved};
use serde::{Deserialize, Serialize};
use tokio::sync::broadcast;
use tokio::time::{Instant, sleep};
use tracing::info;

use crate::spec::{PrepareError, PreparedSingleRun};

const DEFAULT_PHASE_TIMEOUT_SECS: u64 = 300;
const WAIT_HEARTBEAT_SECS: u64 = 10;
const OPENROUTER_CODESTRAL_MODEL: &str = "mistralai/codestral-embed-2505";
const OPENROUTER_CODESTRAL_DIMS: usize = 1536;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunMsbSingleRequest {
    pub run_manifest: PathBuf,
    pub index_debug_snapshots: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayMsbBatchRequest {
    pub run_manifest: PathBuf,
    /// 1-based batch index to replay.
    pub batch_number: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunArtifactPaths {
    pub run_manifest: PathBuf,
    pub execution_log: PathBuf,
    pub repo_state: PathBuf,
    pub indexing_status: PathBuf,
    pub indexing_checkpoint_db: PathBuf,
    pub indexing_failure_db: PathBuf,
    pub snapshot_status: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoStateArtifact {
    pub repo_root: PathBuf,
    pub requested_base_sha: Option<String>,
    pub checked_out_head_sha: Option<String>,
    pub git_status_porcelain: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IndexingStatusArtifact {
    pub status: String,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SnapshotStatusArtifact {
    pub status: String,
    pub snapshot_file: Option<PathBuf>,
    pub registry_file: PathBuf,
    pub config_home: PathBuf,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionLog {
    pub task_id: String,
    pub repo_root: PathBuf,
    pub output_dir: PathBuf,
    pub steps: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReplayBatchArtifact {
    pub batch_number: usize,
    pub run_manifest: PathBuf,
    pub batch_file: PathBuf,
    pub batch: Vec<ploke_db::TypedEmbedData>,
}

impl RunMsbSingleRequest {
    pub async fn run(self) -> Result<RunArtifactPaths, PrepareError> {
        let (manifest_path, prepared) = load_prepared_run(self.run_manifest)?;

        fs::create_dir_all(&prepared.output_dir).map_err(|source| {
            PrepareError::CreateOutputDir {
                path: prepared.output_dir.clone(),
                source,
            }
        })?;

        let execution_log_path = prepared.output_dir.join("execution-log.json");
        let repo_state_path = prepared.output_dir.join("repo-state.json");
        let indexing_status_path = prepared.output_dir.join("indexing-status.json");
        let snapshot_status_path = prepared.output_dir.join("snapshot-status.json");
        let indexing_checkpoint_db = prepared.output_dir.join("indexing-checkpoint.db");
        let indexing_failure_db = prepared.output_dir.join("indexing-failure.db");

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

        let runtime_db = init_runtime_db()?;
        steps.push("init_runtime_db".to_string());

        let config_home = prepared.output_dir.join("config");
        fs::create_dir_all(&config_home).map_err(|source| PrepareError::CreateOutputDir {
            path: config_home.clone(),
            source,
        })?;
        let _config_guard = XdgConfigHomeGuard::set_to(&config_home);
        steps.push("sandbox_config_home".to_string());

        let embedding_processor = codestral_embedding_processor()?;
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
        info!("runner phase: inspect active embedding set before activation");
        let currently_active_set: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set");
        info!(?currently_active_set, "active embedding set before activation");

        activate_codestral_runtime(&state)?;

        let currently_active_set: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set");
        info!(?currently_active_set, "active embedding set after activation");
        steps.push("activate_codestral_embedding_set".to_string());
        let mut app = runtime
            .into_app_with_state_pwd(prepared.repo_root.clone())
            .await;
        steps.push("bootstrap_headless_runtime".to_string());

        info!(task_id = %prepared.task_id, repo_root = %prepared.repo_root.display(), "runner phase: start indexing");
        app.run_command_text("/index").await;
        steps.push("run_index_command".to_string());

        info!("runner phase: waiting for indexing completion");
        wait_for_indexing_completion(
            &mut app,
            &mut realtime_rx,
            &mut background_rx,
            &mut index_rx,
            Arc::clone(&state.db),
            indexing_checkpoint_db.clone(),
            indexing_failure_db.clone(),
            self.index_debug_snapshots,
        )
        .await?;
        steps.push("indexing_completed".to_string());
        info!("runner phase: indexing completed");
        app.pump_pending_events().await;
        steps.push("pump_post_index_events".to_string());

        let indexing_status = IndexingStatusArtifact {
            status: "completed".to_string(),
            detail: "Indexing completed through the full app command path.".to_string(),
        };
        write_json(&indexing_status_path, &indexing_status)?;
        steps.push("write_indexing_status".to_string());

        let snapshot_file = prepared.output_dir.join("final-snapshot.db");
        info!(
            snapshot = %snapshot_file.display(),
            "runner phase: persisting final eval snapshot"
        );
        persist_db_snapshot(Arc::clone(&state.db), snapshot_file.clone(), "final snapshot")
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

        let execution_log = ExecutionLog {
            task_id: prepared.task_id,
            repo_root: prepared.repo_root,
            output_dir: prepared.output_dir,
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
            "runner phase: wrote run artifacts"
        );

        Ok(RunArtifactPaths {
            run_manifest: manifest_path,
            execution_log: execution_log_path,
            repo_state: repo_state_path,
            indexing_status: indexing_status_path,
            indexing_checkpoint_db,
            indexing_failure_db,
            snapshot_status: snapshot_status_path,
        })
    }
}

impl ReplayMsbBatchRequest {
    pub async fn run(self) -> Result<PathBuf, PrepareError> {
        let (manifest_path, prepared) = load_prepared_run(self.run_manifest)?;
        checkout_repo_to_base(&prepared.repo_root, prepared.base_sha.as_deref())?;
        let (_app, state, _config_guard) = setup_replay_runtime(&prepared).await?;
        let replay_batch_path = prepared
            .output_dir
            .join(format!("replay-batch-{:03}.json", self.batch_number));

        let indexer_task = state
            .indexer_task
            .as_ref()
            .cloned()
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "replay_batch_indexer_task",
                detail: "missing indexer task in app state".to_string(),
            })?;

        let batch = indexer_task
            .replay_batch(self.batch_number)
            .await
            .map_err(|err| PrepareError::DatabaseSetup {
                phase: "replay_batch",
                detail: err.to_string(),
            })?
            .ok_or_else(|| PrepareError::IndexingFailed {
                detail: format!(
                    "batch {} was not available in manifest {}",
                    self.batch_number,
                    manifest_path.display()
                ),
            })?;

        log_replay_batch_context(self.batch_number, &batch);
        let replay_artifact = ReplayBatchArtifact {
            batch_number: self.batch_number,
            run_manifest: manifest_path.clone(),
            batch_file: replay_batch_path.clone(),
            batch: batch.clone(),
        };
        write_json(&replay_batch_path, &replay_artifact)?;
        info!(
            batch_file = %replay_batch_path.display(),
            batch_number = self.batch_number,
            "wrote replay batch artifact"
        );
        indexer_task
            .process_batch(batch, |current, total| {
                info!(
                    batch_number = self.batch_number,
                    current,
                    total,
                    "replay batch progress"
                )
            })
            .await
            .map_err(|err| PrepareError::DatabaseSetup {
                phase: "replay_batch_process",
                detail: err.to_string(),
            })?;

        Ok(replay_batch_path)
    }
}

fn init_runtime_db() -> Result<Arc<Database>, PrepareError> {
    info!("initializing eval runtime database");
    let db = Arc::new(
        Database::init_with_schema().map_err(|err| PrepareError::DatabaseSetup {
            phase: "init_with_schema",
            detail: err.to_string(),
        })?,
    );
    db.setup_multi_embedding()
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "setup_multi_embedding",
            detail: err.to_string(),
        })?;
    info!("eval runtime database initialized");
    Ok(db)
}

async fn setup_replay_runtime(
    prepared: &PreparedSingleRun,
) -> Result<(App, Arc<AppState>, XdgConfigHomeGuard), PrepareError> {
    let runtime_db = init_runtime_db()?;

    let config_home = prepared.output_dir.join("config");
    fs::create_dir_all(&config_home).map_err(|source| PrepareError::CreateOutputDir {
        path: config_home.clone(),
        source,
    })?;
    let config_guard = XdgConfigHomeGuard::set_to(&config_home);

    let embedding_processor = codestral_embedding_processor()?;
    let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, embedding_processor)
        .spawn_file_manager()
        .spawn_state_manager()
        .spawn_event_bus();
    let state = runtime.state_arc();

    activate_codestral_runtime(&state)?;

    prepare_workspace_for_replay(&state, prepared).await?;

    let app = runtime
        .into_app_with_state_pwd(prepared.repo_root.clone())
        .await;

    Ok((app, state, config_guard))
}

async fn prepare_workspace_for_replay(
    state: &Arc<AppState>,
    prepared: &PreparedSingleRun,
) -> Result<(), PrepareError> {
    info!(
        repo_root = %prepared.repo_root.display(),
        "preparing replay workspace for batch selection"
    );
    let resolved = resolve_index_target(Some(prepared.repo_root.clone()), &prepared.repo_root)
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "replay_resolve_index_target",
            detail: err.to_string(),
        })?;

    run_parse_resolved(Arc::clone(&state.db), &resolved).map_err(|err| PrepareError::DatabaseSetup {
        phase: "replay_run_parse_resolved",
        detail: err.to_string(),
    })?;

    let outcome = state
        .with_system_txn(|txn| {
            txn.set_loaded_workspace(
                resolved.workspace_root.clone(),
                resolved.member_roots.clone(),
                Some(resolved.focused_root.clone()),
            );
            txn.record_parse_success();
            txn.derive_path_policy(&[])
        })
        .await;
    if let Some(policy) = outcome.result {
        state
            .io_handle
            .update_roots(Some(policy.roots), Some(policy.symlink_policy))
            .await;
    }
    info!(
        workspace_root = %resolved.workspace_root.display(),
        "replay workspace prepared"
    );
    Ok(())
}

fn codestral_embedding_config() -> OpenRouterConfig {
    OpenRouterConfig {
        model: OPENROUTER_CODESTRAL_MODEL.to_string(),
        dimensions: Some(OPENROUTER_CODESTRAL_DIMS),
        request_dimensions: None,
        snippet_batch_size: 100,
        max_in_flight: 1,
        requests_per_second: Some(1),
        max_attempts: 3,
        initial_backoff_ms: 250,
        max_backoff_ms: 10_000,
        input_type: Some("code-snippet".into()),
        timeout_secs: 30,
        truncate_policy: TruncatePolicy::Truncate,
    }
}

fn codestral_embedding_processor() -> Result<EmbeddingProcessor, PrepareError> {
    info!(
        model = OPENROUTER_CODESTRAL_MODEL,
        dimensions = OPENROUTER_CODESTRAL_DIMS,
        "building codestral embedding processor"
    );
    let backend = OpenRouterBackend::new(&codestral_embedding_config()).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "init_codestral_embedder",
            detail: err.to_string(),
        }
    })?;
    info!("codestral embedding processor initialized");
    Ok(EmbeddingProcessor::new(EmbeddingSource::OpenRouter(
        backend,
    )))
}

fn codestral_embedding_set() -> EmbeddingSet {
    EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("openrouter"),
        EmbeddingModelId::new_from_str(OPENROUTER_CODESTRAL_MODEL),
        EmbeddingShape::new_dims_default(OPENROUTER_CODESTRAL_DIMS as u32),
    )
}

fn activate_codestral_runtime(state: &Arc<AppState>) -> Result<(), PrepareError> {
    info!("activating codestral embedding set");
    let processor = Arc::new(codestral_embedding_processor()?);
    state
        .embedder
        .activate(&state.db, codestral_embedding_set(), processor)
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "activate_codestral_embedding_set",
            detail: err.to_string(),
        })?;
    info!("codestral embedding set activated");
    Ok(())
}

async fn wait_for_indexing_completion(
    app: &mut App,
    realtime_rx: &mut broadcast::Receiver<AppEvent>,
    background_rx: &mut broadcast::Receiver<AppEvent>,
    index_rx: &mut broadcast::Receiver<IndexingStatus>,
    db: Arc<Database>,
    checkpoint_snapshot: PathBuf,
    failure_snapshot: PathBuf,
    persist_debug_snapshots: bool,
) -> Result<(), PrepareError> {
    let deadline = Instant::now() + Duration::from_secs(DEFAULT_PHASE_TIMEOUT_SECS);
    let mut last_heartbeat = Instant::now();
    loop {
        app.pump_pending_events().await;
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            if persist_debug_snapshots {
                persist_db_snapshot(
                    Arc::clone(&db),
                    failure_snapshot.clone(),
                    "indexing timeout",
                )
                .await?;
            }
            return Err(PrepareError::Timeout {
                phase: "indexing_completed",
                secs: DEFAULT_PHASE_TIMEOUT_SECS,
            });
        }

        if last_heartbeat.elapsed() >= Duration::from_secs(WAIT_HEARTBEAT_SECS) {
            info!(remaining_secs = remaining.as_secs(), "waiting for indexing completion");
            last_heartbeat = Instant::now();
        }

        let wait_for = remaining.min(Duration::from_millis(250));
        tokio::select! {
            realtime = realtime_rx.recv() => {
                match realtime {
                    Ok(AppEvent::IndexingCompleted) => {
                        app.pump_pending_events().await;
                        return Ok(());
                    }
                    Ok(AppEvent::IndexingFailed) => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                failure_snapshot.clone(),
                                "indexing failed realtime",
                            )
                            .await?;
                        }
                        app.pump_pending_events().await;
                        return Err(PrepareError::IndexingFailed {
                            detail: "received AppEvent::IndexingFailed".to_string(),
                        });
                    }
                    Ok(AppEvent::Error(error)) if error.message.contains("Indexing failed") => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                failure_snapshot.clone(),
                                "indexing failed realtime error",
                            )
                            .await?;
                        }
                        app.pump_pending_events().await;
                        return Err(PrepareError::IndexingFailed {
                            detail: error.message,
                        });
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PrepareError::EventStreamClosed {
                            phase: "indexing_completed",
                        });
                    }
                }
            }
            background = background_rx.recv() => {
                match background {
                    Ok(AppEvent::Error(error)) if error.message.contains("Indexing failed") => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                failure_snapshot.clone(),
                                "indexing failed background error",
                            )
                            .await?;
                        }
                        return Err(PrepareError::IndexingFailed {
                            detail: error.message,
                        });
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PrepareError::EventStreamClosed {
                            phase: "indexing_completed_background",
                        });
                    }
                }
            }
            raw = index_rx.recv() => {
                match raw {
                    Ok(IndexingStatus { status: IndexStatus::Running, .. }) => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                checkpoint_snapshot.clone(),
                                "indexing checkpoint",
                            )
                            .await?;
                        }
                        continue;
                    }
                    Ok(IndexingStatus { status: IndexStatus::Completed, .. }) => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                checkpoint_snapshot.clone(),
                                "completed checkpoint",
                            )
                            .await?;
                        }
                        return Ok(());
                    }
                    Ok(IndexingStatus { status: IndexStatus::Failed(err), .. }) => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                failure_snapshot.clone(),
                                "indexing failure",
                            )
                            .await?;
                        }
                        return Err(PrepareError::IndexingFailed { detail: err });
                    }
                    Ok(IndexingStatus { status: IndexStatus::Cancelled, .. }) => {
                        if persist_debug_snapshots {
                            persist_db_snapshot(
                                Arc::clone(&db),
                                failure_snapshot.clone(),
                                "indexing cancelled",
                            )
                            .await?;
                        }
                        return Err(PrepareError::IndexingFailed {
                            detail: "indexing cancelled".to_string(),
                        });
                    }
                    Ok(_) => continue,
                    Err(broadcast::error::RecvError::Lagged(_)) => continue,
                    Err(broadcast::error::RecvError::Closed) => {
                        return Err(PrepareError::EventStreamClosed {
                            phase: "indexing_completed_raw",
                        });
                    }
                }
            }
            _ = sleep(wait_for) => continue,
        }
    }
}

async fn persist_db_snapshot(
    db: Arc<Database>,
    snapshot_path: PathBuf,
    label: &'static str,
) -> Result<(), PrepareError> {
    if let Some(parent) = snapshot_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::CreateOutputDir {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    info!(snapshot = %snapshot_path.display(), label, "persisting eval db snapshot");
    tokio::task::spawn_blocking(move || {
        if snapshot_path.exists() {
            fs::remove_file(&snapshot_path).map_err(|source| PrepareError::WriteManifest {
                path: snapshot_path.clone(),
                source,
            })?;
        }
        db.backup_db(snapshot_path.clone())
            .map_err(|source| PrepareError::DatabaseSetup {
                phase: "backup_db",
                detail: source.to_string(),
            })
    })
    .await
    .map_err(|join_err| PrepareError::DatabaseSetup {
        phase: "backup_db_join",
        detail: join_err.to_string(),
    })?
}

fn canonicalize_file(path: &Path) -> Result<PathBuf, PrepareError> {
    if !path.exists() {
        return Err(PrepareError::MissingRunManifest(path.to_path_buf()));
    }
    path.canonicalize()
        .map_err(|source| PrepareError::Canonicalize {
            path: path.to_path_buf(),
            source,
        })
}

fn checkout_repo_to_base(repo_root: &Path, base_sha: Option<&str>) -> Result<(), PrepareError> {
    run_git(repo_root, &["reset", "--hard"], "git reset --hard")?;
    if let Some(base_sha) = base_sha {
        run_git(
            repo_root,
            &["checkout", "--detach", base_sha],
            format!("git checkout --detach {base_sha}"),
        )?;
    }
    Ok(())
}

fn run_git(
    repo_root: &Path,
    args: &[&str],
    command_label: impl Into<String>,
) -> Result<(), PrepareError> {
    let command_label = command_label.into();
    let status = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .status()
        .map_err(|source| PrepareError::GitCommand {
            command: command_label.clone(),
            source,
        })?;

    if status.success() {
        Ok(())
    } else {
        Err(PrepareError::GitCommandStatus {
            command: command_label,
            status: status.code().unwrap_or(-1),
        })
    }
}

fn git_stdout(
    repo_root: &Path,
    args: &[&str],
    command_label: impl Into<String>,
) -> Result<Option<String>, PrepareError> {
    let command_label = command_label.into();
    let output = Command::new("git")
        .current_dir(repo_root)
        .args(args)
        .output()
        .map_err(|source| PrepareError::GitCommand {
            command: command_label.clone(),
            source,
        })?;

    if !output.status.success() {
        return Err(PrepareError::GitCommandStatus {
            command: command_label,
            status: output.status.code().unwrap_or(-1),
        });
    }

    Ok(Some(String::from_utf8_lossy(&output.stdout).to_string()))
}

fn write_json<T: Serialize>(path: &Path, value: &T) -> Result<(), PrepareError> {
    let json = serde_json::to_string_pretty(value).map_err(PrepareError::Serialize)?;
    fs::write(path, json).map_err(|source| PrepareError::WriteManifest {
        path: path.to_path_buf(),
        source,
    })
}

fn load_prepared_run(
    run_manifest: PathBuf,
) -> Result<(PathBuf, PreparedSingleRun), PrepareError> {
    let manifest_path = canonicalize_file(&run_manifest)?;
    let manifest_text =
        fs::read_to_string(&manifest_path).map_err(|source| PrepareError::ReadManifest {
            path: manifest_path.clone(),
            source,
        })?;
    let prepared: PreparedSingleRun =
        serde_json::from_str(&manifest_text).map_err(|source| PrepareError::ParseManifest {
            path: manifest_path.clone(),
            source,
        })?;

    Ok((manifest_path, prepared))
}

fn log_replay_batch_context(batch_number: usize, batch: &[ploke_db::TypedEmbedData]) {
    tracing::info!(
        batch_number,
        relation_count = batch.len(),
        "replaying selected batch"
    );

    for (relation_index, relation) in batch.iter().enumerate() {
        tracing::trace!(
            target: "embed-pipeline",
            batch_number,
            relation_index,
            relation = %relation.ty.relation_str(),
            node_count = relation.v.len(),
            "replay batch relation"
        );

        for (node_index, node) in relation.v.iter().enumerate() {
            tracing::trace!(
                target: "embed-pipeline",
                batch_number,
                relation_index,
                node_index,
                node_id = %node.id,
                node_name = %node.name,
                file_path = %node.file_path.display(),
                start_byte = node.start_byte,
                end_byte = node.end_byte,
                "replay batch node"
            );
        }
    }
}

struct XdgConfigHomeGuard {
    old_xdg: Option<String>,
}

impl XdgConfigHomeGuard {
    fn set_to(path: &Path) -> Self {
        let old_xdg = std::env::var("XDG_CONFIG_HOME").ok();
        unsafe {
            std::env::set_var("XDG_CONFIG_HOME", path);
        }
        Self { old_xdg }
    }
}

impl Drop for XdgConfigHomeGuard {
    fn drop(&mut self) {
        if let Some(old_xdg) = self.old_xdg.take() {
            unsafe {
                std::env::set_var("XDG_CONFIG_HOME", old_xdg);
            }
        } else {
            unsafe {
                std::env::remove_var("XDG_CONFIG_HOME");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use ploke_db::multi_embedding::schema::EmbeddingSetExt;
    use tracing_subscriber::fmt::SubscriberBuilder;

    fn init_tracing() {
        let _ = SubscriberBuilder::default()
            .with_max_level(tracing::Level::INFO)
            .with_target(false)
            .with_test_writer()
            .try_init();
    }

    #[tokio::test]
    async fn runner_component_setup_emits_tracing() {
        init_tracing();
        info!("starting eval runner component smoke test");

        let db = init_runtime_db().expect("init runtime db");
        info!("runtime database setup completed");

        // -- checking database's active embedding set --
        let currently_active_set: EmbeddingSet = db
            .with_active_set(|set| set.clone())
            .expect("active embedding set");
        info!(?currently_active_set);

        let _processor = codestral_embedding_processor().expect("init embedding processor");
        info!("embedding processor setup completed");

        // -- checking database's active embedding set --
        let currently_active_set: EmbeddingSet = db
            .with_active_set(|set| set.clone())
            .expect("active embedding set");
        info!(?currently_active_set);
    }

    #[tokio::test]
    async fn runner_activation_switches_active_set() {
        init_tracing();

        let runtime_db = init_runtime_db().expect("init runtime db");
        let processor = codestral_embedding_processor().expect("init embedding processor");
        let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, processor);
        let state = runtime.state_arc();

        let before: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set before activation");
        info!(?before, "active embedding set before activation");

        activate_codestral_runtime(&state).expect("activate codestral runtime");

        let after: EmbeddingSet = runtime_db
            .with_active_set(|set| set.clone())
            .expect("active embedding set after activation");
        info!(?after, "active embedding set after activation");

        assert_ne!(before.hash_id(), after.hash_id());
        assert_eq!(after.hash_id(), codestral_embedding_set().hash_id());
    }
}
