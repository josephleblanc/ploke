use std::collections::{BTreeMap, BTreeSet};

use super::{ProofFactRow, is_process_effect_class, is_proof_evidence};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub(super) struct ProofScope {
    pub(super) call_site_id: String,
    pub(super) build_domain_id: Option<String>,
}

pub(super) fn process_scopes(rows: &[ProofFactRow]) -> Vec<ProofScope> {
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

pub(super) fn unscoped_process_effect_count(rows: &[ProofFactRow]) -> usize {
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

pub(super) fn lineage_keys(rows: &[ProofFactRow]) -> BTreeSet<Option<String>> {
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

pub(super) fn has_process_effect(rows: &[ProofFactRow], build_domain_id: Option<&str>) -> bool {
    process_scopes(rows)
        .iter()
        .any(|scope| scope.build_domain_id.as_deref() == build_domain_id)
}

pub(super) fn first_process_site(
    rows: &[ProofFactRow],
    build_domain_id: Option<&str>,
) -> Option<String> {
    process_scopes(rows)
        .into_iter()
        .find(|scope| scope.build_domain_id.as_deref() == build_domain_id)
        .map(|scope| scope.call_site_id)
}

fn call_site_domains(rows: &[ProofFactRow]) -> BTreeMap<String, String> {
    rows.iter()
        .filter(|row| row.kind == "call_site")
        .filter_map(|row| Some((row.call_site_id.clone()?, row.build_domain_id.clone()?)))
        .collect()
}
