use std::collections::{BTreeMap, BTreeSet};

use crate::{Database, DbError};

use super::{
    ProofBlockerRow, ProofCheckerEdgeRow, ProofGraphContextRow, ProofInvariantFinding,
    ProofSourceProvenanceRow,
    invariants::{derived_proof_blocker_reason, evaluate_proof_invariants},
    rows::ProofFactRow,
};

impl Database {
    pub fn has_proof_graph_facts(&self) -> Result<bool, DbError> {
        let rows = self.raw_query("::relations")?;
        let registered = rows
            .rows
            .iter()
            .filter_map(|row| row.first().and_then(|value| value.get_str()))
            .collect::<BTreeSet<_>>();
        if !registered.contains("proof_fact") {
            return Ok(false);
        }

        let populated = self.raw_query(
            r#"?[fact_id] :=
                *proof_fact { fact_id }
            :limit 1"#,
        )?;
        Ok(!populated.rows.is_empty())
    }

    pub(super) fn proof_symbol_lookup(
        &self,
        symbol: &str,
    ) -> Result<Vec<ProofGraphContextRow>, DbError> {
        let symbol = symbol.to_ascii_lowercase();
        let rows = self.fetch_proof_rows()?;
        Ok(linked_context_rows(rows, |row| row.matches_query(&symbol)))
    }

    pub(super) fn proof_graphrag_context(
        &self,
        query: &str,
    ) -> Result<Vec<ProofGraphContextRow>, DbError> {
        let query = query.to_ascii_lowercase();
        let rows = self.fetch_proof_rows()?;
        Ok(linked_context_rows(rows, |row| row.matches_query(&query)))
    }

    pub(super) fn proof_domain_context(
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

    pub(super) fn proof_checker_edges(&self) -> Result<Vec<ProofCheckerEdgeRow>, DbError> {
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

    pub(super) fn proof_blockers(&self) -> Result<Vec<ProofBlockerRow>, DbError> {
        let rows = self.fetch_proof_rows()?;
        Ok(rows
            .iter()
            .filter_map(explicit_blocker_row)
            .chain(
                rows.iter()
                    .filter_map(|row| derived_blocker_row(row, &rows)),
            )
            .collect())
    }

    pub(super) fn proof_source_provenance(
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

    pub(super) fn proof_invariant_findings(&self) -> Result<Vec<ProofInvariantFinding>, DbError> {
        let rows = self.fetch_proof_rows()?;
        Ok(evaluate_proof_invariants(&rows))
    }
}

fn explicit_blocker_row(row: &ProofFactRow) -> Option<ProofBlockerRow> {
    if row.kind != "proof_blocker" {
        return None;
    }
    Some(ProofBlockerRow {
        blocker_id: row.fact_id.clone(),
        reason: row.blocker_reason.clone()?,
        status: row.status.clone()?,
        build_domain_id: row.build_domain_id.clone(),
        call_site_id: row.call_site_id.clone(),
        detail: row.detail.clone()?,
    })
}

fn derived_blocker_row(row: &ProofFactRow, rows: &[ProofFactRow]) -> Option<ProofBlockerRow> {
    let reason = derived_proof_blocker_reason(row, rows)?;
    let status = row
        .status
        .clone()
        .or_else(|| row.resolution_state.clone())
        .unwrap_or_else(|| "blocked".to_string());
    Some(ProofBlockerRow {
        blocker_id: row.fact_id.clone(),
        detail: row.detail.clone().unwrap_or_else(|| reason.clone()),
        reason,
        status,
        build_domain_id: row.build_domain_id.clone(),
        call_site_id: row.call_site_id.clone(),
    })
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
