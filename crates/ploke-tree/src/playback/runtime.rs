use std::marker::PhantomData;

use ploke_records::ids::{ArtifactId, BlockHash, LineageId};
use ploke_records::playback::EvidenceStrength;
use serde::{Deserialize, Serialize};

use crate::graph::{Graph, HistoryBlockNode};
use crate::{
    AgentTurnRecordSet, TurnEventPlaybackRefSteps, turn_event_steps_from_agent_turn_records,
};

/// Scope for graph-backed runtime playback.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PlaybackScope {
    Lineage { lineage_id: LineageId },
    ArtifactAncestry { artifact_id: ArtifactId },
}

impl PlaybackScope {
    pub fn lineage(lineage_id: LineageId) -> Self {
        Self::Lineage { lineage_id }
    }

    pub fn artifact_ancestry(artifact_id: ArtifactId) -> Self {
        Self::ArtifactAncestry { artifact_id }
    }
}

/// Stable coordinate for one runtime playback step.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind")]
pub enum PlaybackCursor {
    HistoryBlock {
        lineage_id: LineageId,
        block_height: u64,
        block_hash: BlockHash,
    },
}

impl PlaybackCursor {
    pub fn block_hash(&self) -> &BlockHash {
        match self {
            Self::HistoryBlock { block_hash, .. } => block_hash,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct RuntimeCoarse;

pub trait RuntimePlaybackGranularity {
    type StepRef<'g>
    where
        Self: 'g;
}

impl RuntimePlaybackGranularity for RuntimeCoarse {
    type StepRef<'g> = RuntimePlaybackStepRef<'g>;
}

/// Derived index for graph-backed runtime playback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RuntimePlaybackIndex {
    cursors: Vec<PlaybackCursor>,
    warnings: Vec<RuntimePlaybackWarning>,
}

impl RuntimePlaybackIndex {
    pub fn cursors(&self) -> &[PlaybackCursor] {
        &self.cursors
    }

    pub fn warnings(&self) -> &[RuntimePlaybackWarning] {
        &self.warnings
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RuntimePlaybackWarning {
    MissingLineage {
        lineage_id: LineageId,
    },
    MissingHistoryBlock {
        lineage_id: LineageId,
        block_hash: BlockHash,
    },
    ArtifactNotInHistory {
        artifact_id: ArtifactId,
    },
    AgentTurnDrilldownUnscoped {
        event_count: usize,
    },
}

/// Borrowed runtime playback over an already assembled graph.
#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePlaybackRef<'g, G = RuntimeCoarse>
where
    G: RuntimePlaybackGranularity + 'g,
{
    graph: &'g Graph,
    scope: PlaybackScope,
    index: RuntimePlaybackIndex,
    _granularity: PhantomData<G>,
}

impl<'g> RuntimePlaybackRef<'g, RuntimeCoarse> {
    pub fn new(graph: &'g Graph, scope: PlaybackScope) -> Self {
        let index = RuntimePlaybackIndex::from_graph(graph, &scope);
        Self {
            graph,
            scope,
            index,
            _granularity: PhantomData,
        }
    }

    pub fn scope(&self) -> &PlaybackScope {
        &self.scope
    }

    pub fn index(&self) -> &RuntimePlaybackIndex {
        &self.index
    }

    pub fn warnings(&self) -> &[RuntimePlaybackWarning] {
        self.index.warnings()
    }

    pub fn iter(&self) -> impl Iterator<Item = RuntimePlaybackStepRef<'g>> + '_ {
        self.index
            .cursors
            .iter()
            .filter_map(|cursor| self.step_at(cursor))
    }

    pub fn frames(&self) -> impl Iterator<Item = RuntimePlaybackFrameRef<'g>> + '_ {
        self.iter().map(|step| RuntimePlaybackFrameRef {
            step,
            agent_turn_steps: self.agent_turn_steps(),
        })
    }

    pub fn deltas(&self) -> Vec<RuntimePlaybackDeltaRef<'g>> {
        let mut previous = None;
        let mut deltas = Vec::new();
        for step in self.iter() {
            deltas.push(RuntimePlaybackDeltaRef {
                from: previous,
                to: step.clone(),
            });
            previous = Some(step);
        }
        deltas
    }

    pub fn step_at(&self, cursor: &PlaybackCursor) -> Option<RuntimePlaybackStepRef<'g>> {
        self.graph
            .history
            .blocks
            .get(cursor.block_hash())
            .map(|block| RuntimePlaybackStepRef {
                cursor: cursor.clone(),
                block,
                evidence: EvidenceStrength::SealedHistory,
            })
    }

    pub fn agent_turn_steps(&self) -> TurnEventPlaybackRefSteps<'g> {
        turn_event_steps_from_agent_turn_records(self.graph.agent_turn_records())
    }
}

impl RuntimePlaybackIndex {
    fn from_graph(graph: &Graph, scope: &PlaybackScope) -> Self {
        let mut index = match scope {
            PlaybackScope::Lineage { lineage_id } => Self::from_lineage(graph, lineage_id),
            PlaybackScope::ArtifactAncestry { artifact_id } => {
                Self::from_artifact_ancestry(graph, artifact_id)
            }
        };

        let event_count = agent_turn_event_count(graph.agent_turn_records());
        if event_count > 0 && !index.cursors.is_empty() {
            index
                .warnings
                .push(RuntimePlaybackWarning::AgentTurnDrilldownUnscoped { event_count });
        }

        index
    }

