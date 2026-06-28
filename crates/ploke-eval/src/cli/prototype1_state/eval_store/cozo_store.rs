use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock},
};

use cozo::DataValue;
use ploke_db::{Database, DbError, QueryResult};

use crate::{
    CampaignManifest,
    cli::prototype1_state::{
        identity::ParentIdentity,
        profile::{AdmittedRunProfile, EvalStorageBackend},
    },
    closure::ClosureState,
    intervention::CompleteBaseline,
};

use super::{
    cozo_params::{
        attempt_params, channel_message_params, channel_receipt_params, import_event_params,
        invocation_params, log_ref_params, record_ref_params, trace_event_params,
        transition_event_params,
    },
    cozo_schema::{
        AttemptSchema, ChannelMessageSchema, ChannelReceiptSchema, ImportEventSchema,
        InvocationSchema, LogRefSchema, RecordRefSchema, TraceEventSchema, TransitionEventSchema,
        ensure_eval_store_schema,
    },
    error::EvalStoreError,
    evidence::{
        ChannelMessageEvidence, ChannelMessageReceipt, ChannelReceiptEvidence,
        ChannelReceiptReceipt, EvalAttemptRow, EvalChannelMessageRow, EvalChannelReceiptRow,
        EvalImportEventRow, EvalInvocationRow, EvalLogRefRow, EvalRecordRefRow, EvalTraceEventRow,
        EvalTransitionEventRow, ImportEventEvidence, ImportEventReceipt, LogRefEvidence,
        LogRefReceipt, ParentStartedDbReceipt, ParentStartedEvidence, ParentStartedReceipt,
        ParentStartedRows, RecordRefEvidence, RecordRefReceipt, TraceEventEvidence,
        TraceEventReceipt, TraceImportReceipt, attempt_row_from_invocation, channel_message_row,
        channel_receipt_row, import_event_row, invocation_row, log_ref_row, parent_started_rows,
        record_ref_row_from_evidence, trace_event_row,
    },
    observation::{ObservationJsonlImport, parse_observation_jsonl},
    schema::put_eval_params,
    setup,
};

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

    pub(crate) fn put_r0_context(
        &self,
        manifest_path: &Path,
        manifest: &CampaignManifest,
        storage_backend: EvalStorageBackend,
        admitted_profile: Option<&AdmittedRunProfile>,
        closure_path: &Path,
        closure_state: &ClosureState,
    ) -> Result<(), EvalStoreError> {
        self.install_schema()?;
        let profile_ref_id = admitted_profile
            .map(|admitted| setup::put_profile_commitment(self.db, &manifest.campaign_id, admitted))
            .transpose()?;
        setup::put_campaign_manifest(
            self.db,
            manifest_path,
            manifest,
            storage_backend,
            profile_ref_id.as_deref(),
        )?;
        if let (Some(admitted), Some(profile_ref_id)) =
            (admitted_profile, profile_ref_id.as_deref())
        {
            setup::put_run_profile_policy(
                self.db,
                &manifest.campaign_id,
                profile_ref_id,
                admitted,
            )?;
        }
        setup::put_closure_ref(self.db, closure_path, closure_state)?;
        Ok(())
    }

    pub(crate) fn put_baseline(
        &self,
        parent: &ParentIdentity,
        baseline: &CompleteBaseline,
        closure: Option<(&Path, &ClosureState)>,
        evaluation_id: Option<&str>,
        record_ref: Option<&str>,
        recorded_at: String,
    ) -> Result<String, EvalStoreError> {
        self.install_schema()?;
        let closure_ref_id = closure
            .map(|(path, state)| setup::put_closure_ref(self.db, path, state))
            .transpose()?;
        setup::put_baseline(
            self.db,
            parent,
            baseline,
            closure_ref_id.as_deref(),
            evaluation_id,
            record_ref,
            recorded_at,
        )
    }

    pub(crate) fn put_closure_state(
        &self,
        closure_path: &Path,
        closure_state: &ClosureState,
    ) -> Result<String, EvalStoreError> {
        self.install_schema()?;
        setup::put_closure_ref(self.db, closure_path, closure_state)
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

    pub(crate) fn put_log_ref(
        &self,
        evidence: LogRefEvidence,
    ) -> Result<LogRefReceipt, EvalStoreError> {
        self.install_schema()?;
        let row = log_ref_row(evidence)?;
        put_log_ref_row(self.db, &row)?;
        Ok(LogRefReceipt {
            log_ref_id: row.log_ref_id,
        })
    }

    pub(crate) fn put_invocation(
        &self,
        evidence: super::evidence::InvocationEvidence,
    ) -> Result<super::evidence::InvocationReceipt, EvalStoreError> {
        self.install_schema()?;
        let row = invocation_row(evidence)?;
        let attempt = attempt_row_from_invocation(&row);
        put_invocation_row(self.db, &row)?;
        put_attempt_row(self.db, &attempt)?;
        Ok(super::evidence::InvocationReceipt {
            invocation_id: row.invocation_id,
            content_sha256: row.content_sha256,
        })
    }

    pub(crate) fn put_channel_message(
        &self,
        evidence: ChannelMessageEvidence,
    ) -> Result<ChannelMessageReceipt, EvalStoreError> {
        self.install_schema()?;
        let row = channel_message_row(evidence)?;
        put_channel_message_row(self.db, &row)?;
        Ok(ChannelMessageReceipt {
            channel_message_id: row.channel_message_id,
            content_sha256: row.content_sha256,
        })
    }

    pub(crate) fn put_channel_receipt(
        &self,
        evidence: ChannelReceiptEvidence,
    ) -> Result<ChannelReceiptReceipt, EvalStoreError> {
        self.install_schema()?;
        let row = channel_receipt_row(evidence)?;
        put_channel_receipt_row(self.db, &row)?;
        Ok(ChannelReceiptReceipt {
            receipt_id: row.receipt_id,
        })
    }

    pub(crate) fn put_import_event(
        &self,
        evidence: ImportEventEvidence,
    ) -> Result<ImportEventReceipt, EvalStoreError> {
        self.install_schema()?;
        let row = import_event_row(evidence)?;
        put_import_event_row(self.db, &row)?;
        Ok(ImportEventReceipt {
            import_id: row.import_id,
        })
    }

    pub(crate) fn put_record_ref(
        &self,
        evidence: RecordRefEvidence,
    ) -> Result<RecordRefReceipt, EvalStoreError> {
        self.install_schema()?;
        let row = record_ref_row_from_evidence(evidence)?;
        put_record_ref_row(self.db, &row)?;
        Ok(RecordRefReceipt {
            record_ref_id: row.record_ref_id,
            content_sha256: row.content_sha256,
        })
    }

    pub(crate) fn import_observation_jsonl(
        &self,
        import: ObservationJsonlImport,
    ) -> Result<TraceImportReceipt, EvalStoreError> {
        let parsed = parse_observation_jsonl(import)?;
        self.install_schema()?;
        put_log_ref_row(self.db, &parsed.log)?;
        for trace in &parsed.traces {
            put_trace_event_row(self.db, trace)?;
        }
        Ok(TraceImportReceipt {
            log_ref_id: parsed.log.log_ref_id,
            trace_event_ids: parsed
                .traces
                .into_iter()
                .map(|trace| trace.trace_event_id)
                .collect(),
        })
    }

    pub(crate) fn put_trace_event(
        &self,
        evidence: TraceEventEvidence,
    ) -> Result<TraceEventReceipt, EvalStoreError> {
        self.install_schema()?;
        let row = trace_event_row(evidence)?;
        put_trace_event_row(self.db, &row)?;
        Ok(TraceEventReceipt {
            trace_event_id: row.trace_event_id,
        })
    }
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

static OWNER_DB_LOCK: OnceLock<Mutex<()>> = OnceLock::new();

fn mutation_lock() -> &'static Mutex<()> {
    OWNER_DB_LOCK.get_or_init(|| Mutex::new(()))
}

