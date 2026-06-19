use std::collections::{BTreeMap, BTreeSet};

use cozo::{DataValue, ScriptMutability};
use serde_json::Value;

use crate::{Database, DbError};

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

fn evaluate_proof_invariants(rows: &[ProofFactRow]) -> Vec<ProofInvariantFinding> {
    let mut findings = evaluate_detached_process_invariants(rows);
    findings.extend(evaluate_crown_ruling_invariants(rows));
    findings
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
struct ProofScope {
    call_site_id: String,
    build_domain_id: Option<String>,
}

fn evaluate_detached_process_invariants(rows: &[ProofFactRow]) -> Vec<ProofInvariantFinding> {
    let scopes = process_scopes(rows);
    let unscoped_process_count = unscoped_process_effect_count(rows);
    let mut findings = Vec::new();
    if unscoped_process_count > 0 {
        findings.push(finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Blocked,
            format!(
                "detached process proof evidence lacks call_site_id for {unscoped_process_count} effect seed(s)"
            ),
            None,
        ));
    }
    if scopes.is_empty() {
        let blocker_reasons = active_proof_blocker_reasons(rows);
        if !blocker_reasons.is_empty() {
            findings.push(finding(
                "detached_process_successor_handoff",
                ProofInvariantStatus::Blocked,
                format!(
                    "proof blockers prevent detached-process proof: {}",
                    blocker_reasons.join(", ")
                ),
                None,
            ));
        } else if findings.is_empty() {
            findings.push(finding(
                "detached_process_successor_handoff",
                ProofInvariantStatus::Pass,
                "no proof-only detached process effects recorded".to_string(),
                None,
            ));
        }
        return findings;
    }

    findings.extend(
        scopes
            .iter()
            .map(|scope| evaluate_detached_process_scope(rows, scope)),
    );
    findings
}

fn evaluate_detached_process_scope(
    rows: &[ProofFactRow],
    scope: &ProofScope,
) -> ProofInvariantFinding {
    let blocker_reasons = proof_blocker_reasons(rows, scope);
    if !blocker_reasons.is_empty() {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Blocked,
            format!(
                "proof blockers prevent detached-process proof: {}",
                blocker_reasons.join(", ")
            ),
            Some(scope.call_site_id.clone()),
        );
    }
    let authority_blockers = scoped_authority_blockers(rows, scope);
    if !authority_blockers.is_empty() {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Blocked,
            format!(
                "authority evidence is blocked: {}",
                authority_blockers.join(", ")
            ),
            Some(scope.call_site_id.clone()),
        );
    }

    let successor_count = scoped_authority_count(rows, scope, "successor");
    let parent_count = scoped_authority_count(rows, scope, "parent_lineage");
    let predecessor_count = scoped_authority_count(rows, scope, "predecessor_retired");
    let crown_count = scoped_authority_count(rows, scope, "crown_ruling");

    if successor_count == 0 {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Fail,
            "detached process create lacks admitted successor handoff".to_string(),
            Some(scope.call_site_id.clone()),
        );
    }
    if successor_count > 1 {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Fail,
            format!("expected exactly one detached successor Parent, found {successor_count}"),
            Some(scope.call_site_id.clone()),
        );
    }
    if parent_count != 1 {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Fail,
            format!("expected exactly one admitted Parent lineage boundary, found {parent_count}"),
            Some(scope.call_site_id.clone()),
        );
    }
    if predecessor_count == 0 {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Fail,
            "successor admission lacks predecessor retirement or lock evidence".to_string(),
            Some(scope.call_site_id.clone()),
        );
    }
    if crown_count != 1 {
        return finding(
            "detached_process_successor_handoff",
            ProofInvariantStatus::Fail,
            format!("successor handoff requires exactly one Crown<Ruling>, found {crown_count}"),
            Some(scope.call_site_id.clone()),
        );
    }

    finding(
        "detached_process_successor_handoff",
        ProofInvariantStatus::Pass,
        "detached process is covered by exactly one admitted successor Parent with predecessor retired and one Crown<Ruling>".to_string(),
        Some(scope.call_site_id.clone()),
    )
}

