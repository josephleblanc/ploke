use std::path::{Path, PathBuf};

use super::super::{
    cli_facing::parent_target_sample,
    journal::{self, JournalEntry, ParentStartedEntry, PrototypeJournal},
    profile::EvalStorageBackend,
};
use super::{
    agent_turn::{
        AgentTurnBundleEvidence, AgentTurnBundleReceipt, write_agent_turn_bundle_files,
        write_agent_turn_to_owner_db,
    },
    cozo_store::{owner_eval_db_file_for_record_path, write_parent_started_to_owner_db},
    error::EvalStoreError,
    evidence::{ParentStartedEvidence, ParentStartedReceipt, parent_started_db_receipt},
};

pub(crate) trait EvalStore {
    fn put_parent_started(
        &mut self,
        evidence: ParentStartedEvidence,
    ) -> Result<ParentStartedReceipt, EvalStoreError>;

    fn put_agent_turn_bundle(
        &mut self,
        evidence: AgentTurnBundleEvidence,
    ) -> Result<AgentTurnBundleReceipt, EvalStoreError>;
}

pub(crate) enum ConfiguredEvalStore<'a> {
    Fs(FsEvalStore<'a>),
    DbMirror(FileDbEvalStore<'a>),
    DualStrict(FileDbEvalStore<'a>),
}

impl<'a> ConfiguredEvalStore<'a> {
    pub(crate) fn fs(journal: &'a mut PrototypeJournal) -> Self {
        Self::Fs(FsEvalStore::new(journal))
    }

    pub(crate) fn db_mirror(journal: &'a mut PrototypeJournal, db_path: PathBuf) -> Self {
        Self::DbMirror(FileDbEvalStore::new(
            journal,
            db_path,
            EvalStorageMode::DbMirror,
        ))
    }

    pub(crate) fn dual_strict(journal: &'a mut PrototypeJournal, db_path: PathBuf) -> Self {
        Self::DualStrict(FileDbEvalStore::new(
            journal,
            db_path,
            EvalStorageMode::DualStrict,
        ))
    }

    pub(crate) fn for_record_backend(
        backend: EvalStorageBackend,
        record_path: &Path,
    ) -> Result<Self, EvalStoreError> {
        match backend {
            EvalStorageBackend::Fs => Ok(Self::Fs(FsEvalStore::without_journal())),
            EvalStorageBackend::DbMirror | EvalStorageBackend::Database => {
                Ok(Self::DbMirror(FileDbEvalStore::without_journal(
                    owner_eval_db_file_for_record_path(record_path)?,
                    EvalStorageMode::DbMirror,
                )))
            }
            EvalStorageBackend::DualStrict => {
                Ok(Self::DualStrict(FileDbEvalStore::without_journal(
                    owner_eval_db_file_for_record_path(record_path)?,
                    EvalStorageMode::DualStrict,
                )))
            }
        }
    }
}

impl EvalStore for ConfiguredEvalStore<'_> {
    fn put_parent_started(
        &mut self,
        evidence: ParentStartedEvidence,
    ) -> Result<ParentStartedReceipt, EvalStoreError> {
        match self {
            Self::Fs(store) => store.put_parent_started(evidence),
            Self::DbMirror(store) | Self::DualStrict(store) => store.put_parent_started(evidence),
        }
    }

    fn put_agent_turn_bundle(
        &mut self,
        evidence: AgentTurnBundleEvidence,
    ) -> Result<AgentTurnBundleReceipt, EvalStoreError> {
        match self {
            Self::Fs(store) => store.put_agent_turn_bundle(evidence),
            Self::DbMirror(store) | Self::DualStrict(store) => {
                store.put_agent_turn_bundle(evidence)
            }
        }
    }
}

pub(crate) struct FsEvalStore<'a> {
    journal: Option<&'a mut PrototypeJournal>,
}

impl<'a> FsEvalStore<'a> {
    pub(crate) fn new(journal: &'a mut PrototypeJournal) -> Self {
        Self {
            journal: Some(journal),
        }
    }

    pub(crate) fn without_journal() -> Self {
        Self { journal: None }
    }

    fn journal_mut(
        &mut self,
        phase: &'static str,
    ) -> Result<&mut PrototypeJournal, EvalStoreError> {
        self.journal
            .as_deref_mut()
            .ok_or_else(|| EvalStoreError::DbSetup {
                phase,
                detail: "filesystem eval-store operation requires a Prototype 1 journal"
                    .to_string(),
            })
    }
}

