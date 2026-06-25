use std::{collections::BTreeMap, path::Path};

use cozo::DataValue;
use ploke_records::ids::CampaignId;
use sha2::{Digest, Sha256};

use crate::cli::prototype1_state::history::{
    CandidateSetMembership, EvaluationPayload, SelectionDecisionEntry,
};
use crate::successor_selection::traversal::{Formula, ScoreChildPropFormulaRow};

use super::{
    cozo_store::{EvalDb, mutate_owner_db},
    error::EvalStoreError,
    schema::{EvalRelationSchema, define_eval_schema, put_eval_params},
};

define_eval_schema!(SelectionDecisionSchema {
    "eval_selection_decision",
    decision_id: "String" =>
    campaign_id: "String",
    parent_id: "String",
    set_id: "String",
    procedure_id: "String",
    selected_node_id: "String?",
    selected_artifact_id: "String?",
    outcome: "String",
    disposition: "String?",
    decision_ref: "String?",
    decision_hash: "String?",
    recorded_at: "String?",
});

define_eval_schema!(SelectionCandidateSchema {
    "eval_selection_candidate",
    decision_id: "String",
    member_id: "String" =>
    node_id: "String",
    branch_id: "String",
    selectable: "Bool",
    selected: "Bool",
    exclusion_ref: "String?",
});

define_eval_schema!(SelectionFindingSchema {
    "eval_selection_finding",
    finding_id: "String" =>
    decision_id: "String",
    member_id: "String?",
    domain: "String",
    verdict: "String",
    confidence: "String",
    evidence_ref: "String?",
    rationale_ref: "String?",
});

define_eval_schema!(SelectionScoreSchema {
    "eval_selection_score",
    decision_id: "String",
    member_id: "String" =>
    formula_id: "String",
    score_json: "String",
    weight: "Float?",
    rank: "Int?",
    selected: "Bool",
});

