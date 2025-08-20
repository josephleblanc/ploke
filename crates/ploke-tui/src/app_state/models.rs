use super::*;

pub(super) async fn switch_model(
    state: &Arc<AppState>,
    event_bus: &Arc<EventBus>,
    alias_or_id: String,
) {
    tracing::debug!("inside StateCommand::SwitchModel {}", alias_or_id);

    let mut cfg = state.config.write().await;
    if cfg.provider_registry.set_active(&alias_or_id) {
        tracing::debug!(
            "sending AppEvent::System(SystemEvent::ModelSwitched {}
                        Trying to find cfg.provider_registry.get_active_provider(): {:#?}",
            alias_or_id,
            cfg.provider_registry.get_active_provider(),
        );
        let actual_model = cfg
            .provider_registry
            .get_active_provider()
            .map(|p| p.model.clone())
            .unwrap_or_else(|| alias_or_id.clone());
        event_bus.send(AppEvent::System(SystemEvent::ModelSwitched(
            actual_model, // Using actual model ID
        )));
    } else {
        tracing::debug!("Sending AppEvent::Error(ErrorEvent {}", alias_or_id);
        event_bus.send(AppEvent::Error(ErrorEvent {
            message: format!("Unknown model '{}'", alias_or_id),
            severity: ErrorSeverity::Warning,
        }));
    }
}
