//! Eval-owned emission adapters for shared passive records.
//!
//! `ploke-records` defines inert schemas. This module owns the filesystem write
//! capability used by Prototype 1 producers.

use crate::prelude::*;

use std::sync::{Mutex, OnceLock};

use cozo::{DataValue, DbInstance, ScriptMutability};
use ploke_records::ids::CampaignId;
use ploke_records::record::{Record, RecordFamily, RecordFormat};
use sha2::{Digest, Sha256};

use crate::cli::prototype1_state::eval_store::{RecordRefEvidence, write_record_ref_to_owner_db};
use crate::layout::record_mirror_file_for_record;

const MIRROR_SCHEMA: &str = "prototype1-record-mirror.v1";
const RECORD_RELATION: &str = "prototype1_record";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EmittedRecord {
    pub(crate) path: PathBuf,
    pub(crate) family: RecordFamily,
    pub(crate) schema: &'static str,
    pub(crate) format: RecordFormat,
}

pub(crate) trait EmitRecord<R: Record> {
    type Error;
    type Receipt;

    fn emit(&mut self, record: &R) -> Result<Self::Receipt, Self::Error>;
}

pub(crate) struct JsonRecordFile<'a> {
    path: &'a Path,
}

impl<'a> JsonRecordFile<'a> {
    pub(crate) fn new(path: &'a Path) -> Self {
        Self { path }
    }
}

impl<R> EmitRecord<R> for JsonRecordFile<'_>
where
    R: Record,
{
    type Error = PrepareError;
    type Receipt = EmittedRecord;

    fn emit(&mut self, record: &R) -> Result<Self::Receipt, Self::Error> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
                path: parent.to_path_buf(),
                source,
            })?;
        }
        let bytes = serde_json::to_vec_pretty(record).map_err(PrepareError::Serialize)?;
        fs::write(self.path, &bytes).map_err(|source| PrepareError::WriteManifest {
            path: self.path.to_path_buf(),
            source,
        })?;
        mirror_record::<R>(self.path, &bytes)?;
        Ok(EmittedRecord {
            path: self.path.to_path_buf(),
            family: R::FAMILY,
            schema: R::SCHEMA,
            format: R::FORMAT,
        })
    }
}

pub(crate) fn emit_eval_record_ref_if_owner_db_exists(
    receipt: &EmittedRecord,
    campaign_id: &CampaignId,
    producer_id: &str,
) -> Result<(), PrepareError> {
    let payload_json =
        fs::read_to_string(&receipt.path).map_err(|source| PrepareError::ReadManifest {
            path: receipt.path.clone(),
            source,
        })?;
    emit_eval_record_ref_payload_if_owner_db_exists(
        &receipt.path,
        campaign_id,
        record_family(receipt.family),
        receipt.schema,
        producer_id,
        0,
        1,
        payload_json,
    )
}

pub(crate) fn emit_eval_record_ref_for_jsonl_if_owner_db_exists(
    path: &Path,
    campaign_id: &CampaignId,
    family: &'static str,
    schema_version: &str,
    producer_id: &str,
    source_event_index: usize,
    payload_json: String,
) -> Result<(), PrepareError> {
    let source_event_index =
        i64::try_from(source_event_index).map_err(|_| PrepareError::DatabaseSetup {
            phase: "eval_record_ref_coordinates",
            detail: format!(
                "source event index for '{}' does not fit in i64",
                path.display()
            ),
        })?;
    let source_line =
        source_event_index
            .checked_add(1)
            .ok_or_else(|| PrepareError::DatabaseSetup {
                phase: "eval_record_ref_coordinates",
                detail: format!("source line for '{}' overflowed i64", path.display()),
            })?;
    emit_eval_record_ref_payload_if_owner_db_exists(
        path,
        campaign_id,
        family,
        schema_version,
        producer_id,
        source_event_index,
        source_line,
        payload_json,
    )
}

fn emit_eval_record_ref_payload_if_owner_db_exists(
    path: &Path,
    campaign_id: &CampaignId,
    family: &'static str,
    schema_version: &str,
    producer_id: &str,
    source_event_index: i64,
    source_line: i64,
    payload_json: String,
) -> Result<(), PrepareError> {
    let db_path = owner_eval_db_file_for_record(path)?;
    if !db_path.is_file() {
        return Ok(());
    }
    let evidence = RecordRefEvidence::compatibility_import(
        campaign_id.clone(),
        family,
        schema_version,
        producer_id,
        format!("prototype1-record:{campaign_id}:{}", path.display()),
        source_event_index,
        source_line,
        format!("{}:L{}", path.display(), source_line),
        payload_json,
        Utc::now().timestamp_millis(),
    );
    write_record_ref_to_owner_db(&db_path, evidence).map_err(|source| {
        PrepareError::DatabaseSetup {
            phase: "eval_record_ref_put",
            detail: format!(
                "failed to persist eval record ref for '{}': {source}",
                path.display()
            ),
        }
    })?;
    Ok(())
}

