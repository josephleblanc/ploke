use std::{collections::BTreeMap, fs, path::PathBuf};

use cozo::DataValue;
use ploke_db::{Database, QueryResult};

use ploke_records::{identity::ParentIdentityRecord, ids::CampaignId};
use sha2::{Digest, Sha256};

use super::*;
use super::{
    ARTIFACT_REF_REL, ARTIFACT_REL, ARTIFACT_SURFACE_REL, BINARY_REF_REL, BUILD_EVENT_REL,
    CONTINUATION_DECISION_REL, EVALUATION_INSTANCE_REL, EVALUATION_REL, SELECTION_CANDIDATE_REL,
    SELECTION_DECISION_REL, SELECTION_FINDING_REL, SELECTION_SCORE_REL,
    api::EvalStorageMode,
    cozo_schema::eval_relation_exists,
    error::EvalStoreError,
    evidence::{
        ATTEMPT_REL, EVENT_REL, LOG_REF_REL, PARENT_STARTED_OUTCOME, PARENT_STARTED_PHASE,
        PARENT_STARTED_TRANSITION, RECORD_REL, STORE_SCOPE, TRACE_EVENT_REL,
        parent_started_db_receipt,
    },
};
use crate::cli::prototype1_state::{
    event::RecordedAt,
    identity::{PARENT_IDENTITY_SCHEMA_VERSION, ParentIdentity},
    journal::{self, JournalAppendReceipt, JournalEntry, ParentStartedEntry, PrototypeJournal},
};
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
    assert!(eval_relation_exists(&db, ATTEMPT_REL).expect("attempt rel exists"));
    assert!(eval_relation_exists(&db, EVALUATION_REL).expect("evaluation rel exists"));
    assert!(
        eval_relation_exists(&db, EVALUATION_INSTANCE_REL).expect("evaluation instance rel exists")
    );
    assert!(
        eval_relation_exists(&db, CONTINUATION_DECISION_REL)
            .expect("continuation decision rel exists")
    );
    assert!(
        eval_relation_exists(&db, SELECTION_DECISION_REL).expect("selection decision rel exists")
    );
    assert!(
        eval_relation_exists(&db, SELECTION_CANDIDATE_REL).expect("selection candidate rel exists")
    );
    assert!(
        eval_relation_exists(&db, SELECTION_FINDING_REL).expect("selection finding rel exists")
    );
    assert!(eval_relation_exists(&db, SELECTION_SCORE_REL).expect("selection score rel exists"));
    assert!(eval_relation_exists(&db, ARTIFACT_REL).expect("artifact rel exists"));
    assert!(eval_relation_exists(&db, ARTIFACT_SURFACE_REL).expect("artifact surface rel exists"));
    assert!(eval_relation_exists(&db, ARTIFACT_REF_REL).expect("artifact ref rel exists"));
    assert!(eval_relation_exists(&db, BINARY_REF_REL).expect("binary ref rel exists"));
    assert!(eval_relation_exists(&db, BUILD_EVENT_REL).expect("build event rel exists"));
    assert!(eval_relation_exists(&db, RECORD_REL).expect("record rel exists"));
    assert!(eval_relation_exists(&db, LOG_REF_REL).expect("log rel exists"));
    assert!(eval_relation_exists(&db, TRACE_EVENT_REL).expect("trace rel exists"));
}

#[test]
fn prototype1_eval_store_trace_log_ref_round_trips_row() {
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);

    let receipt = store
        .put_log_ref(LogRefEvidence {
            campaign_id: Some(CampaignId::from("campaign")),
            runtime_id: None,
            store_scope: STORE_SCOPE.to_string(),
            log_kind: "observation_jsonl".to_string(),
            source_ref: "/tmp/prototype1-observation.jsonl".to_string(),
            byte_start: Some(0),
            byte_len: Some(12),
            content_sha256: Some("abc123".to_string()),
            sensitivity: Some("internal_diagnostic".to_string()),
            recorded_at: Some("2026-06-23T00:00:00Z".to_string()),
        })
        .expect("log ref writes");

    let refs = query_log_refs(&db);
    assert_eq!(refs.rows.len(), 1);
    let row = refs.row_refs().next().expect("log ref row");
    assert_eq!(
        row.get::<String>("log_ref_id").expect("id"),
        receipt.log_ref_id
    );
    assert_eq!(
        row.get::<String>("log_kind").expect("kind"),
        "observation_jsonl"
    );
    assert_eq!(
        row.get::<String>("source_ref").expect("source"),
        "/tmp/prototype1-observation.jsonl"
    );
    assert_eq!(row.get::<String>("content_sha256").expect("hash"), "abc123");
}

