use super::super::event::{Paths, Refs};
use super::super::invocation::{
    SUCCESSOR_COMPLETION_SCHEMA_VERSION, SUCCESSOR_READY_SCHEMA_VERSION, SuccessorCompletionStatus,
    record_runtime_id,
};
use super::mirror::{channel_message_ref, direction_label};
use super::*;

fn endpoints(root: PathBuf) -> Endpoints {
    Endpoints::new(
        root,
        CampaignId::from("campaign-1"),
        "node-1".to_string(),
        RuntimeId::new(),
    )
}

fn child() -> Child<child::Starting> {
    Child::new(
        PathBuf::from("/tmp/prototype1-test-journal.jsonl"),
        RuntimeId::new(),
        1,
        Refs {
            campaign_id: CampaignId::from("campaign-1"),
            node_id: "node-1".to_string(),
            instance_id: "instance-1".to_string(),
            source_state_id: "source-1".to_string(),
            branch_id: "branch-1".to_string(),
            candidate_id: "candidate-1".to_string(),
            branch_label: "label-1".to_string(),
            spec_id: "spec-1".to_string(),
        },
        Paths {
            repo_root: PathBuf::from("/tmp/repo"),
            workspace_root: PathBuf::from("/tmp/workspace"),
            binary_path: PathBuf::from("/tmp/bin/ploke-eval"),
            target_relpath: PathBuf::from("target.txt"),
            absolute_path: PathBuf::from("/tmp/workspace/target.txt"),
        },
        100,
    )
}

fn channel<R>(endpoints: Endpoints) -> Channel<R, FileTransport> {
    Channel::new(endpoints, FileTransport)
}

fn seed_owner_db(db_path: &Path) {
    std::fs::create_dir_all(db_path.parent().expect("eval db parent"))
        .expect("create eval db parent");
    ploke_db::Database::new_init()
        .expect("empty eval db")
        .write_backup_to_path(db_path)
        .expect("seed owner eval db");
}

#[test]
fn parent_and_child_have_opposite_directions_from_existing_role_states() {
    let temp = tempfile::tempdir().unwrap();
    let endpoints = endpoints(temp.path().join("channels/runtime-1"));
    let child_role = child();
    let parent = channel::<Parent<parent::Selectable>>(endpoints.clone());
    let child = Channel::for_child(&child_role, endpoints, FileTransport);

    parent.send_cancel("test cancellation").unwrap();
    let (child, _) = child.send_ready().unwrap();

    let (_, child_messages) = child.recv_from_parent(Cursor::start()).unwrap();
    let (_, parent_messages) = parent.recv_from_child(Cursor::start()).unwrap();

    assert_eq!(child_messages.len(), 1);
    assert_eq!(child_messages[0].direction(), Direction::ParentToChild);
    assert_eq!(
        child_messages[0].body(),
        &ToChild::Cancel {
            reason: "test cancellation".to_string()
        }
    );
    assert_eq!(parent_messages.len(), 1);
    assert_eq!(parent_messages[0].direction(), Direction::ChildToParent);
    assert!(matches!(parent_messages[0].body(), ToParent::Ready));
}

