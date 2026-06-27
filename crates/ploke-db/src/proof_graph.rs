use serde_json::Value;

use crate::{Database, DbError};
mod call_projection;
mod invariants;
mod projection;
mod queries;
mod rows;
mod schema;
mod storage;

const PROOF_FACT_SCHEMA_VERSION: &str = "ploke-proof-facts.v1";

/// GraphRAG-visible row from the proof graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProofGraphContextRow {
    pub fact_id: String,
    pub kind: String,
    pub build_domain_id: Option<String>,
    pub call_site_id: Option<String>,
    pub call_edge_id: Option<String>,
    pub caller_def_id: Option<String>,
    pub callee_def_id: Option<String>,
    pub resolution_state: Option<String>,
    pub resolved_def_id: Option<String>,
    pub candidate_def_ids: Vec<String>,
    pub external_summary_id: Option<String>,
    pub boundary_id: Option<String>,
    pub boundary_kind: Option<String>,
    pub expanded_item_id: Option<String>,
    pub definition_id: Option<String>,
    pub target_kind: Option<String>,
    pub target_name: Option<String>,
    pub target_root: Option<String>,
    pub profile: Option<String>,
    pub rustc_version: Option<String>,
    pub proof_policy_version: Option<String>,
    pub cfg_domain_id: Option<String>,
    pub active_cfg_hash: Option<String>,
    pub invocation_id: Option<String>,
    pub rustc_program: Option<String>,
    pub working_directory: Option<String>,
    pub argument_vector_hash: Option<String>,
    pub environment_hash: Option<String>,
    pub effect_seed_id: Option<String>,
    pub confidence: Option<String>,
    pub blocker_if_unresolved: Option<bool>,
    pub authority_term: Option<String>,
    pub summary_class: Option<String>,
    pub artifact_hash: Option<String>,
    pub summary_version: Option<String>,
    pub review_method: Option<String>,
    pub scope_of_validity: Option<String>,
    pub allowed_effects: Vec<String>,
    pub required_containment: Option<String>,
    pub invalidation_conditions: Option<String>,
    pub evidence_use: Option<String>,
    pub source_file: Option<String>,
    pub start_byte: Option<u32>,
    pub end_byte: Option<u32>,
    pub line_start: Option<u32>,
    pub line_end: Option<u32>,
    pub effect_class: Option<String>,
    pub blocker_reason: Option<String>,
    pub status: Option<String>,
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
    fn has_proof_graph_facts(&self) -> Result<bool, DbError>;
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
        Database::ensure_proof_graph_schema(self)
    }

    fn upsert_proof_fact_values(&self, values: &[Value]) -> Result<(), DbError> {
        Database::upsert_proof_fact_values(self, values)
    }

    fn has_proof_graph_facts(&self) -> Result<bool, DbError> {
        Database::has_proof_graph_facts(self)
    }

    fn proof_symbol_lookup(&self, symbol: &str) -> Result<Vec<ProofGraphContextRow>, DbError> {
        Database::proof_symbol_lookup(self, symbol)
    }

    fn proof_graphrag_context(&self, query: &str) -> Result<Vec<ProofGraphContextRow>, DbError> {
        Database::proof_graphrag_context(self, query)
    }

    fn proof_domain_context(
        &self,
        build_domain_id: &str,
    ) -> Result<Vec<ProofGraphContextRow>, DbError> {
        Database::proof_domain_context(self, build_domain_id)
    }

    fn proof_checker_edges(&self) -> Result<Vec<ProofCheckerEdgeRow>, DbError> {
        Database::proof_checker_edges(self)
    }

    fn proof_blockers(&self) -> Result<Vec<ProofBlockerRow>, DbError> {
        Database::proof_blockers(self)
    }

    fn proof_source_provenance(
        &self,
        call_site_id: &str,
    ) -> Result<Option<ProofSourceProvenanceRow>, DbError> {
        Database::proof_source_provenance(self, call_site_id)
    }

    fn proof_invariant_findings(&self) -> Result<Vec<ProofInvariantFinding>, DbError> {
        Database::proof_invariant_findings(self)
    }
}
