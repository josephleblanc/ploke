#![cfg(feature = "test_harness")]

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use crossterm::event::{Event, KeyCode, KeyEvent, KeyModifiers};
use futures::StreamExt;
use ploke_core::embeddings::{
    EmbeddingModelId, EmbeddingProviderSlug, EmbeddingSet, EmbeddingShape,
};
use ploke_db::Database;
use ploke_db::multi_embedding::db_ext::EmbeddingExt;
use ploke_embed::cancel_token::CancellationToken;
use ploke_embed::{
    config::{OpenRouterConfig, TruncatePolicy},
    indexer::{EmbeddingProcessor, EmbeddingSource, IndexStatus, IndexerTask, IndexingStatus},
    providers::openrouter::OpenRouterBackend,
    runtime::EmbeddingRuntime,
};
use ploke_io::IoManagerHandle;
use ploke_rag::TokenBudget;
use ploke_test_utils::workspace_root;
use ploke_tui::AppEvent;
use ploke_tui::app::commands::harness::TestRuntime;
use ploke_tui::app_state::handlers::indexing::index_workspace;
use ploke_tui::app_state::{
    AppState, ChatState, ConfigState, IndexTargetDir, RuntimeConfig, SystemState,
};
use ratatui::{Terminal, backend::TestBackend};
use tempfile::TempDir;
use tokio::sync::Mutex as TokioMutex;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{Mutex, RwLock};
use tokio::time::{Instant, sleep, timeout};
use tokio_stream::wrappers::UnboundedReceiverStream;
use uuid::Uuid;

const OVERLAY_WAIT_SECS: u64 = 60;
const DIRECT_INDEX_WAIT_SECS: u64 = 300;
const OPENROUTER_CODESTRAL_MODEL: &str = "mistralai/codestral-embed-2505";
const OPENROUTER_CODESTRAL_DIMS: usize = 1536;

fn config_home_lock() -> &'static TokioMutex<()> {
    static LOCK: OnceLock<TokioMutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| TokioMutex::new(()))
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

struct ConfigSandbox {
    _lock: tokio::sync::MutexGuard<'static, ()>,
    _tmp_dir: TempDir,
    _xdg_guard: XdgConfigHomeGuard,
}

async fn setup_config_sandbox() -> ConfigSandbox {
    let lock = config_home_lock().lock().await;
    let tmp_dir = tempfile::tempdir().expect("temp xdg config dir");
    let xdg_guard = XdgConfigHomeGuard::set_to(tmp_dir.path());
    ConfigSandbox {
        _lock: lock,
        _tmp_dir: tmp_dir,
        _xdg_guard: xdg_guard,
    }
}

fn require_openrouter_gate(test_name: &str) {
    if ploke_tui::test_harness::openrouter_env().is_none() {
        eprintln!("skipping {}: OPENROUTER_API_KEY not set", test_name);
        return;
    }
}