#[test]
fn prototype1_eval_store_channel_ready_writes_owner_db_row() {
    let temp = tempfile::tempdir().unwrap();
    let prototype1_root = temp.path().join("prototype1");
    let db_path = prototype1_root.join("eval-store.cozo.sqlite");
    seed_owner_db(&db_path);
    let endpoints = endpoints(prototype1_root.join("nodes/node-1/channels/runtime-1"));
    let endpoint = endpoints.child_to_parent();
    let child_role = child();
    let child = Channel::for_child(&child_role, endpoints, FileTransport);

    let (_child, receipt) = child.send_ready().expect("send ready");

    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        cozo::DataValue::from(endpoint.campaign_id().to_string()),
    );
    params.insert(
        "node_id".to_string(),
        cozo::DataValue::from(endpoint.node_id().to_string()),
    );
    params.insert(
        "runtime_id".to_string(),
        cozo::DataValue::from(endpoint.runtime_id().to_string()),
    );
    let rows = db
        .raw_query_params(
            r#"
?[
    direction,
    message_kind,
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
    content_sha256
] :=
    *eval_channel_message {
        campaign_id,
        node_id,
        runtime_id,
        direction,
        message_kind,
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
        content_sha256
    },
    campaign_id = $campaign_id,
    node_id = $node_id,
    runtime_id = $runtime_id
"#,
            params,
        )
        .expect("query channel message rows");

    assert_eq!(rows.rows.len(), 1);
    let row = rows.row_refs().next().expect("channel row");
    assert_eq!(
        row.get::<String>("direction").expect("direction"),
        "child_to_parent"
    );
    assert_eq!(row.get::<String>("message_kind").expect("kind"), "ready");
    assert_eq!(row.get::<String>("store_scope").expect("scope"), "channel");
    assert_eq!(
        row.get::<String>("producer_role").expect("producer"),
        "child"
    );
    assert_eq!(
        row.get::<String>("visibility_scope").expect("visibility"),
        "parent_visible"
    );
    assert_eq!(
        row.get::<String>("source_class").expect("source"),
        "direct_write"
    );
    assert_eq!(
        row.get::<String>("evidence_class").expect("evidence"),
        "channel_message"
    );
    assert_eq!(
        row.get::<String>("validation_status").expect("status"),
        "valid"
    );
    assert_eq!(
        row.get::<String>("endpoint_path").expect("path"),
        receipt.endpoint().display().to_string()
    );
    assert_eq!(
        row.get::<i64>("cursor_offset").expect("cursor"),
        receipt.cursor().offset() as i64
    );
    assert_eq!(
        row.get::<i64>("bytes_written").expect("bytes"),
        receipt.bytes_written() as i64
    );
    assert!(
        !row.get::<String>("body_hash")
            .expect("body hash")
            .is_empty(),
        "channel row carries envelope body hash"
    );
    assert!(
        !row.get::<String>("content_sha256")
            .expect("content hash")
            .is_empty(),
        "channel row carries serialized envelope hash"
    );
}

