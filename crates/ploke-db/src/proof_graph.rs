use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, ScriptMutability};
use serde_json::Value;
#[cfg(feature = "call_graph")]
use uuid::Uuid;

use crate::{Database, DbError};
#[cfg(feature = "call_graph")]
use crate::{
    call_graph::{
        CallCallerRow, CallContextRow, CallRelationKind, CallSiteKind, CallStatusKind,
        CallTargetKind, CallTargetRow,
    },
    multi_embedding::db_ext::{ANCESTOR_RULES_NOW, METHOD_NODE_ANCESTOR_RULE},
};

mod invariants;
mod projection;
mod rows;

use invariants::evaluate_proof_invariants;
use projection::ProofFactProjection;
use rows::{ProofFactRow, int_value, string_value};

const PROOF_FACT_SCHEMA_VERSION: &str = "ploke-proof-facts.v1";

/// GraphRAG-visible row from the proof graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofGraphContextRow {
    pub fact_id: String,
    pub kind: String,
    pub call_site_id: Option<String>,
    pub caller_def_id: Option<String>,
    pub callee_def_id: Option<String>,
    pub evidence_use: Option<String>,
    pub blocker_reason: Option<String>,
    pub source_file: Option<String>,
    pub line_start: Option<u32>,
    pub detail: Option<String>,
}

/// Checker traversal row. Blockers are joined by call-site identity and retained.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofCheckerEdgeRow {
    pub call_edge_id: String,
    pub call_site_id: String,
    pub caller_def_id: String,
    pub callee_def_id: Option<String>,
    pub resolution_state: String,
    pub evidence_use: String,
    pub blocker_reason: Option<String>,
}

/// Explicit proof blocker inspection row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofBlockerRow {
    pub blocker_id: String,
    pub reason: String,
    pub status: String,
    pub build_domain_id: Option<String>,
    pub call_site_id: Option<String>,
    pub detail: String,
}

/// Source provenance for one proof call-site.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofSourceProvenanceRow {
    pub call_site_id: String,
    pub source_file: String,
    pub start_byte: u32,
    pub end_byte: u32,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
}

/// Active proof-checker decision for one invariant family.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProofInvariantStatus {
    Pass,
    Fail,
    Blocked,
}

/// Result emitted by the first active proof invariant checker.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofInvariantFinding {
    pub invariant: String,
    pub status: ProofInvariantStatus,
    pub reason: String,
    pub call_site_id: Option<String>,
}

/// Database-backed storage and query surface for proof-useful call/effect facts.
pub trait ProofGraphStore {
    fn ensure_proof_graph_schema(&self) -> Result<(), DbError>;
    fn upsert_proof_fact_values(&self, values: &[Value]) -> Result<(), DbError>;
    fn proof_symbol_lookup(&self, symbol: &str) -> Result<Vec<ProofGraphContextRow>, DbError>;
    fn proof_graphrag_context(&self, query: &str) -> Result<Vec<ProofGraphContextRow>, DbError>;
    fn proof_checker_edges(&self) -> Result<Vec<ProofCheckerEdgeRow>, DbError>;
    fn proof_blockers(&self) -> Result<Vec<ProofBlockerRow>, DbError>;
    fn proof_source_provenance(
        &self,
        call_site_id: &str,
    ) -> Result<Option<ProofSourceProvenanceRow>, DbError>;
    fn proof_invariant_findings(&self) -> Result<Vec<ProofInvariantFinding>, DbError>;
}

