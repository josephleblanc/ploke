use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, ScriptMutability};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    Database, DbError,
    call_graph::{
        CallCallerRow, CallContextRow, CallRelationKind, CallSiteKind, CallStatusKind,
        CallTargetKind, CallTargetRow,
    },
    multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE},
};

use super::{PROOF_FACT_SCHEMA_VERSION, ProofGraphStore};

impl Database {
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
            validate_call_context(&row)?;
            values.push(call_site_fact(&row, build_domain_id, &source_file));
            values.extend(call_edge_facts(&row));
            values.push(call_resolution_fact(&row));
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

        let callers = self.callers_for_target(target_id)?;
        let mut values = Vec::with_capacity(callers.len() * 3);

        for caller in callers {
            let source_file = self.source_file_for_owner(caller.site.owner_id)?;
            let row = caller_context_row(caller);
            validate_call_context(&row)?;
            values.push(call_site_fact(&row, build_domain_id, &source_file));
            values.extend(call_edge_facts(&row));
            values.push(call_resolution_fact(&row));
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

    fn source_file_for_owner(&self, owner_id: Uuid) -> Result<String, DbError> {
        let mut params = BTreeMap::new();
        params.insert(
            "owner_id".to_string(),
            DataValue::Uuid(cozo::UuidWrapper(owner_id)),
        );
        let script = format!(
            r#"
ancestor[desc, desc] := *module{{ id: desc @ 'NOW' }}
{ANCESTOR_RULES_NOW}
{METHOD_NODE_ANCESTOR_RULE}

module_has_file[mid] := *file_mod{{ owner_id: mid @ 'NOW' }}
file_owner_for_module[mod_id, file_id] := module_has_file[mod_id], file_id = mod_id
file_owner_for_module[mod_id, file_id] := ancestor[mod_id, parent], module_has_file[parent], file_id = parent

?[file_path] :=
    owner_id = $owner_id,
    ancestor[owner_id, mod_id],
    *module{{ id: mod_id @ 'NOW' }},
    file_owner_for_module[mod_id, file_id],
    *file_mod{{ owner_id: file_id, file_path @ 'NOW' }}
"#
        );
        let rows = self
            .run_script(&script, params, ScriptMutability::Immutable)
            .map_err(|error| DbError::Cozo(error.to_string()))?;
        let paths = rows
            .rows
            .iter()
            .map(|row| {
                row.first()
                    .and_then(DataValue::get_str)
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| {
                        DbError::Cozo(format!(
                            "source-file query returned non-string row for owner {owner_id}: {row:?}"
                        ))
                    })
            })
            .collect::<Result<BTreeSet<_>, _>>()?;

        match paths.len() {
            0 => Err(DbError::Cozo(format!(
                "missing source file for call graph owner {owner_id}"
            ))),
            1 => Ok(paths.into_iter().next().expect("one path")),
            _ => Err(DbError::Cozo(format!(
                "ambiguous source files for call graph owner {owner_id}: {paths:?}"
            ))),
        }
    }
}

fn validate_call_context(row: &CallContextRow) -> Result<(), DbError> {
    match row.status.status {
        CallStatusKind::Resolved if row.targets.len() != 1 => Err(DbError::Cozo(format!(
            "resolved call site {} expected exactly one target, found {}",
            row.site.id,
            row.targets.len()
        ))),
        CallStatusKind::Unresolved | CallStatusKind::External | CallStatusKind::Unsupported
            if !row.targets.is_empty() =>
        {
            Err(DbError::Cozo(format!(
                "non-resolved call site {} has local call_relation targets: {:?}",
                row.site.id, row.targets
            )))
        }
        CallStatusKind::Ambiguous
            if !row.targets.is_empty() && !is_ambiguous_dynamic_candidate_row(row) =>
        {
            Err(DbError::Cozo(format!(
                "non-resolved call site {} has local call_relation targets: {:?}",
                row.site.id, row.targets
            )))
        }
        _ => Ok(()),
    }
}

