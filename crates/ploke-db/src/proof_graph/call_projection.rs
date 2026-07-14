use serde_json::Value;
use std::collections::{BTreeMap, HashSet};
use uuid::Uuid;

use cozo::{DataValue, ScriptMutability, UuidWrapper};

use crate::{
    Database, DbError,
    call_graph::{CallContextRow, CallStatusKind},
    database::{to_string, to_string_list},
};

use super::{PROOF_FACT_SCHEMA_VERSION, ProofGraphStore};

mod context;
mod facts;
mod source;

use context::validate_call_context;
use facts::{binding_evidence_fact, call_edge_facts, call_resolution_fact, call_site_fact};

fn append_call_proof_facts(
    db: &Database,
    values: &mut Vec<Value>,
    row: CallContextRow,
    build_domain_id: &str,
    source_file: &str,
) -> Result<(), DbError> {
    validate_call_context(&row)?;
    values.push(call_site_fact(&row, build_domain_id, source_file));
    values.extend(call_edge_facts(&row));
    values.push(call_resolution_fact(&row));
    if let Some(evidence) = db.callee_evidence_for_site(row.site.id)? {
        if is_returned_callable_evidence(row.site.id, &evidence)? {
            values.push(binding_evidence_fact(
                &row,
                &evidence.kind,
                &evidence.path,
                build_domain_id,
                source_file,
            ));
        }
        if let Some(fact) = async_blocker_fact(&row, &evidence) {
            values.push(fact);
        }
    }
    Ok(())
}

struct CalleeEvidence {
    kind: String,
    path: Vec<String>,
}

fn is_returned_callable_evidence(
    site_id: Uuid,
    evidence: &CalleeEvidence,
) -> Result<bool, DbError> {
    match evidence.kind.as_str() {
        "ReturnedPathCall" | "AwaitedReturnedPathCall" => Ok(true),
        "AsyncClosureBinding" | "AwaitedAsyncClosureBinding" => Ok(false),
        other => Err(DbError::Cozo(format!(
            "unknown call_callee_evidence kind {other:?} for call site {site_id}"
        ))),
    }
}

fn async_blocker_fact(row: &CallContextRow, evidence: &CalleeEvidence) -> Option<Value> {
    if row.status.status != CallStatusKind::Unsupported {
        return None;
    }

    match evidence.kind.as_str() {
        "AsyncClosureBinding" => Some(serde_json::json!({
            "fact_kind": "proof_blocker",
            "schema_version": PROOF_FACT_SCHEMA_VERSION,
            "blocker_id": format!("blocker:async-closure-poll-resume:{}", row.site.id),
            "reason": "dynamic_dispatch_unbounded",
            "status": "blocked",
            "call_site_id": row.site.id.to_string(),
            "detail": format!(
                "{} calls async closure binding {} without awaiting the returned future; traversal remains targetless until async poll/resume proof is modeled",
                row.site.owner_id,
                evidence.path.join("::")
            ),
            "evidence_use": "proof_only"
        })),
        "ReturnedPathCall" => Some(serde_json::json!({
            "fact_kind": "proof_blocker",
            "schema_version": PROOF_FACT_SCHEMA_VERSION,
            "blocker_id": format!("blocker:returned-callable-poll-resume:{}", row.site.id),
            "reason": "dynamic_dispatch_unbounded",
            "status": "blocked",
            "call_site_id": row.site.id.to_string(),
            "detail": format!(
                "{} invokes callable returned by {} without proof that the returned future is polled at this call boundary; traversal remains targetless until async poll/resume proof is modeled",
                row.site.owner_id,
                evidence.path.join("::")
            ),
            "evidence_use": "proof_only"
        })),
        "AwaitedAsyncClosureBinding" | "AwaitedReturnedPathCall" => None,
        _ => None,
    }
}

