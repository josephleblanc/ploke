use std::{collections::BTreeMap, path::Path};

use cozo::DataValue;
use ploke_records::ids::CampaignId;
use sha2::{Digest, Sha256};

use super::{
    cozo_store::{EvalDb, mutate_owner_db},
    error::EvalStoreError,
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

define_eval_schema!(EvaluationSchema {
    "eval_evaluation",
    evaluation_id: "String" =>
    campaign_id: "String",
    parent_id: "String?",
    branch_id: "String",
    baseline_id: "String?",
    treatment_id: "String?",
    procedure_id: "String?",
    evaluator_id: "String?",
    eval_set_id: "String?",
    policy_ref: "String?",
    disposition: "String",
    record_ref: "String?",
    recorded_at: "String?",
});

define_eval_schema!(EvaluationInstanceSchema {
    "eval_evaluation_instance",
    evaluation_id: "String",
    instance_id: "String" =>
    baseline_run_id: "String?",
    treatment_run_id: "String?",
    baseline_ref: "String?",
    treatment_ref: "String?",
    status: "String",
    outcome: "String?",
    oracle_ref: "String?",
});

pub(crate) const EVALUATION_REL: &str = EvaluationSchema::RELATION;
pub(crate) const EVALUATION_INSTANCE_REL: &str = EvaluationInstanceSchema::RELATION;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvaluationEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) parent_id: Option<String>,
    pub(crate) branch_id: String,
    pub(crate) baseline_id: Option<String>,
    pub(crate) treatment_id: Option<String>,
    pub(crate) procedure_id: Option<String>,
    pub(crate) evaluator_id: Option<String>,
    pub(crate) eval_set_id: Option<String>,
    pub(crate) policy_ref: Option<String>,
    pub(crate) disposition: String,
    pub(crate) record_ref: Option<String>,
    pub(crate) recorded_at: Option<String>,
    pub(crate) content_sha256: String,
    pub(crate) instances: Vec<EvaluationInstanceEvidence>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvaluationInstanceEvidence {
    pub(crate) instance_id: String,
    pub(crate) baseline_run_id: Option<String>,
    pub(crate) treatment_run_id: Option<String>,
    pub(crate) baseline_ref: Option<String>,
    pub(crate) treatment_ref: Option<String>,
    pub(crate) status: String,
    pub(crate) outcome: Option<String>,
    pub(crate) oracle_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EvaluationReceipt {
    pub(crate) evaluation_id: String,
    pub(crate) instance_count: usize,
}

struct EvaluationRows {
    evaluation: EvalEvaluationRow,
    instances: Vec<EvalEvaluationInstanceRow>,
}

struct EvalEvaluationRow {
    evaluation_id: String,
    campaign_id: String,
    parent_id: Option<String>,
    branch_id: String,
    baseline_id: Option<String>,
    treatment_id: Option<String>,
    procedure_id: Option<String>,
    evaluator_id: Option<String>,
    eval_set_id: Option<String>,
    policy_ref: Option<String>,
    disposition: String,
    record_ref: Option<String>,
    recorded_at: Option<String>,
}

struct EvalEvaluationInstanceRow {
    evaluation_id: String,
    instance_id: String,
    baseline_run_id: Option<String>,
    treatment_run_id: Option<String>,
    baseline_ref: Option<String>,
    treatment_ref: Option<String>,
    status: String,
    outcome: Option<String>,
    oracle_ref: Option<String>,
}

pub(super) fn ensure_evaluation_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    EvaluationSchema::SCHEMA.ensure_installed(db, "schema.eval_evaluation")?;
    EvaluationInstanceSchema::SCHEMA.ensure_installed(db, "schema.eval_evaluation_instance")?;
    Ok(())
}

pub(crate) fn write_evaluation_to_owner_db(
    db_path: &Path,
    evidence: EvaluationEvidence,
) -> Result<EvaluationReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        ensure_evaluation_schema(db)?;
        let rows = evaluation_rows(evidence)?;
        put_evaluation_row(db, &rows.evaluation)?;
        for instance in &rows.instances {
            put_evaluation_instance_row(db, instance)?;
        }
        Ok(EvaluationReceipt {
            evaluation_id: rows.evaluation.evaluation_id,
            instance_count: rows.instances.len(),
        })
    })
}