#[test]
fn prototype1_eval_store_remaining_channel_messages_write_owner_db_rows() {
    let temp = tempfile::tempdir().unwrap();
    let prototype1_root = temp.path().join("prototype1");
    let db_path = prototype1_root.join("eval-store.cozo.sqlite");
    seed_owner_db(&db_path);
    let endpoints = endpoints(prototype1_root.join("nodes/node-1/channels/runtime-1"));
    let endpoint = endpoints.child_to_parent();
    let child_role = child();
    let child = Channel::for_child(&child_role, endpoints.clone(), FileTransport);
    let successor_channel = channel::<Parent<parent::Selectable>>(endpoints);

    let (child, _) = child.send_ready().expect("send ready");
    let (child, _) = child.send_evaluating().expect("send evaluating");
    child
        .send_failed("diagnostic failure")
        .expect("send failed");
    child.send_exited(Some(1)).expect("send exited");
    child
        .send_result_written(PathBuf::from("results/runtime-1.json"))
        .expect("send result-written projection");
    successor_channel
        .send_successor_ready(SuccessorReadyRecord {
            schema_version: SUCCESSOR_READY_SCHEMA_VERSION.to_string(),
            campaign_id: endpoint.campaign_id().clone(),
            node_id: endpoint.node_id().to_string(),
            runtime_id: record_runtime_id(endpoint.runtime_id()),
            pid: 42,
            recorded_at: "0".to_string(),
        })
        .expect("send successor ready");
    successor_channel
        .send_successor_completion(SuccessorCompletionRecord {
            schema_version: SUCCESSOR_COMPLETION_SCHEMA_VERSION.to_string(),
            campaign_id: endpoint.campaign_id().clone(),
            node_id: endpoint.node_id().to_string(),
            runtime_id: record_runtime_id(endpoint.runtime_id()),
            status: SuccessorCompletionStatus::Succeeded,
            trace_path: Some(PathBuf::from("successor-trace.json")),
            detail: None,
            recorded_at: "0".to_string(),
        })
        .expect("send successor completion");

    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        cozo::DataValue::from(endpoint.campaign_id().to_string()),
    );
    params.insert(
        "node_id".to_string(),
        cozo::DataValue::from(endpoint.node_id().to_string()),
    );
    params.insert(
        "runtime_id".to_string(),
        cozo::DataValue::from(endpoint.runtime_id().to_string()),
    );
    let rows = db
        .raw_query_params(
            r#"
?[
    message_kind,
    direction,
    source_class,
    evidence_class,
    validation_status,
    endpoint_path,
    body_hash,
    content_sha256
] :=
    *eval_channel_message {
        campaign_id,
        node_id,
        runtime_id,
        message_kind,
        direction,
        source_class,
        evidence_class,
        validation_status,
        endpoint_path,
        body_hash,
        content_sha256
    },
    campaign_id = $campaign_id,
    node_id = $node_id,
    runtime_id = $runtime_id
"#,
            params,
        )
        .expect("query channel message rows");

    let expected = std::collections::BTreeSet::from([
        "result_written".to_string(),
        "failed".to_string(),
        "exited".to_string(),
        "successor_ready".to_string(),
        "successor_completion".to_string(),
    ]);
    let mut observed = std::collections::BTreeSet::new();
    for row in rows.row_refs() {
        let kind = row.get::<String>("message_kind").expect("kind");
        if expected.contains(&kind) {
            observed.insert(kind);
            assert_eq!(
                row.get::<String>("direction").expect("direction"),
                "child_to_parent"
            );
            assert_eq!(
                row.get::<String>("source_class").expect("source"),
                "direct_write"
            );
            assert_eq!(
                row.get::<String>("evidence_class").expect("evidence"),
                "channel_message"
            );
            assert_eq!(
                row.get::<String>("validation_status").expect("status"),
                "valid"
            );
            assert_eq!(
                row.get::<String>("endpoint_path").expect("path"),
                endpoint.path().display().to_string()
            );
            assert!(!row.get::<String>("body_hash").expect("body").is_empty());
            assert!(
                !row.get::<String>("content_sha256")
                    .expect("content")
                    .is_empty()
            );
        }
    }
    assert_eq!(observed, expected);
}