#[test]
fn prototype1_eval_store_record_ref_compatibility_import_round_trips_axes() {
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    let payload = r#"{"schema_version":"prototype1-node.v1","node_id":"node-1"}"#;

    let receipt = store
        .put_record_ref(RecordRefEvidence::compatibility_import(
            CampaignId::from("campaign"),
            "scheduler_node",
            "prototype1-node.v1",
            "parent",
            "prototype1-record:campaign",
            7,
            8,
            "/tmp/prototype1/nodes/node-1/node.json:L8",
            payload,
            1234,
        ))
        .expect("compatibility record ref writes");

    let refs = query_record_refs(&db, &CampaignId::from("campaign"));
    assert_eq!(refs.rows.len(), 1);
    let row = refs.row_refs().next().expect("record ref row");
    assert_eq!(
        row.get::<String>("record_ref_id").expect("id"),
        receipt.record_ref_id
    );
    assert_eq!(
        row.get::<String>("content_sha256").expect("hash"),
        receipt.content_sha256
    );
    assert_eq!(
        row.get::<String>("family").expect("family"),
        "scheduler_node"
    );
    assert_eq!(
        row.get::<String>("schema_version").expect("schema"),
        "prototype1-node.v1"
    );
    assert_eq!(row.get::<String>("store_scope").expect("scope"), "parent");
    assert_eq!(row.get::<String>("producer_role").expect("role"), "parent");
    assert_eq!(
        row.get::<String>("source_class").expect("source"),
        "compatibility_import"
    );
    assert_eq!(
        row.get::<String>("evidence_class").expect("evidence"),
        "compatibility"
    );
    assert_eq!(
        row.get::<String>("visibility_scope").expect("visibility"),
        "parent_visible"
    );
    assert_eq!(
        row.get::<String>("validation_status").expect("status"),
        "valid"
    );
    assert_eq!(row.get::<String>("payload_json").expect("payload"), payload);
    assert!(
        query_all_transition_events(&db).rows.is_empty(),
        "compatibility record refs must not fabricate transition authority"
    );
}

#[test]
fn prototype1_eval_store_record_ref_duplicate_identical_is_idempotent() {
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    let evidence = RecordRefEvidence::compatibility_import(
        CampaignId::from("campaign"),
        "scheduler_node",
        "prototype1-node.v1",
        "parent",
        "prototype1-record:campaign",
        7,
        8,
        "/tmp/prototype1/nodes/node-1/node.json:L8",
        r#"{"schema_version":"prototype1-node.v1","node_id":"node-1"}"#,
        1234,
    );

    let first = store
        .put_record_ref(evidence.clone())
        .expect("first record ref");
    let second = store.put_record_ref(evidence).expect("same record ref");

    assert_eq!(second, first);
    assert_eq!(
        query_record_refs(&db, &CampaignId::from("campaign"))
            .rows
            .len(),
        1
    );
}

#[test]
fn prototype1_eval_store_record_ref_missing_axis_fails_without_rows() {
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    store.install_schema().expect("schema");
    let evidence = RecordRefEvidence::compatibility_import(
        CampaignId::from("campaign"),
        "",
        "prototype1-node.v1",
        "parent",
        "prototype1-record:campaign",
        7,
        8,
        "/tmp/prototype1/nodes/node-1/node.json:L8",
        r#"{"schema_version":"prototype1-node.v1","node_id":"node-1"}"#,
        1234,
    );

    let err = store
        .put_record_ref(evidence)
        .expect_err("missing family fails");

    match err {
        EvalStoreError::Validation { field, detail } => {
            assert_eq!(field, "record_ref.family");
            assert!(detail.contains("required eval-store field"));
        }
        other => panic!("unexpected record ref validation error: {other:?}"),
    }
    assert!(
        query_record_refs(&db, &CampaignId::from("campaign"))
            .rows
            .is_empty()
    );
}

