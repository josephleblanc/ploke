use std::sync::Arc;

use super::super::core::AppState;

pub async fn save_state(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    let serialized_content = {
        let guard = state.chat.0.read().await;
        guard.format_for_persistence().as_bytes().to_vec()
    };
    event_bus.send(AppEvent::System(crate::system::SystemEvent::SaveRequested(
        serialized_content,
    )))
}