#[test]
fn prototype1_eval_store_parent_ready_read_writes_receipt_and_import_rows() {
    let temp = tempfile::tempdir().unwrap();
    let prototype1_root = temp.path().join("prototype1");
    let db_path = prototype1_root.join("eval-store.cozo.sqlite");
    seed_owner_db(&db_path);
    let endpoints = endpoints(prototype1_root.join("nodes/node-1/channels/runtime-1"));
    let endpoint = endpoints.child_to_parent();
    let child_role = child();
    let child = Channel::for_child(&child_role, endpoints.clone(), FileTransport);
    let parent = channel::<Parent<parent::Selectable>>(endpoints);

    child.send_ready().expect("send ready");
    let (_, messages) = parent
        .recv_from_child(Cursor::start())
        .expect("read child channel");

    assert_eq!(messages.len(), 1);
    assert!(matches!(messages[0].body(), ToParent::Ready));

    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let mut params = std::collections::BTreeMap::new();
    params.insert(
        "campaign_id".to_string(),
        cozo::DataValue::from(endpoint.campaign_id().to_string()),
    );
    params.insert(
        "node_id".to_string(),
        cozo::DataValue::from(endpoint.node_id().to_string()),
    );
    params.insert(
        "runtime_id".to_string(),
        cozo::DataValue::from(endpoint.runtime_id().to_string()),
    );
    let receipt_rows = db
        .raw_query_params(
            r#"
?[
    receipt_id,
    message_id,
    observed_by,
    direction,
    validation_status,
    imported_ref
] :=
    *eval_channel_receipt {
        receipt_id,
        message_id,
        campaign_id,
        node_id,
        runtime_id,
        observed_by,
        direction,
        validation_status,
        imported_ref
    },
    campaign_id = $campaign_id,
    node_id = $node_id,
    runtime_id = $runtime_id
"#,
            params.clone(),
        )
        .expect("query channel receipt rows");
    assert_eq!(receipt_rows.rows.len(), 1);
    let receipt_row = receipt_rows.row_refs().next().expect("receipt row");
    let receipt_id = receipt_row.get::<String>("receipt_id").expect("receipt id");
    let message_id = receipt_row.get::<String>("message_id").expect("message id");
    assert_eq!(message_id, messages[0].message_id.to_string());
    assert_eq!(
        receipt_row.get::<String>("observed_by").expect("observer"),
        "parent"
    );
    assert_eq!(
        receipt_row.get::<String>("direction").expect("direction"),
        "child_to_parent"
    );
    assert_eq!(
        receipt_row
            .get::<String>("validation_status")
            .expect("status"),
        "valid"
    );
    assert_eq!(
        receipt_row
            .get::<String>("imported_ref")
            .expect("imported ref"),
        channel_message_ref(&messages[0])
    );

    let import_rows = db
        .raw_query_params(
            r#"
?[
    importer_id,
    source_runtime_id,
    source_scope,
    target_scope,
    evidence_ref,
    receipt_id,
    validation_status
] :=
    *eval_import_event {
        campaign_id,
        importer_id,
        source_runtime_id,
        source_scope,
        target_scope,
        evidence_ref,
        receipt_id,
        validation_status
    },
    campaign_id = $campaign_id
"#,
            params,
        )
        .expect("query import event rows");
    assert_eq!(import_rows.rows.len(), 1);
    let import_row = import_rows.row_refs().next().expect("import row");
    assert_eq!(
        import_row.get::<String>("importer_id").expect("importer"),
        "parent"
    );
    assert_eq!(
        import_row
            .get::<String>("source_runtime_id")
            .expect("source runtime"),
        endpoint.runtime_id().to_string()
    );
    assert_eq!(
        import_row
            .get::<String>("source_scope")
            .expect("source scope"),
        format!("child_runtime:{}", endpoint.runtime_id())
    );
    assert_eq!(
        import_row.get::<String>("target_scope").expect("target"),
        "parent_visible"
    );
    assert_eq!(
        import_row.get::<String>("evidence_ref").expect("evidence"),
        channel_message_ref(&messages[0])
    );
    assert_eq!(
        import_row
            .get::<String>("receipt_id")
            .expect("receipt link"),
        receipt_id
    );
    assert_eq!(
        import_row
            .get::<String>("validation_status")
            .expect("status"),
        "valid"
    );
}

#[test]
fn prototype1_storage_authority_negative_channel_row_cannot_replace_envelope() {
    let temp = tempfile::tempdir().unwrap();
    let prototype1_root = temp.path().join("prototype1");
    let db_path = prototype1_root.join("eval-store.cozo.sqlite");
    let endpoints = endpoints(prototype1_root.join("nodes/node-1/channels/runtime-1"));
    let endpoint = endpoints.child_to_parent();
    eval_store::write_channel_message_to_owner_db(
        &db_path,
        eval_store::ChannelMessageEvidence {
            campaign_id: endpoint.campaign_id().clone(),
            node_id: endpoint.node_id().to_string(),
            runtime_id: endpoint.runtime_id().to_string(),
            direction: direction_label(endpoint.direction()).to_string(),
            message_kind: "ready".to_string(),
            message_id: Uuid::new_v4().to_string(),
            endpoint_path: endpoint.path().to_path_buf(),
            cursor_offset: 1,
            bytes_written: 1,
            body_hash: "missing-envelope-body-hash".to_string(),
            content_sha256: "missing-envelope-content-hash".to_string(),
            recorded_at: "0".to_string(),
        },
    )
    .expect("write channel mirror row");
    let parent = channel::<Parent<parent::Selectable>>(endpoints);

    let (_, messages) = parent
        .recv_from_child(Cursor::start())
        .expect("read child channel");

    assert!(
        messages.is_empty(),
        "eval_channel_message row must not synthesize a channel envelope"
    );
    assert!(!endpoint.path().exists());
    assert!(db_path.is_file());
}

