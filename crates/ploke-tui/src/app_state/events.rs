use uuid::Uuid;

#[derive(Debug, Clone, Copy)]
pub struct MessageUpdatedEvent(pub Uuid);

impl MessageUpdatedEvent {
    pub fn new(message_id: Uuid) -> Self {
        Self(message_id)
    }
}

impl From<MessageUpdatedEvent> for crate::AppEvent {
    fn from(event: MessageUpdatedEvent) -> Self {
        crate::AppEvent::MessageUpdated(event)
    }
}
