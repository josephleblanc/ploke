use serde_json::Value;
use uuid::Uuid;

use crate::{
    Database, DbError,
    call_graph::{CallCallerRow, CallContextRow},
};

use super::{PROOF_FACT_SCHEMA_VERSION, ProofGraphStore};

mod context;
mod facts;
mod source;

use context::{caller_context_row, validate_call_context};
use facts::{call_edge_facts, call_resolution_fact, call_site_fact};

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
}