fn is_ambiguous_dynamic_candidate_row(row: &CallContextRow) -> bool {
    row.site.kind == CallSiteKind::Dynamic
        && row.targets.iter().all(|target| {
            target.relation == CallRelationKind::DynamicFunction
                && target.source_kind == CallSiteKind::Dynamic
                && target.target_kind == CallTargetKind::Function
        })
}

fn caller_context_row(row: CallCallerRow) -> CallContextRow {
    CallContextRow {
        site: row.site,
        status: row.status,
        targets: vec![row.target],
    }
}

fn call_site_fact(row: &CallContextRow, build_domain_id: &str, source_file: &str) -> Value {
    serde_json::json!({
        "fact_kind": "call_site",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_site_id": row.site.id.to_string(),
        "build_domain_id": build_domain_id,
        "caller_def_id": row.site.owner_id.to_string(),
        "source_span": {
            "file": source_file,
            "start_byte": row.site.span.0,
            "end_byte": row.site.span.1
        },
        "evidence_use": "proof_and_navigation"
    })
}

fn call_edge_facts(row: &CallContextRow) -> Vec<Value> {
    if row.status.status != CallStatusKind::Resolved {
        return Vec::new();
    }

    row.targets
        .iter()
        .map(|target| call_edge_fact(row, target))
        .collect()
}

fn call_edge_fact(row: &CallContextRow, target: &CallTargetRow) -> Value {
    serde_json::json!({
        "fact_kind": "call_edge",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_edge_id": format!("edge:{}:{}", row.site.id, target.target_id),
        "call_site_id": row.site.id.to_string(),
        "caller_def_id": row.site.owner_id.to_string(),
        "callee_def_id": target.target_id.to_string(),
        "resolution_state": resolution_state(row.status.status),
        "evidence_use": "proof_and_navigation"
    })
}

fn call_resolution_fact(row: &CallContextRow) -> Value {
    let mut value = serde_json::json!({
        "fact_kind": "call_resolution",
        "schema_version": PROOF_FACT_SCHEMA_VERSION,
        "call_site_id": row.site.id.to_string(),
        "resolution_state": resolution_state(row.status.status)
    });

    if let Some(target) = resolved_target(row) {
        value["resolved_def_id"] = serde_json::json!(target.target_id.to_string());
    }
    let candidates = candidate_targets(row);
    if !candidates.is_empty() {
        value["candidate_def_ids"] = serde_json::json!(candidates);
    }
    if let Some(reason) = blocking_reason(row) {
        value["blocking_reason"] = serde_json::json!(reason);
    }

    value
}

fn resolved_target(row: &CallContextRow) -> Option<&CallTargetRow> {
    (row.status.status == CallStatusKind::Resolved).then(|| &row.targets[0])
}

fn candidate_targets(row: &CallContextRow) -> Vec<String> {
    match row.status.status {
        CallStatusKind::Ambiguous => row
            .targets
            .iter()
            .map(|target| target.target_id.to_string())
            .collect(),
        _ => Vec::new(),
    }
}

fn resolution_state(status: CallStatusKind) -> &'static str {
    match status {
        CallStatusKind::Resolved => "resolved",
        CallStatusKind::Unresolved => "unresolved",
        CallStatusKind::Ambiguous => "ambiguous",
        CallStatusKind::External => "blocked",
        CallStatusKind::Unsupported => "blocked",
    }
}

fn blocking_reason(row: &CallContextRow) -> Option<&'static str> {
    match row.status.status {
        CallStatusKind::Resolved => None,
        CallStatusKind::External => Some("external_dependency_summary_missing"),
        CallStatusKind::Unsupported => Some(match row.site.kind {
            CallSiteKind::Dynamic => "dynamic_dispatch_unbounded",
            CallSiteKind::Macro => "macro_expansion_not_available",
            CallSiteKind::Path | CallSiteKind::Method => "type_resolution_missing",
        }),
        CallStatusKind::Unresolved | CallStatusKind::Ambiguous => Some("type_resolution_missing"),
    }
}
