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
    walk_client::WalkEvidenceQuery,
};

const RELATIONS_QUERY: &str = "::relations";

const OWNER_COUNTS_QUERY: &str = r#"
campaign[count(campaign_id)] := *eval_campaign { campaign_id }
profile[count(profile_ref_id)] := *eval_profile_commitment { profile_ref_id }
policy[count(campaign_id)] := *eval_run_profile_policy { campaign_id }
attempt[count(attempt_id)] := *eval_attempt { attempt_id }
provider_attempt[count(provider_attempt_id)] := *eval_provider_attempt { provider_attempt_id }
trace_event[count(trace_event_id)] := *eval_trace_event { trace_event_id }
agent_turn[count(turn_id)] := *eval_agent_turn { turn_id }
model_exchange[count(exchange_id)] := *eval_model_exchange { exchange_id }
tool_event[count(tool_event_id)] := *eval_tool_event { tool_event_id }
parent_identity[count(parent_id)] := *eval_parent_identity { parent_id }
parent_start[count(start_event_id)] := *eval_parent_start { start_event_id }
child_plan[count(plan_id)] := *eval_child_plan { plan_id }
child[count(child_node_id)] := *eval_child_plan_child { child_node_id }
rejected[count(attempt_index)] := *eval_child_plan_rejected_attempt { attempt_index }
scheduler[count(node_id)] := *eval_scheduler_node { node_id }
scheduler_event[count(status_event_id)] := *eval_scheduler_node_status_event { status_event_id }
runner_request[count(node_id)] := *eval_runner_request { node_id }
runner_result[count(result_path)] := *eval_runner_result { result_path }
evaluation[count(evaluation_id)] := *eval_evaluation { evaluation_id }
selection[count(decision_id)] := *eval_selection_decision { decision_id }
candidate[count(member_id)] := *eval_selection_candidate { member_id }
continuation[count(decision_id)] := *eval_continuation_decision { decision_id }
artifact[count(artifact_id)] := *eval_artifact { artifact_id }
invocation[count(invocation_id)] := *eval_invocation { invocation_id }
walk_event[count(event_id)] := *eval_walk_event { event_id }
walk_edge[count(event_id)] := *eval_walk_event_transition { event_id }

?[evidence_scope, relation, count] := campaign[count],
    evidence_scope = "owner_db_projection", relation = "eval_campaign"
?[evidence_scope, relation, count] := profile[count],
    evidence_scope = "owner_db_projection", relation = "eval_profile_commitment"
?[evidence_scope, relation, count] := policy[count],
    evidence_scope = "owner_db_projection", relation = "eval_run_profile_policy"
?[evidence_scope, relation, count] := attempt[count],
    evidence_scope = "owner_db_projection", relation = "eval_attempt"
?[evidence_scope, relation, count] := provider_attempt[count],
    evidence_scope = "owner_db_projection", relation = "eval_provider_attempt"
?[evidence_scope, relation, count] := trace_event[count],
    evidence_scope = "owner_db_projection", relation = "eval_trace_event"
?[evidence_scope, relation, count] := agent_turn[count],
    evidence_scope = "owner_db_projection", relation = "eval_agent_turn"
?[evidence_scope, relation, count] := model_exchange[count],
    evidence_scope = "owner_db_projection", relation = "eval_model_exchange"
?[evidence_scope, relation, count] := tool_event[count],
    evidence_scope = "owner_db_projection", relation = "eval_tool_event"
?[evidence_scope, relation, count] := parent_identity[count],
    evidence_scope = "owner_db_projection", relation = "eval_parent_identity"
?[evidence_scope, relation, count] := parent_start[count],
    evidence_scope = "owner_db_projection", relation = "eval_parent_start"
?[evidence_scope, relation, count] := child_plan[count],
    evidence_scope = "owner_db_projection", relation = "eval_child_plan"
?[evidence_scope, relation, count] := child[count],
    evidence_scope = "owner_db_projection", relation = "eval_child_plan_child"
?[evidence_scope, relation, count] := rejected[count],
    evidence_scope = "owner_db_projection", relation = "eval_child_plan_rejected_attempt"
?[evidence_scope, relation, count] := scheduler[count],
    evidence_scope = "owner_db_projection", relation = "eval_scheduler_node"
?[evidence_scope, relation, count] := scheduler_event[count],
    evidence_scope = "owner_db_projection", relation = "eval_scheduler_node_status_event"
?[evidence_scope, relation, count] := runner_request[count],
    evidence_scope = "owner_db_projection", relation = "eval_runner_request"
?[evidence_scope, relation, count] := runner_result[count],
    evidence_scope = "owner_db_projection", relation = "eval_runner_result"
?[evidence_scope, relation, count] := evaluation[count],
    evidence_scope = "owner_db_projection", relation = "eval_evaluation"
?[evidence_scope, relation, count] := selection[count],
    evidence_scope = "owner_db_projection", relation = "eval_selection_decision"
?[evidence_scope, relation, count] := candidate[count],
    evidence_scope = "owner_db_projection", relation = "eval_selection_candidate"
?[evidence_scope, relation, count] := continuation[count],
    evidence_scope = "owner_db_projection", relation = "eval_continuation_decision"
?[evidence_scope, relation, count] := artifact[count],
    evidence_scope = "owner_db_projection", relation = "eval_artifact"
?[evidence_scope, relation, count] := invocation[count],
    evidence_scope = "owner_db_projection", relation = "eval_invocation"
?[evidence_scope, relation, count] := walk_event[count],
    evidence_scope = "owner_db_projection", relation = "eval_walk_event"
?[evidence_scope, relation, count] := walk_edge[count],
    evidence_scope = "owner_db_projection", relation = "eval_walk_event_transition"
