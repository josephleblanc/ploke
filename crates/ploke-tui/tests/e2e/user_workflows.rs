use ratatui::backend::TestBackend;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ploke_tui::app::{App, Mode, CommandStyle};
use ploke_tui::event::Event;
use ploke_tui::app_state::{StateCommand, IndexingStatus, IndexStatus};
use tokio::sync::mpsc;
use std::time::Duration;

#[tokio::test]
async fn user_starts_and_monitors_indexing() {
    let (cmd_tx, mut cmd_rx) = mpsc::channel(32);
    let (event_tx, _) = tokio::sync::broadcast::channel(100);
    
    // Initialize app
    let mut app = App::new(
        CommandStyle::Slash,
        Arc::new(AppState::default()),
        cmd_tx,
        &event_tx.into()
    );
    
    // Start indexing via command
    app.mode = Mode::Command;
    app.input_buffer = "/index start".into();
    app.handle_command_mode(KeyEvent::from(KeyCode::Enter));
    
    // Verify command was sent
    assert!(matches!(
        cmd_rx.recv().await,
        Some(StateCommand::IndexWorkspace)
    ));
    
    // Simulate progress
    event_tx.send(AppEvent::IndexingProgress(IndexingStatus {
        status: IndexStatus::Running,
        processed: 5,
        total: 100,
        current_file: None,
        errors: Vec::new(),
    })).unwrap();
    
    // Render
    let mut terminal = build_test_terminal();
    let frames = draw_terminal_with_delay(&mut terminal, &app, Duration::from_millis(50));
    
    // Verify progress appears
    assert!(frames[1].to_string().contains("Indexing: 5/100"));
}
