use std::{collections::BTreeMap, fs, path::Path};

use chrono::Utc;
use cozo::DataValue;
use sha2::{Digest, Sha256};

use super::{
    cozo_store::{DbEvalStore, EvalDb, mutate_owner_db},
    error::EvalStoreError,
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

pub(crate) const WALK_EVENT_SCHEMA_VERSION: &str = "prototype1-walk-event.v1";

define_eval_schema!(WalkEventSchema {
    "eval_walk_event",
    event_id: "String" =>
    campaign_id: "String",
    schema_version: "String",
    node_id: "String",
    parent_id: "String",
    generation: "Int",
    branch_id: "String",
    command: "String",
    status: "String",
    phase_before: "String?",
    phase_after: "String",
    target_phase: "String?",
    watch: "Bool?",
    allow_git_changes: "Bool?",
    transition_count: "Int",
    protocol_version: "Int",
    transition_graph_version: "String",
    repo_root: "String",
    exe_path: "String",
    exe_sha256: "String",
    exe_modified_unix_ms: "Int?",
    git_head: "String?",
    source_status_hash: "String?",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(WalkEventTransitionSchema {
    "eval_walk_event_transition",
    event_id: "String",
    transition_index: "Int" =>
    campaign_id: "String",
    schema_version: "String",
    transition_label: "String",
});

#[derive(Debug, Clone)]
pub(crate) struct WalkEventEvidence {
    pub(crate) campaign_id: String,
    pub(crate) node_id: String,
    pub(crate) parent_id: String,
    pub(crate) generation: u32,
    pub(crate) branch_id: String,
    pub(crate) command: String,
    pub(crate) status: String,
    pub(crate) phase_before: Option<String>,
    pub(crate) phase_after: String,
    pub(crate) target_phase: Option<String>,
    pub(crate) watch: Option<bool>,
    pub(crate) allow_git_changes: Option<bool>,
    pub(crate) transitions: Vec<String>,
    pub(crate) protocol_version: u32,
    pub(crate) transition_graph_version: String,
    pub(crate) repo_root: String,
    pub(crate) exe_path: String,
    pub(crate) exe_modified_unix_ms: Option<u64>,
    pub(crate) git_head: Option<String>,
    pub(crate) source_status_hash: Option<String>,
    pub(crate) recorded_at: String,
}

pub(super) fn ensure_walk_event_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    WalkEventSchema::SCHEMA.ensure_installed(db, "schema.eval_walk_event")?;
    WalkEventTransitionSchema::SCHEMA.ensure_installed(db, "schema.eval_walk_event_transition")?;
    Ok(())
}

pub(crate) fn write_walk_event_to_owner_db(
    db_path: &Path,
    evidence: WalkEventEvidence,
) -> Result<(), EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        DbEvalStore::new(db).install_schema()?;
        let row = WalkEventRow::from_evidence(evidence)?;
        put_walk_event_row(db, &row)?;
        for (index, transition) in row.transitions.iter().enumerate() {
            put_walk_transition_row(
                db,
                &WalkEventTransitionRow {
                    event_id: row.event_id.clone(),
                    transition_index: usize_to_i64(index, "eval_walk_event_transition.index")?,
                    campaign_id: row.campaign_id.clone(),
                    transition_label: transition.clone(),
                },
            )?;
        }
        Ok(())
    })
}

struct WalkEventRow {
    event_id: String,
    campaign_id: String,
    schema_version: String,
    node_id: String,
    parent_id: String,
    generation: i64,
    branch_id: String,
    command: String,
    status: String,
    phase_before: Option<String>,
    phase_after: String,
    target_phase: Option<String>,
    watch: Option<bool>,
    allow_git_changes: Option<bool>,
    transition_count: i64,
    transitions: Vec<String>,
    protocol_version: i64,
    transition_graph_version: String,
    repo_root: String,
    exe_path: String,
    exe_sha256: String,
    exe_modified_unix_ms: Option<i64>,
    git_head: Option<String>,
    source_status_hash: Option<String>,
    recorded_at: String,
    ingested_at: String,
}

impl WalkEventRow {
    fn from_evidence(evidence: WalkEventEvidence) -> Result<Self, EvalStoreError> {
        require_non_empty("eval_walk_event.campaign_id", &evidence.campaign_id)?;
        require_non_empty("eval_walk_event.node_id", &evidence.node_id)?;
        require_non_empty("eval_walk_event.parent_id", &evidence.parent_id)?;
        require_non_empty("eval_walk_event.branch_id", &evidence.branch_id)?;
        require_non_empty("eval_walk_event.command", &evidence.command)?;
        require_non_empty("eval_walk_event.status", &evidence.status)?;
        require_non_empty("eval_walk_event.phase_after", &evidence.phase_after)?;
        require_non_empty(
            "eval_walk_event.transition_graph_version",
            &evidence.transition_graph_version,
        )?;
        require_non_empty("eval_walk_event.repo_root", &evidence.repo_root)?;
        require_non_empty("eval_walk_event.exe_path", &evidence.exe_path)?;
        let exe_sha256 = file_sha256(Path::new(&evidence.exe_path), "eval_walk_event.exe_path")?;
        let recorded_at = evidence.recorded_at.clone();
        let event_id = event_id(&evidence, &recorded_at, &exe_sha256);
        Ok(Self {
            event_id,
            campaign_id: evidence.campaign_id,
            schema_version: WALK_EVENT_SCHEMA_VERSION.to_string(),
            node_id: evidence.node_id,
            parent_id: evidence.parent_id,
            generation: i64::from(evidence.generation),
            branch_id: evidence.branch_id,
            command: evidence.command,
            status: evidence.status,
            phase_before: evidence.phase_before,
            phase_after: evidence.phase_after,
            target_phase: evidence.target_phase,
            watch: evidence.watch,
            allow_git_changes: evidence.allow_git_changes,
            transition_count: usize_to_i64(
                evidence.transitions.len(),
                "eval_walk_event.transition_count",
            )?,
            transitions: evidence.transitions,
            protocol_version: i64::from(evidence.protocol_version),
            transition_graph_version: evidence.transition_graph_version,
            repo_root: evidence.repo_root,
            exe_path: evidence.exe_path,
            exe_sha256,
            exe_modified_unix_ms: option_u64_to_i64(
                evidence.exe_modified_unix_ms,
                "eval_walk_event.exe_modified_unix_ms",
            )?,
            git_head: evidence.git_head,
            source_status_hash: evidence.source_status_hash,
            recorded_at,
            ingested_at: Utc::now().to_rfc3339(),
        })
    }
}

