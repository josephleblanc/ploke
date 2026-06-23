use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use cozo::DataValue;
use ploke_db::{Database, DbError, QueryResult};

use super::{
    cozo_params::{
        channel_message_params, channel_receipt_params, import_event_params, invocation_params,
        log_ref_params, record_ref_params, trace_event_params, transition_event_params,
    },
    cozo_schema::ensure_eval_store_schema,
    error::EvalStoreError,
    evidence::{
        ChannelMessageEvidence, ChannelMessageReceipt, ChannelReceiptEvidence,
        ChannelReceiptReceipt, EvalChannelMessageRow, EvalChannelReceiptRow, EvalImportEventRow,
        EvalInvocationRow, EvalLogRefRow, EvalRecordRefRow, EvalTraceEventRow,
        EvalTransitionEventRow, ImportEventEvidence, ImportEventReceipt, LogRefEvidence,
        LogRefReceipt, ParentStartedDbReceipt, ParentStartedEvidence, ParentStartedReceipt,
        ParentStartedRows, RecordRefEvidence, RecordRefReceipt, TraceEventEvidence,
        TraceEventReceipt, TraceImportReceipt, channel_message_row, channel_receipt_row,
        import_event_row, invocation_row, log_ref_row, parent_started_rows,
        record_ref_row_from_evidence, trace_event_row,
    },
    observation::{ObservationJsonlImport, parse_observation_jsonl},
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
        put_invocation_row(self.db, &row)?;
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

pub(super) fn write_parent_started_to_owner_db(
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

pub(crate) fn write_trace_event_to_owner_db(
    db_path: &Path,
    evidence: TraceEventEvidence,
) -> Result<TraceEventReceipt, EvalStoreError> {
    let db = load_owner_eval_database(db_path)?;
    let store = DbEvalStore::new(&db);
    let receipt = store.put_trace_event(evidence)?;
    persist_owner_eval_database(&db, db_path)?;
    Ok(receipt)
}

pub(crate) fn write_invocation_to_owner_db(
    db_path: &Path,
    evidence: super::evidence::InvocationEvidence,
) -> Result<super::evidence::InvocationReceipt, EvalStoreError> {
    let db = load_owner_eval_database(db_path)?;
    let store = DbEvalStore::new(&db);
    let receipt = store.put_invocation(evidence)?;
    persist_owner_eval_database(&db, db_path)?;
    Ok(receipt)
}

pub(crate) fn write_channel_message_to_owner_db(
    db_path: &Path,
    evidence: ChannelMessageEvidence,
) -> Result<ChannelMessageReceipt, EvalStoreError> {
    let db = load_owner_eval_database(db_path)?;
    let store = DbEvalStore::new(&db);
    let receipt = store.put_channel_message(evidence)?;
    persist_owner_eval_database(&db, db_path)?;
    Ok(receipt)
}

pub(crate) fn write_channel_receipt_to_owner_db(
    db_path: &Path,
    evidence: ChannelReceiptEvidence,
) -> Result<String, EvalStoreError> {
    let db = load_owner_eval_database(db_path)?;
    let store = DbEvalStore::new(&db);
    let receipt = store.put_channel_receipt(evidence)?;
    persist_owner_eval_database(&db, db_path)?;
    Ok(receipt.receipt_id)
}

pub(crate) fn write_import_event_to_owner_db(
    db_path: &Path,
    evidence: ImportEventEvidence,
) -> Result<String, EvalStoreError> {
    let db = load_owner_eval_database(db_path)?;
    let store = DbEvalStore::new(&db);
    let receipt = store.put_import_event(evidence)?;
    persist_owner_eval_database(&db, db_path)?;
    Ok(receipt.import_id)
}

pub(crate) fn write_record_ref_to_owner_db(
    db_path: &Path,
    evidence: RecordRefEvidence,
) -> Result<RecordRefReceipt, EvalStoreError> {
    let db = load_owner_eval_database(db_path)?;
    let store = DbEvalStore::new(&db);
    let receipt = store.put_record_ref(evidence)?;
    persist_owner_eval_database(&db, db_path)?;
    Ok(receipt)
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
    db.eval_query_mut_params(
        r#"
?[
    invocation_id,
    campaign_id,
    node_id,
    runtime_id,
    role,
    store_scope,
    producer_role,
    visibility_scope,
    source_class,
    evidence_class,
    validation_status,
    invocation_path,
    source_ref,
    content_sha256,
    recorded_at,
    ingested_at
] :=
    invocation_id = $invocation_id,
    campaign_id = $campaign_id,
    node_id = $node_id,
    runtime_id = $runtime_id,
    role = $role,
    store_scope = $store_scope,
    producer_role = $producer_role,
    visibility_scope = $visibility_scope,
    source_class = $source_class,
    evidence_class = $evidence_class,
    validation_status = $validation_status,
    invocation_path = $invocation_path,
    source_ref = $source_ref,
    content_sha256 = $content_sha256,
    recorded_at = $recorded_at,
    ingested_at = $ingested_at
:put eval_invocation {
    invocation_id =>
    campaign_id,
    node_id,
    runtime_id,
    role,
    store_scope,
    producer_role,
    visibility_scope,
    source_class,
    evidence_class,
    validation_status,
    invocation_path,
    source_ref,
    content_sha256,
    recorded_at,
    ingested_at
}
"#,
        invocation_params(row),
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_invocation",
        source,
    })?;
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
    db.eval_query_mut_params(
        r#"
?[
    channel_message_id,
    campaign_id,
    node_id,
    runtime_id,
    direction,
    message_kind,
    message_id,
    store_scope,
    producer_role,
    visibility_scope,
    source_class,
    evidence_class,
    validation_status,
    endpoint_path,
    cursor_offset,
    bytes_written,
    body_hash,
    content_sha256,
    source_ref,
    recorded_at,
    ingested_at
] :=
    channel_message_id = $channel_message_id,
    campaign_id = $campaign_id,
    node_id = $node_id,
    runtime_id = $runtime_id,
    direction = $direction,
    message_kind = $message_kind,
    message_id = $message_id,
    store_scope = $store_scope,
    producer_role = $producer_role,
    visibility_scope = $visibility_scope,
    source_class = $source_class,
    evidence_class = $evidence_class,
    validation_status = $validation_status,
    endpoint_path = $endpoint_path,
    cursor_offset = $cursor_offset,
    bytes_written = $bytes_written,
    body_hash = $body_hash,
    content_sha256 = $content_sha256,
    source_ref = $source_ref,
    recorded_at = $recorded_at,
    ingested_at = $ingested_at
:put eval_channel_message {
    channel_message_id =>
    campaign_id,
    node_id,
    runtime_id,
    direction,
    message_kind,
    message_id,
    store_scope,
    producer_role,
    visibility_scope,
    source_class,
    evidence_class,
    validation_status,
    endpoint_path,
    cursor_offset,
    bytes_written,
    body_hash,
    content_sha256,
    source_ref,
    recorded_at,
    ingested_at
}
"#,
        channel_message_params(row),
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_channel_message",
        source,
    })?;
    Ok(())
}

