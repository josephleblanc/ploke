use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, ScriptMutability};
use serde_json::Value;

use crate::{Database, DbError};

#[cfg(feature = "call_graph")]
mod call_projection;
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
    pub build_domain_id: Option<String>,
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
    fn proof_domain_context(
        &self,
        build_domain_id: &str,
    ) -> Result<Vec<ProofGraphContextRow>, DbError>;
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
        Ok(linked_context_rows(rows, |row| row.matches_query(&symbol)))
    }

    fn proof_graphrag_context(&self, query: &str) -> Result<Vec<ProofGraphContextRow>, DbError> {
        let query = query.to_ascii_lowercase();
        let rows = self.fetch_proof_rows()?;
        Ok(linked_context_rows(rows, |row| row.matches_query(&query)))
    }

    fn proof_domain_context(
        &self,
        build_domain_id: &str,
    ) -> Result<Vec<ProofGraphContextRow>, DbError> {
        if build_domain_id.is_empty() {
            return Err(DbError::QueryConstruction(
                "proof domain context requires non-empty build_domain_id".to_string(),
            ));
        }

        let rows = self.fetch_proof_rows()?;
        Ok(linked_context_rows(rows, |row| {
            row.build_domain_id.as_deref() == Some(build_domain_id)
        }))
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

fn is_idempotent_schema_error(message: &str) -> bool {
    message.contains("exists")
        || message.contains("duplicate")
        || message.contains("Duplicate")
        || message.contains("already")
        || message.contains("conflicts with an existing one")
        || message.to_ascii_lowercase().contains("conflict")
}

fn linked_context_rows(
    rows: Vec<ProofFactRow>,
    is_seed: impl Fn(&ProofFactRow) -> bool,
) -> Vec<ProofGraphContextRow> {
    let linked_call_sites = rows
        .iter()
        .filter(|row| is_seed(row))
        .filter_map(|row| row.call_site_id.clone())
        .collect::<BTreeSet<_>>();

    rows.into_iter()
        .filter(|row| {
            is_seed(row)
                || row
                    .call_site_id
                    .as_ref()
                    .is_some_and(|id| linked_call_sites.contains(id))
        })
        .map(ProofGraphContextRow::from)
        .collect()
}
