use std::collections::BTreeMap;

use cozo::{DataValue, ScriptMutability};
use serde_json::Value;

use crate::{Database, DbError};

use super::{
    ProofGraphStore,
    projection::ProofFactProjection,
    rows::{ProofFactRow, int_value, string_value},
};

impl Database {
    pub(super) fn upsert_proof_fact_values(&self, values: &[Value]) -> Result<(), DbError> {
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

    pub(super) fn fetch_proof_rows(&self) -> Result<Vec<ProofFactRow>, DbError> {
        self.ensure_proof_graph_schema()?;
        let script = r#"
?[fact_id, kind, schema_version, evidence_use, build_domain_id, call_site_id, call_edge_id, caller_def_id, callee_def_id, resolution_state, source_file, start_byte, end_byte, line_start, line_end, effect_class, blocker_reason, status, detail, json] :=
    *proof_fact{ fact_id, kind, schema_version, evidence_use, build_domain_id, call_site_id, call_edge_id, caller_def_id, callee_def_id, resolution_state, source_file, start_byte, end_byte, line_start, line_end, effect_class, blocker_reason, status, detail, json }
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
