use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::Arc;

use futures::StreamExt;
use ploke_tui::app_state::IndexTargetDir;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tempfile::tempdir;
use tokio::sync::{Mutex, RwLock, mpsc, oneshot, watch};
use tokio::time::{Duration, timeout};
use tokio_stream::wrappers::ReceiverStream;
use tracing::info;
use tracing::subscriber::DefaultGuard;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;

use ploke_tui as tui;
use tui::CancelChatToken;
use tui::app::App;
use tui::app::RunOptions;
use tui::app_state::{AppState, ChatState, ConfigState, RuntimeConfig, StateCommand, SystemState};
use tui::chat_history::ChatHistory;
use tui::error::ErrorSeverity;
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
        .with(
            tracing_subscriber::fmt::layer()
                .with_writer(non_blocking)
                .with_ansi(false),
        )
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
            None,
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
        let (cancel_tx, _cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
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
            cancel_tx.clone(),
            std::env::current_dir().expect("current dir"),
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
            .send(StateCommand::IndexTargetDir {
                target_dir: Some(IndexTargetDir::new(PathBuf::from(
                    "tests/fixture_crates/fixture_nodes",
                ))),
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
        assert!(observed, "app loop did not process input while indexing");

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
            None,
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

        let focus_path =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixture_crates/fixture_nodes");
        state
            .with_system_txn(|txn| {
                txn.set_focus_from_root(focus_path);
            })
            .await;

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
        let (cancel_tx, _cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
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
            cancel_tx.clone(),
            std::env::current_dir().expect("current dir"),
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
            // Intentionally hold the lock across await to simulate concurrent access
            let _guard = state_for_lock.system_raw_read_guard().await;
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

#[tokio::test]
async fn indexing_setup_failure_emits_warning_and_keeps_app_loop_alive() {
    let _log_guard = init_test_logging();
    info!(
        target: "indexing_freeze_app_loop",
        "starting setup-failure containment regression test"
    );

    let test_body = async {
        let temp = tempdir().expect("tempdir");
        let crate_root = temp.path().join("broken_index_target");
        fs::create_dir_all(crate_root.join("src")).expect("create src dir");
        fs::write(
            crate_root.join("Cargo.toml"),
            "[package]\nname = \"broken_index_target\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[lib]\npath = \"src/lib.rs\"\n",
        )
        .expect("write Cargo.toml");
        fs::write(
            crate_root.join("src/lib.rs"),
            "pub fn broken( {\n    let x = 1;\n}\n",
        )
        .expect("write invalid lib.rs");

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
            None,
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
        let mut rt_obs_rx = event_bus.subscribe(tui::EventPriority::Realtime);
        let mut bg_obs_rx = event_bus.subscribe(tui::EventPriority::Background);
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
        let (cancel_tx, _cancel_rx) = watch::channel(CancelChatToken::KeepOpen);
        let cmd_tx_forward = cmd_tx_state.clone();
        tokio::spawn(async move {
            while let Some(cmd) = cmd_rx_app.recv().await {
                if let StateCommand::AddUserMessage { content, .. } = &cmd {
                    let _ = cmd_obs_tx.send(content.clone()).await;
                }
                let _ = cmd_tx_forward.send(cmd).await;
            }
        });

        timeout(Duration::from_secs(1), async {
            loop {
                match rt_obs_rx.recv().await {
                    Ok(AppEvent::EventBusStarted) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        })
        .await
        .expect("timeout waiting for EventBusStarted");

        let app = App::new(
            CommandStyle::Slash,
            Arc::clone(&state),
            cmd_tx_app.clone(),
            &event_bus,
            "openai/gpt-4o".to_string(),
            ToolVerbosity::Normal,
            cancel_tx.clone(),
            std::env::current_dir().expect("current dir"),
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

        let baseline_ok = wait_for_command(&mut cmd_obs_rx, "ping", 1000).await;
        assert!(baseline_ok, "baseline input was not processed");

        cmd_tx_state
            .send(StateCommand::IndexTargetDir {
                target_dir: Some(IndexTargetDir::new(crate_root.clone())),
                needs_parse: true,
            })
            .await
            .expect("send invalid index target");

        let warning_event = timeout(Duration::from_secs(2), async {
            loop {
                match bg_obs_rx.recv().await {
                    Ok(AppEvent::Error(error))
                        if error
                            .message
                            .contains("Indexing failed: Parse failed for crate") =>
                    {
                        break error;
                    }
                    Ok(_) => continue,
                    Err(err) => panic!("background event stream failed: {err}"),
                }
            }
        })
        .await
        .expect("timeout waiting for indexing warning");

        assert!(matches!(warning_event.severity, ErrorSeverity::Warning));
        assert!(warning_event.message.contains("before the next release"));

        timeout(Duration::from_secs(2), async {
            loop {
                match rt_obs_rx.recv().await {
                    Ok(AppEvent::IndexingFailed) => break,
                    Ok(_) => continue,
                    Err(err) => panic!("realtime event stream failed: {err}"),
                }
            }
        })
        .await
        .expect("timeout waiting for IndexingFailed");

        assert!(
            !app_task.is_finished(),
            "app task ended unexpectedly after handled indexing failure"
        );

        for c in "hello".chars() {
            send_key(&input_tx, crossterm::event::KeyCode::Char(c)).await;
        }
        send_key(&input_tx, crossterm::event::KeyCode::Enter).await;

        let observed = wait_for_command(&mut cmd_obs_rx, "hello", 1000).await;
        assert!(
            observed,
            "app loop did not process input after indexing failure"
        );
        assert!(
            !app_task.is_finished(),
            "app task ended unexpectedly after post-failure input"
        );

        let _ = event_bus.realtime_tx.send(AppEvent::Quit);
        match app_task.await {
            Ok(Ok(())) => {}
            Ok(Err(err)) => panic!("app runtime returned error after quit: {err}"),
            Err(err) => panic!("app runtime panicked: {err}"),
        }
    };

    let result = timeout(Duration::from_secs(20), test_body).await;
    assert!(result.is_ok(), "test timed out after 20s");
}