:sort relation
"#;

const CONFIG_EVIDENCE_QUERY: &str = r#"
?[evidence_scope, campaign_id, manifest_sha256, model_id, provider_slug, route_source,
  profile_ref_id, profile_name, profile_sha256, profile_schema, max_generations,
  max_total_nodes, child_min, child_max, parallel_targets, generation_source,
  control_mode, parallel_cap] :=
    *eval_campaign {
        campaign_id,
        manifest_sha256,
        model_id,
        provider_slug,
        route_source,
        profile_ref_id,
    },
    *eval_profile_commitment {
        profile_ref_id,
        profile_name,
        content_sha256: profile_sha256,
    },
    *eval_run_profile_policy {
        campaign_id,
        profile_ref_id,
        schema_version: profile_schema,
        max_generations,
        max_total_nodes,
        child_min,
        child_max,
        parallel_targets,
        generation_source,
        control_mode,
        parallel_cap,
    },
    evidence_scope = "owner_db_projection"
:sort campaign_id
"#;

const LINEAGE_QUERY: &str = r#"
?[evidence_scope, campaign_id, parent_id, node_id, generation, previous_parent_id,
  parent_node_id, branch_id, artifact_branch, identity_created_at, semantic_hash] :=
    *eval_parent_identity {
        campaign_id,
        parent_id,
        node_id,
        generation,
        previous_parent_id,
        parent_node_id,
        branch_id,
        artifact_branch,
        identity_created_at,
        semantic_hash,
    },
    evidence_scope = "owner_db_projection"
:sort campaign_id, generation, parent_id
"#;

const PROGRESS_QUERY: &str = r#"
evaluation_subject[campaign_id, evaluation_id, subject_ref, actor_generation,
                   subject_generation] :=
    *eval_evaluation {
        evaluation_id,
        campaign_id,
        parent_id,
        branch_id,
    },
    *eval_parent_identity {
        campaign_id,
        parent_id,
        node_id: actor_node_id,
        generation: actor_generation,
    },
    *eval_scheduler_node {
        campaign_id,
        branch_id,
        parent_node_id: actor_node_id,
        node_id: subject_ref,
        generation: subject_generation,
    },
    !is_null(parent_id)

evaluation_subject[campaign_id, evaluation_id, subject_ref, actor_generation,
                   subject_generation] :=
    *eval_evaluation {
        evaluation_id,
        campaign_id,
        parent_id,
        branch_id,
    },
    is_null(parent_id),
    *eval_scheduler_node {
        campaign_id,
        branch_id,
        parent_node_id: actor_node_id,
        node_id: subject_ref,
        generation: subject_generation,
    },
    *eval_parent_identity {
        campaign_id,
        node_id: actor_node_id,
        generation: actor_generation,
    }

evaluation_attributed[campaign_id, evaluation_id] :=
    evaluation_subject[campaign_id, evaluation_id, subject_ref, actor_generation,
                       subject_generation]

continue_disposition[disposition] <- [
    ["continue_ready"],
    ["continue_explore_from_rejected"],
    ["continue_historical_traversal"],
]

progress[stage, campaign_id, subject_kind, subject_ref, entity_kind, entity_id,
         actor_generation, subject_generation, successor_generation, status,
         disposition, evidence_ref, recorded_at] :=
    *eval_child_plan {
        plan_id: entity_id,
        campaign_id,
        parent_node_id: subject_ref,
        child_generation: successor_generation,
        message_sha256: evidence_ref,
        recorded_at,
    },
    *eval_parent_identity {
        campaign_id,
        node_id: subject_ref,
        generation: actor_generation,
    },
    stage = "child_plan",
    subject_kind = "parent_node",
    entity_kind = "child_plan",
    subject_generation = actor_generation,
    status = "recorded",
    disposition = null

progress[stage, campaign_id, subject_kind, subject_ref, entity_kind, entity_id,
         actor_generation, subject_generation, successor_generation, status,
         disposition, evidence_ref, recorded_at] :=
    *eval_scheduler_node {
        campaign_id,
        node_id: entity_id,
        parent_node_id: subject_ref,
        generation: successor_generation,
        status,
        content_sha256: evidence_ref,
        updated_at: recorded_at,
    },
    *eval_parent_identity {
        campaign_id,
        node_id: subject_ref,
        generation: actor_generation,
    },
    stage = "scheduler",
    subject_kind = "parent_node",
    entity_kind = "node",
    subject_generation = actor_generation,
    disposition = null

progress[stage, campaign_id, subject_kind, subject_ref, entity_kind, entity_id,
         actor_generation, subject_generation, successor_generation, status,
         disposition, evidence_ref, recorded_at] :=
    *eval_runner_result {
        campaign_id,
        node_id: subject_ref,
        result_path: entity_id,
        generation: subject_generation,
        status,
        disposition,
        content_sha256: evidence_ref,
        recorded_at,
    },
    *eval_scheduler_node {
        campaign_id,
        node_id: subject_ref,
        parent_node_id: actor_node_id,
    },
    *eval_parent_identity {
        campaign_id,
        node_id: actor_node_id,
        generation: actor_generation,
    },
    stage = "runner",
    subject_kind = "node",
    entity_kind = "runner_result",
    successor_generation = null

progress[stage, campaign_id, subject_kind, subject_ref, entity_kind, entity_id,
         actor_generation, subject_generation, successor_generation, status,
         disposition, evidence_ref, recorded_at] :=
    *eval_evaluation {
        evaluation_id: entity_id,
        campaign_id,
        disposition,
        record_ref: evidence_ref,
        recorded_at,
    },
    evaluation_subject[campaign_id, entity_id, subject_ref, actor_generation,
                       subject_generation],
    stage = "evaluation",
    subject_kind = "node",
    entity_kind = "evaluation",
    successor_generation = null,
    status = "recorded"

