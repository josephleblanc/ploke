use std::ops::Index;
use std::sync::Arc;

use cozo::DataValue;
use ploke_db::QueryResult;
use ploke_embed::local::EmbeddingConfig;
use syn_parser::parser::nodes::ToCozoUuid;

use crate::tracing_setup::init_tracing;

use super::*;
use ploke_embed::{
    indexer::{EmbeddingProcessor, EmbeddingSource},
    local::LocalEmbedder,
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

impl MockTrait for crate::ploke_io::IoManagerHandle {
    fn mock() -> Self {
        crate::ploke_io::IoManagerHandle::new()
    }
}

#[tokio::test]
async fn test_race_condition_without_oneshot() {
    let db = ploke_db::Database::new_init().unwrap();
    let state = Arc::new(AppState::new(
        Arc::new(db),
        Arc::new(EmbeddingProcessor::mock()),
        crate::ploke_io::IoManagerHandle::mock(),
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
                new_msg_id: user_msg_id,
                completion_tx: oneshot::channel().0, // dummy
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
    let db = ploke_db::Database::new_init().unwrap();
    let state = Arc::new(AppState::new(
        Arc::new(db),
        Arc::new(EmbeddingProcessor::mock()),
        crate::ploke_io::IoManagerHandle::mock(),
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
            new_msg_id: user_msg_id,
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
async fn test_concurrency_with_fuzzing() {
    let db = ploke_db::Database::new_init().unwrap();
    let state = Arc::new(AppState::new(
        Arc::new(db),
        Arc::new(EmbeddingProcessor::mock()),
        crate::ploke_io::IoManagerHandle::mock(),
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
                new_msg_id: user_msg_id,
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
