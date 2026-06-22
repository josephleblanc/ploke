//! Prototype 1 eval evidence storage port.
#![cfg_attr(not(test), allow(dead_code))]
//!
//! This module is Domain-C only: it records ordinary eval evidence/projections
//! and must not replace History, channel transport, MessageBox authority,
//! invocation/bootstrap, or artifact/worktree mutation.

use std::path::PathBuf;

use ploke_records::ids::CampaignId;
use thiserror::Error;

use super::{
    cli_facing::parent_target_sample,
    event::{RecordedAt, RuntimeId},
    identity::ParentIdentity,
    journal::{
        self, JournalAppendReceipt, JournalEntry, ParentStartedEntry, PrototypeJournal,
        PrototypeJournalError,
    },
};

pub(crate) trait EvalStore {
    fn put_parent_started(
        &mut self,
        evidence: ParentStartedEvidence,
    ) -> Result<ParentStartedReceipt, EvalStoreError>;
}

pub(crate) enum ConfiguredEvalStore<'a> {
    Fs(FsEvalStore<'a>),
}

impl<'a> ConfiguredEvalStore<'a> {
    pub(crate) fn fs(journal: &'a mut PrototypeJournal) -> Self {
        Self::Fs(FsEvalStore::new(journal))
    }
}

impl EvalStore for ConfiguredEvalStore<'_> {
    fn put_parent_started(
        &mut self,
        evidence: ParentStartedEvidence,
    ) -> Result<ParentStartedReceipt, EvalStoreError> {
        match self {
            Self::Fs(store) => store.put_parent_started(evidence),
        }
    }
}

pub(crate) struct FsEvalStore<'a> {
    journal: &'a mut PrototypeJournal,
}

impl<'a> FsEvalStore<'a> {
    pub(crate) fn new(journal: &'a mut PrototypeJournal) -> Self {
        Self { journal }
    }
}

impl EvalStore for FsEvalStore<'_> {
    fn put_parent_started(
        &mut self,
        evidence: ParentStartedEvidence,
    ) -> Result<ParentStartedReceipt, EvalStoreError> {
        let parent = self
            .journal
            .append_with_receipt(JournalEntry::ParentStarted(ParentStartedEntry {
                recorded_at: evidence.parent_recorded_at,
                campaign_id: evidence.campaign_id.clone(),
                parent_identity: evidence.parent_identity.clone(),
                repo_root: evidence.repo_root.clone(),
                handoff_runtime_id: evidence.handoff_runtime_id,
                pid: evidence.pid,
            }))
            .map_err(|source| EvalStoreError::Journal {
                phase: "parent_started",
                source,
            })?;

        let sample = parent_target_sample(
            &evidence.campaign_id,
            &evidence.parent_identity,
            evidence.handoff_runtime_id,
            &evidence.repo_root,
            journal::resource::Phase::ParentStart,
            evidence.resource_recorded_at,
        );
        let resource = self
            .journal
            .append_with_receipt(JournalEntry::Resource(sample))
            .map_err(|source| EvalStoreError::Journal {
                phase: "parent_start_resource",
                source,
            })?;

        Ok(ParentStartedReceipt { parent, resource })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParentStartedEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) parent_identity: ParentIdentity,
    pub(crate) repo_root: PathBuf,
    pub(crate) handoff_runtime_id: Option<RuntimeId>,
    pub(crate) pid: u32,
    pub(crate) parent_recorded_at: RecordedAt,
    pub(crate) resource_recorded_at: RecordedAt,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParentStartedReceipt {
    pub(crate) parent: JournalAppendReceipt,
    pub(crate) resource: JournalAppendReceipt,
}

#[derive(Debug, Error)]
pub(crate) enum EvalStoreError {
    #[error("failed to append {phase} evidence to transition journal: {source}")]
    Journal {
        phase: &'static str,
        source: PrototypeJournalError,
    },
}

#[cfg(test)]
mod tests {
    use std::fs;

    use ploke_records::identity::ParentIdentityRecord;

    use super::*;
    use crate::cli::prototype1_state::identity::PARENT_IDENTITY_SCHEMA_VERSION;
    use crate::intervention::RecordStore;