progress[stage, campaign_id, subject_kind, subject_ref, entity_kind, entity_id,
         actor_generation, subject_generation, successor_generation, status,
         disposition, evidence_ref, recorded_at] :=
    *eval_evaluation {
        evaluation_id: entity_id,
        campaign_id,
        disposition,
        record_ref: evidence_ref,
        recorded_at,
    },
    not evaluation_attributed[campaign_id, entity_id],
    stage = "evaluation",
    subject_kind = "node",
    subject_ref = null,
    entity_kind = "evaluation",
    actor_generation = null,
    subject_generation = null,
    successor_generation = null,
    status = "recorded"

progress[stage, campaign_id, subject_kind, subject_ref, entity_kind, entity_id,
         actor_generation, subject_generation, successor_generation, status,
         disposition, evidence_ref, recorded_at] :=
    *eval_selection_decision {
        decision_id: entity_id,
        campaign_id,
        parent_id: subject_ref,
        outcome: status,
        disposition,
        decision_hash: evidence_ref,
        recorded_at,
    },
    *eval_parent_identity {
        campaign_id,
        parent_id: subject_ref,
        generation: actor_generation,
    },
    stage = "selection",
    subject_kind = "parent",
    entity_kind = "selection_decision",
    subject_generation = actor_generation,
    successor_generation = null

progress[stage, campaign_id, subject_kind, subject_ref, entity_kind, entity_id,
         actor_generation, subject_generation, successor_generation, status,
         disposition, evidence_ref, recorded_at] :=
    *eval_continuation_decision {
        decision_id: entity_id,
        campaign_id,
        parent_id: subject_ref,
        disposition,
        next_generation: successor_generation,
        policy_ref: evidence_ref,
        recorded_at,
    },
    *eval_parent_identity {
        campaign_id,
        parent_id: subject_ref,
        generation: actor_generation,
    },
    continue_disposition[disposition],
    stage = "continuation",
    subject_kind = "parent",
    entity_kind = "continuation_decision",
    subject_generation = actor_generation,
    status = "recorded"

progress[stage, campaign_id, subject_kind, subject_ref, entity_kind, entity_id,
         actor_generation, subject_generation, successor_generation, status,
         disposition, evidence_ref, recorded_at] :=
    *eval_continuation_decision {
        decision_id: entity_id,
        campaign_id,
        parent_id: subject_ref,
        disposition,
        policy_ref: evidence_ref,
        recorded_at,
    },
    *eval_parent_identity {
        campaign_id,
        parent_id: subject_ref,
        generation: actor_generation,
    },
    not continue_disposition[disposition],
    stage = "continuation",
    subject_kind = "parent",
    entity_kind = "continuation_decision",
    subject_generation = actor_generation,
    successor_generation = null,
    status = "recorded"

?[evidence_scope, stage, campaign_id, subject_kind, subject_ref, entity_kind,
  entity_id, actor_generation, subject_generation, successor_generation, status,
  disposition, evidence_ref, recorded_at] :=
    progress[stage, campaign_id, subject_kind, subject_ref, entity_kind, entity_id,
             actor_generation, subject_generation, successor_generation, status,
             disposition, evidence_ref, recorded_at],
    evidence_scope = "owner_db_projection"
:sort campaign_id, actor_generation, successor_generation, stage, entity_id
"#;

const HANDOFF_EVIDENCE_QUERY: &str = r#"
continue_disposition[disposition] <- [
    ["continue_ready"],
    ["continue_explore_from_rejected"],
    ["continue_historical_traversal"],
]

handoff[stage, campaign_id, actor_parent_id, actor_generation, successor_parent_id,
        successor_node_id, successor_branch_id, successor_generation, runtime_id,
        status, evidence_id] :=
    *eval_selection_decision {
        decision_id: evidence_id,
        campaign_id,
        parent_id: actor_parent_id,
        selected_node_id: successor_node_id,
        outcome: status,
    },
    *eval_parent_identity {
        campaign_id,
        parent_id: actor_parent_id,
        generation: actor_generation,
    },
    !is_null(successor_node_id),
    stage = "selection",
    successor_parent_id = null,
    successor_branch_id = null,
    successor_generation = null,
    runtime_id = null

handoff[stage, campaign_id, actor_parent_id, actor_generation, successor_parent_id,
        successor_node_id, successor_branch_id, successor_generation, runtime_id,
        status, evidence_id] :=
    *eval_continuation_decision {
        decision_id: evidence_id,
        campaign_id,
        parent_id: actor_parent_id,
        selected_branch_id: successor_branch_id,
        next_generation: successor_generation,
        disposition: status,
    },
    *eval_parent_identity {
        campaign_id,
        parent_id: actor_parent_id,
        generation: actor_generation,
    },
    continue_disposition[status],
    !is_null(successor_branch_id),
    stage = "continuation",
    successor_parent_id = null,
    successor_node_id = null,
    runtime_id = null

handoff[stage, campaign_id, actor_parent_id, actor_generation, successor_parent_id,
        successor_node_id, successor_branch_id, successor_generation, runtime_id,
        status, evidence_id] :=
    *eval_parent_start {
        start_event_id: evidence_id,
        campaign_id,
        parent_id: successor_parent_id,
        node_id: successor_node_id,
        generation: successor_generation,
        startup_kind: status,
        handoff_runtime_id: runtime_id,
    },
    *eval_parent_identity {
        campaign_id,
        parent_id: successor_parent_id,
        node_id: successor_node_id,
        generation: successor_generation,
        previous_parent_id: actor_parent_id,
        branch_id: successor_branch_id,
    },
    *eval_parent_identity {
        campaign_id,
        parent_id: actor_parent_id,
        generation: actor_generation,
    },
    stage = "parent_start"

