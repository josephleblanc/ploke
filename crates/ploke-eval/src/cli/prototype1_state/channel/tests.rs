use super::super::event::{Paths, Refs};
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