fn evaluation_rows(evidence: EvaluationEvidence) -> Result<EvaluationRows, EvalStoreError> {
    require_non_empty("evaluation.branch_id", &evidence.branch_id)?;
    require_non_empty("evaluation.disposition", &evidence.disposition)?;
    require_non_empty("evaluation.content_sha256", &evidence.content_sha256)?;
    if let Some(parent_id) = &evidence.parent_id {
        require_non_empty("evaluation.parent_id", parent_id)?;
    }
    if let Some(record_ref) = &evidence.record_ref {
        require_non_empty("evaluation.record_ref", record_ref)?;
    }
    let evaluation_id = evaluation_id(
        &evidence.campaign_id,
        &evidence.branch_id,
        evidence.record_ref.as_deref(),
        &evidence.content_sha256,
    );
    let evaluation = EvalEvaluationRow {
        evaluation_id: evaluation_id.clone(),
        campaign_id: evidence.campaign_id.to_string(),
        parent_id: evidence.parent_id,
        branch_id: evidence.branch_id,
        baseline_id: evidence.baseline_id,
        treatment_id: evidence.treatment_id,
        procedure_id: evidence.procedure_id,
        evaluator_id: evidence.evaluator_id,
        eval_set_id: evidence.eval_set_id,
        policy_ref: evidence.policy_ref,
        disposition: evidence.disposition,
        record_ref: evidence.record_ref,
        recorded_at: evidence.recorded_at,
    };
    let instances = evidence
        .instances
        .into_iter()
        .map(|instance| evaluation_instance_row(&evaluation_id, instance))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(EvaluationRows {
        evaluation,
        instances,
    })
}

fn evaluation_instance_row(
    evaluation_id: &str,
    evidence: EvaluationInstanceEvidence,
) -> Result<EvalEvaluationInstanceRow, EvalStoreError> {
    require_non_empty("evaluation_instance.instance_id", &evidence.instance_id)?;
    require_non_empty("evaluation_instance.status", &evidence.status)?;
    Ok(EvalEvaluationInstanceRow {
        evaluation_id: evaluation_id.to_string(),
        instance_id: evidence.instance_id,
        baseline_run_id: evidence.baseline_run_id,
        treatment_run_id: evidence.treatment_run_id,
        baseline_ref: evidence.baseline_ref,
        treatment_ref: evidence.treatment_ref,
        status: evidence.status,
        outcome: evidence.outcome,
        oracle_ref: evidence.oracle_ref,
    })
}

fn put_evaluation_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalEvaluationRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &EvaluationSchema::SCHEMA,
        evaluation_params(row),
        "put.eval_evaluation",
    )?;
    Ok(())
}

fn put_evaluation_instance_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalEvaluationInstanceRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &EvaluationInstanceSchema::SCHEMA,
        evaluation_instance_params(row),
        "put.eval_evaluation_instance",
    )?;
    Ok(())
}

fn evaluation_params(row: &EvalEvaluationRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "evaluation_id".to_string(),
        row.evaluation_id.clone().into(),
    );
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("parent_id".to_string(), option_string(&row.parent_id));
    params.insert("branch_id".to_string(), row.branch_id.clone().into());
    params.insert("baseline_id".to_string(), option_string(&row.baseline_id));
    params.insert("treatment_id".to_string(), option_string(&row.treatment_id));
    params.insert("procedure_id".to_string(), option_string(&row.procedure_id));
    params.insert("evaluator_id".to_string(), option_string(&row.evaluator_id));
    params.insert("eval_set_id".to_string(), option_string(&row.eval_set_id));
    params.insert("policy_ref".to_string(), option_string(&row.policy_ref));
    params.insert("disposition".to_string(), row.disposition.clone().into());
    params.insert("record_ref".to_string(), option_string(&row.record_ref));
    params.insert("recorded_at".to_string(), option_string(&row.recorded_at));
    params
}

fn evaluation_instance_params(row: &EvalEvaluationInstanceRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert(
        "evaluation_id".to_string(),
        row.evaluation_id.clone().into(),
    );
    params.insert("instance_id".to_string(), row.instance_id.clone().into());
    params.insert(
        "baseline_run_id".to_string(),
        option_string(&row.baseline_run_id),
    );
    params.insert(
        "treatment_run_id".to_string(),
        option_string(&row.treatment_run_id),
    );
    params.insert("baseline_ref".to_string(), option_string(&row.baseline_ref));
    params.insert(
        "treatment_ref".to_string(),
        option_string(&row.treatment_ref),
    );
    params.insert("status".to_string(), row.status.clone().into());
    params.insert("outcome".to_string(), option_string(&row.outcome));
    params.insert("oracle_ref".to_string(), option_string(&row.oracle_ref));
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

fn evaluation_id(
    campaign_id: &CampaignId,
    branch_id: &str,
    record_ref: Option<&str>,
    content_sha256: &str,
) -> String {
    hash_parts(&[
        "p1.eval.evaluation.v1",
        campaign_id.as_str(),
        branch_id,
        record_ref.unwrap_or(""),
        content_sha256,
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