#[test]
fn prototype1_storage_authority_negative_successor_channel_row_cannot_replace_envelope() {
    let temp = tempfile::tempdir().unwrap();
    let prototype1_root = temp.path().join("prototype1");
    let db_path = prototype1_root.join("eval-store.cozo.sqlite");
    let endpoints = endpoints(prototype1_root.join("nodes/node-1/channels/runtime-1"));
    let endpoint = endpoints.child_to_parent();
    eval_store::write_channel_message_to_owner_db(
        &db_path,
        eval_store::ChannelMessageEvidence {
            campaign_id: endpoint.campaign_id().clone(),
            node_id: endpoint.node_id().to_string(),
            runtime_id: endpoint.runtime_id().to_string(),
            direction: direction_label(endpoint.direction()).to_string(),
            message_kind: "successor_ready".to_string(),
            message_id: Uuid::new_v4().to_string(),
            endpoint_path: endpoint.path().to_path_buf(),
            cursor_offset: 1,
            bytes_written: 1,
            body_hash: "missing-successor-body-hash".to_string(),
            content_sha256: "missing-successor-content-hash".to_string(),
            recorded_at: "0".to_string(),
        },
    )
    .expect("write successor channel mirror row");
    let parent = channel::<Parent<parent::Selectable>>(endpoints);

    let (_, messages) = parent
        .recv_from_child(Cursor::start())
        .expect("read child channel");

    assert!(
        messages.is_empty(),
        "successor eval_channel_message row must not synthesize a channel envelope"
    );
    assert!(!endpoint.path().exists());
    assert!(db_path.is_file());
}

#[test]
fn prototype1_storage_authority_negative_receipt_import_rows_cannot_replace_envelope() {
    let temp = tempfile::tempdir().unwrap();
    let prototype1_root = temp.path().join("prototype1");
    let db_path = prototype1_root.join("eval-store.cozo.sqlite");
    let endpoints = endpoints(prototype1_root.join("nodes/node-1/channels/runtime-1"));
    let endpoint = endpoints.child_to_parent();
    let message_id = Uuid::new_v4().to_string();
    let receipt_id = eval_store::write_channel_receipt_to_owner_db(
        &db_path,
        eval_store::ChannelReceiptEvidence {
            campaign_id: endpoint.campaign_id().clone(),
            node_id: endpoint.node_id().to_string(),
            runtime_id: endpoint.runtime_id().to_string(),
            direction: direction_label(endpoint.direction()).to_string(),
            message_id: message_id.clone(),
            observed_by: "parent".to_string(),
            validation_status: "valid".to_string(),
            imported_ref: Some(format!("channel_message:{message_id}")),
            observed_at: "0".to_string(),
        },
    )
    .expect("write channel receipt row");
    eval_store::write_import_event_to_owner_db(
        &db_path,
        eval_store::ImportEventEvidence {
            campaign_id: endpoint.campaign_id().clone(),
            importer_id: "parent".to_string(),
            source_runtime_id: Some(endpoint.runtime_id().to_string()),
            source_scope: format!("child_runtime:{}", endpoint.runtime_id()),
            target_scope: "parent_visible".to_string(),
            evidence_ref: format!("channel_message:{message_id}"),
            receipt_id: Some(receipt_id),
            validation_status: "valid".to_string(),
            imported_at: "0".to_string(),
        },
    )
    .expect("write import row");
    let parent = channel::<Parent<parent::Selectable>>(endpoints);

    let (_, messages) = parent
        .recv_from_child(Cursor::start())
        .expect("read child channel");

    assert!(
        messages.is_empty(),
        "eval_channel_receipt/eval_import_event rows must not synthesize a channel envelope"
    );
    assert!(!endpoint.path().exists());
    assert!(db_path.is_file());
}

