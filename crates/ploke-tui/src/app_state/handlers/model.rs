use std::sync::Arc;

use super::super::core::AppState;

pub async fn switch_model(state: &Arc<AppState>, event_bus: &Arc<EventBus>, alias_or_id: String) {
    super::super::models::switch_model(state, event_bus, alias_or_id).await;
}
