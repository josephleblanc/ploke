use super::*;

pub(super) fn fact_id(value: &Value, kind: &str) -> Result<String, DbError> {
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

pub(super) fn json_string(value: &Value, field: &str) -> Option<String> {
    value.get(field)?.as_str().map(ToOwned::to_owned)
}

pub(super) fn required_json_string<'a>(value: &'a Value, field: &str) -> Result<&'a str, DbError> {
    value.get(field).and_then(Value::as_str).ok_or_else(|| {
        DbError::QueryConstruction(format!("proof fact JSON missing string field {field}"))
    })
}

pub(super) fn json_u32(value: &Value, field: &str) -> Result<Option<u32>, DbError> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .map(u32::try_from)
        .transpose()
        .map_err(|_| DbError::QueryConstruction(format!("proof field {field} is out of u32 range")))
}