impl Database {
    fn callee_evidence_for_site(&self, site_id: Uuid) -> Result<Option<CalleeEvidence>, DbError> {
        let mut params = BTreeMap::new();
        params.insert("site_id".to_string(), DataValue::Uuid(UuidWrapper(site_id)));

        let rows = self.run_script(
            r#"?[callee_kind, callee_path] :=
                site_id = $site_id,
                *call_site { id: site_id, call_kind @ 'NOW' },
                *call_callee_evidence {
                    source_id: site_id,
                    source_kind: call_kind,
                    callee_kind,
                    callee_path @ 'NOW'
                }"#,
            params,
            ScriptMutability::Immutable,
        )?;

        match rows.rows.as_slice() {
            [] => Ok(None),
            [row] => Ok(Some(CalleeEvidence {
                kind: to_string(&row[0])?,
                path: to_string_list(&row[1])?,
            })),
            rows => Err(DbError::Cozo(format!(
                "expected at most one call_callee_evidence row for call site {site_id}, found {}",
                rows.len()
            ))),
        }
    }

    pub fn call_proof_facts_for_owner(
        &self,
        owner_id: Uuid,
        build_domain_id: &str,
    ) -> Result<Vec<Value>, DbError> {
        if build_domain_id.is_empty() {
            return Err(DbError::QueryConstruction(
                "call proof projection requires non-empty build_domain_id".to_string(),
            ));
        }
        let source_file = self.source_file_for_owner(owner_id)?;
        let context = self.call_context_for_owner(owner_id)?;
        let mut values = Vec::with_capacity(context.len() * 3);

        for row in context {
            append_call_proof_facts(self, &mut values, row, build_domain_id, &source_file)?;
        }

        Ok(values)
    }

    pub fn project_call_proof_facts_for_owner(
        &self,
        owner_id: Uuid,
        build_domain_id: &str,
    ) -> Result<usize, DbError> {
        let values = self.call_proof_facts_for_owner(owner_id, build_domain_id)?;
        let count = values.len();
        <Self as ProofGraphStore>::upsert_proof_fact_values(self, &values)?;
        Ok(count)
    }

    pub fn call_proof_facts_for_target(
        &self,
        target_id: Uuid,
        build_domain_id: &str,
    ) -> Result<Vec<Value>, DbError> {
        if build_domain_id.is_empty() {
            return Err(DbError::QueryConstruction(
                "call proof projection requires non-empty build_domain_id".to_string(),
            ));
        }

        let context = self.call_context_for_target(target_id)?;
        let mut values = Vec::with_capacity(context.len() * 3);

        for row in context {
            let source_file = self.source_file_for_owner(row.site.owner_id)?;
            append_call_proof_facts(self, &mut values, row, build_domain_id, &source_file)?;
        }

        Ok(values)
    }

    pub fn project_call_proof_facts_for_target(
        &self,
        target_id: Uuid,
        build_domain_id: &str,
    ) -> Result<usize, DbError> {
        let values = self.call_proof_facts_for_target(target_id, build_domain_id)?;
        let count = values.len();
        <Self as ProofGraphStore>::upsert_proof_fact_values(self, &values)?;
        Ok(count)
    }

    pub fn call_proof_facts_for_node(
        &self,
        node_id: Uuid,
        build_domain_id: &str,
    ) -> Result<Vec<Value>, DbError> {
        if build_domain_id.is_empty() {
            return Err(DbError::QueryConstruction(
                "call proof projection requires non-empty build_domain_id".to_string(),
            ));
        }

        let context = self.call_context_for_node(node_id)?;
        let mut values = Vec::with_capacity((context.outgoing.len() + context.incoming.len()) * 3);
        let mut seen_sites = HashSet::new();

        if !context.outgoing.is_empty() {
            let source_file = self.source_file_for_owner(node_id)?;
            for row in context.outgoing {
                seen_sites.insert(row.site.id);
                append_call_proof_facts(self, &mut values, row, build_domain_id, &source_file)?;
            }
        }

        for row in context.incoming {
            if !seen_sites.insert(row.site.id) {
                continue;
            }
            let source_file = self.source_file_for_owner(row.site.owner_id)?;
            append_call_proof_facts(self, &mut values, row, build_domain_id, &source_file)?;
        }

        Ok(values)
    }

    pub fn project_call_proof_facts_for_node(
        &self,
        node_id: Uuid,
        build_domain_id: &str,
    ) -> Result<usize, DbError> {
        let values = self.call_proof_facts_for_node(node_id, build_domain_id)?;
        let count = values.len();
        <Self as ProofGraphStore>::upsert_proof_fact_values(self, &values)?;
        Ok(count)
    }
}
