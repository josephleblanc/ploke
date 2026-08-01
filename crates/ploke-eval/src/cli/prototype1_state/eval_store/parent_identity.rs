use std::collections::BTreeMap;

use chrono::Utc;
use cozo::DataValue;
use sha2::{Digest, Sha256};

use super::{
    cozo_store::EvalDb,
    error::EvalStoreError,
    evidence::{ParentStartedEvidence, ParentStartedRows},
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

pub(crate) const PARENT_IDENTITY_SCHEMA_VERSION: &str = "prototype1-parent-identity.v1";
pub(crate) const PARENT_START_SCHEMA_VERSION: &str = "prototype1-parent-start.v1";

define_eval_schema!(ParentIdentitySchema {
    "eval_parent_identity",
    campaign_id: "String",
    parent_id: "String" =>
    schema_version: "String",
    identity_schema_version: "String",
    node_id: "String",
    generation: "Int",
    branch_id: "String",
    artifact_branch: "String?",
    instance_id: "String?",
    previous_parent_id: "String?",
    parent_node_id: "String?",
    identity_created_at: "String",
    semantic_hash: "String",
    ingested_at: "String",
});

define_eval_schema!(ParentStartSchema {
    "eval_parent_start",
    start_event_id: "String" =>
    campaign_id: "String",
    schema_version: "String",
    parent_id: "String",
    node_id: "String",
    generation: "Int",
    branch_id: "String",
    repo_root: "String",
    startup_kind: "String",
    handoff_runtime_id: "String?",
    pid: "Int",
    source_stream_id: "String",
    source_event_index: "Int",
    source_line: "Int",
    parent_recorded_at: "Int",
    resource_recorded_at: "Int",
    semantic_hash: "String",
    ingested_at: "String",
});

pub(super) fn ensure_parent_identity_schema<D: EvalDb + ?Sized>(
    db: &D,
) -> Result<(), EvalStoreError> {
    ParentIdentitySchema::SCHEMA.ensure_installed(db, "schema.eval_parent_identity")?;
    ParentStartSchema::SCHEMA.ensure_installed(db, "schema.eval_parent_start")?;
    Ok(())
}

pub(super) fn put_parent_started_identity_rows<D: EvalDb + ?Sized>(
    db: &D,
    evidence: &ParentStartedEvidence,
    rows: &ParentStartedRows,
) -> Result<(), EvalStoreError> {
    let ingested_at = Utc::now().to_rfc3339();
    let identity = ParentIdentityRow::from_evidence(evidence, &ingested_at)?;
    put_parent_identity_row(db, &identity)?;
    let start = ParentStartRow::from_parent_started(evidence, rows, &ingested_at)?;
    put_parent_start_row(db, &start)
}

struct ParentIdentityRow {
    campaign_id: String,
    parent_id: String,
    identity_schema_version: String,
    node_id: String,
    generation: i64,
    branch_id: String,
    artifact_branch: Option<String>,
    instance_id: Option<String>,
    previous_parent_id: Option<String>,
    parent_node_id: Option<String>,
    identity_created_at: String,
    semantic_hash: String,
    ingested_at: String,
}

impl ParentIdentityRow {
    fn from_evidence(
        evidence: &ParentStartedEvidence,
        ingested_at: &str,
    ) -> Result<Self, EvalStoreError> {
        let identity = &evidence.parent_identity;
        let row = Self {
            campaign_id: evidence.campaign_id.to_string(),
            parent_id: identity.parent_id().to_string(),
            identity_schema_version: identity.schema_version().to_string(),
            node_id: identity.node_id().to_string(),
            generation: i64::from(identity.generation()),
            branch_id: identity.branch_id().to_string(),
            artifact_branch: identity.artifact_branch().map(ToString::to_string),
            instance_id: identity.instance_id().map(ToString::to_string),
            previous_parent_id: identity.previous_parent_id().map(ToString::to_string),
            parent_node_id: identity.parent_node_id().map(ToString::to_string),
            identity_created_at: identity.created_at().to_string(),
            semantic_hash: String::new(),
            ingested_at: ingested_at.to_string(),
        };
        row.with_hash()
    }

    fn with_hash(mut self) -> Result<Self, EvalStoreError> {
        require_non_empty("eval_parent_identity.campaign_id", &self.campaign_id)?;
        require_non_empty("eval_parent_identity.parent_id", &self.parent_id)?;
        require_non_empty("eval_parent_identity.node_id", &self.node_id)?;
        require_non_empty("eval_parent_identity.branch_id", &self.branch_id)?;
        require_non_empty(
            "eval_parent_identity.identity_created_at",
            &self.identity_created_at,
        )?;
        self.semantic_hash = hash_parts(&[
            PARENT_IDENTITY_SCHEMA_VERSION,
            &self.campaign_id,
            &self.parent_id,
            &self.identity_schema_version,
            &self.node_id,
            &self.generation.to_string(),
            &self.branch_id,
            self.artifact_branch.as_deref().unwrap_or(""),
            self.instance_id.as_deref().unwrap_or(""),
            self.previous_parent_id.as_deref().unwrap_or(""),
            self.parent_node_id.as_deref().unwrap_or(""),
            &self.identity_created_at,
        ]);
        Ok(self)
    }
}

struct ParentStartRow {
    start_event_id: String,
    campaign_id: String,
    parent_id: String,
    node_id: String,
    generation: i64,
    branch_id: String,
    repo_root: String,
    startup_kind: String,
    handoff_runtime_id: Option<String>,
    pid: i64,
    source_stream_id: String,
    source_event_index: i64,
    source_line: i64,
    parent_recorded_at: i64,
    resource_recorded_at: i64,
    semantic_hash: String,
    ingested_at: String,
}

impl ParentStartRow {
    fn from_parent_started(
        evidence: &ParentStartedEvidence,
        rows: &ParentStartedRows,
        ingested_at: &str,
    ) -> Result<Self, EvalStoreError> {
        let identity = &evidence.parent_identity;
        let startup_kind = if evidence.handoff_runtime_id.is_some() {
            "predecessor"
        } else {
            "genesis"
        };
        let row = Self {
            start_event_id: rows.event.event_id.clone(),
            campaign_id: evidence.campaign_id.to_string(),
            parent_id: identity.parent_id().to_string(),
            node_id: identity.node_id().to_string(),
            generation: i64::from(identity.generation()),
            branch_id: identity.branch_id().to_string(),
            repo_root: evidence.repo_root.display().to_string(),
            startup_kind: startup_kind.to_string(),
            handoff_runtime_id: evidence.handoff_runtime_id.map(|id| id.to_string()),
            pid: i64::from(evidence.pid),
            source_stream_id: rows.event.source_stream_id.clone(),
            source_event_index: rows.event.source_event_index,
            source_line: rows.event.source_line,
            parent_recorded_at: evidence.parent_recorded_at.0,
            resource_recorded_at: evidence.resource_recorded_at.0,
            semantic_hash: String::new(),
            ingested_at: ingested_at.to_string(),
        };
        row.with_hash()
    }

    fn with_hash(mut self) -> Result<Self, EvalStoreError> {
        require_non_empty("eval_parent_start.start_event_id", &self.start_event_id)?;
        require_non_empty("eval_parent_start.campaign_id", &self.campaign_id)?;
        require_non_empty("eval_parent_start.parent_id", &self.parent_id)?;
        require_non_empty("eval_parent_start.node_id", &self.node_id)?;
        require_non_empty("eval_parent_start.branch_id", &self.branch_id)?;
        require_non_empty("eval_parent_start.repo_root", &self.repo_root)?;
        require_non_empty("eval_parent_start.startup_kind", &self.startup_kind)?;
        self.semantic_hash = hash_parts(&[
            PARENT_START_SCHEMA_VERSION,
            &self.start_event_id,
            &self.campaign_id,
            &self.parent_id,
            &self.node_id,
            &self.generation.to_string(),
            &self.branch_id,
            &self.repo_root,
            &self.startup_kind,
            self.handoff_runtime_id.as_deref().unwrap_or(""),
            &self.pid.to_string(),
            &self.source_stream_id,
            &self.source_event_index.to_string(),
            &self.source_line.to_string(),
            &self.parent_recorded_at.to_string(),
            &self.resource_recorded_at.to_string(),
        ]);
        Ok(self)
    }
}

fn put_parent_identity_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &ParentIdentityRow,
) -> Result<(), EvalStoreError> {
    if let Some(existing) = existing_parent_identity_hash(db, &row.campaign_id, &row.parent_id)? {
        if existing == row.semantic_hash {
            return Ok(());
        }
        return Err(EvalStoreError::Validation {
            field: "eval_parent_identity.semantic_hash",
            detail: format!(
                "parent identity '{}:{}' already exists with semantic hash {}, attempted {}",
                row.campaign_id, row.parent_id, existing, row.semantic_hash
            ),
        });
    }
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("parent_id".to_string(), row.parent_id.clone().into());
    params.insert(
        "schema_version".to_string(),
        PARENT_IDENTITY_SCHEMA_VERSION.to_string().into(),
    );
    params.insert(
        "identity_schema_version".to_string(),
        row.identity_schema_version.clone().into(),
    );
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("generation".to_string(), row.generation.into());
    params.insert("branch_id".to_string(), row.branch_id.clone().into());
    params.insert(
        "artifact_branch".to_string(),
        option_string_param(row.artifact_branch.clone()),
    );
    params.insert(
        "instance_id".to_string(),
        option_string_param(row.instance_id.clone()),
    );
    params.insert(
        "previous_parent_id".to_string(),
        option_string_param(row.previous_parent_id.clone()),
    );
    params.insert(
        "parent_node_id".to_string(),
        option_string_param(row.parent_node_id.clone()),
    );
    params.insert(
        "identity_created_at".to_string(),
        row.identity_created_at.clone().into(),
    );
    params.insert(
        "semantic_hash".to_string(),
        row.semantic_hash.clone().into(),
    );
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    put_eval_params(
        db,
        &ParentIdentitySchema::SCHEMA,
        params,
        "put.eval_parent_identity",
    )
}