#[test]
fn prototype1_eval_store_trace_observation_jsonl_imports_rows_idempotently() {
    let tmp = tempfile::tempdir().expect("tmp");
    let log_path = tmp.path().join("prototype1-observation.jsonl");
    fs::write(
            &log_path,
            concat!(
                r#"{"timestamp":"2026-06-23T00:00:00Z","target":"ploke_exec","level":"INFO","event":"typestate_transition","role":"parent","pipeline":"prototype1.child_plan_authority","phase":"typestate_transition","transition":"R7->R8","outcome":"committed","campaign_id":"campaign","parent_id":"parent","node_id":"parent","generation":0,"branch_id":"main","record_access":"write","record_kind":"child_plan_file","record_path":"prototype1/messages/child-plan.json","record_index":0,"record_count":1,"duration_ms":17}"#,
                "\n",
                r#"{"timestamp":"2026-06-23T00:00:01Z","target":"ploke_exec","level":"INFO","span":{"name":"child-build"},"outcome":"rejected","program":"cargo","exit_code":101,"duration_ms":22}"#,
                "\n"
            ),
        )
        .expect("write observation jsonl");
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    let import = ObservationJsonlImport {
        campaign_id: Some(CampaignId::from("campaign")),
        path: log_path.clone(),
    };

    let first = store
        .import_observation_jsonl(import.clone())
        .expect("import observation jsonl");
    let second = store
        .import_observation_jsonl(import)
        .expect("reimport observation jsonl");

    assert_eq!(second, first);
    assert_eq!(query_log_refs(&db).rows.len(), 1);
    let traces = query_trace_events(&db, &first.log_ref_id);
    assert_eq!(traces.rows.len(), 2);
    let mut by_index = std::collections::BTreeMap::new();
    for row in traces.row_refs() {
        by_index.insert(
            row.get::<i64>("source_event_index").expect("index"),
            (
                row.get::<String>("event_name").ok(),
                row.get::<String>("stage").ok(),
                row.get::<String>("transition").ok(),
                row.get::<String>("span_name").ok(),
                row.get::<String>("program").ok(),
                row.get::<i64>("exit_code").ok(),
            ),
        );
    }
    assert_eq!(
        by_index.get(&0).expect("first trace").0.as_deref(),
        Some("typestate_transition")
    );
    assert_eq!(
        by_index.get(&0).expect("first trace").1.as_deref(),
        Some("typestate_transition")
    );
    assert_eq!(
        by_index.get(&0).expect("first trace").2.as_deref(),
        Some("R7->R8")
    );
    assert_eq!(
        by_index.get(&1).expect("second trace").3.as_deref(),
        Some("child-build")
    );
    assert_eq!(
        by_index.get(&1).expect("second trace").4.as_deref(),
        Some("cargo")
    );
    assert_eq!(by_index.get(&1).expect("second trace").5, Some(101));
}

#[test]
fn prototype1_eval_store_trace_observation_jsonl_invalid_line_fails_without_rows() {
    let tmp = tempfile::tempdir().expect("tmp");
    let log_path = tmp.path().join("bad-observation.jsonl");
    fs::write(
        &log_path,
        concat!(
            r#"{"target":"ploke_exec","level":"INFO","event":"typestate_transition"}"#,
            "\n",
            "not json\n"
        ),
    )
    .expect("write bad observation jsonl");
    let db = Database::new_init().expect("db");
    let store = DbEvalStore::new(&db);
    store
        .install_schema()
        .expect("schema for empty-row assertions");

    let err = store
        .import_observation_jsonl(ObservationJsonlImport {
            campaign_id: Some(CampaignId::from("campaign")),
            path: log_path,
        })
        .expect_err("invalid jsonl fails loudly");

    match err {
        EvalStoreError::Validation { field, detail } => {
            assert_eq!(field, "observation_jsonl.line");
            assert!(detail.contains("invalid JSONL line 2"), "{detail}");
        }
        other => panic!("unexpected import error: {other:?}"),
    }
    assert!(query_log_refs(&db).rows.is_empty());
    assert!(query_all_trace_events(&db).rows.is_empty());
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

fn parent_started_fixture(root: &std::path::Path) -> (ParentStartedEvidence, ParentStartedReceipt) {
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
    hex_lower_for_test(&hasher.finalize())
}

fn hex_lower_for_test(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0f) as usize] as char);
    }
    out
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
?[
    record_ref_id,
    family,
    schema_version,
    store_scope,
    producer_role,
    source_class,
    evidence_class,
    visibility_scope,
    validation_status,
    source_event_index,
    source_line,
    content_sha256,
    payload_json
] :=
    *eval_record_ref {
        record_ref_id,
        campaign_id,
        family,
        schema_version,
        store_scope,
        producer_role,
        source_class,
        evidence_class,
        visibility_scope,
        validation_status,
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

fn query_log_refs(db: &Database) -> QueryResult {
    db.raw_query_params(
        r#"
?[log_ref_id, log_kind, source_ref, content_sha256] :=
    *eval_log_ref { log_ref_id, log_kind, source_ref, content_sha256 }
"#,
        BTreeMap::new(),
    )
    .expect("query log refs")
}

fn query_trace_events(db: &Database, log_ref_id: &str) -> QueryResult {
    let mut params = BTreeMap::new();
    params.insert(
        "source_log_ref".to_string(),
        DataValue::from(log_ref_id.to_string()),
    );
    db.raw_query_params(
            r#"
?[trace_event_id, source_event_index, event_name, stage, transition, span_name, program, exit_code] :=
    *eval_trace_event {
        trace_event_id,
        source_log_ref,
        source_event_index,
        event_name,
        stage,
        transition,
        span_name,
        program,
        exit_code
    },
    source_log_ref = $source_log_ref
"#,
            params,
        )
        .expect("query trace events")
}

fn query_all_trace_events(db: &Database) -> QueryResult {
    db.raw_query_params(
        r#"
?[trace_event_id] :=
    *eval_trace_event { trace_event_id }
"#,
        BTreeMap::new(),
    )
    .expect("query all trace events")
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
