//! Prototype 1 eval evidence storage port.
#![cfg_attr(not(test), allow(dead_code))]
//!
//! This module is Domain-C only: it records ordinary eval evidence/projections
//! and must not replace History, channel transport, MessageBox authority,
//! invocation/bootstrap, or artifact/worktree mutation.

use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use cozo::DataValue;
use ploke_db::{Database, DbError, QueryResult};
use ploke_records::ids::CampaignId;
use sha2::{Digest, Sha256};
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
    Db(FileDbEvalStore<'a>),
    DualStrict(FileDbEvalStore<'a>),
}

impl<'a> ConfiguredEvalStore<'a> {
    pub(crate) fn fs(journal: &'a mut PrototypeJournal) -> Self {
        Self::Fs(FsEvalStore::new(journal))
    }

    pub(crate) fn database(journal: &'a mut PrototypeJournal, db_path: PathBuf) -> Self {
        Self::Db(FileDbEvalStore::new(
            journal,
            db_path,
            EvalStorageMode::Database,
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
            Self::Db(store) | Self::DualStrict(store) => store.put_parent_started(evidence),
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
enum EvalStorageMode {
    Database,
    DualStrict,
}

impl EvalStorageMode {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Database => "database",
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
    fn new(journal: &'a mut PrototypeJournal, db_path: PathBuf, mode: EvalStorageMode) -> Self {
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

const EVENT_REL: &str = "eval_transition_event";
const RECORD_REL: &str = "eval_record_ref";
const PARENT_STARTED_TRANSITION: &str = "r4c_to_r5";
const PARENT_STARTED_PHASE: &str = "parent_started";
const PARENT_STARTED_OUTCOME: &str = "recorded";
const STORE_SCOPE: &str = "parent";
const PRODUCER_ROLE_PARENT: &str = "parent";
const VISIBILITY_SCOPE: &str = "parent_visible";
const JOURNAL_SOURCE_CLASS: &str = "direct_write";
const TYPED_TRANSITION_CLASS: &str = "typed_transition";
const DIAGNOSTIC_CLASS: &str = "diagnostic";
const VALID_STATUS: &str = "valid";
const JOURNAL_SCHEMA: &str = "prototype1-transition-journal.jsonl";

pub(crate) trait EvalDb {
    fn eval_query_params(
        &self,
        script: &str,
        params: BTreeMap<String, DataValue>,
    ) -> Result<QueryResult, DbError>;

    fn eval_query_mut_params(
        &self,
        script: &str,
        params: BTreeMap<String, DataValue>,
    ) -> Result<QueryResult, DbError>;
}

impl EvalDb for Database {
    fn eval_query_params(
        &self,
        script: &str,
        params: BTreeMap<String, DataValue>,
    ) -> Result<QueryResult, DbError> {
        self.raw_query_params(script, params)
    }

    fn eval_query_mut_params(
        &self,
        script: &str,
        params: BTreeMap<String, DataValue>,
    ) -> Result<QueryResult, DbError> {
        self.raw_query_mut_params(script, params)
    }
}

pub(crate) struct DbEvalStore<'a, D: EvalDb + ?Sized> {
    db: &'a D,
}

impl<'a, D: EvalDb + ?Sized> DbEvalStore<'a, D> {
    pub(crate) fn new(db: &'a D) -> Self {
        Self { db }
    }

    pub(crate) fn install_schema(&self) -> Result<(), EvalStoreError> {
        ensure_eval_store_schema(self.db)
    }

    pub(crate) fn put_parent_started_from_receipt(
        &self,
        evidence: &ParentStartedEvidence,
        receipt: &ParentStartedReceipt,
    ) -> Result<ParentStartedDbReceipt, EvalStoreError> {
        self.install_schema()?;
        let rows = parent_started_rows(evidence, receipt)?;
        if let Some(existing) = existing_transition_semantic_hash(self.db, &rows.event.event_id)? {
            if existing != rows.event.semantic_hash {
                return Err(EvalStoreError::SemanticConflict {
                    event_id: rows.event.event_id,
                    existing_semantic_hash: existing,
                    attempted_semantic_hash: rows.event.semantic_hash,
                });
            }
            verify_parent_started_db_rows(self.db, &rows)?;
            return Ok(rows.receipt);
        }
        put_transition_event_row(self.db, &rows.event)?;
        for record in &rows.records {
            put_record_ref_row(self.db, record)?;
        }
        verify_parent_started_db_rows(self.db, &rows)?;
        Ok(rows.receipt)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParentStartedDbReceipt {
    pub(crate) event_id: String,
    pub(crate) parent_ref_id: String,
    pub(crate) resource_ref_id: String,
    pub(crate) semantic_hash: String,
}

#[derive(Debug, Clone)]
struct ParentStartedRows {
    event: EvalTransitionEventRow,
    records: Vec<EvalRecordRefRow>,
    receipt: ParentStartedDbReceipt,
}

#[derive(Debug, Clone)]
struct EvalTransitionEventRow {
    event_id: String,
    campaign_id: String,
    parent_id: String,
    runtime_id: String,
    node_id: String,
    generation: i64,
    transition: String,
    phase: String,
    outcome: String,
    store_scope: String,
    producer_role: String,
    visibility_scope: String,
    source_class: String,
    evidence_class: String,
    validation_status: String,
    source_stream_id: String,
    source_event_index: i64,
    source_line: i64,
    source_ref: String,
    content_sha256: String,
    semantic_hash: String,
    recorded_at: i64,
    ingested_at: String,
}

#[derive(Debug, Clone)]
struct EvalRecordRefRow {
    record_ref_id: String,
    campaign_id: String,
    family: String,
    schema_version: String,
    store_scope: String,
    producer_role: String,
    producer_id: String,
    source_class: String,
    evidence_class: String,
    visibility_scope: String,
    validation_status: String,
    source_stream_id: String,
    source_event_index: i64,
    source_line: i64,
    source_ref: String,
    content_sha256: String,
    payload_json: String,
    recorded_at: i64,
    ingested_at: String,
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

pub(crate) fn prototype1_eval_store_db_path(campaign_manifest_path: &Path) -> PathBuf {
    campaign_manifest_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("prototype1")
        .join("eval-store.cozo.sqlite")
}

pub(crate) fn load_owner_eval_database(path: &Path) -> Result<Database, EvalStoreError> {
    if path.exists() {
        if !path.is_file() {
            return Err(EvalStoreError::DbSetup {
                phase: "owner_eval_db.restore",
                detail: format!("eval DB path '{}' is not a file", path.display()),
            });
        }
        let db = cozo::new_cozo_mem().map_err(|source| EvalStoreError::DbSetup {
            phase: "owner_eval_db.open_mem",
            detail: source.to_string(),
        })?;
        db.restore_backup(path)
            .map_err(|source| EvalStoreError::DbSetup {
                phase: "owner_eval_db.restore",
                detail: source.to_string(),
            })?;
        Ok(Database::new(db))
    } else {
        Database::new_init().map_err(|source| EvalStoreError::DbSetup {
            phase: "owner_eval_db.new",
            detail: source.to_string(),
        })
    }
}

fn write_parent_started_to_owner_db(
    db_path: &Path,
    evidence: &ParentStartedEvidence,
    receipt: &ParentStartedReceipt,
) -> Result<ParentStartedDbReceipt, EvalStoreError> {
    let db = load_owner_eval_database(db_path)?;
    let store = DbEvalStore::new(&db);
    let db_receipt = store.put_parent_started_from_receipt(evidence, receipt)?;
    persist_owner_eval_database(&db, db_path)?;
    Ok(db_receipt)
}

fn persist_owner_eval_database(db: &Database, path: &Path) -> Result<(), EvalStoreError> {
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    fs::create_dir_all(parent).map_err(|source| EvalStoreError::Io {
        phase: "owner_eval_db.create_dir",
        path: parent.to_path_buf(),
        source,
    })?;
    let temp_path = owner_eval_db_temp_path(path)?;
    if temp_path.exists() {
        fs::remove_file(&temp_path).map_err(|source| EvalStoreError::Io {
            phase: "owner_eval_db.remove_stale_temp",
            path: temp_path.clone(),
            source,
        })?;
    }
    db.write_backup_to_path(&temp_path)
        .map_err(|source| EvalStoreError::Db {
            phase: "owner_eval_db.backup",
            source,
        })?;
    fs::rename(&temp_path, path).map_err(|source| EvalStoreError::Io {
        phase: "owner_eval_db.rename_backup",
        path: path.to_path_buf(),
        source,
    })?;
    Ok(())
}

fn owner_eval_db_temp_path(path: &Path) -> Result<PathBuf, EvalStoreError> {
    let file_name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| EvalStoreError::DbSetup {
            phase: "owner_eval_db.temp_path",
            detail: format!("eval DB path '{}' has no valid file name", path.display()),
        })?;
    Ok(path.with_file_name(format!(".{file_name}.tmp-{}", std::process::id())))
}

fn ensure_eval_store_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    if !eval_relation_exists(db, EVENT_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_transition_event {
    event_id: String =>
    campaign_id: String,
    parent_id: String,
    runtime_id: String,
    node_id: String,
    generation: Int,
    transition: String,
    phase: String,
    outcome: String,
    store_scope: String,
    producer_role: String,
    visibility_scope: String,
    source_class: String,
    evidence_class: String,
    validation_status: String,
    source_stream_id: String,
    source_event_index: Int,
    source_line: Int,
    source_ref: String,
    content_sha256: String,
    semantic_hash: String,
    recorded_at: Int,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_transition_event",
            source,
        })?;
    }

    if !eval_relation_exists(db, RECORD_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_record_ref {
    record_ref_id: String =>
    campaign_id: String,
    family: String,
    schema_version: String,
    store_scope: String,
    producer_role: String,
    producer_id: String,
    source_class: String,
    evidence_class: String,
    visibility_scope: String,
    validation_status: String,
    source_stream_id: String,
    source_event_index: Int,
    source_line: Int,
    source_ref: String,
    content_sha256: String,
    payload_json: String,
    recorded_at: Int,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_record_ref",
            source,
        })?;
    }

    Ok(())
}

fn eval_relation_exists<D: EvalDb + ?Sized>(
    db: &D,
    relation: &str,
) -> Result<bool, EvalStoreError> {
    let result = db
        .eval_query_params("::relations", BTreeMap::new())
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.relations",
            source,
        })?;
    Ok(result.rows.iter().any(|row| {
        row.first().and_then(|value| match value {
            DataValue::Str(value) => Some(value.as_str()),
            _ => None,
        }) == Some(relation)
    }))
}

fn existing_transition_semantic_hash<D: EvalDb + ?Sized>(
    db: &D,
    event_id: &str,
) -> Result<Option<String>, EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert(
        "event_id".to_string(),
        DataValue::from(event_id.to_string()),
    );
    let result = db
        .eval_query_params(
            r#"
?[semantic_hash] :=
    *eval_transition_event { event_id, semantic_hash },
    event_id = $event_id
"#,
            params,
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "query.eval_transition_event.semantic_hash",
            source,
        })?;
    Ok(result.rows.first().and_then(|row| match row.first() {
        Some(DataValue::Str(value)) => Some(value.to_string()),
        _ => None,
    }))
}

fn verify_parent_started_db_rows<D: EvalDb + ?Sized>(
    db: &D,
    rows: &ParentStartedRows,
) -> Result<(), EvalStoreError> {
    let Some(actual) = existing_transition_semantic_hash(db, &rows.event.event_id)? else {
        return Err(EvalStoreError::Validation {
            field: "eval_transition_event.semantic_hash",
            detail: format!(
                "missing transition event row '{}' after parent-start DB write",
                rows.event.event_id
            ),
        });
    };
    if actual != rows.event.semantic_hash {
        return Err(EvalStoreError::SemanticConflict {
            event_id: rows.event.event_id.clone(),
            existing_semantic_hash: actual,
            attempted_semantic_hash: rows.event.semantic_hash.clone(),
        });
    }
    for record in &rows.records {
        if !record_ref_exists(db, &record.record_ref_id, &record.content_sha256)? {
            return Err(EvalStoreError::Validation {
                field: "eval_record_ref.content_sha256",
                detail: format!(
                    "missing record ref '{}' with expected content hash after parent-start DB write",
                    record.record_ref_id
                ),
            });
        }
    }
    Ok(())
}

fn record_ref_exists<D: EvalDb + ?Sized>(
    db: &D,
    record_ref_id: &str,
    content_sha256: &str,
) -> Result<bool, EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert(
        "record_ref_id".to_string(),
        DataValue::from(record_ref_id.to_string()),
    );
    params.insert(
        "content_sha256".to_string(),
        DataValue::from(content_sha256.to_string()),
    );
    let result = db
        .eval_query_params(
            r#"
?[record_ref_id] :=
    *eval_record_ref { record_ref_id, content_sha256 },
    record_ref_id = $record_ref_id,
    content_sha256 = $content_sha256
"#,
            params,
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "query.eval_record_ref.content_hash",
            source,
        })?;
    Ok(!result.rows.is_empty())
}