pub(super) fn mutate_owner_db<T>(
    db_path: &Path,
    mutate: impl FnOnce(&Database) -> Result<T, EvalStoreError>,
) -> Result<T, EvalStoreError> {
    let _guard = mutation_lock()
        .lock()
        .map_err(|_| EvalStoreError::DbSetup {
            phase: "owner_eval_db.lock",
            detail: "owner eval DB mutation lock is poisoned".to_string(),
        })?;
    let db = load_owner_eval_database(db_path)?;
    let result = mutate(&db)?;
    persist_owner_eval_database(&db, db_path)?;
    Ok(result)
}

pub(super) fn write_parent_started_to_owner_db(
    db_path: &Path,
    evidence: &ParentStartedEvidence,
    receipt: &ParentStartedReceipt,
) -> Result<ParentStartedDbReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store.put_parent_started_from_receipt(evidence, receipt)
    })
}

pub(crate) fn write_trace_event_to_owner_db(
    db_path: &Path,
    evidence: TraceEventEvidence,
) -> Result<TraceEventReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store.put_trace_event(evidence)
    })
}

pub(crate) fn write_invocation_to_owner_db(
    db_path: &Path,
    evidence: super::evidence::InvocationEvidence,
) -> Result<super::evidence::InvocationReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store.put_invocation(evidence)
    })
}

pub(crate) fn write_channel_message_to_owner_db(
    db_path: &Path,
    evidence: ChannelMessageEvidence,
) -> Result<ChannelMessageReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store.put_channel_message(evidence)
    })
}