pub(crate) const SELECTION_DECISION_REL: &str = SelectionDecisionSchema::RELATION;
pub(crate) const SELECTION_CANDIDATE_REL: &str = SelectionCandidateSchema::RELATION;
pub(crate) const SELECTION_FINDING_REL: &str = SelectionFindingSchema::RELATION;
pub(crate) const SELECTION_SCORE_REL: &str = SelectionScoreSchema::RELATION;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct SelectionDecisionEvidence {
    pub(crate) campaign_id: CampaignId,
    pub(crate) parent_id: String,
    pub(crate) entry: SelectionDecisionEntry,
    pub(crate) decision_ref: Option<String>,
    pub(crate) recorded_at: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SelectionDecisionReceipt {
    pub(crate) decision_id: String,
    pub(crate) candidate_count: usize,
    pub(crate) finding_count: usize,
    pub(crate) score_count: usize,
}

struct SelectionRows {
    decision: EvalSelectionDecisionRow,
    candidates: Vec<EvalSelectionCandidateRow>,
    findings: Vec<EvalSelectionFindingRow>,
    scores: Vec<EvalSelectionScoreRow>,
}

struct EvalSelectionDecisionRow {
    decision_id: String,
    campaign_id: String,
    parent_id: String,
    set_id: String,
    procedure_id: String,
    selected_node_id: Option<String>,
    selected_artifact_id: Option<String>,
    outcome: String,
    disposition: Option<String>,
    decision_ref: Option<String>,
    decision_hash: Option<String>,
    recorded_at: Option<String>,
}

struct EvalSelectionCandidateRow {
    decision_id: String,
    member_id: String,
    node_id: String,
    branch_id: String,
    selectable: bool,
    selected: bool,
    exclusion_ref: Option<String>,
}

struct EvalSelectionFindingRow {
    finding_id: String,
    decision_id: String,
    member_id: Option<String>,
    domain: String,
    verdict: String,
    confidence: String,
    evidence_ref: Option<String>,
    rationale_ref: Option<String>,
}

struct EvalSelectionScoreRow {
    decision_id: String,
    member_id: String,
    formula_id: String,
    score_json: String,
    weight: Option<f64>,
    rank: Option<i64>,
    selected: bool,
}

pub(super) fn ensure_selection_schema<D: EvalDb + ?Sized>(db: &D) -> Result<(), EvalStoreError> {
    SelectionDecisionSchema::SCHEMA.ensure_installed(db, "schema.eval_selection_decision")?;
    SelectionCandidateSchema::SCHEMA.ensure_installed(db, "schema.eval_selection_candidate")?;
    SelectionFindingSchema::SCHEMA.ensure_installed(db, "schema.eval_selection_finding")?;
    SelectionScoreSchema::SCHEMA.ensure_installed(db, "schema.eval_selection_score")?;
    Ok(())
}

pub(crate) fn write_selection_decision_to_owner_db(
    db_path: &Path,
    evidence: SelectionDecisionEvidence,
) -> Result<SelectionDecisionReceipt, EvalStoreError> {
    mutate_owner_db(db_path, |db| {
        ensure_selection_schema(db)?;
        let rows = selection_rows(evidence)?;
        put_selection_decision_row(db, &rows.decision)?;
        for candidate in &rows.candidates {
            put_selection_candidate_row(db, candidate)?;
        }
        for finding in &rows.findings {
            put_selection_finding_row(db, finding)?;
        }
        for score in &rows.scores {
            put_selection_score_row(db, score)?;
        }
        Ok(SelectionDecisionReceipt {
            decision_id: rows.decision.decision_id,
            candidate_count: rows.candidates.len(),
            finding_count: rows.findings.len(),
            score_count: rows.scores.len(),
        })
    })
}

fn selection_rows(evidence: SelectionDecisionEvidence) -> Result<SelectionRows, EvalStoreError> {
    require_non_empty("selection.parent_id", &evidence.parent_id)?;
    let set_id = evidence
        .entry
        .candidate_set
        .as_ref()
        .map(|set| set.root.as_str().to_string())
        .unwrap_or_else(|| evidence.entry.considered_order_hash.as_str().to_string());
    require_non_empty("selection.set_id", &set_id)?;
    let procedure_id = evidence.entry.procedure_or_policy.as_str().to_string();
    require_non_empty("selection.procedure_id", &procedure_id)?;
    let outcome = serde_name(&evidence.entry.decision.outcome)?;
    let disposition = evidence
        .entry
        .decision
        .selected_branch_disposition()
        .map(ToOwned::to_owned);
    let decision_hash = evidence
        .entry
        .decision_hash()
        .map(|hash| hash.as_str().to_string())
        .map_err(|source| EvalStoreError::Validation {
            field: "selection.decision_hash",
            detail: source.to_string(),
        })?;
    let decision_id = decision_id(
        &evidence.campaign_id,
        &evidence.parent_id,
        &set_id,
        &procedure_id,
        &decision_hash,
    );
    let selected_node_id = evidence
        .entry
        .decision
        .selected_branch_id
        .as_ref()
        .map(|_| evidence.entry.decision.candidate_node_id.clone());
    let selected_artifact_id = selected_artifact_id(&evidence.entry)?;
    let candidates = evidence
        .entry
        .considered
        .iter()
        .enumerate()
        .map(|(index, payload)| {
            selection_candidate_row(&decision_id, &evidence.entry, index, payload)
        })
        .collect::<Result<Vec<_>, _>>()?;
    let selected_member_id = selected_member_id(&evidence.entry)?;
    let findings = selection_finding_rows(&decision_id, selected_member_id.as_deref(), &evidence)?;
    let scores = selection_score_rows(&decision_id, &evidence.entry)?;
    Ok(SelectionRows {
        decision: EvalSelectionDecisionRow {
            decision_id,
            campaign_id: evidence.campaign_id.to_string(),
            parent_id: evidence.parent_id,
            set_id,
            procedure_id,
            selected_node_id,
            selected_artifact_id,
            outcome,
            disposition,
            decision_ref: evidence.decision_ref,
            decision_hash: Some(decision_hash),
            recorded_at: evidence.recorded_at,
        },
        candidates,
        findings,
        scores,
    })
}

fn selection_candidate_row(
    decision_id: &str,
    entry: &SelectionDecisionEntry,
    index: usize,
    payload: &EvaluationPayload,
) -> Result<EvalSelectionCandidateRow, EvalStoreError> {
    let membership = entry
        .candidate_set_membership_for_payload(index, payload)
        .map_err(|source| EvalStoreError::Validation {
            field: "selection_candidate.membership",
            detail: source.to_string(),
        })?;
    let member_id = member_id(membership, payload)?;
    let (node_id, branch_id) = candidate_coordinate(payload)?;
    Ok(EvalSelectionCandidateRow {
        decision_id: decision_id.to_string(),
        member_id,
        node_id,
        branch_id,
        selectable: true,
        selected: payload_selected_by_decision(entry, membership, payload),
        exclusion_ref: None,
    })
}

fn selection_finding_rows(
    decision_id: &str,
    member_id: Option<&str>,
    evidence: &SelectionDecisionEvidence,
) -> Result<Vec<EvalSelectionFindingRow>, EvalStoreError> {
    evidence
        .entry
        .decision
        .findings
        .iter()
        .enumerate()
        .map(|(index, finding)| {
            let domain = serde_name(&finding.domain)?;
            let verdict = serde_name(&finding.verdict)?;
            let confidence = serde_name(&finding.confidence)?;
            Ok(EvalSelectionFindingRow {
                finding_id: finding_id(
                    &evidence.campaign_id,
                    &evidence.parent_id,
                    decision_id,
                    index,
                    &domain,
                    &verdict,
                    &confidence,
                ),
                decision_id: decision_id.to_string(),
                member_id: member_id.map(ToOwned::to_owned),
                domain,
                verdict,
                confidence,
                evidence_ref: finding.evidence_refs.first().cloned(),
                rationale_ref: finding
                    .rationale
                    .first()
                    .map(|_| rationale_ref(decision_id, index, &finding.rationale)),
            })
        })
        .collect()
}

fn selection_score_rows(
    decision_id: &str,
    entry: &SelectionDecisionEntry,
) -> Result<Vec<EvalSelectionScoreRow>, EvalStoreError> {
    let Some(formula) = entry.formula.as_ref() else {
        return Ok(Vec::new());
    };
    let Formula::ScoreChildProp(score) = &formula.formula;
    score
        .rows
        .iter()
        .map(|row| {
            let payload = entry.considered.get(row.payload_index).ok_or_else(|| {
                EvalStoreError::Validation {
                    field: "selection_score.payload_index",
                    detail: format!(
                        "score row payload_index={} exceeds considered count={}",
                        row.payload_index,
                        entry.considered.len()
                    ),
                }
            })?;
            let membership = entry
                .candidate_set_membership_for_payload(row.payload_index, payload)
                .map_err(|source| EvalStoreError::Validation {
                    field: "selection_score.membership",
                    detail: source.to_string(),
                })?;
            Ok(EvalSelectionScoreRow {
                decision_id: decision_id.to_string(),
                member_id: member_id(membership, payload)?,
                formula_id: format!("score_child_prop:{}", formula.metric_set_id.as_str()),
                score_json: serde_json::to_string(row).map_err(|source| {
                    EvalStoreError::Validation {
                        field: "selection_score.score_json",
                        detail: source.to_string(),
                    }
                })?,
                weight: row.weight,
                rank: score_rank(row),
                selected: row.selected,
            })
        })
        .collect()
}

fn selected_member_id(entry: &SelectionDecisionEntry) -> Result<Option<String>, EvalStoreError> {
    for (index, payload) in entry.considered.iter().enumerate() {
        let membership = entry
            .candidate_set_membership_for_payload(index, payload)
            .map_err(|source| EvalStoreError::Validation {
                field: "selection.selected_member_id",
                detail: source.to_string(),
            })?;
        if payload_selected_by_decision(entry, membership, payload) {
            return member_id(membership, payload).map(Some);
        }
    }
    Ok(None)
}

fn selected_artifact_id(entry: &SelectionDecisionEntry) -> Result<Option<String>, EvalStoreError> {
    for (index, payload) in entry.considered.iter().enumerate() {
        let membership = entry
            .candidate_set_membership_for_payload(index, payload)
            .map_err(|source| EvalStoreError::Validation {
                field: "selection.selected_artifact_id",
                detail: source.to_string(),
            })?;
        if !payload_selected_by_decision(entry, membership, payload) {
            continue;
        }
        let Some(artifact) = payload.artifact.as_ref() else {
            return Ok(None);
        };
        return Ok(artifact
            .node()
            .derived_artifact_id
            .as_ref()
            .map(ToString::to_string)
            .or_else(|| {
                artifact
                    .node()
                    .base_artifact_id
                    .as_ref()
                    .map(ToString::to_string)
            }));
    }
    Ok(None)
}

fn score_rank(row: &ScoreChildPropFormulaRow) -> Option<i64> {
    row.performance.map(|_| row.payload_index as i64)
}

fn rationale_ref(decision_id: &str, index: usize, rationale: &[String]) -> String {
    let payload = serde_json::to_string(rationale).unwrap_or_else(|_| format!("{rationale:?}"));
    let hash = hash_parts(&[
        "p1.eval.selection_finding.rationale.v1",
        decision_id,
        &payload,
    ]);
    format!("inline:rationale:{index}:sha256:{hash}")
}

fn member_id(
    membership: Option<&CandidateSetMembership>,
    payload: &EvaluationPayload,
) -> Result<String, EvalStoreError> {
    if let Some(membership_id) = membership.and_then(|membership| membership.membership_id.as_ref())
    {
        return Ok(membership_id.hash().as_str().to_string());
    }
    if let Some(occurrence_id) = membership.and_then(|membership| membership.occurrence_id.as_ref())
    {
        return Ok(occurrence_id.hash().as_str().to_string());
    }
    payload
        .payload_hash()
        .map(|hash| hash.as_str().to_string())
        .map_err(|source| EvalStoreError::Validation {
            field: "selection_candidate.member_id",
            detail: source.to_string(),
        })
}

fn candidate_coordinate(payload: &EvaluationPayload) -> Result<(String, String), EvalStoreError> {
    if let Some(input) = payload.selection_input.as_ref() {
        return Ok((
            input.candidate.node_id.clone(),
            input.candidate.branch_id.clone(),
        ));
    }
    if let Some(artifact) = payload.artifact.as_ref() {
        return Ok((
            artifact.node().node_id.clone(),
            artifact.node().branch_id.clone(),
        ));
    }
    if let Some(sealed) = payload.sealed_evidence.as_ref()
        && let Some(branch_id) = sealed.coordinate.branch_id.clone()
    {
        return Ok((sealed.coordinate.node_id.clone(), branch_id));
    }
    Err(EvalStoreError::Validation {
        field: "selection_candidate.coordinate",
        detail: format!(
            "candidate '{}' has no node/branch coordinate",
            payload.candidate.as_str()
        ),
    })
}

fn payload_selected_by_decision(
    entry: &SelectionDecisionEntry,
    membership: Option<&CandidateSetMembership>,
    payload: &EvaluationPayload,
) -> bool {
    if let Some(selected_membership_id) = entry.selected_membership_id.as_ref() {
        return membership.and_then(|member| member.membership_id.as_ref())
            == Some(selected_membership_id);
    }
    if let Some(selected_occurrence_id) = entry.selected_occurrence_id.as_ref() {
        return membership.and_then(|member| member.occurrence_id.as_ref())
            == Some(selected_occurrence_id);
    }
    entry.selected_candidate.as_ref() == Some(&payload.candidate)
}

fn put_selection_decision_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalSelectionDecisionRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &SelectionDecisionSchema::SCHEMA,
        selection_decision_params(row),
        "put.eval_selection_decision",
    )?;
    Ok(())
}

