use std::collections::BTreeMap;

use cozo::ScriptMutability;

use crate::{Database, DbError};

impl Database {
    pub(super) fn ensure_proof_graph_schema(&self) -> Result<(), DbError> {
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
}

fn is_idempotent_schema_error(message: &str) -> bool {
    message.contains("exists")
        || message.contains("duplicate")
        || message.contains("Duplicate")
        || message.contains("already")
        || message.contains("conflicts with an existing one")
        || message.to_ascii_lowercase().contains("conflict")
}
