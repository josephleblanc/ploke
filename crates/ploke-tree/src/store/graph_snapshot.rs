use std::error::Error;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::{FsRunStore, FsRunStoreError, Graph, RunRecordSet};

/// Git-portable typed input snapshot for reconstructing a [`Graph`].
///
/// The snapshot stores the canonical record set loaded by [`FsRunStore`].
/// Consumers rebuild the read-side graph with [`Graph::from_records`] instead
/// of depending on the original run root or eval-home file layout.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GraphSnapshot {
    pub format_version: u32,
    pub records: RunRecordSet,
}

impl GraphSnapshot {
    pub const FORMAT_VERSION: u32 = 1;

    pub fn from_records(records: RunRecordSet) -> Self {
        Self {
            format_version: Self::FORMAT_VERSION,
            records,
        }
    }

    pub fn from_run_root(run_root: impl AsRef<Path>) -> Result<Self, GraphSnapshotError> {
        let records = FsRunStore::new(run_root.as_ref()).load_record_set()?;
        Ok(Self::from_records(records))
    }

    pub fn graph(&self) -> Graph {
        Graph::from_records(&self.records)
    }

    pub fn read_json(path: impl AsRef<Path>) -> Result<Self, GraphSnapshotError> {
        let path = path.as_ref();
        let bytes = fs::read(path).map_err(|source| GraphSnapshotError::Io {
            path: path.to_path_buf(),
            source,
        })?;
        serde_json::from_slice(&bytes).map_err(|source| GraphSnapshotError::Json {
            path: path.to_path_buf(),
            source,
        })
    }

    pub fn write_json(&self, path: impl AsRef<Path>) -> Result<(), GraphSnapshotError> {
        let path = path.as_ref();
        if let Some(parent) = path
            .parent()
            .filter(|parent| !parent.as_os_str().is_empty())
        {
            fs::create_dir_all(parent).map_err(|source| GraphSnapshotError::Io {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let bytes = serde_json::to_vec_pretty(self).map_err(|source| GraphSnapshotError::Json {
            path: path.to_path_buf(),
            source,
        })?;
        fs::write(path, bytes).map_err(|source| GraphSnapshotError::Io {
            path: path.to_path_buf(),
            source,
        })
    }
}

#[derive(Debug)]
pub enum GraphSnapshotError {
    Store(FsRunStoreError),
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    Json {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl fmt::Display for GraphSnapshotError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store(error) => write!(f, "{error}"),
            Self::Io { path, source } => write!(f, "{}: {source}", path.display()),
            Self::Json { path, source } => write!(f, "{}: {source}", path.display()),
        }
    }
}

impl Error for GraphSnapshotError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::Store(error) => Some(error),
            Self::Io { source, .. } => Some(source),
            Self::Json { source, .. } => Some(source),
        }
    }
}

impl From<FsRunStoreError> for GraphSnapshotError {
    fn from(error: FsRunStoreError) -> Self {
        Self::Store(error)
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ploke_records::ids::CampaignId;
    use ploke_records::scheduler::SchedulerStateRecord;

    use crate::{
        AgentTurnRecordSet, GraphSnapshot, PassiveEvidence, RunForestInput, RunRecordSet,
        TransitionJournal,
    };

    #[test]
    fn graph_snapshot_round_trips_and_rebuilds_graph() {
        let snapshot = GraphSnapshot::from_records(empty_record_set());
        let path = std::env::temp_dir().join(format!(
            "ploke-tree-graph-snapshot-{}-{}.json",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .expect("clock after epoch")
                .as_nanos()
        ));

        snapshot.write_json(&path).expect("write graph snapshot");
        let loaded = GraphSnapshot::read_json(&path).expect("read graph snapshot");
        fs::remove_file(&path).expect("remove graph snapshot");

        assert_eq!(loaded.format_version, GraphSnapshot::FORMAT_VERSION);
        let graph = loaded.graph();
        assert_eq!(
            graph.forest.as_ref().expect("forest").campaign.campaign_id,
            CampaignId::from("campaign-1")
        );
    }

    fn empty_record_set() -> RunRecordSet {
        RunRecordSet {
            forest_input: RunForestInput {
                scheduler: SchedulerStateRecord {
                    schema_version: "prototype1-scheduler.v1".to_owned(),
                    campaign_id: CampaignId("campaign-1".to_owned()),
                    updated_at: "2026-05-20T00:00:00Z".to_owned(),
                    policy: Default::default(),
                    frontier_node_ids: Vec::new(),
                    completed_node_ids: Vec::new(),
                    failed_node_ids: Vec::new(),
                    last_continuation_decision: None,
                    nodes: Vec::new(),
                },
                node_records: Vec::new(),
                parent_identity: None,
                successor_ready: Vec::new(),
                successor_completion: Vec::new(),
                passive_evidence: PassiveEvidence::default(),
            },
            history_blocks: Vec::new(),
            transition_journal: TransitionJournal::default(),
            agent_turn_records: AgentTurnRecordSet::default(),
        }
    }
}
