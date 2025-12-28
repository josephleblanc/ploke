use std::collections::HashMap;
use std::sync::Arc;

use tokio::sync::{Mutex, RwLock, mpsc};
use tokio::time::{Duration, timeout};
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

#[tokio::test]
async fn index_start_does_not_block_state_manager() {
    tui::app_state::set_indexing_test_delay_ms(500);

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

    cmd_tx
        .send(StateCommand::IndexWorkspace {
            workspace: "tests/fixture_crates/fixture_nodes".to_string(),
            needs_parse: false,
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

    let observed = timeout(Duration::from_millis(200), async {
        loop {
            let has_marker = state.chat.0.read().await.messages.contains_key(&marker_id);
            if has_marker {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await;

    tui::app_state::set_indexing_test_delay_ms(0);

    assert!(
        observed.is_ok(),
        "state manager did not process commands while indexing"
    );
}