handoff[stage, campaign_id, actor_parent_id, actor_generation, successor_parent_id,
        successor_node_id, successor_branch_id, successor_generation, runtime_id,
        status, evidence_id] :=
    *eval_walk_event {
        event_id: evidence_id,
        campaign_id,
        parent_id: actor_parent_id,
        generation: actor_generation,
        status,
        phase_after,
    },
    phase_after = "r13b",
    stage = "handoff_commit",
    successor_parent_id = null,
    successor_node_id = null,
    successor_branch_id = null,
    successor_generation = null,
    runtime_id = null

?[evidence_scope, stage, campaign_id, actor_parent_id, actor_generation,
  successor_parent_id, successor_node_id, successor_branch_id, successor_generation,
  runtime_id, status, evidence_id] :=
    handoff[stage, campaign_id, actor_parent_id, actor_generation, successor_parent_id,
            successor_node_id, successor_branch_id, successor_generation, runtime_id,
            status, evidence_id],
    evidence_scope = "owner_db_projection"
:sort campaign_id, actor_generation, successor_generation, stage, evidence_id
"#;

pub(crate) const fn evidence_query_script(query: WalkEvidenceQuery) -> &'static str {
    match query {
        WalkEvidenceQuery::Relations => RELATIONS_QUERY,
        WalkEvidenceQuery::Counts => OWNER_COUNTS_QUERY,
        WalkEvidenceQuery::ConfigEvidence => CONFIG_EVIDENCE_QUERY,
        WalkEvidenceQuery::Lineage => LINEAGE_QUERY,
        WalkEvidenceQuery::Progress => PROGRESS_QUERY,
        WalkEvidenceQuery::HandoffEvidence => HANDOFF_EVIDENCE_QUERY,
    }
}

/// One immutable query whose provenance determines how its script is selected.
#[derive(Debug, Clone)]
pub(crate) enum SnapshotQuery {
    /// Exact expert script supplied by the client.
    Raw(String),
    /// Closed projection whose canonical script remains server-owned.
    Evidence(WalkEvidenceQuery),
}

impl SnapshotQuery {
    fn into_parts(self) -> (String, Option<WalkEvidenceQuery>) {
        match self {
            Self::Raw(script) => (script, None),
            Self::Evidence(view) => (evidence_query_script(view).to_string(), Some(view)),
        }
    }
}

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
    pub campaign_id: CampaignId,
    pub db_path: PathBuf,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub view: Option<WalkEvidenceQuery>,
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
    query: SnapshotQuery,
) -> Result<DbQueryResult, PrepareError> {
    let (script, view) = query.into_parts();
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
        view,
        revision,
        &result,
    ))
}

