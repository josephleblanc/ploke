//! Native filesystem persistence for diagnostic snapshots.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use super::{Snapshot, SnapshotObservation};

const DEFAULT_MAX_SNAPSHOTS: u64 = 10;

#[derive(Debug)]
pub struct SnapshotSink {
    root: PathBuf,
    max_snapshots: u64,
    sequence: u64,
    last_bytes: Option<Vec<u8>>,
}

impl SnapshotSink {
    pub fn new(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        fs::create_dir_all(root.join("snapshots"))?;
        Ok(Self {
            root,
            max_snapshots: DEFAULT_MAX_SNAPSHOTS,
            sequence: 0,
            last_bytes: None,
        })
    }

    pub fn observe(&mut self, observation: SnapshotObservation<'_>) -> io::Result<bool> {
        let comparison = Snapshot::from_observation(self.sequence, observation.clone());
        let comparison_bytes = serde_json::to_vec_pretty(&comparison).map_err(io::Error::other)?;
        if self.last_bytes.as_ref() == Some(&comparison_bytes) {
            return Ok(false);
        }

        self.sequence = self.sequence.saturating_add(1);
        let snapshot = Snapshot::from_observation(self.sequence, observation);
        let bytes = serde_json::to_vec_pretty(&snapshot).map_err(io::Error::other)?;
        self.last_bytes = Some(bytes.clone());
        self.write_snapshot(&snapshot, bytes)?;
        Ok(true)
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    fn write_snapshot(&self, snapshot: &Snapshot<'_>, bytes: Vec<u8>) -> io::Result<()> {
        fs::write(self.root.join("latest.json"), &bytes)?;
        fs::write(self.root.join("latest.txt"), snapshot.render_text())?;
        let slot = ((snapshot.sequence - 1) % self.max_snapshots) + 1;
        fs::write(
            self.root.join("snapshots").join(format!("{slot:02}.json")),
            bytes,
        )?;
        fs::write(
            self.root.join("snapshots").join(format!("{slot:02}.txt")),
            snapshot.render_text(),
        )
    }
}

pub fn write_manual_snapshot(observation: SnapshotObservation<'_>) -> io::Result<PathBuf> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("data/manual-diagnostics");
    fs::create_dir_all(&root)?;

    let snapshot = Snapshot::from_observation(1, observation);
    let bytes = serde_json::to_vec_pretty(&snapshot).map_err(io::Error::other)?;
    fs::write(root.join("latest.json"), bytes)?;
    fs::write(root.join("latest.txt"), snapshot.render_text())?;
    Ok(root.join("latest.txt"))
}
