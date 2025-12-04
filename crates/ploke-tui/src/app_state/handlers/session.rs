use std::sync::Arc;

use crate::{
    AppEvent, EventBus,
    app_state::{AppState, events::SystemEvent},
};

pub async fn save_state(state: &Arc<AppState>, event_bus: &Arc<EventBus>) {
    let serialized_content = {
        let guard = state.chat.0.read().await;
        guard.format_for_persistence().as_bytes().to_vec()
    };
    event_bus.send(AppEvent::System(SystemEvent::SaveRequested(
        serialized_content,
    )))
}
