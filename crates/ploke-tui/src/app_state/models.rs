use super::*;

pub(super) async fn switch_model(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    alias_or_id: String,
) {
    tracing::debug!("inside StateCommand::SwitchModel {}", alias_or_id);

    use std::str::FromStr;
    match crate::llm2::ModelId::from_str(&alias_or_id) {
        Ok(mid) => {
            // Update runtime active model
            {
                let mut cfg = state.config.write().await;
                cfg.active_model = mid.clone();
                // Ensure there is a ModelPrefs entry so later commands can attach profiles/endpoints
                cfg.model_registry
                    .models
                    .entry(mid.key.clone())
                    .or_default();
            }
            event_bus.send(AppEvent::System(SystemEvent::ModelSwitched(
                mid,
            )));
        }
        Err(_) => {
            tracing::debug!("Sending AppEvent::Error(ErrorEvent {}", alias_or_id);
            event_bus.send(AppEvent::Error(ErrorEvent {
                message: format!("Unknown model '{}'", alias_or_id),
                severity: ErrorSeverity::Warning,
            }));
        }
    }
}
