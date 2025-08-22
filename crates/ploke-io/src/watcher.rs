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
fn event_rank(k: &FileEventKind) -> u8 {
    match k {
        FileEventKind::Removed => 4,
        FileEventKind::Renamed => 3,
        FileEventKind::Created => 2,
        FileEventKind::Modified => 1,
        FileEventKind::Other => 0,
    }
}

pub fn start_watcher(
    roots: Vec<PathBuf>,
    debounce: Duration,
    events_tx: broadcast::Sender<FileChangeEvent>,
) -> thread::JoinHandle<()> {
    // Synchronize startup so callers don't miss very-early events (like Create)
    // due to watcher registration races.
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<()>();

    let jh = thread::spawn(move || {
        let config = Config::default().with_poll_interval(debounce);
        let tx_broadcast = events_tx.clone();

        // Channel to collect raw notify events from callback into our coalescer loop.
        let (notify_tx, notify_rx) = std::sync::mpsc::channel::<Result<Event, notify::Error>>();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| {
                // Forward all results to the aggregator thread.
                if let Err(send_err) = notify_tx.send(res) {
                    tracing::warn!(
                        "ploke-io watcher: failed to forward notify event: {:?}",
                        send_err
                    );
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
                tracing::warn!(
                    "ploke-io watcher: failed to watch {}: {:?}",
                    root.display(),
                    e
                );
            }
        }

        // Signal to the caller that watcher registration is complete.
        let _ = ready_tx.send(());

        // Debounce/coalesce loop
        let mut pending: HashMap<PathBuf, (FileEventKind, Option<PathBuf>, Instant)> =
            HashMap::new();

        loop {
            // Use recv_timeout to provide a heartbeat for flushing
            match notify_rx.recv_timeout(debounce) {
                Ok(Ok(event)) => {
                    // Map notify event into zero or more normalized events
                    for (path, kind, old_path) in map_notify_events(event) {
                        let now = Instant::now();
                        // Coalesce by path with precedence so Created isn't downgraded by Modified, etc.
                        // Precedence (high → low): Removed > Renamed > Created > Modified > Other
                        use std::collections::hash_map::Entry;
                        match pending.entry(path) {
                            Entry::Occupied(mut occ) => {
                                let (existing_kind, existing_old_path, last_update) = occ.get_mut();
                                let existing_rank = event_rank(existing_kind);
                                let new_rank = event_rank(&kind);
                                if new_rank > existing_rank {
                                    *existing_kind = kind;
                                    *existing_old_path = old_path.clone();
                                } else if matches!(existing_kind, FileEventKind::Renamed)
                                    && existing_old_path.is_none()
                                    && old_path.is_some()
                                {
                                    // Preserve rename "from" path if it arrives later
                                    *existing_old_path = old_path.clone();
                                }
                                // Always extend debounce window on new activity
                                *last_update = now;
                            }
                            Entry::Vacant(vac) => {
                                vac.insert((kind, old_path, now));
                            }
                        }
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
    });
    // Block until the watcher thread has registered all roots to avoid missing early events.
    let _ = ready_rx.recv();
    jh
}

fn map_notify_events(event: Event) -> Vec<(PathBuf, FileEventKind, Option<PathBuf>)> {
    use notify::event::{CreateKind, ModifyKind, RemoveKind, RenameMode};
    match &event.kind {
        EventKind::Create(CreateKind::Any | CreateKind::File | CreateKind::Folder) => event
            .paths
            .into_iter()
            .map(|p| (p, FileEventKind::Created, None))
            .collect(),
        EventKind::Modify(ModifyKind::Data(_)) => event
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
        EventKind::Modify(ModifyKind::Any) => event
            .paths
            .into_iter()
            .map(|p| (p, FileEventKind::Modified, None))
            .collect(),
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
        // Handle rename variants first so they are not shadowed by broader Modify matches.
        EventKind::Modify(ModifyKind::Name(RenameMode::Both))
        | EventKind::Modify(ModifyKind::Name(RenameMode::From))
        | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => FileEventKind::Renamed,
        // Specific Modify kinds
        EventKind::Modify(ModifyKind::Data(_)) => FileEventKind::Modified,
        EventKind::Modify(ModifyKind::Name(_)) => FileEventKind::Modified,
        EventKind::Modify(ModifyKind::Any) => FileEventKind::Modified,
        EventKind::Remove(RemoveKind::Any | RemoveKind::File | RemoveKind::Folder) => {
            FileEventKind::Removed
        }
        _ => FileEventKind::Other,
    }
}

#[cfg(all(test, feature = "watcher"))]
mod tests {
    use super::*;
    use std::time::Duration as StdDuration;
    use tempfile::tempdir;
    use tokio::sync::broadcast;
    use tokio::time::{timeout, Duration};

    async fn recv_kind_within(
        rx: &mut broadcast::Receiver<FileChangeEvent>,
        expected_kind: FileEventKind,
        expected_path: Option<&PathBuf>,
        max_wait: Duration,
    ) -> bool {
        let deadline = tokio::time::Instant::now() + max_wait;
        loop {
            let remaining = deadline.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                return false;
            }
            match timeout(remaining, rx.recv()).await {
                Ok(Ok(evt)) => {
                    let kind_matches =
                        std::mem::discriminant(&evt.kind) == std::mem::discriminant(&expected_kind);
                    // AI: This map_or could be simpliefied to a map_or_else
                    // Add this to your mistakes, and process all the instructions I have given in
                    // the `AI:` comments so far AI!
                    let path_matches = expected_path.map_or(true, |p| &evt.path == p);
                    if kind_matches && path_matches {
                        return true;
                    }
                }
                Ok(Err(_)) => return false,
                Err(_) => return false,
            }
        }
    }

    #[tokio::test]
    async fn test_watcher_emits_create_modify_remove() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        let (tx, _) = broadcast::channel(128);
        let debounce = StdDuration::from_millis(100);
        let _jh = start_watcher(vec![root.clone()], debounce, tx.clone());
        let mut rx = tx.subscribe();

        let file = root.join("watch_test.txt");

        // Create
        std::fs::write(&file, b"hello").unwrap();
        assert!(
            recv_kind_within(
                &mut rx,
                FileEventKind::Created,
                Some(&file),
                Duration::from_secs(3)
            )
            .await,
            "Did not receive Created event for {}",
            file.display()
        );

        // Modify
        std::fs::write(&file, b"world").unwrap();
        assert!(
            recv_kind_within(
                &mut rx,
                FileEventKind::Modified,
                Some(&file),
                Duration::from_secs(3)
            )
            .await,
            "Did not receive Modified event for {}",
            file.display()
        );

        // Remove
        std::fs::remove_file(&file).unwrap();
        assert!(
            recv_kind_within(
                &mut rx,
                FileEventKind::Removed,
                Some(&file),
                Duration::from_secs(3)
            )
            .await,
            "Did not receive Removed event for {}",
            file.display()
        );
    }

    #[tokio::test]
    async fn test_watcher_emits_rename() {
        let dir = tempdir().unwrap();
        let root = dir.path().to_path_buf();

        let (tx, _) = broadcast::channel(128);
        let debounce = StdDuration::from_millis(100);
        let _jh = start_watcher(vec![root.clone()], debounce, tx.clone());
        let mut rx = tx.subscribe();

        let old_path = root.join("old_name.txt");
        let new_path = root.join("new_name.txt");

        std::fs::write(&old_path, b"data").unwrap();
        // Wait for initial create to clear the pipeline
        let _ = recv_kind_within(
            &mut rx,
            FileEventKind::Created,
            Some(&old_path),
            Duration::from_secs(3),
        )
        .await;

        std::fs::rename(&old_path, &new_path).unwrap();

        // Accept either form (rename events can surface with 'to' or 'from' depending on backend)
        let deadline = tokio::time::Instant::now() + Duration::from_secs(3);
        let mut seen = false;
        while tokio::time::Instant::now() < deadline {
            if let Ok(evt) = timeout(Duration::from_millis(500), rx.recv()).await {
                if let Ok(evt) = evt {
                    if let FileEventKind::Renamed = evt.kind {
                        // Path may be either old or new depending on platform
                        if evt.path == new_path || evt.path == old_path {
                            seen = true;
                            break;
                        }
                    }
                }
            }
        }
        assert!(
            seen,
            "Did not receive Renamed event for rename {} -> {}",
            old_path.display(),
            new_path.display()
        );
    }
}