#[test]
fn prototype1_storage_authority_negative_bad_body_hash_writes_no_receipt_or_import() {
    let temp = tempfile::tempdir().unwrap();
    let prototype1_root = temp.path().join("prototype1");
    let db_path = prototype1_root.join("eval-store.cozo.sqlite");
    seed_owner_db(&db_path);
    let endpoints = endpoints(prototype1_root.join("nodes/node-1/channels/runtime-1"));
    let endpoint = endpoints.child_to_parent();
    eval_store::write_channel_message_to_owner_db(
        &db_path,
        eval_store::ChannelMessageEvidence {
            campaign_id: endpoint.campaign_id().clone(),
            node_id: endpoint.node_id().to_string(),
            runtime_id: endpoint.runtime_id().to_string(),
            direction: direction_label(endpoint.direction()).to_string(),
            message_kind: "ready".to_string(),
            message_id: Uuid::new_v4().to_string(),
            endpoint_path: endpoint.path().to_path_buf(),
            cursor_offset: 1,
            bytes_written: 1,
            body_hash: "schema-seed-body-hash".to_string(),
            content_sha256: "schema-seed-content-hash".to_string(),
            recorded_at: "0".to_string(),
        },
    )
    .expect("seed owner DB schema");
    let mut envelope = Envelope::new(&endpoint, ToParent::Ready).expect("envelope");
    envelope.body_hash = "wrong-body-hash".to_string();
    let bytes = serde_json::to_vec(&envelope).expect("bad envelope bytes");
    FileTransport
        .append(&endpoint, &bytes)
        .expect("append invalid envelope");
    let parent = channel::<Parent<parent::Selectable>>(endpoints);

    let error = parent
        .recv_from_child(Cursor::start())
        .expect_err("body hash mismatch rejects channel read");

    assert!(matches!(
        error,
        ChannelError::Envelope(EnvelopeError::BodyHash { .. })
    ));
    let db = eval_store::load_owner_eval_database(&db_path).expect("owner eval DB loads");
    let receipt_rows = db
        .raw_query_params(
            r#"
?[
    receipt_id
] :=
    *eval_channel_receipt { receipt_id }
"#,
            std::collections::BTreeMap::new(),
        )
        .expect("query channel receipt rows");
    let import_rows = db
        .raw_query_params(
            r#"
?[
    import_id
] :=
    *eval_import_event { import_id }
"#,
            std::collections::BTreeMap::new(),
        )
        .expect("query import rows");
    assert!(
        receipt_rows.rows.is_empty(),
        "invalid channel envelope must not create a receipt row"
    );
    assert!(
        import_rows.rows.is_empty(),
        "invalid channel envelope must not create an import row"
    );
}

#[test]
fn file_transport_reads_only_new_complete_records() {
    let temp = tempfile::tempdir().unwrap();
    let endpoints = endpoints(temp.path().join("channels/runtime-1"));
    let child_role = child();
    let child = Channel::for_child(&child_role, endpoints.clone(), FileTransport);
    let parent = channel::<Parent<parent::Selectable>>(endpoints);

    let (child, first) = child.send_ready().unwrap();
    child.send_evaluating().unwrap();

    let (_, all_messages) = parent.recv_from_child(Cursor::start()).unwrap();
    let (_, new_messages) = parent.recv_from_child(first.cursor()).unwrap();

    assert_eq!(all_messages.len(), 2);
    assert_eq!(new_messages.len(), 1);
    assert!(matches!(new_messages[0].body(), ToParent::Evaluating));
}

#[test]
fn envelope_validation_rejects_wrong_endpoint_direction() {
    let temp = tempfile::tempdir().unwrap();
    let endpoints = endpoints(temp.path().join("channels/runtime-1"));
    let child_endpoint = endpoints.child_to_parent();
    let parent_endpoint = endpoints.parent_to_child();
    let envelope = Envelope::new(&child_endpoint, ToParent::Ready).unwrap();

    let error = envelope.validate_endpoint(&parent_endpoint).unwrap_err();

    assert!(matches!(error, EnvelopeError::Direction { .. }));
}

#[test]
fn envelope_validation_rejects_wrong_body_hash() {
    let temp = tempfile::tempdir().unwrap();
    let endpoints = endpoints(temp.path().join("channels/runtime-1"));
    let child_endpoint = endpoints.child_to_parent();
    let mut envelope = Envelope::new(&child_endpoint, ToParent::Ready).unwrap();
    envelope.body_hash = "not-the-real-hash".to_string();

    let error = envelope.validate_body_hash().unwrap_err();

    assert!(matches!(error, EnvelopeError::BodyHash { .. }));
}
