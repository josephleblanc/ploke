use ploke_tui::app_state::*;
use ploke_tui::app_event::AppEvent;
use tokio::sync::broadcast;

#[tokio::test]
async fn indexing_lifecycle() {
    // Setup
    let state = Arc::new(AppState::default());
    let (cmd_tx, mut cmd_rx) = mpsc::channel(32);
    let (event_tx, _) = broadcast::channel(100);
    let state_clone = state.clone();
    
    // Start state manager
    tokio::spawn(state_manager(state_clone, cmd_rx, Arc::new(event_tx.into())));
    
    // Start indexing
    cmd_tx.send(StateCommand::IndexWorkspace).await.unwrap();
    
    // Verify RUNNING state
    let guard = state.indexing_state.read().await;
    assert!(guard.as_ref().unwrap().status == IndexStatus::Running);
    
    // ... similar checks for pause/resume/cancel
}