fn evaluate_crown_ruling_invariants(rows: &[ProofFactRow]) -> Vec<ProofInvariantFinding> {
    let lineages = lineage_keys(rows);
    if lineages.is_empty() {
        return vec![finding(
            "crown_ruling_lineage_uniqueness",
            ProofInvariantStatus::Pass,
            "at most one admitted Crown<Ruling> authority is recorded for this lineage".to_string(),
            None,
        )];
    }

    lineages
        .iter()
        .map(|build_domain_id| evaluate_crown_ruling_lineage(rows, build_domain_id.as_deref()))
        .collect()
}

fn evaluate_crown_ruling_lineage(
    rows: &[ProofFactRow],
    build_domain_id: Option<&str>,
) -> ProofInvariantFinding {
    let authority_blockers = lineage_authority_blockers(rows, build_domain_id);
    let call_site_id = first_process_site(rows, build_domain_id);
    if !authority_blockers.is_empty() {
        return finding(
            "crown_ruling_lineage_uniqueness",
            ProofInvariantStatus::Blocked,
            format!(
                "authority evidence is blocked: {}",
                authority_blockers.join(", ")
            ),
            call_site_id,
        );
    }

    let crown_count = lineage_authority_count(rows, build_domain_id, "crown_ruling");
    if crown_count > 1 {
        return finding(
            "crown_ruling_lineage_uniqueness",
            ProofInvariantStatus::Fail,
            format!("multiple Crown<Ruling> authorities admitted in one lineage: {crown_count}"),
            call_site_id,
        );
    }
    if crown_count == 0 && has_process_effect(rows, build_domain_id) {
        return finding(
            "crown_ruling_lineage_uniqueness",
            ProofInvariantStatus::Blocked,
            "ambiguous lineage: no admitted Crown<Ruling> authority evidence".to_string(),
            call_site_id,
        );
    }

    finding(
        "crown_ruling_lineage_uniqueness",
        ProofInvariantStatus::Pass,
        "at most one admitted Crown<Ruling> authority is recorded for this lineage".to_string(),
        call_site_id,
    )
}

fn process_scopes(rows: &[ProofFactRow]) -> Vec<ProofScope> {
    let site_domains = call_site_domains(rows);
    let mut scopes = BTreeMap::<String, Option<String>>::new();
    for row in rows.iter().filter(|row| {
        row.kind == "effect_seed"
            && is_proof_evidence(row)
            && row
                .effect_class
                .as_deref()
                .is_some_and(is_process_effect_class)
    }) {
        let Some(call_site_id) = row.call_site_id.clone() else {
            continue;
        };
        let build_domain_id = row
            .build_domain_id
            .clone()
            .or_else(|| site_domains.get(&call_site_id).cloned());
        let entry = scopes
            .entry(call_site_id)
            .or_insert_with(|| build_domain_id.clone());
        if entry.is_none() && build_domain_id.is_some() {
            *entry = build_domain_id;
        }
    }
    scopes
        .into_iter()
        .map(|(call_site_id, build_domain_id)| ProofScope {
            call_site_id,
            build_domain_id,
        })
        .collect()
}

fn unscoped_process_effect_count(rows: &[ProofFactRow]) -> usize {
    rows.iter()
        .filter(|row| row.kind == "effect_seed")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| {
            row.effect_class
                .as_deref()
                .is_some_and(is_process_effect_class)
        })
        .filter(|row| row.call_site_id.is_none())
        .count()
}

fn call_site_domains(rows: &[ProofFactRow]) -> BTreeMap<String, String> {
    rows.iter()
        .filter(|row| row.kind == "call_site")
        .filter_map(|row| Some((row.call_site_id.clone()?, row.build_domain_id.clone()?)))
        .collect()
}

fn lineage_keys(rows: &[ProofFactRow]) -> BTreeSet<Option<String>> {
    let mut keys = BTreeSet::new();
    keys.extend(
        process_scopes(rows)
            .into_iter()
            .map(|scope| scope.build_domain_id),
    );
    keys.extend(
        rows.iter()
            .filter(|row| row.kind == "authority")
            .filter(|row| is_proof_evidence(row))
            .map(|row| row.build_domain_id.clone()),
    );
    keys
}