impl ProofGraphStore for Database {
    fn ensure_proof_graph_schema(&self) -> Result<(), DbError> {
        let create = r#"
:create proof_fact {
    fact_id: String
    =>
    kind: String,
    schema_version: String,
    json: Json,
    evidence_use: String?,
    build_domain_id: String?,
    call_site_id: String?,
    call_edge_id: String?,
    caller_def_id: String?,
    callee_def_id: String?,
    resolution_state: String?,
    source_file: String?,
    start_byte: Int?,
    end_byte: Int?,
    line_start: Int?,
    line_end: Int?,
    effect_class: String?,
    blocker_reason: String?,
    status: String?,
    detail: String?
}
"#;
        if let Err(error) = self.run_script(create, BTreeMap::new(), ScriptMutability::Mutable) {
            let message = error.to_string();
            if !is_idempotent_schema_error(&message) {
                return Err(DbError::Cozo(message));
            }
        }
        Ok(())
    }

    fn upsert_proof_fact_values(&self, values: &[Value]) -> Result<(), DbError> {
        self.ensure_proof_graph_schema()?;
        let projections = values
            .iter()
            .map(ProofFactProjection::from_value)
            .collect::<Result<Vec<_>, _>>()?;
        for projection in projections {
            self.upsert_projection(projection)?;
        }
        Ok(())
    }

    fn proof_symbol_lookup(&self, symbol: &str) -> Result<Vec<ProofGraphContextRow>, DbError> {
        let symbol = symbol.to_ascii_lowercase();
        let rows = self.fetch_proof_rows()?;
        let linked_call_sites = rows
            .iter()
            .filter(|row| row.matches_query(&symbol))
            .filter_map(|row| row.call_site_id.clone())
            .collect::<BTreeSet<_>>();

        Ok(rows
            .into_iter()
            .filter(|row| {
                row.matches_query(&symbol)
                    || row
                        .call_site_id
                        .as_ref()
                        .is_some_and(|id| linked_call_sites.contains(id))
            })
            .map(ProofGraphContextRow::from)
            .collect())
    }

    fn proof_graphrag_context(&self, query: &str) -> Result<Vec<ProofGraphContextRow>, DbError> {
        let query = query.to_ascii_lowercase();
        let rows = self.fetch_proof_rows()?;
        let linked_call_sites = rows
            .iter()
            .filter(|row| row.matches_query(&query))
            .filter_map(|row| row.call_site_id.clone())
            .collect::<BTreeSet<_>>();

        Ok(rows
            .into_iter()
            .filter(|row| {
                row.matches_query(&query)
                    || row
                        .call_site_id
                        .as_ref()
                        .is_some_and(|id| linked_call_sites.contains(id))
            })
            .map(ProofGraphContextRow::from)
            .collect())
    }

    fn proof_checker_edges(&self) -> Result<Vec<ProofCheckerEdgeRow>, DbError> {
        let rows = self.fetch_proof_rows()?;
        let blockers_by_site = rows
            .iter()
            .filter(|row| row.kind == "proof_blocker")
            .filter_map(|row| Some((row.call_site_id.clone()?, row.blocker_reason.clone()?)))
            .fold(
                BTreeMap::<String, Vec<String>>::new(),
                |mut acc, (site, reason)| {
                    acc.entry(site).or_default().push(reason);
                    acc
                },
            );

        Ok(rows
            .into_iter()
            .filter(|row| row.kind == "call_edge")
            .flat_map(|row| {
                let Some(call_site_id) = row.call_site_id.clone() else {
                    return Vec::new();
                };
                let blocker_reasons = blockers_by_site
                    .get(&call_site_id)
                    .cloned()
                    .unwrap_or_else(|| vec![String::new()]);
                blocker_reasons
                    .into_iter()
                    .map(|reason| ProofCheckerEdgeRow {
                        call_edge_id: row.call_edge_id.clone().unwrap_or_default(),
                        call_site_id: call_site_id.clone(),
                        caller_def_id: row.caller_def_id.clone().unwrap_or_default(),
                        callee_def_id: row.callee_def_id.clone(),
                        resolution_state: row.resolution_state.clone().unwrap_or_default(),
                        evidence_use: row
                            .evidence_use
                            .clone()
                            .unwrap_or_else(|| "proof_only".to_string()),
                        blocker_reason: (!reason.is_empty()).then_some(reason),
                    })
                    .collect::<Vec<_>>()
            })
            .filter(|row| !row.call_edge_id.is_empty() && !row.caller_def_id.is_empty())
            .collect())
    }

