// src/events.rs
use crossterm::event::{self, Event as CrosstermEvent, KeyEventKind};
use flume::Sender;
use futures::{FutureExt, StreamExt};
use tokio::time::{self, Duration};

use crate::app::AppEvent;

/// Spawns a Tokio task to poll for terminal events and send them to the `app_event_tx` channel.
pub async fn start_event_listener(app_event_tx: Sender<AppEvent>) -> color_eyre::Result<()> {
    let mut reader = event::EventStream::new();

    loop {
        tokio::select! {
            // Poll for Crossterm events
            maybe_event = reader.next().fuse() => {
                match maybe_event {
                    Some(Ok(CrosstermEvent::Key(key_event))) => {
                        // Only send key events on press (not repeat or release)
                        if key_event.kind == KeyEventKind::Press {
                            if app_event_tx.send(AppEvent::Key(key_event)).is_err() {
                                // Channel closed, app likely shutting down
                                break;
                            }
                        }
                    }
                    Some(Ok(CrosstermEvent::Resize(x, y))) => {
                        if app_event_tx.send(AppEvent::Resize(x, y)).is_err() {
                            break;
                        }
                    }
                    Some(Err(e)) => {
                        eprintln!("Error reading event: {:?}", e);
                        // Decide how to handle this error; perhaps send a Quit event
                        if app_event_tx.send(AppEvent::Quit).is_err() {
                            break;
                        }
                    }
                    None => {
                        // Event stream ended, should not happen for stdin
                        break;
                    }
                    _ => {} // Ignore mouse events, focus events for now
                }
            }
            // Add a small tick to prevent busy-looping if no events
            _ = time::sleep(Duration::from_millis(10)).fuse() => {}
        }
    }
    Ok(())
}