fn proof_blocker_reasons(rows: &[ProofFactRow], scope: &ProofScope) -> Vec<String> {
    rows.iter()
        .filter(|row| row.kind == "proof_blocker")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| is_active_blocker_status(row))
        .filter(|row| blocker_matches_scope(row, scope))
        .map(blocker_label)
        .collect()
}

fn active_proof_blocker_reasons(rows: &[ProofFactRow]) -> Vec<String> {
    rows.iter()
        .filter(|row| row.kind == "proof_blocker")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| is_active_blocker_status(row))
        .filter(|row| no_process_scope_blocker_can_affect_detached_process(row, rows))
        .map(blocker_label)
        .collect()
}

fn no_process_scope_blocker_can_affect_detached_process(
    blocker: &ProofFactRow,
    rows: &[ProofFactRow],
) -> bool {
    let Some(call_site_id) = blocker.call_site_id.as_deref() else {
        return true;
    };

    !rows
        .iter()
        .any(|row| row.kind == "call_site" && row.call_site_id.as_deref() == Some(call_site_id))
}

fn is_active_blocker_status(row: &ProofFactRow) -> bool {
    matches!(row.status.as_deref(), Some("blocked" | "rejected"))
}

fn scoped_authority_blockers(rows: &[ProofFactRow], scope: &ProofScope) -> Vec<String> {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| authority_matches_scope(row, scope))
        .filter(|row| matches!(row.status.as_deref(), Some("blocked" | "rejected")))
        .map(authority_label)
        .collect()
}

fn lineage_authority_blockers(rows: &[ProofFactRow], build_domain_id: Option<&str>) -> Vec<String> {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| lineage_matches(row, build_domain_id))
        .filter(|row| matches!(row.status.as_deref(), Some("blocked" | "rejected")))
        .map(authority_label)
        .collect()
}

fn authority_label(row: &ProofFactRow) -> String {
    row.effect_class
        .clone()
        .or_else(|| row.detail.clone())
        .unwrap_or_else(|| row.fact_id.clone())
}

fn blocker_label(row: &ProofFactRow) -> String {
    row.blocker_reason
        .clone()
        .or_else(|| row.detail.clone())
        .unwrap_or_else(|| row.fact_id.clone())
}

fn has_process_effect(rows: &[ProofFactRow], build_domain_id: Option<&str>) -> bool {
    process_scopes(rows)
        .iter()
        .any(|scope| scope.build_domain_id.as_deref() == build_domain_id)
}

fn first_process_site(rows: &[ProofFactRow], build_domain_id: Option<&str>) -> Option<String> {
    process_scopes(rows)
        .into_iter()
        .find(|scope| scope.build_domain_id.as_deref() == build_domain_id)
        .map(|scope| scope.call_site_id)
}

fn scoped_authority_count(rows: &[ProofFactRow], scope: &ProofScope, term: &str) -> usize {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| authority_matches_scope(row, scope))
        .filter(|row| row.effect_class.as_deref() == Some(term))
        .filter(|row| row.status.as_deref() == Some("admitted"))
        .count()
}

fn lineage_authority_count(
    rows: &[ProofFactRow],
    build_domain_id: Option<&str>,
    term: &str,
) -> usize {
    rows.iter()
        .filter(|row| row.kind == "authority")
        .filter(|row| is_proof_evidence(row))
        .filter(|row| lineage_matches(row, build_domain_id))
        .filter(|row| row.effect_class.as_deref() == Some(term))
        .filter(|row| row.status.as_deref() == Some("admitted"))
        .count()
}

fn authority_matches_scope(row: &ProofFactRow, scope: &ProofScope) -> bool {
    row.call_site_id.as_deref() == Some(scope.call_site_id.as_str())
        && row.build_domain_id.as_deref() == scope.build_domain_id.as_deref()
}

fn blocker_matches_scope(row: &ProofFactRow, scope: &ProofScope) -> bool {
    if row.call_site_id.as_deref().is_some() {
        return row.call_site_id.as_deref() == Some(scope.call_site_id.as_str());
    }
    if row.build_domain_id.is_none() {
        return true;
    }
    row.build_domain_id.as_deref().is_some()
        && row.build_domain_id.as_deref() == scope.build_domain_id.as_deref()
}

fn lineage_matches(row: &ProofFactRow, build_domain_id: Option<&str>) -> bool {
    row.build_domain_id.as_deref() == build_domain_id
}

