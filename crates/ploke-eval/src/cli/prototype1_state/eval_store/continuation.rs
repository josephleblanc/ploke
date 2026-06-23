use std::{collections::BTreeMap, path::Path};

use cozo::DataValue;
use ploke_records::ids::CampaignId;
use sha2::{Digest, Sha256};

use super::{
    cozo_schema::eval_relation_exists,
    cozo_store::{EvalDb, load_owner_eval_database, persist_owner_eval_database},
    error::EvalStoreError,
};

pub(crate) const CONTINUATION_DECISION_REL: &str = "eval_continuation_decision";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContinuationDecisionEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) parent_id: String,
    pub(crate) disposition: String,
    pub(crate) selected_branch_id: Option<String>,
    pub(crate) next_generation: u32,
    pub(crate) total_nodes: u32,
    pub(crate) policy_ref: Option<String>,
    pub(crate) recorded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContinuationDecisionReceipt {
    pub(crate) decision_id: String,
}

struct EvalContinuationDecisionRow {
    decision_id: String,
    campaign_id: String,
    parent_id: String,
    disposition: String,
    selected_branch_id: Option<String>,
    next_generation: i64,
    total_nodes: i64,
    policy_ref: Option<String>,
    recorded_at: Option<String>,
}

pub(super) fn ensure_continuation_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    if !eval_relation_exists(db, CONTINUATION_DECISION_REL)? {
        db.eval_query_mut_params(
            r#"
:create eval_continuation_decision {
    decision_id: String =>
    campaign_id: String,
    parent_id: String,
    disposition: String,
    selected_branch_id: String?,
    next_generation: Int,
    total_nodes: Int,
    policy_ref: String?,
    recorded_at: String?
}
"#,
            BTreeMap::new(),
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "schema.eval_continuation_decision",
            source,
        })?;
    }

    Ok(())
}

pub(crate) fn write_continuation_decision_to_owner_db(
    db_path: &Path,
    evidence: ContinuationDecisionEvidence,
) -> Result<ContinuationDecisionReceipt, EvalStoreError> {
    let db = load_owner_eval_database(db_path)?;
    ensure_continuation_schema(&db)?;
    let row = continuation_decision_row(evidence)?;
    put_continuation_decision_row(&db, &row)?;
    persist_owner_eval_database(&db, db_path)?;
    Ok(ContinuationDecisionReceipt {
        decision_id: row.decision_id,
    })
}

fn continuation_decision_row(
    evidence: ContinuationDecisionEvidence,
) -> Result<EvalContinuationDecisionRow, EvalStoreError> {
    require_non_empty("continuation.parent_id", &evidence.parent_id)?;
    require_non_empty("continuation.disposition", &evidence.disposition)?;
    if let Some(branch_id) = &evidence.selected_branch_id {
        require_non_empty("continuation.selected_branch_id", branch_id)?;
    }
    let decision_id = decision_id(&evidence);
    Ok(EvalContinuationDecisionRow {
        decision_id,
        campaign_id: evidence.campaign_id.to_string(),
        parent_id: evidence.parent_id,
        disposition: evidence.disposition,
        selected_branch_id: evidence.selected_branch_id,
        next_generation: i64::from(evidence.next_generation),
        total_nodes: i64::from(evidence.total_nodes),
        policy_ref: evidence.policy_ref,
        recorded_at: evidence.recorded_at,
    })
}

fn put_continuation_decision_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalContinuationDecisionRow,
) -> Result<(), EvalStoreError> {
    db.eval_query_mut_params(
        r#"
?[
    decision_id,
    campaign_id,
    parent_id,
    disposition,
    selected_branch_id,
    next_generation,
    total_nodes,
    policy_ref,
    recorded_at
] :=
    decision_id = $decision_id,
    campaign_id = $campaign_id,
    parent_id = $parent_id,
    disposition = $disposition,
    selected_branch_id = $selected_branch_id,
    next_generation = $next_generation,
    total_nodes = $total_nodes,
    policy_ref = $policy_ref,
    recorded_at = $recorded_at
:put eval_continuation_decision {
    decision_id =>
    campaign_id,
    parent_id,
    disposition,
    selected_branch_id,
    next_generation,
    total_nodes,
    policy_ref,
    recorded_at
}
"#,
        continuation_decision_params(row),
    )
    .map_err(|source| EvalStoreError::Db {
        phase: "put.eval_continuation_decision",
        source,
    })?;
    Ok(())
}

fn continuation_decision_params(row: &EvalContinuationDecisionRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("decision_id".to_string(), row.decision_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("parent_id".to_string(), row.parent_id.clone().into());
    params.insert("disposition".to_string(), row.disposition.clone().into());
    params.insert(
        "selected_branch_id".to_string(),
        option_string(&row.selected_branch_id),
    );
    params.insert("next_generation".to_string(), row.next_generation.into());
    params.insert("total_nodes".to_string(), row.total_nodes.into());
    params.insert("policy_ref".to_string(), option_string(&row.policy_ref));
    params.insert("recorded_at".to_string(), option_string(&row.recorded_at));
    params
}

fn option_string(value: &Option<String>) -> DataValue {
    value
        .clone()
        .map(DataValue::from)
        .unwrap_or(DataValue::Null)
}

fn require_non_empty(field: &'static str, value: &str) -> Result<(), EvalStoreError> {
    if value.is_empty() {
        return Err(EvalStoreError::Validation {
            field,
            detail: "must not be empty".to_string(),
        });
    }
    Ok(())
}

fn decision_id(evidence: &ContinuationDecisionEvidence) -> String {
    let next_generation = evidence.next_generation.to_string();
    let total_nodes = evidence.total_nodes.to_string();
    hash_parts(&[
        "p1.eval.continuation_decision.v1",
        evidence.campaign_id.as_str(),
        &evidence.parent_id,
        &evidence.disposition,
        evidence.selected_branch_id.as_deref().unwrap_or(""),
        &next_generation,
        &total_nodes,
        evidence.policy_ref.as_deref().unwrap_or(""),
    ])
}

fn hash_parts(parts: &[&str]) -> String {
    let mut hasher = Sha256::new();
    for part in parts {
        hasher.update((part.len() as u64).to_be_bytes());
        hasher.update(part.as_bytes());
    }
    let bytes = hasher.finalize();
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push(char::from_digit((byte >> 4) as u32, 16).expect("hex"));
        out.push(char::from_digit((byte & 0x0f) as u32, 16).expect("hex"));
    }
    out
}
