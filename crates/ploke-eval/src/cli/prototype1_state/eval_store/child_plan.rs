use std::{
    collections::BTreeMap,
    fs,
    path::{Path, PathBuf},
};

use chrono::Utc;
use cozo::DataValue;
use ploke_records::ids::CampaignId;
use serde::Serialize;

use crate::cli::prototype1_state::{
    history::surface_attempt,
    parent::{ChildFiles, ChildPlanFiles},
};

use super::{
    cozo_store::{DbEvalStore, EvalDb, mutate_owner_db},
    error::EvalStoreError,
    evidence::{hash_parts, sha256_bytes},
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

define_eval_schema!(ChildPlanSchema {
    "eval_child_plan",
    plan_id: "String" =>
    campaign_id: "String",
    schema_version: "String",
    parent_node_id: "String",
    child_generation: "Int",
    message_path: "String",
    message_sha256: "String",
    child_count: "Int",
    rejected_count: "Int",
    recorded_at: "String",
    ingested_at: "String",
});

define_eval_schema!(ChildPlanChildSchema {
    "eval_child_plan_child",
    plan_id: "String",
    child_node_id: "String" =>
    campaign_id: "String",
    parent_node_id: "String",
    child_index: "Int",
    child_generation: "Int",
    node_schema_version: "String",
    branch_id: "String",
    parent_branch_id: "String?",
    candidate_id: "String",
    instance_id: "String",
    source_state_id: "String",
    target_relpath: "String",
    node_path: "String",
    runner_request_path: "String",
    runner_result_path: "String",
    workspace_root: "String",
    binary_path: "String",
    status: "String",
    resolved_branch_id: "String",
    resolved_candidate_id: "String",
    source_content_hash: "String",
    proposed_content_hash: "String",
    selected_branch_id: "String?",
    surface_present: "Bool",
    harness_present: "Bool",
});

define_eval_schema!(ChildPlanRejectedSchema {
    "eval_child_plan_rejected_attempt",
    plan_id: "String",
    attempt_index: "Int" =>
    campaign_id: "String",
    parent_node_id: "String",
    producer_id: "String",
    proposal_id: "String",
    run_id: "String",
    policy: "String",
    target_relpath: "String",
    outcome: "String",
    reason: "String?",
});

pub(crate) const CHILD_PLAN_SCHEMA_VERSION: &str = "prototype1-child-plan-file.v1";
pub(crate) const CHILD_PLAN_REL: &str = ChildPlanSchema::RELATION;
pub(crate) const CHILD_PLAN_CHILD_REL: &str = ChildPlanChildSchema::RELATION;
pub(crate) const CHILD_PLAN_REJECTED_REL: &str = ChildPlanRejectedSchema::RELATION;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildPlanEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) schema_version: String,
    pub(crate) message_path: PathBuf,
    pub(crate) body: ChildPlanFiles,
    pub(crate) recorded_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChildPlanReceipt {
    pub(crate) plan_id: String,
    pub(crate) child_count: usize,
    pub(crate) rejected_count: usize,
    pub(crate) message_sha256: String,
}

struct ChildPlanRows {
    plan: EvalChildPlanRow,
    children: Vec<EvalChildPlanChildRow>,
    rejected: Vec<EvalChildPlanRejectedRow>,
}

struct EvalChildPlanRow {
    plan_id: String,
    campaign_id: String,
    schema_version: String,
    parent_node_id: String,
    child_generation: i64,
    message_path: String,
    message_sha256: String,
    child_count: i64,
    rejected_count: i64,
    recorded_at: String,
    ingested_at: String,
}

struct EvalChildPlanChildRow {
    plan_id: String,
    child_node_id: String,
    campaign_id: String,
    parent_node_id: String,
    child_index: i64,
    child_generation: i64,
    node_schema_version: String,
    branch_id: String,
    parent_branch_id: Option<String>,
    candidate_id: String,
    instance_id: String,
    source_state_id: String,
    target_relpath: String,
    node_path: String,
    runner_request_path: String,
    runner_result_path: String,
    workspace_root: String,
    binary_path: String,
    status: String,
    resolved_branch_id: String,
    resolved_candidate_id: String,
    source_content_hash: String,
    proposed_content_hash: String,
    selected_branch_id: Option<String>,
    surface_present: bool,
    harness_present: bool,
}

struct EvalChildPlanRejectedRow {
    plan_id: String,
    attempt_index: i64,
    campaign_id: String,
    parent_node_id: String,
    producer_id: String,
    proposal_id: String,
    run_id: String,
    policy: String,
    target_relpath: String,
    outcome: String,
    reason: Option<String>,
}