fn owner_eval_db_file_for_record(record_path: &Path) -> Result<PathBuf, PrepareError> {
    let prototype_root = record_path
        .ancestors()
        .find(|ancestor| ancestor.file_name().and_then(|name| name.to_str()) == Some("prototype1"))
        .ok_or_else(|| PrepareError::DatabaseSetup {
            phase: "eval_record_ref_path",
            detail: format!(
                "record '{}' is not under a Prototype 1 record layout",
                record_path.display()
            ),
        })?;
    Ok(prototype_root.join("eval-store.cozo.sqlite"))
}

fn mirror_record<R: Record>(path: &Path, bytes: &[u8]) -> Result<(), PrepareError> {
    let _guard = mirror_write_lock()
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    let db_path = record_mirror_file_for_record(path)?;
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteManifest {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    let db =
        DbInstance::new("sqlite", &db_path, "").map_err(|source| PrepareError::DatabaseSetup {
            phase: "record_mirror_open",
            detail: format!(
                "failed to open Prototype 1 record mirror '{}': {source}",
                db_path.display()
            ),
        })?;
    ensure_mirror_schema(&db)?;

    let payload =
        String::from_utf8(bytes.to_vec()).map_err(|source| PrepareError::DatabaseSetup {
            phase: "record_mirror_payload",
            detail: format!(
                "record '{}' was not valid UTF-8 JSON for mirror persistence: {source}",
                path.display()
            ),
        })?;
    let hash = format!("{:x}", Sha256::digest(bytes));
    let mut params = BTreeMap::new();
    params.insert(
        "family".to_string(),
        DataValue::from(record_family(R::FAMILY)),
    );
    params.insert(
        "record_path".to_string(),
        DataValue::from(path.display().to_string()),
    );
    params.insert("content_sha256".to_string(), DataValue::from(hash));
    params.insert("schema_version".to_string(), DataValue::from(R::SCHEMA));
    params.insert(
        "record_format".to_string(),
        DataValue::from(record_format(R::FORMAT)),
    );
    params.insert("mirror_schema".to_string(), DataValue::from(MIRROR_SCHEMA));
    params.insert(
        "recorded_at".to_string(),
        DataValue::from(Utc::now().to_rfc3339()),
    );
    params.insert("payload_json".to_string(), DataValue::from(payload));
    db.run_script(
        r#"
        ?[
            family,
            record_path,
            content_sha256,
            schema_version,
            record_format,
            mirror_schema,
            recorded_at,
            payload_json
        ] :=
            family = $family,
            record_path = $record_path,
            content_sha256 = $content_sha256,
            schema_version = $schema_version,
            record_format = $record_format,
            mirror_schema = $mirror_schema,
            recorded_at = $recorded_at,
            payload_json = $payload_json
        :put prototype1_record {
            family,
            record_path,
            content_sha256 =>
            schema_version,
            record_format,
            mirror_schema,
            recorded_at,
            payload_json
        }
        "#,
        params,
        ScriptMutability::Mutable,
    )
    .map_err(|source| PrepareError::DatabaseSetup {
        phase: "record_mirror_put",
        detail: format!("failed to mirror record '{}': {source}", path.display()),
    })?;
    Ok(())
}

fn mirror_write_lock() -> &'static Mutex<()> {
    static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    LOCK.get_or_init(|| Mutex::new(()))
}

fn ensure_mirror_schema(db: &DbInstance) -> Result<(), PrepareError> {
    if relation_exists(db, RECORD_RELATION)? {
        return Ok(());
    }
    db.run_script(
        r#"
        :create prototype1_record {
            family: String,
            record_path: String,
            content_sha256: String =>
            schema_version: String,
            record_format: String,
            mirror_schema: String,
            recorded_at: String,
            payload_json: String
        }
        "#,
        BTreeMap::new(),
        ScriptMutability::Mutable,
    )
    .map_err(|source| PrepareError::DatabaseSetup {
        phase: "record_mirror_schema",
        detail: format!("failed to create Prototype 1 record mirror schema: {source}"),
    })?;
    Ok(())
}

fn relation_exists(db: &DbInstance, relation: &str) -> Result<bool, PrepareError> {
    let rows = db
        .run_script("::relations", BTreeMap::new(), ScriptMutability::Immutable)
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "record_mirror_relations",
            detail: format!("failed to inspect Prototype 1 record mirror schema: {source}"),
        })?
        .rows;
    Ok(rows.iter().any(|row| {
        row.first().and_then(|value| match value {
            DataValue::Str(value) => Some(value.as_str()),
            _ => None,
        }) == Some(relation)
    }))
}

