use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot};
use tokio::time::{Duration, timeout};
use tokio_stream::wrappers::ReceiverStream;
use tracing::info;
use tracing::subscriber::DefaultGuard;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;

use ploke_tui as tui;
use tui::app::App;
use tui::app::RunOptions;
use tui::app_state::{
    AppState, ChatState, ConfigState, RuntimeConfig, StateCommand, SystemState,
};
use tui::chat_history::ChatHistory;
use tui::event_bus::{EventBus, EventBusCaps, run_event_bus};
use tui::tools::ToolVerbosity;
use tui::user_config::CommandStyle;
use tui::{AppEvent, RagEvent};

use ploke_db::Database;
use ploke_embed::cancel_token::CancellationToken;
use ploke_embed::indexer::{EmbeddingProcessor, IndexerTask};
use ploke_embed::runtime::EmbeddingRuntime;
use ploke_io::IoManagerHandle;
use ploke_rag::TokenBudget;

struct TestLogGuard {
    _subscriber_guard: DefaultGuard,
    _writer_guard: WorkerGuard,
}

fn init_test_logging() -> TestLogGuard {
    let log_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/reports");
    std::fs::create_dir_all(&log_dir).expect("create test reports dir");
    let file_appender = tracing_appender::rolling::never(&log_dir, "indexing_freeze_app_loop.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("indexing_freeze_app_loop", LevelFilter::INFO);

    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(filter);

    let subscriber_guard = tracing::subscriber::set_default(subscriber);

    TestLogGuard {
        _subscriber_guard: subscriber_guard,
        _writer_guard: guard,
    }
}

async fn send_key(
    input_tx: &mpsc::Sender<crossterm::event::Event>,
    code: crossterm::event::KeyCode,
) {
    let event = crossterm::event::Event::Key(crossterm::event::KeyEvent::new(
        code,
        crossterm::event::KeyModifiers::NONE,
    ));
    let _ = input_tx.send(event).await;
}

async fn wait_for_command(
    cmd_rx: &mut mpsc::Receiver<String>,
    expected: &str,
    wait_ms: u64,
) -> bool {
    timeout(Duration::from_millis(wait_ms), async {
        loop {
            if let Some(content) = cmd_rx.recv().await {
                if content == expected {
                    break;
                }
            }
        }
    })
    .await
    .is_ok()
}

#[tokio::test]
async fn index_start_keeps_ui_responsive_in_app_loop() {
    let _log_guard = init_test_logging();
    info!(target: "indexing_freeze_app_loop", "starting app-loop indexing regression test");

    let test_body = async {
        tui::app_state::set_indexing_test_delay_ms(5_000);

        let db = Arc::new(Database::new_init().expect("init db"));
        let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            EmbeddingProcessor::new_mock(),
        ));
        let io_handle = IoManagerHandle::new();
        let (cancel_token, cancel_handle) = CancellationToken::new();
        let indexer_task = Arc::new(IndexerTask::new(
            Arc::clone(&db),
            io_handle.clone(),
            Arc::clone(&embedder),
            cancel_token,
            cancel_handle,
            8,
        ));

        let state = Arc::new(AppState {
            chat: ChatState::new(ChatHistory::new()),
            config: ConfigState::new(RuntimeConfig::default()),
            system: SystemState::default(),
            indexing_state: RwLock::new(None),
            indexer_task: Some(indexer_task),
            indexing_control: Arc::new(Mutex::new(None)),
            db,
            embedder,
            io_handle,
            proposals: RwLock::new(HashMap::new()),
            create_proposals: RwLock::new(HashMap::new()),
            rag: None,
            budget: TokenBudget::default(),
        });

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (cmd_tx_state, cmd_rx_state) = mpsc::channel::<StateCommand>(64);
        let (cmd_tx_app, mut cmd_rx_app) = mpsc::channel::<StateCommand>(64);
        let (cmd_obs_tx, mut cmd_obs_rx) = mpsc::channel::<String>(8);
        let (ctx_tx, _ctx_rx) = mpsc::channel::<RagEvent>(8);
        {
            let state = Arc::clone(&state);
            let event_bus = Arc::clone(&event_bus);
            tokio::spawn(tui::app_state::state_manager(
                state,
                cmd_rx_state,
                event_bus,
                ctx_tx,
            ));
        }
        tokio::spawn(run_event_bus(Arc::clone(&event_bus)));
        let cmd_tx_forward = cmd_tx_state.clone();
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx_app.recv().await {
                if let StateCommand::AddUserMessage { content, .. } = &cmd {
                    let _ = cmd_obs_tx.send(content.clone()).await;
                }
                let _ = cmd_tx_forward.send(cmd).await;
            }
        });

        let app = App::new(
            CommandStyle::Slash,
            Arc::clone(&state),
            cmd_tx_app.clone(),
            &event_bus,
            "openai/gpt-4o".to_string(),
            ToolVerbosity::Normal,
        );
        let (input_tx, input_rx) = mpsc::channel::<crossterm::event::Event>(32);
        let input_stream = ReceiverStream::new(input_rx).map(Ok);
        let terminal = Terminal::new(TestBackend::new(100, 30)).expect("terminal");

        let app_task = tokio::spawn(async move {
            app.run_with(
                terminal,
                input_stream,
                RunOptions {
                    setup_terminal_modes: false,
                },
            )
            .await
        });

        for c in "ping".chars() {
            send_key(&input_tx, crossterm::event::KeyCode::Char(c)).await;
        }
        send_key(&input_tx, crossterm::event::KeyCode::Enter).await;

        info!(target: "indexing_freeze_app_loop", "waiting for baseline user message");
        let baseline_ok = wait_for_command(&mut cmd_obs_rx, "ping", 1000).await;
        assert!(baseline_ok, "baseline input was not processed");

        cmd_tx_state
            .send(StateCommand::IndexWorkspace {
                workspace: "tests/fixture_crates/fixture_nodes".to_string(),
                needs_parse: true,
            })
            .await
            .expect("send index start");

        tokio::time::sleep(Duration::from_millis(150)).await;

        for c in "hello".chars() {
            send_key(&input_tx, crossterm::event::KeyCode::Char(c)).await;
        }
        send_key(&input_tx, crossterm::event::KeyCode::Enter).await;

        info!(target: "indexing_freeze_app_loop", "waiting for user message to land");
        let observed = wait_for_command(&mut cmd_obs_rx, "hello", 1000).await;

        info!(target: "indexing_freeze_app_loop", "user message observed = {}", observed);
        assert!(
            observed,
            "app loop did not process input while indexing"
        );

        let _ = event_bus.realtime_tx.send(AppEvent::Quit);
        let _ = app_task.await;

        tui::app_state::set_indexing_test_delay_ms(0);
    };

    let result = timeout(Duration::from_secs(60), test_body).await;
    assert!(result.is_ok(), "test timed out after 60s");
}