fn is_proof_evidence(row: &ProofFactRow) -> bool {
    matches!(
        row.evidence_use.as_deref(),
        Some("proof_only" | "proof_and_navigation")
    )
}

fn is_process_effect_class(effect_class: &str) -> bool {
    matches!(
        effect_class,
        "operating_system_process_create" | "operating_system_process_replace"
    )
}

fn finding(
    invariant: &str,
    status: ProofInvariantStatus,
    reason: String,
    call_site_id: Option<String>,
) -> ProofInvariantFinding {
    ProofInvariantFinding {
        invariant: invariant.to_string(),
        status,
        reason,
        call_site_id,
    }
}

#[derive(Debug, Clone)]
struct ProofFactProjection {
    fact_id: String,
    kind: String,
    schema_version: String,
    json: Value,
    evidence_use: Option<String>,
    build_domain_id: Option<String>,
    call_site_id: Option<String>,
    call_edge_id: Option<String>,
    caller_def_id: Option<String>,
    callee_def_id: Option<String>,
    resolution_state: Option<String>,
    source_file: Option<String>,
    start_byte: Option<u32>,
    end_byte: Option<u32>,
    line_start: Option<u32>,
    line_end: Option<u32>,
    effect_class: Option<String>,
    blocker_reason: Option<String>,
    status: Option<String>,
    detail: Option<String>,
}

impl ProofFactProjection {
    fn from_value(value: &Value) -> Result<Self, DbError> {
        let kind = required_json_string(value, "fact_kind")?.to_string();
        let schema_version = required_json_string(value, "schema_version")?.to_string();
        if schema_version != PROOF_FACT_SCHEMA_VERSION {
            return Err(DbError::QueryConstruction(format!(
                "proof fact schema_version {schema_version} does not match {PROOF_FACT_SCHEMA_VERSION}"
            )));
        }
        let fact_id = fact_id(value, &kind)?;
        validate_required_fields(value, &kind)?;
        validate_enum_fields(value, &kind)?;
        let mut projection = Self::base(fact_id, kind.clone(), schema_version, value.clone());
        projection.evidence_use =
            json_string(value, "evidence_use").or(Some("proof_only".to_string()));
        projection.build_domain_id = json_string(value, "build_domain_id");
        projection.call_site_id = json_string(value, "call_site_id");
        projection.call_edge_id = json_string(value, "call_edge_id");
        projection.caller_def_id = json_string(value, "caller_def_id");
        projection.callee_def_id = json_string(value, "callee_def_id");
        projection.resolution_state = json_string(value, "resolution_state");
        projection.effect_class =
            json_string(value, "effect_class").or_else(|| json_string(value, "authority_term"));
        projection.blocker_reason =
            json_string(value, "reason").or_else(|| json_string(value, "blocking_reason"));
        projection.status =
            json_string(value, "status").or_else(|| json_string(value, "expansion_state"));
        projection.detail = json_string(value, "detail")
            .or_else(|| json_string(value, "confidence"))
            .or_else(|| json_string(value, "target_name"));
        if let Some(span) = value.get("source_span") {
            projection.source_file = json_string(span, "file");
            projection.start_byte = json_u32(span, "start_byte")?;
            projection.end_byte = json_u32(span, "end_byte")?;
            projection.line_start = json_u32(span, "line_start")?;
            projection.line_end = json_u32(span, "line_end")?;
        }
        Ok(projection)
    }

    fn base(fact_id: String, kind: String, schema_version: String, json: Value) -> Self {
        Self {
            fact_id,
            kind,
            schema_version,
            json,
            evidence_use: None,
            build_domain_id: None,
            call_site_id: None,
            call_edge_id: None,
            caller_def_id: None,
            callee_def_id: None,
            resolution_state: None,
            source_file: None,
            start_byte: None,
            end_byte: None,
            line_start: None,
            line_end: None,
            effect_class: None,
            blocker_reason: None,
            status: None,
            detail: None,
        }
    }
}

