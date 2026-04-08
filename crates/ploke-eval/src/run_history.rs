use std::fs;
use std::path::{Path, PathBuf};

use chrono::Utc;
use ploke_db::ObservabilityStore;
use serde::{Deserialize, Serialize};

use crate::spec::PrepareError;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LastRunRecord {
    pub run_dir: PathBuf,
    pub completed_at_ms: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct SnapshotStatusRecord {
    snapshot_file: Option<PathBuf>,
}

pub fn record_last_run(run_dir: impl AsRef<Path>) -> Result<(), PrepareError> {
    record_last_run_at(crate::layout::ploke_eval_home()?, run_dir)
}

pub fn load_last_run() -> Result<LastRunRecord, PrepareError> {
    load_last_run_at(crate::layout::ploke_eval_home()?)
}

pub async fn print_last_run_assistant_messages() -> Result<(), PrepareError> {
    let record = load_last_run()?;
    let snapshot_path = load_final_snapshot_path(&record.run_dir)?;
    let db = ploke_db::Database::create_new_backup_default(&snapshot_path)
        .await
        .map_err(|err| PrepareError::DatabaseSetup {
            phase: "load_last_run_snapshot",
            detail: err.to_string(),
        })?;

    let turns = db.list_conversation_since(0, 100_000)?;
    for turn in turns.into_iter().filter(|turn| turn.kind == "assistant") {
        println!("{}", turn.content);
    }
    Ok(())
}

pub fn record_last_run_at(
    eval_home: impl AsRef<Path>,
    run_dir: impl AsRef<Path>,
) -> Result<(), PrepareError> {
    let eval_home = eval_home.as_ref();
    let path = eval_home.join("last-run.json");
    let run_dir = run_dir.as_ref();
    let record = LastRunRecord {
        run_dir: run_dir.to_path_buf(),
        completed_at_ms: Utc::now().timestamp_millis(),
    };
    let json = serde_json::to_string_pretty(&record).map_err(PrepareError::Serialize)?;

    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| PrepareError::WriteLastRunRecord {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(&path, json).map_err(|source| PrepareError::WriteLastRunRecord { path, source })
}

pub fn load_last_run_at(eval_home: impl AsRef<Path>) -> Result<LastRunRecord, PrepareError> {
    let eval_home = eval_home.as_ref();
    let path = eval_home.join("last-run.json");
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text)
            .map_err(|source| PrepareError::ParseLastRunRecord { path, source }),
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => {
            discover_last_run_dir(eval_home)?
                .map(|run_dir| LastRunRecord {
                    run_dir,
                    completed_at_ms: 0,
                })
                .ok_or(PrepareError::MissingLastRun(eval_home.join("runs")))
        }
        Err(source) => Err(PrepareError::ReadLastRunRecord { path, source }),
    }
}

fn load_final_snapshot_path(run_dir: &Path) -> Result<PathBuf, PrepareError> {
    let status_path = run_dir.join("snapshot-status.json");
    let text = fs::read_to_string(&status_path).map_err(|source| PrepareError::ReadManifest {
        path: status_path.clone(),
        source,
    })?;
    let status: SnapshotStatusRecord =
        serde_json::from_str(&text).map_err(|source| PrepareError::ParseManifest {
            path: status_path.clone(),
            source,
        })?;
    if let Some(snapshot_file) = status.snapshot_file.filter(|path| path.exists()) {
        return Ok(snapshot_file);
    }
    let fallback = run_dir.join("final-snapshot.db");
    if fallback.exists() {
        Ok(fallback)
    } else {
        Err(PrepareError::MissingFinalSnapshot(run_dir.to_path_buf()))
    }
}

fn discover_last_run_dir(eval_home: impl AsRef<Path>) -> Result<Option<PathBuf>, PrepareError> {
    let root = eval_home.as_ref().join("runs");
    let mut best: Option<(std::time::SystemTime, PathBuf)> = None;

    let entries = match fs::read_dir(&root) {
        Ok(entries) => entries,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => {
            return Err(PrepareError::ReadLastRunRecord { path: root, source });
        }
    };

    for entry in entries {
        let entry = match entry {
            Ok(entry) => entry,
            Err(_) => continue,
        };
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }

        let snapshot_status = path.join("snapshot-status.json");
        let final_snapshot = path.join("final-snapshot.db");
        if !snapshot_status.exists() || !final_snapshot.exists() {
            continue;
        }

        let modified = fs::metadata(&snapshot_status)
            .and_then(|meta| meta.modified())
            .or_else(|_| fs::metadata(&final_snapshot).and_then(|meta| meta.modified()));
        let modified = match modified {
            Ok(modified) => modified,
            Err(_) => continue,
        };

        if best
            .as_ref()
            .is_none_or(|(best_time, _)| modified > *best_time)
        {
            best = Some((modified, path));
        }
    }

    Ok(best.map(|(_, path)| path))
}

#[cfg(test)]
mod tests {
    use super::*;
    use ploke_db::multi_embedding::db_ext::EmbeddingExt;
    use ploke_db::{Database, ObservabilityStore, observability::ConversationTurn};
    use tempfile::tempdir;
    use uuid::Uuid;

    #[tokio::test]
    async fn prints_only_assistant_messages_from_last_run() {
        let tmp = tempdir().expect("tempdir");
        let eval_home = tmp.path().join("eval-home");
        let runs_root = eval_home.join("runs");
        let run_dir = runs_root.join("demo-run");
        std::fs::create_dir_all(&run_dir).expect("run dir");

        let db = Database::init_with_schema().expect("init db");
        db.setup_multi_embedding().expect("setup multi embedding");

        let user_id = Uuid::new_v4();
        let assistant_id = Uuid::new_v4();
        db.upsert_conversation_turn(ConversationTurn {
            id: user_id,
            parent_id: None,
            message_id: user_id,
            kind: "user".to_string(),
            content: "user".to_string(),
            created_at: ploke_db::observability::Validity {
                at: 1,
                is_valid: true,
            },
            thread_id: None,
        })
        .expect("user turn");
        db.upsert_conversation_turn(ConversationTurn {
            id: assistant_id,
            parent_id: Some(user_id),
            message_id: assistant_id,
            kind: "assistant".to_string(),
            content: "assistant one".to_string(),
            created_at: ploke_db::observability::Validity {
                at: 2,
                is_valid: true,
            },
            thread_id: None,
        })
        .expect("assistant turn");

        let snapshot_path = run_dir.join("final-snapshot.db");
        db.backup_db(snapshot_path.clone()).expect("backup db");

        std::fs::write(
            run_dir.join("snapshot-status.json"),
            serde_json::to_string_pretty(&serde_json::json!({
                "status": "completed",
                "snapshot_file": snapshot_path,
                "registry_file": run_dir.join("final-snapshot.db"),
                "config_home": run_dir.join("config")
            }))
            .expect("serialize snapshot status"),
        )
        .expect("write snapshot status");

        record_last_run_at(&eval_home, &run_dir).expect("record last run");
        let record = load_last_run_at(&eval_home).expect("load last run");
        assert_eq!(record.run_dir, run_dir);

        let snapshot_path = load_final_snapshot_path(&record.run_dir).expect("load snapshot path");
        let db = Database::create_new_backup_default(&snapshot_path)
            .await
            .expect("restore snapshot");
        let turns = db.list_conversation_since(0, 100).expect("list turns");
        let assistant_turns: Vec<_> = turns
            .into_iter()
            .filter(|turn| turn.kind == "assistant")
            .map(|turn| turn.content)
            .collect();
        assert_eq!(assistant_turns, vec!["assistant one".to_string()]);
    }
}