#[tokio::test]
async fn indexing_completed_event_does_not_block_input_when_system_read_held() {
    let _log_guard = init_test_logging();
    info!(target: "indexing_freeze_app_loop", "starting system-lock regression test");

    let test_body = async {
        let db = Arc::new(Database::new_init().expect("init db"));
        let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
            Arc::clone(&db.active_embedding_set),
            EmbeddingProcessor::new_mock(),
        ));
        let io_handle = IoManagerHandle::new();
        let (cancel_token, cancel_handle) = CancellationToken::new();
        let indexer_task = Arc::new(IndexerTask::new(
            Arc::clone(&db),
            io_handle.clone(),
            Arc::clone(&embedder),
            cancel_token,
            cancel_handle,
            8,
        ));

        let state = Arc::new(AppState {
            chat: ChatState::new(ChatHistory::new()),
            config: ConfigState::new(RuntimeConfig::default()),
            system: SystemState::default(),
            indexing_state: RwLock::new(None),
            indexer_task: Some(indexer_task),
            indexing_control: Arc::new(Mutex::new(None)),
            db,
            embedder,
            io_handle,
            proposals: RwLock::new(HashMap::new()),
            create_proposals: RwLock::new(HashMap::new()),
            rag: None,
            budget: TokenBudget::default(),
        });

        let focus_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixture_crates/fixture_nodes");
        {
            let mut sys = state.system.write().await;
            sys.set_focus_from_root(focus_path);
        }

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (cmd_tx_state, cmd_rx_state) = mpsc::channel::<StateCommand>(64);
        let (cmd_tx_app, mut cmd_rx_app) = mpsc::channel::<StateCommand>(64);
        let (cmd_obs_tx, mut cmd_obs_rx) = mpsc::channel::<String>(8);
        let (ctx_tx, _ctx_rx) = mpsc::channel::<RagEvent>(8);
        {
            let state = Arc::clone(&state);
            let event_bus = Arc::clone(&event_bus);
            tokio::spawn(tui::app_state::state_manager(
                state,
                cmd_rx_state,
                event_bus,
                ctx_tx,
            ));
        }
        tokio::spawn(run_event_bus(Arc::clone(&event_bus)));
        let cmd_tx_forward = cmd_tx_state.clone();
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx_app.recv().await {
                if let StateCommand::AddUserMessage { content, .. } = &cmd {
                    let _ = cmd_obs_tx.send(content.clone()).await;
                }
                let _ = cmd_tx_forward.send(cmd).await;
            }
        });

        let app = App::new(
            CommandStyle::Slash,
            Arc::clone(&state),
            cmd_tx_app.clone(),
            &event_bus,
            "openai/gpt-4o".to_string(),
            ToolVerbosity::Normal,
        );
        let (input_tx, input_rx) = mpsc::channel::<crossterm::event::Event>(32);
        let input_stream = ReceiverStream::new(input_rx).map(Ok);
        let terminal = Terminal::new(TestBackend::new(100, 30)).expect("terminal");

        let app_task = tokio::spawn(async move {
            app.run_with(
                terminal,
                input_stream,
                RunOptions {
                    setup_terminal_modes: false,
                },
            )
            .await
        });

        for c in "ping".chars() {
            send_key(&input_tx, crossterm::event::KeyCode::Char(c)).await;
        }
        send_key(&input_tx, crossterm::event::KeyCode::Enter).await;

        info!(target: "indexing_freeze_app_loop", "waiting for baseline user message");
        let baseline_ok = wait_for_command(&mut cmd_obs_rx, "ping", 1000).await;
        assert!(baseline_ok, "baseline input was not processed");

        let (hold_tx, hold_rx) = oneshot::channel::<()>();
        let state_for_lock = Arc::clone(&state);
        tokio::spawn(async move {
            let _guard = state_for_lock.system.read().await;
            let _ = hold_rx.await;
        });

        let _ = event_bus.realtime_tx.send(AppEvent::IndexingCompleted);
        tokio::time::sleep(Duration::from_millis(50)).await;

        for c in "hello".chars() {
            send_key(&input_tx, crossterm::event::KeyCode::Char(c)).await;
        }
        send_key(&input_tx, crossterm::event::KeyCode::Enter).await;

        info!(target: "indexing_freeze_app_loop", "waiting for user message to land");
        let observed = wait_for_command(&mut cmd_obs_rx, "hello", 1000).await;

        let _ = hold_tx.send(());
        let _ = event_bus.realtime_tx.send(AppEvent::Quit);
        app_task.abort();
        let _ = app_task.await;

        assert!(
            observed,
            "app loop did not process input while indexing completion event was pending"
        );
    };

    let result = timeout(Duration::from_secs(20), test_body).await;
    assert!(result.is_ok(), "test timed out after 20s");
}
