use serde_json::Value;
use std::collections::HashSet;
use uuid::Uuid;

use crate::{Database, DbError, call_graph::CallContextRow};

use super::{PROOF_FACT_SCHEMA_VERSION, ProofGraphStore};

mod context;
mod facts;
mod source;

use context::validate_call_context;
use facts::{call_edge_facts, call_resolution_fact, call_site_fact};

fn append_call_proof_facts(
    values: &mut Vec<Value>,
    row: CallContextRow,
    build_domain_id: &str,
    source_file: &str,
) -> Result<(), DbError> {
    validate_call_context(&row)?;
    values.push(call_site_fact(&row, build_domain_id, source_file));
    values.extend(call_edge_facts(&row));
    values.push(call_resolution_fact(&row));
    Ok(())
}

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
            append_call_proof_facts(&mut values, row, build_domain_id, &source_file)?;
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
            append_call_proof_facts(&mut values, row, build_domain_id, &source_file)?;
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
                append_call_proof_facts(&mut values, row, build_domain_id, &source_file)?;
            }
        }

        for row in context.incoming {
            if !seen_sites.insert(row.site.id) {
                continue;
            }
            let source_file = self.source_file_for_owner(row.site.owner_id)?;
            append_call_proof_facts(&mut values, row, build_domain_id, &source_file)?;
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
