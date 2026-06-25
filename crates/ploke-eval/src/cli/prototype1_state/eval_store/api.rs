use std::path::PathBuf;

use super::super::{
    cli_facing::parent_target_sample,
    journal::{self, JournalEntry, ParentStartedEntry, PrototypeJournal},
};
use super::{
    cozo_store::write_parent_started_to_owner_db,
    error::EvalStoreError,
    evidence::{ParentStartedEvidence, ParentStartedReceipt, parent_started_db_receipt},
};

pub(crate) trait EvalStore {
    fn put_parent_started(
        &mut self,
        evidence: ParentStartedEvidence,
    ) -> Result<ParentStartedReceipt, EvalStoreError>;
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
        append_parent_started_entries(self.journal, &evidence)
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
    journal: &'a mut PrototypeJournal,
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
            journal,
            db_path,
            mode,
        }
    }
}

impl EvalStore for FileDbEvalStore<'_> {
    fn put_parent_started(
        &mut self,
        evidence: ParentStartedEvidence,
    ) -> Result<ParentStartedReceipt, EvalStoreError> {
        let receipt = append_parent_started_entries(self.journal, &evidence)?;
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