    fn proof_blockers(&self) -> Result<Vec<ProofBlockerRow>, DbError> {
        Ok(self
            .fetch_proof_rows()?
            .into_iter()
            .filter(|row| row.kind == "proof_blocker")
            .filter_map(|row| {
                Some(ProofBlockerRow {
                    blocker_id: row.fact_id,
                    reason: row.blocker_reason?,
                    status: row.status?,
                    build_domain_id: row.build_domain_id,
                    call_site_id: row.call_site_id,
                    detail: row.detail?,
                })
            })
            .collect())
    }

    fn proof_source_provenance(
        &self,
        call_site_id: &str,
    ) -> Result<Option<ProofSourceProvenanceRow>, DbError> {
        Ok(self
            .fetch_proof_rows()?
            .into_iter()
            .find(|row| {
                row.kind == "call_site" && row.call_site_id.as_deref() == Some(call_site_id)
            })
            .and_then(|row| {
                Some(ProofSourceProvenanceRow {
                    call_site_id: row.call_site_id?,
                    source_file: row.source_file?,
                    start_byte: row.start_byte?,
                    end_byte: row.end_byte?,
                    line_start: row.line_start,
                    line_end: row.line_end,
                })
            }))
    }

    fn proof_invariant_findings(&self) -> Result<Vec<ProofInvariantFinding>, DbError> {
        let rows = self.fetch_proof_rows()?;
        Ok(evaluate_proof_invariants(&rows))
    }
}

impl Database {
    #[cfg(feature = "call_graph")]
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

    #[cfg(feature = "call_graph")]
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

    #[cfg(feature = "call_graph")]
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

    #[cfg(feature = "call_graph")]
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

    #[cfg(feature = "call_graph")]
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

    fn upsert_projection(&self, projection: ProofFactProjection) -> Result<(), DbError> {
        let mut params = BTreeMap::new();
        params.insert("fact_id".into(), string_value(Some(projection.fact_id)));
        params.insert("kind".into(), string_value(Some(projection.kind)));
        params.insert(
            "schema_version".into(),
            string_value(Some(projection.schema_version)),
        );
        params.insert(
            "json".into(),
            DataValue::Json(cozo::JsonData(projection.json)),
        );
        params.insert("evidence_use".into(), string_value(projection.evidence_use));
        params.insert(
            "build_domain_id".into(),
            string_value(projection.build_domain_id),
        );
        params.insert("call_site_id".into(), string_value(projection.call_site_id));
        params.insert("call_edge_id".into(), string_value(projection.call_edge_id));
        params.insert(
            "caller_def_id".into(),
            string_value(projection.caller_def_id),
        );
        params.insert(
            "callee_def_id".into(),
            string_value(projection.callee_def_id),
        );
        params.insert(
            "resolution_state".into(),
            string_value(projection.resolution_state),
        );
        params.insert("source_file".into(), string_value(projection.source_file));
        params.insert("start_byte".into(), int_value(projection.start_byte));
        params.insert("end_byte".into(), int_value(projection.end_byte));
        params.insert("line_start".into(), int_value(projection.line_start));
        params.insert("line_end".into(), int_value(projection.line_end));
        params.insert("effect_class".into(), string_value(projection.effect_class));
        params.insert(
            "blocker_reason".into(),
            string_value(projection.blocker_reason),
        );
        params.insert("status".into(), string_value(projection.status));
        params.insert("detail".into(), string_value(projection.detail));

        let script = r#"
{
    ?[fact_id, kind, schema_version, json, evidence_use, build_domain_id, call_site_id, call_edge_id, caller_def_id, callee_def_id, resolution_state, source_file, start_byte, end_byte, line_start, line_end, effect_class, blocker_reason, status, detail] :=
        fact_id = $fact_id,
        kind = $kind,
        schema_version = $schema_version,
        json = $json,
        evidence_use = $evidence_use,
        build_domain_id = $build_domain_id,
        call_site_id = $call_site_id,
        call_edge_id = $call_edge_id,
        caller_def_id = $caller_def_id,
        callee_def_id = $callee_def_id,
        resolution_state = $resolution_state,
        source_file = $source_file,
        start_byte = $start_byte,
        end_byte = $end_byte,
        line_start = $line_start,
        line_end = $line_end,
        effect_class = $effect_class,
        blocker_reason = $blocker_reason,
        status = $status,
        detail = $detail
    :put proof_fact { fact_id => kind, schema_version, json, evidence_use, build_domain_id, call_site_id, call_edge_id, caller_def_id, callee_def_id, resolution_state, source_file, start_byte, end_byte, line_start, line_end, effect_class, blocker_reason, status, detail }
}
"#;
        self.run_script(script, params, ScriptMutability::Mutable)
            .map(|_| ())
            .map_err(|error| DbError::Cozo(error.to_string()))
    }