fn put_channel_receipt_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalChannelReceiptRow,
) -> Result<(), EvalStoreError> {
    db.eval_query_mut_params(
        r#"
?[
    receipt_id,
    channel_id,
    message_id,
    campaign_id,
    node_id,
    runtime_id,
    observed_by,
    direction,
    validation_status,
    imported_ref,
    observed_at
] :=
    receipt_id = $receipt_id,
    channel_id = $channel_id,
    message_id = $message_id,
    campaign_id = $campaign_id,
    node_id = $node_id,
    runtime_id = $runtime_id,
    observed_by = $observed_by,
    direction = $direction,
    validation_status = $validation_status,
    imported_ref = $imported_ref,
    observed_at = $observed_at
:put eval_channel_receipt {
    receipt_id =>
    channel_id,
    message_id,
    campaign_id,
    node_id,
    runtime_id,
    observed_by,
    direction,
    validation_status,
    imported_ref,
    observed_at
}
"#,
        channel_receipt_params(row),
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_channel_receipt",
        source,
    })?;
    Ok(())
}

fn put_import_event_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalImportEventRow,
) -> Result<(), EvalStoreError> {
    db.eval_query_mut_params(
        r#"
?[
    import_id,
    campaign_id,
    importer_id,
    source_runtime_id,
    source_scope,
    target_scope,
    evidence_ref,
    receipt_id,
    validation_status,
    imported_at
] :=
    import_id = $import_id,
    campaign_id = $campaign_id,
    importer_id = $importer_id,
    source_runtime_id = $source_runtime_id,
    source_scope = $source_scope,
    target_scope = $target_scope,
    evidence_ref = $evidence_ref,
    receipt_id = $receipt_id,
    validation_status = $validation_status,
    imported_at = $imported_at
:put eval_import_event {
    import_id =>
    campaign_id,
    importer_id,
    source_runtime_id,
    source_scope,
    target_scope,
    evidence_ref,
    receipt_id,
    validation_status,
    imported_at
}
"#,
        import_event_params(row),
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_import_event",
        source,
    })?;
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