fn put_walk_event_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &WalkEventRow,
) -> Result<(), EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert("event_id".to_string(), row.event_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert(
        "schema_version".to_string(),
        row.schema_version.clone().into(),
    );
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("parent_id".to_string(), row.parent_id.clone().into());
    params.insert("generation".to_string(), row.generation.into());
    params.insert("branch_id".to_string(), row.branch_id.clone().into());
    params.insert("command".to_string(), row.command.clone().into());
    params.insert("status".to_string(), row.status.clone().into());
    params.insert(
        "phase_before".to_string(),
        option_string_param(row.phase_before.clone()),
    );
    params.insert("phase_after".to_string(), row.phase_after.clone().into());
    params.insert(
        "target_phase".to_string(),
        option_string_param(row.target_phase.clone()),
    );
    params.insert("watch".to_string(), option_bool_param(row.watch));
    params.insert(
        "allow_git_changes".to_string(),
        option_bool_param(row.allow_git_changes),
    );
    params.insert("transition_count".to_string(), row.transition_count.into());
    params.insert("protocol_version".to_string(), row.protocol_version.into());
    params.insert(
        "transition_graph_version".to_string(),
        row.transition_graph_version.clone().into(),
    );
    params.insert("repo_root".to_string(), row.repo_root.clone().into());
    params.insert("exe_path".to_string(), row.exe_path.clone().into());
    params.insert("exe_sha256".to_string(), row.exe_sha256.clone().into());
    params.insert(
        "exe_modified_unix_ms".to_string(),
        option_i64_param(row.exe_modified_unix_ms),
    );
    params.insert(
        "git_head".to_string(),
        option_string_param(row.git_head.clone()),
    );
    params.insert(
        "source_status_hash".to_string(),
        option_string_param(row.source_status_hash.clone()),
    );
    params.insert("recorded_at".to_string(), row.recorded_at.clone().into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    put_eval_params(db, &WalkEventSchema::SCHEMA, params, "put.eval_walk_event")
}

struct WalkEventTransitionRow {
    event_id: String,
    transition_index: i64,
    campaign_id: String,
    transition_label: String,
}

fn put_walk_transition_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &WalkEventTransitionRow,
) -> Result<(), EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert("event_id".to_string(), row.event_id.clone().into());
    params.insert("transition_index".to_string(), row.transition_index.into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert(
        "schema_version".to_string(),
        WALK_EVENT_SCHEMA_VERSION.to_string().into(),
    );
    params.insert(
        "transition_label".to_string(),
        row.transition_label.clone().into(),
    );
    put_eval_params(
        db,
        &WalkEventTransitionSchema::SCHEMA,
        params,
        "put.eval_walk_event_transition",
    )
}

fn event_id(evidence: &WalkEventEvidence, recorded_at: &str, exe_sha256: &str) -> String {
    hash_parts(&[
        WALK_EVENT_SCHEMA_VERSION,
        &evidence.campaign_id,
        &evidence.node_id,
        &evidence.command,
        evidence.phase_before.as_deref().unwrap_or(""),
        &evidence.phase_after,
        evidence.target_phase.as_deref().unwrap_or(""),
        recorded_at,
        exe_sha256,
    ])
}

fn file_sha256(path: &Path, field: &'static str) -> Result<String, EvalStoreError> {
    let bytes = fs::read(path).map_err(|source| EvalStoreError::Io {
        phase: field,
        path: path.to_path_buf(),
        source,
    })?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Ok(sha256_hex(&hasher.finalize()))
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
        detail: format!("value {value} does not fit in Int"),
    })
}

fn option_u64_to_i64(
    value: Option<u64>,
    field: &'static str,
) -> Result<Option<i64>, EvalStoreError> {
    value
        .map(|value| {
            i64::try_from(value).map_err(|_| EvalStoreError::Validation {
                field,
                detail: format!("value {value} does not fit in Int"),
            })
        })
        .transpose()
}

fn option_string_param(value: Option<String>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}

fn option_bool_param(value: Option<bool>) -> DataValue {
    value.map(DataValue::Bool).unwrap_or(DataValue::Null)
}

fn option_i64_param(value: Option<i64>) -> DataValue {
    value.map(DataValue::from).unwrap_or(DataValue::Null)
}
