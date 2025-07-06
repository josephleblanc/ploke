use std::sync::Arc;
use std::time::Duration;

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ploke_embed::indexer::{IndexStatus, IndexingStatus};
use ploke_tui::app::{App, Mode};
use ploke_tui::app_state::{AppState, StateCommand};
use ploke_tui::user_config::CommandStyle;
use ratatui::Terminal;
use ratatui::backend::TestBackend;
use tokio::sync::{broadcast, mpsc};
use uuid::Uuid;

// Helper function to create a test terminal
fn build_test_terminal() -> Terminal<TestBackend> {
    let backend = TestBackend::new(100, 30);
    Terminal::new(backend).unwrap()
}

// Helper function to render terminal with delay
fn draw_terminal_with_delay(
    terminal: &mut Terminal<TestBackend>,
    app: &App,
    _delay: Duration,
) -> Vec<String> {
    terminal
        .draw(|f| {
            // Use dummy data for rendering
            app.draw(f, &[], Uuid::new_v4());
        })
        .unwrap();
    // Extract rendered content
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|c| c.symbol.clone())
        .collect()
}

#[tokio::test]
async fn user_starts_and_monitors_indexing() {
    let (cmd_tx, mut cmd_rx) = mpsc::channel(32);
    let event_bus = Arc::new(EventBus::new(EventBusCaps {
        realtime_cap: 100,
        background_cap: 100,
        error_cap: 100,
        index_cap: 100,
    }));

    // Initialize app
    let mut app = App::new(
        CommandStyle::Slash,
        Arc::new(AppState::default()),
        cmd_tx,
        &event_bus,
    );

    // Start indexing via command
    app.mode = Mode::Command;
    app.input_buffer = "/index start".into();
    app.handle_command_mode(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));

    // Verify command was sent
    assert!(matches!(
        cmd_rx.recv().await,
        Some(StateCommand::IndexWorkspace)
    ));

    // Simulate progress
    event_bus.send(AppEvent::IndexingProgress(IndexingStatus {
        status: IndexStatus::Running,
        processed: 5,
        total: 100,
        current_file: None,
        errors: Vec::new(),
    }));

    // Render
    let mut terminal = build_test_terminal();
    let frames = draw_terminal_with_delay(&mut terminal, &app, Duration::from_millis(50));

    // Verify progress appears
    let frame_string = frames.join("");
    assert!(frame_string.contains("Indexing"));
    assert!(frame_string.contains("5/100"));
}