pub(crate) fn record_family(family: RecordFamily) -> &'static str {
    match family {
        RecordFamily::ChildPlan => "child_plan",
        RecordFamily::SchedulerState => "scheduler_state",
        RecordFamily::SchedulerNode => "scheduler_node",
        RecordFamily::RunnerRequest => "runner_request",
        RecordFamily::RunnerResult => "runner_result",
        RecordFamily::ClosureState => "closure_state",
        RecordFamily::EvaluationArtifact => "evaluation_artifact",
        RecordFamily::ProtocolArtifact => "protocol_artifact",
        RecordFamily::RunProfile => "run_profile",
        RecordFamily::RunProfileCommitment => "run_profile_commitment",
        RecordFamily::AgentTurnTrace => "agent_turn_trace",
        RecordFamily::AgentTurnSummary => "agent_turn_summary",
        RecordFamily::LlmFullResponseTrace => "llm_full_response_trace",
        RecordFamily::RunRecord => "run_record",
    }
}

fn record_format(format: RecordFormat) -> &'static str {
    match format {
        RecordFormat::Json => "json",
        RecordFormat::JsonLines => "jsonl",
        RecordFormat::Toml => "toml",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(serde::Serialize)]
    struct MirrorTestRecord {
        schema_version: &'static str,
        value: &'static str,
    }

    impl Record for MirrorTestRecord {
        const FAMILY: RecordFamily = RecordFamily::SchedulerState;
        const SCHEMA: &'static str = "mirror-test-record.v1";
        const FORMAT: RecordFormat = RecordFormat::Json;
    }

    #[test]
    fn json_record_emission_writes_parallel_cozo_mirror() {
        let temp = tempfile::tempdir().expect("tempdir");
        let record_path = temp
            .path()
            .join("prototype1")
            .join("scheduler")
            .join("test-record.json");
        let record = MirrorTestRecord {
            schema_version: "mirror-test-record.v1",
            value: "ok",
        };

        let receipt = JsonRecordFile::new(&record_path)
            .emit(&record)
            .expect("record emits");

        assert_eq!(receipt.path, record_path);
        assert_eq!(receipt.family, RecordFamily::SchedulerState);
        assert_eq!(receipt.schema, "mirror-test-record.v1");
        assert_eq!(receipt.format, RecordFormat::Json);
        let mirror_path = temp.path().join("records").join("mirror.cozo.sqlite");
        assert!(mirror_path.exists());

        let db = DbInstance::new("sqlite", &mirror_path, "").expect("open mirror");
        let rows = db
            .run_script(
                r#"
                ?[
                    family,
                    record_path,
                    content_sha256,
                    schema_version,
                    record_format,
                    mirror_schema,
                    recorded_at,
                    payload_json
                ] :=
                    *prototype1_record {
                        family,
                        record_path,
                        content_sha256,
                        schema_version,
                        record_format,
                        mirror_schema,
                        recorded_at,
                        payload_json
                    }
                "#,
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .expect("query mirror")
            .rows;

        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(data_str(row, 0), "scheduler_state");
        assert_eq!(data_str(row, 1), record_path.display().to_string());
        assert_eq!(
            data_str(row, 2),
            format!(
                "{:x}",
                Sha256::digest(fs::read(&record_path).expect("record bytes"))
            )
        );
        assert_eq!(data_str(row, 3), "mirror-test-record.v1");
        assert_eq!(data_str(row, 4), "json");
        assert_eq!(data_str(row, 5), MIRROR_SCHEMA);
        assert!(!data_str(row, 6).is_empty());
        assert!(data_str(row, 7).contains("\"value\": \"ok\""));
    }

    #[test]
    fn prototype1_record_emission_uses_local_campaign_mirror() {
        let temp = tempfile::tempdir().expect("tempdir");
        let record_path = temp
            .path()
            .join("prototype1")
            .join("nodes")
            .join("node-a")
            .join("node.json");
        let record = MirrorTestRecord {
            schema_version: "mirror-test-record.v1",
            value: "local",
        };

        JsonRecordFile::new(&record_path)
            .emit(&record)
            .expect("record emits");

        let local_mirror = temp.path().join("records").join("mirror.cozo.sqlite");
        assert!(
            local_mirror.exists(),
            "prototype1 temp records should mirror locally at {}",
            local_mirror.display()
        );

        let db = DbInstance::new("sqlite", &local_mirror, "").expect("open local mirror");
        let rows = db
            .run_script(
                r#"
                ?[record_path, payload_json] :=
                    *prototype1_record {
                        family,
                        record_path,
                        content_sha256,
                        schema_version,
                        record_format,
                        mirror_schema,
                        recorded_at,
                        payload_json
                    }
                "#,
                BTreeMap::new(),
                ScriptMutability::Immutable,
            )
            .expect("query local mirror")
            .rows;

        assert_eq!(rows.len(), 1);
        assert_eq!(data_str(&rows[0], 0), record_path.display().to_string());
        assert!(data_str(&rows[0], 1).contains("\"value\": \"local\""));
    }

    fn data_str(row: &[DataValue], index: usize) -> String {
        match &row[index] {
            DataValue::Str(value) => value.as_str().to_string(),
            other => panic!("expected string at {index}, got {other:?}"),
        }
    }
}
