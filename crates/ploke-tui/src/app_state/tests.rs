use std::ops::Index;
use std::sync::Arc;

use cozo::DataValue;
use ploke_db::QueryResult;
use ploke_embed::local::EmbeddingConfig;
use ploke_rag::RagService;
use ploke_core::ArcStr;
use syn_parser::parser::nodes::ToCozoUuid;

use crate::tracing_setup::init_tracing;
use crate::app::message_item::should_render_tool_buttons;
use crate::app_state::handlers::chat;
use crate::chat_history::MessageKind;
use crate::tools::{ToolName, ToolUiPayload};

use super::*;
use ploke_embed::{
    indexer::{EmbeddingProcessor, EmbeddingSource},
    local::LocalEmbedder,
    runtime::EmbeddingRuntime,
};
use rand::Rng;
use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, sleep};
use uuid::Uuid;

pub trait MockTrait {
    fn mock() -> Self;
}

// Mock implementations for testing
impl MockTrait for EmbeddingProcessor {
    fn mock() -> Self {
        // Simple mock that does nothing
        Self::new(EmbeddingSource::Local(
            LocalEmbedder::new(EmbeddingConfig::default())
                .expect("LocalEmbedder failed to construct within test - should not happen"),
        ))
    }
}

fn mock_runtime(db: &Arc<ploke_db::Database>) -> Arc<EmbeddingRuntime> {
    Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        EmbeddingProcessor::mock(),
    ))
}

impl MockTrait for ploke_io::IoManagerHandle {
    fn mock() -> Self {
        ploke_io::IoManagerHandle::new()
    }
}

#[tokio::test]
#[ignore = "needs refactor"]
async fn test_race_condition_without_oneshot() {
    let db = Arc::new(ploke_db::Database::new_init().unwrap());
    let mock_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        EmbeddingProcessor::mock(),
    ));
    let rag = Arc::new(RagService::new(db.clone(), Arc::clone(&mock_runtime)).unwrap());
    let (rag_tx, _) = mpsc::channel::<RagEvent>(100);
    let state = Arc::new(AppState::new(
        db.clone(),
        Arc::clone(&mock_runtime),
        ploke_io::IoManagerHandle::mock(),
        rag,
        TokenBudget::default(),
        rag_tx,
    ));
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    // Start state manager
    tokio::spawn(super::dispatcher::state_manager(
        state.clone(),
        cmd_rx,
        event_bus.clone(),
        mpsc::channel(32).0,
    ));

    let user_msg_id = Uuid::new_v4();
    let embed_msg_id = Uuid::new_v4();

    // Simulate sending both commands concurrently without synchronization
    let tx1 = cmd_tx.clone();
    let tx2 = cmd_tx.clone();

    tokio::join!(
        async {
            tx1.send(super::commands::StateCommand::AddUserMessage {
                content: "tell me a haiku".to_string(),
                completion_tx: oneshot::channel().0,
                new_user_msg_id: user_msg_id,
            })
            .await
            .unwrap();
        },
        async {
            tx2.send(super::commands::StateCommand::EmbedMessage {
                new_msg_id: embed_msg_id,
                completion_rx: oneshot::channel().1, // dummy
                scan_rx: oneshot::channel().1,       // dummy
            })
            .await
            .unwrap();
        }
    );

    tokio::time::sleep(Duration::from_millis(100)).await;

    // Check if the embed message read the user message or not
    let chat = state.chat.0.read().await;
    let last_user_msg = chat.last_user_msg();
    assert!(
        last_user_msg.is_ok_and(|m| m.is_some_and(|im| !im.1.is_empty())),
        "User message should be present"
    );
}

#[tokio::test]
async fn test_fix_with_oneshot() {
    let db = Arc::new(ploke_db::Database::new_init().unwrap());
    let mock_runtime = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        EmbeddingProcessor::mock(),
    ));
    let rag = Arc::new(RagService::new(db.clone(), Arc::clone(&mock_runtime)).unwrap());
    let (rag_tx, _) = mpsc::channel::<RagEvent>(100);
    let state = Arc::new(AppState::new(
        db.clone(),
        Arc::clone(&mock_runtime),
        ploke_io::IoManagerHandle::mock(),
        rag,
        TokenBudget::default(),
        rag_tx,
    ));
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    // Start state manager
    tokio::spawn(super::dispatcher::state_manager(
        state.clone(),
        cmd_rx,
        event_bus.clone(),
        mpsc::channel(32).0,
    ));

    let user_msg_id = Uuid::new_v4();
    let embed_msg_id = Uuid::new_v4();

    let (tx, rx) = oneshot::channel();

    cmd_tx
        .send(super::commands::StateCommand::AddUserMessage {
            content: "tell me a haiku".to_string(),
            new_user_msg_id: user_msg_id,
            completion_tx: tx,
        })
        .await
        .unwrap();

    cmd_tx
        .send(super::commands::StateCommand::EmbedMessage {
            new_msg_id: embed_msg_id,
            completion_rx: rx,
            // TODO: revisit this test
            scan_rx: oneshot::channel().1, // dummy
        })
        .await
        .unwrap();

    tokio::time::sleep(Duration::from_millis(100)).await;

    let chat = state.chat.0.read().await;
    let last_user_msg = chat.last_user_msg();
    assert!(
        last_user_msg.is_ok_and(|m| m.is_some_and(|im| !im.1.is_empty())),
        "User message should always be present"
    );
}