    #[test]
    fn prototype1_eval_store_parent_start_fs_appends_expected_entries() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).expect("repo dir");
        let path = tmp.path().join("transition-journal.jsonl");
        let mut journal = PrototypeJournal::new(&path);
        let evidence = parent_started_evidence(repo);
        let mut store = FsEvalStore::new(&mut journal);

        let receipt = store
            .put_parent_started(evidence.clone())
            .expect("parent start writes");

        assert_eq!(receipt.parent.source_event_index, 0);
        assert_eq!(receipt.parent.source_line, 1);
        assert_eq!(receipt.resource.source_event_index, 1);
        assert_eq!(receipt.resource.source_line, 2);
        let entries = PrototypeJournal::new(&path)
            .load_entries()
            .expect("journal loads");
        assert_eq!(entries.len(), 2);
        match &entries[0] {
            JournalEntry::ParentStarted(entry) => {
                assert_eq!(entry.campaign_id, evidence.campaign_id);
                assert_eq!(entry.parent_identity, evidence.parent_identity);
                assert_eq!(entry.repo_root, evidence.repo_root);
                assert_eq!(entry.pid, evidence.pid);
            }
            other => panic!("unexpected first entry: {other:?}"),
        }
        match &entries[1] {
            JournalEntry::Resource(sample) => {
                assert_eq!(sample.campaign_id, evidence.campaign_id);
                assert_eq!(sample.parent_id, evidence.parent_identity.parent_id());
                assert_eq!(sample.phase, journal::resource::Phase::ParentStart);
                assert_eq!(sample.status, journal::resource::Status::Missing);
                assert_eq!(sample.path, evidence.repo_root.join("target"));
            }
            other => panic!("unexpected second entry: {other:?}"),
        }
    }

    #[test]
    fn append_with_receipt_preserves_record_store_bytes() {
        let tmp = tempfile::tempdir().expect("tmp");
        let old_path = tmp.path().join("old.jsonl");
        let new_path = tmp.path().join("new.jsonl");
        let entry = JournalEntry::ParentStarted(parent_entry(tmp.path().join("repo")));

        let mut old = PrototypeJournal::new(&old_path);
        old.append(entry.clone()).expect("old append");

        let mut new = PrototypeJournal::new(&new_path);
        let receipt = new.append_with_receipt(entry).expect("receipt append");

        let old_bytes = fs::read(&old_path).expect("old bytes");
        let new_bytes = fs::read(&new_path).expect("new bytes");
        assert_eq!(new_bytes, old_bytes);
        assert_eq!(receipt.byte_start, 0);
        assert_eq!(receipt.byte_len, receipt.payload_json.len());
        assert_eq!(new_bytes, format!("{}\n", receipt.payload_json).as_bytes());
        assert!(!receipt.content_sha256.is_empty());
    }

    fn parent_started_evidence(repo_root: PathBuf) -> ParentStartedEvidence {
        ParentStartedEvidence {
            campaign_id: CampaignId::from("campaign"),
            parent_identity: parent_identity(),
            repo_root,
            handoff_runtime_id: None,
            pid: 42,
            parent_recorded_at: RecordedAt(1000),
            resource_recorded_at: RecordedAt(1001),
        }
    }

    fn parent_entry(repo_root: PathBuf) -> ParentStartedEntry {
        let evidence = parent_started_evidence(repo_root);
        ParentStartedEntry {
            recorded_at: evidence.parent_recorded_at,
            campaign_id: evidence.campaign_id,
            parent_identity: evidence.parent_identity,
            repo_root: evidence.repo_root,
            handoff_runtime_id: evidence.handoff_runtime_id,
            pid: evidence.pid,
        }
    }

    fn parent_identity() -> ParentIdentity {
        ParentIdentity::from_record_for_test(ParentIdentityRecord {
            schema_version: PARENT_IDENTITY_SCHEMA_VERSION.to_string(),
            campaign_id: CampaignId::from("campaign"),
            parent_id: "parent".to_string(),
            node_id: "parent".to_string(),
            generation: 0,
            instance_id: Some("instance".to_string()),
            previous_parent_id: None,
            parent_node_id: None,
            branch_id: "branch-parent".to_string(),
            artifact_branch: Some("artifact-parent".to_string()),
            created_at: "2026-06-22T00:00:00Z".to_string(),
        })
    }
}
