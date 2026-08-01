use std::path::PathBuf;
use std::sync::Arc;
#[cfg(feature = "test_harness")]
use std::sync::atomic::{AtomicU64, Ordering};

use ploke_embed::indexer::{IndexStatus, IndexingStatus};
use tokio::sync::mpsc;
use uuid::Uuid;

use crate::EventBus;
use crate::INDEXING_FAILURE_CONTAINMENT_NOTE;
use crate::app_state::{AppState, IndexTarget, IndexTargetDir, handlers};
use crate::chat_history::MessageKind;
use crate::parser::{resolve_index_target, run_parse_resolved};
use crate::utils::parse_errors::{extract_nested_parser_diagnostics, format_parse_failure};

use super::chat::add_msg_immediate;

#[cfg(feature = "test_harness")]
static INDEXING_TEST_DELAY_MS: AtomicU64 = AtomicU64::new(0);

#[cfg(feature = "test_harness")]
pub fn set_indexing_test_delay_ms(delay_ms: u64) {
    INDEXING_TEST_DELAY_MS.store(delay_ms, Ordering::SeqCst);
}

async fn maybe_delay_indexing_for_test() {
    #[cfg(feature = "test_harness")]
    {
        let delay_ms = INDEXING_TEST_DELAY_MS.load(Ordering::SeqCst);
        if delay_ms > 0 {
            tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
        }
    }
}

fn emit_indexing_failed_status(
    event_bus: &Arc<EventBus>,
    detail: String,
    current_file: Option<PathBuf>,
) {
    let _ = event_bus.index_tx.send(IndexingStatus {
        status: IndexStatus::Failed(detail.clone()),
        recent_processed: 0,
        num_not_proc: 0,
        current_file,
        errors: vec![detail],
    });
}

pub async fn index_workspace(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    target_dir: Option<IndexTargetDir>,
    needs_parse: bool,
) {
    let add_msg_shortcut = |msg: &str| {
        handlers::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
        )
    };
    let (control_tx, control_rx) = tokio::sync::mpsc::channel(4);

    let stopgap_pathbuf = target_dir.clone().map(|p| p.as_path().to_path_buf());

    let resolved_target = if let Some(dir) = target_dir {
        let anchored = state
            .with_system_read(|sys| dir.resolve_against_loaded_state(sys))
            .await;
        Some(anchored.unwrap_or(dir).as_path().to_path_buf())
    } else {
        stopgap_pathbuf.clone()
    };

    // Extract pwd from SystemState before calling sync function
    let pwd = state.with_system_read(|sys| sys.pwd().to_path_buf()).await;

    let resolved = match resolve_index_target(resolved_target, &pwd) {
        Ok(resolved) => resolved,
        Err(err) => {
            let msg = err.to_string();
            state
                .with_system_txn(|txn| {
                    txn.record_parse_failure(
                        stopgap_pathbuf
                            .clone()
                            .unwrap_or_else(|| PathBuf::from("todo-add-error-handling")),
                        msg.clone(),
                    );
                })
                .await;
            add_msg_shortcut(&msg).await;
            tracing::warn!(
                "Indexing target resolution failed: {}. {}",
                msg,
                INDEXING_FAILURE_CONTAINMENT_NOTE
            );
            emit_indexing_failed_status(event_bus, msg, stopgap_pathbuf);
            return;
        }
    };

    if needs_parse {
        match run_parse_resolved(Arc::clone(&state.db), &resolved) {
            Ok(_) => {
                tracing::info!(
                    "Parse of target {} successful",
                    resolved.workspace_root.display()
                );
            }
            Err(e) => {
                let msg = format_parse_failure(&resolved.focused_root, &e);
                let diagnostics = extract_nested_parser_diagnostics(&e);
                state
                    .with_system_txn(|txn| {
                        txn.record_parse_failure_with_diagnostics(
                            resolved.requested_path.clone(),
                            msg.clone(),
                            diagnostics.clone(),
                        );
                    })
                    .await;
                emit_indexing_failed_status(event_bus, msg, Some(resolved.focused_root.clone()));
                tracing::warn!(
                    "Failure parsing directory from IndexWorkspace event: {}. {}",
                    e,
                    INDEXING_FAILURE_CONTAINMENT_NOTE
                );
                return;
            }
        }
    }

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
    let policy = outcome.result;
    if let Some(policy) = policy {
        state
            .io_handle
            .update_roots(Some(policy.roots), Some(policy.symlink_policy))
            .await;
    }
    tracing::info!("end parse");

    add_msg_immediate(
        state,
        event_bus,
        Uuid::new_v4(),
        "Indexing...".to_string(),
        crate::chat_history::MessageKind::SysInfo,
    )
    .await;

    maybe_delay_indexing_for_test().await;

    let event_bus_clone = event_bus.clone();
    let progress_tx = Arc::clone(&event_bus.index_tx);
    let progress_rx = event_bus.index_subscriber();

    let state_arc = state.indexer_task.as_ref().map(Arc::clone);
    if let Some(indexer_task) = state_arc
        && let Ok((callback_manager, db_callbacks, unreg_codes_arc, shutdown)) =
            ploke_db::CallbackManager::new_bounded(Arc::clone(&indexer_task.db), 1000)
    {
        let counter = callback_manager.clone_counter();
        let callback_handler = std::thread::spawn(move || callback_manager.run());
        let res = tokio::spawn(async move {
            let indexing_result = ploke_embed::indexer::IndexerTask::index_workspace(
                indexer_task,
                resolved.workspace_root.display().to_string(),
                progress_tx,
                progress_rx,
                control_rx,
                callback_handler,
                db_callbacks,
                counter,
                shutdown,
            )
            .await;
            tracing::info!("Indexer task returned");
            match indexing_result {
                Ok(_) => {
                    tracing::info!("Indexer finished successfully");
                    // SSoT: Do not emit AppEvent here; run_event_bus will forward IndexingStatus::Completed
                }
                Err(e) => {
                    tracing::warn!("Indexer finished with error: {}", e);
                    // SSoT: Do not emit AppEvent here; run_event_bus will forward IndexingStatus::Cancelled/Failed
                }
            }
        })
        .await;
        match res {
            Ok(_) => {
                tracing::info!("Sending Indexing Completed");
            }
            Err(e) => {
                let err_msg = e.to_string();
                add_msg_shortcut(&err_msg).await;
                tracing::warn!("Sending Indexing Failed with error message: {}", err_msg);
            }
        }
        tracing::info!("Indexer task returned");
    }
}