fn put_selection_candidate_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalSelectionCandidateRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &SelectionCandidateSchema::SCHEMA,
        selection_candidate_params(row),
        "put.eval_selection_candidate",
    )?;
    Ok(())
}

fn put_selection_finding_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalSelectionFindingRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &SelectionFindingSchema::SCHEMA,
        selection_finding_params(row),
        "put.eval_selection_finding",
    )?;
    Ok(())
}

fn put_selection_score_row<D: EvalDb + ?Sized>(
    db: &D,
    row: &EvalSelectionScoreRow,
) -> Result<(), EvalStoreError> {
    put_eval_params(
        db,
        &SelectionScoreSchema::SCHEMA,
        selection_score_params(row),
        "put.eval_selection_score",
    )?;
    Ok(())
}

fn selection_decision_params(row: &EvalSelectionDecisionRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("decision_id".to_string(), row.decision_id.clone().into());
    params.insert("campaign_id".to_string(), row.campaign_id.clone().into());
    params.insert("parent_id".to_string(), row.parent_id.clone().into());
    params.insert("set_id".to_string(), row.set_id.clone().into());
    params.insert("procedure_id".to_string(), row.procedure_id.clone().into());
    params.insert(
        "selected_node_id".to_string(),
        option_string(&row.selected_node_id),
    );
    params.insert(
        "selected_artifact_id".to_string(),
        option_string(&row.selected_artifact_id),
    );
    params.insert("outcome".to_string(), row.outcome.clone().into());
    params.insert("disposition".to_string(), option_string(&row.disposition));
    params.insert("decision_ref".to_string(), option_string(&row.decision_ref));
    params.insert(
        "decision_hash".to_string(),
        option_string(&row.decision_hash),
    );
    params.insert("recorded_at".to_string(), option_string(&row.recorded_at));
    params
}