fn put_transition_event_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalTransitionEventRow,
) -> Result<(), EvalStoreError> {
    db.eval_query_mut_params(
        r#"
?[
    event_id,
    campaign_id,
    parent_id,
    runtime_id,
    node_id,
    generation,
    transition,
    phase,
    outcome,
    store_scope,
    producer_role,
    visibility_scope,
    source_class,
    evidence_class,
    validation_status,
    source_stream_id,
    source_event_index,
    source_line,
    source_ref,
    content_sha256,
    semantic_hash,
    recorded_at,
    ingested_at
] :=
    event_id = $event_id,
    campaign_id = $campaign_id,
    parent_id = $parent_id,
    runtime_id = $runtime_id,
    node_id = $node_id,
    generation = $generation,
    transition = $transition,
    phase = $phase,
    outcome = $outcome,
    store_scope = $store_scope,
    producer_role = $producer_role,
    visibility_scope = $visibility_scope,
    source_class = $source_class,
    evidence_class = $evidence_class,
    validation_status = $validation_status,
    source_stream_id = $source_stream_id,
    source_event_index = $source_event_index,
    source_line = $source_line,
    source_ref = $source_ref,
    content_sha256 = $content_sha256,
    semantic_hash = $semantic_hash,
    recorded_at = $recorded_at,
    ingested_at = $ingested_at
:put eval_transition_event {
    event_id =>
    campaign_id,
    parent_id,
    runtime_id,
    node_id,
    generation,
    transition,
    phase,
    outcome,
    store_scope,
    producer_role,
    visibility_scope,
    source_class,
    evidence_class,
    validation_status,
    source_stream_id,
    source_event_index,
    source_line,
    source_ref,
    content_sha256,
    semantic_hash,
    recorded_at,
    ingested_at
}
"#,
        transition_event_params(row),
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_transition_event",
        source,
    })?;
    Ok(())
}