pub async fn index_target(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    target: Option<IndexTarget>,
    needs_parse: bool,
) {
    let add_msg_shortcut = |msg: &str| {
        handlers::chat::add_msg_immediate(
            state,
            event_bus,
            Uuid::new_v4(),
            msg.to_string(),
            MessageKind::SysInfo,
        )
    };

    let Some(target) = target else {
        index_workspace(state, event_bus, None, needs_parse).await;
        return;
    };

    let display_target = target.describe();
    let resolved = state
        .with_system_read(|sys| target.resolve_against_loaded_state(sys))
        .await;

    let Some(target_dir) = resolved else {
        let msg = format!(
            "Indexing target resolution failed: {display_target} is not available in loaded state."
        );
        state
            .with_system_txn(|txn| {
                txn.record_parse_failure(PathBuf::from(display_target.clone()), msg.clone());
            })
            .await;
        add_msg_shortcut(&msg).await;
        tracing::warn!(
            "Semantic indexing target resolution failed: {}. {}",
            msg,
            INDEXING_FAILURE_CONTAINMENT_NOTE
        );
        emit_indexing_failed_status(event_bus, msg, None);
        return;
    };

    index_workspace(state, event_bus, Some(target_dir), needs_parse).await;
}

#[cfg(test)]
mod tests {
    use crate::app_state::core::SystemStatus;
    use crate::app_state::{IndexTarget, IndexTargetDir};
    use crate::event_bus::{EventBus, EventBusCaps};
    use crate::test_utils::mock::create_mock_app_state;
    use ploke_core::CrateId;
    use ploke_embed::indexer::IndexStatus;
    use std::path::PathBuf;
    use std::sync::Arc;
    use tempfile::tempdir;
    use tokio::time::{Duration, timeout};

    /// Regression witness for the `test_update_embed` failure mode.
    ///
    /// The `IndexWorkspace` command historically carried a repo-relative string
    /// while `ploke-tui` already knew the loaded crate root as an absolute path.
    /// Re-resolving the string from process cwd silently broke that agreement.
    /// A pass here proves `IndexTargetDir::resolve_against_loaded_state` can
    /// recover the authoritative absolute path from loaded state.
    #[test]
    fn anchor_relative_target_matches_loaded_crate_suffix() {
        let focused_root = PathBuf::from("/repo/tests/fixture_crates/fixture_update_embed");
        let mut status = SystemStatus::default();
        status.set_focus_from_root(focused_root.clone());

        let target = IndexTargetDir::from("tests/fixture_crates/fixture_update_embed");
        let resolved = target.resolve_against_loaded_state(&status);

        assert_eq!(
            resolved.map(|d| d.as_path().to_path_buf()),
            Some(focused_root)
        );
    }

