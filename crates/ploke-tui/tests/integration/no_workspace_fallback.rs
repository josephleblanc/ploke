use std::{path::PathBuf, sync::Arc};

use tokio::sync::{mpsc, oneshot};
use tokio::time::{Duration, timeout};
use uuid::Uuid;

use ploke_tui as tui;
use tui::app_state::{AppState, StateCommand};
use tui::event_bus::{EventBus, EventBusCaps, EventPriority};

use ploke_db::Database;
use ploke_embed::indexer::EmbeddingProcessor;
use ploke_embed::{
    indexer::EmbeddingSource,
    local::{EmbeddingConfig, LocalEmbedder},
    runtime::EmbeddingRuntime,
};
use ploke_io::IoManagerHandle;
use ploke_rag::{RagService, TokenBudget};

fn mock_embedder() -> EmbeddingProcessor {
    EmbeddingProcessor::new(EmbeddingSource::Local(
        LocalEmbedder::new(EmbeddingConfig::default())
            .expect("LocalEmbedder should construct in test"),
    ))
}

#[tokio::test]
async fn conversation_only_prompt_and_persistent_tip_without_workspace() {
    // App state with no focused crate and a valid (empty) DB + RAG service
    let db = Arc::new(Database::new_init().expect("init db"));
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        mock_embedder(),
    ));
    let rag =
        Arc::new(RagService::new(Arc::clone(&db), Arc::clone(&embedder)).expect("rag service"));
    let io_handle = IoManagerHandle::new();
    let (rag_tx, _rag_rx) = mpsc::channel(8);
    let state = Arc::new(AppState::new(
        Arc::clone(&db),
        Arc::clone(&embedder),
        io_handle,
        Arc::clone(&rag),
        TokenBudget::default(),
        rag_tx,
    ));

    // Wire state manager with an EventBus
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(128);
    let (ctx_tx, _ctx_rx) = mpsc::channel(8);
    {
        let s = Arc::clone(&state);
        let eb = Arc::clone(&event_bus);
        tokio::spawn(ploke_tui::app_state::state_manager(s, cmd_rx, eb, ctx_tx));
    }

    // Subscribe to realtime events to observe PromptConstructed
    let mut bg_rx = event_bus.subscribe(EventPriority::Background);

    // 1) Submit a user message without any workspace loaded
    let user_msg_id_1 = Uuid::new_v4();
    let (completion_tx, completion_rx) = oneshot::channel();
    let (scan_tx, scan_rx) = oneshot::channel();
    cmd_tx
        .send(StateCommand::AddUserMessage {
            content: "What's the status?".to_string(),
            new_user_msg_id: user_msg_id_1,
            completion_tx,
        })
        .await
        .unwrap();
    cmd_tx
        .send(StateCommand::ScanForChange { scan_tx })
        .await
        .unwrap();
    cmd_tx
        .send(StateCommand::EmbedMessage {
            new_msg_id: user_msg_id_1,
            completion_rx,
            scan_rx,
        })
        .await
        .unwrap();

    // Expect a PromptConstructed event (conversation-only fallback) for this parent_id
    let evt = timeout(Duration::from_secs(1), async {
        loop {
            match bg_rx.recv().await {
                Ok(e) => {
                    let dbg = format!("{:?}", e);
                    if dbg.contains("PromptConstructed") && dbg.contains(&user_msg_id_1.to_string()) {
                        assert!(dbg.contains("No workspace context loaded;"),
                            "expected conversation-only system note in PromptConstructed payload. got: {}",
                            dbg);
                        break;
                    }
                }
                Err(_) => panic!("event channel closed"),
            }
        }
    })
    .await;
    assert!(evt.is_ok(), "timed out waiting for PromptConstructed");

    // Guidance message should be present and should NOT have stolen focus
    {
        let chat = state.chat.0.read().await;
        let tip_count = chat
            .messages
            .values()
            .filter(|m| m.kind == tui::chat_history::MessageKind::SysInfo)
            .filter(|m| m.content.starts_with("No workspace is selected."))
            .count();
        assert_eq!(
            tip_count, 1,
            "guidance tip should appear exactly once so far"
        );
        assert_eq!(
            chat.current, user_msg_id_1,
            "current selection should remain on the user message, not the tip"
        );
    }

    // 2) Submit a second user message; the guidance tip should not repeat
    let user_msg_id_2 = Uuid::new_v4();
    let (completion_tx2, completion_rx2) = oneshot::channel();
    let (scan_tx2, scan_rx2) = oneshot::channel();
    cmd_tx
        .send(StateCommand::AddUserMessage {
            content: "And another one.".to_string(),
            new_user_msg_id: user_msg_id_2,
            completion_tx: completion_tx2,
        })
        .await
        .unwrap();
    cmd_tx
        .send(StateCommand::ScanForChange { scan_tx: scan_tx2 })
        .await
        .unwrap();
    cmd_tx
        .send(StateCommand::EmbedMessage {
            new_msg_id: user_msg_id_2,
            completion_rx: completion_rx2,
            scan_rx: scan_rx2,
        })
        .await
        .unwrap();

    // We should again see a PromptConstructed for the second parent_id
    let evt2 = timeout(Duration::from_secs(1), async {
        loop {
            match bg_rx.recv().await {
                Ok(e) => {
                    let dbg = format!("{:?}", e);
                    if dbg.contains("PromptConstructed") && dbg.contains(&user_msg_id_2.to_string())
                    {
                        break;
                    }
                }
                Err(_) => panic!("event channel closed"),
            }
        }
    })
    .await;
    assert!(
        evt2.is_ok(),
        "timed out waiting for second PromptConstructed"
    );

    // Tip should still be exactly once; current should be the second user message
    let chat = state.chat.0.read().await;
    let tip_count_total = chat
        .messages
        .values()
        .filter(|m| m.kind == tui::chat_history::MessageKind::SysInfo)
        .filter(|m| m.content.starts_with("No workspace is selected."))
        .count();
    assert_eq!(
        tip_count_total, 1,
        "guidance tip should be emitted only once per session"
    );
    assert_eq!(
        chat.current, user_msg_id_2,
        "current selection should remain on the latest user message"
    );
}