pub(super) fn ensure_child_plan_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    ChildPlanSchema::SCHEMA.ensure_installed(db, "schema.eval_child_plan")?;
    ChildPlanChildSchema::SCHEMA.ensure_installed(db, "schema.eval_child_plan_child")?;
    ChildPlanRejectedSchema::SCHEMA
        .ensure_installed(db, "schema.eval_child_plan_rejected_attempt")?;
    Ok(())
}

pub(crate) fn write_child_plan_to_owner_db(
    db_path: &Path,
    evidence: ChildPlanEvidence,
) -> Result<ChildPlanReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        DbEvalStore::new(db).install_schema()?;
        let rows = child_plan_rows(evidence)?;
        put_child_plan_row(db, &rows.plan)?;
        for child in &rows.children {
            put_child_plan_child_row(db, child)?;
        }
        for rejected in &rows.rejected {
            put_child_plan_rejected_row(db, rejected)?;
        }
        Ok(ChildPlanReceipt {
            plan_id: rows.plan.plan_id,
            child_count: rows.children.len(),
            rejected_count: rows.rejected.len(),
            message_sha256: rows.plan.message_sha256,
        })
    })
}

fn child_plan_rows(evidence: ChildPlanEvidence) -> Result<ChildPlanRows, EvalStoreError> {
    require_non_empty("child_plan.schema_version", &evidence.schema_version)?;
    let parent_node_id = evidence.body.parent_node_id().to_string();
    require_non_empty("child_plan.parent_node_id", &parent_node_id)?;
    if evidence.body.message() != evidence.message_path.as_path() {
        return Err(EvalStoreError::Validation {
            field: "child_plan.message_path",
            detail: format!(
                "body message '{}' did not match source path '{}'",
                evidence.body.message().display(),
                evidence.message_path.display()
            ),
        });
    }
    let message_bytes = fs::read(&evidence.message_path).map_err(|source| EvalStoreError::Io {
        phase: "child_plan.read_message",
        path: evidence.message_path.clone(),
        source,
    })?;
    let message_sha256 = sha256_bytes(&message_bytes);
    let message_path = evidence.message_path.display().to_string();
    let child_generation = i64::from(evidence.body.child_generation());
    let plan_id = plan_id(
        &evidence.campaign_id,
        &parent_node_id,
        child_generation,
        &message_path,
    );
    let campaign_id = evidence.campaign_id.to_string();
    let ingested_at = Utc::now().to_rfc3339();
    let children = evidence
        .body
        .children()
        .iter()
        .enumerate()
        .map(|(index, child)| {
            child_row(
                &plan_id,
                &campaign_id,
                &parent_node_id,
                child_generation,
                index,
                child,
            )
        })
        .collect::<Result<Vec<_>, _>>()?;
    let rejected = evidence
        .body
        .rejected_surface_attempts()
        .iter()
        .enumerate()
        .map(|(index, attempt)| {
            rejected_row(&plan_id, &campaign_id, &parent_node_id, index, attempt)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let plan = EvalChildPlanRow {
        plan_id,
        campaign_id,
        schema_version: evidence.schema_version,
        parent_node_id,
        child_generation,
        message_path,
        message_sha256,
        child_count: usize_to_i64(children.len(), "child_plan.child_count")?,
        rejected_count: usize_to_i64(rejected.len(), "child_plan.rejected_count")?,
        recorded_at: evidence.recorded_at,
        ingested_at,
    };
    Ok(ChildPlanRows {
        plan,
        children,
        rejected,
    })
}

fn child_row(
    plan_id: &str,
    campaign_id: &str,
    parent_node_id: &str,
    child_generation: i64,
    index: usize,
    child: &ChildFiles,
) -> Result<EvalChildPlanChildRow, EvalStoreError> {
    let node = child.node_record();
    let request = child.runner_request();
    let resolved = child.resolved();
    require_non_empty("child_plan_child.node_id", &node.node_id)?;
    require_non_empty("child_plan_child.branch_id", &node.branch_id)?;
    require_non_empty("child_plan_child.candidate_id", &node.candidate_id)?;
    require_non_empty("child_plan_child.instance_id", &node.instance_id)?;
    require_non_empty("child_plan_child.source_state_id", &node.source_state_id)?;
    require_non_empty(
        "child_plan_child.resolved_branch_id",
        &resolved.branch.branch_id,
    )?;
    require_non_empty(
        "child_plan_child.resolved_candidate_id",
        &resolved.branch.candidate_id,
    )?;
    require_non_empty(
        "child_plan_child.source_content_hash",
        &resolved.source_content_hash,
    )?;
    require_non_empty(
        "child_plan_child.proposed_content_hash",
        &resolved.branch.proposed_content_hash,
    )?;
    Ok(EvalChildPlanChildRow {
        plan_id: plan_id.to_string(),
        child_node_id: node.node_id.clone(),
        campaign_id: campaign_id.to_string(),
        parent_node_id: parent_node_id.to_string(),
        child_index: usize_to_i64(index, "child_plan_child.child_index")?,
        child_generation,
        node_schema_version: node.schema_version.clone(),
        branch_id: node.branch_id.clone(),
        parent_branch_id: node.parent_branch_id.clone(),
        candidate_id: node.candidate_id.clone(),
        instance_id: node.instance_id.clone(),
        source_state_id: node.source_state_id.clone(),
        target_relpath: node.target_relpath.display().to_string(),
        node_path: node.node_dir.join("node.json").display().to_string(),
        runner_request_path: node.runner_request_path.display().to_string(),
        runner_result_path: node.runner_result_path.display().to_string(),
        workspace_root: request.workspace_root.display().to_string(),
        binary_path: request.binary_path.display().to_string(),
        status: serde_name(&node.status, "child_plan_child.status")?,
        resolved_branch_id: resolved.branch.branch_id.clone(),
        resolved_candidate_id: resolved.branch.candidate_id.clone(),
        source_content_hash: resolved.source_content_hash.clone(),
        proposed_content_hash: resolved.branch.proposed_content_hash.clone(),
        selected_branch_id: resolved.selected_branch_id.clone(),
        surface_present: child.surface().is_some(),
        harness_present: child.harness_evidence().is_some(),
    })
}

fn rejected_row(
    plan_id: &str,
    campaign_id: &str,
    parent_node_id: &str,
    index: usize,
    attempt: &surface_attempt::Evidence,
) -> Result<EvalChildPlanRejectedRow, EvalStoreError> {
    require_non_empty("child_plan_rejected.producer_id", &attempt.producer_id)?;
    require_non_empty("child_plan_rejected.proposal_id", &attempt.proposal_id)?;
    require_non_empty("child_plan_rejected.run_id", &attempt.run_id)?;
    require_non_empty("child_plan_rejected.policy", &attempt.policy)?;
    let (outcome, reason) = match &attempt.outcome {
        surface_attempt::Outcome::Applied => ("applied".to_string(), None),
        surface_attempt::Outcome::Rejected { reason } => {
            require_non_empty("child_plan_rejected.reason", reason)?;
            ("rejected".to_string(), Some(reason.clone()))
        }
    };
    Ok(EvalChildPlanRejectedRow {
        plan_id: plan_id.to_string(),
        attempt_index: usize_to_i64(index, "child_plan_rejected.attempt_index")?,
        campaign_id: campaign_id.to_string(),
        parent_node_id: parent_node_id.to_string(),
        producer_id: attempt.producer_id.clone(),
        proposal_id: attempt.proposal_id.clone(),
        run_id: attempt.run_id.clone(),
        policy: attempt.policy.clone(),
        target_relpath: attempt.target_relpath.display().to_string(),
        outcome,
        reason,
    })
}

fn put_child_plan_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalChildPlanRow,
) -> Result<(), EvalStoreError> {
    if let Some(existing) = existing_child_plan_hash(db, &row.plan_id)? {
        if existing != row.message_sha256 {
            return Err(EvalStoreError::Validation {
                field: "eval_child_plan.message_sha256",
                detail: format!(
                    "child plan '{}' already exists with message hash {}, attempted {}",
                    row.plan_id, existing, row.message_sha256
                ),
            });
        }
    }
    put_eval_params(
        db,
        &ChildPlanSchema::SCHEMA,
        child_plan_params(row),
        "put.eval_child_plan",
    )?;
    Ok(())
}

fn put_child_plan_child_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalChildPlanChildRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &ChildPlanChildSchema::SCHEMA,
        child_plan_child_params(row),
        "put.eval_child_plan_child",
    )?;
    Ok(())
}