    fn from_lineage(graph: &Graph, lineage_id: &LineageId) -> Self {
        let Some(lineage) = graph.history.lineages.get(lineage_id) else {
            return Self {
                cursors: Vec::new(),
                warnings: vec![RuntimePlaybackWarning::MissingLineage {
                    lineage_id: lineage_id.clone(),
                }],
            };
        };

        let mut warnings = Vec::new();
        let cursors = lineage
            .blocks
            .iter()
            .filter_map(|block_hash| {
                let Some(block) = graph.history.blocks.get(block_hash) else {
                    warnings.push(RuntimePlaybackWarning::MissingHistoryBlock {
                        lineage_id: lineage_id.clone(),
                        block_hash: block_hash.clone(),
                    });
                    return None;
                };
                Some(PlaybackCursor::HistoryBlock {
                    lineage_id: block.lineage_id.clone(),
                    block_height: block.block_height,
                    block_hash: block.block_hash.clone(),
                })
            })
            .collect();

        Self { cursors, warnings }
    }

    fn from_artifact_ancestry(graph: &Graph, artifact_id: &ArtifactId) -> Self {
        let mut cursors = Vec::new();
        for lineage in graph.history.lineages.values() {
            for block_hash in &lineage.blocks {
                let Some(block) = graph.history.blocks.get(block_hash) else {
                    continue;
                };
                if block_mentions_artifact(block, artifact_id) {
                    cursors.push(PlaybackCursor::HistoryBlock {
                        lineage_id: block.lineage_id.clone(),
                        block_height: block.block_height,
                        block_hash: block.block_hash.clone(),
                    });
                }
            }
        }

        let warnings = if cursors.is_empty() {
            vec![RuntimePlaybackWarning::ArtifactNotInHistory {
                artifact_id: artifact_id.clone(),
            }]
        } else {
            Vec::new()
        };

        Self { cursors, warnings }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePlaybackStepRef<'g> {
    pub cursor: PlaybackCursor,
    pub block: &'g HistoryBlockNode,
    pub evidence: EvidenceStrength,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePlaybackFrameRef<'g> {
    pub step: RuntimePlaybackStepRef<'g>,
    pub agent_turn_steps: TurnEventPlaybackRefSteps<'g>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RuntimePlaybackDeltaRef<'g> {
    pub from: Option<RuntimePlaybackStepRef<'g>>,
    pub to: RuntimePlaybackStepRef<'g>,
}

fn block_mentions_artifact(block: &HistoryBlockNode, artifact_id: &ArtifactId) -> bool {
    block
        .opened_from_artifact
        .artifact_id()
        .is_some_and(|id| id == artifact_id)
        || block
            .active_artifact
            .artifact_id()
            .is_some_and(|id| id == artifact_id)
        || block
            .selected_successor
            .artifact
            .artifact_id()
            .is_some_and(|id| id == artifact_id)
}

fn agent_turn_event_count(records: &AgentTurnRecordSet) -> usize {
    records
        .traces
        .values()
        .chain(records.summaries.values())
        .map(|record| record.events.len())
        .sum()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use ploke_records::history::{
        ActorRefRecord, ArtifactRefRecord, EvidenceRefRecord, ProcedureRefRecord,
        SurfaceCommitmentRecord, SurfaceDeltaRecord, SurfaceRecord, SurfaceRootRecord,
    };
    use ploke_records::ids::{
        ArtifactId, BlockHash, BlockId, HistoryHash, LineageId, RecordedAt, RuntimeId,
    };

    use super::*;
    use crate::graph::{
        HistoryBlockNode, HistoryIndex, LineageNode, OpeningAuthorityNode, SuccessorNode,
    };

    #[test]
    fn runtime_playback_lineage_scope_orders_blocks_from_graph_lineage() {
        let lineage = LineageId("lineage:primary".to_owned());
        let first = BlockHash("block:first".to_owned());
        let second = BlockHash("block:second".to_owned());
        let graph = Graph {
            history: HistoryIndex {
                lineages: BTreeMap::from([(
                    lineage.clone(),
                    LineageNode {
                        lineage_id: lineage.clone(),
                        blocks: vec![second.clone(), first.clone()],
                    },
                )]),
                blocks: BTreeMap::from([
                    (
                        first.clone(),
                        history_block(first, lineage.clone(), 0, "artifact:base", "artifact:mid"),
                    ),
                    (
                        second.clone(),
                        history_block(second, lineage.clone(), 1, "artifact:mid", "artifact:leaf"),
                    ),
                ]),
                ..HistoryIndex::default()
            },
            ..Graph::default()
        };

        let playback = RuntimePlaybackRef::new(&graph, PlaybackScope::lineage(lineage));
        let heights = playback
            .iter()
            .map(|step| step.block.block_height)
            .collect::<Vec<_>>();

        assert_eq!(heights, vec![1, 0]);
        assert!(playback.warnings().is_empty());
    }

    #[test]
    fn runtime_playback_missing_block_is_warning_not_fabricated_step() {
        let lineage = LineageId("lineage:primary".to_owned());
        let present = BlockHash("block:present".to_owned());
        let missing = BlockHash("block:missing".to_owned());
        let graph = Graph {
            history: HistoryIndex {
                lineages: BTreeMap::from([(
                    lineage.clone(),
                    LineageNode {
                        lineage_id: lineage.clone(),
                        blocks: vec![present.clone(), missing.clone()],
                    },
                )]),
                blocks: BTreeMap::from([(
                    present.clone(),
                    history_block(
                        present,
                        lineage.clone(),
                        0,
                        "artifact:base",
                        "artifact:leaf",
                    ),
                )]),
                ..HistoryIndex::default()
            },
            ..Graph::default()
        };

        let playback = RuntimePlaybackRef::new(&graph, PlaybackScope::lineage(lineage.clone()));
        let steps = playback.iter().collect::<Vec<_>>();

        assert_eq!(steps.len(), 1);
        assert_eq!(
            playback.warnings(),
            &[RuntimePlaybackWarning::MissingHistoryBlock {
                lineage_id: lineage,
                block_hash: missing
            }]
        );
    }

    #[test]
    fn runtime_playback_step_ref_borrows_history_block_from_graph() {
        let lineage = LineageId("lineage:primary".to_owned());
        let block_hash = BlockHash("block:one".to_owned());
        let graph = Graph {
            history: HistoryIndex {
                lineages: BTreeMap::from([(
                    lineage.clone(),
                    LineageNode {
                        lineage_id: lineage.clone(),
                        blocks: vec![block_hash.clone()],
                    },
                )]),
                blocks: BTreeMap::from([(
                    block_hash.clone(),
                    history_block(
                        block_hash.clone(),
                        lineage.clone(),
                        0,
                        "artifact:base",
                        "artifact:leaf",
                    ),
                )]),
                ..HistoryIndex::default()
            },
            ..Graph::default()
        };

        let playback = RuntimePlaybackRef::new(&graph, PlaybackScope::lineage(lineage));
        let step = playback.iter().next().expect("playback step");
        let graph_block = graph
            .history
            .blocks
            .get(&block_hash)
            .expect("graph history block");

        assert!(std::ptr::eq(step.block, graph_block));
        assert_eq!(step.evidence, EvidenceStrength::SealedHistory);
    }

    #[test]
    fn runtime_playback_artifact_ancestry_uses_history_artifact_refs() {
        let lineage = LineageId("lineage:primary".to_owned());
        let first = BlockHash("block:first".to_owned());
        let second = BlockHash("block:second".to_owned());
        let graph = Graph {
            history: HistoryIndex {
                lineages: BTreeMap::from([(
                    lineage.clone(),
                    LineageNode {
                        lineage_id: lineage.clone(),
                        blocks: vec![first.clone(), second.clone()],
                    },
                )]),
                blocks: BTreeMap::from([
                    (
                        first.clone(),
                        history_block(first, lineage.clone(), 0, "artifact:base", "artifact:mid"),
                    ),
                    (
                        second.clone(),
                        history_block(second, lineage, 1, "artifact:mid", "artifact:leaf"),
                    ),
                ]),
                ..HistoryIndex::default()
            },
            ..Graph::default()
        };

        let playback = RuntimePlaybackRef::new(
            &graph,
            PlaybackScope::artifact_ancestry(ArtifactId("artifact:mid".to_owned())),
        );
        let heights = playback
            .iter()
            .map(|step| step.block.block_height)
            .collect::<Vec<_>>();

        assert_eq!(heights, vec![0, 1]);
        assert!(playback.warnings().is_empty());
    }

    fn history_block(
        block_hash: BlockHash,
        lineage_id: LineageId,
        block_height: u64,
        active: &str,
        successor: &str,
    ) -> HistoryBlockNode {
        HistoryBlockNode {
            block_hash,
            block_id: BlockId(format!("block-id:{block_height}")),
            lineage_id,
            block_height,
            parent_block_hashes: Vec::new(),
            opened_from_artifact: artifact_ref(active),
            active_artifact: artifact_ref(active),
            selected_successor: SuccessorNode {
                runtime: ActorRefRecord::Runtime(RuntimeId("runtime".to_owned())),
                artifact: artifact_ref(successor),
            },
            opening_authority: OpeningAuthorityNode::Genesis {
                bootstrap_policy: policy(),
                tree_key_hash: "tree".to_owned(),
                parent_identity_ref: evidence_ref("parent-identity"),
            },
            ruling_authority: ActorRefRecord::Runtime(RuntimeId("runtime".to_owned())),
            policy_ref: policy(),
            surface: surface(),
            opened_at: RecordedAt(1),
            sealed_at: RecordedAt(2),
            entry_count: 0,
            entries: Vec::new(),
        }
    }

    fn artifact_ref(value: &str) -> ArtifactRefRecord {
        ArtifactRefRecord::from_artifact_id(ArtifactId(value.to_owned()))
    }

    fn policy() -> ProcedureRefRecord {
        ProcedureRefRecord {
            value: "policy".to_owned(),
        }
    }

    fn evidence_ref(value: &str) -> EvidenceRefRecord {
        EvidenceRefRecord {
            value: value.to_owned(),
        }
    }

    fn surface() -> SurfaceCommitmentRecord {
        SurfaceCommitmentRecord {
            immutable: surface_record("immutable"),
            mutated: surface_delta("mutated"),
            ambient: surface_delta("ambient"),
        }
    }

    fn surface_delta(value: &str) -> SurfaceDeltaRecord {
        SurfaceDeltaRecord {
            before: surface_record(&format!("{value}:before")),
            after: surface_record(&format!("{value}:after")),
        }
    }

    fn surface_record(value: &str) -> SurfaceRecord {
        SurfaceRecord {
            root: SurfaceRootRecord {
                hash: HistoryHash(value.to_owned()),
            },
        }
    }
}