#[tokio::test]
async fn conversation_only_prompt_names_loaded_workspace_when_context_off() {
    let db = Arc::new(Database::new_init().expect("init db"));
    let embedder = Arc::new(EmbeddingRuntime::from_shared_set(
        Arc::clone(&db.active_embedding_set),
        mock_embedder(),
    ));
    let rag =
        Arc::new(RagService::new(Arc::clone(&db), Arc::clone(&embedder)).expect("rag service"));
    let io_handle = IoManagerHandle::new();
    let (rag_tx, _rag_rx) = mpsc::channel(8);
    let state = Arc::new(AppState::new(
        Arc::clone(&db),
        Arc::clone(&embedder),
        io_handle,
        Arc::clone(&rag),
        TokenBudget::default(),
        rag_tx,
    ));
    let workspace_root = PathBuf::from("/tmp/ploke_loaded_workspace_for_context_notice");
    let member_root = workspace_root.join("crates/member");
    state
        .with_system_txn(|txn| {
            txn.set_loaded_workspace(
                workspace_root.clone(),
                vec![member_root.clone()],
                Some(member_root),
            );
        })
        .await;
    {
        let mut cfg = state.config.write().await;
        cfg.context_management.mode = tui::user_config::CtxMode::Off;
    }

    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let (cmd_tx, cmd_rx) = mpsc::channel::<StateCommand>(128);
    let (ctx_tx, _ctx_rx) = mpsc::channel(8);
    {
        let s = Arc::clone(&state);
        let eb = Arc::clone(&event_bus);
        tokio::spawn(ploke_tui::app_state::state_manager(s, cmd_rx, eb, ctx_tx));
    }
    let mut bg_rx = event_bus.subscribe(EventPriority::Background);

    let user_msg_id = Uuid::new_v4();
    let (completion_tx, completion_rx) = oneshot::channel();
    let (scan_tx, scan_rx) = oneshot::channel();
    cmd_tx
        .send(StateCommand::AddUserMessage {
            content: "Check loaded workspace notice.".to_string(),
            new_user_msg_id: user_msg_id,
            completion_tx,
        })
        .await
        .unwrap();
    cmd_tx
        .send(StateCommand::ScanForChange { scan_tx })
        .await
        .unwrap();
    cmd_tx
        .send(StateCommand::EmbedMessage {
            new_msg_id: user_msg_id,
            completion_rx,
            scan_rx,
        })
        .await
        .unwrap();

    let evt = timeout(Duration::from_secs(1), async {
        loop {
            match bg_rx.recv().await {
                Ok(e) => {
                    let dbg = format!("{:?}", e);
                    if dbg.contains("PromptConstructed") && dbg.contains(&user_msg_id.to_string())
                    {
                        assert!(
                            dbg.contains("Context mode is Off; workspace loaded at "),
                            "expected loaded-workspace system note in PromptConstructed payload. got: {}",
                            dbg
                        );
                        assert!(
                            dbg.contains(workspace_root.to_string_lossy().as_ref()),
                            "expected workspace root in PromptConstructed payload. got: {}",
                            dbg
                        );
                        assert!(
                            !dbg.contains("No workspace context loaded"),
                            "loaded workspace should not be described as missing. got: {}",
                            dbg
                        );
                        break;
                    }
                }
                Err(_) => panic!("event channel closed"),
            }
        }
    })
    .await;
    assert!(evt.is_ok(), "timed out waiting for PromptConstructed");

    let chat = state.chat.0.read().await;
    let tip_count = chat
        .messages
        .values()
        .filter(|m| m.kind == tui::chat_history::MessageKind::SysInfo)
        .filter(|m| m.content.starts_with("No workspace is selected."))
        .count();
    assert_eq!(
        tip_count, 0,
        "no-workspace tip must not appear when a workspace is loaded"
    );
}