fn put_log_ref_row<D: EvalDb + ?Sized>(db: &D, row: &EvalLogRefRow) -> Result<(), EvalStoreError> {
    db.eval_query_mut_params(
        r#"
?[
    log_ref_id,
    campaign_id,
    runtime_id,
    store_scope,
    log_kind,
    source_ref,
    byte_start,
    byte_len,
    content_sha256,
    sensitivity,
    recorded_at
] :=
    log_ref_id = $log_ref_id,
    campaign_id = $campaign_id,
    runtime_id = $runtime_id,
    store_scope = $store_scope,
    log_kind = $log_kind,
    source_ref = $source_ref,
    byte_start = $byte_start,
    byte_len = $byte_len,
    content_sha256 = $content_sha256,
    sensitivity = $sensitivity,
    recorded_at = $recorded_at
:put eval_log_ref {
    log_ref_id =>
    campaign_id,
    runtime_id,
    store_scope,
    log_kind,
    source_ref,
    byte_start,
    byte_len,
    content_sha256,
    sensitivity,
    recorded_at
}
"#,
        log_ref_params(row),
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_log_ref",
        source,
    })?;
    Ok(())
}

fn put_trace_event_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalTraceEventRow,
) -> Result<(), EvalStoreError> {
    db.eval_query_mut_params(
        r#"
?[
    trace_event_id,
    campaign_id,
    parent_id,
    runtime_id,
    node_id,
    generation,
    branch_id,
    role,
    pipeline,
    stage,
    authority,
    transition,
    event_name,
    span_name,
    target,
    level,
    outcome,
    duration_ms,
    record_access,
    record_kind,
    record_path,
    record_index,
    record_count,
    program,
    exit_code,
    error,
    source_log_ref,
    source_event_index,
    recorded_at
] :=
    trace_event_id = $trace_event_id,
    campaign_id = $campaign_id,
    parent_id = $parent_id,
    runtime_id = $runtime_id,
    node_id = $node_id,
    generation = $generation,
    branch_id = $branch_id,
    role = $role,
    pipeline = $pipeline,
    stage = $stage,
    authority = $authority,
    transition = $transition,
    event_name = $event_name,
    span_name = $span_name,
    target = $target,
    level = $level,
    outcome = $outcome,
    duration_ms = $duration_ms,
    record_access = $record_access,
    record_kind = $record_kind,
    record_path = $record_path,
    record_index = $record_index,
    record_count = $record_count,
    program = $program,
    exit_code = $exit_code,
    error = $error,
    source_log_ref = $source_log_ref,
    source_event_index = $source_event_index,
    recorded_at = $recorded_at
:put eval_trace_event {
    trace_event_id =>
    campaign_id,
    parent_id,
    runtime_id,
    node_id,
    generation,
    branch_id,
    role,
    pipeline,
    stage,
    authority,
    transition,
    event_name,
    span_name,
    target,
    level,
    outcome,
    duration_ms,
    record_access,
    record_kind,
    record_path,
    record_index,
    record_count,
    program,
    exit_code,
    error,
    source_log_ref,
    source_event_index,
    recorded_at
}
"#,
        trace_event_params(row),
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_trace_event",
        source,
    })?;
    Ok(())
}
