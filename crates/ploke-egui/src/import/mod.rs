//! Read-only imports from typed run projections into the operator graph.
//!
//! This module adapts records loaded by `ploke-tree`; it does not parse
//! Prototype 1 files directly and it does not add runtime authority.

#[cfg(test)]
mod tests;

use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use ploke_tree::{
    FsRunStore, FsRunStoreError, Graph, GraphSnapshot, GraphSnapshotError, RunRecordSet,
};

pub fn graph_from_run_root(run_root: impl AsRef<Path>) -> Result<Graph, ImportError> {
    let store = FsRunStore::new(run_root.as_ref());
    let records = store.load_record_set()?;

    Ok(graph_from_run_records(&records))
}

pub fn graph_from_run_records(records: &RunRecordSet) -> Graph {
    Graph::from_records(records)
}

pub fn graph_from_snapshot(path: impl AsRef<Path>) -> Result<Graph, ImportError> {
    Ok(GraphSnapshot::read_json(path)?.graph())
}

pub fn graph_from_snapshot_bytes(bytes: &[u8]) -> Result<Graph, ImportError> {
    let snapshot: GraphSnapshot = serde_json::from_slice(bytes).map_err(|source| {
        ImportError::Snapshot(GraphSnapshotError::Json {
            path: PathBuf::from("<bytes>"),
            source,
        })
    })?;
    Ok(snapshot.graph())
}

#[derive(Debug)]
pub enum ImportError {
    Store(FsRunStoreError),
    Snapshot(GraphSnapshotError),
}

impl fmt::Display for ImportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(f, "{error}"),
            Self::Snapshot(error) => write!(f, "{error}"),
        }
    }
}

impl Error for ImportError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Snapshot(error) => Some(error),
        }
    }
}

impl From<FsRunStoreError> for ImportError {
    fn from(error: FsRunStoreError) -> Self {
        Self::Store(error)
    }
}

impl From<GraphSnapshotError> for ImportError {
    fn from(error: GraphSnapshotError) -> Self {
        Self::Snapshot(error)
    }
}