pub(crate) fn write_channel_receipt_to_owner_db(
    db_path: &Path,
    evidence: ChannelReceiptEvidence,
) -> Result<String, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store
            .put_channel_receipt(evidence)
            .map(|receipt| receipt.receipt_id)
    })
}

pub(crate) fn write_import_event_to_owner_db(
    db_path: &Path,
    evidence: ImportEventEvidence,
) -> Result<String, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store
            .put_import_event(evidence)
            .map(|receipt| receipt.import_id)
    })
}

pub(crate) fn write_record_ref_to_owner_db(
    db_path: &Path,
    evidence: RecordRefEvidence,
) -> Result<RecordRefReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store.put_record_ref(evidence)
    })
}

pub(crate) fn write_log_ref_to_owner_db(
    db_path: &Path,
    evidence: LogRefEvidence,
) -> Result<LogRefReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store.put_log_ref(evidence)
    })
}

pub(crate) fn write_r0_context_to_owner_db(
    db_path: &Path,
    manifest_path: &Path,
    manifest: &CampaignManifest,
    storage_backend: EvalStorageBackend,
    admitted_profile: Option<&AdmittedRunProfile>,
    closure_path: &Path,
    closure_state: &ClosureState,
) -> Result<(), EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store.put_r0_context(
            manifest_path,
            manifest,
            storage_backend,
            admitted_profile,
            closure_path,
            closure_state,
        )
    })
}

pub(crate) fn write_baseline_to_owner_db(
    db_path: &Path,
    parent: &ParentIdentity,
    baseline: &CompleteBaseline,
    closure: Option<(&Path, &ClosureState)>,
    evaluation_id: Option<&str>,
    record_ref: Option<&str>,
    recorded_at: String,
) -> Result<String, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store.put_baseline(
            parent,
            baseline,
            closure,
            evaluation_id,
            record_ref,
            recorded_at,
        )
    })
}

pub(crate) fn write_closure_state_to_owner_db(
    db_path: &Path,
    closure_path: &Path,
    closure_state: &ClosureState,
) -> Result<String, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        let store = DbEvalStore::new(db);
        store.put_closure_state(closure_path, closure_state)
    })
}

pub(super) fn persist_owner_eval_database(
    db: &Database,
    path: &Path,
) -> Result<(), EvalStoreError> {
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

pub(crate) fn owner_eval_db_file_for_record_path(
    record_path: &Path,
) -> Result<PathBuf, EvalStoreError> {
    let prototype_root = record_path
        .ancestors()
        .find(|ancestor| ancestor.file_name().and_then(|name| name.to_str()) == Some("prototype1"))
        .ok_or_else(|| EvalStoreError::Validation {
            field: "owner_eval_db.path",
            detail: format!(
                "record '{}' is not under a Prototype 1 record layout",
                record_path.display()
            ),
        })?;
    Ok(prototype_root.join("eval-store.cozo.sqlite"))
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
    Ok(existing_record_ref_content_hash(db, record_ref_id)?.as_deref() == Some(content_sha256))
}

fn existing_record_ref_content_hash<D: EvalDb + ?Sized>(
    db: &D,
    record_ref_id: &str,
) -> Result<Option<String>, EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert(
        "record_ref_id".to_string(),
        DataValue::from(record_ref_id.to_string()),
    );
    let result = db
        .eval_query_params(
            r#"
?[content_sha256] :=
    *eval_record_ref { record_ref_id, content_sha256 },
    record_ref_id = $record_ref_id
"#,
            params,
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "query.eval_record_ref.content_hash",
            source,
        })?;
    Ok(result.rows.first().and_then(|row| match row.first() {
        Some(DataValue::Str(value)) => Some(value.to_string()),
        _ => None,
    }))
}

fn existing_invocation_content_hash<D: EvalDb + ?Sized>(
    db: &D,
    invocation_id: &str,
) -> Result<Option<String>, EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert(
        "invocation_id".to_string(),
        DataValue::from(invocation_id.to_string()),
    );
    let result = db
        .eval_query_params(
            r#"
?[content_sha256] :=
    *eval_invocation { invocation_id, content_sha256 },
    invocation_id = $invocation_id
"#,
            params,
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "query.eval_invocation.content_hash",
            source,
        })?;
    Ok(result.rows.first().and_then(|row| match row.first() {
        Some(DataValue::Str(value)) => Some(value.to_string()),
        _ => None,
    }))
}

fn existing_attempt_invocation_id<D: EvalDb + ?Sized>(
    db: &D,
    attempt_id: &str,
) -> Result<Option<String>, EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert(
        "attempt_id".to_string(),
        DataValue::from(attempt_id.to_string()),
    );
    let result = db
        .eval_query_params(
            r#"
?[invocation_id] :=
    *eval_attempt { attempt_id, invocation_id },
    attempt_id = $attempt_id
"#,
            params,
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "query.eval_attempt.invocation_id",
            source,
        })?;
    Ok(result.rows.first().and_then(|row| match row.first() {
        Some(DataValue::Str(value)) => Some(value.to_string()),
        _ => None,
    }))
}

