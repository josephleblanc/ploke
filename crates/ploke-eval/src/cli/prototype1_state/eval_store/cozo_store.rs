use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use cozo::DataValue;
use ploke_db::{Database, DbError, QueryResult};

use super::{
    cozo_params::{
        invocation_params, log_ref_params, record_ref_params, trace_event_params,
        transition_event_params,
    },
    error::EvalStoreError,
    evidence::{
        EVENT_REL, EvalInvocationRow, EvalLogRefRow, EvalRecordRefRow, EvalTraceEventRow,
        EvalTransitionEventRow, INVOCATION_REL, LOG_REF_REL, LogRefEvidence, LogRefReceipt,
        ObservationJsonlImport, ParentStartedDbReceipt, ParentStartedEvidence,
        ParentStartedReceipt, ParentStartedRows, RECORD_REL, RecordRefEvidence, RecordRefReceipt,
        TRACE_EVENT_REL, TraceEventEvidence, TraceEventReceipt, TraceImportReceipt, invocation_row,
        log_ref_row, parent_started_rows, parse_observation_jsonl, record_ref_row_from_evidence,
        trace_event_row,
    },
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

    if !eval_relation_exists(db, LOG_REF_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_log_ref {
    log_ref_id: String =>
    campaign_id: String?,
    runtime_id: String?,
    store_scope: String,
    log_kind: String,
    source_ref: String,
    byte_start: Int?,
    byte_len: Int?,
    content_sha256: String?,
    sensitivity: String?,
    recorded_at: String?
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_log_ref",
            source,
        })?;
    }

    if !eval_relation_exists(db, INVOCATION_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_invocation {
    invocation_id: String =>
    campaign_id: String,
    node_id: String,
    runtime_id: String,
    role: String,
    store_scope: String,
    producer_role: String,
    visibility_scope: String,
    source_class: String,
    evidence_class: String,
    validation_status: String,
    invocation_path: String,
    source_ref: String,
    content_sha256: String,
    recorded_at: String,
    ingested_at: String
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_invocation",
            source,
        })?;
    }

    if !eval_relation_exists(db, TRACE_EVENT_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_trace_event {
    trace_event_id: String =>
    campaign_id: String?,
    parent_id: String?,
    runtime_id: String?,
    node_id: String?,
    generation: Int?,
    branch_id: String?,
    role: String?,
    pipeline: String?,
    stage: String?,
    authority: String?,
    transition: String?,
    event_name: String?,
    span_name: String?,
    target: String,
    level: String,
    outcome: String?,
    duration_ms: Int?,
    record_access: String?,
    record_kind: String?,
    record_path: String?,
    record_index: Int?,
    record_count: Int?,
    program: String?,
    exit_code: Int?,
    error: String?,
    source_log_ref: String?,
    source_event_index: Int?,
    recorded_at: String?
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_trace_event",
            source,
        })?;
    }

    Ok(())
}

pub(super) fn eval_relation_exists<D: EvalDb + ?Sized>(
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
