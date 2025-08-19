use color_eyre::Result;
use std::sync::Arc;
use tracing::instrument;

use ploke_embed::indexer::{self, IndexStatus};
use tokio::sync::broadcast;

use crate::{AppEvent, error::ErrorSeverity};

#[derive(Clone, Copy, Debug)]
pub enum EventPriority {
    Realtime,
    Background,
}

#[derive(Debug, Clone)]
pub struct ErrorEvent {
    pub message: String,
    pub severity: ErrorSeverity,
}

#[derive(Debug)]
pub struct EventBus {
    pub realtime_tx: broadcast::Sender<AppEvent>,
    pub background_tx: broadcast::Sender<AppEvent>,
    error_tx: broadcast::Sender<ErrorEvent>,
    // NOTE: dedicated for indexing manager control
    pub index_tx: Arc<broadcast::Sender<indexer::IndexingStatus>>,
    // NOTE: Dedicated for context control
}

/// Convenience struct to help with the initialization of EventBus
#[derive(Clone, Copy)]
pub struct EventBusCaps {
    realtime_cap: usize,
    background_cap: usize,
    error_cap: usize,
    index_cap: usize,
}

impl Default for EventBusCaps {
    fn default() -> Self {
        Self {
            realtime_cap: 100,
            background_cap: 1000,
            error_cap: 1000,
            index_cap: 1000,
        }
    }
}

pub async fn run_event_bus(event_bus: Arc<EventBus>) -> Result<()> {
    use broadcast::error::RecvError;
    let mut index_rx = event_bus.index_subscriber();
    #[allow(unused_mut)]
    let mut bg_rx = event_bus.background_tx.subscribe();
    // more here?
    loop {
        tokio::select! {
        // bg_event = bg_rx.recv() => {
        // tracing::trace!("event bus received a background event: {:?}", bg_event);
        //     match bg_event {
        //         Ok(AppEvent::System(sys_event)) => match sys_event {
        //             SystemEvent::ModelSwitched(alias_or_id) => {
        //                 tracing::info!("event bus Sent RAG event with snippets: {:#?}", alias_or_id);
        //                 event_bus.send(AppEvent::System(SystemEvent::ModelSwitched(alias_or_id)));
        //             }
        //             SystemEvent::SaveRequested(vec_bytes) => {
        //                 tracing::info!("event bus Sent save event of Vec<u8> len = {}", vec_bytes.len());
        //                 event_bus.send(AppEvent::System(SystemEvent::SaveRequested(vec_bytes)));
        //         }
        //             _ => {}
        //         },
        //         Ok(_) => {}
        //         Err(e) => {
        //             match e {
        //                 RecvError::Closed => {
        //                     tracing::trace!("System Event event channel closed {}", e.to_string());
        //                     break;
        //                 }
        //                 RecvError::Lagged(lag) => {
        //                     tracing::trace!(
        //                         "System Event event channel lagging {} with {} messages",
        //                         e.to_string(),
        //                         lag,
        //                     )
        //                 }
        //             };
        //         }
        //     }
        // }
            index_event = index_rx.recv() => {
            // let index_event = index_rx.recv().await;
            tracing::trace!("event bus received IndexStatus");
            match index_event {
                Ok(status) => {
                    match status.status {
                        IndexStatus::Running => {
                            tracing::info!("event bus sending {:?}", status.status);
                            let result = event_bus
                                .realtime_tx
                                .send(AppEvent::IndexingProgress(status));
                            tracing::warn!("with result {:?}", result);
                            continue;
                        }
                        IndexStatus::Completed => {
                            let result = event_bus.realtime_tx.send(AppEvent::IndexingCompleted);
                            tracing::info!(
                                "event bus sending {:?} with result {:?}",
                                status.status,
                                result
                            );
                            continue;
                        }
                        IndexStatus::Cancelled => {
                            // WARN: Consider whether this should count as a failure or not
                            // when doing better error handling later.
                            let result = event_bus.realtime_tx.send(AppEvent::IndexingFailed);
                            tracing::warn!(
                                "event bus sending {:?} with result {:?}",
                                status.status,
                                result
                            );
                            continue;
                        }
                        _ => {}
                    }
                }
                Err(e) => match e {
                    RecvError::Closed => {
                        tracing::trace!("indexing task event channel closed {}", e.to_string());
                        // break;
                    }
                    RecvError::Lagged(lag) => {
                        tracing::trace!(
                            "indexing task event channel lagging {} with {} messages",
                            e.to_string(),
                            lag
                        )
                    }
                },
            }
            }
            // };
        };
    }
    // Ok(())
}
impl EventBus {
    pub fn new(b: EventBusCaps) -> Self {
        Self {
            realtime_tx: broadcast::channel(b.realtime_cap).0,
            background_tx: broadcast::channel(b.background_cap).0,
            error_tx: broadcast::channel(b.error_cap).0,
            index_tx: Arc::new(broadcast::channel(b.index_cap).0),
        }
    }

    #[instrument]
    pub fn send(&self, event: AppEvent) {
        let priority = event.priority();
        tracing::debug!("event_priority: {:?}", priority);
        let tx = match priority {
            EventPriority::Realtime => &self.realtime_tx,
            EventPriority::Background => &self.background_tx,
        };
        let _ = tx.send(event); // Ignore receiver count
    }

    pub fn send_error(&self, message: String, severity: ErrorSeverity) {
        let _ = self.error_tx.send(ErrorEvent { message, severity });
    }

    pub fn subscribe(&self, priority: EventPriority) -> broadcast::Receiver<AppEvent> {
        match priority {
            EventPriority::Realtime => self.realtime_tx.subscribe(),
            EventPriority::Background => self.background_tx.subscribe(),
        }
    }

    pub fn index_subscriber(&self) -> broadcast::Receiver<indexer::IndexingStatus> {
        self.index_tx.subscribe()
    }
}
