//! Immutable, revision-tagged reads of the owner eval database.

use std::{collections::BTreeMap, fs, io::Write, path::PathBuf};

use cozo::DataValue;
use ploke_db::QueryResult;
use ploke_records::ids::CampaignId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    campaign::campaign_manifest_path,
    cli::prototype1_state::eval_store::{load_owner_eval_database, prototype1_eval_store_db_path},
    spec::PrepareError,
};

/// Opaque content identity for the exact database snapshot observed by a read.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ReadRevision(String);

impl ReadRevision {
    pub fn as_str(&self) -> &str {
        &self.0
    }

    fn from_bytes(bytes: &[u8]) -> Self {
        Self(format!("{:x}", Sha256::digest(bytes)))
    }
}

/// Backend-neutral row set produced by one immutable expert query.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbQueryResult {
    pub repo_root: PathBuf,
    pub campaign_id: String,
    pub db_path: PathBuf,
    pub script: String,
    pub revision: ReadRevision,
    pub headers: Vec<String>,
    pub row_count: usize,
    pub rows: Vec<DbQueryRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DbQueryRow {
    pub cells: Vec<serde_json::Value>,
    pub object: serde_json::Value,
}

/// Read one exact owner snapshot, then query an isolated in-memory restore.
pub(crate) fn run_snapshot_query(
    repo_root: PathBuf,
    campaign_id: CampaignId,
    script: String,
) -> Result<DbQueryResult, PrepareError> {
    let manifest = campaign_manifest_path(&campaign_id)?;
    let db_path = prototype1_eval_store_db_path(&manifest);
    let bytes = fs::read(&db_path).map_err(|source| PrepareError::DatabaseSetup {
        phase: "prototype1_state_walk_db_query_snapshot",
        detail: format!(
            "failed to read owner eval DB snapshot '{}': {source}",
            db_path.display()
        ),
    })?;
    let revision = ReadRevision::from_bytes(&bytes);
    let mut snapshot =
        tempfile::NamedTempFile::new().map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_db_query_temp",
            detail: format!("failed to create isolated query snapshot: {source}"),
        })?;
    snapshot
        .write_all(&bytes)
        .and_then(|()| snapshot.flush())
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_db_query_temp",
            detail: format!("failed to materialize isolated query snapshot: {source}"),
        })?;
    let db = load_owner_eval_database(snapshot.path()).map_err(|source| {
        PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_db_query_open",
            detail: source.to_string(),
        }
    })?;
    let result = db
        .raw_query_params(&script, BTreeMap::new())
        .map_err(|source| PrepareError::DatabaseSetup {
            phase: "prototype1_state_walk_db_query_run",
            detail: source.to_string(),
        })?;
    Ok(query_result_view(
        repo_root,
        campaign_id,
        db_path,
        script,
        revision,
        &result,
    ))
}

fn query_result_view(
    repo_root: PathBuf,
    campaign_id: CampaignId,
    db_path: PathBuf,
    script: String,
    revision: ReadRevision,
    result: &QueryResult,
) -> DbQueryResult {
    let rows = result
        .rows
        .iter()
        .map(|row| {
            let cells = row.iter().map(data_value_json).collect::<Vec<_>>();
            let object = query_row_object(&result.headers, row);
            DbQueryRow { cells, object }
        })
        .collect::<Vec<_>>();
    DbQueryResult {
        repo_root,
        campaign_id: campaign_id.to_string(),
        db_path,
        script,
        revision,
        headers: result.headers.clone(),
        row_count: rows.len(),
        rows,
    }
}

fn query_row_object(headers: &[String], row: &[DataValue]) -> serde_json::Value {
    let mut object = serde_json::Map::new();
    for (header, value) in headers.iter().zip(row.iter()) {
        object.insert(header.clone(), data_value_json(value));
    }
    serde_json::Value::Object(object)
}

fn data_value_json(value: &DataValue) -> serde_json::Value {
    match value {
        DataValue::Bot => serde_json::json!({ "cozo": "bot" }),
        other => serde_json::Value::from(other.clone()),
    }
}

#[cfg(test)]
mod tests {
    use std::{ffi::OsString, fs, path::Path};

    use super::*;

    #[test]
    fn query_revision_tracks_exact_owner_snapshot_bytes() {
        let tmp = tempfile::tempdir().expect("temp eval home");
        let _env = crate::test_support::env_guard_os(vec![(
            "PLOKE_EVAL_HOME",
            OsString::from(tmp.path()),
        )]);
        let campaign = CampaignId::from("walk-query-revision");
        let manifest = campaign_manifest_path(&campaign).expect("campaign manifest path");
        let db_path = prototype1_eval_store_db_path(&manifest);
        publish_fixture(&db_path, 1);

        let first =
            run_snapshot_query(tmp.path().join("parent"), campaign.clone(), fixture_query())
                .expect("first immutable query");
        let repeated =
            run_snapshot_query(tmp.path().join("parent"), campaign.clone(), fixture_query())
                .expect("repeated immutable query");
        assert_eq!(first.revision, repeated.revision);
        assert_eq!(first.rows[0].object, serde_json::json!({"value": 1}));

        publish_fixture(&db_path, 2);
        let changed = run_snapshot_query(tmp.path().join("parent"), campaign, fixture_query())
            .expect("changed immutable query");
        assert_ne!(first.revision, changed.revision);
        assert_eq!(changed.rows[0].object, serde_json::json!({"value": 2}));
    }

    fn fixture_query() -> String {
        "?[value] := *walk_query_fixture { key: \"answer\", value }".to_string()
    }

    fn publish_fixture(path: &Path, value: i64) {
        fs::create_dir_all(path.parent().expect("owner DB parent")).expect("owner DB directory");
        let db = ploke_db::Database::new_init().expect("fixture database");
        db.raw_query_mut(":create walk_query_fixture { key: String => value: Int }")
            .expect("create fixture relation");
        db.raw_query_mut(&format!(
            "?[key, value] <- [[\"answer\", {value}]] :put walk_query_fixture {{ key => value }}"
        ))
        .expect("write fixture row");
        let temp = path.with_extension("next");
        db.write_backup_to_path(&temp).expect("write owner backup");
        fs::rename(temp, path).expect("publish owner backup");
    }
}
