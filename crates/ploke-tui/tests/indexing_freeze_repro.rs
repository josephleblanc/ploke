use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::time::{Duration, timeout};
use tracing::info;
use tracing::subscriber::DefaultGuard;
use tracing_appender::non_blocking::WorkerGuard;
use tracing_subscriber::filter::LevelFilter;
use tracing_subscriber::layer::SubscriberExt;
use uuid::Uuid;

use ploke_tui as tui;
use tui::app_state::{
    AppState, ChatState, ConfigState, RuntimeConfig, StateCommand, SystemState,
};
use tui::chat_history::{ChatHistory, MessageKind};
use tui::event_bus::{EventBus, EventBusCaps};

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
    let file_appender = tracing_appender::rolling::never(&log_dir, "indexing_freeze_test.log");
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    let filter = tracing_subscriber::filter::Targets::new()
        .with_target("indexing_freeze_test", LevelFilter::INFO);

    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer().with_writer(non_blocking).with_ansi(false))
        .with(filter);

    let subscriber_guard = tracing::subscriber::set_default(subscriber);

    TestLogGuard {
        _subscriber_guard: subscriber_guard,
        _writer_guard: guard,
    }
}

#[tokio::test]
async fn index_start_fixture_nodes_does_not_freeze_ui() {
    let _log_guard = init_test_logging();
    info!(target: "indexing_freeze_test", "starting indexing freeze repro test");
    tui::app_state::set_indexing_test_delay_ms(5_000);

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

        let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(64);
        let (ctx_tx, _ctx_rx) = mpsc::channel(8);
        {
            let state = Arc::clone(&state);
            let event_bus = Arc::clone(&event_bus);
            tokio::spawn(tui::app_state::state_manager(state, cmd_rx, event_bus, ctx_tx));
        }

        let fixture_workspace = "tests/fixture_crates/fixture_nodes";
        info!(target: "indexing_freeze_test", "sending index start for {fixture_workspace}");
        cmd_tx
            .send(StateCommand::IndexWorkspace {
                workspace: fixture_workspace.to_string(),
                needs_parse: true,
            })
            .await
            .expect("send index start");

        let marker_id = Uuid::new_v4();
        cmd_tx
            .send(StateCommand::AddMessageImmediate {
                msg: "marker".to_string(),
                kind: MessageKind::SysInfo,
                new_msg_id: marker_id,
            })
            .await
            .expect("send marker");

        info!(target: "indexing_freeze_test", "waiting for marker to land in chat");
        let observed = timeout(Duration::from_millis(200), async {
            loop {
                let has_marker = state.chat.0.read().await.messages.contains_key(&marker_id);
                if has_marker {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        })
        .await;

        info!(target: "indexing_freeze_test", "marker observed = {}", observed.is_ok());
        assert!(
            observed.is_ok(),
            "state manager did not process commands while indexing"
        );
    };

    let result = timeout(Duration::from_secs(60), test_body).await;
    info!(target: "indexing_freeze_test", "test body completed = {}", result.is_ok());
    assert!(result.is_ok(), "test timed out after 60s");
    tui::app_state::set_indexing_test_delay_ms(0);
}