fn put_record_ref_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalRecordRefRow,
) -> Result<(), EvalStoreError> {
    db.eval_query_mut_params(
        r#"
?[
    record_ref_id,
    campaign_id,
    family,
    schema_version,
    store_scope,
    producer_role,
    producer_id,
    source_class,
    evidence_class,
    visibility_scope,
    validation_status,
    source_stream_id,
    source_event_index,
    source_line,
    source_ref,
    content_sha256,
    payload_json,
    recorded_at,
    ingested_at
] :=
    record_ref_id = $record_ref_id,
    campaign_id = $campaign_id,
    family = $family,
    schema_version = $schema_version,
    store_scope = $store_scope,
    producer_role = $producer_role,
    producer_id = $producer_id,
    source_class = $source_class,
    evidence_class = $evidence_class,
    visibility_scope = $visibility_scope,
    validation_status = $validation_status,
    source_stream_id = $source_stream_id,
    source_event_index = $source_event_index,
    source_line = $source_line,
    source_ref = $source_ref,
    content_sha256 = $content_sha256,
    payload_json = $payload_json,
    recorded_at = $recorded_at,
    ingested_at = $ingested_at
:put eval_record_ref {
    record_ref_id =>
    campaign_id,
    family,
    schema_version,
    store_scope,
    producer_role,
    producer_id,
    source_class,
    evidence_class,
    visibility_scope,
    validation_status,
    source_stream_id,
    source_event_index,
    source_line,
    source_ref,
    content_sha256,
    payload_json,
    recorded_at,
    ingested_at
}
"#,
        record_ref_params(row),
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_record_ref",
        source,
    })?;
    Ok(())
}