    fn fetch_proof_rows(&self) -> Result<Vec<ProofFactRow>, DbError> {
        self.ensure_proof_graph_schema()?;
        let script = r#"
?[fact_id, kind, schema_version, evidence_use, build_domain_id, call_site_id, call_edge_id, caller_def_id, callee_def_id, resolution_state, source_file, start_byte, end_byte, line_start, line_end, effect_class, blocker_reason, status, detail] :=
    *proof_fact{ fact_id, kind, schema_version, evidence_use, build_domain_id, call_site_id, call_edge_id, caller_def_id, callee_def_id, resolution_state, source_file, start_byte, end_byte, line_start, line_end, effect_class, blocker_reason, status, detail }
"#;
        let rows = self
            .run_script(script, BTreeMap::new(), ScriptMutability::Immutable)
            .map_err(|error| DbError::Cozo(error.to_string()))?;
        rows.rows
            .iter()
            .map(|row| ProofFactRow::from_data_values(row))
            .collect()
    }
}

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
fn is_ambiguous_dynamic_candidate_row(row: &CallContextRow) -> bool {
    row.site.kind == CallSiteKind::Dynamic
        && row.targets.iter().all(|target| {
            target.relation == CallRelationKind::DynamicFunction
                && target.source_kind == CallSiteKind::Dynamic
                && target.target_kind == CallTargetKind::Function
        })
}

#[cfg(feature = "call_graph")]
fn caller_context_row(row: CallCallerRow) -> CallContextRow {
    CallContextRow {
        site: row.site,
        status: row.status,
        targets: vec![row.target],
    }
}

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
fn call_edge_facts(row: &CallContextRow) -> Vec<Value> {
    if row.status.status != CallStatusKind::Resolved {
        return Vec::new();
    }

    row.targets
        .iter()
        .map(|target| call_edge_fact(row, target))
        .collect()
}

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
fn resolved_target(row: &CallContextRow) -> Option<&CallTargetRow> {
    (row.status.status == CallStatusKind::Resolved).then(|| &row.targets[0])
}

#[cfg(feature = "call_graph")]
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

#[cfg(feature = "call_graph")]
fn resolution_state(status: CallStatusKind) -> &'static str {
    match status {
        CallStatusKind::Resolved => "resolved",
        CallStatusKind::Unresolved => "unresolved",
        CallStatusKind::Ambiguous => "ambiguous",
        CallStatusKind::External => "blocked",
        CallStatusKind::Unsupported => "blocked",
    }
}

#[cfg(feature = "call_graph")]
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

fn is_idempotent_schema_error(message: &str) -> bool {
    message.contains("exists")
        || message.contains("duplicate")
        || message.contains("Duplicate")
        || message.contains("already")
        || message.contains("conflicts with an existing one")
        || message.to_ascii_lowercase().contains("conflict")
}