impl EvalStore for FsEvalStore<'_> {
    fn put_parent_started(
        &mut self,
        evidence: ParentStartedEvidence,
    ) -> Result<ParentStartedReceipt, EvalStoreError> {
        let journal = self.journal_mut("parent_started.journal")?;
        append_parent_started_entries(journal, &evidence)
    }

    fn put_agent_turn_bundle(
        &mut self,
        evidence: AgentTurnBundleEvidence,
    ) -> Result<AgentTurnBundleReceipt, EvalStoreError> {
        write_agent_turn_bundle_files(&evidence)?;
        Ok(AgentTurnBundleReceipt {
            trace_path: evidence.trace_path,
            summary_path: evidence.summary_path,
            full_response_path: evidence.full_response_path,
            db_receipt: None,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum EvalStorageMode {
    DbMirror,
    DualStrict,
}

impl EvalStorageMode {
    pub(super) const fn as_str(self) -> &'static str {
        match self {
            Self::DbMirror => "db-mirror",
            Self::DualStrict => "dual-strict",
        }
    }
}

pub(crate) struct FileDbEvalStore<'a> {
    journal: Option<&'a mut PrototypeJournal>,
    db_path: PathBuf,
    mode: EvalStorageMode,
}

impl<'a> FileDbEvalStore<'a> {
    pub(super) fn new(
        journal: &'a mut PrototypeJournal,
        db_path: PathBuf,
        mode: EvalStorageMode,
    ) -> Self {
        Self {
            journal: Some(journal),
            db_path,
            mode,
        }
    }

    pub(super) fn without_journal(db_path: PathBuf, mode: EvalStorageMode) -> Self {
        Self {
            journal: None,
            db_path,
            mode,
        }
    }

    fn journal_mut(
        &mut self,
        phase: &'static str,
    ) -> Result<&mut PrototypeJournal, EvalStoreError> {
        self.journal
            .as_deref_mut()
            .ok_or_else(|| EvalStoreError::DbSetup {
                phase,
                detail: "filesystem eval-store operation requires a Prototype 1 journal"
                    .to_string(),
            })
    }
}

impl EvalStore for FileDbEvalStore<'_> {
    fn put_parent_started(
        &mut self,
        evidence: ParentStartedEvidence,
    ) -> Result<ParentStartedReceipt, EvalStoreError> {
        let journal = self.journal_mut("parent_started.journal")?;
        let receipt = append_parent_started_entries(journal, &evidence)?;
        let expected = parent_started_db_receipt(&evidence, &receipt).ok();
        let db_result = write_parent_started_to_owner_db(&self.db_path, &evidence, &receipt);
        match db_result {
            Ok(db_receipt) => {
                if self.mode == EvalStorageMode::DualStrict {
                    let expected = expected.ok_or_else(|| EvalStoreError::post_fs_db(
                        self.mode,
                        &receipt,
                        None,
                        "failed to construct expected parent-start semantic receipt after filesystem append".to_string(),
                    ))?;
                    if db_receipt.semantic_hash != expected.semantic_hash {
                        return Err(EvalStoreError::post_fs_db(
                            self.mode,
                            &receipt,
                            Some(expected.semantic_hash.clone()),
                            format!(
                                "db semantic hash mismatch: wrote {}, expected {}",
                                db_receipt.semantic_hash, expected.semantic_hash
                            ),
                        ));
                    }
                }
                Ok(receipt)
            }
            Err(err) => Err(EvalStoreError::post_fs_db(
                self.mode,
                &receipt,
                expected.map(|receipt| receipt.semantic_hash),
                err.to_string(),
            )),
        }
    }

    fn put_agent_turn_bundle(
        &mut self,
        evidence: AgentTurnBundleEvidence,
    ) -> Result<AgentTurnBundleReceipt, EvalStoreError> {
        write_agent_turn_bundle_files(&evidence)?;
        let expected_events = evidence.summary_record.0.events.len();
        let expected_exchanges = evidence.full_responses.len();
        let db_receipt = write_agent_turn_to_owner_db(&self.db_path, evidence.db_evidence())
            .map_err(|err| EvalStoreError::Validation {
                field: "agent_turn.db_mirror",
                detail: format!(
                    "{} DB write failed after filesystem agent-turn bundle '{}': {err}",
                    self.mode.as_str(),
                    evidence.trace_path.display()
                ),
            })?;
        if self.mode == EvalStorageMode::DualStrict
            && (db_receipt.event_ids.len() != expected_events
                || db_receipt.exchange_ids.len() != expected_exchanges)
        {
            return Err(EvalStoreError::Validation {
                field: "agent_turn.dual_strict",
                detail: format!(
                    "DB receipt count mismatch after filesystem agent-turn bundle '{}': events {}/{}, exchanges {}/{}",
                    evidence.trace_path.display(),
                    db_receipt.event_ids.len(),
                    expected_events,
                    db_receipt.exchange_ids.len(),
                    expected_exchanges
                ),
            });
        }
        Ok(AgentTurnBundleReceipt {
            trace_path: evidence.trace_path,
            summary_path: evidence.summary_path,
            full_response_path: evidence.full_response_path,
            db_receipt: Some(db_receipt),
        })
    }
}

fn append_parent_started_entries(
    journal: &mut PrototypeJournal,
    evidence: &ParentStartedEvidence,
) -> Result<ParentStartedReceipt, EvalStoreError> {
    let parent = journal
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
    let resource = journal
        .append_with_receipt(JournalEntry::Resource(sample))
        .map_err(|source| EvalStoreError::Journal {
            phase: "parent_start_resource",
            source,
        })?;

    Ok(ParentStartedReceipt { parent, resource })
}