fn selection_finding_params(row: &EvalSelectionFindingRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("finding_id".to_string(), row.finding_id.clone().into());
    params.insert("decision_id".to_string(), row.decision_id.clone().into());
    params.insert("member_id".to_string(), option_string(&row.member_id));
    params.insert("domain".to_string(), row.domain.clone().into());
    params.insert("verdict".to_string(), row.verdict.clone().into());
    params.insert("confidence".to_string(), row.confidence.clone().into());
    params.insert("evidence_ref".to_string(), option_string(&row.evidence_ref));
    params.insert(
        "rationale_ref".to_string(),
        option_string(&row.rationale_ref),
    );
    params
}

fn selection_score_params(row: &EvalSelectionScoreRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("decision_id".to_string(), row.decision_id.clone().into());
    params.insert("member_id".to_string(), row.member_id.clone().into());
    params.insert("formula_id".to_string(), row.formula_id.clone().into());
    params.insert("score_json".to_string(), row.score_json.clone().into());
    params.insert("weight".to_string(), option_f64(row.weight));
    params.insert(
        "rank".to_string(),
        row.rank.map(DataValue::from).unwrap_or(DataValue::Null),
    );
    params.insert("selected".to_string(), DataValue::Bool(row.selected));
    params
}

fn selection_candidate_params(row: &EvalSelectionCandidateRow) -> BTreeMap<String, DataValue> {
    let mut params = BTreeMap::new();
    params.insert("decision_id".to_string(), row.decision_id.clone().into());
    params.insert("member_id".to_string(), row.member_id.clone().into());
    params.insert("node_id".to_string(), row.node_id.clone().into());
    params.insert("branch_id".to_string(), row.branch_id.clone().into());
    params.insert("selectable".to_string(), DataValue::Bool(row.selectable));
    params.insert("selected".to_string(), DataValue::Bool(row.selected));
    params.insert(
        "exclusion_ref".to_string(),
        option_string(&row.exclusion_ref),
    );
    params
}