fn fact_id(value: &Value, kind: &str) -> Result<String, DbError> {
    let field = match kind {
        "build_domain" => "build_domain_id",
        "cfg_domain" => "cfg_domain_id",
        "rustc_invocation" => "invocation_id",
        "expansion_boundary" => "boundary_id",
        "expanded_item" => "expanded_item_id",
        "call_site" => "call_site_id",
        "call_edge" => "call_edge_id",
        "call_resolution" => "call_site_id",
        "effect_seed" => "effect_seed_id",
        "authority" => "authority_fact_id",
        "proof_blocker" => "blocker_id",
        other => {
            return Err(DbError::QueryConstruction(format!(
                "unknown proof fact kind {other}"
            )));
        }
    };
    let id = required_json_string(value, field)?;
    if kind == "call_resolution" {
        Ok(format!("resolution:{id}"))
    } else {
        Ok(id.to_string())
    }
}

fn validate_required_fields(value: &Value, kind: &str) -> Result<(), DbError> {
    match kind {
        "build_domain" => require_fields(
            value,
            &[
                "build_domain_id",
                "cargo_metadata_hash",
                "cargo_lock_hash",
                "package_id",
                "target_kind",
                "target_name",
                "target_root",
                "target_triple",
                "host_triple",
                "profile",
                "features_hash",
                "active_cfg_hash",
                "rustc_version",
                "extractor_version",
                "proof_policy_version",
            ],
        ),
        "cfg_domain" => require_fields(
            value,
            &[
                "cfg_domain_id",
                "build_domain_id",
                "active_cfg_hash",
                "status",
            ],
        ),
        "rustc_invocation" => require_fields(
            value,
            &[
                "invocation_id",
                "build_domain_id",
                "rustc_program",
                "rustc_version",
                "working_directory",
                "argument_vector_hash",
                "environment_hash",
                "status",
            ],
        ),
        "expansion_boundary" => {
            require_fields(
                value,
                &[
                    "boundary_id",
                    "build_domain_id",
                    "boundary_kind",
                    "expansion_state",
                ],
            )?;
            require_source_span(value)
        }
        "expanded_item" => {
            require_fields(
                value,
                &[
                    "expanded_item_id",
                    "boundary_id",
                    "build_domain_id",
                    "definition_id",
                ],
            )?;
            require_source_span(value)
        }
        "call_site" => {
            require_fields(value, &["call_site_id", "build_domain_id", "caller_def_id"])?;
            require_source_span(value)
        }
        "call_edge" => require_fields(
            value,
            &[
                "call_edge_id",
                "call_site_id",
                "caller_def_id",
                "resolution_state",
            ],
        ),
        "call_resolution" => require_fields(value, &["call_site_id", "resolution_state"]),
        "effect_seed" => {
            require_fields(
                value,
                &[
                    "effect_seed_id",
                    "call_site_id",
                    "effect_class",
                    "confidence",
                ],
            )?;
            require_json_bool(value, "blocker_if_unresolved")
        }
        "authority" => {
            require_fields(
                value,
                &[
                    "authority_fact_id",
                    "build_domain_id",
                    "authority_term",
                    "status",
                ],
            )?;
            require_source_span(value)
        }
        "proof_blocker" => require_fields(value, &["blocker_id", "reason", "status", "detail"]),
        other => Err(DbError::QueryConstruction(format!(
            "unknown proof fact kind {other}"
        ))),
    }
}