fn put_parent_start_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &ParentStartRow,
) -> Result<(), EvalStoreError> {
    if let Some(existing) = existing_parent_start_hash(db, &row.start_event_id)? {
        if existing == row.semantic_hash {
            return Ok(());
        }
        return Err(EvalStoreError::Validation {
            field: "eval_parent_start.semantic_hash",
            detail: format!(
                "parent start '{}' already exists with semantic hash {}, attempted {}",
                row.start_event_id, existing, row.semantic_hash
            ),
        });
    }
    let mut params = BTreeMap::new();
    params.insert(
        "start_event_id".to_string(),
        row.start_event_id.clone().into(),
    );
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert(
        "schema_version".to_string(),
        PARENT_START_SCHEMA_VERSION.to_string().into(),
    );
    params.insert("parent_id".to_string(), row.parent_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("generation".to_string(), row.generation.into());
    params.insert("branch_id".to_string(), row.branch_id.clone().into());
    params.insert("repo_root".to_string(), row.repo_root.clone().into());
    params.insert("startup_kind".to_string(), row.startup_kind.clone().into());
    params.insert(
        "handoff_runtime_id".to_string(),
        option_string_param(row.handoff_runtime_id.clone()),
    );
    params.insert("pid".to_string(), row.pid.into());
    params.insert(
        "source_stream_id".to_string(),
        row.source_stream_id.clone().into(),
    );
    params.insert(
        "source_event_index".to_string(),
        row.source_event_index.into(),
    );
    params.insert("source_line".to_string(), row.source_line.into());
    params.insert(
        "parent_recorded_at".to_string(),
        row.parent_recorded_at.into(),
    );
    params.insert(
        "resource_recorded_at".to_string(),
        row.resource_recorded_at.into(),
    );
    params.insert(
        "semantic_hash".to_string(),
        row.semantic_hash.clone().into(),
    );
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    put_eval_params(
        db,
        &ParentStartSchema::SCHEMA,
        params,
        "put.eval_parent_start",
    )
}

fn existing_parent_identity_hash<D: EvalDb + ?Sized>(
    db: &D,
    campaign_id: &str,
    parent_id: &str,
) -> Result<Option<String>, EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert("campaign_id".to_string(), campaign_id.to_string().into());
    params.insert("parent_id".to_string(), parent_id.to_string().into());
    let result = db
        .eval_query_params(
            r#"
?[semantic_hash] :=
    *eval_parent_identity { campaign_id, parent_id, semantic_hash },
    campaign_id = $campaign_id,
    parent_id = $parent_id
"#,
            params,
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "query.eval_parent_identity.semantic_hash",
            source,
        })?;
    Ok(result.rows.first().and_then(|row| match row.first() {
        Some(DataValue::Str(value)) => Some(value.to_string()),
        _ => None,
    }))
}

fn existing_parent_start_hash<D: EvalDb + ?Sized>(
    db: &D,
    start_event_id: &str,
) -> Result<Option<String>, EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert(
        "start_event_id".to_string(),
        start_event_id.to_string().into(),
    );
    let result = db
        .eval_query_params(
            r#"
?[semantic_hash] :=
    *eval_parent_start { start_event_id, semantic_hash },
    start_event_id = $start_event_id
"#,
            params,
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "query.eval_parent_start.semantic_hash",
            source,
        })?;
    Ok(result.rows.first().and_then(|row| match row.first() {
        Some(DataValue::Str(value)) => Some(value.to_string()),
        _ => None,
    }))
}

fn option_string_param(value: Option<String>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
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

fn hash_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update(part.as_bytes());
        hasher.update([0]);
    }
    sha256_hex(&hasher.finalize())
}

fn sha256_hex(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
}
