use sha2::{Digest, Sha256};

use super::*;

pub(super) fn mirror_channel_message(
    endpoint: &Endpoint,
    envelope: &Envelope<ToParent>,
    bytes: &[u8],
    receipt: &Receipt,
    message_kind: &'static str,
) -> Result<(), eval_store::EvalStoreError> {
    let db_path = match eval_store::owner_eval_db_file_for_record_path(receipt.endpoint()) {
        Ok(path) => path,
        Err(eval_store::EvalStoreError::Validation { .. }) => return Ok(()),
        Err(source) => return Err(source),
    };
    if !db_path.is_file() {
        return Ok(());
    }
    eval_store::write_channel_message_to_owner_db(
        &db_path,
        eval_store::ChannelMessageEvidence {
            campaign_id: endpoint.campaign_id.clone(),
            node_id: endpoint.node_id.clone(),
            runtime_id: endpoint.runtime_id.to_string(),
            direction: direction_label(endpoint.direction).to_string(),
            message_kind: message_kind.to_string(),
            message_id: envelope.message_id.to_string(),
            endpoint_path: receipt.endpoint().to_path_buf(),
            cursor_offset: receipt.cursor().offset() as i64,
            bytes_written: receipt.bytes_written() as i64,
            body_hash: envelope.body_hash.clone(),
            content_sha256: format!("{:x}", Sha256::digest(bytes)),
            recorded_at: envelope.recorded_at.0.to_string(),
        },
    )?;
    Ok(())
}

pub(super) fn mirror_parent_channel_imports(
    endpoint: &Endpoint,
    records: &[ChannelReadRecord<ToParent>],
) -> Result<(), eval_store::EvalStoreError> {
    if records.is_empty() {
        return Ok(());
    }

    let db_path = match eval_store::owner_eval_db_file_for_record_path(endpoint.path()) {
        Ok(path) => path,
        Err(eval_store::EvalStoreError::Validation { .. }) => return Ok(()),
        Err(source) => return Err(source),
    };
    if !db_path.is_file() {
        return Ok(());
    }

    for record in records {
        let envelope = &record.envelope;
        eval_store::write_channel_message_to_owner_db(
            &db_path,
            eval_store::ChannelMessageEvidence {
                campaign_id: endpoint.campaign_id.clone(),
                node_id: endpoint.node_id.clone(),
                runtime_id: endpoint.runtime_id.to_string(),
                direction: direction_label(endpoint.direction).to_string(),
                message_kind: to_parent_message_kind(envelope.body()).to_string(),
                message_id: envelope.message_id.to_string(),
                endpoint_path: record.receipt.endpoint().to_path_buf(),
                cursor_offset: record.receipt.cursor().offset() as i64,
                bytes_written: record.receipt.bytes_written() as i64,
                body_hash: envelope.body_hash.clone(),
                content_sha256: format!("{:x}", Sha256::digest(&record.bytes)),
                recorded_at: envelope.recorded_at.0.to_string(),
            },
        )?;
        let imported_at = RecordedAt::now().0.to_string();
        let evidence_ref = channel_message_ref(envelope);
        let receipt_id = eval_store::write_channel_receipt_to_owner_db(
            &db_path,
            eval_store::ChannelReceiptEvidence {
                campaign_id: endpoint.campaign_id.clone(),
                node_id: endpoint.node_id.clone(),
                runtime_id: endpoint.runtime_id.to_string(),
                direction: direction_label(endpoint.direction).to_string(),
                message_id: envelope.message_id.to_string(),
                observed_by: "parent".to_string(),
                validation_status: "valid".to_string(),
                imported_ref: Some(evidence_ref.clone()),
                observed_at: imported_at.clone(),
            },
        )?;
        eval_store::write_import_event_to_owner_db(
            &db_path,
            eval_store::ImportEventEvidence {
                campaign_id: endpoint.campaign_id.clone(),
                importer_id: "parent".to_string(),
                source_runtime_id: Some(endpoint.runtime_id.to_string()),
                source_scope: format!("child_runtime:{}", endpoint.runtime_id),
                target_scope: "parent_visible".to_string(),
                evidence_ref,
                receipt_id: Some(receipt_id),
                validation_status: "valid".to_string(),
                imported_at,
            },
        )?;
    }

    Ok(())
}

pub(super) fn channel_message_ref(envelope: &Envelope<ToParent>) -> String {
    format!("channel_message:{}", envelope.message_id)
}

fn to_parent_message_kind(message: &ToParent) -> &'static str {
    match message {
        ToParent::Ready => "ready",
        ToParent::Evaluating => "evaluating",
        ToParent::Result { .. } => "result",
        ToParent::ResultWritten { .. } => "result_written",
        ToParent::SuccessorReady { .. } => "successor_ready",
        ToParent::SuccessorCompletion { .. } => "successor_completion",
        ToParent::Failed { .. } => "failed",
        ToParent::Exited { .. } => "exited",
    }
}

pub(super) fn direction_label(direction: Direction) -> &'static str {
    match direction {
        Direction::ParentToChild => "parent_to_child",
        Direction::ChildToParent => "child_to_parent",
    }
}