#[tokio::test]
#[ignore = "test broken, cause unclear, non-trivial fix needs attention"]
async fn test_concurrency_with_fuzzing() {
    let db = Arc::new(ploke_db::Database::new_init().unwrap());
    let mock_runtime = mock_runtime(&db);
    let rag = Arc::new(RagService::new(db.clone(), Arc::clone(&mock_runtime)).unwrap());
    let (rag_tx, _) = mpsc::channel::<RagEvent>(100);
    let state = Arc::new(AppState::new(
        db.clone(),
        Arc::clone(&mock_runtime),
        ploke_io::IoManagerHandle::mock(),
        rag,
        TokenBudget::default(),
        rag_tx,
    ));
    let (cmd_tx, cmd_rx) = mpsc::channel(32);
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    // Start state manager
    tokio::spawn(super::dispatcher::state_manager(
        state.clone(),
        cmd_rx,
        event_bus.clone(),
        mpsc::channel(32).0,
    ));

    let mut rng = rand::rng();

    // Send 50 pairs of commands with random delays
    for i in 0..50 {
        let delay_ms = rng.random_range(5..=20);
        sleep(Duration::from_millis(delay_ms)).await;

        let user_msg_id = Uuid::new_v4();
        let embed_msg_id = Uuid::new_v4();
        let (tx, rx) = oneshot::channel();

        // Send both commands
        cmd_tx
            .send(super::commands::StateCommand::AddUserMessage {
                content: format!("message {}", i),
                new_user_msg_id: user_msg_id,
                completion_tx: tx,
            })
            .await
            .unwrap();

        cmd_tx
            .send(super::commands::StateCommand::EmbedMessage {
                new_msg_id: embed_msg_id,
                completion_rx: rx,
                // TODO: Revisit and update this test
                scan_rx: oneshot::channel().1, // dummy
            })
            .await
            .unwrap();
    }

    tokio::time::sleep(Duration::from_millis(500)).await;

    // Verify all messages were processed
    let chat = state.chat.0.read().await;
    let messages = chat.messages.len();
    assert!(messages >= 50, "Should have processed at least 50 messages");
}

#[tokio::test]
async fn test_tool_message_refresh_and_background_sysinfo() {
    let db = Arc::new(ploke_db::Database::new_init().unwrap());
    let mock_runtime = mock_runtime(&db);
    let rag = Arc::new(RagService::new(db.clone(), Arc::clone(&mock_runtime)).unwrap());
    let (rag_tx, _) = mpsc::channel::<RagEvent>(100);
    let state = Arc::new(AppState::new(
        db,
        Arc::clone(&mock_runtime),
        ploke_io::IoManagerHandle::mock(),
        rag,
        TokenBudget::default(),
        rag_tx,
    ));
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));

    let user_msg_id = Uuid::new_v4();
    chat::add_msg_immediate(
        &state,
        &event_bus,
        user_msg_id,
        "hello".to_string(),
        MessageKind::User,
    )
    .await;

    let tool_msg_id = Uuid::new_v4();
    let call_id = ArcStr::from("test_call_id");
    let request_id = Uuid::new_v4();
    let pending_payload = ToolUiPayload::new(ToolName::ApplyCodeEdit, call_id.clone(), "staged")
        .with_request_id(request_id)
        .with_field("status", "pending");
    assert!(should_render_tool_buttons(&pending_payload));
    chat::add_tool_msg_immediate(
        &state,
        &event_bus,
        tool_msg_id,
        "proposal".to_string(),
        call_id.clone(),
        Some(pending_payload),
    )
    .await;

    let child_msg_id = Uuid::new_v4();
    chat::add_msg_immediate(
        &state,
        &event_bus,
        child_msg_id,
        "child".to_string(),
        MessageKind::Assistant,
    )
    .await;

    {
        let mut chat_guard = state.chat.0.write().await;
        chat_guard.current = tool_msg_id;
    }

    let sysinfo_id = Uuid::new_v4();
    chat::add_msg_immediate_background(
        &state,
        &event_bus,
        sysinfo_id,
        "sysinfo".to_string(),
        MessageKind::SysInfo,
    )
    .await;

    {
        let chat_guard = state.chat.0.read().await;
        let tool_msg = chat_guard
            .messages
            .get(&tool_msg_id)
            .expect("tool message");
        assert_eq!(chat_guard.tail, child_msg_id);
        assert_eq!(tool_msg.selected_child, Some(child_msg_id));
    }

    let applied_payload = ToolUiPayload::new(ToolName::ApplyCodeEdit, call_id.clone(), "applied")
        .with_request_id(request_id)
        .with_field("status", "applied");
    assert!(!should_render_tool_buttons(&applied_payload));
    chat::update_tool_message_by_call_id(
        &state,
        &event_bus,
        &call_id,
        Some("applied content".to_string()),
        Some(applied_payload),
    )
    .await;

    let chat_guard = state.chat.0.read().await;
    let tool_msg = chat_guard
        .messages
        .get(&tool_msg_id)
        .expect("tool message");
    let payload = tool_msg.tool_payload.as_ref().expect("tool payload");
    assert_eq!(tool_msg.content, "applied content");
    assert!(
        payload
            .fields
            .iter()
            .any(|field| field.name.as_ref() == "status" && field.value.as_ref() == "applied")
    );
}