fn parent_started_db_receipt(
    evidence: &ParentStartedEvidence,
    receipt: &ParentStartedReceipt,
) -> Result<ParentStartedDbReceipt, EvalStoreError> {
    Ok(parent_started_rows(evidence, receipt)?.receipt)
}

fn parent_started_rows(
    evidence: &ParentStartedEvidence,
    receipt: &ParentStartedReceipt,
) -> Result<ParentStartedRows, EvalStoreError> {
    validate_parent_started_receipt(receipt)?;
    let stream = receipt.parent.path.display().to_string();
    let parent_index = usize_to_i64(
        receipt.parent.source_event_index,
        "parent.source_event_index",
    )?;
    let parent_line = usize_to_i64(receipt.parent.source_line, "parent.source_line")?;
    let resource_index = usize_to_i64(
        receipt.resource.source_event_index,
        "resource.source_event_index",
    )?;
    let resource_line = usize_to_i64(receipt.resource.source_line, "resource.source_line")?;
    let runtime_id = evidence
        .handoff_runtime_id
        .map(|id| id.to_string())
        .unwrap_or_default();
    let ingested_at = chrono::Utc::now().to_rfc3339();
    let source_stream_id = source_stream_id(&evidence.campaign_id, &stream);
    let content_sha256 = hash_parts(&[
        "p1.eval.parent_started.content.v1",
        &receipt.parent.content_sha256,
        &receipt.resource.content_sha256,
    ]);
    let semantic_hash = hash_parts(&[
        "p1.eval.parent_started.semantic.v1",
        evidence.campaign_id.as_str(),
        evidence.parent_identity.parent_id(),
        evidence.parent_identity.node_id(),
        &evidence.parent_identity.generation().to_string(),
        evidence.parent_identity.branch_id(),
        &evidence.repo_root.display().to_string(),
        &runtime_id,
        &evidence.pid.to_string(),
        &evidence.parent_recorded_at.0.to_string(),
        &evidence.resource_recorded_at.0.to_string(),
        &receipt.parent.payload_json,
        &receipt.resource.payload_json,
    ]);
    let event_id = hash_parts(&[
        "p1.eval.transition_event.v1",
        evidence.campaign_id.as_str(),
        evidence.parent_identity.parent_id(),
        PARENT_STARTED_TRANSITION,
        &source_stream_id,
        &parent_index.to_string(),
        &receipt.parent.content_sha256,
    ]);
    let parent_ref_id = record_ref_id(
        evidence.campaign_id.as_str(),
        "parent_started",
        &source_stream_id,
        parent_index,
        &receipt.parent.content_sha256,
    );
    let resource_ref_id = record_ref_id(
        evidence.campaign_id.as_str(),
        "resource_parent_start",
        &source_stream_id,
        resource_index,
        &receipt.resource.content_sha256,
    );
    let event = EvalTransitionEventRow {
        event_id: event_id.clone(),
        campaign_id: evidence.campaign_id.to_string(),
        parent_id: evidence.parent_identity.parent_id().to_string(),
        runtime_id,
        node_id: evidence.parent_identity.node_id().to_string(),
        generation: i64::from(evidence.parent_identity.generation()),
        transition: PARENT_STARTED_TRANSITION.to_string(),
        phase: PARENT_STARTED_PHASE.to_string(),
        outcome: PARENT_STARTED_OUTCOME.to_string(),
        store_scope: STORE_SCOPE.to_string(),
        producer_role: PRODUCER_ROLE_PARENT.to_string(),
        visibility_scope: VISIBILITY_SCOPE.to_string(),
        source_class: JOURNAL_SOURCE_CLASS.to_string(),
        evidence_class: TYPED_TRANSITION_CLASS.to_string(),
        validation_status: VALID_STATUS.to_string(),
        source_stream_id: source_stream_id.clone(),
        source_event_index: parent_index,
        source_line: parent_line,
        source_ref: event_source_ref(&stream, parent_line, resource_line),
        content_sha256: content_sha256.clone(),
        semantic_hash: semantic_hash.clone(),
        recorded_at: evidence.parent_recorded_at.0,
        ingested_at: ingested_at.clone(),
    };
    let parent_record = record_ref_row(
        parent_ref_id.clone(),
        evidence,
        "parent_started",
        TYPED_TRANSITION_CLASS,
        &source_stream_id,
        &stream,
        parent_index,
        parent_line,
        &receipt.parent,
        evidence.parent_recorded_at.0,
        &ingested_at,
    );
    let resource_record = record_ref_row(
        resource_ref_id.clone(),
        evidence,
        "resource_parent_start",
        DIAGNOSTIC_CLASS,
        &source_stream_id,
        &stream,
        resource_index,
        resource_line,
        &receipt.resource,
        evidence.resource_recorded_at.0,
        &ingested_at,
    );
    let receipt = ParentStartedDbReceipt {
        event_id,
        parent_ref_id,
        resource_ref_id,
        semantic_hash,
    };
    Ok(ParentStartedRows {
        event,
        records: vec![parent_record, resource_record],
        receipt,
    })
}