fn existing_channel_message_content_hash<D: EvalDb + ?Sized>(
    db: &D,
    channel_message_id: &str,
) -> Result<Option<String>, EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert(
        "channel_message_id".to_string(),
        DataValue::from(channel_message_id.to_string()),
    );
    let result = db
        .eval_query_params(
            r#"
?[content_sha256] :=
    *eval_channel_message { channel_message_id, content_sha256 },
    channel_message_id = $channel_message_id
"#,
            params,
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "query.eval_channel_message.content_hash",
            source,
        })?;
    Ok(result.rows.first().and_then(|row| match row.first() {
        Some(DataValue::Str(value)) => Some(value.to_string()),
        _ => None,
    }))
}

fn put_transition_event_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalTransitionEventRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &TransitionEventSchema::SCHEMA,
        transition_event_params(row),
        "put.eval_transition_event",
    )?;
    Ok(())
}

fn put_invocation_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalInvocationRow,
) -> Result<(), EvalStoreError> {
    if let Some(existing) = existing_invocation_content_hash(db, &row.invocation_id)? {
        if existing == row.content_sha256 {
            return Ok(());
        }
        return Err(EvalStoreError::Validation {
            field: "eval_invocation.content_sha256",
            detail: format!(
                "invocation '{}' already exists with content hash {}, attempted {}",
                row.invocation_id, existing, row.content_sha256
            ),
        });
    }
    put_eval_params(
        db,
        &InvocationSchema::SCHEMA,
        invocation_params(row),
        "put.eval_invocation",
    )?;
    Ok(())
}

fn put_attempt_row<D: EvalDb + ?Sized>(db: &D, row: &EvalAttemptRow) -> Result<(), EvalStoreError> {
    if let Some(existing) = existing_attempt_invocation_id(db, &row.attempt_id)? {
        if row.invocation_id.as_deref() == Some(existing.as_str()) {
            return Ok(());
        }
        return Err(EvalStoreError::Validation {
            field: "eval_attempt.invocation_id",
            detail: format!(
                "attempt '{}' already exists with invocation id {}, attempted {}",
                row.attempt_id,
                existing,
                row.invocation_id.as_deref().unwrap_or("<none>")
            ),
        });
    }
    put_eval_params(
        db,
        &AttemptSchema::SCHEMA,
        attempt_params(row),
        "put.eval_attempt",
    )?;
    Ok(())
}

fn put_channel_message_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalChannelMessageRow,
) -> Result<(), EvalStoreError> {
    if let Some(existing) = existing_channel_message_content_hash(db, &row.channel_message_id)? {
        if existing == row.content_sha256 {
            return Ok(());
        }
        return Err(EvalStoreError::Validation {
            field: "eval_channel_message.content_sha256",
            detail: format!(
                "channel message '{}' already exists with content hash {}, attempted {}",
                row.channel_message_id, existing, row.content_sha256
            ),
        });
    }
    put_eval_params(
        db,
        &ChannelMessageSchema::SCHEMA,
        channel_message_params(row),
        "put.eval_channel_message",
    )?;
    Ok(())
}

fn put_channel_receipt_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalChannelReceiptRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &ChannelReceiptSchema::SCHEMA,
        channel_receipt_params(row),
        "put.eval_channel_receipt",
    )?;
    Ok(())
}

fn put_import_event_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalImportEventRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &ImportEventSchema::SCHEMA,
        import_event_params(row),
        "put.eval_import_event",
    )?;
    Ok(())
}

fn put_record_ref_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalRecordRefRow,
) -> Result<(), EvalStoreError> {
    if let Some(existing) = existing_record_ref_content_hash(db, &row.record_ref_id)? {
        if existing == row.content_sha256 {
            return Ok(());
        }
        return Err(EvalStoreError::Validation {
            field: "eval_record_ref.content_sha256",
            detail: format!(
                "record ref '{}' already exists with content hash {}, attempted {}",
                row.record_ref_id, existing, row.content_sha256
            ),
        });
    }
    put_eval_params(
        db,
        &RecordRefSchema::SCHEMA,
        record_ref_params(row),
        "put.eval_record_ref",
    )?;
    Ok(())
}

fn put_log_ref_row<D: EvalDb + ?Sized>(db: &D, row: &EvalLogRefRow) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &LogRefSchema::SCHEMA,
        log_ref_params(row),
        "put.eval_log_ref",
    )?;
    Ok(())
}

fn put_trace_event_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalTraceEventRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &TraceEventSchema::SCHEMA,
        trace_event_params(row),
        "put.eval_trace_event",
    )?;
    Ok(())
}