fn validate_enum_fields(value: &Value, kind: &str) -> Result<(), DbError> {
    validate_optional_enum(
        value,
        "evidence_use",
        &["proof_only", "navigation_only", "proof_and_navigation"],
    )?;
    validate_optional_enum(value, "status", &["admitted", "rejected", "blocked"])?;
    validate_optional_enum(
        value,
        "resolution_state",
        &[
            "resolved",
            "candidate_set",
            "ambiguous",
            "unresolved",
            "externally_summarized",
            "blocked",
        ],
    )?;
    validate_optional_enum(
        value,
        "expansion_state",
        &["expanded", "unresolved", "externally_summarized", "blocked"],
    )?;
    validate_optional_enum(
        value,
        "effect_class",
        &[
            "operating_system_process_create",
            "operating_system_process_replace",
            "operating_system_process_configure",
            "operating_system_process_wait",
            "operating_system_process_kill",
            "operating_system_process_reap",
            "async_task_spawn",
            "async_task_join",
            "async_task_abort",
            "authority_mint",
            "authority_retire",
            "authority_lock",
            "authority_unlock",
            "history_open_block",
            "history_seal_block",
            "history_append_block",
            "surface_measure",
            "surface_digest_compare",
            "durable_evidence_write",
            "durable_evidence_read",
            "external_summary_boundary",
        ],
    )?;
    validate_optional_enum(
        value,
        "authority_term",
        &[
            "parent_lineage",
            "crown_ruling",
            "authority_token_constructor",
            "successor",
            "predecessor_retired",
            "immutable_surface_digest_admission",
        ],
    )?;
    validate_optional_enum(value, "reason", PROOF_BLOCKER_REASONS)?;
    validate_optional_enum(value, "blocking_reason", PROOF_BLOCKER_REASONS)?;
    validate_optional_enum(
        value,
        "boundary_kind",
        &[
            "macro_rules_invocation",
            "proc_macro_derive",
            "proc_macro_attribute",
            "proc_macro_function",
            "build_script",
            "include",
            "external_summary",
        ],
    )?;
    if kind == "build_domain" {
        validate_optional_enum(
            value,
            "target_kind",
            &[
                "library",
                "binary",
                "test",
                "example",
                "benchmark",
                "build_script",
                "proc_macro",
            ],
        )?;
    }
    Ok(())
}

const PROOF_BLOCKER_REASONS: &[&str] = &[
    "macro_expansion_not_available",
    "proc_macro_summary_missing",
    "build_script_summary_missing",
    "external_dependency_summary_missing",
    "cfg_domain_not_materialized",
    "type_resolution_missing",
    "dynamic_dispatch_unbounded",
    "external_command_summary_missing",
    "build_script_execution_blocked",
    "proc_macro_execution_blocked",
    "canonical_identity_mismatch",
    "schema_version_mismatch",
    "authority_evidence_missing",
    "process_lifetime_evidence_missing",
    "rustc_invocation_evidence_missing",
];

fn validate_optional_enum(value: &Value, field: &str, allowed: &[&str]) -> Result<(), DbError> {
    let Some(raw) = value.get(field) else {
        return Ok(());
    };
    let Some(text) = raw.as_str() else {
        return Err(DbError::QueryConstruction(format!(
            "proof fact JSON field {field} is not a string"
        )));
    };
    if allowed.contains(&text) {
        Ok(())
    } else {
        Err(DbError::QueryConstruction(format!(
            "proof fact JSON field {field} has invalid value {text}"
        )))
    }
}

fn require_fields(value: &Value, fields: &[&str]) -> Result<(), DbError> {
    for field in fields {
        required_json_string(value, field)?;
    }
    Ok(())
}

fn require_source_span(value: &Value) -> Result<(), DbError> {
    let span = value.get("source_span").ok_or_else(|| {
        DbError::QueryConstruction("proof fact JSON missing object field source_span".to_string())
    })?;
    required_json_string(span, "file")?;
    for field in ["start_byte", "end_byte"] {
        if span.get(field).and_then(Value::as_u64).is_none() {
            return Err(DbError::QueryConstruction(format!(
                "proof fact source_span missing integer field {field}"
            )));
        }
    }
    for field in ["line_start", "line_end"] {
        if span.get(field).is_some() && span.get(field).and_then(Value::as_u64).is_none() {
            return Err(DbError::QueryConstruction(format!(
                "proof fact source_span field {field} is not an integer"
            )));
        }
    }
    Ok(())
}

fn require_json_bool(value: &Value, field: &str) -> Result<(), DbError> {
    value
        .get(field)
        .and_then(Value::as_bool)
        .map(|_| ())
        .ok_or_else(|| {
            DbError::QueryConstruction(format!("proof fact JSON missing bool field {field}"))
        })
}

#[derive(Debug, Clone)]
struct ProofFactRow {
    fact_id: String,
    kind: String,
    evidence_use: Option<String>,
    build_domain_id: Option<String>,
    call_site_id: Option<String>,
    call_edge_id: Option<String>,
    caller_def_id: Option<String>,
    callee_def_id: Option<String>,
    resolution_state: Option<String>,
    source_file: Option<String>,
    start_byte: Option<u32>,
    end_byte: Option<u32>,
    line_start: Option<u32>,
    line_end: Option<u32>,
    effect_class: Option<String>,
    blocker_reason: Option<String>,
    status: Option<String>,
    detail: Option<String>,
}

