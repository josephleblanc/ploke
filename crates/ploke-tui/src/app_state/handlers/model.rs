use std::sync::Arc;

use crate::{
    EventBus,
    app_state::{AppState, models},
};

pub async fn switch_model(state: &Arc<AppState>, event_bus: &Arc<EventBus>, alias_or_id: String) {
    models::switch_model(state, event_bus, alias_or_id).await;
}