fn record_ref_row(
    record_ref_id: String,
    evidence: &ParentStartedEvidence,
    family: &str,
    evidence_class: &str,
    source_stream_id: &str,
    source_path: &str,
    index: i64,
    line: i64,
    receipt: &JournalAppendReceipt,
    recorded_at: i64,
    ingested_at: &str,
) -> EvalRecordRefRow {
    EvalRecordRefRow {
        record_ref_id,
        campaign_id: evidence.campaign_id.to_string(),
        family: family.to_string(),
        schema_version: JOURNAL_SCHEMA.to_string(),
        store_scope: STORE_SCOPE.to_string(),
        producer_role: PRODUCER_ROLE_PARENT.to_string(),
        producer_id: evidence.parent_identity.parent_id().to_string(),
        source_class: JOURNAL_SOURCE_CLASS.to_string(),
        evidence_class: evidence_class.to_string(),
        visibility_scope: VISIBILITY_SCOPE.to_string(),
        validation_status: VALID_STATUS.to_string(),
        source_stream_id: source_stream_id.to_string(),
        source_event_index: index,
        source_line: line,
        source_ref: source_ref(source_path, line),
        content_sha256: receipt.content_sha256.clone(),
        payload_json: receipt.payload_json.clone(),
        recorded_at,
        ingested_at: ingested_at.to_string(),
    }
}

fn validate_parent_started_receipt(receipt: &ParentStartedReceipt) -> Result<(), EvalStoreError> {
    let stream = receipt.parent.path.display().to_string();
    require_non_empty("source_stream_id", &stream)?;
    if receipt.parent.path != receipt.resource.path {
        return Err(EvalStoreError::Validation {
            field: "source_stream_id",
            detail: "parent and resource receipts must reference the same journal path".to_string(),
        });
    }
    require_non_empty("parent.content_sha256", &receipt.parent.content_sha256)?;
    require_non_empty("resource.content_sha256", &receipt.resource.content_sha256)?;
    require_non_empty("parent.payload_json", &receipt.parent.payload_json)?;
    require_non_empty("resource.payload_json", &receipt.resource.payload_json)?;
    if receipt.parent.source_line == 0 || receipt.resource.source_line == 0 {
        return Err(EvalStoreError::Validation {
            field: "source_line",
            detail: "journal source lines are one-based and must be non-zero".to_string(),
        });
    }
    Ok(())
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), EvalStoreError> {
    if value.is_empty() {
        return Err(EvalStoreError::Validation {
            field,
            detail: "required eval-store field is empty".to_string(),
        });
    }
    Ok(())
}

fn usize_to_i64(value: usize, field: &'static str) -> Result<i64, EvalStoreError> {
    i64::try_from(value).map_err(|_| EvalStoreError::Validation {
        field,
        detail: format!("value {value} does not fit in Cozo Int"),
    })
}

fn source_ref(stream: &str, line: i64) -> String {
    format!("{stream}:L{line}")
}

fn event_source_ref(stream: &str, parent_line: i64, resource_line: i64) -> String {
    format!("{stream}:L{parent_line}-L{resource_line}")
}

fn source_stream_id(campaign_id: &CampaignId, stream: &str) -> String {
    format!("prototype1-transition-journal:{campaign_id}:{stream}")
}

fn record_ref_id(
    campaign_id: &str,
    family: &str,
    source_stream_id: &str,
    index: i64,
    content_sha256: &str,
) -> String {
    hash_parts(&[
        "p1.eval.record_ref.v1",
        campaign_id,
        family,
        source_stream_id,
        &index.to_string(),
        content_sha256,
    ])
}

fn hash_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    hex_lower(&hasher.finalize())
}

fn hex_lower(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}

