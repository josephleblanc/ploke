use std::collections::{BTreeMap, BTreeSet};

use crate::{Database, DbError};

use super::{
    ProofBlockerRow, ProofCheckerEdgeRow, ProofGraphContextRow, ProofInvariantFinding,
    ProofSourceProvenanceRow,
    invariants::{
        derived_proof_blocker_reason, derived_proof_blocker_reasons, evaluate_proof_invariants,
    },
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
            .iter()
            .filter(|row| row.kind == "call_edge")
            .flat_map(|row| {
                let Some(call_site_id) = row.call_site_id.clone() else {
                    return Vec::new();
                };
                let mut blocker_reasons = blockers_by_site
                    .get(&call_site_id)
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .collect::<BTreeSet<_>>();
                if let Some(reason) = derived_proof_blocker_reason(row, &rows) {
                    blocker_reasons.insert(reason);
                }
                if blocker_reasons.is_empty() {
                    blocker_reasons.insert(String::new());
                }
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
            .chain(rows.iter().flat_map(|row| derived_blocker_rows(row, &rows)))
            .collect())
    }

    pub(crate) fn proof_blockers_for_call_sites(
        &self,
        call_site_ids: &BTreeSet<String>,
    ) -> Result<Vec<ProofBlockerRow>, DbError> {
        if call_site_ids.is_empty() {
            return Ok(Vec::new());
        }

        let rows = self.fetch_proof_rows()?;
        let scoped_rows = rows
            .iter()
            .filter(|row| {
                row.call_site_id
                    .as_deref()
                    .is_some_and(|id| call_site_ids.contains(id))
            })
            .collect::<Vec<_>>();

        Ok(scoped_rows
            .iter()
            .filter_map(|row| explicit_blocker_row(row))
            .chain(
                scoped_rows
                    .iter()
                    .flat_map(|row| derived_blocker_rows(row, &rows)),
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

    pub(crate) fn admitted_effect_policy_allowed_effects_for_definition(
        &self,
        definition_id: &str,
    ) -> Result<Option<Vec<String>>, DbError> {
        if definition_id.is_empty() {
            return Err(DbError::QueryConstruction(
                "effect policy lookup requires non-empty definition_id".to_string(),
            ));
        }

        let mut policies = self
            .fetch_proof_rows()?
            .into_iter()
            .filter(|row| row.kind == "effect_policy")
            .filter(|row| row.definition_id.as_deref() == Some(definition_id))
            .filter(|row| row.status.as_deref() == Some("admitted"))
            .filter(|row| {
                matches!(
                    row.evidence_use.as_deref(),
                    Some("proof_only" | "proof_and_navigation")
                )
            })
            .collect::<Vec<_>>();
        policies.sort_by(|left, right| left.fact_id.cmp(&right.fact_id));

        match policies.len() {
            0 => Ok(None),
            1 => {
                let policy = policies.remove(0);
                if policy.allowed_effects.is_empty() {
                    return Err(DbError::Cozo(format!(
                        "admitted effect_policy {} has no allowed_effects",
                        policy.fact_id
                    )));
                }
                Ok(Some(policy.allowed_effects))
            }
            _ => Err(DbError::Cozo(format!(
                "multiple admitted effect_policy proof rows for definition {definition_id}"
            ))),
        }
    }

    pub(crate) fn proof_build_domain_rows_for_definition(
        &self,
        definition_id: &str,
    ) -> Result<Vec<ProofGraphContextRow>, DbError> {
        if definition_id.is_empty() {
            return Err(DbError::QueryConstruction(
                "build domain lookup requires non-empty definition_id".to_string(),
            ));
        }

        let rows = self.fetch_proof_rows()?;
        let build_domains = rows
            .iter()
            .filter(|row| proof_row_links_definition(row, definition_id))
            .filter_map(|row| row.build_domain_id.clone())
            .collect::<BTreeSet<_>>();
        if build_domains.is_empty() {
            return Ok(Vec::new());
        }

        let all_rows = rows.clone();
        let mut domains = rows
            .into_iter()
            .filter(|row| {
                row.kind == "build_domain"
                    && row
                        .build_domain_id
                        .as_ref()
                        .is_some_and(|id| build_domains.contains(id))
            })
            .flat_map(|row| proof_context_rows(row, &all_rows))
            .collect::<Vec<_>>();
        domains.sort_by(|left, right| {
            (
                left.build_domain_id.as_deref(),
                left.blocker_reason.as_deref(),
                left.fact_id.as_str(),
            )
                .cmp(&(
                    right.build_domain_id.as_deref(),
                    right.blocker_reason.as_deref(),
                    right.fact_id.as_str(),
                ))
        });
        Ok(domains)
    }

    pub(crate) fn proof_entrypoint_summary_rows_for_definition(
        &self,
        definition_id: &str,
    ) -> Result<Vec<ProofGraphContextRow>, DbError> {
        if definition_id.is_empty() {
            return Err(DbError::QueryConstruction(
                "entrypoint summary lookup requires non-empty definition_id".to_string(),
            ));
        }

        let rows = self.fetch_proof_rows()?;
        let all_rows = rows.clone();
        let mut summaries = rows
            .into_iter()
            .filter(|row| row.kind == "entrypoint_summary")
            .filter(|row| proof_row_links_definition(row, definition_id))
            .flat_map(|row| proof_context_rows(row, &all_rows))
            .collect::<Vec<_>>();
        summaries.sort_by(|left, right| {
            (
                left.fact_id.as_str(),
                left.build_domain_id.as_deref(),
                left.blocker_reason.as_deref(),
            )
                .cmp(&(
                    right.fact_id.as_str(),
                    right.build_domain_id.as_deref(),
                    right.blocker_reason.as_deref(),
                ))
        });
        Ok(summaries)
    }
}

fn proof_row_links_definition(row: &ProofFactRow, definition_id: &str) -> bool {
    row.definition_id.as_deref() == Some(definition_id)
        || row.caller_def_id.as_deref() == Some(definition_id)
        || row.callee_def_id.as_deref() == Some(definition_id)
        || row.resolved_def_id.as_deref() == Some(definition_id)
        || row
            .candidate_def_ids
            .iter()
            .any(|candidate| candidate == definition_id)
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

fn derived_blocker_rows(row: &ProofFactRow, rows: &[ProofFactRow]) -> Vec<ProofBlockerRow> {
    let reasons = derived_proof_blocker_reasons(row, rows);
    let status = row
        .status
        .clone()
        .or_else(|| row.resolution_state.clone())
        .unwrap_or_else(|| "blocked".to_string());
    reasons
        .into_iter()
        .map(|reason| ProofBlockerRow {
            blocker_id: row.fact_id.clone(),
            detail: row.detail.clone().unwrap_or_else(|| reason.clone()),
            reason,
            status: status.clone(),
            build_domain_id: row.build_domain_id.clone(),
            call_site_id: row.call_site_id.clone(),
        })
        .collect()
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
    let linked_external_summaries = rows
        .iter()
        .filter(|row| {
            is_seed(row)
                || row
                    .call_site_id
                    .as_ref()
                    .is_some_and(|id| linked_call_sites.contains(id))
        })
        .filter_map(|row| row.external_summary_id.clone())
        .collect::<BTreeSet<_>>();
    let linked_boundaries = rows
        .iter()
        .filter(|row| {
            is_seed(row)
                || row
                    .call_site_id
                    .as_ref()
                    .is_some_and(|id| linked_call_sites.contains(id))
                || row
                    .external_summary_id
                    .as_ref()
                    .is_some_and(|id| linked_external_summaries.contains(id))
        })
        .filter_map(|row| row.boundary_id.clone())
        .collect::<BTreeSet<_>>();
    let all_rows = rows.clone();

    rows.into_iter()
        .filter(|row| {
            is_seed(row)
                || row
                    .call_site_id
                    .as_ref()
                    .is_some_and(|id| linked_call_sites.contains(id))
                || (row.kind == "external_summary"
                    && row
                        .external_summary_id
                        .as_ref()
                        .is_some_and(|id| linked_external_summaries.contains(id)))
                || row
                    .boundary_id
                    .as_ref()
                    .is_some_and(|id| linked_boundaries.contains(id))
        })
        .flat_map(|row| proof_context_rows(row, &all_rows))
        .collect()
}

fn proof_context_rows(row: ProofFactRow, rows: &[ProofFactRow]) -> Vec<ProofGraphContextRow> {
    if row.blocker_reason.is_some() {
        return vec![ProofGraphContextRow::from(row)];
    }

    let reasons = derived_proof_blocker_reasons(&row, rows);
    if reasons.is_empty() {
        return vec![ProofGraphContextRow::from(row)];
    }

    reasons
        .into_iter()
        .map(|reason| {
            let mut context = ProofGraphContextRow::from(row.clone());
            context.blocker_reason = Some(reason);
            context
        })
        .collect()
}
