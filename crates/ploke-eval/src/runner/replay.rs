use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use ploke_tui::app::App;
use ploke_tui::app::commands::harness::TestRuntime;
use ploke_tui::app_state::AppState;
use ploke_tui::parser::{resolve_index_target, run_parse_resolved};
use tracing::info;

use crate::spec::{PrepareError, PreparedSingleRun};

use super::artifacts::*;
use super::*;

impl ReplayMsbBatchRequest {
    pub async fn run(self) -> Result<PathBuf, PrepareError> {
        let (manifest_path, prepared) = load_prepared_run(self.run_manifest)?;
        checkout_repo_to_base(&prepared.repo_root, prepared.base_sha.as_deref())?;
        let (_app, state, _config_guard) = setup_replay_runtime(&prepared).await?;
        let replay_batch_path = prepared
            .output_dir
            .join(format!("replay-batch-{:03}.json", self.batch_number));

        let indexer_task =
            state
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
                    current, total, "replay batch progress"
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
pub(crate) async fn setup_replay_runtime(
    prepared: &PreparedSingleRun,
) -> Result<(App, Arc<AppState>, XdgConfigHomeGuard), PrepareError> {
    let runtime_db = init_runtime_db()?;
    let embedding_selection = resolve_eval_embedding_selection(None, None).await?;

    let config_home = prepared.output_dir.join("config");
    fs::create_dir_all(&config_home).map_err(|source| PrepareError::CreateOutputDir {
        path: config_home.clone(),
        source,
    })?;
    let config_guard = XdgConfigHomeGuard::set_to(&config_home);

    let embedding_processor = eval_embedding_processor(&embedding_selection)?;
    let runtime = TestRuntime::new_with_embedding_processor(&runtime_db, embedding_processor)
        .spawn_file_manager()
        .spawn_state_manager()
        .spawn_event_bus();
    let state = runtime.state_arc();

    activate_eval_embedding_runtime(&state, &embedding_selection)?;

    prepare_workspace_for_replay(&state, prepared).await?;

    let app = runtime
        .into_app_with_state_pwd(prepared.repo_root.clone())
        .await;

    Ok((app, state, config_guard))
}
pub(crate) async fn prepare_workspace_for_replay(
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

    run_parse_resolved(Arc::clone(&state.db), &resolved).map_err(|err| {
        PrepareError::DatabaseSetup {
            phase: "replay_run_parse_resolved",
            detail: err.to_string(),
        }
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
pub(crate) fn log_replay_batch_context(batch_number: usize, batch: &[ploke_db::TypedEmbedData]) {
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