fn transition_event_params(row: &EvalTransitionEventRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("event_id".to_string(), row.event_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("parent_id".to_string(), row.parent_id.clone().into());
    params.insert("runtime_id".to_string(), row.runtime_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("generation".to_string(), row.generation.into());
    params.insert("transition".to_string(), row.transition.clone().into());
    params.insert("phase".to_string(), row.phase.clone().into());
    params.insert("outcome".to_string(), row.outcome.clone().into());
    params.insert("store_scope".to_string(), row.store_scope.clone().into());
    params.insert(
        "producer_role".to_string(),
        row.producer_role.clone().into(),
    );
    params.insert(
        "visibility_scope".to_string(),
        row.visibility_scope.clone().into(),
    );
    params.insert("source_class".to_string(), row.source_class.clone().into());
    params.insert(
        "evidence_class".to_string(),
        row.evidence_class.clone().into(),
    );
    params.insert(
        "validation_status".to_string(),
        row.validation_status.clone().into(),
    );
    params.insert(
        "source_stream_id".to_string(),
        row.source_stream_id.clone().into(),
    );
    params.insert(
        "source_event_index".to_string(),
        row.source_event_index.into(),
    );
    params.insert("source_line".to_string(), row.source_line.into());
    params.insert("source_ref".to_string(), row.source_ref.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert(
        "semantic_hash".to_string(),
        row.semantic_hash.clone().into(),
    );
    params.insert("recorded_at".to_string(), row.recorded_at.into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

fn record_ref_params(row: &EvalRecordRefRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "record_ref_id".to_string(),
        row.record_ref_id.clone().into(),
    );
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("family".to_string(), row.family.clone().into());
    params.insert(
        "schema_version".to_string(),
        row.schema_version.clone().into(),
    );
    params.insert("store_scope".to_string(), row.store_scope.clone().into());
    params.insert(
        "producer_role".to_string(),
        row.producer_role.clone().into(),
    );
    params.insert("producer_id".to_string(), row.producer_id.clone().into());
    params.insert("source_class".to_string(), row.source_class.clone().into());
    params.insert(
        "evidence_class".to_string(),
        row.evidence_class.clone().into(),
    );
    params.insert(
        "visibility_scope".to_string(),
        row.visibility_scope.clone().into(),
    );
    params.insert(
        "validation_status".to_string(),
        row.validation_status.clone().into(),
    );
    params.insert(
        "source_stream_id".to_string(),
        row.source_stream_id.clone().into(),
    );
    params.insert(
        "source_event_index".to_string(),
        row.source_event_index.into(),
    );
    params.insert("source_line".to_string(), row.source_line.into());
    params.insert("source_ref".to_string(), row.source_ref.clone().into());
    params.insert(
        "content_sha256".to_string(),
        row.content_sha256.clone().into(),
    );
    params.insert("payload_json".to_string(), row.payload_json.clone().into());
    params.insert("recorded_at".to_string(), row.recorded_at.into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

#[derive(Debug, Error)]
pub(crate) enum EvalStoreError {
    #[error("failed to append {phase} evidence to transition journal: {source}")]
    Journal {
        phase: &'static str,
        source: PrototypeJournalError,
    },
    #[error("eval-store db operation {phase} failed: {source}")]
    Db {
        phase: &'static str,
        source: DbError,
    },
    #[error("eval-store db setup {phase} failed: {detail}")]
    DbSetup { phase: &'static str, detail: String },
    #[error("eval-store filesystem operation {phase} failed for {path:?}: {source}")]
    Io {
        phase: &'static str,
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("eval-store validation failed for {field}: {detail}")]
    Validation { field: &'static str, detail: String },
    #[error(
        "semantic hash mismatch for eval transition event '{event_id}': existing {existing_semantic_hash}, attempted {attempted_semantic_hash}"
    )]
    SemanticConflict {
        event_id: String,
        existing_semantic_hash: String,
        attempted_semantic_hash: String,
    },
    #[error(
        "eval-store {backend} DB write failed after filesystem parent-start append: journal_path={journal_path}, parent_started_source_event_index={parent_started_source_event_index}, resource_source_event_index={resource_source_event_index}, parent_started_content_sha256={parent_started_content_sha256}, resource_content_sha256={resource_content_sha256}, expected_semantic_hash={expected_semantic_hash:?}, db_error_or_mismatch={db_error_or_mismatch}, suggested_recovery={suggested_recovery}"
    )]
    PostFsDb {
        backend: &'static str,
        journal_path: String,
        parent_started_source_event_index: usize,
        resource_source_event_index: usize,
        parent_started_content_sha256: String,
        resource_content_sha256: String,
        expected_semantic_hash: Option<String>,
        db_error_or_mismatch: String,
        suggested_recovery: &'static str,
    },
}

impl EvalStoreError {
    fn post_fs_db(
        mode: EvalStorageMode,
        receipt: &ParentStartedReceipt,
        expected_semantic_hash: Option<String>,
        db_error_or_mismatch: String,
    ) -> Self {
        Self::PostFsDb {
            backend: mode.as_str(),
            journal_path: receipt.parent.path.display().to_string(),
            parent_started_source_event_index: receipt.parent.source_event_index,
            resource_source_event_index: receipt.resource.source_event_index,
            parent_started_content_sha256: receipt.parent.content_sha256.clone(),
            resource_content_sha256: receipt.resource.content_sha256.clone(),
            expected_semantic_hash,
            db_error_or_mismatch,
            suggested_recovery: "re-run deterministic import for these source indices",
        }
    }
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

    #[test]
    fn prototype1_eval_store_parent_start_db_schema_installs_idempotently() {
        let db = Database::new_init().expect("db");
        let store = DbEvalStore::new(&db);

        store.install_schema().expect("schema install");
        store
            .install_schema()
            .expect("schema install is idempotent");

        assert!(eval_relation_exists(&db, EVENT_REL).expect("event rel exists"));
        assert!(eval_relation_exists(&db, RECORD_REL).expect("record rel exists"));
    }

    #[test]
    fn prototype1_eval_store_parent_start_db_round_trips_rows() {
        let tmp = tempfile::tempdir().expect("tmp");
        let (evidence, receipt) = parent_started_fixture(tmp.path());
        let db = Database::new_init().expect("db");
        let store = DbEvalStore::new(&db);

        let db_receipt = store
            .put_parent_started_from_receipt(&evidence, &receipt)
            .expect("db parent start write");

        let event = query_transition_event(&db, &db_receipt.event_id);
        assert_eq!(event.rows.len(), 1);
        let row = event.row_refs().next().expect("event row");
        assert_eq!(
            row.get::<String>("campaign_id").expect("campaign"),
            "campaign"
        );
        assert_eq!(row.get::<String>("parent_id").expect("parent"), "parent");
        assert_eq!(row.get::<String>("node_id").expect("node"), "parent");
        assert_eq!(row.get::<i64>("generation").expect("generation"), 0);
        assert_eq!(
            row.get::<String>("transition").expect("transition"),
            PARENT_STARTED_TRANSITION
        );
        assert_eq!(
            row.get::<String>("phase").expect("phase"),
            PARENT_STARTED_PHASE
        );
        assert_eq!(
            row.get::<String>("outcome").expect("outcome"),
            PARENT_STARTED_OUTCOME
        );
        assert_eq!(
            row.get::<i64>("source_event_index").expect("event index"),
            receipt.parent.source_event_index as i64
        );
        assert_eq!(
            row.get::<i64>("source_line").expect("source line"),
            receipt.parent.source_line as i64
        );
        assert_eq!(
            row.get::<String>("semantic_hash").expect("semantic hash"),
            db_receipt.semantic_hash
        );

        let records = query_record_refs(&db, &evidence.campaign_id);
        assert_eq!(records.rows.len(), 2);
        let mut families = std::collections::BTreeMap::new();
        for row in records.row_refs() {
            families.insert(
                row.get::<String>("family").expect("family"),
                (
                    row.get::<i64>("source_event_index").expect("index"),
                    row.get::<String>("content_sha256").expect("hash"),
                    row.get::<String>("payload_json").expect("payload"),
                ),
            );
        }
        assert_eq!(
            families.get("parent_started").expect("parent ref").0,
            receipt.parent.source_event_index as i64
        );
        assert_eq!(
            families.get("parent_started").expect("parent ref").1,
            receipt.parent.content_sha256
        );
        assert_eq!(
            families
                .get("resource_parent_start")
                .expect("resource ref")
                .0,
            receipt.resource.source_event_index as i64
        );
        assert_eq!(
            families
                .get("resource_parent_start")
                .expect("resource ref")
                .1,
            receipt.resource.content_sha256
        );
    }

    #[test]
    fn prototype1_eval_store_parent_start_db_duplicate_identical_is_idempotent() {
        let tmp = tempfile::tempdir().expect("tmp");
        let (evidence, receipt) = parent_started_fixture(tmp.path());
        let db = Database::new_init().expect("db");
        let store = DbEvalStore::new(&db);

        let first = store
            .put_parent_started_from_receipt(&evidence, &receipt)
            .expect("first db write");
        let second = store
            .put_parent_started_from_receipt(&evidence, &receipt)
            .expect("second identical write");

        assert_eq!(second, first);
        assert_eq!(query_transition_event(&db, &first.event_id).rows.len(), 1);
        assert_eq!(query_record_refs(&db, &evidence.campaign_id).rows.len(), 2);
    }

    #[test]
    fn prototype1_eval_store_parent_start_db_duplicate_semantic_mismatch_fails() {
        let tmp = tempfile::tempdir().expect("tmp");
        let (evidence, receipt) = parent_started_fixture(tmp.path());
        let db = Database::new_init().expect("db");
        let store = DbEvalStore::new(&db);
        let first = store
            .put_parent_started_from_receipt(&evidence, &receipt)
            .expect("first db write");
        let mut changed_evidence = evidence.clone();
        changed_evidence.pid = evidence.pid + 1;

        let err = store
            .put_parent_started_from_receipt(&changed_evidence, &receipt)
            .expect_err("semantic mismatch fails");

        match err {
            EvalStoreError::SemanticConflict {
                event_id,
                existing_semantic_hash,
                attempted_semantic_hash,
            } => {
                assert_eq!(event_id, first.event_id);
                assert_eq!(existing_semantic_hash, first.semantic_hash);
                assert_ne!(attempted_semantic_hash, first.semantic_hash);
            }
            other => panic!("unexpected mismatch error: {other:?}"),
        }
        assert_eq!(query_transition_event(&db, &first.event_id).rows.len(), 1);
        assert_eq!(query_record_refs(&db, &evidence.campaign_id).rows.len(), 2);
    }

    #[test]
    fn prototype1_eval_store_parent_start_db_missing_required_hash_fails_before_rows() {
        let tmp = tempfile::tempdir().expect("tmp");
        let (evidence, mut receipt) = parent_started_fixture(tmp.path());
        let db = Database::new_init().expect("db");
        let store = DbEvalStore::new(&db);
        receipt.parent.content_sha256.clear();

        let err = store
            .put_parent_started_from_receipt(&evidence, &receipt)
            .expect_err("missing hash fails");

        match err {
            EvalStoreError::Validation { field, detail } => {
                assert_eq!(field, "parent.content_sha256");
                assert!(detail.contains("required eval-store field"));
            }
            other => panic!("unexpected validation error: {other:?}"),
        }
        assert!(query_all_transition_events(&db).rows.is_empty());
        assert!(
            query_record_refs(&db, &evidence.campaign_id)
                .rows
                .is_empty()
        );
    }

    #[test]
    fn prototype1_eval_store_parent_start_dual_strict_persists_owner_db() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).expect("repo dir");
        let journal_path = tmp.path().join("prototype1/transition-journal.jsonl");
        let db_path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        let mut journal = PrototypeJournal::new(&journal_path);
        let evidence = parent_started_evidence(repo);
        let mut store =
            FileDbEvalStore::new(&mut journal, db_path.clone(), EvalStorageMode::DualStrict);

        let receipt = store
            .put_parent_started(evidence.clone())
            .expect("dual-strict parent start writes");

        assert!(db_path.is_file());
        let db = load_owner_eval_database(&db_path).expect("owner eval db loads");
        let expected = parent_started_db_receipt(&evidence, &receipt).expect("expected receipt");
        assert_eq!(
            query_transition_event(&db, &expected.event_id).rows.len(),
            1
        );
        assert_eq!(query_record_refs(&db, &evidence.campaign_id).rows.len(), 2);
    }

    #[test]
    fn prototype1_eval_store_parent_start_dual_strict_failure_keeps_repairable_journal() {
        let tmp = tempfile::tempdir().expect("tmp");
        let repo = tmp.path().join("repo");
        fs::create_dir_all(&repo).expect("repo dir");
        let journal_path = tmp.path().join("prototype1/transition-journal.jsonl");
        let db_path = tmp.path().join("prototype1/eval-store.cozo.sqlite");
        fs::create_dir_all(&db_path).expect("poison db path as directory");
        let mut journal = PrototypeJournal::new(&journal_path);
        let evidence = parent_started_evidence(repo);
        let mut store =
            FileDbEvalStore::new(&mut journal, db_path.clone(), EvalStorageMode::DualStrict);

        let err = store
            .put_parent_started(evidence.clone())
            .expect_err("db failure after fs append fails loudly");
        let receipt = receipt_from_journal(&journal_path);
        let expected = parent_started_db_receipt(&evidence, &receipt).expect("expected receipt");
        match err {
            EvalStoreError::PostFsDb {
                backend,
                journal_path: err_journal_path,
                parent_started_source_event_index,
                resource_source_event_index,
                parent_started_content_sha256,
                resource_content_sha256,
                expected_semantic_hash,
                suggested_recovery,
                ..
            } => {
                assert_eq!(backend, "dual-strict");
                assert_eq!(err_journal_path, journal_path.display().to_string());
                assert_eq!(parent_started_source_event_index, 0);
                assert_eq!(resource_source_event_index, 1);
                assert_eq!(parent_started_content_sha256, receipt.parent.content_sha256);
                assert_eq!(resource_content_sha256, receipt.resource.content_sha256);
                assert_eq!(expected_semantic_hash, Some(expected.semantic_hash.clone()));
                assert_eq!(
                    suggested_recovery,
                    "re-run deterministic import for these source indices"
                );
            }
            other => panic!("unexpected dual-strict failure: {other:?}"),
        }

        let db = Database::new_init().expect("repair db");
        let repaired = DbEvalStore::new(&db)
            .put_parent_started_from_receipt(&evidence, &receipt)
            .expect("deterministic repair import");
        assert_eq!(repaired.semantic_hash, expected.semantic_hash);
        assert_eq!(
            query_transition_event(&db, &repaired.event_id).rows.len(),
            1
        );
        assert_eq!(query_record_refs(&db, &evidence.campaign_id).rows.len(), 2);
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

    fn parent_started_fixture(
        root: &std::path::Path,
    ) -> (ParentStartedEvidence, ParentStartedReceipt) {
        let repo = root.join("repo");
        fs::create_dir_all(&repo).expect("repo dir");
        let path = root.join("transition-journal.jsonl");
        let mut journal = PrototypeJournal::new(&path);
        let evidence = parent_started_evidence(repo);
        let mut store = FsEvalStore::new(&mut journal);
        let receipt = store
            .put_parent_started(evidence.clone())
            .expect("fs parent start write");
        (evidence, receipt)
    }

    fn receipt_from_journal(path: &std::path::Path) -> ParentStartedReceipt {
        let text = fs::read_to_string(path).expect("journal text");
        let mut offset = 0_u64;
        let mut receipts = Vec::new();
        for (index, line) in text.lines().enumerate() {
            let byte_len = line.len();
            receipts.push(JournalAppendReceipt {
                path: path.to_path_buf(),
                source_event_index: index,
                source_line: index + 1,
                byte_start: offset,
                byte_len,
                content_sha256: sha256_for_test(line.as_bytes()),
                payload_json: line.to_string(),
            });
            offset += byte_len as u64 + 1;
        }
        assert_eq!(receipts.len(), 2);
        ParentStartedReceipt {
            parent: receipts.remove(0),
            resource: receipts.remove(0),
        }
    }

    fn sha256_for_test(bytes: &[u8]) -> String {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        hex_lower(&hasher.finalize())
    }

    fn query_transition_event(db: &Database, event_id: &str) -> QueryResult {
        let mut params = BTreeMap::new();
        params.insert(
            "event_id".to_string(),
            DataValue::from(event_id.to_string()),
        );
        db.raw_query_params(
            r#"
?[event_id, campaign_id, parent_id, node_id, generation, transition, phase, outcome, source_event_index, source_line, content_sha256, semantic_hash] :=
    *eval_transition_event {
        event_id,
        campaign_id,
        parent_id,
        node_id,
        generation,
        transition,
        phase,
        outcome,
        source_event_index,
        source_line,
        content_sha256,
        semantic_hash
    },
    event_id = $event_id
"#,
            params,
        )
        .expect("query transition event")
    }

    fn query_all_transition_events(db: &Database) -> QueryResult {
        db.raw_query_params(
            r#"
?[event_id] :=
    *eval_transition_event { event_id }
"#,
            BTreeMap::new(),
        )
        .expect("query all transition events")
    }

    fn query_record_refs(db: &Database, campaign_id: &CampaignId) -> QueryResult {
        let mut params = BTreeMap::new();
        params.insert(
            "campaign_id".to_string(),
            DataValue::from(campaign_id.to_string()),
        );
        db.raw_query_params(
            r#"
?[record_ref_id, family, source_event_index, source_line, content_sha256, payload_json] :=
    *eval_record_ref {
        record_ref_id,
        campaign_id,
        family,
        source_event_index,
        source_line,
        content_sha256,
        payload_json
    },
    campaign_id = $campaign_id
"#,
            params,
        )
        .expect("query record refs")
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