fn put_child_plan_rejected_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalChildPlanRejectedRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &ChildPlanRejectedSchema::SCHEMA,
        child_plan_rejected_params(row),
        "put.eval_child_plan_rejected_attempt",
    )?;
    Ok(())
}

fn existing_child_plan_hash<D: EvalDb + ?Sized>(
    db: &D,
    plan_id: &str,
) -> Result<Option<String>, EvalStoreError> {
    let mut params = BTreeMap::new();
    params.insert("plan_id".to_string(), DataValue::from(plan_id.to_string()));
    let result = db
        .eval_query_params(
            r#"
?[message_sha256] :=
    *eval_child_plan { plan_id, message_sha256 },
    plan_id = $plan_id
"#,
            params,
        )
        .map_err(|source| EvalStoreError::Db {
            phase: "query.eval_child_plan.message_sha256",
            source,
        })?;
    Ok(result.rows.first().and_then(|row| match row.first() {
        Some(DataValue::Str(value)) => Some(value.to_string()),
        _ => None,
    }))
}

fn child_plan_params(row: &EvalChildPlanRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("plan_id".to_string(), row.plan_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert(
        "schema_version".to_string(),
        row.schema_version.clone().into(),
    );
    params.insert(
        "parent_node_id".to_string(),
        row.parent_node_id.clone().into(),
    );
    params.insert("child_generation".to_string(), row.child_generation.into());
    params.insert("message_path".to_string(), row.message_path.clone().into());
    params.insert(
        "message_sha256".to_string(),
        row.message_sha256.clone().into(),
    );
    params.insert("child_count".to_string(), row.child_count.into());
    params.insert("rejected_count".to_string(), row.rejected_count.into());
    params.insert("recorded_at".to_string(), row.recorded_at.clone().into());
    params.insert("ingested_at".to_string(), row.ingested_at.clone().into());
    params
}

fn child_plan_child_params(row: &EvalChildPlanChildRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("plan_id".to_string(), row.plan_id.clone().into());
    params.insert(
        "child_node_id".to_string(),
        row.child_node_id.clone().into(),
    );
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert(
        "parent_node_id".to_string(),
        row.parent_node_id.clone().into(),
    );
    params.insert("child_index".to_string(), row.child_index.into());
    params.insert("child_generation".to_string(), row.child_generation.into());
    params.insert(
        "node_schema_version".to_string(),
        row.node_schema_version.clone().into(),
    );
    params.insert("branch_id".to_string(), row.branch_id.clone().into());
    params.insert(
        "parent_branch_id".to_string(),
        option_string(&row.parent_branch_id),
    );
    params.insert("candidate_id".to_string(), row.candidate_id.clone().into());
    params.insert("instance_id".to_string(), row.instance_id.clone().into());
    params.insert(
        "source_state_id".to_string(),
        row.source_state_id.clone().into(),
    );
    params.insert(
        "target_relpath".to_string(),
        row.target_relpath.clone().into(),
    );
    params.insert("node_path".to_string(), row.node_path.clone().into());
    params.insert(
        "runner_request_path".to_string(),
        row.runner_request_path.clone().into(),
    );
    params.insert(
        "runner_result_path".to_string(),
        row.runner_result_path.clone().into(),
    );
    params.insert(
        "workspace_root".to_string(),
        row.workspace_root.clone().into(),
    );
    params.insert("binary_path".to_string(), row.binary_path.clone().into());
    params.insert("status".to_string(), row.status.clone().into());
    params.insert(
        "resolved_branch_id".to_string(),
        row.resolved_branch_id.clone().into(),
    );
    params.insert(
        "resolved_candidate_id".to_string(),
        row.resolved_candidate_id.clone().into(),
    );
    params.insert(
        "source_content_hash".to_string(),
        row.source_content_hash.clone().into(),
    );
    params.insert(
        "proposed_content_hash".to_string(),
        row.proposed_content_hash.clone().into(),
    );
    params.insert(
        "selected_branch_id".to_string(),
        option_string(&row.selected_branch_id),
    );
    params.insert("surface_present".to_string(), row.surface_present.into());
    params.insert("harness_present".to_string(), row.harness_present.into());
    params
}

fn child_plan_rejected_params(row: &EvalChildPlanRejectedRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("plan_id".to_string(), row.plan_id.clone().into());
    params.insert("attempt_index".to_string(), row.attempt_index.into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert(
        "parent_node_id".to_string(),
        row.parent_node_id.clone().into(),
    );
    params.insert("producer_id".to_string(), row.producer_id.clone().into());
    params.insert("proposal_id".to_string(), row.proposal_id.clone().into());
    params.insert("run_id".to_string(), row.run_id.clone().into());
    params.insert("policy".to_string(), row.policy.clone().into());
    params.insert(
        "target_relpath".to_string(),
        row.target_relpath.clone().into(),
    );
    params.insert("outcome".to_string(), row.outcome.clone().into());
    params.insert("reason".to_string(), option_string(&row.reason));
    params
}

fn plan_id(
    campaign_id: &CampaignId,
    parent_node_id: &str,
    child_generation: i64,
    message_path: &str,
) -> String {
    hash_parts(&[
        "p1.eval.child_plan.v1",
        &campaign_id.to_string(),
        parent_node_id,
        &child_generation.to_string(),
        message_path,
    ])
}

fn serde_name<T: Serialize>(value: &T, field: &'static str) -> Result<String, EvalStoreError> {
    match serde_json::to_value(value).map_err(|source| EvalStoreError::Validation {
        field,
        detail: source.to_string(),
    })? {
        serde_json::Value::String(value) => Ok(value),
        other => Err(EvalStoreError::Validation {
            field,
            detail: format!("expected serde string, got {other}"),
        }),
    }
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
        detail: format!("value {value} does not fit in Cozo Int"),
    })
}

fn option_string(value: &Option<String>) -> DataValue {
    value
        .clone()
        .map(DataValue::from)
        .unwrap_or(DataValue::Null)
}
