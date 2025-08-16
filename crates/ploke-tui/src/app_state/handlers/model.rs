use std::sync::Arc;

use crate::{app_state::{models, AppState}, EventBus};

pub async fn switch_model(state: &Arc<AppState>, event_bus: &Arc<EventBus>, alias_or_id: String) {
    models::switch_model(state, event_bus, alias_or_id).await;
}
