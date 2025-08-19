#![cfg(feature = "watcher")]

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::collections::HashMap;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};
use tokio::sync::broadcast;
use uuid::Uuid;

/// Kinds of file events we surface to clients.
#[derive(Debug, Clone)]
pub enum FileEventKind {
    Created,
    Modified,
    Removed,
    Renamed,
    Other,
}

/// A normalized file change event.
#[derive(Debug, Clone)]
pub struct FileChangeEvent {
    pub path: PathBuf,
    pub kind: FileEventKind,
    /// For rename events, the previous path when available.
    pub old_path: Option<PathBuf>,
    /// Origin correlation for echo suppression (e.g., write operations).
    pub origin: Option<Uuid>,
}

/// Start a background watcher thread over the provided roots, broadcasting events with debouncing/coalescing.
///
/// Debouncing aggregates rapid sequences of events per path and emits the latest kind after
/// at least `debounce` has elapsed since the last observed event for that path.
pub fn start_watcher(
    roots: Vec<PathBuf>,
    debounce: Duration,
    events_tx: broadcast::Sender<FileChangeEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let config = Config::default().with_poll_interval(debounce);
        let tx_broadcast = events_tx.clone();

        // Channel to collect raw notify events from callback into our coalescer loop.
        let (notify_tx, notify_rx) = std::sync::mpsc::channel::<Result<Event, notify::Error>>();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                // Forward all results to the aggregator thread.
                if let Err(send_err) = notify_tx.send(res) {
                    tracing::warn!("ploke-io watcher: failed to forward notify event: {:?}", send_err);
                    // Best-effort surface as synthetic event
                    let _ = tx_broadcast.send(FileChangeEvent {
                        path: PathBuf::from("watcher-error:forward-failed"),
                        kind: FileEventKind::Other,
                        old_path: None,
                        origin: None,
                    });
                }
            },
            config,
        )
        .expect("Failed to create file watcher");

        for root in roots {
            if let Err(e) = watcher.watch(&root, RecursiveMode::Recursive) {
                let _ = events_tx.send(FileChangeEvent {
                    path: root.clone(),
                    kind: FileEventKind::Other,
                    old_path: None,
                    origin: None,
                });
                tracing::warn!("ploke-io watcher: failed to watch {}: {:?}", root.display(), e);
            }
        }

        // Debounce/coalesce loop
        let mut pending: HashMap<PathBuf, (FileEventKind, Option<PathBuf>, Instant)> = HashMap::new();

        loop {
            // Use recv_timeout to provide a heartbeat for flushing
            match notify_rx.recv_timeout(debounce) {
                Ok(Ok(event)) => {
                    // Map notify event into zero or more normalized events
                    for (path, kind, old_path) in map_notify_events(event) {
                        let now = Instant::now();
                        // Coalesce by path: keep the latest kind and reset the timer
                        pending.insert(path, (kind, old_path, now));
                    }
                }
                Ok(Err(e)) => {
                    tracing::warn!("ploke-io watcher error: {:?}", e);
                    let _ = events_tx.send(FileChangeEvent {
                        path: PathBuf::from(format!("watcher-error:{e}")),
                        kind: FileEventKind::Other,
                        old_path: None,
                        origin: None,
                    });
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // No new events within debounce; proceed to flush eligible items
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    // Underlying watcher dropped; exit thread
                    break;
                }
            }

            // Flush items whose last update is older than debounce
            let now = Instant::now();
            let mut to_flush = Vec::new();
            for (p, (_k, _old, t)) in pending.iter() {
                if now.duration_since(*t) >= debounce {
                    to_flush.push(p.clone());
                }
            }
            for p in to_flush {
                if let Some((kind, old_path, _)) = pending.remove(&p) {
                    let _ = events_tx.send(FileChangeEvent {
                        path: p,
                        kind,
                        old_path,
                        origin: None,
                    });
                }
            }

            // Park briefly to avoid busy loop when events are steady
            thread::park_timeout(Duration::from_millis(5));
        }
    })
}

fn map_notify_events(event: Event) -> Vec<(PathBuf, FileEventKind, Option<PathBuf>)> {
    use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
    match &event.kind {
        EventKind::Create(CreateKind::Any | CreateKind::File | CreateKind::Folder) => event
            .paths
            .into_iter()
            .map(|p| (p, FileEventKind::Created, None))
            .collect(),
        EventKind::Modify(ModifyKind::Any | ModifyKind::Data(_) | ModifyKind::Name(_)) => event
            .paths
            .into_iter()
            .map(|p| (p, FileEventKind::Modified, None))
            .collect(),
        EventKind::Remove(RemoveKind::Any | RemoveKind::File | RemoveKind::Folder) => event
            .paths
            .into_iter()
            .map(|p| (p, FileEventKind::Removed, None))
            .collect(),
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) => {
            if event.paths.len() >= 2 {
                let from = event.paths[0].clone();
                let to = event.paths[1].clone();
                vec![(to, FileEventKind::Renamed, Some(from))]
            } else {
                // Fallback when both paths not provided
                event
                    .paths
                    .into_iter()
                    .map(|p| (p, FileEventKind::Renamed, None))
                    .collect()
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::From)) => {
            if let Some(from) = event.paths.first() {
                vec![(from.clone(), FileEventKind::Renamed, None)]
            } else {
                vec![]
            }
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            if let Some(to) = event.paths.first() {
                vec![(to.clone(), FileEventKind::Renamed, None)]
            } else {
                vec![]
            }
        }
        _ => event
            .paths
            .into_iter()
            .map(|p| (p, FileEventKind::Other, None))
            .collect(),
    }
}

fn map_event_kind(kind: &EventKind) -> FileEventKind {
    use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
    match kind {
        EventKind::Create(CreateKind::Any | CreateKind::File | CreateKind::Folder) => {
            FileEventKind::Created
        }
        EventKind::Modify(ModifyKind::Any | ModifyKind::Data(_) | ModifyKind::Name(_)) => {
            FileEventKind::Modified
        }
        EventKind::Remove(RemoveKind::Any | RemoveKind::File | RemoveKind::Folder) => {
            FileEventKind::Removed
        }
        EventKind::Modify(ModifyKind::Name(RenameMode::Both))
        | EventKind::Modify(ModifyKind::Name(RenameMode::From))
        | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => FileEventKind::Renamed,
        _ => FileEventKind::Other,
    }
}