impl ProofFactRow {
    fn from_data_values(row: &[DataValue]) -> Result<Self, DbError> {
        Ok(Self {
            fact_id: required_string(row, 0, "fact_id")?,
            kind: required_string(row, 1, "kind")?,
            evidence_use: optional_string(row, 3),
            build_domain_id: optional_string(row, 4),
            call_site_id: optional_string(row, 5),
            call_edge_id: optional_string(row, 6),
            caller_def_id: optional_string(row, 7),
            callee_def_id: optional_string(row, 8),
            resolution_state: optional_string(row, 9),
            source_file: optional_string(row, 10),
            start_byte: optional_u32(row, 11, "start_byte")?,
            end_byte: optional_u32(row, 12, "end_byte")?,
            line_start: optional_u32(row, 13, "line_start")?,
            line_end: optional_u32(row, 14, "line_end")?,
            effect_class: optional_string(row, 15),
            blocker_reason: optional_string(row, 16),
            status: optional_string(row, 17),
            detail: optional_string(row, 18),
        })
    }

    fn matches_query(&self, query: &str) -> bool {
        if query.is_empty() {
            return true;
        }
        [
            Some(self.fact_id.as_str()),
            Some(self.kind.as_str()),
            self.evidence_use.as_deref(),
            self.build_domain_id.as_deref(),
            self.call_site_id.as_deref(),
            self.call_edge_id.as_deref(),
            self.caller_def_id.as_deref(),
            self.callee_def_id.as_deref(),
            self.resolution_state.as_deref(),
            self.source_file.as_deref(),
            self.effect_class.as_deref(),
            self.blocker_reason.as_deref(),
            self.status.as_deref(),
            self.detail.as_deref(),
        ]
        .into_iter()
        .flatten()
        .any(|value| value.to_ascii_lowercase().contains(query))
    }
}

impl From<ProofFactRow> for ProofGraphContextRow {
    fn from(row: ProofFactRow) -> Self {
        Self {
            fact_id: row.fact_id,
            kind: row.kind,
            call_site_id: row.call_site_id,
            caller_def_id: row.caller_def_id,
            callee_def_id: row.callee_def_id,
            evidence_use: row.evidence_use,
            blocker_reason: row.blocker_reason,
            source_file: row.source_file,
            line_start: row.line_start,
            detail: row.detail,
        }
    }
}

fn json_string(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(ToOwned::to_owned)
}

fn required_json_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, DbError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        DbError::QueryConstruction(format!("proof fact JSON missing string field {field}"))
    })
}

fn json_u32(value: &Value, field: &str) -> Result<Option<u32>, DbError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .map(u32::try_from)
        .transpose()
        .map_err(|_| DbError::QueryConstruction(format!("proof field {field} is out of u32 range")))
}

fn string_value(value: Option<String>) -> DataValue {
    value
        .map(|value| DataValue::Str(value.into()))
        .unwrap_or(DataValue::Null)
}

fn int_value(value: Option<u32>) -> DataValue {
    value
        .map(|value| DataValue::from(i64::from(value)))
        .unwrap_or(DataValue::Null)
}

fn optional_string(row: &[DataValue], index: usize) -> Option<String> {
    row.get(index)
        .and_then(DataValue::get_str)
        .map(ToOwned::to_owned)
}

fn required_string(row: &[DataValue], index: usize, field: &str) -> Result<String, DbError> {
    optional_string(row, index)
        .ok_or_else(|| DbError::Cozo(format!("proof graph row missing string field {field}")))
}

fn optional_u32(row: &[DataValue], index: usize, field: &str) -> Result<Option<u32>, DbError> {
    row.get(index)
        .and_then(DataValue::get_int)
        .map(u32::try_from)
        .transpose()
        .map_err(|_| DbError::Cozo(format!("proof graph field {field} is out of u32 range")))
}

fn is_idempotent_schema_error(message: &str) -> bool {
    message.contains("exists")
        || message.contains("duplicate")
        || message.contains("Duplicate")
        || message.contains("already")
        || message.contains("conflicts with an existing one")
        || message.to_ascii_lowercase().contains("conflict")
}
