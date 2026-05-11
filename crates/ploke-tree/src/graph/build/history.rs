use std::collections::BTreeMap;

use ploke_records::history::{
    AdmittedEntryRecord, EntryPayloadRecord, OpeningAuthorityRecord, SealedBlockRecord,
};
use ploke_records::ids::BlockHash;

use crate::graph::{
    EvidenceKind, EvidenceSubject, GraphWarningKind, HistoryBlockNode, HistoryEntryNode,
    HistoryPayloadKind, LineageNode, OpeningAuthorityNode, SuccessorNode,
};

use super::{Builder, artifact_ref_key};

impl Builder {
    pub(super) fn normalize_history_order(&mut self) {
        let mut duplicate_heights = Vec::new();

        for lineage in self.graph.history.lineages.values_mut() {
            lineage.blocks.sort_by_key(|block_hash| {
                self.graph
                    .history
                    .blocks
                    .get(block_hash)
                    .map(|block| block.block_height)
                    .unwrap_or(u64::MAX)
            });

            let mut height_owner = BTreeMap::new();
            for block_hash in &lineage.blocks {
                let Some(block) = self.graph.history.blocks.get(block_hash) else {
                    continue;
                };
                if let Some(existing) = height_owner.insert(block.block_height, block_hash.clone())
                {
                    duplicate_heights.push((
                        lineage.lineage_id.clone(),
                        block.block_height,
                        existing,
                        block_hash.clone(),
                    ));
                }
            }
        }

        for (lineage_id, blocks) in &mut self.graph.authority.epochs_by_lineage {
            blocks.sort_by_key(|block_hash| {
                self.graph
                    .history
                    .blocks
                    .get(block_hash)
                    .map(|block| block.block_height)
                    .unwrap_or(u64::MAX)
            });
            if let Some(lineage) = self.graph.history.lineages.get(lineage_id) {
                *blocks = lineage.blocks.clone();
            }
        }

        for (lineage_id, height, first, second) in duplicate_heights {
            self.warn(
                GraphWarningKind::DuplicateLineageBlockHeight,
                format!(
                    "lineage {} has duplicate block_height={} at blocks {} and {}",
                    lineage_id.0, height, first.0, second.0
                ),
            );
        }
    }

    pub(super) fn ingest_history(&mut self, blocks: &[SealedBlockRecord]) {
        for block in blocks {
            let header = &block.state.header;
            let common = &header.common;
            let block_hash = header.block_hash.clone();

            if self.graph.history.blocks.contains_key(&block_hash) {
                self.warn(
                    GraphWarningKind::DuplicateBlockHash,
                    format!("duplicate sealed block hash {}", block_hash.0),
                );
                continue;
            }

            self.graph
                .history
                .lineages
                .entry(common.lineage_id.clone())
                .or_insert_with(|| LineageNode {
                    lineage_id: common.lineage_id.clone(),
                    blocks: Vec::new(),
                })
                .blocks
                .push(block_hash.clone());

            self.graph
                .authority
                .epochs_by_lineage
                .entry(common.lineage_id.clone())
                .or_default()
                .push(block_hash.clone());

            self.observe_artifact_ref(&common.opened_from_artifact);
            self.observe_artifact_ref(&header.active_artifact);
            self.observe_artifact_ref(&header.selected_successor.artifact);
            self.observe_actor_ref(&header.selected_successor.runtime);
            self.observe_actor_ref(&common.opened_by);
            self.observe_actor_ref(&common.ruling_authority);

            let mut entry_ids = Vec::with_capacity(block.entries.len());
            for entry in &block.entries {
                entry_ids.push(entry.core.entry_id.clone());
                self.ingest_entry(&block_hash, entry);
            }

            if header.entry_count != block.entries.len() {
                self.warn(
                    GraphWarningKind::BlockEntryCountMismatch,
                    format!(
                        "block {} header entry_count={} actual={}",
                        block_hash.0,
                        header.entry_count,
                        block.entries.len()
                    ),
                );
            }

            let evidence_id = self.attach_evidence(
                EvidenceSubject::HistoryBlock(block_hash.clone()),
                EvidenceKind::HistoryBlockHeader,
                vec![header.crown_lock_transition.clone()],
            );

            let node = HistoryBlockNode {
                block_hash: block_hash.clone(),
                block_id: common.block_id.clone(),
                lineage_id: common.lineage_id.clone(),
                block_height: common.block_height,
                parent_block_hashes: common.parent_block_hashes.clone(),
                opened_from_artifact: common.opened_from_artifact.clone(),
                active_artifact: header.active_artifact.clone(),
                selected_successor: SuccessorNode {
                    runtime: header.selected_successor.runtime.clone(),
                    artifact: header.selected_successor.artifact.clone(),
                },
                opening_authority: opening_authority_node(&common.opening_authority),
                ruling_authority: common.ruling_authority.clone(),
                policy_ref: common.policy_ref.clone(),
                surface: common.surface.clone(),
                opened_at: common.opened_at,
                sealed_at: header.sealed_at,
                entry_count: header.entry_count,
                entries: entry_ids,
            };
            self.graph.history.blocks.insert(block_hash, node);

            let artifact_key = artifact_ref_key(&header.active_artifact);
            if let Some(artifact) = self.graph.artifacts.artifacts.get_mut(&artifact_key) {
                artifact.evidence.push(evidence_id);
            }
        }
    }

