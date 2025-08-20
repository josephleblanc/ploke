use color_eyre::Result;
use std::sync::Arc;
use std::time::{Duration, Instant};
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
    let mut started_sent = false;
    let mut last_lag_warn: Option<Instant> = None;
    // Signal readiness to subscribers/tests
    let _ = event_bus.realtime_tx.send(AppEvent::EventBusStarted);
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
                            if !started_sent {
                                let _ = event_bus.realtime_tx.send(AppEvent::IndexingStarted);
                                started_sent = true;
                            }
                            let _ = event_bus
                                .realtime_tx
                                .send(AppEvent::IndexingProgress(status));
                            continue;
                        }
                        IndexStatus::Completed => {
                            let result = event_bus.realtime_tx.send(AppEvent::IndexingCompleted);
                            tracing::info!(
                                "event bus sending {:?} with result {:?}",
                                status.status,
                                result
                            );
                            // reset for next run
                            started_sent = false;
                            continue;
                        }
                        IndexStatus::Cancelled => {
                            // Treat as failure-equivalent for UI in M0
                            let result = event_bus.realtime_tx.send(AppEvent::IndexingFailed);
                            tracing::warn!(
                                "event bus sending {:?} with result {:?}",
                                status.status,
                                result
                            );
                            // reset for next run
                            started_sent = false;
                            continue;
                        }
                        IndexStatus::Failed(err) => {
                            let _ = event_bus.realtime_tx.send(AppEvent::Error(ErrorEvent {
                                message: format!("Indexing failed: {}", err),
                                severity: ErrorSeverity::Error,
                            }));
                            let _ = event_bus.realtime_tx.send(AppEvent::IndexingFailed);
                            // reset for next run
                            started_sent = false;
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
                        let now = Instant::now();
                        let should_emit = last_lag_warn
                            .map(|prev| now.duration_since(prev) >= Duration::from_secs(1))
                            .unwrap_or(true);
                        let msg = format!(
                            "indexing task event channel lagging with {} messages",
                            lag
                        );
                        if should_emit {
                            tracing::warn!("{}", msg);
                            let _ = event_bus
                                .realtime_tx
                                .send(AppEvent::Error(ErrorEvent {
                                    message: msg,
                                    severity: ErrorSeverity::Warning,
                                }));
                            last_lag_warn = Some(now);
                        } else {
                            tracing::debug!("{}", msg);
                        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;
    use tokio::time::{timeout, Duration};

    #[tokio::test]
    async fn ssot_forwards_indexing_completed_once() {
        let bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let mut rx = bus.realtime_tx.subscribe();
        let bus_clone = Arc::clone(&bus);
        tokio::spawn(async move {
            let _ = run_event_bus(bus_clone).await;
        });

        // Wait for event bus readiness signal instead of sleeping
        let _ = timeout(Duration::from_secs(1), async {
            loop {
                match rx.recv().await {
                    Ok(AppEvent::EventBusStarted) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        })
        .await
        .expect("timeout waiting for EventBusStarted");

        // Inject a single Completed status into the indexing channel
        let _ = bus.index_tx.send(indexer::IndexingStatus {
            status: IndexStatus::Completed,
            recent_processed: 0,
            num_not_proc: 0,
            current_file: None,
            errors: vec![],
        });

        // Expect exactly one IndexingCompleted event
        let ev = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for event")
            .expect("realtime channel closed");

        match ev {
            AppEvent::IndexingCompleted => {}
            other => panic!("expected IndexingCompleted, got {:?}", other),
        }

        // No additional IndexingCompleted should be received immediately after
        assert!(rx.try_recv().is_err());
    }

    #[tokio::test]
    async fn ssot_forwards_indexing_failed_once() {
        let bus = Arc::new(EventBus::new(EventBusCaps::default()));
        let mut rx = bus.realtime_tx.subscribe();
        let bus_clone = Arc::clone(&bus);
        tokio::spawn(async move {
            let _ = run_event_bus(bus_clone).await;
        });

        // Wait for event bus readiness signal instead of sleeping
        let _ = timeout(Duration::from_secs(1), async {
            loop {
                match rx.recv().await {
                    Ok(AppEvent::EventBusStarted) => break,
                    Ok(_) => continue,
                    Err(_) => break,
                }
            }
        })
        .await
        .expect("timeout waiting for EventBusStarted");

        // Inject a single Failed status into the indexing channel
        let _ = bus.index_tx.send(indexer::IndexingStatus {
            status: IndexStatus::Failed("boom".to_string()),
            recent_processed: 0,
            num_not_proc: 0,
            current_file: None,
            errors: vec![],
        });

        // Expect exactly one IndexingFailed event
        let ev = timeout(Duration::from_secs(1), rx.recv())
            .await
            .expect("timeout waiting for event")
            .expect("realtime channel closed");

        match ev {
            AppEvent::IndexingFailed => {}
            other => panic!("expected IndexingFailed, got {:?}", other),
        }

        // No additional IndexingFailed should be received immediately after
        assert!(rx.try_recv().is_err());
    }
}