fn query_result_view(
    repo_root: PathBuf,
    campaign_id: CampaignId,
    db_path: PathBuf,
    script: String,
    view: Option<WalkEvidenceQuery>,
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
        campaign_id,
        db_path,
        view,
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

        let first = run_snapshot_query(
            tmp.path().join("parent"),
            campaign.clone(),
            SnapshotQuery::Raw(fixture_query()),
        )
        .expect("first immutable query");
        let repeated = run_snapshot_query(
            tmp.path().join("parent"),
            campaign.clone(),
            SnapshotQuery::Raw(fixture_query()),
        )
        .expect("repeated immutable query");
        assert_eq!(first.revision, repeated.revision);
        assert_eq!(first.rows[0].object, serde_json::json!({"value": 1}));

        publish_fixture(&db_path, 2);
        let changed = run_snapshot_query(
            tmp.path().join("parent"),
            campaign,
            SnapshotQuery::Raw(fixture_query()),
        )
        .expect("changed immutable query");
        assert_ne!(first.revision, changed.revision);
        assert_eq!(changed.rows[0].object, serde_json::json!({"value": 2}));
    }

    #[test]
    fn named_evidence_queries_parse_against_current_schema() {
        let db = ploke_db::Database::new_init().expect("named-query database");
        crate::cli::prototype1_state::eval_store::DbEvalStore::new(&db)
            .install_schema()
            .expect("install current eval schema");

        let relations = db
            .raw_query_params(
                evidence_query_script(WalkEvidenceQuery::Relations),
                BTreeMap::new(),
            )
            .expect("relations evidence query");
        assert!(
            relations.headers.iter().any(|header| header == "name"),
            "relations view retains Cozo relation metadata"
        );

        for (query, headers) in [
            (
                WalkEvidenceQuery::Counts,
                &["evidence_scope", "relation", "count"][..],
            ),
            (
                WalkEvidenceQuery::ConfigEvidence,
                &[
                    "evidence_scope",
                    "campaign_id",
                    "manifest_sha256",
                    "model_id",
                    "provider_slug",
                    "route_source",
                    "profile_ref_id",
                    "profile_name",
                    "profile_sha256",
                    "profile_schema",
                    "max_generations",
                    "max_total_nodes",
                    "child_min",
                    "child_max",
                    "parallel_targets",
                    "generation_source",
                    "control_mode",
                    "parallel_cap",
                ][..],
            ),
            (
                WalkEvidenceQuery::Lineage,
                &[
                    "evidence_scope",
                    "campaign_id",
                    "parent_id",
                    "node_id",
                    "generation",
                    "previous_parent_id",
                    "parent_node_id",
                    "branch_id",
                    "artifact_branch",
                    "identity_created_at",
                    "semantic_hash",
                ][..],
            ),
            (
                WalkEvidenceQuery::Progress,
                &[
                    "evidence_scope",
                    "stage",
                    "campaign_id",
                    "subject_kind",
                    "subject_ref",
                    "entity_kind",
                    "entity_id",
                    "actor_generation",
                    "subject_generation",
                    "successor_generation",
                    "status",
                    "disposition",
                    "evidence_ref",
                    "recorded_at",
                ][..],
            ),
            (
                WalkEvidenceQuery::HandoffEvidence,
                &[
                    "evidence_scope",
                    "stage",
                    "campaign_id",
                    "actor_parent_id",
                    "actor_generation",
                    "successor_parent_id",
                    "successor_node_id",
                    "successor_branch_id",
                    "successor_generation",
                    "runtime_id",
                    "status",
                    "evidence_id",
                ][..],
            ),
        ] {
            let result = db
                .raw_query_params(evidence_query_script(query), BTreeMap::new())
                .unwrap_or_else(|error| panic!("{query:?} evidence query failed: {error}"));
            assert_eq!(
                result
                    .headers
                    .iter()
                    .map(String::as_str)
                    .collect::<Vec<_>>(),
                headers,
                "{query:?} headers drifted"
            );
        }
    }

    #[test]
    fn core_counts_include_critical_typed_trace_relations() {
        let db = ploke_db::Database::new_init().expect("core-count database");
        crate::cli::prototype1_state::eval_store::DbEvalStore::new(&db)
            .install_schema()
            .expect("install current eval schema");

        let result = db
            .raw_query_params(
                evidence_query_script(WalkEvidenceQuery::Counts),
                BTreeMap::new(),
            )
            .expect("core-count evidence query");
        let rows = result
            .rows
            .iter()
            .map(|row| query_row_object(&result.headers, row))
            .collect::<Vec<_>>();

        for relation in [
            "eval_attempt",
            "eval_provider_attempt",
            "eval_agent_turn",
            "eval_model_exchange",
            "eval_tool_event",
            "eval_trace_event",
        ] {
            assert!(
                rows.iter().any(|row| {
                    row["relation"] == relation
                        && row["evidence_scope"] == "owner_db_projection"
                        && row["count"] == 0
                }),
                "curated core counts omitted typed trace relation {relation}: {rows:#?}"
            );
        }
    }

    #[test]
    fn named_evidence_query_identity_is_preserved() {
        for view in [
            WalkEvidenceQuery::Relations,
            WalkEvidenceQuery::Counts,
            WalkEvidenceQuery::ConfigEvidence,
            WalkEvidenceQuery::Lineage,
            WalkEvidenceQuery::Progress,
            WalkEvidenceQuery::HandoffEvidence,
        ] {
            let (script, identity) = SnapshotQuery::Evidence(view).into_parts();
            let result = query_result_view(
                PathBuf::from("/repo"),
                CampaignId::from("campaign"),
                PathBuf::from("/owner.cozo"),
                script.clone(),
                identity,
                ReadRevision::from_bytes(b"owner"),
                &QueryResult {
                    headers: Vec::new(),
                    rows: Vec::new(),
                },
            );
            assert_eq!(result.view, Some(view));
            assert_eq!(result.script, evidence_query_script(view));

            let (script, identity) = SnapshotQuery::Raw(script).into_parts();
            let raw = query_result_view(
                PathBuf::from("/repo"),
                CampaignId::from("campaign"),
                PathBuf::from("/owner.cozo"),
                script,
                identity,
                ReadRevision::from_bytes(b"owner"),
                &QueryResult {
                    headers: Vec::new(),
                    rows: Vec::new(),
                },
            );
            assert_eq!(raw.view, None);
        }
    }

    #[test]
    fn named_evidence_queries_preserve_reference_kinds_and_handoff_chain() {
        let db = ploke_db::Database::new_init().expect("named-query database");
        crate::cli::prototype1_state::eval_store::DbEvalStore::new(&db)
            .install_schema()
            .expect("install current eval schema");
        db.raw_query_mut(
            r#"
?[plan_id, campaign_id, schema_version, parent_node_id, child_generation,
  message_path, message_sha256, child_count, rejected_count, recorded_at,
  ingested_at] <- [
    ["plan-1", "campaign", "prototype1-child-plan-file.v1", "node-1", 1,
     "/plans/1.json", "plan-hash-1", 1, 0, "2026-07-26T00:00:00Z",
     "2026-07-26T00:00:00Z"],
]
:put eval_child_plan {
    plan_id =>
    campaign_id,
    schema_version,
    parent_node_id,
    child_generation,
    message_path,
    message_sha256,
    child_count,
    rejected_count,
    recorded_at,
    ingested_at,
}
"#,
        )
        .expect("seed child-plan evidence");
        db.raw_query_mut(
            r#"
?[campaign_id, node_id, projection_schema_version, node_schema_version,
  parent_node_id, generation, instance_id, source_state_id,
  operation_target_kind, base_artifact_id, patch_id, derived_artifact_id,
  parent_branch_id, branch_id, candidate_id, target_relpath, node_path,
  node_dir, workspace_root, binary_path, runner_request_path,
  runner_result_path, status, created_at, updated_at, content_sha256,
  ingested_at] <- [
    ["campaign", "node-2", "prototype1-scheduler-node.v1", "node-schema.v1",
     "node-1", 1, "instance-1", "state-1", null, null, null, null,
     "branch-1", "branch-2", "candidate-2", "src/lib.rs", "/nodes/2.json",
     "/nodes/2", "/workspace/2", "/bin/runner", "/requests/2.json",
     "/results/2.json", "succeeded", "2026-07-26T00:00:01Z",
     "2026-07-26T00:00:02Z", "node-hash-2", "2026-07-26T00:00:02Z"],
]
:put eval_scheduler_node {
    campaign_id,
    node_id =>
    projection_schema_version,
    node_schema_version,
    parent_node_id,
    generation,
    instance_id,
    source_state_id,
    operation_target_kind,
    base_artifact_id,
    patch_id,
    derived_artifact_id,
    parent_branch_id,
    branch_id,
    candidate_id,
    target_relpath,
    node_path,
    node_dir,
    workspace_root,
    binary_path,
    runner_request_path,
    runner_result_path,
    status,
    created_at,
    updated_at,
    content_sha256,
    ingested_at,
}
"#,
        )
        .expect("seed scheduler evidence");
        db.raw_query_mut(
            r#"
?[campaign_id, node_id, result_path, projection_schema_version,
  result_schema_version, generation, branch_id, status, disposition,
  treatment_campaign_id, evaluation_artifact_path, detail, exit_code,
  stdout_excerpt, stderr_excerpt, runtime_id, path_kind, content_sha256,
  recorded_at, ingested_at] <- [
    ["campaign", "node-2", "/results/2.json", "prototype1-runner-result.v1",
     "runner-result.v1", 1, "branch-2", "succeeded", "keep", null, null,
     null, 0, null, null, "runtime-2", "absolute", "runner-hash-2",
     "2026-07-26T00:00:03Z", "2026-07-26T00:00:03Z"],
]
:put eval_runner_result {
    campaign_id,
    node_id,
    result_path =>
    projection_schema_version,
    result_schema_version,
    generation,
    branch_id,
    status,
    disposition,
    treatment_campaign_id,
    evaluation_artifact_path,
    detail,
    exit_code,
    stdout_excerpt,
    stderr_excerpt,
    runtime_id,
    path_kind,
    content_sha256,
    recorded_at,
    ingested_at,
}
"#,
        )
        .expect("seed runner evidence");
        db.raw_query_mut(
            r#"
?[evaluation_id, campaign_id, parent_id, branch_id, baseline_id, treatment_id,
  procedure_id, evaluator_id, eval_set_id, policy_ref, disposition, record_ref,
  recorded_at] <- [
    ["evaluation-1", "campaign", null, "branch-2", null, null,
     "procedure-1", "evaluator-1", "eval-set-1", "eval-policy-1", "keep",
     "evaluation-ref-1", "2026-07-26T00:00:04Z"],
    ["evaluation-unattributed", "campaign", null, "branch-unattributed", null,
     null, "procedure-1", "evaluator-1", "eval-set-1", "eval-policy-1",
     "reject", "evaluation-ref-unattributed", "2026-07-26T00:00:05Z"],
]
:put eval_evaluation {
    evaluation_id =>
    campaign_id,
    parent_id,
    branch_id,
    baseline_id,
    treatment_id,
    procedure_id,
    evaluator_id,
    eval_set_id,
    policy_ref,
    disposition,
    record_ref,
    recorded_at,
}
"#,
        )
        .expect("seed evaluation evidence");
        db.raw_query_mut(
            r#"
?[decision_id, campaign_id, parent_id, set_id, procedure_id, selected_node_id,
  selected_artifact_id, outcome, disposition, decision_ref, decision_hash,
  recorded_at] <- [
    ["selection-1", "campaign", "parent-1", "set-1", "procedure-1", "node-2",
     "artifact-2", "accepted", "keep", "selection-ref-1", "selection-hash-1",
     "2026-07-26T00:00:05Z"],
]
:put eval_selection_decision {
    decision_id =>
    campaign_id,
    parent_id,
    set_id,
    procedure_id,
    selected_node_id,
    selected_artifact_id,
    outcome,
    disposition,
    decision_ref,
    decision_hash,
    recorded_at,
}
"#,
        )
        .expect("seed selection evidence");
        db.raw_query_mut(
            r#"
?[decision_id, campaign_id, parent_id, disposition, selected_branch_id,
  next_generation, total_nodes, policy_ref, recorded_at] <- [
    ["continue-ready", "campaign", "parent-1", "continue_ready", "branch-2", 1, 2,
     "policy-1", "2026-07-26T00:00:00Z"],
    ["continue-rejected", "campaign", "parent-1",
     "continue_explore_from_rejected", "branch-2", 1, 2, "policy-1",
     "2026-07-26T00:00:01Z"],
    ["continue-history", "campaign", "parent-1",
     "continue_historical_traversal", "branch-2", 1, 2, "policy-1",
     "2026-07-26T00:00:02Z"],
    ["stop-1", "campaign", "parent-1", "stop_max_generations", "branch-2", 1,
     2, null,
     "2026-07-26T00:00:01Z"],
]
:put eval_continuation_decision {
    decision_id =>
    campaign_id,
    parent_id,
    disposition,
    selected_branch_id,
    next_generation,
    total_nodes,
    policy_ref,
    recorded_at,
}
"#,
        )
        .expect("seed continuation evidence");
        db.raw_query_mut(
            r#"
?[campaign_id, parent_id, schema_version, identity_schema_version, node_id,
  generation, branch_id, artifact_branch, instance_id, previous_parent_id,
  parent_node_id, identity_created_at, semantic_hash, ingested_at] <- [
    ["campaign", "parent-1", "eval-parent-identity.v1",
     "prototype1-parent-identity.v1", "node-1", 0, "branch-1", null, null,
     null, null, "2026-07-26T00:00:00Z", "hash-1",
     "2026-07-26T00:00:00Z"],
    ["campaign", "parent-2", "eval-parent-identity.v1",
     "prototype1-parent-identity.v1", "node-2", 1, "branch-2", null, null,
     "parent-1", "node-1", "2026-07-26T00:00:01Z", "hash-2",
     "2026-07-26T00:00:01Z"],
    ["campaign", "parent-2-alt", "eval-parent-identity.v1",
     "prototype1-parent-identity.v1", "node-2-alt", 1, "branch-2-alt", null,
     null, "parent-1", "node-1", "2026-07-26T00:00:01Z", "hash-2-alt",
     "2026-07-26T00:00:01Z"],
    ["campaign", "parent-3", "eval-parent-identity.v1",
     "prototype1-parent-identity.v1", "node-3", 2, "branch-3", null, null,
     "parent-2", "node-2", "2026-07-26T00:00:02Z", "hash-3",
     "2026-07-26T00:00:02Z"],
]
:put eval_parent_identity {
    campaign_id,
    parent_id =>
    schema_version,
    identity_schema_version,
    node_id,
    generation,
    branch_id,
    artifact_branch,
    instance_id,
    previous_parent_id,
    parent_node_id,
    identity_created_at,
    semantic_hash,
    ingested_at,
}
"#,
        )
        .expect("seed lineage evidence");
        db.raw_query_mut(
            r#"
?[start_event_id, campaign_id, schema_version, parent_id, node_id, generation,
  branch_id, repo_root, startup_kind, handoff_runtime_id, pid, source_stream_id,
  source_event_index, source_line, parent_recorded_at, resource_recorded_at,
  semantic_hash, ingested_at] <- [
    ["start-2", "campaign", "eval-parent-start.v1", "parent-2", "node-2", 1,
     "branch-2", "/repo/2", "successor", "runtime-2", 2, "stream-2", 0, 1,
     10, 11, "start-hash-2", "2026-07-26T00:00:01Z"],
    ["start-3", "campaign", "eval-parent-start.v1", "parent-3", "node-3", 2,
     "branch-3", "/repo/3", "successor", "runtime-3", 3, "stream-3", 0, 1,
     20, 21, "start-hash-3", "2026-07-26T00:00:02Z"],
]
:put eval_parent_start {
    start_event_id =>
    campaign_id,
    schema_version,
    parent_id,
    node_id,
    generation,
    branch_id,
    repo_root,
    startup_kind,
    handoff_runtime_id,
    pid,
    source_stream_id,
    source_event_index,
    source_line,
    parent_recorded_at,
    resource_recorded_at,
    semantic_hash,
    ingested_at,
}
"#,
        )
        .expect("seed parent-start evidence");
        db.raw_query_mut(
            r#"
?[event_id, campaign_id, schema_version, node_id, parent_id, generation, branch_id,
  command, status, phase_before, phase_after, target_phase, watch, allow_live_api,
  allow_git_changes, transition_count, protocol_version, transition_graph_version,
  repo_root, exe_path, exe_sha256, exe_modified_unix_ms, git_head,
  source_status_hash, recorded_at, ingested_at] <- [
    ["handoff-1", "campaign", "eval-walk-event.v1", "node-1", "parent-1", 0,
     "branch-1", "step", "ok", "r12", "r13b", "r13b", false, false, true, 1,
     12, "walk-graph", "/repo/1", "/bin/ploke-eval", "exe-hash", null, null,
     null, "2026-07-26T00:00:01Z", "2026-07-26T00:00:01Z"],
]
:put eval_walk_event {
    event_id =>
    campaign_id,
    schema_version,
    node_id,
    parent_id,
    generation,
    branch_id,
    command,
    status,
    phase_before,
    phase_after,
    target_phase,
    watch,
    allow_live_api,
    allow_git_changes,
    transition_count,
    protocol_version,
    transition_graph_version,
    repo_root,
    exe_path,
    exe_sha256,
    exe_modified_unix_ms,
    git_head,
    source_status_hash,
    recorded_at,
    ingested_at,
}
"#,
        )
        .expect("seed ambiguous handoff commit");

        let progress = db
            .raw_query_params(
                evidence_query_script(WalkEvidenceQuery::Progress),
                BTreeMap::new(),
            )
            .expect("progress evidence query");
        let progress_rows = progress
            .rows
            .iter()
            .map(|row| query_row_object(&progress.headers, row))
            .collect::<Vec<_>>();
        let expected = [
            serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "child_plan",
                "campaign_id": "campaign",
                "subject_kind": "parent_node",
                "subject_ref": "node-1",
                "entity_kind": "child_plan",
                "entity_id": "plan-1",
                "actor_generation": 0,
                "subject_generation": 0,
                "successor_generation": 1,
                "status": "recorded",
                "disposition": null,
                "evidence_ref": "plan-hash-1",
                "recorded_at": "2026-07-26T00:00:00Z",
            }),
            serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "scheduler",
                "campaign_id": "campaign",
                "subject_kind": "parent_node",
                "subject_ref": "node-1",
                "entity_kind": "node",
                "entity_id": "node-2",
                "actor_generation": 0,
                "subject_generation": 0,
                "successor_generation": 1,
                "status": "succeeded",
                "disposition": null,
                "evidence_ref": "node-hash-2",
                "recorded_at": "2026-07-26T00:00:02Z",
            }),
            serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "runner",
                "campaign_id": "campaign",
                "subject_kind": "node",
                "subject_ref": "node-2",
                "entity_kind": "runner_result",
                "entity_id": "/results/2.json",
                "actor_generation": 0,
                "subject_generation": 1,
                "successor_generation": null,
                "status": "succeeded",
                "disposition": "keep",
                "evidence_ref": "runner-hash-2",
                "recorded_at": "2026-07-26T00:00:03Z",
            }),
            serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "evaluation",
                "campaign_id": "campaign",
                "subject_kind": "node",
                "subject_ref": "node-2",
                "entity_kind": "evaluation",
                "entity_id": "evaluation-1",
                "actor_generation": 0,
                "subject_generation": 1,
                "successor_generation": null,
                "status": "recorded",
                "disposition": "keep",
                "evidence_ref": "evaluation-ref-1",
                "recorded_at": "2026-07-26T00:00:04Z",
            }),
            serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "selection",
                "campaign_id": "campaign",
                "subject_kind": "parent",
                "subject_ref": "parent-1",
                "entity_kind": "selection_decision",
                "entity_id": "selection-1",
                "actor_generation": 0,
                "subject_generation": 0,
                "successor_generation": null,
                "status": "accepted",
                "disposition": "keep",
                "evidence_ref": "selection-hash-1",
                "recorded_at": "2026-07-26T00:00:05Z",
            }),
            serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "evaluation",
                "campaign_id": "campaign",
                "subject_kind": "node",
                "subject_ref": null,
                "entity_kind": "evaluation",
                "entity_id": "evaluation-unattributed",
                "actor_generation": null,
                "subject_generation": null,
                "successor_generation": null,
                "status": "recorded",
                "disposition": "reject",
                "evidence_ref": "evaluation-ref-unattributed",
                "recorded_at": "2026-07-26T00:00:05Z",
            }),
            serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "continuation",
                "campaign_id": "campaign",
                "subject_kind": "parent",
                "subject_ref": "parent-1",
                "entity_kind": "continuation_decision",
                "entity_id": "continue-ready",
                "actor_generation": 0,
                "subject_generation": 0,
                "successor_generation": 1,
                "status": "recorded",
                "disposition": "continue_ready",
                "evidence_ref": "policy-1",
                "recorded_at": "2026-07-26T00:00:00Z",
            }),
            serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "continuation",
                "campaign_id": "campaign",
                "subject_kind": "parent",
                "subject_ref": "parent-1",
                "entity_kind": "continuation_decision",
                "entity_id": "continue-rejected",
                "actor_generation": 0,
                "subject_generation": 0,
                "successor_generation": 1,
                "status": "recorded",
                "disposition": "continue_explore_from_rejected",
                "evidence_ref": "policy-1",
                "recorded_at": "2026-07-26T00:00:01Z",
            }),
            serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "continuation",
                "campaign_id": "campaign",
                "subject_kind": "parent",
                "subject_ref": "parent-1",
                "entity_kind": "continuation_decision",
                "entity_id": "continue-history",
                "actor_generation": 0,
                "subject_generation": 0,
                "successor_generation": 1,
                "status": "recorded",
                "disposition": "continue_historical_traversal",
                "evidence_ref": "policy-1",
                "recorded_at": "2026-07-26T00:00:02Z",
            }),
            serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "continuation",
                "campaign_id": "campaign",
                "subject_kind": "parent",
                "subject_ref": "parent-1",
                "entity_kind": "continuation_decision",
                "entity_id": "stop-1",
                "actor_generation": 0,
                "subject_generation": 0,
                "successor_generation": null,
                "status": "recorded",
                "disposition": "stop_max_generations",
                "evidence_ref": null,
                "recorded_at": "2026-07-26T00:00:01Z",
            }),
        ];
        assert_eq!(progress_rows.len(), expected.len(), "{progress_rows:#?}");
        for row in expected {
            assert!(
                progress_rows.contains(&row),
                "missing progress row {row:#?}"
            );
        }

        let handoff = db
            .raw_query_params(
                evidence_query_script(WalkEvidenceQuery::HandoffEvidence),
                BTreeMap::new(),
            )
            .expect("handoff evidence query");
        let handoff_rows = handoff
            .rows
            .iter()
            .map(|row| query_row_object(&handoff.headers, row))
            .collect::<Vec<_>>();
        assert_eq!(
            handoff_rows
                .iter()
                .filter(|row| row["stage"] == "parent_start")
                .count(),
            2
        );
        assert!(handoff_rows.contains(&serde_json::json!({
            "evidence_scope": "owner_db_projection",
            "stage": "parent_start",
            "campaign_id": "campaign",
            "actor_parent_id": "parent-1",
            "actor_generation": 0,
            "successor_parent_id": "parent-2",
            "successor_node_id": "node-2",
            "successor_branch_id": "branch-2",
            "successor_generation": 1,
            "runtime_id": "runtime-2",
            "status": "successor",
            "evidence_id": "start-2",
        })));
        assert!(handoff_rows.contains(&serde_json::json!({
            "evidence_scope": "owner_db_projection",
            "stage": "parent_start",
            "campaign_id": "campaign",
            "actor_parent_id": "parent-2",
            "actor_generation": 1,
            "successor_parent_id": "parent-3",
            "successor_node_id": "node-3",
            "successor_branch_id": "branch-3",
            "successor_generation": 2,
            "runtime_id": "runtime-3",
            "status": "successor",
            "evidence_id": "start-3",
        })));
        assert_eq!(
            handoff_rows
                .iter()
                .filter(|row| row["stage"] == "handoff_commit")
                .collect::<Vec<_>>(),
            vec![&serde_json::json!({
                "evidence_scope": "owner_db_projection",
                "stage": "handoff_commit",
                "campaign_id": "campaign",
                "actor_parent_id": "parent-1",
                "actor_generation": 0,
                "successor_parent_id": null,
                "successor_node_id": null,
                "successor_branch_id": null,
                "successor_generation": null,
                "runtime_id": null,
                "status": "ok",
                "evidence_id": "handoff-1",
            })]
        );
        assert!(
            handoff_rows
                .iter()
                .all(|row| row["evidence_id"] != "stop-1"),
            "stop disposition is not handoff evidence even with a selected branch"
        );
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
