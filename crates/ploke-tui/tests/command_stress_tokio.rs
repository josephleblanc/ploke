//! Tokio multi-thread stress test for command dispatch concurrency.
//!
//! Covered (exactly):
//! - Real `state_manager` async actor loop.
//! - `StateCommand` variants sent by this test:
//!   - `AddMessageImmediate` with `MessageKind::SysInfo`
//!   - `AddUserMessage` (completion oneshot is unused)
//!   - `UpdateMessage` (content set, status set to Generating/Completed, metadata None)
//!   - `DeleteMessage`
//!   - `NavigateList`
//!   - `SetEditingPreviewMode`
//!   - `SetEditingMaxPreviewLines`
//!   - `SetEditingAutoConfirm`
//! - Concurrency: 8 workers, each sends 200 commands (total 1600 commands).
//! - RNG: fixed seeds per worker (`SEED_BASE + worker_index`) for reproducibility.
//! - Interleavings: tokio multi-thread runtime + explicit `yield_now()` for ~10% of sends.
//! - Assertions:
//!   - Test completes without panic/deadlock.
//!   - `chat.current` and `chat.tail` both exist in `chat.messages` at end.
//!
//! Not covered:
//! - Command parsing from user input strings (parser is bypassed).
//! - UI rendering, overlay behavior, or terminal IO.
//! - External services (LLM, embeddings, IO manager, filesystem).
//! - Commands not listed above (e.g., indexing, RAG, DB, model/provider selection).
//! - Strong state invariants beyond message ID existence (e.g., path_cache correctness).
//! - True exhaustive scheduling (this is probabilistic concurrency).
use std::sync::Arc;

use ploke_tui::EventBusCaps;
use ploke_tui::app_state::commands::{ListNavigation, StateCommand};
use ploke_tui::app_state::core::PreviewMode;
use ploke_tui::app_state::state_manager;
use ploke_tui::chat_history::{MessageKind, MessageStatus, MessageUpdate};
use ploke_tui::test_utils::mock;
use ploke_tui::{EventBus, RagEvent};
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};
use tokio::sync::{Barrier, Mutex, mpsc, oneshot};
use tokio::time::{Duration, sleep};
use uuid::Uuid;

const WORKERS: usize = 8;
const CMDS_PER_WORKER: usize = 200;
const SEED_BASE: u64 = 0xC0FFEE;

fn random_uuid(rng: &mut StdRng) -> Uuid {
    let bytes: [u8; 16] = rng.random();
    Uuid::from_bytes(bytes)
}

fn random_content(rng: &mut StdRng, prefix: &str) -> String {
    format!("{}-{}", prefix, rng.random::<u64>())
}

async fn pick_known_id(ids: &Mutex<Vec<Uuid>>, rng: &mut StdRng) -> Uuid {
    let guard = ids.lock().await;
    if guard.is_empty() {
        random_uuid(rng)
    } else {
        guard[rng.random_range(0..guard.len())]
    }
}

async fn build_command(
    rng: &mut StdRng,
    ids: &Mutex<Vec<Uuid>>,
) -> StateCommand {
    match rng.random_range(0..8) {
        0 => {
            let id = random_uuid(rng);
            {
                let mut guard = ids.lock().await;
                guard.push(id);
            }
            StateCommand::AddMessageImmediate {
                msg: random_content(rng, "sys"),
                kind: MessageKind::SysInfo,
                new_msg_id: id,
            }
        }
        1 => {
            let id = random_uuid(rng);
            {
                let mut guard = ids.lock().await;
                guard.push(id);
            }
            let (tx, _rx) = oneshot::channel();
            StateCommand::AddUserMessage {
                content: random_content(rng, "user"),
                new_user_msg_id: id,
                completion_tx: tx,
            }
        }
        2 => {
            let id = pick_known_id(ids, rng).await;
            let update = MessageUpdate {
                content: Some(random_content(rng, "edit")),
                append_content: None,
                status: Some(if rng.random_bool(0.5) {
                    MessageStatus::Generating
                } else {
                    MessageStatus::Completed
                }),
                metadata: None,
            };
            StateCommand::UpdateMessage { id, update }
        }
        3 => {
            let id = pick_known_id(ids, rng).await;
            StateCommand::DeleteMessage { id }
        }
        4 => {
            let direction = match rng.random_range(0..4) {
                0 => ListNavigation::Up,
                1 => ListNavigation::Down,
                2 => ListNavigation::Top,
                _ => ListNavigation::Bottom,
            };
            StateCommand::NavigateList { direction }
        }
        5 => {
            let mode = if rng.random_bool(0.5) {
                PreviewMode::CodeBlock
            } else {
                PreviewMode::Diff
            };
            StateCommand::SetEditingPreviewMode { mode }
        }
        6 => StateCommand::SetEditingMaxPreviewLines {
            lines: rng.random_range(1..600),
        },
        _ => StateCommand::SetEditingAutoConfirm {
            enabled: rng.random_bool(0.5),
        },
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn stress_commands_tokio() {
    let state = Arc::new(mock::create_mock_app_state());
    let (cmd_tx, cmd_rx) = mpsc::channel(256);
    let event_bus = Arc::new(EventBus::new(EventBusCaps::default()));
    let (rag_tx, _rag_rx) = mpsc::channel::<RagEvent>(32);

    tokio::spawn(state_manager(
        state.clone(),
        cmd_rx,
        event_bus,
        rag_tx,
    ));

    let ids = Arc::new(Mutex::new(Vec::new()));
    let barrier = Arc::new(Barrier::new(WORKERS + 1));
    let mut handles = Vec::with_capacity(WORKERS);

    for worker in 0..WORKERS {
        let cmd_tx = cmd_tx.clone();
        let ids = ids.clone();
        let barrier = barrier.clone();
        handles.push(tokio::spawn(async move {
            let mut rng = StdRng::seed_from_u64(SEED_BASE + worker as u64);
            barrier.wait().await;
            for _ in 0..CMDS_PER_WORKER {
                let cmd = build_command(&mut rng, &ids).await;
                if cmd_tx.send(cmd).await.is_err() {
                    break;
                }
                if rng.random_bool(0.1) {
                    tokio::task::yield_now().await;
                }
            }
        }));
    }

    barrier.wait().await;
    for handle in handles {
        let _ = handle.await;
    }

    drop(cmd_tx);
    sleep(Duration::from_millis(200)).await;

    let chat = state.chat.0.read().await;
    assert!(chat.messages.contains_key(&chat.current));
    assert!(chat.messages.contains_key(&chat.tail));
}