    /// Anchoring logic must not invent matches for unrelated relative paths;
    /// those should fall through to normal resolution and explicit errors.
    #[test]
    fn anchor_relative_target_does_not_match_unrelated_suffix() {
        let member_root = PathBuf::from("/repo/tests/fixture_workspace/ws_fixture_01/member_root");
        let mut status = SystemStatus::default();
        status.set_loaded_workspace(
            PathBuf::from("/repo/tests/fixture_workspace/ws_fixture_01"),
            vec![member_root],
            None,
        );

        let target = IndexTargetDir::from("other/location");
        let resolved = target.resolve_against_loaded_state(&status);

        assert!(resolved.is_none());
    }

    #[tokio::test]
    async fn loaded_crate_target_uses_loaded_root_instead_of_pwd_resolution() {
        let fixture_root = std::fs::canonicalize(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../tests/fixture_crates/fixture_update_embed"),
        )
        .expect("canonical fixture root");
        let crate_id = CrateId::from_root_path(&fixture_root);

        let state = Arc::new(create_mock_app_state());
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

        state
            .with_system_txn(|txn| {
                txn.set_pwd(PathBuf::from("/tmp/not-the-crate-root"));
                txn.set_focus_from_root(fixture_root.clone());
            })
            .await;

        super::index_target(
            &state,
            &event_bus,
            Some(IndexTarget::LoadedCrate(crate_id)),
            false,
        )
        .await;

        let failure = state
            .with_system_read(|sys| sys.last_parse_failure().cloned())
            .await;
        assert!(
            failure.is_none(),
            "unexpected parse failure after semantic target resolution: {:?}",
            failure
        );
        assert_eq!(
            state.system.loaded_workspace_root_for_test().await,
            Some(fixture_root)
        );
    }

    #[tokio::test]
    async fn parse_failure_emits_failed_index_status_before_indexer_runs() {
        let temp = tempdir().expect("tempdir");
        let crate_root = temp.path();
        std::fs::create_dir_all(crate_root.join("src")).expect("create src");
        std::fs::write(
            crate_root.join("Cargo.toml"),
            "[package]\nname = \"broken_fixture\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
        )
        .expect("write Cargo.toml");
        std::fs::write(crate_root.join("src/lib.rs"), "pub fn broken( {\n")
            .expect("write broken lib.rs");

        let state = Arc::new(create_mock_app_state());
        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let mut index_rx = event_bus.index_subscriber();

        super::index_workspace(
            &state,
            &event_bus,
            Some(IndexTargetDir::new(crate_root.to_path_buf())),
            true,
        )
        .await;

        let status = timeout(Duration::from_secs(2), index_rx.recv())
            .await
            .expect("expected indexing failure status")
            .expect("index status channel open");

        match status.status {
            IndexStatus::Failed(detail) => {
                assert!(
                    detail.contains("Parse failed for crate"),
                    "unexpected detail: {detail}"
                );
                assert!(
                    detail.contains(&crate_root.display().to_string()),
                    "detail should mention failing crate root: {detail}"
                );
            }
            other => panic!("expected failed indexing status, got {other:?}"),
        }

        let last_failure = state
            .with_system_read(|sys| sys.last_parse_failure().cloned())
            .await
            .expect("parse failure recorded");
        assert!(last_failure.message.contains("Parse failed for crate"));
        assert!(
            last_failure
                .diagnostics
                .iter()
                .filter_map(|diag| diag.source_path.as_ref())
                .any(|path| path.ends_with("src/lib.rs")),
            "expected broken source path in diagnostics: {:?}",
            last_failure.diagnostics
        );
    }
}

pub async fn pause(state: &Arc<AppState>) {
    if let Some(ctrl) = &mut *state.indexing_control.lock().await {
        let _ = ctrl.send(ploke_embed::indexer::IndexerCommand::Pause).await;
    }
}

pub async fn resume(state: &Arc<AppState>) {
    if let Some(ctrl) = &mut *state.indexing_control.lock().await {
        let _ = ctrl
            .send(ploke_embed::indexer::IndexerCommand::Resume)
            .await;
    }
}

pub async fn cancel(state: &Arc<AppState>) {
    if let Some(ctrl) = &mut *state.indexing_control.lock().await {
        let _ = ctrl
            .send(ploke_embed::indexer::IndexerCommand::Cancel)
            .await;
    }
}