    fn ingest_entry(&mut self, block_hash: &BlockHash, entry: &AdmittedEntryRecord) {
        if self
            .graph
            .history
            .entries
            .contains_key(&entry.core.entry_id)
        {
            self.warn(
                GraphWarningKind::DuplicateEntryId,
                format!("duplicate History entry id {}", entry.core.entry_id.0),
            );
            return;
        }

        self.observe_actor_ref(&entry.core.executor);
        self.observe_actor_ref(&entry.state.observed.observer);
        self.observe_actor_ref(&entry.state.observed.recorder);
        self.observe_actor_ref(&entry.state.proposer);
        self.observe_actor_ref(&entry.state.admitting_authority);
        self.observe_actor_ref(&entry.state.ruling_authority);
        if let Some(runtime) = entry
            .state
            .observed
            .operational_environment
            .runtime
            .as_ref()
        {
            self.observe_runtime(runtime);
        }
        if let Some(artifact) = entry
            .state
            .observed
            .operational_environment
            .artifact
            .as_ref()
        {
            self.observe_artifact_ref(artifact);
        }

        let mut refs = entry.core.input_refs.clone();
        refs.extend(entry.core.output_refs.clone());
        refs.push(entry.state.observed.payload_ref.clone());
        self.attach_evidence(
            EvidenceSubject::HistoryEntry(entry.core.entry_id.clone()),
            EvidenceKind::HistoryEntryCustody,
            refs,
        );

        if let EntryPayloadRecord::SelectionDecision(selection) = &entry.core.payload {
            self.ingest_selection(entry, selection);
        }

        let payload = match entry.core.payload {
            EntryPayloadRecord::Direct => HistoryPayloadKind::Direct,
            EntryPayloadRecord::SelectionDecision(_) => HistoryPayloadKind::SelectionDecision,
            EntryPayloadRecord::IngressImport(_) => HistoryPayloadKind::IngressImport,
        };

        let node = HistoryEntryNode {
            entry_id: entry.core.entry_id.clone(),
            block_hash: block_hash.clone(),
            lineage_id: entry.state.lineage_id.clone(),
            block_id: entry.state.block_id.clone(),
            block_height: entry.state.block_height,
            kind: entry.core.entry_kind.clone(),
            subject: entry.core.subject.clone(),
            executor: entry.core.executor.clone(),
            observer: entry.state.observed.observer.clone(),
            recorder: entry.state.observed.recorder.clone(),
            proposer: entry.state.proposer.clone(),
            admitting_authority: entry.state.admitting_authority.clone(),
            ruling_authority: entry.state.ruling_authority.clone(),
            procedure_or_policy: entry.state.procedure_or_policy.clone(),
            payload,
            occurred_at: entry.core.occurred_at,
            observed_at: entry.state.observed.observed_at,
            recorded_at: entry.state.observed.recorded_at,
            input_refs: entry.core.input_refs.clone(),
            output_refs: entry.core.output_refs.clone(),
            payload_ref: entry.state.observed.payload_ref.clone(),
        };

        self.graph
            .history
            .entries
            .insert(entry.core.entry_id.clone(), node);
    }
}

fn opening_authority_node(record: &OpeningAuthorityRecord) -> OpeningAuthorityNode {
    match record {
        OpeningAuthorityRecord::Genesis(genesis) => OpeningAuthorityNode::Genesis {
            bootstrap_policy: genesis.bootstrap_policy.clone(),
            tree_key_hash: genesis.tree_key.hash.0.clone(),
            parent_identity_ref: genesis.parent_identity.evidence.clone(),
        },
        OpeningAuthorityRecord::Predecessor(predecessor) => OpeningAuthorityNode::Predecessor {
            predecessor_block_hash: predecessor.predecessor_block_hash.clone(),
        },
    }
}