fn codestral_config() -> OpenRouterConfig {
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

fn codestral_embedding_set() -> EmbeddingSet {
    EmbeddingSet::new(
        EmbeddingProviderSlug::new_from_str("openrouter"),
        EmbeddingModelId::new_from_str(OPENROUTER_CODESTRAL_MODEL),
        EmbeddingShape::new_dims_default(OPENROUTER_CODESTRAL_DIMS as u32),
    )
}

fn codestral_processor() -> Result<EmbeddingProcessor, Box<dyn std::error::Error>> {
    let backend = OpenRouterBackend::new(&codestral_config())?;
    Ok(EmbeddingProcessor::new(EmbeddingSource::OpenRouter(
        backend,
    )))
}

fn send_key(input_tx: &UnboundedSender<Result<Event, std::io::Error>>, key: KeyEvent) {
    input_tx.send(Ok(Event::Key(key))).expect("send key event");
}

async fn send_command(input_tx: &UnboundedSender<Result<Event, std::io::Error>>, command: &str) {
    for ch in command.chars() {
        send_key(input_tx, KeyEvent::from(KeyCode::Char(ch)));
        tokio::task::yield_now().await;
    }
    send_key(input_tx, KeyEvent::from(KeyCode::Enter));
}

async fn select_codestral_via_overlay_input(
    input_tx: &UnboundedSender<Result<Event, std::io::Error>>,
    state: &Arc<AppState>,
) -> String {
    let deadline = Instant::now() + Duration::from_secs(OVERLAY_WAIT_SECS);
    loop {
        let rel = state
            .db
            .with_active_set(|set| set.rel_name.as_ref().to_string())
            .expect("read active set relation");
        if rel.contains("codestral") {
            return rel;
        }
        if let Some(last_msg) = latest_chat_message(state).await {
            if last_msg.contains("Missing OPENROUTER_API_KEY")
                || last_msg.contains("Failed to query OpenRouter embedding models")
                || last_msg.contains("Failed to build OpenRouter embedder")
                || last_msg.contains("Runtime error setting active_embedding_set")
            {
                panic!("embedding selection failed before activation: {}", last_msg);
            }
        }
        assert!(
            Instant::now() < deadline,
            "timed out waiting for active set relation containing codestral (last={})",
            rel
        );
        send_key(
            input_tx,
            KeyEvent::new(KeyCode::Char('s'), KeyModifiers::NONE),
        );
        sleep(Duration::from_millis(250)).await;
    }
}

async fn latest_chat_message(state: &Arc<AppState>) -> Option<String> {
    let chat = state.chat.0.read().await;
    chat.iter_path().last().map(|msg| msg.content.clone())
}

fn function_has_embedding(
    db: &Database,
    embedding_set: &EmbeddingSet,
    function_name: &str,
) -> bool {
    let vec_rel = embedding_set.rel_name.clone();
    let script = format!(
        r#"?[name] := *function {{ id, name @ 'NOW' }},
            *{vec_rel} {{ node_id: id @ 'NOW' }},
            name = "{function_name}""#
    );
    println!("script:\n{script:#?}");
    let query = db.raw_query(&script).expect("query function embeddings");
    println!("query:\n{query:#?}");
    !query.rows.is_empty()
}

fn build_codestral_index_state() -> Result<Arc<AppState>, Box<dyn std::error::Error>> {
    let db = Arc::new(Database::init_with_schema()?);
    db.setup_multi_embedding()?;

    let runtime = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        codestral_processor()?,
    ));
    runtime.activate(
        &db,
        codestral_embedding_set(),
        Arc::new(codestral_processor()?),
    )?;

    let io_handle = IoManagerHandle::new();
    let (index_cancellation_token, index_cancel_handle) = CancellationToken::new();
    let indexer_task = IndexerTask::new(
        Arc::clone(&db),
        io_handle.clone(),
        Arc::clone(&runtime),
        index_cancellation_token,
        index_cancel_handle,
        None,
    );

    Ok(Arc::new(AppState {
        chat: ChatState::new(ploke_tui::chat_history::ChatHistory::new()),
        config: ConfigState::new(RuntimeConfig::default()),
        system: SystemState::default(),
        indexing_state: RwLock::new(None),
        indexer_task: Some(Arc::new(indexer_task)),
        indexing_control: Arc::new(Mutex::new(None)),
        db,
        embedder: runtime,
        io_handle,
        proposals: RwLock::new(HashMap::new()),
        create_proposals: RwLock::new(HashMap::new()),
        rag: None,
        budget: TokenBudget::default(),
    }))
}

