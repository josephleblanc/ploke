#![cfg(feature = "watcher")]

use notify::{Config, Event, EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;
use tokio::sync::broadcast;

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
}

/// Start a background watcher thread over the provided roots, broadcasting events.
///
/// This is a minimal scaffolding; debouncing behavior is best-effort and may be refined.
pub fn start_watcher(
    roots: Vec<PathBuf>,
    debounce: Duration,
    events_tx: broadcast::Sender<FileChangeEvent>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        let config = Config::default().with_poll_interval(debounce);
        let tx = events_tx.clone();

        let mut watcher = RecommendedWatcher::new(
            move |res: Result<Event, notify::Error>| match res {
                Ok(event) => {
                    for path in event.paths {
                        let kind = map_event_kind(&event.kind);
                        let _ = tx.send(FileChangeEvent {
                            path: path.clone(),
                            kind: kind.clone(),
                        });
                    }
                }
                Err(e) => {
                    tracing::warn!("ploke-io watcher error: {:?}", e);
                    // Surface watcher errors as "Other" events on a synthetic path
                    let _ = tx.send(FileChangeEvent {
                        path: PathBuf::from(format!("watcher-error:{}", e)),
                        kind: FileEventKind::Other,
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
                });
                tracing::warn!("ploke-io watcher: failed to watch {}: {:?}", root.display(), e);
            }
        }

        // Keep the thread alive; watcher must not be dropped.
        loop {
            thread::park();
        }
    })
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
        EventKind::Modify(ModifyKind::Name(RenameMode::Both)) | EventKind::Modify(ModifyKind::Name(RenameMode::From)) | EventKind::Modify(ModifyKind::Name(RenameMode::To)) => {
            FileEventKind::Renamed
        }
        _ => FileEventKind::Other,
    }
}
