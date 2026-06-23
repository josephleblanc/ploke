use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use cozo::DataValue;
use ploke_db::{Database, DbError, QueryResult};

use super::{
    error::EvalStoreError,
    evidence::{
        EVENT_REL, EvalLogRefRow, EvalRecordRefRow, EvalTraceEventRow, EvalTransitionEventRow,
        LOG_REF_REL, LogRefEvidence, LogRefReceipt, ObservationJsonlImport, ParentStartedDbReceipt,
        ParentStartedEvidence, ParentStartedReceipt, ParentStartedRows, RECORD_REL,
        TRACE_EVENT_REL, TraceEventEvidence, TraceEventReceipt, TraceImportReceipt, log_ref_row,
        parent_started_rows, parse_observation_jsonl, trace_event_row,
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

fn log_ref_params(row: &EvalLogRefRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("log_ref_id".to_string(), row.log_ref_id.clone().into());
    params.insert(
        "campaign_id".to_string(),
        option_string_param(&row.campaign_id),
    );
    params.insert(
        "runtime_id".to_string(),
        option_string_param(&row.runtime_id),
    );
    params.insert("store_scope".to_string(), row.store_scope.clone().into());
    params.insert("log_kind".to_string(), row.log_kind.clone().into());
    params.insert("source_ref".to_string(), row.source_ref.clone().into());
    params.insert("byte_start".to_string(), option_i64_param(row.byte_start));
    params.insert("byte_len".to_string(), option_i64_param(row.byte_len));
    params.insert(
        "content_sha256".to_string(),
        option_string_param(&row.content_sha256),
    );
    params.insert(
        "sensitivity".to_string(),
        option_string_param(&row.sensitivity),
    );
    params.insert(
        "recorded_at".to_string(),
        option_string_param(&row.recorded_at),
    );
    params
}

fn trace_event_params(row: &EvalTraceEventRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "trace_event_id".to_string(),
        row.trace_event_id.clone().into(),
    );
    params.insert(
        "campaign_id".to_string(),
        option_string_param(&row.campaign_id),
    );
    params.insert("parent_id".to_string(), option_string_param(&row.parent_id));
    params.insert(
        "runtime_id".to_string(),
        option_string_param(&row.runtime_id),
    );
    params.insert("node_id".to_string(), option_string_param(&row.node_id));
    params.insert("generation".to_string(), option_i64_param(row.generation));
    params.insert("branch_id".to_string(), option_string_param(&row.branch_id));
    params.insert("role".to_string(), option_string_param(&row.role));
    params.insert("pipeline".to_string(), option_string_param(&row.pipeline));
    params.insert("stage".to_string(), option_string_param(&row.stage));
    params.insert("authority".to_string(), option_string_param(&row.authority));
    params.insert(
        "transition".to_string(),
        option_string_param(&row.transition),
    );
    params.insert(
        "event_name".to_string(),
        option_string_param(&row.event_name),
    );
    params.insert("span_name".to_string(), option_string_param(&row.span_name));
    params.insert("target".to_string(), row.target.clone().into());
    params.insert("level".to_string(), row.level.clone().into());
    params.insert("outcome".to_string(), option_string_param(&row.outcome));
    params.insert("duration_ms".to_string(), option_i64_param(row.duration_ms));
    params.insert(
        "record_access".to_string(),
        option_string_param(&row.record_access),
    );
    params.insert(
        "record_kind".to_string(),
        option_string_param(&row.record_kind),
    );
    params.insert(
        "record_path".to_string(),
        option_string_param(&row.record_path),
    );
    params.insert(
        "record_index".to_string(),
        option_i64_param(row.record_index),
    );
    params.insert(
        "record_count".to_string(),
        option_i64_param(row.record_count),
    );
    params.insert("program".to_string(), option_string_param(&row.program));
    params.insert("exit_code".to_string(), option_i64_param(row.exit_code));
    params.insert("error".to_string(), option_string_param(&row.error));
    params.insert(
        "source_log_ref".to_string(),
        option_string_param(&row.source_log_ref),
    );
    params.insert(
        "source_event_index".to_string(),
        option_i64_param(row.source_event_index),
    );
    params.insert(
        "recorded_at".to_string(),
        option_string_param(&row.recorded_at),
    );
    params
}

fn option_string_param(value: &Option<String>) -> DataValue {
    value
        .clone()
        .map(DataValue::from)
        .unwrap_or(DataValue::Null)
}

fn option_i64_param(value: Option<i64>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}