#[tokio::test(flavor = "multi_thread")]
async fn live_codestral_selection_via_overlay_switches_active_set() {
    if ploke_tui::test_harness::openrouter_env().is_none() {
        require_openrouter_gate("live_codestral_selection_via_overlay_switches_active_set");
        return;
    }

    let _sandbox = setup_config_sandbox().await;
    let crate_root = workspace_root().join("tests/fixture_crates/simple_crate");

    let db = Arc::new(Database::init_with_schema().expect("init db"));
    db.setup_multi_embedding().expect("setup multi embedding");

    let rt = TestRuntime::new_with_embedding_processor(&db, EmbeddingProcessor::new_mock())
        .spawn_file_manager()
        .spawn_state_manager()
        .spawn_event_bus();
    let state = rt.state_arc();
    let app = rt.into_app_with_state_pwd(crate_root).await;

    let initial_rel = state
        .db
        .with_active_set(|set| set.rel_name.as_ref().to_string())
        .expect("initial active set");
    assert!(
        initial_rel.contains("MiniLM") || initial_rel.contains("sentence_transformers"),
        "expected initial local embedding relation, got {}",
        initial_rel
    );

    let backend = TestBackend::new(120, 40);
    let terminal = Terminal::new(backend).expect("create terminal");
    let (input_tx, input_rx) =
        tokio::sync::mpsc::unbounded_channel::<Result<Event, std::io::Error>>();
    let input = UnboundedReceiverStream::new(input_rx);

    let app_task = tokio::spawn(async move {
        app.run_with(
            terminal,
            input.fuse(),
            ploke_tui::app::RunOptions {
                setup_terminal_modes: false,
            },
        )
        .await
    });

    send_command(
        &input_tx,
        &format!("/embedding search {OPENROUTER_CODESTRAL_MODEL}"),
    )
    .await;
    let active_rel = select_codestral_via_overlay_input(&input_tx, &state).await;
    assert!(
        active_rel.contains("codestral"),
        "expected active relation to switch to codestral, got {}",
        active_rel
    );
    assert!(
        !active_rel.contains("MiniLM"),
        "active relation should no longer be the default local relation: {}",
        active_rel
    );

    app_task.abort();
    let _ = app_task.await;
}

#[tokio::test(flavor = "multi_thread")]
async fn live_direct_codestral_indexing_completes() -> Result<(), Box<dyn std::error::Error>> {
    if ploke_tui::test_harness::openrouter_env().is_none() {
        require_openrouter_gate("live_direct_codestral_indexing_completes");
        return Ok(());
    }

    let _sandbox = setup_config_sandbox().await;
    let crate_root = workspace_root().join("tests/fixture_crates/simple_crate");
    let state = build_codestral_index_state()?;
    let event_bus = Arc::new(ploke_tui::event_bus::EventBus::new(
        ploke_tui::event_bus::EventBusCaps::default(),
    ));
    let mut index_rx = event_bus.index_subscriber();

    let status_task = tokio::spawn(async move {
        let deadline = Instant::now() + Duration::from_secs(DIRECT_INDEX_WAIT_SECS);
        let mut saw_running = false;
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            assert!(
                !remaining.is_zero(),
                "timed out waiting for raw indexing status"
            );
            match timeout(Duration::from_millis(500).min(remaining), index_rx.recv()).await {
                Ok(Ok(IndexingStatus {
                    status: IndexStatus::Running,
                    ..
                })) => {
                    saw_running = true;
                }
                Ok(Ok(IndexingStatus {
                    status: IndexStatus::Completed,
                    ..
                })) => {
                    return saw_running;
                }
                Ok(Ok(IndexingStatus {
                    status: IndexStatus::Failed(err),
                    ..
                })) => {
                    panic!("raw indexing status reported failure: {}", err);
                }
                Ok(Ok(IndexingStatus {
                    status: IndexStatus::Cancelled,
                    ..
                })) => {
                    panic!("raw indexing status reported cancellation");
                }
                Ok(Ok(_)) => continue,
                Ok(Err(tokio::sync::broadcast::error::RecvError::Lagged(_))) => continue,
                Ok(Err(tokio::sync::broadcast::error::RecvError::Closed)) => {
                    panic!("raw indexing status stream closed");
                }
                Err(_) => continue,
            }
        }
    });

    timeout(Duration::from_secs(DIRECT_INDEX_WAIT_SECS), async {
        index_workspace(
            &state,
            &event_bus,
            Some(IndexTargetDir::new(crate_root.clone())),
            true,
        )
        .await;
    })
    .await
    .expect("direct indexing should complete before timeout");

    assert!(
        status_task.await.expect("status task join"),
        "expected to observe at least one running indexing status before completion"
    );

    let active_set = state.db.with_active_set(|set| set.clone())?;
    assert!(
        active_set.rel_name.as_ref().contains("codestral"),
        "expected active embedding relation to remain codestral, got {}",
        active_set.rel_name
    );
    assert_eq!(
        state.db.count_unembedded_nonfiles()?,
        0,
        "expected all non-file nodes to be embedded after direct indexing"
    );
    assert!(
        function_has_embedding(&state.db, &active_set, "add"),
        "expected the simple_crate::add function to have an embedding"
    );

    Ok(())
}
