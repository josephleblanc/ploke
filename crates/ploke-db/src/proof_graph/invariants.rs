use super::{ProofInvariantFinding, ProofInvariantStatus, rows::ProofFactRow};

mod authority;
mod blockers;
mod crown;
mod detached;
mod evidence;
mod scope;

use authority::{
    lineage_authority_blockers, lineage_authority_count, scoped_authority_blockers,
    scoped_authority_count,
};
use blockers::{active_proof_blocker_reasons, proof_blocker_reasons};
use evidence::{is_process_effect_class, is_proof_evidence};
use scope::{
    ProofScope, first_process_site, has_process_effect, lineage_keys, process_scopes,
    unscoped_process_effect_count,
};

pub(super) fn evaluate_proof_invariants(rows: &[ProofFactRow]) -> Vec<ProofInvariantFinding> {
    let mut findings = detached::evaluate_detached_process_invariants(rows);
    findings.extend(crown::evaluate_crown_ruling_invariants(rows));
    findings
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