fn option_f64(value: Option<f64>) -> DataValue {
    value
        .map(|value| DataValue::Num(cozo::Num::Float(value)))
        .unwrap_or(DataValue::Null)
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

fn serde_name<T: serde::Serialize>(value: &T) -> Result<String, EvalStoreError> {
    serde_json::to_value(value)
        .map_err(|source| EvalStoreError::Validation {
            field: "selection.serde_name",
            detail: source.to_string(),
        })
        .map(|value| match value {
            serde_json::Value::String(value) => value,
            other => other.to_string(),
        })
}

fn finding_id(
    campaign_id: &CampaignId,
    parent_id: &str,
    decision_id: &str,
    index: usize,
    domain: &str,
    verdict: &str,
    confidence: &str,
) -> String {
    hash_parts(&[
        "p1.eval.selection_finding.v1",
        campaign_id.as_str(),
        parent_id,
        decision_id,
        &index.to_string(),
        domain,
        verdict,
        confidence,
    ])
}

fn decision_id(
    campaign_id: &CampaignId,
    parent_id: &str,
    set_id: &str,
    procedure_id: &str,
    decision_hash: &str,
) -> String {
    hash_parts(&[
        "p1.eval.selection_decision.v1",
        campaign_id.as_str(),
        parent_id,
        set_id,
        procedure_id,
        decision_hash,
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
